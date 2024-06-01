#![allow(clippy::module_name_repetitions)]
use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use super::{ArrSkip, CCGet, CFPGet, CSTGet, EnumMapGet, SFError, ServerTime};

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The information about a characters underworld
pub struct Underworld {
    /// All the buildings, that the underworld can have. If they are not yet
    /// build, they are level 0
    pub buildings: EnumMap<UnderworldBuildingType, UnderworldBuilding>,
    /// Information about all the buildable units in the underworld
    pub units: EnumMap<UnderworldUnitType, UnderworldUnit>,
    /// All information about the production of resources in the underworld
    pub production: EnumMap<UnderWorldResourceType, UnderworldProduction>,
    /// The `last_collectable` value in `UnderWorldResource` is always out of
    /// date. Refer to the `Fortress.last_collectable_updated` for more
    /// information
    pub last_collectable_update: Option<DateTime<Local>>,

    // Both XP&silver are not really resources, so I just have this here,
    // instead of in a resouce info struct like in fortress
    /// The current souls in the underworld
    pub souls_current: u64,
    /// The maximum amount of souls, that you can store in the underworld.  If
    /// `current == limit`, you will not be able to collect resources from
    /// the building
    pub souls_limit: u64,

    /// The building, that is currently being upgraded
    pub upgrade_building: Option<UnderworldBuildingType>,
    /// The time at which the upgrade is finished
    pub upgrade_finish: Option<DateTime<Local>>,
    /// The time the building upgrade began
    pub upgrade_begin: Option<DateTime<Local>>,

    /// The combined level of all buildings in the underworld, which is
    /// equivalent to honor
    pub honor: u16,
    /// The amount of players, that have been lured into the underworld today
    pub lured_today: u16,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The price an upgrade, or building something in the underworld costs. These
/// are always for one upgrade/build, which is important for unit builds
pub struct UnderworldCost {
    /// The time it takes to complete one build/upgrade
    pub time: Duration,
    /// The price in silver this costs
    pub silver: u64,
    /// The price in sould this costs
    pub souls: u64,
}

impl UnderworldCost {
    pub(crate) fn parse(data: &[i64]) -> Result<UnderworldCost, SFError> {
        Ok(UnderworldCost {
            time: Duration::from_secs(data.csiget(0, "u time cost", 0)?),
            // Guessing here
            silver: data.csiget(1, "u silver cost", u64::MAX)?,
            souls: data.csiget(2, "u sould cost", u64::MAX)?,
        })
    }
}

impl Underworld {
    pub(crate) fn update_building_prices(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (pos, typ) in UnderworldBuildingType::iter().enumerate() {
            self.buildings.get_mut(typ).upgrade_cost = UnderworldCost::parse(
                data.skip(pos * 3, "underworld building prices")?,
            )?;
        }
        Ok(())
    }

    pub(crate) fn update_underworld_unit_prices(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (pos, typ) in UnderworldUnitType::iter().enumerate() {
            self.units.get_mut(typ).upgrade_next_lvl =
                data.csiget(pos * 3, "uunit next lvl", 0)?;
            self.units.get_mut(typ).upgrade_cost.silver =
                data.csiget(1 + pos * 3, "uunit upgrade gold", 0)?;
            self.units.get_mut(typ).upgrade_cost.souls =
                data.csiget(2 + pos * 3, "uunit upgrade gold", 0)?;
        }
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        for (pos, typ) in UnderworldBuildingType::iter().enumerate() {
            self.buildings.get_mut(typ).level =
                data.csiget(448 + pos, "building level", 0)?;
        }

        for (i, typ) in UnderworldUnitType::iter().enumerate() {
            let start = 146 + i * 148;
            self.units.get_mut(typ).upgraded_amount =
                data.csiget(start, "uunit upgrade level", 0)?;
            self.units.get_mut(typ).count =
                data.csiget(start + 1, "uunit count", 0)?;
            self.units.get_mut(typ).total_attributes =
                data.csiget(start + 2, "uunit atr bonus", 0)?;
            self.units.get_mut(typ).level =
                data.csiget(start + 3, "uunit level", 0)?;
        }

        #[allow(clippy::enum_glob_use)]
        {
            use UnderWorldResourceType::*;
            self.production.get_mut(Souls).last_collectable =
                data.csiget(459, "uu souls in building", 0)?;
            self.production.get_mut(Souls).limit =
                data.csiget(460, "uu sould max in building", 0)?;
            self.souls_limit = data.csiget(461, "uu souls max saved", 0)?;
            self.production.get_mut(Souls).per_hour =
                data.csiget(463, "uu souls per hour", 0)?;

            self.production.get_mut(Silver).last_collectable =
                data.csiget(464, "uu gold in building", 0)?;
            self.production.get_mut(Silver).limit =
                data.csiget(465, "uu max gold in building", 0)?;
            self.production.get_mut(Silver).per_hour =
                data.csiget(466, "uu gold ", 0)?;

            self.production.get_mut(ThirstForAdventure).last_collectable =
                data.csiget(473, "uu alu in building", 0)?;
            self.production.get_mut(ThirstForAdventure).limit =
                data.csiget(474, "uu max stored alu", 0)?;
            self.production.get_mut(ThirstForAdventure).per_hour =
                data.csiget(475, "uu alu per day", 0)?;
        }

        self.last_collectable_update =
            data.cstget(467, "uw resource time", server_time)?;
        self.upgrade_building =
            data.cfpget(468, "u building upgrade", |x| x - 1)?;
        self.upgrade_finish = data.cstget(469, "u expand end", server_time)?;
        self.upgrade_begin =
            data.cstget(470, "u upgrade begin", server_time)?;
        self.honor = data.csiget(471, "uu honor", 0)?;
        self.lured_today = data.csiget(472, "u battles today", 0)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, strum::EnumCount, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of a produceable resource in the underworld
pub enum UnderWorldResourceType {
    Souls = 0,
    Silver = 1,
    #[doc(alias = "ALU")]
    ThirstForAdventure = 2,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the producion of a resource in the fortress.  Note that
/// experience will not have some of these fields
pub struct UnderworldProduction {
    /// The amount the production building has already produced, that you can
    /// collect. Note that this value will be out of date by some amount of
    /// time. If you need the exact current amount collectable, look at
    /// `last_collectable_update`
    pub last_collectable: u64,
    /// The maximum amount of this resource, that this building can store. If
    /// `building_collectable == building_limit` the production stops
    pub limit: u64,
    /// The amount of this resource the coresponding production building
    /// produces per hour. The adventuromatics amount will be per day here
    pub per_hour: u64,
}

#[derive(
    Debug, Clone, Copy, FromPrimitive, strum::EnumCount, Enum, EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of building in the underworld
pub enum UnderworldBuildingType {
    HeartOfDarkness = 0,
    Gate = 1,
    GoldPit = 2,
    SoulExtractor = 3,
    GoblinPit = 4,
    TortureChamber = 5,
    GladiatorTrainer = 6,
    TrollBlock = 7,
    Adventuromatic = 8,
    Keeper = 9,
}

#[derive(Debug, Clone, Copy, strum::EnumCount, Enum, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of unit in the underworld
pub enum UnderworldUnitType {
    Goblin = 0,
    Troll = 1,
    Keeper = 2,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the current building state of a building
pub struct UnderworldBuilding {
    /// The current level of this building. If this is 0, it has not yet been
    /// built
    pub level: u8,
    /// The amount of resources it costs to upgrade to the next level
    pub upgrade_cost: UnderworldCost,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about a single type of unit
pub struct UnderworldUnit {
    /// The current (battle) level this unit has
    pub level: u16,
    /// The amount of units the character has of this type
    pub count: u16,
    /// The total amount of attributes this unit has
    pub total_attributes: u32,

    /// The amount of times this unit has been upgraded already
    pub upgraded_amount: u16,

    /// The price to pay for this unit to be upgraded once
    pub upgrade_cost: UnderworldCost,
    /// The level this unit will have, when the upgrade has been bought
    pub upgrade_next_lvl: u16,
}
