use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{
    error::SFError,
    gamestate::items::Item,
    misc::{CCGet, CFPGet},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DungeonEffect<T: FromPrimitive> {
    /// The type this effect has
    pub typ: T,
    /// The amount of rooms, or uses this effect is still active for
    pub remaining_uses: u32,
    /// The amount of rooms, or uses this effect will be active for after you
    /// get it (always >= remainign)
    pub max_uses: u32,
    /// The strength of this effect. I.e. 50 => chance to escape +50%
    pub strength: u32,
}

impl<T: FromPrimitive> DungeonEffect<T> {
    pub(crate) fn parse(
        typ: i64,
        remaining: i64,
        max_uses: i64,
        strength: i64,
    ) -> Option<Self> {
        let typ: T = FromPrimitive::from_i64(typ)?;
        let remaining_uses: u32 = remaining.try_into().ok()?;
        let max_uses: u32 = max_uses.try_into().ok()?;
        let strength: u32 = strength.try_into().ok()?;

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
/// A curse, that you can get in the legendary dungeon
pub enum Curse {
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
pub enum Blessing {
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

    /// A blessing, that has not yet been implemented
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
    RoomFinished = 100,

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
    FallenWarrior,
    /// The thing that transforms into an enemy
    SleepingSkeleton,
    Barrel,
    // TODO: Check the real name
    WorkerChest,
    MimicChest,
    SacrificialChest,
    CurseChest,
    WeirdChest,
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
            300 => RoomEncounter::FallenWarrior,
            301 => RoomEncounter::SleepingSkeleton,
            400 => RoomEncounter::Barrel,
            500 => RoomEncounter::MimicChest,
            600 => RoomEncounter::SacrificialChest,
            601 => RoomEncounter::CurseChest,
            602 => RoomEncounter::WeirdChest,
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
pub struct DungeonStats {
    pub items_found: u32,
    pub epics_found: u32,
    pub keys_found: u32,
    pub silver_found: u64,
    pub attempts: u32,
}

impl DungeonStats {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        Ok(DungeonStats {
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
pub enum LegendaryDungeonsEventTheme {
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
pub struct LegendaryDungeonsEvent {
    pub theme: Option<LegendaryDungeonsEventTheme>,
    /// The time after which we are allowed to interact with the legendaty
    /// dungeons
    pub start_time: Option<DateTime<Local>>,
    /// The time up until which we are allowed to start new runs
    pub end_time: Option<DateTime<Local>>,
    /// The time at which the dungeon is expected to completely close.
    /// Interacting with the dungeon (at all) is not possible after this.
    pub close_time: Option<DateTime<Local>>,

    pub(crate) active: Option<LegendaryDungeons>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LegendaryDungeons {
    pub stats: DungeonStats,
    pub total_stats: DungeonTotalStats,

    /// The hp you currently have
    pub current_hp: u64,
    // Any action, that reduces hp will immediately update `current_hp`. In
    // order for the game to properly transition from your old hp to the
    // current hp (visually), this here will contain your previous hp from
    // befor you took the action
    pub pre_battle_hp: u64,
    /// The hp you started the dungeon with
    pub max_hp: u64,

    pub blessings: [Option<DungeonEffect<Blessing>>; 3],
    pub curses: [Option<DungeonEffect<Curse>>; 3],

    pub(crate) stage: LegendaryDungeonStage,

    pub current_floor: u32,
    pub max_floor: u32,
    /// The amount of keys you have available to unlock doors
    pub keys: u32,

    /// The doors that you can pick between when in the `DoorSelect` stage
    pub(crate) doors: [DoorType; 2],
    /// The thing you currently have in the room with you
    pub(crate) encounter: RoomEncounter,
    /// Items, that must be collected/chosen between before you can continue
    pub(crate) pending_items: Vec<Item>,
    /// The blessings you can get from the merchant, if you enter it
    pub(crate) merchant_blessings: Vec<MerchantBlessing>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MerchantBlessing {
    pub typ: Blessing,
    pub max_uses: u32,
    pub strength: u32,
    pub price: u32,
}

impl MerchantBlessing {
    pub(crate) fn parse(data: &[i64]) -> Result<Option<Self>, SFError> {
        if data.iter().all(|a| *a == 0) {
            return Ok(None);
        }
        let typ: Blessing = data
            .cfpget(0, "ld merchant blessing", |a| a)?
            .unwrap_or_default();

        let s: u32 = data.csiget(1, "ld merchant effect", 0)?;
        let price = data.csiget(2, "ld merchant price", u32::MAX)?;
        Ok(Some(Self {
            typ,
            max_uses: s / 10_000,
            strength: s % 10_000,
            price,
        }))
    }
}

impl LegendaryDungeons {
    pub(crate) fn update(&mut self, data: &[i64]) -> Result<(), SFError> {
        // [00] 718719374 <= Some sort of random id?
        // [01] 2 <= ?

        self.current_hp = data.csiget(2, "ld current hp", 0)?;
        self.pre_battle_hp = data.csiget(3, "ld pre hp", 0)?;
        self.max_hp = data.csiget(4, "ld max hp", 0)?;

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
            data.cfpuget(15, "dungeon stage", |a| a).unwrap_or_default();

        // [16] 0 // ?

        self.current_floor = data.csiget(17, "ld floor", 0)?;
        self.max_floor = data.csiget(18, "ld max floor", 0)?;

        for (pos, v) in self.doors.iter_mut().enumerate() {
            *v = data
                .cfpuget(19 + pos, "dungeon stage", |a| a)
                .unwrap_or_default();
        }

        // [21] 0 // ?

        let raw_enc = data.csiget(22, "ld max floor", 999)?;
        self.encounter = RoomEncounter::parse(raw_enc);

        // Most of this should be an unused (moved) item, but I
        // don't know where it starts and ends

        // [23] 0
        // [24] 0
        // [25] 0
        // [26] 0
        // [27] 0
        // [28] 0
        // [29] 0
        // [30] 0
        // [31] 0
        // [32] 0
        // [33] 0
        // [34] 0
        // [35] 0
        // [36] 0
        // [37] 0
        // [38] 0

        self.keys = data.csiget(39, "ld keys", 0)?;

        // [40] 0
        // [41] 0

        // [48] 0
        // [49] 0

        for (pos, n) in data.iter().enumerate() {
            log::info!("[{pos}] {n}");
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DungeonTotalStats {
    pub legendaries_found: u32,
    pub attempts_best_run: u32,
    pub enemies_defeated: u32,
    pub epics_found: u32,
    pub gold_found: u64,
}

impl DungeonTotalStats {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        // Note: There is another value (5), but I can not figure out what it is
        Ok(DungeonTotalStats {
            legendaries_found: data.csiget(0, "ld total legendaries", 0)?,
            attempts_best_run: data.csiget(1, "ld best attempts", 0)?,
            enemies_defeated: data.csiget(2, "ld enemies defeated", 0)?,
            epics_found: data.csiget(3, "ld total epics", 0)?,
            gold_found: data.csiget(4, "ld total gold", 0)?,
        })
    }
}
