use std::collections::HashSet;

use chrono::{DateTime, Local};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{
    character::Class, items::*, tavern::QuestLocation, unlockables::PetClass,
    CCGet, CGet,
};
use crate::{
    command::AttributeType,
    error::SFError,
    misc::{soft_into, warning_parse},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[allow(missing_docs)]
/// The type of a reward you can win by spinning the wheel. The wheel can be
/// upgraded, so some rewards may not always eb available
pub enum WheelRewardType {
    Mushrooms,
    Stone,
    StoneXL,
    Wood,
    WoodXL,
    Experience,
    ExperienceXL,
    Silver,
    SilverXL,
    Arcane,
    Souls,
    Item,
    PetItem(PetItem),
    Unknown,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The thing you won from spinning the wheel
pub struct WheelReward {
    /// The type of item you have won
    pub typ: WheelRewardType,
    /// The amount of the type you have won
    pub amount: i64,
}

impl WheelReward {
    pub(crate) fn parse(
        data: &[i64],
        upgraded: bool,
    ) -> Result<WheelReward, SFError> {
        let raw_typ = data.cget(0, "wheel reward typ")?;
        let mut amount = data.cget(1, "wheel reward amount")?;
        // NOTE: I have only tested upgraded and infered not upgraded from that
        let typ = match raw_typ {
            0 => WheelRewardType::Mushrooms,
            1 => {
                if upgraded {
                    WheelRewardType::Arcane
                } else {
                    WheelRewardType::Wood
                }
            }
            2 => WheelRewardType::ExperienceXL,
            3 => {
                if upgraded {
                    let res = WheelRewardType::PetItem(
                        PetItem::parse(amount).ok_or_else(|| {
                            SFError::ParsingError(
                                "pet wheel reward type",
                                amount.to_string(),
                            )
                        })?,
                    );
                    amount = 1;
                    res
                } else {
                    WheelRewardType::Stone
                }
            }
            4 => WheelRewardType::SilverXL,
            5 => {
                // The amount does not seem to do anything.
                // 1 => equipment
                // 2 => potion
                amount = 1;
                WheelRewardType::Item
            }
            6 => WheelRewardType::WoodXL,
            7 => WheelRewardType::Experience,
            8 => WheelRewardType::StoneXL,
            9 => {
                if upgraded {
                    WheelRewardType::Souls
                } else {
                    WheelRewardType::Silver
                }
            }
            x => {
                warn!("unknown wheel reward type: {x}");
                WheelRewardType::Unknown
            }
        };
        Ok(WheelReward { typ, amount })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A possible reward on the calendar
pub struct CalendarReward {
    /// Note that this is technically correct, but at low levels, these are
    /// often overwritten to silver
    // FIXME: figure out how exactly
    pub typ: CalendarRewardType,
    /// The mount of the type this reward yielded
    pub amount: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of reward gainable by collecting the calendar
pub enum CalendarRewardType {
    Silver,
    Mushrooms,
    Experience,
    Wood,
    Stone,
    Souls,
    Arcane,
    Runes,
    Item,
    Attribute(AttributeType),
    Fruit(PetClass),
    Level,
    Potion(PotionType),
    TenQuicksandGlasses,
    LevelUp,
    Unknown,
}

impl CalendarReward {
    pub(crate) fn parse(data: &[i64]) -> Result<CalendarReward, SFError> {
        let amount = data.cget(1, "c reward amount")?;
        let typ = data.cget(0, "c reward typ")?;
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
                if let Some(typ) = PetClass::from_typ_id(x - 15) {
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

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Everything, that changes over time
pub struct TimedSpecials {
    /// All of the events active in the tavern
    pub events: Events,
    /// The stuff you can do for bonus rewards
    pub tasks: Tasks,
    /// Grants rewards once a day
    pub calendar: Calendar,
    /// Dr. Abawuwu's wheel
    pub wheel: Wheel,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the events active in the tavern
pub struct Events {
    /// All of the events active in the tavern
    pub active: HashSet<Event>,
    /// The time at which all of the events end. Mostly just Sunday 23:59.
    pub ends: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[doc(alias = "DailyLoginBonus")]
/// Grants rewards once a day
pub struct Calendar {
    /// The things you can get from the calendar
    pub rewards: Vec<CalendarReward>,
    /// The time at which the calendar door wll be unlocked. If this is in the
    /// past, that means it is available to open
    pub next_possible: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The tasks you get from the goblin gleeman
pub struct Tasks {
    /// The tasks, that update daily
    pub daily: DailyTasks,
    /// The tasks, that follow some server wide theme
    pub event: EventTasks,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the tasks, that reset every day
pub struct DailyTasks {
    /// The tasks you have to do
    pub tasks: Vec<DailyTask>,
    /// The rewards available for completing tasks.
    pub rewards: [RewardChest; 3],
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the tasks, that are based on some event theme
pub struct EventTasks {
    /// The "theme" the event task has. This is mainly irrelevant
    pub theme: EventTaskTheme,
    /// The time at which the event tasks have been set
    pub start: Option<DateTime<Local>>,
    /// The time at which the event tasks will reset
    pub end: Option<DateTime<Local>>,
    /// The actual tasks you have to complete
    pub tasks: Vec<EventTask>,
    /// The rewards available for completing tasks.
    pub rewards: [RewardChest; 3],
}

macro_rules! impl_tasks {
    ($t:ty) => {
        impl $t {
            /// The amount of tasks you have collected
            #[must_use]
            pub fn completed(&self) -> usize {
                self.tasks.iter().filter(|a| a.is_completed()).count()
            }

            /// The amount of points you have collected from completing tasks
            #[must_use]
            pub fn earned_points(&self) -> u32 {
                self.tasks
                    .iter()
                    .filter(|a| a.is_completed())
                    .map(|a| a.point_reward)
                    .sum()
            }
            /// The amount of points, that are available in total
            #[must_use]
            pub fn total_points(&self) -> u32 {
                self.tasks.iter().map(|a| a.point_reward).sum()
            }
        }
    };
}

impl_tasks!(DailyTasks);
impl_tasks!(EventTasks);

macro_rules! impl_task {
    ($t:ty) => {
        impl $t {
            /// The amount of tasks you have collected
            #[must_use]
            pub fn is_completed(&self) -> bool {
                self.current >= self.target
            }
        }
    };
}

impl_task!(EventTask);
impl_task!(DailyTask);

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Dr. Abawuwu's wheel
pub struct Wheel {
    /// The amount of lucky coins you have to spin the weel
    pub lucky_coins: u32,
    /// The amount of times you have spun the wheel today already (0 -> 20)
    pub spins_today: u8,
    /// The next time you can spin the wheel for free
    pub next_free_spin: Option<DateTime<Local>>,
    /// The result of spinning the wheel
    pub result: Option<WheelReward>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventTaskTheme {
    ShoppingSpree = 4,
    TimeSkipper = 5,
    RuffianReset = 6,
    PartTimeNudist = 7,
    Scrimper = 8,
    Scholar = 9,
    UnderworldFigure = 11,
    #[default]
    Unknown = 245,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventTaskTyp {
    LureHeroesIntoUnderworld,
    WinFightsAgainst(Class),
    WinFightsBareHands,
    SpendGoldInShop,
    SpendGoldOnUpgrades,
    RequestNewGoods,
    BuyHourGlasses,
    SkipQuest,
    SkipGameOfDiceWait,
    WinFights,
    WinFightsBackToBack,
    WinFightsNoChestplate,
    WinFightsNoGear,
    WinFightsNoEpicsLegendaries,
    EarnMoneyCityGuard,
    EarnMoneyFromHoFFights,
    EarnMoneySellingItems,
    ColectGoldFromPit,
    GainXpFromQuests,
    GainXpFromAcademy,
    GainXpFromArenaFights,
    GainXpFromAdventuromatic,
    ClaimSoulsFromExtractor,
    FillMushroomsInAdventuromatic,
    Unknown,
}

impl EventTaskTyp {
    pub fn parse(num: i64) -> EventTaskTyp {
        use EventTaskTyp::*;
        match num {
            12 => LureHeroesIntoUnderworld,
            48 => WinFightsAgainst(Class::Warrior),
            49 => WinFightsAgainst(Class::Mage),
            50 => WinFightsAgainst(Class::Scout),
            51 => WinFightsAgainst(Class::Assassin),
            52 => WinFightsAgainst(Class::Druid),
            53 => WinFightsAgainst(Class::Bard),
            54 => WinFightsAgainst(Class::BattleMage),
            55 => WinFightsAgainst(Class::Berserker),
            56 => WinFightsAgainst(Class::DemonHunter),
            57 => WinFightsBareHands,
            65 => SpendGoldInShop,
            66 => SpendGoldOnUpgrades,
            67 => RequestNewGoods,
            68 => BuyHourGlasses,
            69 => SkipQuest,
            70 => SkipGameOfDiceWait,
            71 => WinFights,
            72 => WinFightsBackToBack,
            75 => WinFightsNoChestplate,
            76 => WinFightsNoGear,
            77 => WinFightsNoEpicsLegendaries,
            78 => EarnMoneyCityGuard,
            79 => EarnMoneyFromHoFFights,
            80 => EarnMoneySellingItems,
            81 => ColectGoldFromPit,
            82 => GainXpFromQuests,
            83 => GainXpFromAcademy,
            84 => GainXpFromArenaFights,
            85 => GainXpFromAdventuromatic,
            90 => ClaimSoulsFromExtractor,
            91 => FillMushroomsInAdventuromatic,
            92 => WinFightsAgainst(Class::Necromancer),
            x => {
                warn!("Unknown event task typ: {x}");
                Unknown
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventTask {
    pub typ: EventTaskTyp,
    pub current: u64,
    pub target: u64,
    pub point_reward: u32,
}

impl EventTask {
    pub(crate) fn parse(data: &[i64]) -> Result<EventTask, SFError> {
        let raw_typ = data.cget(0, "event task typ")?;
        Ok(EventTask {
            typ: EventTaskTyp::parse(raw_typ),
            current: soft_into(data[1], "current eti", 0),
            target: soft_into(data[2], "target eti", u64::MAX),
            point_reward: soft_into(data[3], "reward eti", 0),
        })
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RewardChest {
    pub opened: bool,
    pub reward: [Option<Reward>; 2],
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Reward {
    pub typ: RewardTyp,
    pub amount: u64,
}

impl Reward {
    pub(crate) fn parse(data: &[i64]) -> Reward {
        Reward {
            typ: warning_parse(data[0], "reward typ", FromPrimitive::from_i64)
                .unwrap_or(RewardTyp::Unknown),
            amount: soft_into(data[1], "reward amount", 0),
        }
    }
}

impl RewardChest {
    pub(crate) fn parse(data: &[i64]) -> RewardChest {
        let mut reward: [Option<Reward>; 2] = Default::default();

        let indices: &[usize] = match data.len() {
            5 => &[3],
            _ => &[3, 5],
        };

        for (i, reward) in indices.iter().copied().zip(&mut reward) {
            *reward = Some(Reward::parse(&data[i..]));
        }

        RewardChest {
            opened: data[0] == 1,
            reward,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, FromPrimitive, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RewardTyp {
    ExtraBeer = 2,
    Mushroom = 3,
    Silver = 4,
    LuckyCoins = 5,
    Stone = 9,
    Souls = 10,
    Experience = 24,
    Hourglass = 26,
    Beer = 28,
    Unknown = 999,
}

#[derive(
    Debug,
    Clone,
    Copy,
    FromPrimitive,
    PartialEq,
    Eq,
    Hash,
    strum::EnumCount,
    strum::EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    ExceptionalXPEvent = 0,
    GloriousGoldGalore,
    TidyToiletTime,
    AssemblyOfAwesomeAnimals,
    FantasticFortressFestivity,
    DaysOfDoomedSouls,
    WitchesDance,
    SandsOfTimeSpecial,
    ForgeFrenzyFestival,
    EpicShoppingSpreeExtravaganza,
    EpicQuestExtravaganza,
    EpicGoodLuckExtravaganza,
    OneBeerTwoBeerFreeBeer,
    PieceworkParty,
    LuckyDay,
    CrazyMushroomHarvest,
    HollidaySale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DailyTask {
    pub typ: DailyQuestType,
    pub current: u64,
    pub target: u64,
    pub point_reward: u32,
}

impl DailyTask {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        Ok(DailyTask {
            point_reward: data.csiget(3, "daily bells", 0)?,
            typ: DailyQuestType::parse(data[0]),
            current: data.csiget(1, "daily current", 0)?,
            target: data.csiget(2, "daily target", 999)?,
        })
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DailyQuestType {
    DrinkBeer,
    FindGemInFortress,
    ConsumeThirstForAdventure,
    FightGuildHydra,
    FightGuildPortal,
    SpinWheelOfFortune,
    FeedPets,
    FightOtherPets,
    BlacksmithDismantle,
    ThrowItemInToilet,
    PlayDice,
    LureHeoesInUnderworld,
    EnterDemonPortal,
    GuildReadyFight,
    SacrificeRunes,
    TravelTo(QuestLocation),
    WinFights(Option<Class>),
    DefeatOtherPet,
    ThrowItemInCauldron,
    WinFightsWithBareHands,
    DefeatGambler,
    Upgrade(AttributeType),
    ConsumeThirstFromUnderworld,
    UpgradeArenaManager,
    ThrowEpicInToilet,
    BuyOfferFromArenaManager,
    FightInPetHabitat,
    WinFightsWithoutEpics,
    Unknown,
}

impl DailyQuestType {
    pub(crate) fn parse(val: i64) -> DailyQuestType {
        use DailyQuestType::*;
        match val {
            1 => DrinkBeer,
            2 => ConsumeThirstForAdventure,
            3 => WinFights(None),
            4 => SpinWheelOfFortune,
            5 => FightGuildHydra,
            6 => FightGuildPortal,
            7 => FeedPets,
            8 => FightOtherPets,
            9 => BlacksmithDismantle,
            10 => ThrowItemInToilet,
            11 => PlayDice,
            12 => LureHeoesInUnderworld,
            13 => EnterDemonPortal,
            14 => DefeatGambler,
            15 => Upgrade(AttributeType::Strength),
            16 => Upgrade(AttributeType::Dexterity),
            17 => Upgrade(AttributeType::Intelligence),
            18 => ConsumeThirstFromUnderworld,
            19 => GuildReadyFight,
            20 => FindGemInFortress,
            21 => ThrowItemInCauldron,
            22 => FightInPetHabitat,
            23 => UpgradeArenaManager,
            24 => SacrificeRunes,
            25..=45 => {
                let Some(location) = FromPrimitive::from_i64(val - 24) else {
                    return Unknown;
                };
                TravelTo(location)
            }
            46 => ThrowEpicInToilet,
            47 => BuyOfferFromArenaManager,
            48 => WinFights(Some(Class::Warrior)),
            49 => WinFights(Some(Class::Mage)),
            50 => WinFights(Some(Class::Scout)),
            51 => WinFights(Some(Class::Assassin)),
            52 => WinFights(Some(Class::Druid)),
            53 => WinFights(Some(Class::Bard)),
            54 => WinFights(Some(Class::BattleMage)),
            55 => WinFights(Some(Class::Berserker)),
            56 => WinFights(Some(Class::DemonHunter)),
            57 => WinFightsWithBareHands,
            58 => DefeatOtherPet,
            77 => WinFightsWithoutEpics,
            92 => WinFights(Some(Class::Necromancer)),
            x => {
                warn!("Unknown daily quest: {x}");
                Unknown
            }
        }
    }
}
