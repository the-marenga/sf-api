use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{
    error::SFError,
    gamestate::items::Item,
    misc::{CCGet, CFPGet, CGet},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DungeonEffect {
    /// The type this effect has
    pub typ: DungeonEffectType,
    /// The amount of rooms, or uses this effect is still active for
    pub remaining_uses: u32,
    /// The amount of rooms, or uses this effect will be active for after you
    /// get it (always >= remainign)
    pub max_uses: u32,
    /// The strength of this effect. I.e. 50 => chance to escape +50%
    pub strength: u32,
}

impl DungeonEffect {
    pub(crate) fn parse(
        typ: i64,
        remaining: i64,
        max_uses: i64,
        strength: i64,
    ) -> Option<Self> {
        if typ <= 0 {
            return None;
        }
        let typ: DungeonEffectType =
            FromPrimitive::from_i64(typ).unwrap_or_default();

        let remaining_uses: u32 = remaining.try_into().unwrap_or(0);
        let max_uses: u32 = max_uses.try_into().unwrap_or(0);
        let strength: u32 = strength.try_into().unwrap_or(0);

        Some(DungeonEffect {
            typ,
            remaining_uses,
            max_uses,
            strength,
        })
    }
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An effect, that you can get in the legendary dungeon
pub enum DungeonEffectType {
    /// More gold from chests
    Raider = 1,
    /// Kill monster in one hit
    OneHitWonder = 2,
    /// Better chance to escape
    EscapeAssistant = 3,
    /// Disarm the next X traps
    DisarmTraps = 4,
    /// Open the next X doors without keys
    LockPick = 5,
    /// 50% chance for 2 keys
    KeyMoment = 6,
    /// Heal X% of life immediately
    ElixirOfLife = 7,
    /// Heal X% per room
    RoadToRecovery = 8,

    /// Enemy deals more damage
    BrokenArmor = 101,
    /// Receive X% damage each room
    Poisoned = 102,
    /// Lower chance to escape
    Panderous = 103,
    /// Less gold from chests
    GoldRushHangover = 104,
    /// Double key price
    HardLock = 105,

    /// A curse, that has not yet been implemented
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DoorType {
    Monster1 = 1,
    Monster2 = 2,
    Monster3 = 3,
    Boss1 = 4,
    Boss2 = 5,

    Blocked = 1000,
    MysteryDoor = 1001,
    LockedDoor = 1002,
    OpenDoor = 1003,
    EpicDoor = 1004,
    DoubleLockedDoor = 1005,
    GoldenDoor = 1006,
    SacrificialDoor = 1007,
    CursedDoor = 1008,
    KeyMasterShop = 1009,
    BlessingDoor = 1010,
    Wheel = 1011,
    Wood = 1012,
    Stone = 1013,
    Souls = 1014,
    Metal = 1015,
    Arcane = 1016,
    SandWatches = 1017,
    TrialRoom1 = 1018,
    TrialRoom2 = 1019,
    TrialRoom3 = 1020,
    TrialRoom4 = 1021,
    TrialRoom5 = 1022,
    TrialRoomExit = 1023,

    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LegendaryDungeonStage {
    NotEntered = 0,

    DoorSelect = 1,

    RoomEntered = 10,
    RoomInteracted = 11,
    PickGem = 12,
    RoomFinished = 100,
    Healing = 101,

    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RoomEncounter {
    BronzeChest,
    SilverChest,
    EpicChest,
    Crate1,
    Crate2,
    Crate3,

    /// Pretty sure this is exclusively the dead (lootable) one
    WarriorSkeleton,
    /// The thing that transforms into an enemy
    MageSkeleton,
    Barrel,

    MimicChest,
    SacrificialChest,
    CurseChest,
    /// The price chest for completing the trial
    PrizeChest,
    SatedChest,

    Monster(u16),
    #[default]
    Unknown,
}

impl RoomEncounter {
    pub(crate) fn parse(val: i64) -> RoomEncounter {
        match val {
            0 => RoomEncounter::BronzeChest,
            1 => RoomEncounter::SilverChest,
            2 => RoomEncounter::EpicChest,
            100 => RoomEncounter::Crate1,
            101 => RoomEncounter::Crate2,
            102 => RoomEncounter::Crate3,
            300 => RoomEncounter::WarriorSkeleton,
            301 => RoomEncounter::MageSkeleton,
            400 => RoomEncounter::Barrel,
            500 => RoomEncounter::MimicChest,
            600 => RoomEncounter::SacrificialChest,
            601 => RoomEncounter::CurseChest,
            602 => RoomEncounter::PrizeChest,
            603 => RoomEncounter::SatedChest,
            x if x.is_negative() => {
                RoomEncounter::Monster(x.abs().try_into().unwrap_or_default())
            }
            _ => {
                log::warn!("Unknown room encounter: {val}");
                RoomEncounter::Unknown
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stats {
    pub items_found: u32,
    pub epics_found: u32,
    pub keys_found: u32,
    pub silver_found: u64,
    pub attempts: u32,
}

impl Stats {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        Ok(Stats {
            items_found: data.csiget(0, "ld item count", 0)?,
            epics_found: data.csiget(1, "ld item count", 0)?,
            keys_found: data.csiget(2, "ld item count", 0)?,
            silver_found: data.csiget(3, "ld item count", 0)?,
            attempts: data.csiget(4, "ld item count", 0)?,
        })
    }
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LegendaryDungeonEventTheme {
    DiabolicalCompanyParty = 1,
    LordOfTheThings = 2,
    /// .. and where to find them
    FantasticLegendaries = 3,
    ShadyBirthdayBash = 4,
    /// .. and Gingerbread Brawl
    MassiveWinterSpectacle = 5,
    AbyssOfMadness = 6,
    HuntForBlazingEasterEgg = 7,
    VileVacation = 8,

    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LegendaryDungeonEvent {
    pub theme: Option<LegendaryDungeonEventTheme>,
    /// The time after which we are allowed to interact with the legendaty
    /// dungeons
    pub start: Option<DateTime<Local>>,
    /// The time up until which we are allowed to start new runs
    pub end: Option<DateTime<Local>>,
    /// The time at which the dungeon is expected to completely close.
    /// Interacting with the dungeon (at all) is not possible after this.
    pub close: Option<DateTime<Local>>,

    pub(crate) active: Option<LegendaryDungeon>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendaryDungeonStatus<'a> {
    /// The legendary dungeon is not open, so you can not interact with it in
    /// any way, shape or form
    Unavailable,
    /// You have not yet entered the dungeon. Start the dungeon by sending a
    /// `LegendaryDungeonStart` command
    NotEntered,
    /// The event has ended, so you are not allowed to start another attempt,
    /// but you may look at the your stats
    Ended(&'a TotalStats),
    /// You are in the door select screen and must either pick the left, or
    /// right door. You do so with the `LegendaryDungeonPickDoor` command with
    /// the index of the door
    DoorSelect {
        dungeon: &'a LegendaryDungeon,
        doors: &'a [Door; 2],
    },
    /// You have defeated the dungeon boss and must pick one of the offered
    /// gems. You do so with the `IADungeonSelectSoulStone` command and the
    /// type of the gem you want
    PickGem {
        dungeon: &'a LegendaryDungeon,
        available_gems: &'a [GemOfFate],
    },
    /// We are currently healing. Wait until you can continue.
    // TODO: Do we continue with `LegendaryDungeonStart`?
    Healing {
        dungeon: &'a LegendaryDungeon,
        can_continue: bool,
    },
    Room {
        dungeon: &'a LegendaryDungeon,
        status: RoomStatus,
        encounter: RoomEncounter,
        typ: RoomType,
    },
    TakeItem {
        dungeon: &'a LegendaryDungeon,
        items: &'a [Item],
    },
    /// The dungeon is in a state, that has not been anticipated. Your best bet
    /// is to send a `LegendaryDungeonInteract` with a value of:
    /// [0,20,40,50,51,60,70] If you get this status, please report it
    Unknown,
}

impl LegendaryDungeonEvent {
    /// Checks if the event has started and not yet ended compared to the
    /// current time
    #[must_use]
    pub fn is_event_enterable(&self) -> bool {
        let now = Local::now();
        matches!((self.start, self.end), (Some(start), Some(end)) if end > now && start < now)
    }

    #[must_use]
    /// Returns the status of the legendary dungeon event. This is basically the
    /// screen, that you would be looking at in-game
    pub fn status(&self) -> LegendaryDungeonStatus<'_> {
        use LegendaryDungeonStage as Stage;
        use LegendaryDungeonStatus as Status;

        let now = Local::now();
        if self.start.is_none_or(|a| a > now) {
            return Status::Unavailable;
        }
        if self.close.is_none_or(|a| a < now) {
            return Status::Unavailable;
        }

        let Some(active) = &self.active else {
            return if self.end.is_some_and(|a| a > now) {
                Status::NotEntered
            } else {
                Status::Unavailable
            };
        };

        if !active.pending_items.is_empty() {
            return Status::TakeItem {
                dungeon: active,
                items: &active.pending_items,
            };
        }

        match active.stage {
            Stage::NotEntered => {
                // TODO: Do we need to provide the amount of runs already
                // done or smth. here?
                Status::NotEntered
            }
            Stage::DoorSelect => Status::DoorSelect {
                dungeon: active,
                doors: &active.doors,
            },
            Stage::PickGem => Status::PickGem {
                dungeon: active,
                available_gems: &active.available_gems,
            },
            Stage::Healing => Status::Healing {
                dungeon: active,
                can_continue: active.health_status == 2,
            },
            Stage::RoomEntered => Status::Room {
                dungeon: active,
                status: RoomStatus::Entered,
                encounter: active.encounter,
                typ: active.room_type,
            },
            // TODO: Does this have valid values for encounter & room type?
            Stage::RoomInteracted => Status::Room {
                dungeon: active,
                status: RoomStatus::Interacted,
                encounter: active.encounter,
                typ: active.room_type,
            },
            // TODO: Does this have valid values for encounter & room type?
            Stage::RoomFinished => Status::Room {
                dungeon: active,
                status: RoomStatus::Finished,
                encounter: active.encounter,
                typ: active.room_type,
            },
            Stage::Unknown => Status::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LegendaryDungeon {
    pub stats: Stats,
    pub total_stats: TotalStats,

    /// The hp you currently have
    pub current_hp: i64,
    // Any action, that reduces hp will immediately update `current_hp`. In
    // order for the game to properly transition from your old hp to the
    // current hp (visually), this here will contain your previous hp from
    // befor you took the action
    pub pre_battle_hp: i64,
    /// The hp you started the dungeon with
    pub max_hp: i64,

    pub blessings: [Option<DungeonEffect>; 3],
    pub curses: [Option<DungeonEffect>; 3],

    pub(crate) stage: LegendaryDungeonStage,

    pub current_floor: u32,
    pub max_floor: u32,
    /// The amount of keys you have available to unlock doors
    pub keys: u32,

    /// The amount of mushrooms you would have to spend to heal 20% of your
    /// health
    pub heal_quarter_cost: u32,
    /// The effects the merchant is currently trying to sell you
    pub merchant_offers: Vec<MerchantOffer>,
    /// The gems available to choose from after defeating the boss
    pub active_gems: Vec<GemOfFate>,

    /// The doors that you can pick between when in the `DoorSelect` stage
    pub(crate) doors: [Door; 2],
    pub(crate) room_type: RoomType,
    /// The thing you currently have in the room with you
    pub(crate) encounter: RoomEncounter,
    /// Items, that must be collected/chosen between before you can continue
    pub(crate) pending_items: Vec<Item>,
    /// The gems available to choose from after defeating the boss
    pub(crate) available_gems: Vec<GemOfFate>,

    // 2 = alive
    // 1 = ?
    // 0 = dead (healing)
    health_status: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RoomType {
    Generic = 1,
    BossRoom = 4,

    Encounter = 100,
    Empty = 200,
    /// The base version of the fountain of life heals a part of a character's
    /// life energy.
    FountainOfLife = 301,
    /// Immediately triggers the game to interact. No idea why, or what this is
    HoleInTheFloor = 302,
    /// The rocks must be cleared away to progress in the Legendary Dungeon. As
    /// a reward, stones are added to the fortress storage.
    PileOfRocks = 303,
    /// You have no choice; the lava must be crossed to progress in the
    /// Legendary Dungeon. Unfortunately, this also means a loss of a part of
    /// the character's life energy.
    TheFloorIsLava = 304,
    /// The dungeon narrator offers you a cup of tea. You can decide whether to
    /// drink the tea or not. If you drink the tea, it heals a part of the
    /// character's life energy, and you receive a blessing. If you do not
    /// drink the tea, you leave the room without any consequences.
    DungeonNarrator = 305,
    /// The room slowly fills with water. If you do not leave within ten
    /// seconds, you will drown and die.
    // 50 => continue
    // 51 => death
    FloodedRoom = 306,
    /// If you throw a gold coin into the wishing well, you will receive either
    /// an item or a blessing.
    WishingWell = 307,
    /// The gambler challenges you to rock-paper-scissors.
    /// If you avoid the challenge, you leave the room without any effects.
    /// If you dare to compete against the gambler, you choose either rock,
    /// paper, or scissors and wait to see what the gambler chooses.
    /// If you defeat the gambler, you receive a blessing.
    /// If the gambler wins, you receive a curse and lose a part of your life
    /// energy.
    /// If it's a draw, nothing happens.
    RockPaperScissors = 308,
    /// If you have wondered where the items go that you threw into the arcane
    /// toilet, here you find the answer.
    /// If you search through the broth, you receive an item.
    /// Too disgusting? The sewers can be left without any effects.
    Sewers = 309,
    /// The zombie with the lantern challenges you to a fight. You can accept
    /// the fight or try to flee.
    UndeadFiend = 310,
    /// An epic item is hidden in the locker. It's worth taking a look.
    LockerRoom = 311,
    /// The unlocked sarcophagus can be opened without a key. It contains gold.
    UnlockedSarcophagus = 312,
    /// If you defeat the Valaraukar, you lose a part of your hit points, but
    /// in return, you also receive the blessing "Road to Recovery".
    /// Cowardly heroes try to dodge the Valaraukar.
    Valaraukar = 313,
    /// The wood must be cleared away to progress in the Legendary Dungeon. As a
    /// reward, wood is added to the fortress storage.
    PileOfWood = 314,
    /// Buy blessings with keys
    KeyMasterShop = 315,
    /// The wheel of fortune can be spun. With some luck, you receive a reward
    /// (gold, blessing), but with bad luck, you can lose keys or receive a
    /// curse. You can leave the room without spinning the wheel, without
    /// having to worry about any consequences.
    WheelOfFortune = 316,
    /// Keys can be found in the spider web. However, it's also possible to get
    /// bitten by the spider and get poisoned (curse). If you don't want to
    /// take any risks, you can simply leave the room. There are three
    /// variations of the spider web:
    /// - Only the spider legs are visible: high chance for a key, low chance of
    ///   a spider bite.
    /// - The spider head is visible: equal chance for two keys or a spider
    ///   bite.
    /// - The entire spider is visible: low chance for 5 keys, high chance of a
    ///   spider bite.
    SpiderWeb = 317,

    // 318 unused?
    /// You can try to defeat the monster or dodge and take flight.
    BetaRoom = 319,
    /// If you defeat the flying tube, you are rewarded with ten lucky coins.
    FlyingTube = 320,
    /// If you click on the soul bath, you get credited souls in the underworld.
    SoulBath = 321,
    /// The arcane splinters can be collected and then used at the blacksmith.
    ArcaneSplintersCave = 322,
    /// The key to failure shop master offers curses for purchase. This might
    /// not sound very appealing at first. However, you receive keys if you take
    /// up the offer. If you're not interested, you can simply leave the
    /// store.
    KeyToFailureShop = 323,
    /// If you subject yourself to the torture rack, you receive a blessing but
    /// also lose a part of the character's life energy at the same time.
    /// The rainbow room can be left without any consequences.
    RainbowRoom = 324,
    /// You lose a part of your life energy in the fight. If the fight is won,
    /// you receive life energy as a reward, where the gain in life energy
    /// exceeds the loss from the fight. Lucky you!
    /// The room can be left without any consequences.
    PigRoom = 325,
    /// If you click on the offer board, you receive an item that can be epic.
    /// You can also simply leave the room without anything happening.
    AuctionHouse = 326,
    // 327 => normal
    // 328 => normal
    // 329 => normal, no enemy
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Door {
    pub typ: DoorType,
    pub trap: Option<DoorTrap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MerchantOffer {
    pub typ: DungeonEffectType,
    pub max_uses: u32,
    pub strength: u32,
    /// The amount of keys you pay, or get depending on curse/blessing
    pub keys: u32,
}

impl MerchantOffer {
    pub(crate) fn parse(data: &[i64]) -> Result<Option<Self>, SFError> {
        if data.iter().all(|a| *a == 0) {
            return Ok(None);
        }
        let typ: DungeonEffectType = data
            .cfpget(0, "ld merchant offer type", |a| a)?
            .unwrap_or_default();

        let s: u32 = data.csiget(1, "ld merchant effect", 0)?;
        let price = data.csiget(2, "ld merchant price", u32::MAX)?;
        Ok(Some(Self {
            typ,
            max_uses: s / 10_000,
            strength: s % 10_000,
            keys: price,
        }))
    }
}

impl LegendaryDungeon {
    pub(crate) fn update(&mut self, data: &[i64]) -> Result<(), SFError> {
        // [00] 718719374 <= Some sort of random id?
        self.health_status = data.cget(1, "ld unknown")?;

        self.current_hp = data.cget(2, "ld current hp")?;
        self.pre_battle_hp = data.cget(3, "ld pre hp")?;
        self.max_hp = data.cget(4, "ld max hp")?;

        for (pos, v) in self.blessings.iter_mut().enumerate() {
            let s = data.csiget(11 + pos, "ld blessing rem", 0)?;
            *v = DungeonEffect::parse(
                data.csiget(5 + pos, "ld blessing typ", 0)?,
                s / 10_000,
                data.csiget(42 + pos, "ld blessing max", 0)?,
                s % 10_000,
            );
        }
        for (pos, v) in self.curses.iter_mut().enumerate() {
            let s_pos = match pos {
                0 => 14,
                1 => 40,
                _ => 41,
            };
            let s = data.csiget(s_pos, "ld blessing rem", 0)?;

            *v = DungeonEffect::parse(
                data.csiget(8 + pos, "ld blessing typ", 0)?,
                s / 10_000,
                data.csiget(45 + pos, "ld blessing max", 0)?,
                s % 10_000,
            );
        }

        self.stage =
            data.cfpget(15, "dungeon stage", |a| a)?.unwrap_or_default();

        // 16 => gem count

        self.current_floor = data.csiget(17, "ld floor", 0)?;
        self.max_floor = data.csiget(18, "ld max floor", 0)?;

        if self.stage == LegendaryDungeonStage::DoorSelect {
            for (pos, v) in self.doors.iter_mut().enumerate() {
                v.typ = data
                    .cfpget(19 + pos, "ld door typ", |a| a)?
                    .unwrap_or_default();

                let raw_trap = data.cget(25 + pos, "ld door trap")?;
                v.trap = match raw_trap {
                    0 => None,
                    x => FromPrimitive::from_i64(x),
                }
            }
        } else {
            self.room_type =
                data.cfpget(19, "ld room type", |a| a)?.unwrap_or_default();
        }

        let raw_enc = data.csiget(22, "ld encounter", 999)?;
        self.encounter = RoomEncounter::parse(raw_enc);

        // 27..= 38 has moved

        self.keys = data.csiget(39, "ld keys", 0)?;

        let unknown_slots = [21, 23, 24, 40, 41, 48];
        #[allow(clippy::indexing_slicing)]
        for pos in unknown_slots {
            let val = data[pos];
            if val != 0 {
                log::info!("Found a non 0 val for [{pos}]: {val}");
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomStatus {
    Entered,
    Interacted,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DoorTrap {
    PoisonedDaggers = 1,
    SwingingAxe = 2,
    PaintBucket = 3,
    BearTrap = 4,
    Guillotine = 5,
    HammerAmbush = 6,
    TripWire = 7,
    TopSpikes = 8,
    Shark = 9,

    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TotalStats {
    pub legendaries_found: u32,
    pub attempts_best_run: u32,
    pub enemies_defeated: u32,
    pub epics_found: u32,
    pub gold_found: u64,
}

impl TotalStats {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        // Note: There is another value (5), but I can not figure out what it is
        Ok(TotalStats {
            legendaries_found: data.csiget(0, "ld total legendaries", 0)?,
            attempts_best_run: data.csiget(1, "ld best attempts", 0)?,
            enemies_defeated: data.csiget(2, "ld enemies defeated", 0)?,
            epics_found: data.csiget(3, "ld total epics", 0)?,
            gold_found: data.csiget(4, "ld total gold", 0)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemOfFate {
    pub typ: GemOfFateType,
    pub advantage: Option<GemOfFateAdvantage>,
    pub advantage_pwr: i64,
    pub disadvantage: Option<GemOfFateDisadvantage>,
    pub disadvantage_pwr: i64,
    pub disadvantage_effect: Option<GemOfFateDisadvantageEffect>,
}

impl GemOfFate {
    pub(crate) fn parse(data: &[i64]) -> Result<Option<GemOfFate>, SFError> {
        if data.iter().all(|a| *a == 0) {
            return Ok(None);
        }
        Ok(Some(Self {
            typ: data.cfpget(0, "ld gof typ", |a| a)?.unwrap_or_default(),
            advantage: data.cfpget(1, "ld gof adv", |a| a)?,
            advantage_pwr: data.cget(2, "ld gof dis val")?,
            disadvantage: data.cfpget(3, "ld gof dis", |a| a)?,
            disadvantage_pwr: data.cget(4, "ld gof dis val")?,
            disadvantage_effect: data.cfpget(5, "ld gof dis effect", |a| a)?,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GemOfFateType {
    EyeOfTheBull = 1,
    SoulOfTheRabbit = 2,
    BoulderOfGreed = 3,
    EmeraldOfTheExplorer = 4,
    PearlOfTheMasochist = 5,
    PendantOfTheKeyMaster = 6,

    // TODO:
    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GemOfFateAdvantage {
    // TODO:
    IncreasedChanceOfKeys = 1,
    IncreasedEscapeChance = 10,
    IncreasedDurationOfBlessings = 30,
    ReduceDamageFromMonsters = 40,
    IncreasedBlessingsInBarrels = 50,
    ReducedDamageFromSacDoors = 70,
    ReducedDamageFromTraps = 90,
    IncreasedBlessingsInBarrelsChestsCorpses = 110,
    MoreHealingFromBlessings = 130,

    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GemOfFateDisadvantage {
    // TODO:
    ReduceEscapeChance = 10,
    ReduceBlessingDuration = 30,
    IncreaseCurseDuration = 31,
    IncreaseStrongCurseChance = 32,

    IncreasedMonsterDamage = 40,

    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GemOfFateDisadvantageEffect {
    WeakerMonstersSpawn = 1,
    StrongerMonstersSpawn = 2,
    MoreTrapsSpawn = 3,
    // 4-5?
    SacChestsSpawnBehindClosedDoors = 6,

    MoreSacDoors = 8,
    FewerSacDoors = 9,
    CursedChestsSpawnBehindClosedDoors = 10,

    MoreCursedDoors = 12,
    FewerCursedDoors = 13,

    // ??
    ChanceOfEpicDoors = 17,
    ChanceOfUnlockedDoors = 18,
    ChanceOfDoubleLockedDoor = 19,

    MoreMysteriousRooms = 20,
    FewerMysteriousRooms = 21,
    AlwaysOneTrap = 22,
    AlwaysOneLock = 23,
    MonstersBehindDoors = 24,
    NoMoreEpicChests = 25,
    TrapsInflictCurse = 26,

    #[default]
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RPCChoice {
    Rock = 90,
    Paper = 91,
    Scissors = 92,
}
