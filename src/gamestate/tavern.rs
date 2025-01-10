use chrono::{DateTime, Local};
use log::{error, warn};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{
    items::Item, ArrSkip, CCGet, CFPGet, CGet, CSTGet, ExpeditionSetting,
    SFError, ServerTime,
};
use crate::{
    command::{DiceReward, DiceType},
    gamestate::rewards::Reward,
    misc::soft_into,
};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Anything related to things you can do in the tavern
pub struct Tavern {
    /// All the available quests
    pub quests: [Quest; 3],
    /// How many seconds the character still has left to do adventures
    #[doc(alias = "alu")]
    pub thirst_for_adventure_sec: u32,
    /// Whether or not skipping with mushrooms is allowed
    pub mushroom_skip_allowed: bool,
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
    /// Information about everything related to expeditions
    pub expeditions: ExpeditionsEvent,
    /// Decides if you can on on expeditions, or quests, when this event is
    /// currently ongoing
    pub questing_preference: ExpeditionSetting,
    /// The result of playing the shell game
    pub gamble_result: Option<GambleResult>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about everything related to expeditions
pub struct ExpeditionsEvent {
    /// The time the expeditions mechanic was enabled at
    pub start: Option<DateTime<Local>>,
    /// The time until which expeditions will be available
    pub end: Option<DateTime<Local>>,
    /// The expeditions available to do
    pub available: Vec<AvailableExpedition>,
    /// The expedition the player is currently doing. Accessible via the
    /// `active()` method.
    pub(crate) active: Option<Expedition>,
}

impl ExpeditionsEvent {
    /// Checks if the event has started and not yet ended compared to the
    /// current time
    #[must_use]
    pub fn is_event_ongoing(&self) -> bool {
        let now = Local::now();
        matches!((self.start, self.end), (Some(start), Some(end)) if end > now && start < now)
    }

    /// Expeditions finish after the last timer elapses. That means, this can
    /// happen without any new requests. To make sure you do not access an
    /// expedition, that has elapsed, you access expeditions with this
    #[must_use]
    pub fn active(&self) -> Option<&Expedition> {
        self.active.as_ref().filter(|a| !a.is_finished())
    }

    /// Expeditions finish after the last timer elapses. That means, this can
    /// happen without any new requests. To make sure you do not access an
    /// expedition, that has elapsed, you access expeditions with this
    #[must_use]
    pub fn active_mut(&mut self) -> Option<&mut Expedition> {
        self.active.as_mut().filter(|a| !a.is_finished())
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

#[derive(Debug, Clone)]
#[allow(missing_docs)]
/// The tasks you will presented with, when clicking the person in the tavern.
/// Make sure you are not currently busy and have enough ALU/thirst of adventure
/// before trying to start them
pub enum AvailableTasks<'a> {
    Quests(&'a [Quest; 3]),
    Expeditions(&'a [AvailableExpedition]),
}

impl Tavern {
    /// Checks if the player is currently doing anything. Note that this may
    /// change between calls, as expeditions finish without sending any collect
    /// commands. In most cases you should match on the `current_action`
    /// yourself and collect/wait, if necessary, but if you want a quick sanity
    /// check somewhere, to make sure you are idle, this is the function for you
    #[must_use]
    pub fn is_idle(&self) -> bool {
        match self.current_action {
            CurrentAction::Idle => true,
            CurrentAction::Expedition => self.expeditions.active.is_none(),
            _ => false,
        }
    }

    /// Gives you the same tasks, that the person in the tavern would present
    /// you with. When expeditions are available and they are not disabled by
    /// the `questing_preference`, they will be shown. Otherwise you will get
    /// quests
    #[must_use]
    pub fn available_tasks(&self) -> AvailableTasks {
        if self.questing_preference == ExpeditionSetting::PreferExpeditions
            && self.expeditions.is_event_ongoing()
        {
            AvailableTasks::Expeditions(&self.expeditions.available)
        } else {
            AvailableTasks::Quests(&self.quests)
        }
    }

    /// The expedition/questing setting can only be changed, before any
    /// alu/thirst for adventure is used that day
    #[must_use]
    pub fn can_change_questing_preference(&self) -> bool {
        self.thirst_for_adventure_sec == 6000 && self.beer_drunk == 0
    }

    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.current_action = CurrentAction::parse(
            data.cget(45, "action id")? & 0xFF,
            data.cget(46, "action sec")? & 0xFF,
            data.cstget(47, "current action time", server_time)?,
        );
        self.thirst_for_adventure_sec = data.csiget(456, "remaining ALU", 0)?;
        self.beer_drunk = data.csiget(457, "beer drunk count", 0)?;

        for (qidx, quest) in self.quests.iter_mut().enumerate() {
            let quest_start = data.skip(235 + qidx, "tavern quest")?;
            *quest = Quest::parse(quest_start, qidx, server_time)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// One of the three possible quests in the tavern
pub struct Quest {
    /// The length of this quest in sec (without item enchantment)
    pub base_length: u32,
    /// The silver reward for this quest (without item enchantment)
    pub base_silver: u32,
    /// The xp reward for this quest (without item enchantment)
    pub base_experience: u32,
    /// The item reward for this quest
    pub item: Option<Item>,
    /// The place where this quest takes place. Useful for the scrapbook
    pub location_id: Location,
    /// The enemy you fight in this quest. Useful for the scrapbook
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
    #[must_use]
    pub fn is_red(&self) -> bool {
        matches!(self.monster_id, 139 | 145 | 148 | 152 | 155 | 157)
    }

    pub(crate) fn parse(
        data: &[i64],
        quest_index: usize,
        server_time: ServerTime,
    ) -> Result<Quest, SFError> {
        let item_start = data.skip(9 + quest_index * 11, "quest item")?;
        Ok(Quest {
            base_length: data.csiget(6, "quest length", 100_000)?,
            base_silver: data.csiget(48, "quest silver", 0)?,
            base_experience: data.csiget(45, "quest xp", 0)?,
            item: Item::parse(item_start, server_time)?,
            location_id: data
                .cfpget(3, "quest location id", |a| a)?
                .unwrap_or_default(),
            monster_id: data.csimget(0, "quest monster id", 0, |a| -a)?,
        })
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The thing the player is currently doing
pub enum CurrentAction {
    #[default]
    /// The character is not doing anything and can basically do anything
    Idle,
    /// The character is working on guard duty right now. If `busy_until <
    /// Local::now()`, you can send a `WorkFinish` command
    CityGuard {
        /// The total amount of hours the character has decided to work
        hours: u8,
        /// The time at which the guard job will be over
        busy_until: DateTime<Local>,
    },
    /// The character is doing a quest right now. If `busy_until <
    /// Local::now()` you can send a `FinishQuest` command
    Quest {
        /// 0-2 index into tavern quest array
        quest_idx: u8,
        /// The time, at which the quest can be finished
        busy_until: DateTime<Local>,
    },
    /// The player is currently doing an expedition. This can be wrong, if the
    /// last expedition timer elapsed since sending the last request
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
/// The unlocked toilet, that you can throw items into
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
    pub(crate) fn update(&mut self, data: &[i64]) -> Result<(), SFError> {
        self.aura = data.csiget(491, "aura level", 0)?;
        self.mana_currently = data.csiget(492, "mana now", 0)?;
        self.mana_total = data.csiget(515, "mana missing", 1000)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The state of an ongoing expedition
pub struct Expedition {
    /// The items collected durign the expedition
    pub items: [Option<ExpeditionThing>; 4],

    /// The thing, that we are searching on this expedition
    pub target_thing: ExpeditionThing,
    /// The amount of the target item we have found
    pub target_current: u8,
    /// The amount of the target item that we are supposed to find
    pub target_amount: u8,

    /// The level we are currently clearing. Starts at 1
    pub current_floor: u8,
    ///  The heroism we have collected so far
    pub heroism: i32,

    pub(crate) adjusted_bounty_heroism: bool,

    pub(crate) floor_stage: i64,

    /// Choose one of these rewards
    pub(crate) rewards: Vec<Reward>,
    pub(crate) halftime_for_boss_id: i64,
    /// If we encountered a boss, this will contain information about it
    pub(crate) boss: ExpeditionBoss,
    /// The different encounters, that you can choose between. Should be 3
    pub(crate) encounters: Vec<ExpeditionEncounter>,
    pub(crate) busy_until: Option<DateTime<Local>>,
}

impl Expedition {
    pub(crate) fn adjust_bounty_heroism(&mut self) {
        if self.adjusted_bounty_heroism {
            return;
        }

        for ExpeditionEncounter { typ, heroism } in &mut self.encounters {
            if let Some(possible_bounty) = typ.required_bounty() {
                if self.items.iter().flatten().any(|a| a == &possible_bounty) {
                    *heroism += 10;
                }
            }
        }
        self.adjusted_bounty_heroism = true;
    }

    pub(crate) fn update_encounters(&mut self, data: &[i64]) {
        if data.len() % 2 != 0 {
            warn!("weird encounters: {data:?}");
        }
        let default_ecp = |ci| {
            warn!("Unknown encounter: {ci}");
            ExpeditionThing::Unknown
        };
        self.encounters = data
            .chunks_exact(2)
            .filter_map(|ci| {
                let raw = *ci.first()?;
                let typ = FromPrimitive::from_i64(raw)
                    .unwrap_or_else(|| default_ecp(raw));
                let heroism = soft_into(*ci.get(1)?, "e heroism", 0);
                Some(ExpeditionEncounter { typ, heroism })
            })
            .collect();
    }

    #[must_use]
    /// Returns the current stage the player is doing. This is dependent on
    /// time, because the timers are lazily evaluated. That means it might
    /// flip from Waiting->Encounters/Finished between calls
    pub fn current_stage(&self) -> ExpeditionStage {
        let cross_roads =
            || ExpeditionStage::Encounters(self.encounters.clone());

        match self.floor_stage {
            1 => cross_roads(),
            2 => ExpeditionStage::Boss(self.boss),
            3 => ExpeditionStage::Rewards(self.rewards.clone()),
            4 => match self.busy_until {
                Some(x) if x > Local::now() => ExpeditionStage::Waiting(x),
                _ if self.current_floor == 10 => ExpeditionStage::Finished,
                _ => cross_roads(),
            },
            _ => ExpeditionStage::Unknown,
        }
    }

    #[must_use]
    /// Checks, if the last timer of this expedition has run out
    pub fn is_finished(&self) -> bool {
        matches!(self.current_stage(), ExpeditionStage::Finished)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The current thing, that would be on screen, when using the web client
pub enum ExpeditionStage {
    /// Choose one of these rewards after winning against the boss
    Rewards(Vec<Reward>),
    /// If we encountered a boss, this will contain information about it
    Boss(ExpeditionBoss),
    /// The different encounters, that you can choose between. Should be <= 3
    Encounters(Vec<ExpeditionEncounter>),
    /// We have to wait until the specified time to continue in the expedition.
    /// When this is `< Local::now()`, you can send the update command to
    /// update the expedition stage, which will make `current_stage()`
    /// yield the new encounters
    Waiting(DateTime<Local>),
    /// The expedition has finished and you can choose another one
    Finished,
    /// Something strange happened and the current stage is not known. Feel
    /// free to try anything from logging in again to just continuing
    Unknown,
}

impl Default for ExpeditionStage {
    fn default() -> Self {
        ExpeditionStage::Encounters(Vec::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The monster you fight after 5 and 10 expedition encounters
pub struct ExpeditionBoss {
    /// The monster id of this boss
    pub id: i64,
    /// The amount of items this boss is supposed to drop
    pub items: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// One of up to three encounters you can find. In comparison to
/// `ExpeditionThing`, this also includes the expected heroism
pub struct ExpeditionEncounter {
    /// The type of thing you engage, or find on this path
    pub typ: ExpeditionThing,
    /// The heroism you get from picking this encounter. This contains the
    /// bonus from bounties, but no further boni from
    pub heroism: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The type of something you can encounter on the expedition. Can also be found
/// as the target, or in the items section
#[allow(missing_docs, clippy::doc_markdown)]
pub enum ExpeditionThing {
    #[default]
    Unknown = 0,

    Dummy1 = 1,
    Dummy2 = 2,
    Dumy3 = 3,

    ToiletPaper = 11,

    Bait = 21,
    /// New name: `DragonTaming`
    Dragon = 22,

    CampFire = 31,
    Phoenix = 32,
    /// New name: `ExtinguishedCampfire`
    BurntCampfire = 33,

    UnicornHorn = 41,
    Donkey = 42,
    Rainbow = 43,
    /// New name: `UnicornWhisperer`
    Unicorn = 44,

    CupCake = 51,
    /// New name: `SucklingPig`
    Cake = 61,

    SmallHurdle = 71,
    BigHurdle = 72,
    /// New name: `PodiumClimber`
    WinnersPodium = 73,

    Socks = 81,
    ClothPile = 82,
    /// New name: `RevealingLady`
    RevealingCouple = 83,

    SwordInStone = 91,
    BentSword = 92,
    BrokenSword = 93,

    Well = 101,
    Girl = 102,
    /// New name: `BewitchedStew`
    Balloons = 103,

    Prince = 111,
    /// New name: `ToxicFountainCure`
    RoyalFrog = 112,

    Hand = 121,
    Feet = 122,
    Body = 123,
    // New name: BuildAFriend
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

impl ExpeditionThing {
    #[must_use]
    #[allow(clippy::enum_glob_use)]
    /// Returns the associated bounty item required to get a +10 bonus for
    /// picking up this item
    pub fn required_bounty(&self) -> Option<ExpeditionThing> {
        use ExpeditionThing::*;
        Some(match self {
            Dummy1 | Dummy2 | Dumy3 => DummyBounty,
            ToiletPaper => ToiletPaperBounty,
            Dragon => DragonBounty,
            BurntCampfire => BurntCampfireBounty,
            Unicorn => UnicornBounty,
            WinnersPodium => WinnerPodiumBounty,
            RevealingCouple => RevealingCoupleBounty,
            BrokenSword => BrokenSwordBounty,
            Balloons => BaloonBounty,
            RoyalFrog => FrogBounty,
            Klaus => KlausBounty,
            _ => return None,
        })
    }

    #[must_use]
    #[allow(clippy::enum_glob_use)]
    /// If the thing is a bounty, this will contain all the things, that receive
    /// a bonus
    pub fn is_bounty_for(&self) -> Option<&'static [ExpeditionThing]> {
        use ExpeditionThing::*;
        Some(match self {
            DummyBounty => &[Dummy1, Dummy2, Dumy3],
            ToiletPaperBounty => &[ToiletPaper],
            DragonBounty => &[Dragon],
            BurntCampfireBounty => &[BurntCampfire],
            UnicornBounty => &[Unicorn],
            WinnerPodiumBounty => &[WinnersPodium],
            RevealingCoupleBounty => &[RevealingCouple],
            BrokenSwordBounty => &[BrokenSword],
            BaloonBounty => &[Balloons],
            FrogBounty => &[RoyalFrog],
            KlausBounty => &[Klaus],
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about a possible expedition, that you could start
pub struct AvailableExpedition {
    /// The target, that will be collected during the expedition
    pub target: ExpeditionThing,
    /// The amount of thirst for adventure, that selecting this expedition
    /// costs and also the expected time the two waiting periods take
    pub thirst_for_adventure_sec: u32,
    /// The first location, that you visit during the expedition. Might
    /// influence the haltime monsters type
    pub location_1: Location,
    /// The second location, that you visit during the expedition. Might
    /// influence the final monsters type
    pub location_2: Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The amount, that you either won or lost gambling. If the value is negative,
/// you lost
pub enum GambleResult {
    SilverChange(i64),
    MushroomChange(i32),
}
