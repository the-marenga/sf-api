use std::collections::HashSet;

use chrono::{DateTime, Local};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use super::{
    IdleBuildingType, LightDungeon, Mount, Result, ServerTime, ShopType,
    character::Class,
    items::{PetItem, PotionType},
    tavern::Location,
    unlockables::HabitatType,
};
use crate::{command::AttributeType, error::SFError, response::Vals};

/// The type of a reward you can win by spinning the wheel. The wheel can be
/// upgraded, so some rewards may not always be available
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum WheelRewardType {
    /// Mushrooms currency.
    Mushrooms,
    /// Stone fortress resource.
    Stone,
    /// Large stone fortress resource.
    StoneXL,
    /// Wood fortress resource.
    Wood,
    /// Large wood fortress resource.
    WoodXL,
    /// Experience points.
    Experience,
    /// Large experience points.
    ExperienceXL,
    /// Silver coins.
    Silver,
    /// Large silver coins.
    SilverXL,
    /// Arcane splinters blacksmith resource.
    Arcane,
    /// Souls underworld resource.
    Souls,
    /// An equipment item or potion.
    Item,
    /// A pet food or egg item.
    PetItem(PetItem),
    /// An unknown wheel reward type.
    #[default]
    Unknown,
}

/// The thing you won from spinning the wheel
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WheelReward {
    /// The type of item you have won
    pub typ: WheelRewardType,
    /// The amount of the type you have won
    pub amount: i64,
}

impl WheelReward {
    /// Parses a wheel reward from a 2-value window: `[raw_typ, amount]`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        upgraded: bool,
    ) -> Result<WheelReward> {
        let raw_typ = data.i64(0)?;
        let mut amount = data.i64(1)?;
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

/// Information about the events active in the tavern
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Events {
    /// All of the events active in the tavern
    pub active: HashSet<Event>,
    /// The time at which all of the events end. Mostly just Sunday 23:59.
    pub ends: Option<DateTime<Local>>,
}

impl Events {
    /// Updates the active events and their end time from an `events` response:
    /// `theme / flags / theme_end / ? / event_end / ...`.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        if data.len() < 8 {
            data.ignore_all("too short events response");
            return Ok(());
        }
        data.ignore(0, "event theme / tavern decoration");
        self.active.clear();
        let flags = data.u64(1)?;
        for (idx, event) in Event::iter().enumerate() {
            if (flags & (1_u64 << idx)) != 0 {
                self.active.insert(event);
            }
        }
        data.ignore(2, "theme end timestamp");
        data.ignore(3, "unknown flag");
        self.ends = data.server_time(4, server_time)?;
        data.ignore(5..9, "trailing event values");
        Ok(())
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

/// The tasks you get from the goblin gleeman
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tasks {
    /// The tasks, that update daily
    pub daily: DailyTasks,
    /// The tasks, that follow some server wide theme
    pub event: EventTasks,
}

/// Information about the tasks, that reset every day
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DailyTasks {
    /// The tasks you have to do
    pub tasks: Vec<Task>,
    /// The rewards available for completing tasks.
    pub rewards: [RewardChest; 3],
}

impl DailyTasks {
    /// Updates the daily tasks from a `dailytasklist` response.
    /// The first value is ignored, followed by repeating 4-value task records:
    /// `typ / current / target / point_reward`.
    pub(crate) fn update_tasks(&mut self, data: &Vals<'_>) -> Result<()> {
        data.ignore(0, "amount of > 1 bell quests");
        self.tasks.clear();
        for chunk in data.window(1..data.len()).chunks(4) {
            self.tasks.push(Task::parse(&chunk)?);
        }
        Ok(())
    }

    /// Updates the daily task reward chests from a `dailytaskrewardpreview`
    /// response: 3 variable-length chest records.
    pub(crate) fn update_rewards(&mut self, data: &Vals<'_>) -> Result<()> {
        self.rewards = parse_chests(data)?;
        Ok(())
    }
}

/// Information about the tasks, that are based on some event theme
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

impl EventTasks {
    /// Updates event tasks from an `eventtasklist` response:
    /// repeating 4-value task records:
    /// `typ / current / target / point_reward`.
    pub(crate) fn update_tasks(&mut self, data: &Vals<'_>) -> Result<()> {
        self.tasks.clear();
        for chunk in data.chunks(4) {
            self.tasks.push(Task::parse(&chunk)?);
        }
        Ok(())
    }

    /// Updates event task reward chests from an `eventtaskrewardpreview`
    /// response: 3 variable-length chest records.
    pub(crate) fn update_rewards(&mut self, data: &Vals<'_>) -> Result<()> {
        self.rewards = parse_chests(data)?;
        Ok(())
    }

    /// Updates event task information from a 3-value `eventtaskinfo` response:
    /// `start / end / theme`.
    pub(crate) fn update_info(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.start = data.server_time(0, server_time)?;
        self.end = data.server_time(1, server_time)?;
        self.theme = data.enum_or(2, EventTaskTheme::Unknown);
        Ok(())
    }
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
    /// Whether the task requirements have been satisfied
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.current >= self.target
    }
}

/// Dr. Abawuwu's wheel
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Wheel {
    /// The amount of lucky coins you have to spin the wheel
    pub lucky_coins: u32,
    /// The amount of times you have spun the wheel today already (0 -> 20)
    pub spins_today: u8,
    /// The next time you can spin the wheel for free
    pub next_free_spin: Option<DateTime<Local>>,
    /// The result of spinning the wheel
    pub result: Option<WheelReward>,
}

impl Wheel {
    /// Updates the wheel from a 3-value `wheel` response:
    /// `? / spins_today / next_free_spin`.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        data.ignore(0, "unknown wheel state or version");
        self.spins_today = data.u8(1)?;
        self.next_free_spin = data.server_time(2, server_time)?;
        Ok(())
    }

    /// Updates the wheel result from a 2-value `wheelresult` response:
    /// `typ / amount`.
    pub(crate) fn update_result(
        &mut self,
        data: &Vals<'_>,
        upgraded: bool,
    ) -> Result<()> {
        self.result = Some(WheelReward::parse(data, upgraded)?);
        Ok(())
    }
}

/// The theme the event tasks have
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum EventTaskTheme {
    // 1 is not set
    /// Gambler theme.
    Gambler = 2,
    /// Rank climber theme.
    RankClimber = 3,
    /// Shopping spree theme.
    ShoppingSpree = 4,
    /// Time skipper theme.
    TimeSkipper = 5,
    /// Ruffian reset theme.
    RuffianReset = 6,
    /// Part time nudist theme.
    PartTimeNudist = 7,
    /// Scrimper theme.
    Scrimper = 8,
    /// Scholar theme.
    Scholar = 9,
    /// Maximizer theme.
    Maximizer = 10,
    /// Underworld figure theme.
    UnderworldFigure = 11,
    /// Egg hunt theme.
    EggHunt = 12,
    /// Summer collectifun theme.
    SummerCollectifun = 13,
    /// Walpurgis theme.
    Walpurgis = 14,
    /// Pet trainer theme.
    PetTrainer = 15,
    /// Fortress master theme.
    FortressMaster = 16,
    /// Legendary dungeon theme.
    LegendaryDungeon = 17,
    /// Hellevator theme.
    Hellevator = 18,
    /// Unknown theme.
    #[default]
    Unknown = 245,
}

/// The type of task you have to do
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
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

    #[default]
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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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
    /// Parses a task from a 4-value slice:
    /// `typ / current / target / point_reward`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Task> {
        let raw_typ = data.i64(0)?;
        let typ = TaskType::parse(raw_typ);

        if typ == TaskType::Unknown {
            warn!("Unknown task: {data} {raw_typ}");
        }
        Ok(Task {
            typ,
            current: data.u64_or(1, 0),
            target: data.u64_or(2, u64::MAX),
            point_reward: data.u32_or(3, 0),
        })
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

/// The type of event, that is currently happening on the server
#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    /// Exceptional XP event (+100% XP from quests).
    ExceptionalXPEvent = 0,
    /// Glorious Gold Galore event (extra gold).
    GloriousGoldGalore,
    /// Tidy Toilet Time event (mana from toilet).
    TidyToiletTime,
    /// Assembly of Awesome Animals event (pet food/find chance).
    AssemblyOfAwesomeAnimals,
    /// Fantastic Fortress Festivity event (faster fortress production/search).
    FantasticFortressFestivity,
    /// Days of Doomed Souls event (extra souls).
    DaysOfDoomedSouls,
    /// Witches' Dance event (witch cauldron boosts).
    WitchesDance,
    /// Sands of Time Special event (more hourglasses).
    SandsOfTimeSpecial,
    /// Forge Frenzy Festival event (blacksmith resources).
    ForgeFrenzyFestival,
    /// Epic Shopping Spree Extravaganza event (increased epic chance in shops).
    EpicShoppingSpreeExtravaganza,
    /// Epic Quest Extravaganza event (epics from quests).
    EpicQuestExtravaganza,
    /// Epic Good Luck Extravaganza event (wheel dice lucky rewards).
    EpicGoodLuckExtravaganza,
    /// One Beer, Two Beer, Free Beer event (free tavern beer).
    OneBeerTwoBeerFreeBeer,
    /// Piecework Party event (quick work).
    PieceworkParty,
    /// Lucky Day event (lucky coins and wheel spins).
    LuckyDay,
    /// Crazy Mushroom Harvest event (20% more mushrooms).
    CrazyMushroomHarvest,
    /// Holiday sale event.
    HolidaySale,
    /// Valentine's blessing event.
    ValentinesBlessing,
    /// Black Gem Rush event (higher black gem chance).
    BlackGemRush,
    /// Rumble for Riches event.
    RumbleForRiches,
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
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        clippy::assert_is_empty
    )]
    use super::*;

    #[test]
    fn test_wheel_reward_parse() {
        crate::parse_test!(WheelReward("0/5", false), {
            typ: WheelRewardType::Mushrooms,
            amount: 5,
        });

        crate::parse_test!(WheelReward("1/10", true), {
            typ: WheelRewardType::Arcane,
            amount: 10,
        });

        crate::parse_test!(WheelReward("1/10", false), {
            typ: WheelRewardType::Wood,
            amount: 10,
        });

        let data = Vals::from("3/1");
        let reward = WheelReward::parse(&data, true).unwrap();
        assert_eq!(
            reward.typ,
            WheelRewardType::PetItem(PetItem::Egg(HabitatType::Shadow))
        );
        assert_eq!(reward.amount, 1);
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_wheel_update() {
        let raw = "2/0/1762300801";
        let mut wheel = Wheel::default();
        let server_time = ServerTime::default();
        let data = Vals::from(raw);
        wheel.update(&data, server_time).unwrap();
        assert_eq!(wheel.spins_today, 0);
        assert!(wheel.next_free_spin.is_some());
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_wheel_update_result() {
        let mut wheel = Wheel::default();
        let data = Vals::from("0/5");
        wheel.update_result(&data, false).unwrap();
        assert_eq!(
            wheel.result,
            Some(WheelReward {
                typ: WheelRewardType::Mushrooms,
                amount: 5,
            })
        );
        assert!(data.unaccessed_indices().is_empty());
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
        assert!(data.unaccessed_indices().is_empty());
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
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_events_update() {
        let raw = "52/2097664/1792108740/0/1792108740/0/0/0/0";
        let mut events = Events::default();
        let data = Vals::from(raw);
        events.update(&data, ServerTime::default()).unwrap();
        assert!(events.ends.is_some());
        assert!(!events.active.is_empty());
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_task_parse() {
        crate::parse_test!(Task("2/0/320/1"), {
            typ: TaskType::ConsumeThirstForAdventure,
            current: 0,
            target: 320,
            point_reward: 1,
        });
    }

    #[test]
    fn test_daily_tasks_update_tasks() {
        let raw = "4/2/0/320/1/3/0/10/2/4/0/20/2/5/0/1/1/6/0/1/1/7/0/1/1/8/0/\
                   5/1/9/0/5/1/10/0/1/1/11/0/10/1/12/0/5/1/13/0/1/1/133/0/5/1/\
                   18/0/1/2/136/0/3/2/57/0/1/2/14/0/3/4";
        let mut daily = DailyTasks::default();
        let data = Vals::from(raw);
        daily.update_tasks(&data).unwrap();
        assert_eq!(daily.tasks.len(), 17);
        assert_eq!(daily.tasks[0].typ, TaskType::ConsumeThirstForAdventure);
        assert_eq!(daily.tasks[0].target, 320);
        assert_eq!(daily.tasks[16].typ, TaskType::DefeatGambler);
        assert_eq!(daily.tasks[16].point_reward, 4);
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_daily_tasks_update_rewards() {
        let raw = "0/5/1/24/4291/0/10/1/24/4291/0/13/1/4/271840";
        let mut daily = DailyTasks::default();
        let data = Vals::from(raw);
        daily.update_rewards(&data).unwrap();
        assert!(!daily.rewards[0].opened);
        assert_eq!(daily.rewards[0].required_points, 5);
        assert_eq!(daily.rewards[0].rewards.len(), 1);
        assert_eq!(
            daily.rewards[0].rewards[0],
            Reward {
                typ: RewardType::XP,
                amount: 4291,
            }
        );
        assert_eq!(daily.rewards[2].required_points, 13);
        assert_eq!(
            daily.rewards[2].rewards[0],
            Reward {
                typ: RewardType::Silver,
                amount: 271_840,
            }
        );
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_event_tasks_update_tasks() {
        let raw = "77/0/10/1/57/0/10/1/75/0/10/1/76/0/10/1";
        let mut event_tasks = EventTasks::default();
        let data = Vals::from(raw);
        event_tasks.update_tasks(&data).unwrap();
        assert_eq!(event_tasks.tasks.len(), 4);
        assert_eq!(
            event_tasks.tasks[0].typ,
            TaskType::WinFightsNoEpicsLegendaries
        );
        assert_eq!(event_tasks.tasks[3].typ, TaskType::WinFightsNoGear);
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_event_tasks_update_rewards() {
        let raw = "0/1/1/26/50/0/2/1/4/543680/0/3/2/4/135920/28/1";
        let mut event_tasks = EventTasks::default();
        let data = Vals::from(raw);
        event_tasks.update_rewards(&data).unwrap();
        assert_eq!(event_tasks.rewards[0].required_points, 1);
        assert_eq!(event_tasks.rewards[0].rewards.len(), 1);
        assert_eq!(
            event_tasks.rewards[0].rewards[0].typ,
            RewardType::QuicksandGlass
        );
        assert_eq!(event_tasks.rewards[2].required_points, 3);
        assert_eq!(event_tasks.rewards[2].rewards.len(), 2);
        assert_eq!(event_tasks.rewards[2].rewards[0].typ, RewardType::Silver);
        assert_eq!(event_tasks.rewards[2].rewards[1].typ, RewardType::Beer);
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_event_tasks_update_info() {
        let raw = "1785110400/1785455999/7";
        let mut event_tasks = EventTasks::default();
        let data = Vals::from(raw);
        event_tasks
            .update_info(&data, ServerTime::default())
            .unwrap();
        assert!(event_tasks.start.is_some());
        assert!(event_tasks.end.is_some());
        assert_eq!(event_tasks.theme, EventTaskTheme::PartTimeNudist);
        assert!(data.unaccessed_indices().is_empty());
    }

    #[test]
    fn test_timed_specials_update_advent_calendar() {
        let mut specials = TimedSpecials::default();
        let data = Vals::from("0");
        specials.update_advent_calendar(&data).unwrap();
        assert_eq!(specials.advent_calendar, None);
        assert!(data.unaccessed_indices().is_empty());

        let data = Vals::from("3/10");
        specials.update_advent_calendar(&data).unwrap();
        assert_eq!(
            specials.advent_calendar,
            Some(Reward {
                typ: RewardType::Mushrooms,
                amount: 10,
            })
        );
        assert!(data.unaccessed_indices().is_empty());
    }
}
