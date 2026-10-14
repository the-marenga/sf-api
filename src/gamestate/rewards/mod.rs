//! Rewards, tasks, daily bonuses, and server events.

/// Daily login calendar and bonuses.
pub mod calendar;
/// Weekend server events and event buffs.
pub mod events;
/// Daily and event tasks from the goblin gleeman.
pub mod tasks;
/// Dr. Abawuwu's lucky wheel of fortune.
pub mod wheel;

pub use calendar::*;
pub use events::*;
use log::warn;
pub use tasks::*;
pub use wheel::*;

use crate::{
    error::Result,
    gamestate::{Mount, unlockables::HabitatType},
    response::Vals,
};

/// Everything, that changes over time
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimedSpecials {
    /// All of the events active in the tavern
    pub events: Events,
    /// The stuff you can do for bonus rewards
    pub tasks: Tasks,
    /// Grants rewards once a day
    pub calendar: Calendar,
    /// Dr. Abawuwu's wheel
    pub wheel: Wheel,
    /// The daily reward you can collect before christmas
    pub advent_calendar: Option<Reward>,
}

impl TimedSpecials {
    /// Updates the advent calendar reward from an `adventscalendar` response.
    pub(crate) fn update_advent_calendar(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<()> {
        if data.is_empty() || data.u32_or(0, 0) == 0 {
            self.advent_calendar = None;
            data.ignore_all("empty or zero advent calendar reward");
            return Ok(());
        }
        self.advent_calendar = Some(Reward::parse(data)?);
        Ok(())
    }
}

/// Something you can unlock for completing tasks
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RewardChest {
    /// Whether or not this chest has been unlocked
    pub opened: bool,
    /// The amount of points required to open this chest
    pub required_points: u32,
    /// The things you will get for opening this chest
    pub rewards: Vec<Reward>,
}

/// The reward for opening a chest
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Reward {
    /// The type of the thing you are getting
    pub typ: RewardType,
    /// The amount of `typ` you get
    pub amount: u64,
}

/// The type of reward contained in a reward chest or advent calendar door
#[derive(Debug, Default, Clone, PartialEq, Eq, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RewardType {
    /// Points for the Hellevator event.
    HellevatorPoints,
    /// Key cards for the Hellevator event.
    HellevatorCards,
    /// Mushrooms currency.
    Mushrooms,
    /// Silver coins.
    Silver,
    /// Lucky coins for the wheel.
    LuckyCoins,
    /// Wood fortress resource.
    Wood,
    /// Stone fortress resource.
    Stone,
    /// Arcane splinters blacksmith resource.
    Arcane,
    /// Metal blacksmith resource.
    Metal,
    /// Souls underworld resource.
    Souls,
    /// Pet fruit of a specific habitat.
    Fruit(HabitatType),
    /// A legendary gem.
    LegendaryGem,
    /// Gold fidget spinner.
    GoldFidget,
    /// Silver fidget spinner.
    SilverFidget,
    /// Bronze fidget spinner.
    BronzeFidget,
    /// A gem.
    Gem,
    /// A fruit basket.
    FruitBasket,
    /// Experience points.
    XP,
    /// A pet egg.
    Egg,
    /// Quicksand glasses / hourglasses.
    QuicksandGlass,
    /// Hall of Fame honor.
    Honor,
    /// Tavern beer.
    Beer,
    /// A portrait frame.
    Frame,
    /// A rented mount.
    Mount(Mount),
    /// An unknown reward type.
    #[default]
    Unknown,
}

impl RewardType {
    #[must_use]
    pub(crate) fn parse(val: i64) -> RewardType {
        match val {
            1 => RewardType::HellevatorPoints,
            2 => RewardType::HellevatorCards,
            3 => RewardType::Mushrooms,
            4 => RewardType::Silver,
            5 => RewardType::LuckyCoins,
            6 => RewardType::Wood,
            7 => RewardType::Stone,
            8 => RewardType::Arcane,
            9 => RewardType::Metal,
            10 => RewardType::Souls,
            11 => RewardType::Fruit(HabitatType::Shadow),
            12 => RewardType::Fruit(HabitatType::Light),
            13 => RewardType::Fruit(HabitatType::Earth),
            14 => RewardType::Fruit(HabitatType::Fire),
            15 => RewardType::Fruit(HabitatType::Water),
            16 => RewardType::LegendaryGem,
            17 => RewardType::GoldFidget,
            18 => RewardType::SilverFidget,
            19 => RewardType::BronzeFidget,
            20..=22 => RewardType::Gem,
            23 => RewardType::FruitBasket,
            24 => RewardType::XP,
            25 => RewardType::Egg,
            26 => RewardType::QuicksandGlass,
            27 => RewardType::Honor,
            28 => RewardType::Beer,
            29 => RewardType::Frame,
            30 => RewardType::Mount(Mount::Cow),
            31 => RewardType::Mount(Mount::Horse),
            32 => RewardType::Mount(Mount::Tiger),
            33 => RewardType::Mount(Mount::Dragon),
            x => {
                warn!("Unknown reward type: {x}");
                RewardType::Unknown
            }
        }
    }
}

impl Reward {
    /// Parses a single reward `[typ, amount]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Reward> {
        Ok(Reward {
            typ: RewardType::parse(data.i64(0)?),
            amount: data.u64_or(1, 0),
        })
    }
}

impl RewardChest {
    /// Parses a reward chest from a window of values:
    /// `opened / required_points / reward_count / [typ / amount]...`
    pub(crate) fn parse(data: &Vals<'_>) -> Result<RewardChest> {
        let opened = data.i64_or(0, 0) != 0;
        let required_points = data.u32(1)?;
        let reward_count: usize = data.usize(2)?;
        let mut rewards = Vec::with_capacity(reward_count);
        for pos in 0..reward_count {
            let reward_data = data.window(3 + pos * 2..3 + (pos + 1) * 2);
            rewards.push(Reward::parse(&reward_data)?);
        }
        Ok(RewardChest {
            opened,
            required_points,
            rewards,
        })
    }
}

/// Parses the 3 reward chests from a response of variable length:
/// each chest starts with `[opened, required_points, reward_count]`,
/// followed by `reward_count` pairs of `[typ, amount]`.
pub(crate) fn parse_chests(data: &Vals<'_>) -> Result<[RewardChest; 3]> {
    let mut start = 0;
    let mut next = || -> Result<RewardChest> {
        let chest = RewardChest::parse(&data.window(start..data.len()))?;
        start += 3 + chest.rewards.len() * 2;
        Ok(chest)
    };
    let chests = [next()?, next()?, next()?];
    Ok(chests)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

    #[test]
    fn test_timed_specials_update_advent_calendar() {
        let mut specials = TimedSpecials::default();
        let data = Vals::from("0");
        specials.update_advent_calendar(&data).unwrap();
        assert_eq!(specials.advent_calendar, None);
        assert_all_read(&data);

        let data = Vals::from("3/10");
        specials.update_advent_calendar(&data).unwrap();
        assert_eq!(
            specials.advent_calendar,
            Some(Reward {
                typ: RewardType::Mushrooms,
                amount: 10,
            })
        );
        assert_all_read(&data);
    }
}
