use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

use super::{items::GemType, ServerTime};
use crate::{
    misc::{soft_into, warning_try_into},
    PlayerId,
};

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fortress {
    pub buildings: EnumMap<FortressBuildingType, FortressBuilding>,
    pub units: EnumMap<FortressUnitType, FortressUnit>,
    pub resources: EnumMap<FortressResourceType, FortessRessource>,

    /// The highest level buildings can be upgraded to
    pub building_max_lvl: u8,
    pub wall_combat_lvl: u16,
    /// This seems to be the last time resources were collected, but this also
    /// seems to get set by something else
    pub time_stamp: Option<DateTime<Local>>,

    /// The building, that is currently being upgraded
    pub building_upgrade: Option<FortressBuildingType>,
    /// The time at which the upgrade is finished
    pub building_upgrade_finish: Option<DateTime<Local>>,
    // The time the building upgrade began
    pub building_upgrade_began: Option<DateTime<Local>>,

    /// The level visible on the HOF screen for fortress. Should be all
    /// building levels summed up
    pub level: u16,

    pub honor: u32,
    pub rank: u32,

    pub gem_stone_target: Option<GemType>,
    pub gem_search_finish: Option<DateTime<Local>>,
    pub gem_search_began: Option<DateTime<Local>>,
    pub gem_search_cost: FortressCost,

    // This is some level of a building or smth.
    // TODO: Check what this is
    _some_level: u16,

    /// The next enemy you can choose to battle. This should always be Some(),
    /// but there is the edgecase of being the first player on a server to get
    /// a fortress, which I can not even test for, so I just assume this could
    /// be none then.
    pub attack_target: Option<PlayerId>,
    /// The time at which switching is free again
    pub attack_free_reroll: Option<DateTime<Local>>,
    /// The price in silver rerolling costs
    pub opponent_reroll_price: u64,
    /// The amount of stone the quarry produces on the next level per hour
    pub quarry_next_level_production: u64,
    /// The amount of wood the woodcutter produces on the next level per hour
    pub woodcutter_next_level_production: u64,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressCost {
    pub time: Duration,
    pub wood: u64,
    pub stone: u64,
    pub silver: u64,
}

impl FortressCost {
    pub(crate) fn parse(data: &[i64]) -> FortressCost {
        FortressCost {
            time: Duration::from_secs(data[0] as u64),
            // Guessing here
            silver: soft_into(data[1], "silver cost", u64::MAX),
            wood: soft_into(data[2], "wood cost", u64::MAX),
            stone: soft_into(data[3], "stone cost", u64::MAX),
        }
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortessRessource {
    pub limit: u64,
    pub current: u64,
    pub max_in_building: u64,
    pub max_save: u64,
    pub per_hour: u64,
    /// The limit after the next upgrade
    pub max_limit_next: u64,
}

#[derive(
    Debug,
    Clone,
    Copy,
    EnumCount,
    EnumIter,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Enum,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressResourceType {
    Wood = 0,
    Stone = 1,
    Experience = 2,
}

#[derive(
    Debug,
    Clone,
    Copy,
    EnumCount,
    FromPrimitive,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Enum,
    EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressBuildingType {
    Fortress = 0,
    LaborersQuarters = 1,
    WoodcuttersHut = 2,
    Quarry = 3,
    GemMine = 4,
    Academy = 5,
    ArcheryGuild = 6,
    Barracks = 7,
    MagesTower = 8,
    Treasury = 9,
    Smithy = 10,
    Wall = 11,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressUnit {
    pub level: u16,
    pub upgrade_began: Option<DateTime<Local>>,
    pub upgrade_finish: Option<DateTime<Local>>,
    pub max_count: u16,
    pub count: u16,
    pub in_que: u16,
    pub training_cost: FortressCost,
    pub next_lvl: u64,
    pub upgrade_stone_cost: u64,
    pub upgrade_wood_cost: u64,
}

#[derive(
    Debug,
    Clone,
    Copy,
    EnumCount,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Enum,
    EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressUnitType {
    Soldier = 0,
    Magician = 1,
    Archer = 2,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressBuilding {
    pub level: u16,
    pub upgrade_cost: FortressCost,
}

impl Fortress {
    pub fn get_building(&self, typ: FortressBuildingType) -> &FortressBuilding {
        &self.buildings[typ]
    }

    pub fn get_unit(&self, typ: FortressUnitType) -> &FortressUnit {
        &self.units[typ]
    }

    pub fn get_ressource(
        &self,
        typ: FortressResourceType,
    ) -> &FortessRessource {
        &self.resources[typ]
    }

    pub(crate) fn update(&mut self, data: &[i64], server_time: ServerTime) {
        // Buildings
        for (idx, typ) in FortressBuildingType::iter().enumerate() {
            self.buildings[typ].level =
                soft_into(data[524 + idx], "building lvl", 0);
        }
        self._some_level = soft_into(data[598], "group bonus level", 0);

        // Units
        for (idx, typ) in FortressUnitType::iter().enumerate() {
            self.units[typ].upgrade_began = server_time.convert_to_local(
                data[550 + idx],
                "fortress unit upgrade start",
            );
            self.units[typ].upgrade_finish = server_time.convert_to_local(
                data[553 + idx],
                "fortress unit upgrade finish",
            );
        }
        use FortressBuildingType::*;
        use FortressUnitType::*;
        self.units[Soldier].max_count = soft_into(
            self.buildings[Barracks].level * 3,
            "soldier max count",
            0,
        );
        self.units[Magician].max_count = soft_into(
            self.buildings[MagesTower].level,
            "magician max count",
            0,
        );
        self.units[Archer].max_count = soft_into(
            self.buildings[ArcheryGuild].level * 2,
            "archer max count",
            0,
        );

        self.units[Soldier].count =
            soft_into(data[547] & 0xFFFF, "soldier count", 0);
        self.units[Soldier].in_que =
            soft_into(data[548] >> 16, "soldier in que", 0);

        self.units[Magician].count =
            soft_into(data[547] >> 16, "magician count", 0);
        self.units[Magician].in_que =
            soft_into(data[549] & 0xFFFF, "magicians in que", 0);

        self.units[Archer].count =
            soft_into(data[548] & 0xFFFF, "archer count", 0);
        self.units[Archer].in_que =
            soft_into(data[549] >> 16, "archer in que", 0);

        // Items
        for (idx, typ) in FortressResourceType::iter().enumerate() {
            if idx != 2 {
                // self.resources[idx].saved =
                //     soft_into(data[544 + idx], "saved resource", 0);
                self.resources[typ].max_limit_next =
                    soft_into(data[584 + idx], "max saved next resource", 0);
            }
            self.resources[typ].current =
                soft_into(data[562 + idx], "resource in store", 0);
            self.resources[typ].max_in_building =
                soft_into(data[565 + idx], "resource max in store", 0);
            self.resources[typ].max_save =
                soft_into(data[568 + idx], "resource max save", 0);
            self.resources[typ].per_hour =
                soft_into(data[574 + idx], "resource per hour", 0);
        }
        self.time_stamp =
            server_time.convert_to_local(data[577], "resource time");
        self.building_upgrade = FromPrimitive::from_i64(data[571] - 1);
        self.building_upgrade_finish =
            server_time.convert_to_local(data[572], "fortress expand end");
        self.building_upgrade_began =
            server_time.convert_to_local(data[573], "fortress expand begin");

        self.level = soft_into(data[581], "fortress lvl", 0);
        self.honor = soft_into(data[582], "fortress honor", 0);
        self.rank = soft_into(data[583], "fortress rank", 0);
        self.gem_stone_target = GemType::parse(data[594]);
        self.gem_search_finish =
            server_time.convert_to_local(data[595], "gem search start");
        self.gem_search_began =
            server_time.convert_to_local(data[596], "gem search end");
        self.attack_target = warning_try_into(data[587], "fortress enemy");
        self.attack_free_reroll =
            server_time.convert_to_local(data[586], "fortress attack reroll");
    }

    pub(crate) fn update_unit_prices(&mut self, data: &[i64]) {
        for (i, typ) in FortressUnitType::iter().enumerate() {
            self.units[typ].training_cost = FortressCost::parse(&data[i * 4..]);
        }
    }

    pub(crate) fn update_unit_upgrade_info(&mut self, data: &[i64]) {
        for (i, typ) in FortressUnitType::iter().enumerate() {
            self.units[typ].next_lvl =
                soft_into(data[i * 3], "unit next lvl", 0);
            self.units[typ].upgrade_stone_cost =
                soft_into(data[1 + i * 3], "stone price next unit lvl", 0);
            self.units[typ].upgrade_wood_cost =
                soft_into(data[2 + i * 3], "wood price next unit lvl", 0);
        }
    }

    pub(crate) fn update_levels(&mut self, data: &[i64]) {
        use FortressUnitType::*;
        self.units[Soldier].level = soft_into(data[1], "soldier level", 0);
        self.units[Magician].level = soft_into(data[2], "magician level", 0);
        self.units[Archer].level = soft_into(data[3], "archer level", 0);
    }

    pub(crate) fn update_prices(&mut self, data: &[i64]) {
        for (i, typ) in FortressBuildingType::iter().enumerate() {
            self.buildings[typ].upgrade_cost =
                FortressCost::parse(&data[i * 4..]);
        }
        self.gem_search_cost = FortressCost::parse(&data[48..]);
    }
}
