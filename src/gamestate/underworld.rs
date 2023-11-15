use std::time::Duration;

use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::EnumCount;

use super::ServerTime;
use crate::misc::soft_into;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Underworld {
    pub buildings: [UnderworldBuilding; UnderworldBuildingType::COUNT],
    pub units: [UnderworldUnits; UnderworldUnitType::COUNT],
    pub resources: [UnderWorldResource; UnderWorldResourceType::COUNT],
    // I think this is the last time interacted, or something?
    _time_stamp: Option<DateTime<Local>>,
    pub upgrade_building: Option<UnderworldBuildingType>,
    pub upgrade_finish: Option<DateTime<Local>>,
    pub upgrade_begin: Option<DateTime<Local>>,
    pub total_level: u16,
    pub battles_today: u16,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderworldCost {
    pub time: std::time::Duration,
    pub silver: u64,
    pub souls: u64,
}

impl UnderworldCost {
    pub(crate) fn parse(data: &[i64]) -> UnderworldCost {
        UnderworldCost {
            time: Duration::from_secs(data[0] as u64),
            // Guessing here
            silver: soft_into(data[1], "silver cost", u64::MAX),
            souls: soft_into(data[2], "sould cost", u64::MAX),
        }
    }
}

impl Underworld {
    pub(crate) fn update_building_prices(&mut self, data: &[i64]) {
        for i in 0..UnderworldBuildingType::COUNT {
            self.buildings[i].upgrade_cost =
                UnderworldCost::parse(&data[i * 3..])
        }
    }

    pub(crate) fn update_underworld_unit_prices(&mut self, data: &[i64]) {
        for i in 0..UnderworldUnitType::COUNT {
            self.units[i].upgrade_price.next_level =
                soft_into(data[i * 3], "uunit next lvl", 0);
            self.units[i].upgrade_price.silver =
                soft_into(data[1 + i * 3], "uunit upgrade gold", 0);
            self.units[i].upgrade_price.souls =
                soft_into(data[2 + i * 3], "uunit upgrade gold", 0);
        }
    }

    pub(crate) fn update(&mut self, data: &[i64], server_time: ServerTime) {
        for i in 0..UnderworldBuildingType::COUNT {
            self.buildings[i].level =
                soft_into(data[448 + i], "building level", 0);
        }

        for i in 0..UnderworldUnitType::COUNT {
            let start = 146 + i * 148;
            self.units[i].upgraded_amount =
                soft_into(data[start], "uunit upgrade level", 0);
            self.units[i].count = soft_into(data[start + 1], "uunit count", 0);
            self.units[i].atr_bonus =
                soft_into(data[start + 2], "uunit atr bonus", 0);
            self.units[i].level = soft_into(data[start + 3], "uunit level", 0);
        }

        use UnderWorldResourceType::*;

        self.resources[Souls as usize].in_building =
            soft_into(data[459], "uu souls in building", 0);
        self.resources[Souls as usize].max_in_building =
            soft_into(data[460], "uu sould max in building", 0);
        self.resources[Souls as usize].limit =
            soft_into(data[461], "uu souls max saved", 0);
        self.resources[Souls as usize].max_limit_next =
            soft_into(data[462], "uu souls", 0);
        self.resources[Souls as usize].per_hour =
            soft_into(data[463], "uu souls per hour", 0);
        self.resources[Silver as usize].in_building =
            soft_into(data[464], "uu gold in building", 0);
        self.resources[Silver as usize].max_in_building =
            soft_into(data[465], "uu gold in building", 0);
        self.resources[Silver as usize].per_hour =
            soft_into(data[466], "uu gold ", 0);
        self.resources[Alu as usize].in_building =
            soft_into(data[473], "uu alu in building", 0);
        self.resources[Alu as usize].max_in_building =
            soft_into(data[474], "uu max stored alu", 0);
        self._time_stamp =
            server_time.convert_to_local(data[467], "uw resource time");
        self.upgrade_building = FromPrimitive::from_i64(data[468] - 1);
        self.upgrade_finish =
            server_time.convert_to_local(data[469], "u expand begin");
        self.upgrade_begin =
            server_time.convert_to_local(data[470], "u expand begin");
        self.total_level = soft_into(data[471], "uu max stored alu", 0);
        self.battles_today = soft_into(data[472], "u battles today", 0);
    }
}

#[derive(Debug, Clone, Copy, strum::EnumCount)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UnderWorldResourceType {
    Souls = 0,
    Silver = 1,
    Alu = 2,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderWorldResource {
    pub current: u32,
    pub limit: u32,
    pub in_building: u32,
    pub max_in_building: u32,
    pub max_limit_next: u32,
    pub per_hour: u32,
}

#[derive(Debug, Clone, Copy, FromPrimitive, strum::EnumCount)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Debug, Clone, Copy, strum::EnumCount)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UnderworldUnitType {
    Goblin = 0,
    Troll = 1,
    Keeper = 2,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderworldBuilding {
    // 0 => not build
    pub level: u8,
    pub upgrade_cost: UnderworldCost,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderworldUnits {
    pub upgraded_amount: u16,
    pub count: u16,
    pub atr_bonus: u32,
    pub level: u16,
    pub upgrade_price: UnderworldUnitUpradeInfo,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderworldUnitUpradeInfo {
    pub next_level: u8,
    pub silver: u32,
    pub souls: u32,
}
