use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

use super::{items::GemType, ArrSkip, CCGet, SFError, ServerTime};
use crate::{
    gamestate::{CGet, EnumMapGet},
    misc::soft_into,
    PlayerId,
};

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The information about a characters fortress
pub struct Fortress {
    /// All the buildings, that a fortress can have. If they are not yet build,
    /// they are level 0
    pub buildings: EnumMap<FortressBuildingType, FortressBuilding>,
    /// Information about all the buildable units in the fortress
    pub units: EnumMap<FortressUnitType, FortressUnit>,
    /// All information about ressources in the fortress
    pub resources: EnumMap<FortressResourceType, FortessResource>,
    /// The `collectable` variable in `FortessProduction` is NOT calculated
    /// whenever you did the last request, instead the server calculates it at
    /// regular points in time and whenever you collect resources. That point
    /// in time is this variable here. That means if you want to know the exact
    /// current value, that you can collect, you need to calculate that
    /// yourself based on the current time, this time, the collectable
    /// value and the per hour production of whatever you are looking at
    // TODO: Make such a function as a convenient helper
    pub last_collectable_update: Option<DateTime<Local>>,

    /// The highest level buildings can be upgraded to
    pub building_max_lvl: u8,
    /// The level the fortress wall will have when defending against another
    /// player
    pub wall_combat_lvl: u16,

    /// The building, that is currently being upgraded
    pub building_upgrade: Option<FortressBuildingType>,
    /// The time at which the upgrade is finished
    pub building_upgrade_finish: Option<DateTime<Local>>,
    /// The time the building upgrade began
    pub building_upgrade_began: Option<DateTime<Local>>,

    /// The level visible on the HOF screen for fortress. Should be all
    /// building levels summed up
    pub level: u16,
    /// The honor you have in the fortress Hall of Fame
    pub honor: u32,
    /// The rank you have in the fortress Hall of Fame
    pub rank: u32,

    /// The type of gem, that is currently being searched
    pub gem_stone_target: Option<GemType>,
    /// The time at which the serach for the gem will be finished
    pub gem_search_finish: Option<DateTime<Local>>,
    /// The time at which the search was started
    pub gem_search_began: Option<DateTime<Local>>,
    /// The price to start the search for gems
    pub gem_search_cost: FortressCost,

    /// The level of the hall of knights
    pub hall_of_knights_level: u16,
    /// The price to upgrade the hall of knights. Note, that the duration here
    /// will be 0, as the game does not tell you how long it will take
    pub hall_of_knights_upgrade_price: FortressCost,

    /// The next enemy you can choose to battle. This should always be Some,
    /// but there is the edgecase of being the first player on a server to get
    /// a fortress, which I can not even test for, so I just assume this could
    /// be none then.
    pub attack_target: Option<PlayerId>,
    /// The time at which switching is free again
    pub attack_free_reroll: Option<DateTime<Local>>,
    /// The price in silver rerolling costs
    pub opponent_reroll_price: u64,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::module_name_repetitions)]
/// The price an upgrade, or building something in the fortress costs. These
/// are always for one upgrade/build, which is important for unit builds
pub struct FortressCost {
    /// The time it takes to complete one build/upgrade
    pub time: Duration,
    /// The price in wood this costs
    pub wood: u64,
    /// The price in stone this costs
    pub stone: u64,
    /// The price in silver this costs
    pub silver: u64,
}

impl FortressCost {
    pub(crate) fn parse(data: &[i64]) -> Result<FortressCost, SFError> {
        Ok(FortressCost {
            time: Duration::from_secs(data.csiget(0, "fortress time", 0)?),
            // Guessing here
            silver: data.csiget(1, "silver cost", u64::MAX)?,
            wood: data.csiget(2, "wood cost", u64::MAX)?,
            stone: data.csiget(3, "stone cost", u64::MAX)?,
        })
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about one of the three resources, that the fortress can produce.
pub struct FortessResource {
    /// The amount of this resource you have available to spend on upgrades and
    /// recruitment
    pub current: u64,
    /// The maximum amount of this resource, that you can store without. If
    /// `current == limit`, you will not be able to collect resources from
    /// buildings
    pub limit: u64,
    /// Information about the production building, that produces this resource.
    pub production: FortessProduction,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the producion of a resource in the fortress.  Note that
/// experience will not have some of these fields
pub struct FortessProduction {
    /// The amount the production building has already produced, that you can
    /// collect. Note that this value will for some reason be delayed. That
    /// means you this is NOT the value you see ticking up every second in
    /// the webui, that value is not in the API. This here seems to be the
    /// collectable amount at a recent amount of time, from which the ui
    /// then calculates the amount your building should have produces since
    /// that point in time and adds that to this value. The
    pub collectable: u64,
    /// The maximum amount of this resource, that this building can store. If
    /// `building_collectable == building_limit` the production stops
    pub limit: u64,
    /// The amount of this resource the coresponding production building
    /// produces per hour
    pub per_hour: u64,
    /// The amount of this resource the building produces on the next level per
    /// hour. If the resouce is Experience, this will be 0
    pub per_hour_next_lvl: u64,
}

#[derive(Debug, Clone, Copy, EnumCount, EnumIter, PartialEq, Eq, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressResourceType {
    Wood = 0,
    Stone = 1,
    Experience = 2,
}

#[derive(
    Debug, Clone, Copy, EnumCount, FromPrimitive, PartialEq, Eq, Enum, EnumIter,
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

#[derive(Debug, Clone, Copy, EnumCount, PartialEq, Eq, Enum, EnumIter)]
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
    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        // Buildings
        for (idx, typ) in FortressBuildingType::iter().enumerate() {
            self.buildings.get_mut(typ).level =
                data.csiget(524 + idx, "building lvl", 0)?;
        }
        self.hall_of_knights_level =
            data.csiget(598, "hall of knights level", 0)?;

        // Units
        for (idx, typ) in FortressUnitType::iter().enumerate() {
            let msg = "fortress unit upgrade start";
            self.units.get_mut(typ).upgrade_began =
                server_time.convert_to_local(data.cget(550 + idx, msg)?, msg);
            let msg = "fortress unit upgrade finish";
            self.units.get_mut(typ).upgrade_finish =
                server_time.convert_to_local(data.cget(553 + idx, msg)?, msg);
        }
        use FortressBuildingType::*;
        use FortressUnitType::*;
        self.units.get_mut(Soldier).max_count = soft_into(
            self.buildings.get_mut(Barracks).level * 3,
            "soldier max count",
            0,
        );
        self.units.get_mut(Magician).max_count = soft_into(
            self.buildings.get_mut(MagesTower).level,
            "magician max count",
            0,
        );
        self.units.get_mut(Archer).max_count = soft_into(
            self.buildings.get_mut(ArcheryGuild).level * 2,
            "archer max count",
            0,
        );

        self.units.get_mut(Soldier).count =
            data.csiget(547 & 0xFFFF, "soldier count", 0)?;
        self.units.get_mut(Soldier).in_que =
            data.csiget(548 >> 16, "soldier in que", 0)?;

        self.units.get_mut(Magician).count =
            data.csiget(547 >> 16, "magician count", 0)?;
        self.units.get_mut(Magician).in_que =
            data.csiget(549 & 0xFFFF, "magicians in que", 0)?;

        self.units.get_mut(Archer).count =
            data.csiget(548 & 0xFFFF, "archer count", 0)?;
        self.units.get_mut(Archer).in_que =
            data.csiget(549 >> 16, "archer in que", 0)?;

        // Items
        for (idx, typ) in FortressResourceType::iter().enumerate() {
            if typ != FortressResourceType::Experience {
                self.resources.get_mut(typ).production.per_hour_next_lvl =
                    data.csiget(584 + idx, "max saved next resource", 0)?;
            }

            self.resources.get_mut(typ).limit =
                data.csiget(568 + idx, "resource max save", 0)?;
            self.resources.get_mut(typ).production.collectable =
                data.csiget(562 + idx, "resource in collectable", 0)?;
            self.resources.get_mut(typ).production.limit =
                data.csiget(565 + idx, "resource max in store", 0)?;
            self.resources.get_mut(typ).production.per_hour =
                data.csiget(574 + idx, "resource per hour", 0)?;
        }

        self.last_collectable_update =
            server_time.convert_to_local(data[577], "resource time");

        self.building_upgrade = FromPrimitive::from_i64(data[571] - 1);
        self.building_upgrade_finish =
            server_time.convert_to_local(data[572], "fortress expand end");
        self.building_upgrade_began =
            server_time.convert_to_local(data[573], "fortress expand begin");

        self.level = data.csiget(581, "fortress lvl", 0)?;
        self.honor = data.csiget(582, "fortress honor", 0)?;
        self.rank = data.csiget(583, "fortress rank", 0)?;
        self.gem_stone_target = GemType::parse(data[594]);
        self.gem_search_finish =
            server_time.convert_to_local(data[595], "gem search start");
        self.gem_search_began =
            server_time.convert_to_local(data[596], "gem search end");
        self.attack_target = data.cwiget(587, "fortress enemy")?;
        self.attack_free_reroll =
            server_time.convert_to_local(data[586], "fortress attack reroll");
        Ok(())
    }

    pub(crate) fn update_unit_prices(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (i, typ) in FortressUnitType::iter().enumerate() {
            self.units.get_mut(typ).training_cost =
                FortressCost::parse(data.skip(i * 4, "unit prices")?)?;
        }
        Ok(())
    }

    pub(crate) fn update_unit_upgrade_info(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (i, typ) in FortressUnitType::iter().enumerate() {
            self.units.get_mut(typ).next_lvl =
                data.csiget(i * 3, "unit next lvl", 0)?;
            self.units.get_mut(typ).upgrade_stone_cost =
                data.csiget(1 + i * 3, "stone price next unit lvl", 0)?;
            self.units.get_mut(typ).upgrade_wood_cost =
                data.csiget(2 + i * 3, "wood price next unit lvl", 0)?;
        }
        Ok(())
    }

    pub(crate) fn update_levels(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        use FortressUnitType::*;
        self.units.get_mut(Soldier).level =
            data.csiget(1, "soldier level", 0)?;
        self.units.get_mut(Magician).level =
            data.csiget(2, "magician level", 0)?;
        self.units.get_mut(Archer).level = data.csiget(3, "archer level", 0)?;
        Ok(())
    }

    pub(crate) fn update_prices(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (i, typ) in FortressBuildingType::iter().enumerate() {
            self.buildings.get_mut(typ).upgrade_cost =
                FortressCost::parse(&data[i * 4..])?;
        }
        self.gem_search_cost = FortressCost::parse(&data[48..])?;

        Ok(())
    }
}
