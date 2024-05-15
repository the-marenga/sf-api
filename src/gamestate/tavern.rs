use chrono::{DateTime, Local};
use log::error;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{items::Item, SFError, ServerTime};
use crate::{
    command::{DiceReward, DiceType},
    gamestate::rewards::Reward,
    misc::{soft_into, warning_parse},
};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tavern {
    /// All the available quests
    pub quests: [Quest; 3],
    /// How many seconds the character still has left to do adventures
    #[doc(alias = "alu")]
    pub thirst_for_adventure_sec: u32,
    /// Whether or not skipping is allowed
    pub skip_allowed: bool,
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
    /// Informations about everything related to expeditions
    pub expeditions: ExpeditionsEvent,
    /// The result of playing the shell game
    pub gamble_result: Option<GambleResult>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Informations about everything related to expeditions
pub struct ExpeditionsEvent {
    /// The time the expeditions mechanic was enabled at
    pub start: Option<DateTime<Local>>,
    /// The time until which expeditions will be available
    pub end: Option<DateTime<Local>>,
    /// The expeditions available to do
    pub available: Vec<AvailableExpedition>,
    /// The expedition the player is currenty doing. Accessable via the
    /// `active()` method.
    pub(crate) active: Option<Expedition>,
}

impl ExpeditionsEvent {
    /// Expeditions finish after the last timer elapses. That means, this can
    /// happen without any new requests. To make sure you do not access an
    /// expedition, that has elapsed, you access expeditions with this
    pub fn active(&self) -> Option<&Expedition> {
        self.active.as_ref().filter(|a| !a.is_finished())
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the current state of the dice game
pub struct DiceGame {
    /// The amount of dice games you can still play today
    pub remaining: u8,
    /// The next free dice game can be played at this point in time
    pub next_free: Option<DateTime<Local>>,
    /// These are the dices, that are laying on the table after the first
    /// round. The ones you can select to keep from
    pub current_dice: Vec<DiceType>,
    /// Whatever we won in the dice game
    pub reward: Option<DiceReward>,
}

impl Tavern {
    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.current_action = CurrentAction::parse(
            data[45] & 0xFF,
            data[46] & 0xFF,
            server_time.convert_to_local(data[47], "current action time"),
        );
        self.thirst_for_adventure_sec =
            soft_into(data[456], "remaining ALU", 0);
        self.beer_drunk = soft_into(data[457], "beer drunk count", 0);

        for (quest_index, start_idx) in (235..=237).enumerate() {
            self.quests[quest_index] =
                Quest::parse(&data[start_idx..], quest_index, server_time)?
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quest {
    /// The legth of this quest in sec (without item enchantment)
    pub base_length: u32,
    /// The silver reward for this quest (without item enchantment)
    pub base_silver: u32,
    /// The xp reward for this quest (without item enchantment)
    pub base_experience: u32,
    /// The item reward for this quest
    pub item: Option<Item>,
    /// The place where this quest takes place. Usefull for the scrapbook
    pub location_id: Location,
    /// The enemy you fight in this quest. Usefull for the scrapbook
    pub monster_id: u16,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Copy, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The background/location for a quest, or another activity
pub enum Location {
    #[default]
    SprawlingJungle = 1,
    SkullIsland,
    EvernightForest,
    StumbleSteppe,
    ShadowrockMountain,
    SplitCanyon,
    BlackWaterSwamp,
    FloodedCaldwell,
    TuskMountain,
    MoldyForest,
    Nevermoor,
    BustedLands,
    Erogenion,
    Magmaron,
    SunburnDesert,
    Gnarogrim,
    Northrunt,
    BlackForest,
    Maerwynn,
    PlainsOfOzKorr,
    RottenLands,
}

impl Quest {
    /// Checks if this is a red quest, which means a special enemy + extra
    /// rewards
    pub fn is_red(&self) -> bool {
        matches!(self.monster_id, 139 | 145 | 148 | 152 | 155 | 157)
    }

    pub(crate) fn parse(
        data: &[i64],
        quest_index: usize,
        server_time: ServerTime,
    ) -> Result<Quest, SFError> {
        Ok(Quest {
            base_length: soft_into(data[6], "quest length", 100_000),
            base_silver: soft_into(data[48], "quest silver", 0),
            base_experience: soft_into(data[45], "quest xp", 0),
            item: Item::parse(&data[9 + quest_index * 11..], server_time)?,
            location_id: warning_parse(data[3], "quest location id", |a| {
                FromPrimitive::from_i64(a)
            })
            .unwrap_or_default(),
            monster_id: soft_into(-data[0], "quest monster id", 0),
        })
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CurrentAction {
    #[default]
    /// The character is not doing anything and can basically do anything
    Idle,
    CityGuard {
        hours: u8,
        busy_until: DateTime<Local>,
    },
    Quest {
        quest_idx: u8,
        busy_until: DateTime<Local>,
    },
    Expedition,
    /// The character is not able to do something, but we do not know what.
    /// Most likely something from a new update
    Unkown(Option<DateTime<Local>>),
}

impl CurrentAction {
    pub(crate) fn parse(
        id: i64,
        sec: i64,
        busy: Option<DateTime<Local>>,
    ) -> Self {
        match (id, busy) {
            (0, None) => CurrentAction::Idle,
            (1, Some(busy_until)) => CurrentAction::CityGuard {
                hours: soft_into(sec, "city guard time", 10),
                busy_until,
            },
            (2, Some(busy_until)) => CurrentAction::Quest {
                quest_idx: soft_into(sec, "quest index", 0),
                busy_until,
            },
            (4, None) => CurrentAction::Expedition,
            _ => {
                error!("Unknown action id combination: {id}, {busy:?}");
                CurrentAction::Unkown(busy)
            }
        }
    }
}

#[derive(Debug, Clone, Default, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Toilet {
    /// Has the toilet been used today?
    pub used: bool,
    /// The level the aura is at currently
    pub aura: u32,
    /// The amount of mana currently in the toilet
    pub mana_currently: u32,
    /// The total amount of mana you have to collect to flush the toilet
    pub mana_total: u32,
}

impl Toilet {
    pub(crate) fn update(&mut self, data: &[i64]) {
        self.aura = soft_into(data[491], "aura level", 0);
        self.mana_currently = soft_into(data[492], "aura now", 0);
        self.mana_total = soft_into(data[515], "aura missing", 1000);
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Expedition {
    /// The items
    pub items: [Option<ExpeditionThing>; 4],

    /// The thing, that we are searching on this expedition
    pub target_thing: ExpeditionThing,
    /// The amount of the target item we have found
    pub target_current: u8,
    /// The amount of the target item that we are supposed to find
    pub target_amount: u8,

    /// The level we are currently clearing. Starts at 1
    pub(crate) current_floor: u8,
    ///  The heroism we have collected so far
    pub heroism: i32,

    /// Choose one of these rewards
    pub(crate) halftime_rewards: Vec<Reward>,
    pub(crate) halftime_for_boss_id: i64,
    /// If we encountered a boss, this will contain information about it
    pub(crate) boss: ExpeditionBoss,
    /// The different crossroads, that you can choose between. Should be 3
    pub(crate) crossroads: Vec<ExpeditionEncounter>,
    pub(crate) busy_until: Option<DateTime<Local>>,
}

impl Expedition {
    #[must_use]
    /// Returns the current floor the player is on. This is dependant on time,
    /// because the timers are lazily evaluated. That means it might flip
    /// from 5->6, 10 -> None between calls. None here means the expedition
    /// is finished
    pub fn current_floor(&self) -> Option<u8> {
        if matches!(self.current_floor, 5 | 10) {
            if let Some(busy_until) = self.busy_until {
                if busy_until < Local::now() {
                    if self.current_floor == 5 {
                        return Some(6);
                    }
                    return None;
                }
            }
        }
        Some(self.current_floor)
    }

    #[must_use]
    /// Returns the current stage the player is doing. This is dependant on
    /// time, because the timers are lazily evaluated. That means it might
    /// flip from Waiting->Crossroads/Finished between calls
    pub fn current_stage(&self) -> ExpeditionStage {
        match self.current_floor() {
            None => ExpeditionStage::Finished,
            Some(5 | 10) => {
                if self.halftime_rewards.is_empty()
                    || self.halftime_for_boss_id != self.boss.id
                {
                    return ExpeditionStage::Boss(self.boss);
                }
                ExpeditionStage::Rewards(self.halftime_rewards.clone())
            }
            _ => ExpeditionStage::Crossroads(self.crossroads.clone()),
        }
    }

    #[must_use]
    /// Cheks, if the last timer of this expedition has run out
    pub fn is_finished(&self) -> bool {
        matches!(self.current_stage(), ExpeditionStage::Finished)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExpeditionStage {
    /// Choose one of these rewards after winning against the boss
    Rewards(Vec<Reward>),
    /// If we encountered a boss, this will contain information about it
    Boss(ExpeditionBoss),
    /// The different crossroads, that you can choose between. Should be 3
    Crossroads(Vec<ExpeditionEncounter>),
    /// We have to wait until the specified time to continue in the expedition.
    /// When this is `< Local::now()`, you can can send teh update command to
    /// update the expedition stage, which will make `current_stage()` yield
    /// the new crossroads
    Waiting(DateTime<Local>),
    /// The expedition has finished and you can choose another one
    Finished,
}

impl Default for ExpeditionStage {
    fn default() -> Self {
        ExpeditionStage::Crossroads(Vec::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExpeditionBoss {
    /// The monster id of this boss
    pub id: i64,
    /// The amount of items this boss is supposed to drop
    pub items: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExpeditionEncounter {
    pub typ: ExpeditionThing,
    /// The heroism you get from picking this encounter. Note that this is only
    /// the base  value, so for the alst encounter, or if you have a
    /// corresponding item, you may need to adjust this +10
    // TODO: Do this automatically
    pub base_heroism: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExpeditionThing {
    #[default]
    Unknown = 0,

    Dummy1 = 1,
    Dummy2 = 2,
    Dumy3 = 3,

    ToiletPaper = 11,

    Bait = 21,
    Dragon = 22,

    CampFire = 31,
    Phoenix = 32,
    BurntCampfire = 33,

    UnicornHorn = 41,
    Donkey = 42,
    Rainbow = 43,
    Unicorn = 44,

    CupCake = 51,

    Cake = 61,

    SmallHurdle = 71,
    BigHurdle = 72,
    WinnersPodium = 73,

    Socks = 81,
    ClothPile = 82,
    RevealingCouple = 83,

    SwordInStone = 91,
    BentSword = 92,
    BrokenSword = 93,

    Well = 101,
    Girl = 102,
    Balloons = 103,

    Prince = 111,
    RoyalFrog = 112,

    Hand = 121,
    Feet = 122,
    Body = 123,
    Klaus = 124,

    Key = 131,
    Suitcase = 132,

    // Dont know if they all exist tbh
    DummyBounty = 1000,
    ToiletPaperBounty = 1001,
    DragonBounty = 1002,
    BurntCampfireBounty = 1003,
    UnicornBounty = 1004,
    WinnerPodiumBounty = 1007,
    RevealingCoupleBounty = 1008,
    BrokenSwordBounty = 1009,
    BaloonBounty = 1010,
    FrogBounty = 1011,
    KlausBounty = 1012,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AvailableExpedition {
    pub target: ExpeditionThing,
    pub thirst_for_adventure_sec: u32,

    // No idea how these work
    pub(crate) location1: Location,
    pub(crate) location2: Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GambleResult {
    SilverChange(i64),
    MushroomChange(i32),
}
