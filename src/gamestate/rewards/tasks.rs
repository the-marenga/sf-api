use chrono::{DateTime, Local};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{RewardChest, parse_chests};
use crate::{
    command::AttributeType,
    error::Result,
    gamestate::{
        IdleBuildingType, LightDungeon, ServerTime, ShopType, character::Class,
        items::PotionType, tavern::Location, unlockables::HabitatType,
    },
    response::Vals,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamestate::rewards::{Reward, RewardType};

    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
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
        assert_all_read(&data);
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
        assert_all_read(&data);
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
        assert_all_read(&data);
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
        assert_all_read(&data);
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
        assert_all_read(&data);
    }
}
