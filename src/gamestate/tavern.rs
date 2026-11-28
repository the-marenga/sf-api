use chrono::{DateTime, Local};
use log::{error, warn};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{
    ExpeditionSetting, SFError, ServerTime,
    items::{ITEM_PARSE_LEN, Item},
};
use crate::{
    command::{DiceReward, DiceType},
    gamestate::rewards::{Reward, RewardType},
    misc::{CFPGet, soft_into},
    response::Vals,
};

/// Parses a known enum from raw server data, warning about new variants.
fn new_enum<T: FromPrimitive>(raw: i64, name: &'static str) -> Option<T> {
    // Delegate to the legacy helper so its warning semantics stay identical.
    [raw].cfpget(0, name, |a| a).ok().flatten()
}

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

    pub(crate) fn update_event(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.start = data.server_time_opt(0, server_time);
        self.end = data.server_time_opt(1, server_time);
        data.ignore(2.., "extra expedition event values");
    }

    pub(crate) fn update_available(&mut self, data: &Vals<'_>) {
        if !data.len().is_multiple_of(8) {
            warn!(
                "Available expeditions have weird size: {data:?} {}",
                data.len()
            );
        }
        let complete = data.len() / 8;
        self.available = (0..complete)
            .map(|idx| {
                AvailableExpedition::parse(&data.window(idx * 8..(idx + 1) * 8))
            })
            .collect();
        data.ignore(complete * 8.., "partial available expedition tail");
    }

    pub(crate) fn update_halftime(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        let raw = data
            .i64(0)
            .inspect_err(|_| data.ignore_all("malformed halftime data"))?;
        let halftime_for_boss_id = raw.wrapping_neg();
        let reward_data = data.window(1..data.len());
        let complete = reward_data.len() / 2;
        let mut rewards = Vec::with_capacity(complete);
        for idx in 0..complete {
            let chunk = reward_data.window(idx * 2..(idx + 1) * 2);
            let typ = RewardType::parse(chunk.i64_or(0, 0));
            let raw_amount = chunk.i64_or(1, 0);
            let amount = u64::try_from(raw_amount).unwrap_or_else(|_| {
                warn!(
                    "Invalid value for reward amount in server response: \
                     {raw_amount}"
                );
                0
            });
            rewards.push(Reward { typ, amount });
        }
        reward_data.ignore(complete * 2.., "odd halftime reward tail");

        let exp = self.active.get_or_insert_default();
        exp.halftime_for_boss_id = halftime_for_boss_id;
        exp.rewards = rewards;
        Ok(())
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
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        self.next_free = data.server_time_opt(0, server_time);
        self.remaining = data.u8_or(1, 0);
        data.ignore(2.., "extra dice values");
    }

    pub(crate) fn update_status(&mut self, data: &Vals<'_>) {
        let mut valid = true;
        let mut dices = Vec::with_capacity(data.len());
        for raw in data.iter() {
            match raw.parse::<u8>().ok().and_then(DiceType::from_u8) {
                Some(typ) => dices.push(typ),
                None => valid = false,
            }
        }
        self.current_dice = if valid { dices } else { Vec::new() };
    }

    pub(crate) fn update_reward(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        let reward = (|| {
            let raw = data.u32(0)?;
            let win_typ = DiceType::from_u32(raw.saturating_sub(1))
                .ok_or_else(|| {
                    SFError::ParsingError("dice reward type", raw.to_string())
                })?;
            let amount = data.u32(1)?;
            data.ignore(2.., "extra dice reward values");
            Ok(DiceReward { win_typ, amount })
        })();
        match reward {
            Ok(reward) => {
                self.reward = Some(reward);
                Ok(())
            }
            Err(err) => {
                data.ignore_all("malformed dice reward");
                Err(err)
            }
        }
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

    pub(crate) fn update_quests(&mut self, data: &Vals<'_>) {
        data.ignore(0..2, "quest header values");
        let quest_data = data.window(2..data.len());
        let complete = quest_data.len() / 7;
        for (idx, quest) in self.quests.iter_mut().enumerate().take(complete) {
            quest.update(&quest_data.window(idx * 7..(idx + 1) * 7));
        }
        quest_data.ignore(complete * 7.., "partial quest tail");
        data.ignore(23.., "values beyond the third quest");
    }

    pub(crate) fn update_quest_items(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        let complete = data.len() / ITEM_PARSE_LEN;
        for (idx, quest) in
            self.quests.iter_mut().enumerate().take(complete.min(3))
        {
            let chunk =
                data.window(idx * ITEM_PARSE_LEN..(idx + 1) * ITEM_PARSE_LEN);
            quest.item = Item::parse(&chunk)
                .inspect_err(|_| data.ignore_all("bad quest item"))?;
        }
        let leftover = data.len() % ITEM_PARSE_LEN;
        if leftover > 0 {
            warn!(
                "Server response contains {leftover} leftover values when \
                 chunked by {ITEM_PARSE_LEN}"
            );
        }
        data.ignore(complete * ITEM_PARSE_LEN.., "partial quest item tail");
        data.ignore(3 * ITEM_PARSE_LEN.., "quest items beyond the third slot");
        Ok(())
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

    pub(crate) fn update(&mut self, data: &Vals<'_>) {
        // NOTE: [0], [1] are just flavor text
        data.ignore(0..2, "quest flavor text");
        self.monster_id = soft_into(-data.i64_or(2, 0), "quest monster id", 0);
        self.location_id = new_enum(data.i64_or(3, 0), "quest location id")
            .unwrap_or_default();
        self.base_length = data.u32_or(4, 100_000);
        self.base_experience = data.u32_or(5, 0);
        self.base_silver = data.u32_or(6, 0);
        data.ignore(7.., "extra quest values");
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
                hours: soft_into(sec, "city guard time", 10),
                busy_until,
            },
            2 => CurrentAction::Quest {
                quest_idx: soft_into(sec, "quest index", 0),
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
    // The amount of sacrifices you have left today
    pub sacrifices_left: u32,
}

impl Toilet {
    /// Updates the daily sacrifice count, without creating a locked toilet.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub(crate) fn update_sacrifices(
        data: &Vals<'_>,
        toilet: &mut Option<Toilet>,
    ) {
        if data.len() < 3 {
            data.ignore_all("short toilet state");
            return;
        }
        data.ignore(0..2, "unknown toilet state values");
        toilet.get_or_insert_with(Default::default).sacrifices_left =
            data.i64_or(2, 0) as u32;
        data.ignore(3.., "extra toilet state values");
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.aura = data.u32_or(0, 0);
        self.mana_currently = data.u32_or(1, 0);
        // TODO: What is this? Last time we flushed/got an item?
        let _unknown_time = data.server_time_opt(2, server_time);
        self.mana_total = data.u32_or(3, 1000);
        data.ignore(4.., "extra toilet values");
        Ok(())
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
    pub(crate) fn update_encounters(&mut self, data: &Vals<'_>) {
        if !data.len().is_multiple_of(2) {
            warn!("weird encounters: {data:?}");
        }
        let complete = data.len() / 2;
        self.encounters = (0..complete)
            .map(|idx| {
                let pair = data.window(idx * 2..(idx + 1) * 2);
                let raw = pair
                    .value(0)
                    .and_then(|raw| raw.parse::<i64>().ok())
                    .unwrap_or_default();
                let typ = FromPrimitive::from_i64(raw).unwrap_or_else(|| {
                    warn!("Unknown encounter: {raw}");
                    ExpeditionThing::Unknown
                });
                let heroism = pair
                    .value(1)
                    .and_then(|raw| raw.parse::<i64>().ok())
                    .map_or(0, |raw| soft_into(raw, "e heroism", 0));
                ExpeditionEncounter { typ, heroism }
            })
            .collect();
        data.ignore(complete * 2.., "odd encounter tail");
    }

    pub(crate) fn update_state(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        let floor_stage = data.i64_or(2, 0);
        let target_thing = new_enum(data.i64_or(3, 0), "expedition target")
            .unwrap_or_default();
        let target_current = data.u8_or(7, 100);
        let target_amount = data.u8_or(8, 100);
        let current_floor = data.u8_or(0, 0);
        let heroism = data.i32_or(13, 0);
        let busy_since = data.server_time_opt(15, server_time);
        let busy_until = data.server_time_opt(16, server_time);

        let mut items = self.items;
        for (raw, item) in data.slice(9..13).iter().zip(items.iter_mut()) {
            let x = raw.parse::<i64>().unwrap_or_default();
            *item = match ExpeditionThing::from_i64(x) {
                None if x != 0 => {
                    warn!("Unknown item: {x}");
                    Some(ExpeditionThing::Unknown)
                }
                other => other,
            };
        }
        data.ignore(1, "unknown expedition state value");
        data.ignore(4..7, "unknown expedition state values");
        data.ignore(14, "unknown expedition state value");
        data.ignore(17.., "extra expedition state values");

        self.floor_stage = floor_stage;
        self.target_thing = target_thing;
        self.target_current = target_current;
        self.target_amount = target_amount;
        self.current_floor = current_floor;
        self.heroism = heroism;
        self.busy_since = busy_since;
        self.busy_until = busy_until;
        self.items = items;
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
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self, SFError> {
        let raw = data
            .i64(0)
            .inspect_err(|_| data.ignore_all("malformed expedition boss"))?;
        let id = raw.wrapping_neg();
        let raw_items = data
            .value(1)
            .and_then(|raw| raw.parse::<i64>().ok())
            .unwrap_or_default();
        let items = soft_into(raw_items, "exp monster items", 3);
        data.ignore(2.., "extra expedition boss values");
        Ok(Self { id, items })
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
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
        let target =
            new_enum(data.i64_or(0, 0), "expedition typ").unwrap_or_default();
        data.ignore(1..4, "unused expedition fields");
        let location_1 =
            new_enum(data.i64_or(4, 0), "exp loc 1").unwrap_or_default();
        let location_2 =
            new_enum(data.i64_or(5, 0), "exp loc 2").unwrap_or_default();
        let thirst_for_adventure_sec = data.u32_or(6, 600);
        let special = new_enum(data.i64_or(7, 0), "exp special");
        data.ignore(8.., "extra expedition fields");
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

    fn item_chunk() -> Vec<i64> {
        let mut chunk = vec![0; ITEM_PARSE_LEN];
        chunk[0] = 15;
        chunk[3] = 23;
        chunk[13] = 36_384;
        chunk[16] = 31;
        chunk
    }

    fn join(values: &[i64]) -> String {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("/")
    }

    #[test]
    fn quest_update_uses_per_field_defaults() {
        let mut quest = Quest::default();
        let data = Vals::from("flavor/flavor/-5/1/600/100/200/extra");
        quest.update(&data);
        data.assert_all_read();
        assert_eq!(quest.monster_id, 5);
        assert_eq!(quest.location_id, Location::SprawlingJungle);
        assert_eq!(quest.base_length, 600);
        assert_eq!(quest.base_experience, 100);
        assert_eq!(quest.base_silver, 200);

        let mut quest = Quest::default();
        let data = Vals::from("flavor/flavor/-2");
        quest.update(&data);
        data.assert_all_read();
        assert_eq!(quest.monster_id, 2);
        assert_eq!(quest.location_id, Location::default());
        assert_eq!(quest.base_length, 100_000);
        assert_eq!(quest.base_experience, 0);
        assert_eq!(quest.base_silver, 0);
    }

    #[test]
    fn update_quests_parses_only_complete_chunks() {
        let mut tavern = Tavern::default();
        let values = [
            "h0", "h1", "f", "f", "-1", "1", "100", "10", "20", "f", "f", "-2",
            "2", "200", "20", "30", "f", "f", "-3", "3", "300", "30", "40",
            "extra",
        ];
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        tavern.update_quests(&data);
        data.assert_all_read();
        assert_eq!(tavern.quests[0].base_length, 100);
        assert_eq!(tavern.quests[1].base_length, 200);
        assert_eq!(tavern.quests[2].base_length, 300);

        // A partial fourth chunk is ignored, never parsed as a quest.
        let mut tavern = Tavern::default();
        let data = Vals::from(
            "h0/h1/f/f/-1/1/100/10/20/f/f/-2/2/200/20/30/partial1/partial2",
        );
        tavern.update_quests(&data);
        data.assert_all_read();
        assert_eq!(tavern.quests[0].base_length, 100);
        assert_eq!(tavern.quests[1].base_length, 200);
        assert_eq!(tavern.quests[2].base_length, 0);
    }

    #[test]
    fn quest_items_skip_partial_tails_and_keep_omitted_slots() {
        let full = join(&item_chunk());
        let original = Item::parse(&Vals::from(full.as_str())).unwrap();
        let mut tavern = Tavern::default();
        tavern.quests[1].item = original.clone();

        let raw = format!("{full}/tail1/tail2/tail3");
        let data = Vals::from(raw.as_str());
        tavern.update_quest_items(&data).unwrap();
        data.assert_all_read();
        assert!(tavern.quests[0].item.is_some());
        assert!(tavern.quests[1].item.is_some());
        assert!(tavern.quests[2].item.is_none());

        // Two complete chunks plus values after the third slot are ignored.
        let mut tavern = Tavern::default();
        let raw = format!("{full}/{full}/{full}/extra1/extra2");
        let data = Vals::from(raw.as_str());
        tavern.update_quest_items(&data).unwrap();
        data.assert_all_read();
        assert!(tavern.quests.iter().all(|quest| quest.item.is_some()));
    }

    #[test]
    fn toiletstate_short_does_not_create_a_toilet() {
        let mut toilet = Some(Toilet {
            sacrifices_left: 7,
            ..Default::default()
        });
        let data = Vals::from("0/1");
        Toilet::update_sacrifices(&data, &mut toilet);
        data.assert_all_read();
        assert_eq!(toilet.unwrap().sacrifices_left, 7);

        let mut toilet = None;
        let data = Vals::from("0/1");
        Toilet::update_sacrifices(&data, &mut toilet);
        data.assert_all_read();
        assert!(toilet.is_none());
    }

    #[test]
    fn toiletstate_full_wraps_negative_sacrifices() {
        let mut toilet = None;
        let data = Vals::from("0/1/-3/extra");
        Toilet::update_sacrifices(&data, &mut toilet);
        data.assert_all_read();
        assert_eq!(toilet.unwrap().sacrifices_left, u32::MAX - 2);
    }

    #[test]
    fn toilet_update_ignores_unknown_time_and_uses_field_defaults() {
        let mut toilet = Toilet::default();
        let data = Vals::from("5/100/1700000000/2000/extra");
        toilet.update(&data, ServerTime::default()).unwrap();
        data.assert_all_read();
        assert_eq!(toilet.aura, 5);
        assert_eq!(toilet.mana_currently, 100);
        assert_eq!(toilet.mana_total, 2000);

        let mut toilet = Toilet::default();
        let data = Vals::from("1/2/0");
        toilet.update(&data, ServerTime::default()).unwrap();
        data.assert_all_read();
        assert_eq!(toilet.mana_total, 1000);
    }

    #[test]
    fn dice_update_reads_next_free_and_remaining() {
        let mut dice = DiceGame::default();
        let data = Vals::from("1700000000/3/extra");
        dice.update(&data, ServerTime::default());
        data.assert_all_read();
        assert!(dice.next_free.is_some());
        assert_eq!(dice.remaining, 3);

        let mut dice = DiceGame::default();
        let data = Vals::from("0");
        dice.update(&data, ServerTime::default());
        data.assert_all_read();
        assert!(dice.next_free.is_none());
        assert_eq!(dice.remaining, 0);
    }

    #[test]
    fn dice_status_is_all_or_nothing() {
        let mut dice = DiceGame {
            current_dice: vec![DiceType::Silver],
            ..Default::default()
        };
        let data = Vals::from("1/2/0/6");
        dice.update_status(&data);
        data.assert_all_read();
        assert_eq!(
            dice.current_dice,
            [
                DiceType::Silver,
                DiceType::Stone,
                DiceType::ReRoll,
                DiceType::Hourglass,
            ]
        );

        let data = Vals::from("1/unknown/3");
        dice.update_status(&data);
        data.assert_all_read();
        assert_eq!(dice.current_dice, Vec::new());
    }

    #[test]
    fn dice_reward_handles_raw_zero_unknown_and_partial() {
        let mut dice = DiceGame::default();
        let data = Vals::from("0/10/extra");
        dice.update_reward(&data).unwrap();
        data.assert_all_read();
        assert_eq!(dice.reward.unwrap().win_typ, DiceType::ReRoll);

        let previous = dice.reward;
        let data = Vals::from("99/10");
        assert!(dice.update_reward(&data).is_err());
        data.assert_all_read();
        assert_eq!(
            dice.reward.map(|reward| reward.amount),
            previous.map(|reward| reward.amount)
        );

        let data = Vals::from("2");
        assert!(dice.update_reward(&data).is_err());
        data.assert_all_read();
        assert_eq!(
            dice.reward.map(|reward| reward.amount),
            previous.map(|reward| reward.amount)
        );

        let data = Vals::from("3/500");
        dice.update_reward(&data).unwrap();
        data.assert_all_read();
        assert_eq!(dice.reward.unwrap().win_typ, DiceType::Stone);
    }

    #[test]
    fn expedition_event_reads_times_and_ignores_tails() {
        let mut event = ExpeditionsEvent::default();
        let data = Vals::from("1700000000/1700000100/extra");
        event.update_event(&data, ServerTime::default());
        data.assert_all_read();
        assert!(event.start.is_some());
        assert!(event.end.is_some());

        let mut event = ExpeditionsEvent::default();
        let data = Vals::from("0/-1");
        event.update_event(&data, ServerTime::default());
        data.assert_all_read();
        assert!(event.start.is_none());
        assert!(event.end.is_none());
    }

    #[test]
    fn available_expeditions_handle_weird_sizes_and_partial_tails() {
        let mut event = ExpeditionsEvent::default();
        let data = Vals::from("11/x/x/x/2/3/600/1/tail1/tail2/tail3");
        event.update_available(&data);
        data.assert_all_read();
        assert_eq!(event.available.len(), 1);
        let expedition = &event.available[0];
        assert_eq!(expedition.target, ExpeditionThing::ToiletPaper);
        assert_eq!(expedition.location_1, Location::SkullIsland);
        assert_eq!(expedition.location_2, Location::EvernightForest);
        assert_eq!(expedition.thirst_for_adventure_sec, 600);
        assert_eq!(expedition.special, Some(ExpeditionSpecial::Egg));

        let mut event = ExpeditionsEvent::default();
        let data = Vals::from("11/x/x/x/2/3/600/1");
        event.update_available(&data);
        data.assert_all_read();
        assert_eq!(event.available.len(), 1);
    }

    #[test]
    fn boss_parse_negates_id_and_defaults_items() {
        let data = Vals::from("-5/4/extra");
        let boss = ExpeditionBoss::parse(&data).unwrap();
        data.assert_all_read();
        assert_eq!(boss.id, 5);
        assert_eq!(boss.items, 4);

        let data = Vals::from("-5");
        let boss = ExpeditionBoss::parse(&data).unwrap();
        data.assert_all_read();
        assert_eq!(boss.items, 0);

        let data = Vals::from("bad/2");
        assert!(ExpeditionBoss::parse(&data).is_err());
        data.assert_all_read();
    }

    #[test]
    fn halftime_negates_boss_id_and_ignores_odd_tail() {
        let mut event = ExpeditionsEvent::default();
        let data = Vals::from("-7/1/100/2/200/odd");
        event.update_halftime(&data).unwrap();
        data.assert_all_read();
        let active = event.active.as_ref().unwrap();
        assert_eq!(active.halftime_for_boss_id, 7);
        assert_eq!(active.rewards.len(), 2);
        assert_eq!(active.rewards[0].amount, 100);
        assert_eq!(active.rewards[1].amount, 200);

        let data = Vals::from("bad/1/100");
        assert!(event.update_halftime(&data).is_err());
        data.assert_all_read();
    }

    #[test]
    fn expedition_state_reads_offsets_and_remaps_items() {
        let mut expedition = Expedition::default();
        expedition.items[3] = Some(ExpeditionThing::SwordInStone);
        let data = Vals::from(
            "3/x/2/11/x/x/x/4/10/0/11/999/-1/500/x/1700000015/1700000016/tail",
        );
        expedition.update_state(&data, ServerTime::default());
        data.assert_all_read();
        assert_eq!(expedition.current_floor, 3);
        assert_eq!(expedition.floor_stage, 2);
        assert_eq!(expedition.target_thing, ExpeditionThing::ToiletPaper);
        assert_eq!(expedition.target_current, 4);
        assert_eq!(expedition.target_amount, 10);
        assert_eq!(expedition.heroism, 500);
        assert!(expedition.busy_since.is_some());
        assert!(expedition.busy_until.is_some());
        assert_eq!(expedition.items[0], Some(ExpeditionThing::Unknown));
        assert_eq!(expedition.items[1], Some(ExpeditionThing::ToiletPaper));
        assert_eq!(expedition.items[2], Some(ExpeditionThing::Unknown));
        assert_eq!(expedition.items[3], Some(ExpeditionThing::Unknown));

        // Missing item positions keep their previous value.
        let mut expedition = Expedition::default();
        expedition.items[3] = Some(ExpeditionThing::SwordInStone);
        let data = Vals::from("3/x/2/11/x/x/x/4/10/0/11");
        expedition.update_state(&data, ServerTime::default());
        data.assert_all_read();
        assert_eq!(expedition.items[0], Some(ExpeditionThing::Unknown));
        assert_eq!(expedition.items[3], Some(ExpeditionThing::SwordInStone));
    }

    #[test]
    fn crossroads_ignore_odd_tails() {
        let mut expedition = Expedition::default();
        let data = Vals::from("11/10/41/20/999/30/tail");
        expedition.update_encounters(&data);
        data.assert_all_read();
        assert_eq!(expedition.encounters.len(), 3);
        assert_eq!(expedition.encounters[0].typ, ExpeditionThing::ToiletPaper);
        assert_eq!(expedition.encounters[0].heroism, 10);
        assert_eq!(expedition.encounters[1].typ, ExpeditionThing::UnicornHorn);
        assert_eq!(expedition.encounters[2].typ, ExpeditionThing::Unknown);

        let data = Vals::from("0/5");
        expedition.update_encounters(&data);
        data.assert_all_read();
        assert_eq!(expedition.encounters.len(), 1);
        assert_eq!(expedition.encounters[0].typ, ExpeditionThing::Unknown);
    }

    #[test]
    fn warning_enum_helper_matches_legacy_fallback() {
        for raw in [0_i64, -1, 1, 999] {
            let legacy = [raw].cfpget(0, "expedition target", |a| a).unwrap();
            assert_eq!(
                new_enum::<ExpeditionThing>(raw, "expedition target"),
                legacy
            );
        }
    }
}
