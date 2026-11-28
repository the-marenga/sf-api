use std::collections::HashSet;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use super::{
    IdleBuildingType, LightDungeon, Mount, ServerTime, ShopType,
    character::Class, items::*, tavern::Location, unlockables::HabitatType,
};
use crate::{command::AttributeType, error::SFError, response::Vals};

/// The type of a reward you can win by spinning the wheel. The wheel can be
/// upgraded, so some rewards may not always be available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[allow(missing_docs)]
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

/// The thing you won from spinning the wheel
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WheelReward {
    /// The type of item you have won
    pub typ: WheelRewardType,
    /// The amount of the type you have won
    pub amount: i64,
}

impl WheelReward {
    pub(crate) fn parse(
        data: &Vals<'_>,
        upgraded: bool,
    ) -> Result<WheelReward, SFError> {
        let result = (|| {
            let raw_typ = data.i64(0)?;
            let mut amount = data.i64(1)?;
            // NOTE: I have only tested upgraded and inferred not upgraded from
            // that
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
        })();

        if result.is_err() {
            data.ignore_all("invalid wheel reward");
        } else {
            data.ignore(2..data.len(), "extra wheel reward values");
        }
        result
    }
}

impl Wheel {
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        data.ignore(0, "unknown wheel value");
        self.spins_today = data.u8_or(1, 0);
        self.next_free_spin = data.server_time_opt(2, server_time);
        data.ignore(3..data.len(), "extra wheel values");
    }

    pub(crate) fn update_result(
        &mut self,
        data: &Vals<'_>,
        upgraded: bool,
    ) -> Result<(), SFError> {
        self.result = Some(WheelReward::parse(data, upgraded)?);
        Ok(())
    }
}

/// A possible reward on the calendar
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CalendarReward {
    /// Note that this is technically correct, but at low levels, these are
    /// often overwritten to silver
    // FIXME: figure out how exactly
    pub typ: CalendarRewardType,
    /// The mount of the type this reward yielded
    pub amount: i64,
}

/// The type of reward gainable by collecting the calendar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
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
    pub(crate) fn parse(data: &Vals<'_>) -> CalendarReward {
        let amount = data.i64_or(1, 0);
        let raw_typ = data.i64_or(0, 0);
        let typ = match raw_typ {
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

        data.ignore(2..data.len(), "extra calendar reward values");
        CalendarReward { typ, amount }
    }
}

impl Calendar {
    pub(crate) fn update_rewards(&mut self, data: &Vals<'_>) {
        self.rewards.clear();
        let complete_len = data.len() / 2 * 2;
        self.rewards = data
            .window(0..complete_len)
            .chunks(2)
            .map(|reward| CalendarReward::parse(&reward))
            .collect();
        data.ignore(complete_len..data.len(), "partial calendar reward");
    }
}

/// Everything, that changes over time
#[derive(Debug, Clone, Default)]
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
    pub(crate) fn update_advent_calendar(&mut self, data: &Vals<'_>) {
        match data.i64(0) {
            Ok(0) | Err(_) => {
                data.ignore_all("empty or malformed advent calendar");
                self.advent_calendar = None;
            }
            Ok(_) => {
                self.advent_calendar = Some(Reward::parse_vals(data));
            }
        }
    }
}

/// Information about the events active in the tavern
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Events {
    /// All of the events active in the tavern
    pub active: HashSet<Event>,
    /// All of the sales, that are currently active
    pub sales: EnumMap<Sale, bool>,
    /// The time at which all of the events end. Mostly just Sunday 23:59.
    pub ends: Option<DateTime<Local>>,
}

/// A sale, that the server can run on top of the regular events
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Sale {
    /// The black friday sale, that happens around the end of november.
    BlackFriday,
}

impl Sale {
    /// Maps the sale name, that the server appends to the `events` response,
    /// to the corresponding sale
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "blackfridaysale" => Some(Self::BlackFriday),

            _ => None,
        }
    }
}

impl Events {
    /// Updates this state from the raw server response for the `events` key
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        // The list is normally purely numeric, but the server appends the
        // name of an active sale like `blackfridaysale` to it
        let mut numeric = Vec::new();
        let mut sales = Vec::new();
        for part in data.iter() {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Ok(number) = part.parse() {
                numeric.push(number);
            } else {
                match Sale::from_name(part) {
                    Some(sale) => sales.push(sale),
                    None => warn!("Unknown sale: {part}"),
                }
            }
        }
        if numeric.len() < 8 {
            return;
        }

        // [0] might be the events theme (tavern decoration)?
        self.active.clear();
        let flags = numeric.get(1).copied().unwrap_or_default();
        for (idx, event) in Event::iter().enumerate() {
            if (flags & (1 << idx)) > 0 {
                self.active.insert(event);
            }
        }
        // NOTE: there are two end times, that seem to be identical. I will
        // just guess, that [4] is the correct one and [2] is for the theme
        let end = numeric.get(4).copied().unwrap_or_default();
        self.ends = server_time.convert_to_local(end, "event end");

        self.sales = EnumMap::default();
        for sale in sales {
            self.sales[sale] = true;
        }

        // [3],[5],[6],[7] are just 0, so no idea what they could be
    }
}

/// Grants rewards once a day
#[derive(Debug, Clone, Default)]
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

/// The tasks you get from the goblin gleeman
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tasks {
    /// The tasks, that update daily
    pub daily: DailyTasks,
    /// The tasks, that follow some server wide theme
    pub event: EventTasks,
}

/// Information about the tasks, that reset every day
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DailyTasks {
    /// The tasks you have to do
    pub tasks: Vec<Task>,
    /// The rewards available for completing tasks.
    pub rewards: [RewardChest; 3],
}

/// Information about the tasks, that are based on some event theme
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

            pub(crate) fn update_rewards(&mut self, data: &Vals<'_>) {
                self.rewards = parse_rewards(data);
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

impl DailyTasks {
    pub(crate) fn update_tasks(&mut self, data: &Vals<'_>) {
        self.tasks.clear();
        data.ignore(0, "number of daily tasks requiring more than one bell");
        let complete_len = 1 + (data.len().saturating_sub(1) / 4 * 4);
        self.tasks = data
            .window(1..complete_len)
            .chunks(4)
            .map(|task| Task::parse(&task))
            .collect();
        data.ignore(complete_len..data.len(), "partial daily task");
    }
}

impl EventTasks {
    pub(crate) fn update_tasks(&mut self, data: &Vals<'_>) {
        self.tasks.clear();
        let complete_len = data.len() / 4 * 4;
        self.tasks = data
            .window(0..complete_len)
            .chunks(4)
            .map(|task| Task::parse(&task))
            .collect();
        data.ignore(complete_len..data.len(), "partial event task");
    }

    pub(crate) fn update_info(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.start = data.server_time_opt(0, server_time);
        self.end = data.server_time_opt(1, server_time);
        let raw_theme = data.i64_or(2, 0);
        self.theme = FromPrimitive::from_i64(raw_theme).unwrap_or_else(|| {
            if !matches!(raw_theme, 0 | -1) {
                warn!("There might be a new event task theme -> {raw_theme}");
            }
            EventTaskTheme::Unknown
        });
        data.ignore(3..data.len(), "extra event task info values");
    }
}

impl Task {
    /// The amount of tasks you have collected
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.current >= self.target
    }
}

/// Dr. Abawuwu's wheel
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// The theme the event tasks have
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[allow(missing_docs)]
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
    PetTrainer = 15,
    FortressMaster = 16,
    LegendaryDungeon = 17,
    Hellevator = 18,
    #[default]
    Unknown = 245,
}

/// The type of task you have to do
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[allow(missing_docs)]
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
    OpenAdventCalendar,
    UpgradeArenaManagerBuilding(IdleBuildingType),
    EarnMoneyFromExpeditions,

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
            132 => TaskType::WinFightsAgainst(Class::Paladin),
            133 => {
                TaskType::UpgradeArenaManagerBuilding(IdleBuildingType::Seat)
            }
            134 => TaskType::OpenAdventCalendar,
            135 => TaskType::EarnMoneyFromExpeditions,
            136 => TaskType::WinFightsAgainst(Class::PlagueDoctor),

            ..=0 | 137.. => TaskType::Unknown,
        }
    }
}

/// Something to do to get a point reward
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub(crate) fn parse(data: &Vals<'_>) -> Task {
        let raw_typ = data.i64_or(0, 0);
        let typ = TaskType::parse(raw_typ);

        if typ == TaskType::Unknown {
            warn!("Unknown task: {data:?} {raw_typ}");
        }
        let task = Task {
            typ,
            current: data.u64_or(1, 0),
            target: data.u64_or(2, u64::MAX),
            point_reward: data.u32_or(3, 0),
        };
        data.ignore(4..data.len(), "extra task values");
        task
    }
}

/// Something you can unlock for completing tasks
#[derive(Debug, Clone, Default)]
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub(crate) fn parse_vals(data: &Vals<'_>) -> Reward {
        let typ = RewardType::parse(data.i64_or(0, 0));
        let amount = u64::try_from(data.i64_or(1, 0)).unwrap_or(0);
        data.ignore(2..data.len(), "extra reward values");
        Reward { typ, amount }
    }
}

impl RewardChest {
    pub(crate) fn parse(data: &Vals<'_>) -> Result<RewardChest, SFError> {
        let result = (|| {
            let opened = data.i64_or(0, 0) != 0;
            let required_points = data.u32(1)?;
            let reward_count = data.usize(2)?;
            let available = data.len().saturating_sub(3) / 2;
            if reward_count > available {
                return Err(SFError::ParsingError(
                    "reward chest count",
                    format!(
                        "{reward_count} rewards declared, only {available} \
                         available"
                    ),
                ));
            }

            let mut rewards = Vec::with_capacity(reward_count);
            for pos in 0..reward_count {
                let start = 3 + pos * 2;
                rewards
                    .push(Reward::parse_vals(&data.window(start..start + 2)));
            }
            Ok(RewardChest {
                opened,
                required_points,
                rewards,
            })
        })();

        if result.is_err() {
            data.ignore_all("invalid reward chest");
        }
        result
    }
}

/// The type of event, that is currently happening on the server
#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
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
    ValentinesBlessing,
    BlackGemRush,
    RumbleForRiches,
}

pub(crate) fn parse_rewards(vals: &Vals<'_>) -> [RewardChest; 3] {
    let mut start = 0;
    let chests = core::array::from_fn(|_| {
        let result: Result<RewardChest, SFError> = (|| {
            let chest = RewardChest::parse(&vals.window(start..vals.len()))?;
            start += 3 + chest.rewards.len() * 2;
            Ok(chest)
        })();
        match result {
            Ok(chest) => chest,
            Err(err) => {
                warn!("Bad task rewards: {err}");
                vals.ignore(
                    start..vals.len(),
                    "unread task rewards after chest error",
                );
                RewardChest::default()
            }
        }
    });
    vals.ignore(start..vals.len(), "extra task reward values");
    chests
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    fn local_time(timestamp: i64) -> DateTime<Local> {
        DateTime::from_timestamp(timestamp, 0)
            .unwrap()
            .naive_utc()
            .and_local_timezone(Local)
            .latest()
            .unwrap()
    }

    #[test]
    fn wheel_reward_preserves_mappings_and_ignores_extras() {
        let cases = [
            (0, 10, false, WheelRewardType::Mushrooms, 10),
            (1, 11, false, WheelRewardType::Wood, 11),
            (1, 12, true, WheelRewardType::Arcane, 12),
            (2, 13, false, WheelRewardType::ExperienceXL, 13),
            (3, 14, false, WheelRewardType::Stone, 14),
            (4, 15, false, WheelRewardType::SilverXL, 15),
            (5, 2, false, WheelRewardType::Item, 1),
            (6, 17, false, WheelRewardType::WoodXL, 17),
            (7, 18, false, WheelRewardType::Experience, 18),
            (8, 19, false, WheelRewardType::StoneXL, 19),
            (9, 20, false, WheelRewardType::Silver, 20),
            (9, 21, true, WheelRewardType::Souls, 21),
            (999, 22, false, WheelRewardType::Unknown, 22),
        ];
        for (raw_typ, amount, upgraded, expected_typ, expected_amount) in cases
        {
            let raw = format!("{raw_typ}/{amount}/ignored/also-ignored");
            let data = Vals::from(raw.as_str());
            let reward = WheelReward::parse(&data, upgraded).unwrap();
            assert_eq!(reward.typ, expected_typ);
            assert_eq!(reward.amount, expected_amount);
            data.assert_all_read();
        }
    }

    #[test]
    fn upgraded_wheel_pet_reward_is_strict_and_normalizes_amount() {
        let data = Vals::from("3/21/extra");
        let reward = WheelReward::parse(&data, true).unwrap();
        assert_eq!(reward.typ, WheelRewardType::PetItem(PetItem::GoldenEgg));
        assert_eq!(reward.amount, 1);
        data.assert_all_read();

        let invalid = Vals::from("3/999/extra");
        let err = WheelReward::parse(&invalid, true).unwrap_err();
        assert!(matches!(
            err,
            SFError::ParsingError("pet wheel reward type", ref value)
                if value == "999"
        ));
        invalid.assert_all_read();

        for raw in ["", "1", "bad/2", "1/bad"] {
            let data = Vals::from(raw);
            assert!(WheelReward::parse(&data, false).is_err());
            data.assert_all_read();
        }
    }

    #[test]
    fn wheel_update_handles_full_and_truncated_data() {
        let mut wheel = Wheel {
            lucky_coins: 77,
            ..Default::default()
        };
        let full = Vals::from("ignored/12/1700000000/extra");
        wheel.update(&full, ServerTime::default());
        assert_eq!(wheel.spins_today, 12);
        assert_eq!(wheel.next_free_spin, Some(local_time(1_700_000_000)));
        assert_eq!(wheel.lucky_coins, 77);
        full.assert_all_read();

        let truncated = Vals::from("ignored/not-a-number");
        wheel.update(&truncated, ServerTime::default());
        assert_eq!(wheel.spins_today, 0);
        assert_eq!(wheel.next_free_spin, None);
        truncated.assert_all_read();
    }

    #[test]
    fn wheel_result_update_keeps_previous_result_on_error() {
        let mut wheel = Wheel {
            result: Some(WheelReward {
                typ: WheelRewardType::Mushrooms,
                amount: 5,
            }),
            ..Default::default()
        };
        let valid = Vals::from("5/2");
        wheel.update_result(&valid, false).unwrap();
        assert_eq!(wheel.result.unwrap().typ, WheelRewardType::Item);
        valid.assert_all_read();

        let invalid = Vals::from("3/999");
        assert!(wheel.update_result(&invalid, true).is_err());
        let previous = wheel.result.unwrap();
        assert_eq!(previous.typ, WheelRewardType::Item);
        assert_eq!(previous.amount, 1);
        invalid.assert_all_read();
    }

    #[test]
    fn calendar_reward_preserves_mappings_and_unknowns() {
        let cases = [
            (1, CalendarRewardType::Silver),
            (2, CalendarRewardType::Mushrooms),
            (3, CalendarRewardType::Experience),
            (4, CalendarRewardType::Wood),
            (5, CalendarRewardType::Stone),
            (6, CalendarRewardType::Souls),
            (7, CalendarRewardType::Arcane),
            (8, CalendarRewardType::Runes),
            (10, CalendarRewardType::Item),
            (11, CalendarRewardType::Attribute(AttributeType::Strength)),
            (12, CalendarRewardType::Attribute(AttributeType::Dexterity)),
            (
                13,
                CalendarRewardType::Attribute(AttributeType::Intelligence),
            ),
            (
                14,
                CalendarRewardType::Attribute(AttributeType::Constitution),
            ),
            (15, CalendarRewardType::Attribute(AttributeType::Luck)),
            (16, CalendarRewardType::Fruit(HabitatType::Shadow)),
            (17, CalendarRewardType::Fruit(HabitatType::Light)),
            (18, CalendarRewardType::Fruit(HabitatType::Earth)),
            (19, CalendarRewardType::Fruit(HabitatType::Fire)),
            (20, CalendarRewardType::Fruit(HabitatType::Water)),
            (21, CalendarRewardType::LevelUp),
            (22, CalendarRewardType::Potion(PotionType::EternalLife)),
            (23, CalendarRewardType::TenQuicksandGlasses),
            (24, CalendarRewardType::Potion(PotionType::Strength)),
            (25, CalendarRewardType::Potion(PotionType::Dexterity)),
            (26, CalendarRewardType::Potion(PotionType::Intelligence)),
            (27, CalendarRewardType::Potion(PotionType::Constitution)),
            (28, CalendarRewardType::Potion(PotionType::Luck)),
            (999, CalendarRewardType::Unknown),
        ];
        for (raw_typ, expected_typ) in cases {
            let raw = format!("{raw_typ}/42/extra");
            let data = Vals::from(raw.as_str());
            let reward = CalendarReward::parse(&data);
            assert_eq!(reward.typ, expected_typ);
            assert_eq!(reward.amount, 42);
            data.assert_all_read();
        }

        let malformed = Vals::from("bad/also-bad");
        let reward = CalendarReward::parse(&malformed);
        assert_eq!(reward.typ, CalendarRewardType::Unknown);
        assert_eq!(reward.amount, 0);
        malformed.assert_all_read();
    }

    #[test]
    fn calendar_update_ignores_a_partial_reward_tail() {
        let mut calendar = Calendar {
            rewards: vec![CalendarReward {
                typ: CalendarRewardType::Unknown,
                amount: 0,
            }],
            ..Default::default()
        };
        let data = Vals::from("1/10/2/20/999");
        calendar.update_rewards(&data);
        data.assert_all_read();
        assert_eq!(calendar.rewards.len(), 2);
        assert_eq!(calendar.rewards[0].typ, CalendarRewardType::Silver);
        assert_eq!(calendar.rewards[0].amount, 10);
        assert_eq!(calendar.rewards[1].typ, CalendarRewardType::Mushrooms);
        assert_eq!(calendar.rewards[1].amount, 20);
    }

    #[test]
    fn events_short_response_does_not_mutate_state() {
        let ends = local_time(1_700_000_000);
        let mut events = Events {
            active: HashSet::from([Event::ExceptionalXPEvent]),
            sales: EnumMap::default(),
            ends: Some(ends),
        };
        events.sales[Sale::BlackFriday] = true;
        let data = Vals::from("0/1/2/3/4/5/6/blackfridaysale/unknownsale");
        events.update(&data, ServerTime::default());
        data.assert_all_read();
        assert_eq!(events.active, HashSet::from([Event::ExceptionalXPEvent]));
        assert!(events.sales[Sale::BlackFriday]);
        assert_eq!(events.ends, Some(ends));
    }

    #[test]
    fn events_apply_flags_sales_and_unknown_sale_warnings() {
        let mut events = Events::default();
        let data = Vals::from(
            "0/5/1700000000/0/1700000000/0/0/0/blackfridaysale/unknownsale/99",
        );
        events.update(&data, ServerTime::default());
        data.assert_all_read();
        assert_eq!(
            events.active,
            HashSet::from([Event::ExceptionalXPEvent, Event::TidyToiletTime,])
        );
        assert!(events.sales[Sale::BlackFriday]);
        assert_eq!(events.ends, Some(local_time(1_700_000_000)));

        let no_sale = Vals::from("0/0/0/0/1700000000/0/0/0");
        events.update(&no_sale, ServerTime::default());
        no_sale.assert_all_read();
        assert!(events.active.is_empty());
        assert!(!events.sales[Sale::BlackFriday]);
    }

    #[test]
    fn advent_calendar_handles_empty_zero_malformed_and_negative_values() {
        let mut specials = TimedSpecials::default();
        for raw in ["", "0/3/25/extra", "bad/3/25/extra"] {
            let data = Vals::from(raw);
            specials.update_advent_calendar(&data);
            data.assert_all_read();
            assert!(specials.advent_calendar.is_none());
        }

        let positive = Vals::from("3/25/extra");
        specials.update_advent_calendar(&positive);
        positive.assert_all_read();
        let reward = specials.advent_calendar.as_ref().unwrap();
        assert_eq!(reward.typ, RewardType::Mushrooms);
        assert_eq!(reward.amount, 25);

        let negative = Vals::from("-1/30/extra");
        specials.update_advent_calendar(&negative);
        negative.assert_all_read();
        let reward = specials.advent_calendar.as_ref().unwrap();
        assert_eq!(reward.typ, RewardType::Unknown);
        assert_eq!(reward.amount, 30);
    }

    #[test]
    fn task_parses_valid_unknown_and_malformed_values() {
        let valid = Vals::from("1/3/10/2/extra");
        let task = Task::parse(&valid);
        assert_eq!(task.typ, TaskType::DrinkBeer);
        assert_eq!(task.current, 3);
        assert_eq!(task.target, 10);
        assert_eq!(task.point_reward, 2);
        valid.assert_all_read();

        let unknown = Vals::from("999/1/2/3");
        assert_eq!(Task::parse(&unknown).typ, TaskType::Unknown);
        unknown.assert_all_read();

        let malformed = Vals::from("bad/-1/not-a-number/-5");
        let task = Task::parse(&malformed);
        assert_eq!(task.typ, TaskType::Unknown);
        assert_eq!(task.current, 0);
        assert_eq!(task.target, u64::MAX);
        assert_eq!(task.point_reward, 0);
        malformed.assert_all_read();
    }

    #[test]
    fn daily_and_event_tasks_ignore_partial_tails() {
        let mut daily = DailyTasks {
            tasks: vec![Task {
                typ: TaskType::Unknown,
                current: 0,
                target: 1,
                point_reward: 0,
            }],
            ..Default::default()
        };
        let daily_data = Vals::from("2/1/1/5/1/4/2/6/2/99");
        daily.update_tasks(&daily_data);
        daily_data.assert_all_read();
        assert_eq!(daily.tasks.len(), 2);
        assert_eq!(daily.tasks[0].current, 1);
        assert_eq!(daily.tasks[0].target, 5);
        assert_eq!(daily.tasks[1].current, 2);
        assert_eq!(daily.tasks[1].target, 6);

        let mut event = EventTasks {
            tasks: vec![Task {
                typ: TaskType::Unknown,
                current: 0,
                target: 1,
                point_reward: 0,
            }],
            ..Default::default()
        };
        let event_data = Vals::from("1/1/5/1/4/2/6/2/99");
        event.update_tasks(&event_data);
        event_data.assert_all_read();
        assert_eq!(event.tasks.len(), 2);
        assert_eq!(event.tasks[0].point_reward, 1);
        assert_eq!(event.tasks[1].point_reward, 2);
    }

    #[test]
    fn event_task_info_handles_theme_sentinels_unknowns_and_extras() {
        let cases = [
            ("1700000000/1700000100/0/extra", EventTaskTheme::Unknown),
            ("1700000000/1700000100/-1/extra", EventTaskTheme::Unknown),
            ("1700000000/1700000100/245/extra", EventTaskTheme::Unknown),
            ("1700000000/1700000100/2/extra", EventTaskTheme::Gambler),
            ("1700000000/1700000100/999/extra", EventTaskTheme::Unknown),
        ];
        for (raw, expected_theme) in cases {
            let data = Vals::from(raw);
            let mut info = EventTasks::default();
            info.update_info(&data, ServerTime::default());
            data.assert_all_read();
            assert_eq!(info.theme, expected_theme);
            assert_eq!(info.start, Some(local_time(1_700_000_000)));
            assert_eq!(info.end, Some(local_time(1_700_000_100)));
        }
    }

    #[test]
    fn reward_parse_vals_uses_tolerant_field_defaults() {
        let valid = Vals::from("4/25/extra");
        let reward = Reward::parse_vals(&valid);
        assert_eq!(reward.typ, RewardType::Silver);
        assert_eq!(reward.amount, 25);
        valid.assert_all_read();

        let malformed = Vals::from("bad/-1");
        let reward = Reward::parse_vals(&malformed);
        assert_eq!(reward.typ, RewardType::Unknown);
        assert_eq!(reward.amount, 0);
        malformed.assert_all_read();
    }

    #[test]
    fn reward_chest_parses_full_data_and_rejects_truncated_counts() {
        let full = Vals::from("1/100/2/4/10/5/20");
        let chest = RewardChest::parse(&full).unwrap();
        assert!(chest.opened);
        assert_eq!(chest.required_points, 100);
        assert_eq!(chest.rewards.len(), 2);
        assert_eq!(chest.rewards[0].typ, RewardType::Silver);
        assert_eq!(chest.rewards[0].amount, 10);
        assert_eq!(chest.rewards[1].typ, RewardType::LuckyCoins);
        assert_eq!(chest.rewards[1].amount, 20);
        full.assert_all_read();

        for raw in ["0/100/2/4/10", "0/100/999999999"] {
            let truncated = Vals::from(raw);
            assert!(matches!(
                RewardChest::parse(&truncated),
                Err(SFError::ParsingError("reward chest count", _))
            ));
            truncated.assert_all_read();
        }

        let bad_header = Vals::from("0/not-points/0");
        assert!(RewardChest::parse(&bad_header).is_err());
        bad_header.assert_all_read();
    }

    #[test]
    fn parse_rewards_preserves_per_chest_fallback_without_advancing_on_error() {
        let data = Vals::from("0/10/1/4/5/0/not-points/0/4/5/6");
        let chests = parse_rewards(&data);
        data.assert_all_read();
        assert_eq!(chests[0].required_points, 10);
        assert_eq!(chests[0].rewards.len(), 1);
        assert_eq!(chests[0].rewards[0].typ, RewardType::Silver);
        assert_eq!(chests[0].rewards[0].amount, 5);
        for fallback in &chests[1..] {
            assert!(!fallback.opened);
            assert_eq!(fallback.required_points, 0);
            assert!(fallback.rewards.is_empty());
        }
    }

    #[test]
    fn parse_rewards_ignores_values_after_the_third_chest() {
        let data = Vals::from("0/10/0/0/20/0/0/30/0/999/888");
        let chests = parse_rewards(&data);
        data.assert_all_read();
        assert_eq!(chests[0].required_points, 10);
        assert_eq!(chests[1].required_points, 20);
        assert_eq!(chests[2].required_points, 30);
        assert!(chests.iter().all(|chest| chest.rewards.is_empty()));
    }
}
