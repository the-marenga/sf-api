#![allow(clippy::module_name_repetitions)]
use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive as _;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

use super::{
    ArrSkip, CCGet, CSTGet, SFError, ServerTime, items::GemType,
};
use crate::{
    PlayerId,
    gamestate::EnumMapGet,
    response::ResponseVals,
};

/// The information about a characters fortress
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// The upgrades count visible on the HOF screen for fortress. Should be
    /// all building levels summed up
    pub upgrades: u16,
    /// The honor you have in the fortress Hall of Fame
    pub honor: u32,
    /// The rank you have in the fortress Hall of Fame if you have any
    pub rank: Option<u32>,

    /// Information about searching for gems
    pub gem_search: FortressAction<GemType>,

    /// The level of the hall of knights
    pub hall_of_knights_level: u16,
    /// The price to upgrade the hall of knights. Note, that the duration here
    /// will be 0, as the game does not tell you how long it will take
    pub hall_of_knights_upgrade_price: FortressCost,

    /// The next enemy you can choose to battle. This should always be Some,
    /// but there is the edge case of being the first player on a server to get
    /// a fortress, which I can not even test for, so I just assume this could
    /// be none then.
    pub attack_target: Option<PlayerId>,
    /// The time at which switching is free again
    pub attack_free_reroll: Option<DateTime<Local>>,
    /// The price in silver re-rolling costs
    pub opponent_reroll_price: u64,
}

/// The price an upgrade, or building something in the fortress costs. These
/// are always for one upgrade/build, which is important for unit builds
#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Parses a fortress cost `[time_seconds, silver, wood, stone]`.
    pub(crate) fn parse(
        data: &ResponseVals<4>,
    ) -> Result<FortressCost, SFError> {
        Ok(FortressCost {
            time: Duration::from_secs(data.u64(0)?),
            silver: data.u64(1)?,
            wood: data.u64(2)?,
            stone: data.u64(3)?,
        })
    }

    #[deprecated(note = "use FortressCost::parse with ResponseVals instead")]
    pub(crate) fn parse_slice(data: &[i64]) -> Result<FortressCost, SFError> {
        Ok(FortressCost {
            time: Duration::from_secs(data.csiget(0, "fortress time", 0)?),
            // Guessing here
            silver: data.csiget(1, "silver cost", u64::MAX)?,
            wood: data.csiget(2, "wood cost", u64::MAX)?,
            stone: data.csiget(3, "stone cost", u64::MAX)?,
        })
    }
}

/// Information about one of the three resources, that the fortress can produce.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressResource {
    /// The amount of this resource you have available to spend on upgrades and
    /// recruitment
    pub current: u64,
    /// The maximum amount of this resource, that you can store. If `current ==
    /// limit`, you will not be able to collect resources from buildings
    pub limit: u64,
    /// The maximum amount of this resource, that you can store in the fortress
    /// after you upgrade the Heart of Darkness
    pub limit_next_level: u64,
    /// Information about the production building, that produces this resource.
    pub production: FortressProduction,
    /// The secret storage, available in the treasury
    pub secret_storage: FortressSecretStorage,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressSecretStorage {
    pub amount: u64,
    pub limit: u64,
}

/// Information about the production of a resource in the fortress.  Note that
/// experience will not have some of these fields
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressProduction {
    /// The amount the production building has already produced, that you can
    /// collect. Note that this value will be out of date by some amount of
    /// time. If you need the exact current amount collectable, look at
    /// `last_collectable_updated`
    pub last_collectable: u64,
    /// The maximum amount of this resource, that this building can store. If
    /// `building_collectable == building_limit` the production stops
    pub limit: u64,
    /// The amount of this resource the corresponding production building
    /// produces per hour
    pub per_hour: u64,
    /// The amount of this resource the building produces on the next level per
    /// hour. If the resource is Experience, this will be 0
    pub per_hour_next_lvl: u64,
}

/// The type of resource, that the fortress available in the fortress
#[derive(Debug, Clone, Copy, EnumCount, EnumIter, PartialEq, Eq, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum FortressResourceType {
    Wood = 0,
    Stone = 1,
    Experience = 2,
}

/// The type of building, that can be build in the fortress
#[derive(
    Debug, Clone, Copy, EnumCount, FromPrimitive, PartialEq, Eq, Enum, EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
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

impl FortressBuildingType {
    /// Minimal fortress level which is required to be allowed to build this
    /// building
    #[must_use]
    pub fn required_min_fortress_level(&self) -> u16 {
        match self {
            FortressBuildingType::Fortress => 0,
            FortressBuildingType::LaborersQuarters
            | FortressBuildingType::Quarry
            | FortressBuildingType::Smithy
            | FortressBuildingType::WoodcuttersHut => 1,
            FortressBuildingType::Treasury => 2,
            FortressBuildingType::GemMine => 3,
            FortressBuildingType::Barracks | FortressBuildingType::Wall => 4,
            FortressBuildingType::ArcheryGuild => 5,
            FortressBuildingType::Academy => 6,
            FortressBuildingType::MagesTower => 7,
        }
    }

    /// Get the unit type associated with this building type
    #[must_use]
    pub fn unit_produced(self) -> Option<FortressUnitType> {
        match self {
            FortressBuildingType::Barracks => Some(FortressUnitType::Soldier),
            FortressBuildingType::MagesTower => {
                Some(FortressUnitType::Magician)
            }
            FortressBuildingType::ArcheryGuild => {
                Some(FortressUnitType::Archer)
            }
            _ => None,
        }
    }
}

/// Information about a single type of unit
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// An action, that costs some amount of resources to do and will finish at a
/// certain point in time
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressAction<T> {
    /// When this action was started. This can be months in the past, as this
    /// will often not be cleared by the server
    pub start: Option<DateTime<Local>>,
    /// When this action will be finished
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

/// The type of a unit usable in the fortress
#[derive(Debug, Clone, Copy, EnumCount, PartialEq, Eq, Enum, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum FortressUnitType {
    Soldier = 0,
    Magician = 1,
    Archer = 2,
}

impl FortressUnitType {
    /// The building, that trains this unit type
    #[must_use]
    pub fn training_building(&self) -> FortressBuildingType {
        match self {
            FortressUnitType::Archer => FortressBuildingType::ArcheryGuild,
            FortressUnitType::Magician => FortressBuildingType::MagesTower,
            FortressUnitType::Soldier => FortressBuildingType::Barracks,
        }
    }
}

/// Generic information about a building in the fortress. If you want
/// information about a production building, you should look at the resources
#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressBuilding {
    /// The current level of this building. If this is 0, it has not yet been
    /// build
    pub level: u16,
    /// The amount of resources it costs to upgrade to the next level
    pub upgrade_cost: FortressCost,
}

impl Fortress {
    /// Check if units are being trained in the building (soldiers in barracks,
    /// magicians in mages' tower, archers in archery guild), or gem mining is
    /// in progress
    #[must_use]
    pub fn in_use(&self, building_type: FortressBuildingType) -> bool {
        // Check if associated units are training
        if let Some(unit_type) = building_type.unit_produced()
            && let Some(finish) = self.units.get(unit_type).training.finish
            && finish > Local::now()
        {
            return true;
        }

        // Check if gem mining is in progress
        if building_type == FortressBuildingType::GemMine
            && self.gem_search.finish.is_some()
        {
            return true;
        }
        false
    }

    /// Checks whether or not it is possible to build/upgrade a building
    #[must_use]
    pub fn can_build(
        &self,
        building_type: FortressBuildingType,
        available_silver: u64,
    ) -> bool {
        let building_info = self.buildings.get(building_type);
        let fortress_level =
            self.buildings.get(FortressBuildingType::Fortress).level;
        let smithy_required_buildings = [
            FortressBuildingType::ArcheryGuild,
            FortressBuildingType::Barracks,
            FortressBuildingType::MagesTower,
            FortressBuildingType::Wall,
        ];

        // Checks whether a building is in use, in such a way that it prevents
        // upgrading (for example, if a unit is being trained, or gem mining is
        // in progress)
        if self.in_use(building_type) {
            return false;
        }

        // Smithy can only be built if these buildings exist
        let can_smithy_be_built = smithy_required_buildings
            .map(|required_building| {
                self.buildings.get(required_building).level
            })
            .iter()
            .all(|level| *level > 0);

        if matches!(building_type, FortressBuildingType::Smithy)
            && !can_smithy_be_built
        {
            // Some buildings which are required to built Smithy do not exist
            false
        } else if !matches!(building_type, FortressBuildingType::Fortress)
            && building_info.level == fortress_level
        {
            // It is not possible to upgrade a building to a higher level than
            // the fortress level
            false
        } else {
            let upgrade_cost = building_info.upgrade_cost;

            // Check if required fortress level has been reached
            building_type.required_min_fortress_level() <= fortress_level
            // Check that no construction is in progress
            && self.building_upgrade.target.is_none()
            // Check if there are enough resources
            && upgrade_cost.stone <= self.resources.get(FortressResourceType::Stone).current
            && upgrade_cost.wood <= self.resources.get(FortressResourceType::Wood).current
            && upgrade_cost.silver <= available_silver
        }
    }

    pub(crate) fn update_resources(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        for (idx, (typ, resource)) in self.resources.iter_mut().enumerate() {
            resource.production.last_collectable =
                data.csiget(idx, "ft resource last collectable", 0)?;
            resource.production.limit =
                data.csiget(3 + idx, "ft resource production limit", 0)?;
            resource.production.per_hour =
                data.csiget(8 + idx, "ft resource per hour", 0)?;

            if typ != FortressResourceType::Experience {
                resource.limit =
                    data.csiget(6 + idx, "ft resource limit", 0)?;
                resource.limit_next_level =
                    data.csiget(12 + idx, "ft resource per hour", 0)?;
                resource.secret_storage.limit =
                    data.csiget(14 + idx, "ft secret storage limit", 0)?;
            }
        }
        self.last_collectable_updated =
            data.cstget(11, "ft resource update", server_time)?;
        Ok(())
    }

    pub(crate) fn update_unit_prices(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        for (i, typ) in FortressUnitType::iter().enumerate() {
            self.units.get_mut(typ).training.cost =
                FortressCost::parse_slice(data.skip(i * 4, "unit prices")?)?;
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
            self.units.get_mut(typ).upgrade_cost.wood =
                data.csiget(1 + i * 3, "wood price next unit lvl", 0)?;
            self.units.get_mut(typ).upgrade_cost.stone =
                data.csiget(2 + i * 3, "stone price next unit lvl", 0)?;
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

    /// Updates building upgrade costs and gem search cost.
    pub(crate) fn update_prices(
        &mut self,
        data: &ResponseVals<52>,
    ) -> Result<(), SFError> {
        let mut chunks = data.chunks_exact::<4>();
        for typ in FortressBuildingType::iter() {
            if let Some(cost_vals) = chunks.next() {
                self.buildings.get_mut(typ).upgrade_cost =
                    FortressCost::parse(&cost_vals)?;
            }
        }
        if let Some(gem_cost_vals) = chunks.next() {
            self.gem_search.cost = FortressCost::parse(&gem_cost_vals)?;
        }
        Ok(())
    }

    pub(crate) fn update_units(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        for (idx, unit) in self.units.values_mut().enumerate() {
            unit.count = data.csiget(idx, "ft unit count", 0)?;
            unit.in_training = data.csiget(3 + idx, "ft unit in que", 0)?;
            unit.training.start =
                data.cstget(6 + idx, "ft training start", server_time)?;
            unit.training.finish =
                data.cstget(9 + idx, "ft training end", server_time)?;
            // NOTE: 12 + idx has another value, which I can not associate with
            // anything. It is similar to the base amount of units you can have
            // max, but it diverges after a few levels
        }
        Ok(())
    }

    /// Updates the fortress state from a 27-element response chunk.
    pub(crate) fn update(
        &mut self,
        data: &ResponseVals<27>,
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        // Buildings
        for (idx, building) in self.buildings.values_mut().enumerate() {
            building.level = data.u16(idx)?;
        }

        let upgrade = &mut self.building_upgrade;
        upgrade.target = data
            .u32(12)?
            .checked_sub(1)
            .and_then(FortressBuildingType::from_u32);
        upgrade.finish = data.server_time(13, server_time)?;
        upgrade.start = data.server_time(14, server_time)?;

        self.upgrades = data.u16(15)?;
        self.honor = data.u32(16)?;
        self.rank = Some(data.u32(17)?).filter(|&x| x != 0);

        self.attack_free_reroll = data.server_time(18, server_time)?;
        self.attack_target = Some(data.u32(19)?).filter(|&x| x != 0);
        data.ignore(20, "unknown server timestamp");
        data.ignore(21, "unknown server timestamp");
        self.gem_search.target = GemType::parse(data.i64(22)?, 0);
        self.gem_search.finish = data.server_time(23, server_time)?;
        self.gem_search.start = data.server_time(24, server_time)?;
        self.hall_of_knights_level = data.u16(25)?;
        data.ignore(26, "unknown server value");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn test_fortress_cost_parse() {
        crate::parse_test!(FortressCost("7200/4000/1100/333"), {
            time: Duration::from_secs(7200),
            silver: 4000,
            wood: 1100,
            stone: 333,
        });
    }

    #[test]
    fn test_fortress_update_prices() {
        let raw = "7200/4000/1100/333/7200/2000/1015/308/7200/800/220/133/7200/1200/660/107/7200/6000/1467/444/765/700/7/9/765/500/41/7/765/400/20/14/765/600/61/20/7200/10000/1173/356/765/400/25/8/765/15000/30/13/9180/45000/29/9";
        let mut fortress = Fortress::default();
        let vals = match raw.try_into() {
            Ok(v) => v,
            Err(e) => panic!("invalid vals: {e}"),
        };
        fortress.update_prices(&vals).unwrap();

        let ft = fortress.buildings[FortressBuildingType::Fortress].upgrade_cost;
        assert_eq!(
            (ft.time.as_secs(), ft.silver, ft.wood, ft.stone),
            (7200, 4000, 1100, 333)
        );

        let gem = fortress.gem_search.cost;
        assert_eq!(
            (gem.time.as_secs(), gem.silver, gem.wood, gem.stone),
            (9180, 45000, 29, 9)
        );
    }

    #[test]
    fn test_fortress_update() {
        let raw = "19/15/18/18/18/18/15/15/15/18/18/18/3/1786227769/1785018169/376/7053/1565/1785024031/278315/1459278099/1784875170/0/0/0/18/348143";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress(raw, ServerTime::default()), {
            upgrades: 376,
            honor: 7053,
            rank: Some(1565),
            attack_target: Some(278_315),
            hall_of_knights_level: 18,
        });

        assert_eq!(
            fortress.buildings[FortressBuildingType::Fortress].level,
            19
        );
        assert_eq!(
            fortress.buildings[FortressBuildingType::LaborersQuarters].level,
            15
        );
        assert_eq!(
            fortress.buildings[FortressBuildingType::WoodcuttersHut].level,
            18
        );
        assert_eq!(
            fortress.building_upgrade.target,
            Some(FortressBuildingType::WoodcuttersHut)
        );
        assert!(fortress.building_upgrade.start.is_some());
        assert!(fortress.building_upgrade.finish.is_some());
        assert!(fortress.attack_free_reroll.is_some());
        assert_eq!(fortress.gem_search.target, None);
    }

    #[test]
    fn test_fortress_update_idle_and_gem_search() {
        let raw = "19/15/19/18/18/18/15/15/15/18/18/18/0/0/0/377/7069/1583/1790325015/370262/1459278099/1791987830/13/1790346770/1790323370/18/521703";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress(raw, ServerTime::default()), {
            upgrades: 377,
            honor: 7069,
            rank: Some(1583),
            attack_target: Some(370_262),
            hall_of_knights_level: 18,
        });

        assert_eq!(fortress.building_upgrade.target, None);
        assert_eq!(fortress.building_upgrade.start, None);
        assert_eq!(fortress.building_upgrade.finish, None);
        assert_eq!(fortress.gem_search.target, Some(GemType::Constitution));
        assert!(fortress.gem_search.start.is_some());
        assert!(fortress.gem_search.finish.is_some());
    }
}
