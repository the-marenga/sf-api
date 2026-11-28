use chrono::{DateTime, Local};
use log::warn;
use num_derive::FromPrimitive;

use super::Location;
use crate::{
    error::Result,
    gamestate::{
        ServerTime,
        rewards::{Reward, RewardType},
    },
    response::Vals,
};

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
        data.ignore(17..21, "unknown expedition state fields");
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
    /// Parses an expedition boss from a `[monster_id, items]` list. The server
    /// may omit the item count, in which case it defaults to 0.
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
        let raw_id = data.i64_or(0, 0);
        let items = if data.len() > 1 { data.u8_or(1, 0) } else { 0 };
        Self { id: -raw_id, items }
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
pub enum ExpeditionSpecial {
    /// This expedition will have an egg to collect
    Egg = 1,
    /// This expedition will advance a daily task
    DailyTask,
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // The server may send only the monster id, omitting the item count.
        let boss_only_id = Vals::from("-162");
        exp.boss = ExpeditionBoss::parse(&boss_only_id);
        assert_eq!(exp.boss.id, 162);
        assert_eq!(exp.boss.items, 0);
        assert!(
            boss_only_id.unaccessed_indices().is_empty(),
            "unread values in {boss_only_id}"
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

        // Newer servers append extra fields after `busy_until`.
        let state_raw_extra = "5/0/2/22/0/0/0/10/20/22/0/0/0/150/0/1600000000/\
                               1600000600/5/-5/0/1";
        let state_data_extra = Vals::from(state_raw_extra);
        exp.update_state(&state_data_extra, ServerTime::default())
            .unwrap();
        assert_eq!(exp.current_floor, 5);
        assert!(exp.busy_since.is_some());
        assert!(
            state_data_extra.unaccessed_indices().is_empty(),
            "unread values in {state_data_extra}"
        );
    }
}
