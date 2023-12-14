use std::collections::HashSet;

use chrono::{DateTime, Local};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{
    character::Class, items::*, tavern::QuestLocation, unlockables::PetClass,
};
use crate::{
    command::AttributeType,
    error::SFError,
    misc::{soft_into, warning_parse, warning_try_into},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    PetItem(PetItemType),
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WheelReward {
    pub typ: WheelRewardType,
    pub amount: i64,
}

impl WheelReward {
    pub(crate) fn parse(
        data: &[i64],
        upgraded: bool,
    ) -> Result<WheelReward, SFError> {
        use WheelRewardType::*;
        let mut amount = data[1];
        let typ = match data[0] {
            // NOTE: I have only tested 2.0 and infered 1.0 from that
            0 => Mushrooms,
            1 => match upgraded {
                true => Arcane,
                false => Wood,
            },
            2 => ExperienceXL,
            3 => match upgraded {
                true => {
                    amount = 1;
                    PetItem(PetItemType::parse(data[1]).ok_or_else(|| {
                        SFError::ParsingError(
                            "pet wheel reward type",
                            data[1].to_string(),
                        )
                    })?)
                }
                false => Stone,
            },
            4 => SilverXL,
            5 => {
                // The amount does not seem to do anything.
                // 1 => equipment
                // 2 => potion
                amount = 1;
                Item
            }
            6 => WoodXL,
            7 => Experience,
            8 => StoneXL,
            9 => match upgraded {
                true => Souls,
                false => Silver,
            },
            _ => {
                return Err(SFError::ParsingError(
                    "unknown wheel reward type",
                    data[0].to_string(),
                ))
            }
        };
        Ok(WheelReward { typ, amount })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CalendarReward {
    /// Note that this is technically correct, but at low levels, these are
    /// often overwritten to silver
    pub typ: CalendarRewardType,
    pub amount: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
}

impl CalendarReward {
    pub(crate) fn parse(data: &[i64]) -> Option<CalendarReward> {
        use CalendarRewardType::*;
        let amount = data[1];
        let typ = match data[0] {
            1 => Silver,
            2 => Mushrooms,
            3 => Experience,
            4 => Wood,
            5 => Stone,
            6 => Souls,
            7 => Arcane,
            8 => Runes,
            10 => Item,
            11 => Attribute(AttributeType::Intelligence),
            12 => Attribute(AttributeType::Dexterity),
            13 => Attribute(AttributeType::Intelligence),
            14 => Attribute(AttributeType::Constitution),
            15 => Attribute(AttributeType::Luck),
            16..=20 => Fruit(PetClass::from_typ_id(data[0] - 15)?),
            21 => LevelUp,
            22 => Potion(PotionType::EternalLife),
            23 => TenQuicksandGlasses,
            24 => Potion(PotionType::Strength),
            25 => Potion(PotionType::Dexterity),
            26 => Potion(PotionType::Intelligence),
            27 => Potion(PotionType::Constitution),
            28 => Potion(PotionType::Luck),
            x => {
                warn!("Unknown calendar reward: {x}");
                return None;
            }
        };

        Some(CalendarReward { typ, amount })
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Special {
    /// All of the events active in the tavern
    pub events: HashSet<Event>,
    /// The time at which all of the events end. Mostly just Sunday 23:59.
    pub events_ends: Option<DateTime<Local>>,

    pub event_task_end: Option<DateTime<Local>>,
    pub event_task_start: Option<DateTime<Local>>,

    /// I do not know if this is even correct, or if there are > 1 possible
    pub(crate) event_task_typ: Option<EventTaskSetting>,

    pub event_tasks: Vec<EventTask>,
    pub event_tasks_rewards: [RewardChest; 3],

    pub daily_quests: Vec<DailyQuest>,
    pub daily_quest_rewards: [RewardChest; 3],

    pub calendar: Vec<CalendarReward>,
    /// The time at which the calendar door wll be unlocked. If this is in the
    /// past, that means it is available to open
    pub calendar_next_possible: Option<DateTime<Local>>,

    pub gamble_result: Option<GambleResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GambleResult {
    SilverChange(i64),
    MushroomChange(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventTaskSetting {
    ShoppingSpree = 4,
    PartTimeNudist = 7,
    Scrimper = 8,
    Scholar = 9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventTaskTyp {
    WinFightsBareHands = 57,
    SpendGoldInShop = 65,
    SpendGoldOnUpgrades = 66,
    RequestNewGoods = 67,
    BuyHourGlasses = 68,
    WinFightsNoChestplate = 75,
    WinFightsNoGear = 76,
    WinFightsNoEpicsLegendaries = 77,
    EarnMoneyCityGuard = 78,
    EarnMoneyFromHoFFights = 79,
    EarnMoneySellingItems = 80,
    ColectGoldFromPit = 81,
    GainXpFromQuests = 82,
    GainXpFromAcademy = 83,
    GainXpFromArenaFights = 84,
    GainXpFromAdventuromatic = 85,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventTask {
    pub typ: EventTaskTyp,
    pub current: u64,
    pub target: u64,
    pub reward: u8,
}

impl EventTask {
    pub(crate) fn parse(data: &[i64]) -> Option<EventTask> {
        Some(EventTask {
            typ: FromPrimitive::from_i64(data[0])?,
            current: soft_into(data[1], "current eti", 0),
            target: soft_into(data[2], "target eti", u64::MAX),
            reward: soft_into(data[3], "reward eti", 0),
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
    Mushroom = 3,
    Silver = 4,
    LuckyCoins = 5,
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
pub struct DailyQuest {
    pub typ: DailyQuestType,
    pub current: u64,
    pub target: u64,
    pub bell_reward: u32,
}

impl DailyQuest {
    pub(crate) fn parse(data: &[i64]) -> Option<Self> {
        Some(DailyQuest {
            bell_reward: warning_try_into(data[3], "bells")?,
            typ: DailyQuestType::parse(data[0])?,
            current: warning_try_into(data[1], "current")?,
            target: warning_try_into(data[2], "target")?,
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
}

impl DailyQuestType {
    pub(crate) fn parse(val: i64) -> Option<DailyQuestType> {
        use DailyQuestType::*;
        Some(match val {
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
            25..=45 => TravelTo(FromPrimitive::from_i64(val - 24)?),
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
                return None;
            }
        })
    }
}
