use chrono::{DateTime, Local};
use log::warn;

use crate::{
    command::AttributeType,
    error::Result,
    gamestate::{items::PotionType, unlockables::HabitatType},
    response::Vals,
};

/// A possible reward on the calendar
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CalendarReward {
    /// Note that this is technically correct, but at low levels, these are
    /// often overwritten to silver
    // FIXME: figure out how exactly
    pub typ: CalendarRewardType,
    /// The amount of the type this reward yielded
    pub amount: i64,
}

/// The type of reward gainable by collecting the calendar
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CalendarRewardType {
    /// Silver coins.
    Silver,
    /// Mushrooms currency.
    Mushrooms,
    /// Experience points.
    Experience,
    /// Wood fortress resource.
    Wood,
    /// Stone fortress resource.
    Stone,
    /// Souls underworld resource.
    Souls,
    /// Arcane splinters blacksmith resource.
    Arcane,
    /// Runes resource.
    Runes,
    /// An equipment item.
    Item,
    /// Direct attribute increase.
    Attribute(AttributeType),
    /// Pet fruit of a specific habitat.
    Fruit(HabitatType),
    /// Level reward.
    Level,
    /// A potion.
    Potion(PotionType),
    /// Ten quicksand glasses / hourglasses.
    TenQuicksandGlasses,
    /// Level up reward.
    LevelUp,
    /// An unknown calendar reward type.
    #[default]
    Unknown,
}

impl CalendarReward {
    /// Parses a calendar reward from a 2-value window: `[typ, amount]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<CalendarReward> {
        let typ = data.i64(0)?;
        let amount = data.i64(1)?;
        let typ = match typ {
            1 => CalendarRewardType::Silver,
            2 => CalendarRewardType::Mushrooms,
            3 => CalendarRewardType::Experience,
            4 => CalendarRewardType::Wood,
            5 => CalendarRewardType::Stone,
            6 => CalendarRewardType::Souls,
            7 => CalendarRewardType::Arcane,
            8 => CalendarRewardType::Runes,
            10 => CalendarRewardType::Item,
            11 => CalendarRewardType::Attribute(AttributeType::Strength),
            12 => CalendarRewardType::Attribute(AttributeType::Dexterity),
            13 => CalendarRewardType::Attribute(AttributeType::Intelligence),
            14 => CalendarRewardType::Attribute(AttributeType::Constitution),
            15 => CalendarRewardType::Attribute(AttributeType::Luck),
            x @ 16..=20 => {
                if let Some(typ) = HabitatType::from_typ_id(x - 15) {
                    CalendarRewardType::Fruit(typ)
                } else {
                    warn!("unknown pet class in c rewards");
                    CalendarRewardType::Unknown
                }
            }
            21 => CalendarRewardType::LevelUp,
            22 => CalendarRewardType::Potion(PotionType::EternalLife),
            23 => CalendarRewardType::TenQuicksandGlasses,
            24 => CalendarRewardType::Potion(PotionType::Strength),
            25 => CalendarRewardType::Potion(PotionType::Dexterity),
            26 => CalendarRewardType::Potion(PotionType::Intelligence),
            27 => CalendarRewardType::Potion(PotionType::Constitution),
            28 => CalendarRewardType::Potion(PotionType::Luck),
            x => {
                warn!("Unknown calendar reward: {x}");
                CalendarRewardType::Unknown
            }
        };

        Ok(CalendarReward { typ, amount })
    }
}

/// Grants rewards once a day
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[doc(alias = "DailyLoginBonus")]
pub struct Calendar {
    /// The amount of times the calendar has been collected already.
    /// `rewards[collected]` will give you the position in the rewards you will
    /// get for collecting today (if you can)
    pub collected: usize,
    /// The things you can get from the calendar
    pub rewards: Vec<CalendarReward>,
    /// The time at which the calendar door will be unlocked. If this is in the
    /// past, that means it is available to open
    pub next_possible: Option<DateTime<Local>>,
}

impl Calendar {
    /// Updates the available calendar rewards from a repeating
    /// `[reward_type, amount]` list.
    pub(crate) fn update_rewards(&mut self, data: &Vals<'_>) -> Result<()> {
        self.rewards.clear();
        for chunk in data.chunks(2) {
            self.rewards.push(CalendarReward::parse(&chunk)?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

    #[test]
    fn test_calendar_reward_parse() {
        crate::parse_test!(CalendarReward("12/1"), {
            typ: CalendarRewardType::Attribute(AttributeType::Dexterity),
            amount: 1,
        });

        crate::parse_test!(CalendarReward("21/1"), {
            typ: CalendarRewardType::LevelUp,
            amount: 1,
        });

        let data = Vals::from("16/5");
        let reward = CalendarReward::parse(&data).unwrap();
        assert_eq!(reward.typ, CalendarRewardType::Fruit(HabitatType::Shadow));
        assert_eq!(reward.amount, 5);
        assert_all_read(&data);
    }

    #[test]
    fn test_calendar_update_rewards() {
        let raw = "12/1/8/1/3/1/25/1/5/1/2/1/3/2/1/1/24/1/18/5/6/1/22/1/7/1/6/\
                   2/8/2/22/2/5/2/2/2/3/3/21/1";
        let mut calendar = Calendar::default();
        let data = Vals::from(raw);
        calendar.update_rewards(&data).unwrap();
        assert_eq!(calendar.rewards.len(), 20);
        assert_eq!(
            calendar.rewards[0],
            CalendarReward {
                typ: CalendarRewardType::Attribute(AttributeType::Dexterity),
                amount: 1,
            }
        );
        assert_eq!(
            calendar.rewards[19],
            CalendarReward {
                typ: CalendarRewardType::LevelUp,
                amount: 1,
            }
        );
        assert_all_read(&data);
    }
}
