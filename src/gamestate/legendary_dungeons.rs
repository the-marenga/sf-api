use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{error::SFError, misc::CCGet};

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
    pub (crate) fn parse(val: i64) -> RoomEncounter {
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
            items_found: data.csiget(0, "ldung item count", 0)?,
            epics_found: data.csiget(1, "ldung item count", 0)?,
            keys_found: data.csiget(2, "ldung item count", 0)?,
            silver_found: data.csiget(3, "ldung item count", 0)?,
            attempts: data.csiget(4, "ldung item count", 0)?,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LegendaryDungeons {
    pub stats: DungeonStats,

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

    pub stage: LegendaryDungeonStage,

    pub current_floor: u32,
    pub max_floor: u32,
    /// The doors that you can pick between when in the `DoorSelect` stage
    pub doors: [DoorType; 2],
    /// The amount of keys you have available to unlock doors
    pub keys: u32,

    pub encounter: RoomEncounter,
}
