use std::num::NonZeroU16;

use num_derive::FromPrimitive;

use crate::{error::SFError, misc::CCGet};

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A curse, that you can get in the legendary dungeon
pub enum CurseType {
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
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash)]
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
    Unknown = -1,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash)]
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

    Unknown = -1,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DungeonStage {
    NotEntered = 0,
    PickDoor = 1,
    RoomEntered = 10,
    RoomInteracted = 11,
    RoomFinished = 100,

    Unknown = -1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Unknown,
}

impl RoomEncounter {
    fn parse(val: i64) -> RoomEncounter {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
