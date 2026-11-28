use chrono::{DateTime, Local};
use log::{error, warn};
use num_derive::FromPrimitive;

use super::AvailableExpedition;
use crate::{
    error::Result,
    gamestate::{ServerTime, items::Item},
    response::Vals,
};

/// One of the three possible quests in the tavern
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quest {
    /// The length of this quest in sec (without item enchantment)
    pub base_length: u32,
    /// The silver reward for this quest (without item enchantment)
    pub base_silver: u32,
    /// The xp reward for this quest (without item enchantment)
    pub base_experience: u32,
    /// The item reward for this quest
    pub item: Option<Item>,
    /// The place where this quest takes place. Useful for the scrapbook
    pub location_id: Location,
    /// The enemy you fight in this quest. Useful for the scrapbook
    pub monster_id: u16,
}

/// The background/location for a quest, or another activity
#[derive(Debug, Default, Clone, PartialEq, Eq, Copy, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Location {
    /// Sprawling Jungle location.
    #[default]
    SprawlingJungle = 1,
    /// Skull Island location.
    SkullIsland,
    /// Evernight Forest location.
    EvernightForest,
    /// Stumble Steppe location.
    StumbleSteppe,
    /// Shadowrock Mountain location.
    ShadowrockMountain,
    /// Split Canyon location.
    SplitCanyon,
    /// Black Water Swamp location.
    BlackWaterSwamp,
    /// Flooded Caldwell location.
    FloodedCaldwell,
    /// Tusk Mountain location.
    TuskMountain,
    /// Moldy Forest location.
    MoldyForest,
    /// Nevermoor location.
    Nevermoor,
    /// Busted Lands location.
    BustedLands,
    /// Erogenion location.
    Erogenion,
    /// Magmaron location.
    Magmaron,
    /// Sunburn Desert location.
    SunburnDesert,
    /// Gnarogrim location.
    Gnarogrim,
    /// Northrunt location.
    Northrunt,
    /// Black Forest location.
    BlackForest,
    /// Maerwynn location.
    Maerwynn,
    /// Plains of Oz'Korr location.
    PlainsOfOzKorr,
    /// Rotten Lands location.
    RottenLands,
}

impl Quest {
    /// Checks if this is a red quest, which means a special enemy + extra
    /// rewards
    #[must_use]
    pub fn is_red(&self) -> bool {
        matches!(self.monster_id, 139 | 145 | 148 | 152 | 155 | 157)
    }

    /// Updates this quest from a 7-value chunk: `[flavor1, flavor2,
    /// monster_id, location_id, length, xp, silver]`.
    pub(crate) fn update(&mut self, data: &Vals<'_>) -> Result<()> {
        data.ignore(0..2, "flavor / quest text");
        let raw_monster = data.i64(2)?;
        self.monster_id = u16::try_from(-raw_monster).unwrap_or_else(|_| {
            warn!("Invalid quest monster id: {raw_monster}");
            0
        });
        self.location_id = data.enum_or(3, Location::default());
        self.base_length = data.u32(4)?;
        self.base_experience = data.u32(5)?;
        self.base_silver = data.u32(6)?;
        Ok(())
    }
}

/// The thing the player is currently doing
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CurrentAction {
    /// The character is not doing anything and can basically do anything
    #[default]
    Idle,
    /// The character is working on guard duty right now. If `busy_until <
    /// Local::now()`, you can send a `WorkFinish` command
    CityGuard {
        /// The total amount of hours the character has decided to work
        hours: u8,
        /// The time at which the guard job will be over
        busy_until: DateTime<Local>,
    },
    /// The character is doing a quest right now. If `busy_until <
    /// Local::now()` you can send a `FinishQuest` command
    Quest {
        /// 0-2 index into tavern quest array
        quest_idx: u8,
        /// The time, at which the quest can be finished
        busy_until: DateTime<Local>,
    },
    /// The player is currently doing an expedition. This can be wrong, if the
    /// last expedition timer elapsed since sending the last request
    Expedition,
    /// The character is not able to do something, but we do not know what.
    /// Most likely something from a new update
    Unknown(Option<DateTime<Local>>),
}

impl CurrentAction {
    /// Parses a current action record `[action_id, action_sec, busy_until]`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<Self> {
        let id = data.i64_or(0, 0);
        let sec = data.i64_or(1, 0);
        // Sometimes the game "forgets" when an action was supposed to end.
        // This only happens when the action is very old, so falling back to a
        // busy_until that is super old is fine here
        let busy_until = data.server_time(2, server_time)?.unwrap_or_default();
        let action = match id {
            0 => CurrentAction::Idle,
            1 => CurrentAction::CityGuard {
                hours: u8::try_from(sec).unwrap_or_else(|_| {
                    warn!("Invalid city guard hours: {sec}");
                    10
                }),
                busy_until,
            },
            2 => CurrentAction::Quest {
                quest_idx: u8::try_from(sec).unwrap_or_else(|_| {
                    warn!("Invalid quest index: {sec}");
                    0
                }),
                busy_until,
            },
            4 => CurrentAction::Expedition,
            _ => {
                error!("Unknown action id combination: {id}, {busy_until:?}");
                CurrentAction::Unknown(Some(busy_until))
            }
        };
        Ok(action)
    }
}

/// The tasks you will presented with, when clicking the person in the tavern.
/// Make sure you are not currently busy and have enough ALU/thirst of adventure
/// before trying to start them
#[derive(Debug, Clone)]
pub enum AvailableTasks<'a> {
    /// Available quests.
    Quests(&'a [Quest; 3]),
    /// Available expeditions.
    Expeditions(&'a [AvailableExpedition]),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quest_update() {
        let mut quest = Quest::default();
        let raw = "366/3/-65/1/600/2239844/98169000";
        let data = Vals::from(raw);
        quest.update(&data).unwrap();
        data.assert_all_read();
        assert_eq!(quest.monster_id, 65);
        assert_eq!(quest.location_id, Location::SprawlingJungle);
        assert_eq!(quest.base_length, 600);
        assert_eq!(quest.base_experience, 2_239_844);
        assert_eq!(quest.base_silver, 98_169_000);
        assert!(!quest.is_red());
    }

    #[test]
    fn test_red_quest() {
        let mut quest = Quest::default();
        let raw = "366/3/-139/1/600/2239844/98169000";
        let data = Vals::from(raw);
        quest.update(&data).unwrap();
        data.assert_all_read();
        assert_eq!(quest.monster_id, 139);
        assert!(quest.is_red());
    }

    #[test]
    fn test_current_action_parse() {
        let raw = "1/2/1700000000";
        let data = Vals::from(raw);
        let action =
            CurrentAction::parse(&data, ServerTime::default()).unwrap();
        match action {
            CurrentAction::CityGuard { hours, .. } => {
                assert_eq!(hours, 2);
            }
            _ => panic!("expected CityGuard action"),
        }
        assert_eq!(data.unaccessed_indices(), Vec::<usize>::new());

        let raw_quest = "2/1/1700000000";
        let data_quest = Vals::from(raw_quest);
        let action_quest =
            CurrentAction::parse(&data_quest, ServerTime::default()).unwrap();
        match action_quest {
            CurrentAction::Quest { quest_idx, .. } => {
                assert_eq!(quest_idx, 1);
            }
            _ => panic!("expected Quest action"),
        }
        assert_eq!(data_quest.unaccessed_indices(), Vec::<usize>::new());

        let raw_idle = "0/0/0";
        let data_idle = Vals::from(raw_idle);
        let action_idle =
            CurrentAction::parse(&data_idle, ServerTime::default()).unwrap();
        assert_eq!(action_idle, CurrentAction::Idle);
        assert_eq!(data_idle.unaccessed_indices(), Vec::<usize>::new());

        let raw_exp = "4/0/0";
        let data_exp = Vals::from(raw_exp);
        let action_exp =
            CurrentAction::parse(&data_exp, ServerTime::default()).unwrap();
        assert_eq!(action_exp, CurrentAction::Expedition);
        assert_eq!(data_exp.unaccessed_indices(), Vec::<usize>::new());
    }
}
