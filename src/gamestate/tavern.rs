use chrono::{DateTime, Local};
use log::{error, warn};
use num_derive::FromPrimitive;

use super::{
    ExpeditionSetting, Result, ServerTime,
    items::{ITEM_PARSE_LEN, Item},
    rewards::{Reward, RewardType},
};
use crate::{
    command::{DiceReward, DiceType},
    error::SFError,
    response::Vals,
};

/// Anything related to things you can do in the tavern
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Decides if you can go on expeditions, or quests, when this event is
    /// currently ongoing
    pub questing_preference: ExpeditionSetting,
    /// The result of playing the shell game
    pub gamble_result: Option<GambleResult>,
    /// Total amount of beers you can drink today
    pub beer_max: u8,
}

/// Information about everything related to expeditions
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// Updates the event start and end time from an `expeditionevent`
    /// response: `[start, end, ...]`.
    pub(crate) fn update_event(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.start = data.server_time_opt(0, server_time);
        self.end = data.server_time_opt(1, server_time);
        if data.len() > 2 {
            data.ignore(2..4, "unknown expedition event fields");
        }
    }

    /// Updates available expeditions from repeating 8-value chunks.
    pub(crate) fn update_available(&mut self, data: &Vals<'_>) {
        if !data.len().is_multiple_of(8) {
            warn!(
                "Available expeditions have weird size: {} values",
                data.len()
            );
        }
        self.available = data
            .chunks(8)
            .map(|chunk| AvailableExpedition::parse(&chunk))
            .collect();
    }
}

/// Information about the current state of the dice game
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl DiceGame {
    /// Updates the next free game time and remaining games from a 2-value
    /// `dice` response: `[next_free, remaining]`.
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        self.next_free = data.server_time_opt(0, server_time);
        self.remaining = data.u8_or(1, 0);
    }

    /// Updates the dice currently lying on the table from a `dicestatus`
    /// response.
    pub(crate) fn update_status(&mut self, data: &Vals<'_>) {
        self.current_dice = data
            .iter()
            .filter_map(|x| {
                x.parse::<u8>()
                    .ok()
                    .and_then(num_traits::FromPrimitive::from_u8)
            })
            .collect();
    }

    /// Updates the dice game reward from a 2-value `dicereward` response:
    /// `[win_typ, amount]`.
    pub(crate) fn update_reward(&mut self, data: &Vals<'_>) -> Result<()> {
        let win_typ =
            data.enum_map(0, |a| a.saturating_sub(1)).ok_or_else(|| {
                SFError::ParsingErrorV2(format!(
                    "Invalid dice reward type: {}",
                    data.str_or(0, "")
                ))
            })?;
        let amount = data.u32(1)?;
        self.reward = Some(DiceReward { win_typ, amount });
        Ok(())
    }
}

/// The tasks you will presented with, when clicking the person in the tavern.
/// Make sure you are not currently busy and have enough ALU/thirst of adventure
/// before trying to start them
#[derive(Debug, Clone)]
#[allow(missing_docs)]
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
    pub fn available_tasks(&self) -> AvailableTasks<'_> {
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

    /// Updates the available quests from a 23-value `adventure` response.
    pub(crate) fn update_quests(&mut self, data: &Vals<'_>) -> Result<()> {
        data.ignore(0..2, "flavor / quest giver");
        for (chunk, quest) in data.window(2..23).chunks(7).zip(&mut self.quests)
        {
            quest.update(&chunk)?;
        }
        Ok(())
    }

    /// Updates quest offer items from a `questofferitems` response.
    pub(crate) fn update_quest_items(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        for (chunk, quest) in data.chunks(ITEM_PARSE_LEN).zip(&mut self.quests)
        {
            quest.item = Item::parse(&chunk, server_time).ok().flatten();
        }
    }
}

/// One of the three possible quests in the tavern
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// The background/location for a quest, or another activity
#[derive(Debug, Default, Clone, PartialEq, Eq, Copy, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
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

    /// Updates this quest from a 7-value chunk: `[flavor1, flavor2,
    /// monster_id, location_id, length, xp, silver]`.
    pub(crate) fn update(&mut self, data: &Vals<'_>) -> Result<()> {
        data.ignore(0..2, "flavor / quest text");
        let raw_monster = data.i64(2)?;
        self.monster_id = u16::try_from(-raw_monster).unwrap_or_else(|_| {
            warn!("Invalid quest monster id: {raw_monster}");
            0
        });
        self.location_id = data.enum_or(3, Location::default());
        self.base_length = data.u32(4)?;
        self.base_experience = data.u32(5)?;
        self.base_silver = data.u32(6)?;
        Ok(())
    }
}

/// The thing the player is currently doing
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CurrentAction {
    /// The character is not doing anything and can basically do anything
    #[default]
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
    Unknown(Option<DateTime<Local>>),
}

impl CurrentAction {
    pub(crate) fn parse(
        id: i64,
        sec: i64,
        busy_until: Option<DateTime<Local>>,
    ) -> Self {
        // XXX: Sometimes the game "forgets" when an action was supposed to end.
        // This only happens when the action is very old, so falling back to a
        // busy_until that is super old is fine here
        let busy_until = busy_until.unwrap_or_default();
        match id {
            0 => CurrentAction::Idle,
            1 => CurrentAction::CityGuard {
                hours: u8::try_from(sec).unwrap_or_else(|_| {
                    warn!("Invalid city guard hours: {sec}");
                    10
                }),
                busy_until,
            },
            2 => CurrentAction::Quest {
                quest_idx: u8::try_from(sec).unwrap_or_else(|_| {
                    warn!("Invalid quest index: {sec}");
                    0
                }),
                busy_until,
            },
            4 => CurrentAction::Expedition,
            _ => {
                error!("Unknown action id combination: {id}, {busy_until:?}");
                CurrentAction::Unknown(Some(busy_until))
            }
        }
    }
}

/// The unlocked toilet, that you can throw items into
#[derive(Debug, Clone, Default, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Toilet {
    /// The level the aura is at currently
    pub aura: u32,
    /// The amount of mana currently in the toilet
    pub mana_currently: u32,
    /// The total amount of mana you have to collect to flush the toilet
    pub mana_total: u32,
    /// The amount of sacrifices you have left today
    pub sacrifices_left: u32,
}

impl Toilet {
    /// Updates the toilet state from a 4-value response: `[aura,
    /// mana_current, timestamp, mana_total]`.
    pub(crate) fn update(&mut self, data: &Vals<'_>) -> Result<()> {
        self.aura = data.u32(0)?;
        self.mana_currently = data.u32(1)?;
        data.ignore(2, "unknown server timestamp (last flush / item time)");
        self.mana_total = data.u32(3)?;
        Ok(())
    }

    /// Updates the sacrifices left from a 3-value `toiletstate` response.
    pub(crate) fn update_sacrifices(&mut self, data: &Vals<'_>) {
        data.ignore(0..2, "unknown toilet state fields");
        self.sacrifices_left = data.u32_or(2, 0);
    }
}

/// The state of an ongoing expedition
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    pub(crate) floor_stage: i64,

    /// Choose one of these rewards
    pub(crate) rewards: Vec<Reward>,
    pub(crate) halftime_for_boss_id: i64,
    /// If we encountered a boss, this will contain information about it
    pub(crate) boss: ExpeditionBoss,
    /// The different encounters, that you can choose between. Should be 3
    pub(crate) encounters: Vec<ExpeditionEncounter>,
    pub(crate) busy_until: Option<DateTime<Local>>,
    pub(crate) busy_since: Option<DateTime<Local>>,
}

impl Expedition {
    /// Updates the available encounters at a crossroad from repeating
    /// `[thing, heroism]` pairs.
    pub(crate) fn update_encounters(&mut self, data: &Vals<'_>) {
        self.encounters = data
            .chunks(2)
            .map(|chunk| {
                let typ = chunk.enum_opt(0).unwrap_or_else(|| {
                    let raw = chunk.i64_or(0, 0);
                    warn!("Unknown encounter: {raw}");
                    ExpeditionThing::Unknown
                });
                let heroism = chunk.i32_or(1, 0);
                ExpeditionEncounter { typ, heroism }
            })
            .collect();
    }

    /// Updates the halftime boss ID and reward choices from an
    /// `expeditionhalftime` response.
    pub(crate) fn update_halftime(&mut self, data: &Vals<'_>) {
        let raw_id = data.i64_or(0, 0);
        self.halftime_for_boss_id = -raw_id;
        self.rewards = data
            .window(1..data.len())
            .chunks(2)
            .map(|chunk| Reward {
                typ: RewardType::parse(chunk.i64_or(0, 0)),
                amount: chunk.u64_or(1, 0),
            })
            .collect();
    }

    /// Updates the current expedition state from an `expeditionstate`
    /// response.
    pub(crate) fn update_state(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.current_floor = data.u8_or(0, 0);
        data.ignore(1, "unknown expedition field");
        self.floor_stage = data.i64(2)?;
        self.target_thing = data.enum_or(3, ExpeditionThing::Unknown);
        data.ignore(4..7, "unknown expedition fields");
        self.target_current = data.u8_or(7, 100);
        self.target_amount = data.u8_or(8, 100);

        for (chunk, item) in data.window(9..13).chunks(1).zip(&mut self.items) {
            let x = chunk.i64_or(0, 0);
            *item = match x {
                0 => None,
                _ => num_traits::FromPrimitive::from_i64(x).or_else(|| {
                    warn!("Unknown item: {x}");
                    Some(ExpeditionThing::Unknown)
                }),
            };
        }

        self.heroism = data.i32_or(13, 0);
        data.ignore(14, "unknown expedition field");
        self.busy_since = data.server_time_opt(15, server_time);
        self.busy_until = data.server_time_opt(16, server_time);
        Ok(())
    }

    /// Returns the current stage the player is doing. This is dependent on
    /// time, because the timers are lazily evaluated. That means it might
    /// flip from Waiting->Encounters/Finished between calls
    #[must_use]
    pub fn current_stage(&self) -> ExpeditionStage {
        let cross_roads =
            || ExpeditionStage::Encounters(self.encounters.clone());

        match self.floor_stage {
            1 => cross_roads(),
            2 => ExpeditionStage::Boss(self.boss),
            3 => ExpeditionStage::Rewards(self.rewards.clone()),
            4 => match self.busy_until {
                Some(x) if x > Local::now() => ExpeditionStage::Waiting {
                    busy_until: x,
                    busy_since: self.busy_since.unwrap_or_default(),
                },
                _ if self.current_floor == 10 => ExpeditionStage::Finished,
                _ => cross_roads(),
            },
            _ => ExpeditionStage::Unknown,
        }
    }

    /// Checks, if the last timer of this expedition has run out
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self.current_stage(), ExpeditionStage::Finished)
    }
}

/// The current thing, that would be on screen, when using the web client
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    Waiting {
        /// The time at which the next stage will be available
        busy_since: DateTime<Local>,
        /// The start time of this waiting period
        busy_until: DateTime<Local>,
    },
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

/// The monster you fight after 5 and 10 expedition encounters
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExpeditionBoss {
    /// The monster id of this boss
    pub id: i64,
    /// The amount of items this boss is supposed to drop
    pub items: u8,
}

impl ExpeditionBoss {
    /// Parses an expedition boss from a 2-value chunk: `[monster_id, items]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
        let raw_id = data.i64_or(0, 0);
        Self {
            id: -raw_id,
            items: data.u8_or(1, 3),
        }
    }
}

/// One of up to three encounters you can find. In comparison to
/// `ExpeditionThing`, this also includes the expected heroism
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExpeditionEncounter {
    /// The type of thing you engage, or find on this path
    pub typ: ExpeditionThing,
    /// The base heroism you get from picking this encounter. This does not
    /// contains the bonus from bounties
    pub heroism: i32,
}

/// The type of something you can encounter on the expedition. Can also be found
/// as the target, or in the items section
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs, clippy::doc_markdown)]
pub enum ExpeditionThing {
    #[default]
    Unknown = 0,

    Dummy1 = 1,
    Dummy2 = 2,
    Dummy3 = 3,

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

    FishingRod = 141,
    FishingBait = 142,
    Merman = 143,

    Mugs = 151,
    DraftBeer = 152,
    Barkeeper = 153,

    Chicken = 161,
    Tiger = 162,
    RidingStan = 163,

    // These may just be the event ones
    Cupid = 171,
    LovestruckShakes = 172,
    LoveBirds = 173,

    BeachTourist1 = 181,
    BeachTourist2 = 182,
    BeachTourist3 = 183,

    Leafbold1 = 191,
    Leafbold2 = 192,
    Leafbold3 = 193,

    SnowShakes1 = 201,
    SnowShakes2 = 202,
    SnowShakes3 = 203,

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
    // 1013 has to be a bounty for something, right?
    MermanBounty = 1014,
    BarkeeperBounty = 1015,
    StanBounty = 1016,
    LoveBirdBounty = 1017,
    BeachTouristBounty = 1018,
    LeafboldBounty = 1019,
    SnowShakesBounty = 1020,
}

impl ExpeditionThing {
    /// Returns the associated bounty item required to get a +10 bonus for
    /// picking up this item
    #[must_use]
    #[allow(clippy::enum_glob_use)]
    pub fn required_bounty(&self) -> Option<ExpeditionThing> {
        use ExpeditionThing::*;
        Some(match self {
            Dummy1 | Dummy2 | Dummy3 => DummyBounty,
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
            Merman => MermanBounty,
            RidingStan => StanBounty,
            Barkeeper => BarkeeperBounty,
            LoveBirds => LoveBirdBounty,
            BeachTourist3 => BeachTouristBounty,
            Leafbold3 => LeafboldBounty,
            SnowShakes3 => SnowShakesBounty,
            _ => return None,
        })
    }

    /// If the thing is a bounty, this will contain all the things, that receive
    /// a bonus
    #[must_use]
    #[allow(clippy::enum_glob_use)]
    pub fn is_bounty_for(&self) -> Option<&'static [ExpeditionThing]> {
        use ExpeditionThing::*;
        Some(match self {
            DummyBounty => &[Dummy1, Dummy2, Dummy3],
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
            MermanBounty => &[Merman],
            StanBounty => &[RidingStan],
            BarkeeperBounty => &[Barkeeper],
            LoveBirdBounty => &[LoveBirds],
            BeachTouristBounty => &[BeachTourist3],
            LeafboldBounty => &[Leafbold3],
            SnowShakesBounty => &[SnowShakes3],
            _ => return None,
        })
    }
}

/// Information about a possible expedition, that you could start
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Anything special about this expedition, that may be relevant for us to
    /// pick this
    pub special: Option<ExpeditionSpecial>,
}

impl AvailableExpedition {
    /// Parses an available expedition from an 8-value chunk.
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
        let target = data.enum_or(0, ExpeditionThing::Unknown);
        data.ignore(1..4, "unknown expedition parameters");
        let location_1 = data.enum_or(4, Location::default());
        let location_2 = data.enum_or(5, Location::default());
        let thirst_for_adventure_sec = data.u32_or(6, 600);
        let special = data.enum_opt(7);
        Self {
            target,
            thirst_for_adventure_sec,
            location_1,
            location_2,
            special,
        }
    }
}

/// A special reward, that will be encountered, or earned by going on this
/// expedition
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum ExpeditionSpecial {
    /// This expedition will have an egg to collect
    Egg = 1,
    /// This expedition will advance a daily task
    DailyTask,
}

/// The amount, that you either won or lost gambling. If the value is negative,
/// you lost
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum GambleResult {
    SilverChange(i64),
    MushroomChange(i32),
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn test_quest_update() {
        let mut quest = Quest::default();
        let raw = "366/3/-65/1/600/2239844/98169000";
        crate::parse_test!(update: quest(raw), {
            monster_id: 65,
            location_id: Location::SprawlingJungle,
            base_length: 600,
            base_experience: 2_239_844,
            base_silver: 98_169_000,
        });
        assert!(!quest.is_red());
    }

    #[test]
    fn test_red_quest() {
        let mut quest = Quest::default();
        let raw = "366/3/-139/1/600/2239844/98169000";
        crate::parse_test!(update: quest(raw), {
            monster_id: 139,
        });
        assert!(quest.is_red());
    }

    #[test]
    fn test_toilet_update() {
        let mut toilet = Toilet::default();
        let raw = "96/335/1780095716/280";
        crate::parse_test!(update: toilet(raw), {
            aura: 96,
            mana_currently: 335,
            mana_total: 280,
        });
    }

    #[test]
    fn test_expedition_encounters() {
        let mut exp = Expedition::default();
        let raw = "43/5/22/10/43/5";
        let vals = Vals::from(raw);
        exp.update_encounters(&vals);
        assert_eq!(vals.unaccessed_indices(), Vec::<usize>::new());
        assert_eq!(
            exp.encounters.as_slice(),
            [
                ExpeditionEncounter {
                    typ: ExpeditionThing::Rainbow,
                    heroism: 5,
                },
                ExpeditionEncounter {
                    typ: ExpeditionThing::Dragon,
                    heroism: 10,
                },
                ExpeditionEncounter {
                    typ: ExpeditionThing::Rainbow,
                    heroism: 5,
                },
            ]
        );
    }

    #[test]
    fn test_seasonal_bounties() {
        assert_eq!(
            ExpeditionThing::BeachTourist3.required_bounty(),
            Some(ExpeditionThing::BeachTouristBounty)
        );
        assert_eq!(
            ExpeditionThing::BeachTouristBounty.is_bounty_for(),
            Some(&[ExpeditionThing::BeachTourist3][..])
        );
        assert_eq!(
            ExpeditionThing::Leafbold3.required_bounty(),
            Some(ExpeditionThing::LeafboldBounty)
        );
        assert_eq!(
            ExpeditionThing::SnowShakes3.required_bounty(),
            Some(ExpeditionThing::SnowShakesBounty)
        );
    }

    #[test]
    fn test_tavern_update_quests() {
        let mut tavern = Tavern::default();
        let raw = "198/198/366/3/-65/1/600/2239844/98169000/367/4/-66/2/700/\
                   2239845/98169001/368/5/-67/3/800/2239846/98169002";
        let data = Vals::from(raw);
        tavern.update_quests(&data).unwrap();
        assert_eq!(tavern.quests[0].monster_id, 65);
        assert_eq!(tavern.quests[0].location_id, Location::SprawlingJungle);
        assert_eq!(tavern.quests[0].base_length, 600);
        assert_eq!(tavern.quests[1].monster_id, 66);
        assert_eq!(tavern.quests[1].location_id, Location::SkullIsland);
        assert_eq!(tavern.quests[2].monster_id, 67);
        assert_eq!(tavern.quests[2].location_id, Location::EvernightForest);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_tavern_update_quest_items() {
        let mut tavern = Tavern::default();
        let item_raw = "1/0/0/1010/0/149/265/5/4/0/108/72/0/83940/10/0/0/45/0";
        let empty_item = "0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0";
        let raw = format!("{item_raw}/{empty_item}/{empty_item}");
        let data = Vals::from(raw.as_str());
        tavern.update_quest_items(&data, ServerTime::default());
        assert!(tavern.quests[0].item.is_some());
        assert_eq!(tavern.quests[0].item.as_ref().unwrap().price, 83940);
        assert!(tavern.quests[1].item.is_none());
        assert!(tavern.quests[2].item.is_none());
    }

    #[test]
    fn test_toilet_update_sacrifices() {
        let mut toilet = Toilet::default();
        let raw = "0/0/3";
        let data = Vals::from(raw);
        toilet.update_sacrifices(&data);
        assert_eq!(toilet.sacrifices_left, 3);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_dice_game_updates() {
        let mut game = DiceGame::default();

        let dice_raw = "1600000000/5";
        let dice_data = Vals::from(dice_raw);
        game.update(&dice_data, ServerTime::default());
        assert!(game.next_free.is_some());
        assert_eq!(game.remaining, 5);
        assert!(
            dice_data.unaccessed_indices().is_empty(),
            "unread values in {dice_data}"
        );

        let status_raw = "1/2/3/4/5";
        let status_data = Vals::from(status_raw);
        game.update_status(&status_data);
        assert_eq!(
            game.current_dice.as_slice(),
            [
                DiceType::Silver,
                DiceType::Stone,
                DiceType::Wood,
                DiceType::Souls,
                DiceType::Arcane,
            ]
        );
        assert!(
            status_data.unaccessed_indices().is_empty(),
            "unread values in {status_data}"
        );

        let reward_raw = "2/50000";
        let reward_data = Vals::from(reward_raw);
        game.update_reward(&reward_data).unwrap();
        assert_eq!(
            game.reward,
            Some(DiceReward {
                win_typ: DiceType::Silver,
                amount: 50000,
            })
        );
        assert!(
            reward_data.unaccessed_indices().is_empty(),
            "unread values in {reward_data}"
        );
    }

    #[test]
    fn test_expeditions_event_updates() {
        let mut event = ExpeditionsEvent::default();

        let event_raw = "1600000000/1700000000";
        let event_data = Vals::from(event_raw);
        event.update_event(&event_data, ServerTime::default());
        assert!(event.start.is_some());
        assert!(event.end.is_some());
        assert!(
            event_data.unaccessed_indices().is_empty(),
            "unread values in {event_data}"
        );

        let event_raw_4 = "1600000000/1700000000/1/1792001382";
        let event_data_4 = Vals::from(event_raw_4);
        event.update_event(&event_data_4, ServerTime::default());
        assert!(
            event_data_4.unaccessed_indices().is_empty(),
            "unread values in {event_data_4}"
        );

        let avail_raw = "22/0/0/0/1/2/600/1";
        let avail_data = Vals::from(avail_raw);
        event.update_available(&avail_data);
        assert_eq!(event.available.len(), 1);
        assert_eq!(event.available[0].target, ExpeditionThing::Dragon);
        assert_eq!(event.available[0].location_1, Location::SprawlingJungle);
        assert_eq!(event.available[0].location_2, Location::SkullIsland);
        assert_eq!(event.available[0].thirst_for_adventure_sec, 600);
        assert_eq!(event.available[0].special, Some(ExpeditionSpecial::Egg));
        assert!(
            avail_data.unaccessed_indices().is_empty(),
            "unread values in {avail_data}"
        );
    }

    #[test]
    fn test_expedition_updates() {
        let mut exp = Expedition::default();

        let boss_raw = "45/3";
        let boss_data = Vals::from(boss_raw);
        exp.boss = ExpeditionBoss::parse(&boss_data);
        assert_eq!(exp.boss.id, -45);
        assert_eq!(exp.boss.items, 3);
        assert!(
            boss_data.unaccessed_indices().is_empty(),
            "unread values in {boss_data}"
        );

        let half_raw = "45/3/100/4/50000";
        let half_data = Vals::from(half_raw);
        exp.update_halftime(&half_data);
        assert_eq!(exp.halftime_for_boss_id, -45);
        assert_eq!(exp.rewards.len(), 2);
        assert_eq!(exp.rewards[0].typ, RewardType::Mushrooms);
        assert_eq!(exp.rewards[0].amount, 100);
        assert_eq!(exp.rewards[1].typ, RewardType::Silver);
        assert_eq!(exp.rewards[1].amount, 50000);
        assert!(
            half_data.unaccessed_indices().is_empty(),
            "unread values in {half_data}"
        );

        let state_raw =
            "5/0/2/22/0/0/0/10/20/22/0/0/0/150/0/1600000000/1600000600";
        let state_data = Vals::from(state_raw);
        exp.update_state(&state_data, ServerTime::default())
            .unwrap();
        assert_eq!(exp.current_floor, 5);
        assert_eq!(exp.floor_stage, 2);
        assert_eq!(exp.target_thing, ExpeditionThing::Dragon);
        assert_eq!(exp.target_current, 10);
        assert_eq!(exp.target_amount, 20);
        assert_eq!(exp.items[0], Some(ExpeditionThing::Dragon));
        assert_eq!(exp.items[1], None);
        assert_eq!(exp.heroism, 150);
        assert!(exp.busy_since.is_some());
        assert!(exp.busy_until.is_some());
        assert!(
            state_data.unaccessed_indices().is_empty(),
            "unread values in {state_data}"
        );
    }
}
