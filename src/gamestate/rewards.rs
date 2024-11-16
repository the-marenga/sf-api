use std::collections::HashSet;

use chrono::{DateTime, Local};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::EnumIter;

use super::{
    character::Class, items::*, tavern::Location, unlockables::HabitatType,
    ArrSkip, CCGet, CGet, LightDungeon, Mount, ShopType,
};
use crate::{command::AttributeType, error::SFError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[allow(missing_docs)]
/// The type of a reward you can win by spinning the wheel. The wheel can be
/// upgraded, so some rewards may not always be available
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
        // NOTE: I have only tested upgraded and inferred not upgraded from that
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
    Fruit(HabitatType),
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
    pub tasks: Vec<Task>,
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
    pub tasks: Vec<Task>,
    /// The rewards available for completing tasks.
    pub rewards: [RewardChest; 3],
}

macro_rules! impl_tasks {
    ($t:ty) => {
        impl $t {
            /// The amount of tasks you have completed
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

            /// Checks if a task of the given type is available and not
            /// completed
            #[must_use]
            pub fn get_available(&self, task_type: TaskType) -> Option<&Task> {
                self.tasks
                    .iter()
                    .find(|task| task.typ == task_type && !task.is_completed())
            }

            /// Returns all uncompleted tasks
            #[must_use]
            pub fn get_uncompleted(&self) -> Vec<&Task> {
                self.tasks
                    .iter()
                    .filter(|task| !task.is_completed())
                    .collect()
            }

            /// Checks if the chest at the given index can be opened
            #[must_use]
            pub fn can_open_chest(&self, index: usize) -> bool {
                // Get the chest at the given index
                let Some(chest) = self.rewards.get(index) else {
                    return false;
                };

                // We can't open the chest twice
                if chest.opened {
                    return false;
                }

                // Check if we have enough points to open the given chest
                self.earned_points() >= chest.required_points
            }
        }
    };
}

impl_tasks!(DailyTasks);
impl_tasks!(EventTasks);

impl Task {
    /// The amount of tasks you have collected
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.current >= self.target
    }
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The theme the event tasks have
pub enum EventTaskTheme {
    // 1 is not set
    Gambler = 2,
    RankClimber = 3,
    ShoppingSpree = 4,
    TimeSkipper = 5,
    RuffianReset = 6,
    PartTimeNudist = 7,
    Scrimper = 8,
    Scholar = 9,
    Maximizer = 10,
    UnderworldFigure = 11,
    EggHunt = 12,
    SummerCollectifun = 13,
    Walpurgis = 14,
    #[default]
    Unknown = 245,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of task you have to do
pub enum TaskType {
    AddSocketToItem,
    BlacksmithDismantle,
    BuyHourGlasses,
    BuyOfferFromArenaManager,
    ClaimSoulsFromExtractor,
    CollectGoldFromPit,
    ConsumeThirstForAdventure,
    ConsumeThirstFromUnderworld,
    DefeatGambler,
    DrinkBeer,
    EarnMoneyCityGuard,
    EarnMoneyFromHoFFights,
    EarnMoneySellingItems,
    EnterDemonPortal,
    FeedPets,

    FightGuildHydra,
    FightGuildPortal,
    FightInDungeons,
    FightInPetHabitat,
    FightMonsterInLegendaryDungeon,
    FightOtherPets,

    FillMushroomsInAdventuromatic,
    FindGemInFortress,
    GainArcaneFromDismantle,
    GainEpic,
    GainHonorExpeditions,
    GainHonorFortress,
    GainHonorInArena,
    GainHonorInHoF,
    GainLegendaryFromLegendaryDungeon,
    GainMetalFromDismantle,
    GainSilver,
    GainSilverFromFightsInHoF,
    GainXP,
    GainXpFromAcademy,
    GainXpFromAdventuromatic,
    GainXpFromArenaFights,
    GainXpFromQuests,
    GetLuckyCoinsFromFlyingTube,
    GuildReadyFight,
    LureHeroesIntoUnderworld,
    PlayGameOfDice,
    RequestNewGoods,
    SacrificeRunes,
    SkipGameOfDiceWait,
    SkipQuest,
    SpendGoldInShop,
    SpendGoldOnUpgrades,
    SpinWheelOfFortune,
    ThrowEpicInToilet,
    ThrowItemInCauldron,
    ThrowItemInToilet,
    TravelTo(Location),

    Upgrade(AttributeType),
    UpgradeAnyAttribute,
    UpgradeArenaManager,
    UpgradeItemAttributes,

    WinFightsPlayerPet,
    WinFightsAgainst(Class),
    WinFightsBackToBack,
    WinFightsBareHands,
    WinFightsInArena,
    WinFightsInHoF,
    WinFightsNoChestplate,
    WinFightsNoEpicsLegendaries,
    WinFightsNoGear,

    LeaseMount,
    DefeatMonstersLightDungeon(LightDungeon),
    BuyWeaponInWeaponsShop,
    FightHigherRankedPlayer,
    AddFriend,
    ClaimNewCustomerPack,
    JoinOrCreateGuild,
    UpgradeAnyGuildSkill,
    CityGuardHours,
    DrinkPotion(PotionType),

    BuyFromShop(ShopType),
    FindFruitsOnExpedition,
    BrewPotions,
    CollectWood,
    CollectStone,
    CommandFortressBattle,
    FightHellevator,
    BuyHellevatorTreats,
    DefeatHellevatorFloors,
    EnterLegendaryDungeon,
    OpenLegendaryDungeonCrateChests,
    FeedPetType(HabitatType),
    SpendCardsHellevator,

    Unknown,
}

impl TaskType {
    pub(crate) fn parse(num: i64) -> TaskType {
        match num {
            1 => TaskType::DrinkBeer,
            2 => TaskType::ConsumeThirstForAdventure,
            3 => TaskType::WinFightsInArena,
            4 => TaskType::SpinWheelOfFortune,
            5 => TaskType::FightGuildHydra,
            6 => TaskType::FightGuildPortal,
            7 => TaskType::FeedPets,
            8 => TaskType::FightOtherPets,
            9 => TaskType::BlacksmithDismantle,
            10 => TaskType::ThrowItemInToilet,
            11 => TaskType::PlayGameOfDice,
            12 => TaskType::LureHeroesIntoUnderworld,
            13 => TaskType::EnterDemonPortal,
            14 => TaskType::DefeatGambler,
            15 => TaskType::Upgrade(AttributeType::Strength),
            16 => TaskType::Upgrade(AttributeType::Dexterity),
            17 => TaskType::Upgrade(AttributeType::Intelligence),
            18 => TaskType::ConsumeThirstFromUnderworld,
            19 => TaskType::GuildReadyFight,
            20 => TaskType::FindGemInFortress,
            21 => TaskType::ThrowItemInCauldron,
            22 => TaskType::FightInPetHabitat,
            23 => TaskType::UpgradeArenaManager,
            24 => TaskType::SacrificeRunes,
            25..=45 => {
                let Some(location) = FromPrimitive::from_i64(num - 24) else {
                    return TaskType::Unknown;
                };
                TaskType::TravelTo(location)
            }
            46 => TaskType::ThrowEpicInToilet,
            47 => TaskType::BuyOfferFromArenaManager,
            48 => TaskType::WinFightsAgainst(Class::Warrior),
            49 => TaskType::WinFightsAgainst(Class::Mage),
            50 => TaskType::WinFightsAgainst(Class::Scout),
            51 => TaskType::WinFightsAgainst(Class::Assassin),
            52 => TaskType::WinFightsAgainst(Class::Druid),
            53 => TaskType::WinFightsAgainst(Class::Bard),
            54 => TaskType::WinFightsAgainst(Class::BattleMage),
            55 => TaskType::WinFightsAgainst(Class::Berserker),
            56 => TaskType::WinFightsAgainst(Class::DemonHunter),
            57 => TaskType::WinFightsBareHands,
            58 => TaskType::WinFightsPlayerPet,
            59 => TaskType::GetLuckyCoinsFromFlyingTube,
            60 => TaskType::Upgrade(AttributeType::Luck),
            61 => TaskType::GainHonorInArena,
            62 => TaskType::GainHonorInHoF,
            63 => TaskType::GainHonorFortress,
            64 => TaskType::GainHonorExpeditions,
            65 => TaskType::SpendGoldInShop,
            66 => TaskType::SpendGoldOnUpgrades,
            67 => TaskType::RequestNewGoods,
            68 => TaskType::BuyHourGlasses,
            69 => TaskType::SkipQuest,
            70 => TaskType::SkipGameOfDiceWait,
            71 => TaskType::WinFightsInHoF,
            72 => TaskType::WinFightsBackToBack,
            73 => TaskType::SpendCardsHellevator,
            74 => TaskType::GainSilverFromFightsInHoF,
            75 => TaskType::WinFightsNoChestplate,
            76 => TaskType::WinFightsNoGear,
            77 => TaskType::WinFightsNoEpicsLegendaries,
            78 => TaskType::EarnMoneyCityGuard,
            79 => TaskType::EarnMoneyFromHoFFights,
            80 => TaskType::EarnMoneySellingItems,
            81 => TaskType::CollectGoldFromPit,
            82 => TaskType::GainXpFromQuests,
            83 => TaskType::GainXpFromAcademy,
            84 => TaskType::GainXpFromArenaFights,
            85 => TaskType::GainXpFromAdventuromatic,
            86 => TaskType::GainArcaneFromDismantle,
            87 => TaskType::GainMetalFromDismantle,
            88 => TaskType::UpgradeItemAttributes,
            89 => TaskType::AddSocketToItem,
            90 => TaskType::ClaimSoulsFromExtractor,
            91 => TaskType::FillMushroomsInAdventuromatic,
            92 => TaskType::WinFightsAgainst(Class::Necromancer),
            93 => TaskType::GainLegendaryFromLegendaryDungeon,
            94 => TaskType::FightMonsterInLegendaryDungeon,
            95 => TaskType::FightInDungeons,
            96 => TaskType::UpgradeAnyAttribute,
            97 => TaskType::GainSilver,
            98 => TaskType::GainXP,
            99 => TaskType::GainEpic,
            100 => TaskType::BuyFromShop(ShopType::Magic),
            101 => TaskType::BuyFromShop(ShopType::Weapon),
            102 => TaskType::FindFruitsOnExpedition,
            103 => TaskType::BrewPotions,
            104 => TaskType::CollectWood,
            105 => TaskType::CollectStone,
            106 => TaskType::CommandFortressBattle,
            107 => TaskType::FightHellevator,
            108 => TaskType::BuyHellevatorTreats,
            109 => TaskType::DefeatHellevatorFloors,
            110 => TaskType::EnterLegendaryDungeon,
            111 => TaskType::OpenLegendaryDungeonCrateChests,
            112 => TaskType::FeedPetType(HabitatType::Shadow),
            113 => TaskType::FeedPetType(HabitatType::Light),
            114 => TaskType::FeedPetType(HabitatType::Earth),
            115 => TaskType::FeedPetType(HabitatType::Fire),
            116 => TaskType::FeedPetType(HabitatType::Water),
            117 => {
                TaskType::DefeatMonstersLightDungeon(LightDungeon::TrainingCamp)
            }
            118 => TaskType::ClaimNewCustomerPack,
            119 => TaskType::JoinOrCreateGuild,
            120 => TaskType::UpgradeAnyGuildSkill,
            121 => TaskType::AddFriend,
            122 => TaskType::DrinkPotion(PotionType::Constitution),
            123 => TaskType::DrinkPotion(PotionType::Strength),
            124 => TaskType::DrinkPotion(PotionType::Dexterity),
            125 => TaskType::DrinkPotion(PotionType::Intelligence),
            126 => TaskType::DrinkPotion(PotionType::EternalLife),
            127 => TaskType::LeaseMount,
            128 => TaskType::FightHigherRankedPlayer,
            129 => TaskType::CityGuardHours,
            130 => TaskType::BuyWeaponInWeaponsShop,
            131 => TaskType::Upgrade(AttributeType::Constitution),

            ..=0 | 132.. => TaskType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Something to do to get a point reward
pub struct Task {
    /// The thing you are tasked with doing or getting for this task
    pub typ: TaskType,
    /// The amount of times, or the amount of `typ` you have currently
    pub current: u64,
    /// The amount current has to be at to complete this task
    pub target: u64,
    /// The amount of points you get for completing this task
    pub point_reward: u32,
}

impl Task {
    pub(crate) fn parse(data: &[i64]) -> Result<Task, SFError> {
        let raw_typ = data.cget(0, "task typ")?;
        let typ = TaskType::parse(raw_typ);

        if typ == TaskType::Unknown {
            warn!("Unknown  task: {data:?} {raw_typ}");
        }
        Ok(Task {
            typ,
            current: data.csiget(1, "current ti", 0)?,
            target: data.csiget(2, "target ti", u64::MAX)?,
            point_reward: data.csiget(3, "reward ti", 0)?,
        })
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Something you can unlock for completing tasks
pub struct RewardChest {
    /// Whether or not this chest has been unlocked
    pub opened: bool,
    /// The amount of points required to open this chest
    pub required_points: u32,
    /// The things you will get for opening this chest
    pub rewards: Vec<Reward>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The reward for opening a chest
pub struct Reward {
    /// The type of the thing you are getting
    pub typ: RewardType,
    /// The amount of `typ` you get
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RewardType {
    HellevatorPoints,
    HellevatorCards,
    Mushrooms,
    Silver,
    LuckyCoins,
    Wood,
    Stone,
    Arcane,
    Metal,
    Souls,
    Fruit(HabitatType),
    LegendaryGem,
    GoldFidget,
    SilverFidget,
    BronzeFidget,
    Gem,
    FruitBasket,
    XP,
    Egg,
    QuicksandGlass,
    Honor,
    Beer,
    Frame,
    Mount(Mount),
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
    pub(crate) fn parse(data: &[i64]) -> Result<Reward, SFError> {
        Ok(Reward {
            typ: RewardType::parse(data.cget(0, "reward typ")?),
            amount: data.csiget(1, "reward amount", 0)?,
        })
    }
}

impl RewardChest {
    pub(crate) fn parse(data: &[i64]) -> Result<RewardChest, SFError> {
        let opened = data.cget(0, "rchest opened")? != 0;
        let required_points = data.ciget(1, "reward chest required points")?;
        let reward_count: usize = data.ciget(2, "reward chest count")?;
        let mut rewards = Vec::new();
        for pos in 0..reward_count {
            let data = data.skip(3 + pos * 2, "rchest rewards")?;
            rewards.push(Reward::parse(data)?);
        }
        Ok(RewardChest {
            opened,
            required_points,
            rewards,
        })
    }
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of event, that is currently happening on the server
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
    HolidaySale,
}

pub(crate) fn parse_rewards(vals: &[i64]) -> [RewardChest; 3] {
    let mut start = 0;
    core::array::from_fn(|_| -> Result<RewardChest, SFError> {
        let vals = vals.skip(start, "multi reward chest")?;
        let chest = RewardChest::parse(vals)?;
        let consumed = 3 + chest.rewards.len() * 2;
        start += consumed;
        Ok(chest)
    })
    .map(|res| match res {
        Ok(res) => res,
        Err(err) => {
            warn!("Bad task rewards: {err}");
            RewardChest::default()
        }
    })
}
