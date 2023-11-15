use chrono::{DateTime, Local};
use log::error;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{items::Item, ServerTime, WheelReward};
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
    /// How many seconds of ALU the character still has left
    pub alu_sec: u32,
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
    /// The amount of dice games you can still play today
    pub dice_games_remaining: u8,
    /// The next free dice game can be played at this point in time
    pub dice_games_next_free: Option<DateTime<Local>>,
    /// These are the dices, that are laying on the table after the first
    /// round. The ones you can select to keep from
    pub current_dice: Vec<DiceType>,
    /// Whatever we won in the dice game
    pub dice_reward: Option<DiceReward>,

    pub wheel_result: Option<WheelReward>,

    pub expedition_start: Option<DateTime<Local>>,
    pub expedition_end: Option<DateTime<Local>>,

    pub expeditions: Option<[ExpeditionInfo; 2]>,

    pub expedition: Option<Expedition>,
}

impl Tavern {
    pub(crate) fn update(&mut self, data: &[i64], server_time: ServerTime) {
        self.current_action = CurrentAction::parse(
            data[45] & 0xFF,
            data[46] & 0xFF,
            server_time.convert_to_local(data[47], "current action time"),
        );
        self.alu_sec = soft_into(data[456], "remaining ALU", 0);
        self.beer_drunk = soft_into(data[457], "beer drunk count", 0);

        for (quest_index, start_idx) in (235..=237).enumerate() {
            self.quests[quest_index] =
                Quest::parse(&data[start_idx..], quest_index, server_time)
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quest {
    /// The legth of this quest in sec (without item enchantment)
    pub base_length: u32,
    /// The silver reward for this quest (without item enchantment)
    pub base_silver: u32,
    /// The xp reward for this quest  (without item enchantment)
    pub base_experience: u32,
    /// The item reward for this quest
    pub item: Option<Item>,
    /// The place where this quest takes place. Usefull for the scrapbook
    pub location_id: QuestLocation,
    /// The enemy you fight in this quest. Usefull for the scrapbook
    pub monster_id: u16,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Copy, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QuestLocation {
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
    ) -> Quest {
        Quest {
            base_length: soft_into(data[6], "quest length", 100_000),
            base_silver: soft_into(data[48], "quest silver", 0),
            base_experience: soft_into(data[45], "quest xp", 0),
            item: Item::parse(&data[9 + quest_index * 11..], server_time),
            location_id: warning_parse(data[3], "quest location id", |a| {
                FromPrimitive::from_i64(a)
            })
            .unwrap_or_default(),
            monster_id: soft_into(-data[0], "quest monster id", 0),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CurrentAction {
    #[default]
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
    pub used: bool,
    pub aura_level: u32,
    pub aura_now: u32,
    pub aura_missing: u32,
}

impl Toilet {
    pub(crate) fn update(&mut self, data: &[i64]) {
        self.aura_level = soft_into(data[491], "aura level", 0);
        self.aura_now = soft_into(data[492], "aura now", 0);
        self.aura_missing = soft_into(data[515], "aura missing", 1000);
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Expedition {
    pub crossroads: [ExpeditionEncounter; 3],
    pub items: [Option<ExpeditionThing>; 4],

    /// The amount of the target item we have found
    pub current: u8,
    /// The amount of the target item that we are supposed to find
    pub target_amount: u8,
    /// The level we are currently clearing
    pub clearing: u8,

    pub heroism: i32,

    pub target: ExpeditionThing,

    pub boss: Option<ExpeditionBoss>,

    pub halftime_choice: Vec<Reward>,

    pub busy_until: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExpeditionBoss {
    pub id: i64,
    /// The amount of items this boss is supposed to drop
    pub items: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExpeditionEncounter {
    pub typ: ExpeditionThing,
    /// Note that this value will be +10 for the last one
    pub heroism: i32,
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
pub struct ExpeditionInfo {
    pub target: ExpeditionThing,
    pub alu_sec: u32,

    // No idea how these work
    pub(crate) location1_id: i64,
    pub(crate) location2_id: i64,
}
