//! Tavern activities, quests, dice game, toilet, and expeditions.

/// The dice game played with the gambler in the tavern.
pub mod dice;
/// Expedition events, crossroads choices, and active expeditions.
pub mod expeditions;
/// Tavern quests, adventure lengths, and quest rewards.
pub mod quest;
/// The arcane toilet of the gods in the tavern.
pub mod toilet;

pub use dice::*;
pub use expeditions::*;
pub use quest::*;
pub use toilet::*;

use crate::{
    error::Result,
    gamestate::{
        ExpeditionSetting, ServerTime,
        items::{ITEM_PARSE_LEN, Item},
    },
    response::Vals,
};

/// Anything related to things you can do in the tavern
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tavern {
    /// All the available quests
    pub quests: [Quest; 3],
    /// How many seconds the character still has left to do adventures
    #[doc(alias = "alu")]
    pub thirst_for_adventure_sec: u32,
    /// Whether or not skipping with mushrooms is allowed
    pub mushroom_skip_allowed: bool,
    /// The amount of beers we already drank today
    pub beer_drunk: u8,
    /// The amount of quicksand glasses we have and can use to skip quests
    pub quicksand_glasses: u32,
    /// The thing the player is currently doing (either questing or working)
    pub current_action: CurrentAction,
    /// The amount of silver earned per hour working the guard jobs
    pub guard_wage: u64,
    /// The toilet, if it has been unlocked
    pub toilet: Option<Toilet>,
    /// The dice game you can play with the weird guy in the tavern
    pub dice_game: DiceGame,
    /// Information about everything related to expeditions
    pub expeditions: ExpeditionsEvent,
    /// Decides if you can go on expeditions, or quests, when this event is
    /// currently ongoing
    pub questing_preference: ExpeditionSetting,
    /// The result of playing the shell game
    pub gamble_result: Option<GambleResult>,
    /// Total amount of beers you can drink today
    pub beer_max: u8,
}

impl Tavern {
    /// Checks if the player is currently doing anything. Note that this may
    /// change between calls, as expeditions finish without sending any collect
    /// commands. In most cases you should match on the `current_action`
    /// yourself and collect/wait, if necessary, but if you want a quick sanity
    /// check somewhere, to make sure you are idle, this is the function for you
    #[must_use]
    pub fn is_idle(&self) -> bool {
        match self.current_action {
            CurrentAction::Idle => true,
            CurrentAction::Expedition => self.expeditions.active.is_none(),
            _ => false,
        }
    }

    /// Gives you the same tasks, that the person in the tavern would present
    /// you with. When expeditions are available and they are not disabled by
    /// the `questing_preference`, they will be shown. Otherwise you will get
    /// quests
    #[must_use]
    pub fn available_tasks(&self) -> AvailableTasks<'_> {
        if self.questing_preference == ExpeditionSetting::PreferExpeditions
            && self.expeditions.is_event_ongoing()
        {
            AvailableTasks::Expeditions(&self.expeditions.available)
        } else {
            AvailableTasks::Quests(&self.quests)
        }
    }

    /// The expedition/questing setting can only be changed, before any
    /// alu/thirst for adventure is used that day
    #[must_use]
    pub fn can_change_questing_preference(&self) -> bool {
        self.thirst_for_adventure_sec == 6000 && self.beer_drunk == 0
    }

    /// Updates the available quests from a 23-value `adventure` response.
    pub(crate) fn update_quests(&mut self, data: &Vals<'_>) -> Result<()> {
        data.ignore(0..2, "flavor / quest giver");
        for (chunk, quest) in data.window(2..23).chunks(7).zip(&mut self.quests)
        {
            quest.update(&chunk)?;
        }
        Ok(())
    }

    /// Updates quest offer items from a `questofferitems` response.
    pub(crate) fn update_quest_items(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        for (chunk, quest) in data.chunks(ITEM_PARSE_LEN).zip(&mut self.quests)
        {
            quest.item = Item::parse(&chunk, server_time).ok().flatten();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tavern_update_quests() {
        let mut tavern = Tavern::default();
        let raw = "198/198/366/3/-65/1/600/2239844/98169000/367/4/-66/2/700/\
                   2239845/98169001/368/5/-67/3/800/2239846/98169002";
        let data = Vals::from(raw);
        tavern.update_quests(&data).unwrap();
        assert_eq!(tavern.quests[0].monster_id, 65);
        assert_eq!(tavern.quests[0].location_id, Location::SprawlingJungle);
        assert_eq!(tavern.quests[0].base_length, 600);
        assert_eq!(tavern.quests[1].monster_id, 66);
        assert_eq!(tavern.quests[1].location_id, Location::SkullIsland);
        assert_eq!(tavern.quests[2].monster_id, 67);
        assert_eq!(tavern.quests[2].location_id, Location::EvernightForest);
        data.assert_all_read();
    }

    #[test]
    fn test_tavern_update_quest_items() {
        let mut tavern = Tavern::default();
        let item_raw = "1/0/0/1010/0/149/265/5/4/0/108/72/0/83940/10/0/0/45/0";
        let empty_item = "0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0";
        let raw = format!("{item_raw}/{empty_item}/{empty_item}");
        let data = Vals::from(raw.as_str());
        tavern.update_quest_items(&data, ServerTime::default());
        assert!(tavern.quests[0].item.is_some());
        assert_eq!(tavern.quests[0].item.as_ref().unwrap().price, 83940);
        assert!(tavern.quests[1].item.is_none());
        assert!(tavern.quests[2].item.is_none());
        data.assert_all_read();
    }
}
