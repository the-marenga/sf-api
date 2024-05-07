#![allow(clippy::module_name_repetitions)]
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
    /// All the buildings, that a fortress can have. If they are not yet built,
    /// they are level 0
    pub buildings: EnumMap<FortressBuildingType, FortressBuilding>,
    /// Information about all the buildable units in the fortress
    pub units: EnumMap<FortressUnitType, FortressUnit>,
    /// All information about resources in the fortress
    pub resources: EnumMap<FortressResourceType, FortressResource>,
    /// The `last_collectable` variable in `FortessProduction` is NOT
    /// calculated whenever you did the last request, instead the server
    /// calculates it at regular points in time and whenever you collect
    /// resources. That point in time is this variable here. That means if
    /// you want to know the exact current value, that you can collect, you
    /// need to calculate that yourself based on the current time, this
    /// time, the last collectable value and the per hour production of
    /// whatever you are looking at
    // TODO: Make such a function as a convenient helper
    pub last_collectable_updated: Option<DateTime<Local>>,

    /// The highest level buildings can be upgraded to
    pub building_max_lvl: u8,
    /// The level the fortress wall will have when defending against another
    /// player
    pub wall_combat_lvl: u16,

    /// Information about the building, that is currently being upgraded
    pub building_upgrade: FortressAction<FortressBuildingType>,

    /// The level visible on the HOF screen for fortress. Should be all
    /// building levels summed up
    pub level: u16,
    /// The honor you have in the fortress Hall of Fame
    pub honor: u32,
    /// The rank you have in the fortress Hall of Fame
    pub rank: u32,

    /// Information about searching for gems
    pub gem_search: FortressAction<GemType>,

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
pub struct FortressResource {
    /// The amount of this resource you have available to spend on upgrades and
    /// recruitment
    pub current: u64,
    /// The maximum amount of this resource, that you can store. If `current ==
    /// limit`, you will not be able to collect resources from buildings
    pub limit: u64,
    /// Information about the production building, that produces this resource.
    pub production: FortressProduction,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the producion of a resource in the fortress.  Note that
/// experience will not have some of these fields
pub struct FortressProduction {
    /// The amount the production building has already produced, that you can
    /// collect. Note that this value will be out of date by some amount of
    /// time. If you need the exact current amount collectable, look at
    /// `last_collectable_updated`
    pub last_collectable: u64,
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
#[allow(missing_docs)]
/// The type of resource, that the fortress available in the fortress
pub enum FortressResourceType {
    Wood = 0,
    Stone = 1,
    Experience = 2,
}

#[derive(
    Debug, Clone, Copy, EnumCount, FromPrimitive, PartialEq, Eq, Enum, EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of building, that can be build in the fortress
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
/// Information about a single type of unit
pub struct FortressUnit {
    /// The level this unit has
    pub level: u16,

    /// The amount of this unit, that you have available for combat
    pub count: u16,
    /// The amount of this unit, that are currently being trained/build
    pub in_training: u16,
    /// The maximum `count + in_training` you have of this unit
    pub limit: u16,
    /// All information about training up new units of this type
    pub training: FortressAction<()>,

    /// The price to pay in stone for the next upgrade
    pub upgrade_cost: FortressCost,
    /// The level this unit will be at, when you upgrade it
    pub upgrade_next_lvl: u64,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An action, that costs some amount of resources to do and will finish at a
/// certain point in time
pub struct FortressAction<T> {
    /// When this action was started. This can be months in the past, as this
    /// will often not be cleared by the server
    pub start: Option<DateTime<Local>>,
    /// Wheen this action will be finished
    pub finish: Option<DateTime<Local>>,
    /// The amount of resources it costs to do this
    pub cost: FortressCost,
    /// If it is not clear from the place where this is located, this will
    /// contain the specific type, that this action will be applied to/for
    pub target: Option<T>,
}

impl<T> Default for FortressAction<T> {
    fn default() -> Self {
        Self {
            start: None,
            finish: None,
            cost: FortressCost::default(),
            target: None,
        }
    }
}

#[derive(Debug, Clone, Copy, EnumCount, PartialEq, Eq, Enum, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type of a unit useable in the fortress
pub enum FortressUnitType {
    Soldier = 0,
    Magician = 1,
    Archer = 2,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Generic information about a building in the fortress. If you want
/// information about a production building, you should look at the resources
pub struct FortressBuilding {
    /// The current level of this building. If this is 0, it has not yet been
    /// build
    pub level: u16,
    /// The amount of resources it costs to upgrade to the next level
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
            let msg = "fortress unit training start";
            self.units.get_mut(typ).training.start =
                server_time.convert_to_local(data.cget(550 + idx, msg)?, msg);
            let msg = "fortress unit training finish";
            self.units.get_mut(typ).training.finish =
                server_time.convert_to_local(data.cget(553 + idx, msg)?, msg);
        }

        #[allow(clippy::enum_glob_use)]
        {
            use FortressBuildingType::*;
            use FortressUnitType::*;
            self.units.get_mut(Soldier).limit = soft_into(
                self.buildings.get_mut(Barracks).level * 3,
                "soldier max count",
                0,
            );
            self.units.get_mut(Magician).limit = soft_into(
                self.buildings.get_mut(MagesTower).level,
                "magician max count",
                0,
            );
            self.units.get_mut(Archer).limit = soft_into(
                self.buildings.get_mut(ArcheryGuild).level * 2,
                "archer max count",
                0,
            );

            self.units.get_mut(Soldier).count =
                data.csimget(547, "soldier count", 0, |x| x & 0xFFFF)?;
            self.units.get_mut(Soldier).in_training =
                data.csimget(548, "soldier in que", 0, |x| x >> 16)?;

            self.units.get_mut(Magician).count =
                data.csimget(547, "magician count", 0, |x| x >> 16)?;
            self.units.get_mut(Magician).in_training =
                data.csimget(549, "magicians in que", 0, |x| x & 0xFFFF)?;

            self.units.get_mut(Archer).count =
                data.csimget(548, "archer count", 0, |x| x & 0xFFFF)?;
            self.units.get_mut(Archer).in_training =
                data.csimget(549, "archer in que", 0, |x| x >> 16)?;
        }

        // Items
        for (idx, typ) in FortressResourceType::iter().enumerate() {
            if typ != FortressResourceType::Experience {
                self.resources.get_mut(typ).production.per_hour_next_lvl =
                    data.csiget(584 + idx, "max saved next resource", 0)?;
            }

            self.resources.get_mut(typ).limit =
                data.csiget(568 + idx, "resource max save", 0)?;
            self.resources.get_mut(typ).production.last_collectable =
                data.csiget(562 + idx, "resource in collectable", 0)?;
            self.resources.get_mut(typ).production.limit =
                data.csiget(565 + idx, "resource max in store", 0)?;
            self.resources.get_mut(typ).production.per_hour =
                data.csiget(574 + idx, "resource per hour", 0)?;
        }

        let get_local = |spot, name| {
            let val = data.cget(spot, name)?;
            Ok(server_time.convert_to_local(val, name))
        };

        self.last_collectable_updated =
            get_local(577, "fortress collection update")?;

        self.building_upgrade = FortressAction {
            start: get_local(573, "fortress upgrade begin")?,
            finish: get_local(572, "fortress upgrade end")?,
            cost: FortressCost::default(),
            target: FromPrimitive::from_i64(
                data.cget(571, "fortress building upgrade")? - 1,
            ),
        };

        self.level = data.csiget(581, "fortress lvl", 0)?;
        self.honor = data.csiget(582, "fortress honor", 0)?;
        self.rank = data.csiget(583, "fortress rank", 0)?;

        self.gem_search.start = get_local(595, "gem search start")?;
        self.gem_search.finish = get_local(596, "gem search end")?;
        self.gem_search.target = GemType::parse(data.cget(594, "gem target")?);

        self.attack_target = data.cwiget(587, "fortress enemy")?;
        self.attack_free_reroll = get_local(586, "fortress attack reroll")?;
        Ok(())
    }

    pub(crate) fn update_unit_prices(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (i, typ) in FortressUnitType::iter().enumerate() {
            self.units.get_mut(typ).training.cost =
                FortressCost::parse(data.skip(i * 4, "unit prices")?)?;
        }
        Ok(())
    }

    pub(crate) fn update_unit_upgrade_info(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (i, typ) in FortressUnitType::iter().enumerate() {
            self.units.get_mut(typ).upgrade_next_lvl =
                data.csiget(i * 3, "unit next lvl", 0)?;
            self.units.get_mut(typ).upgrade_cost.stone =
                data.csiget(1 + i * 3, "stone price next unit lvl", 0)?;
            self.units.get_mut(typ).upgrade_cost.wood =
                data.csiget(2 + i * 3, "wood price next unit lvl", 0)?;
        }
        Ok(())
    }

    pub(crate) fn update_levels(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        self.units.get_mut(FortressUnitType::Soldier).level =
            data.csiget(1, "soldier level", 0)?;
        self.units.get_mut(FortressUnitType::Magician).level =
            data.csiget(2, "magician level", 0)?;
        self.units.get_mut(FortressUnitType::Archer).level =
            data.csiget(3, "archer level", 0)?;
        Ok(())
    }

    pub(crate) fn update_prices(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (i, typ) in FortressBuildingType::iter().enumerate() {
            self.buildings.get_mut(typ).upgrade_cost =
                FortressCost::parse(data.skip(i * 4, "fortress unit prices")?)?;
        }
        self.gem_search.cost =
            FortressCost::parse(data.skip(48, "gem_search_cost")?)?;
        Ok(())
    }
}
