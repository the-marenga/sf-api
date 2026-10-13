#![allow(clippy::module_name_repetitions)]
use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive as _;
use strum::{EnumCount, EnumIter};

use super::{Result, ServerTime, items::GemType};
use crate::response::{Vals, ValueArray};

/// The information about a characters fortress
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fortress {
    /// All the buildings, that a fortress can have. If they are not yet built,
    /// they are level 0
    pub buildings: EnumMap<FortressBuildingType, FortressBuilding>,
    /// Information about all the buildable units in the fortress
    pub units: EnumMap<FortressUnitType, FortressUnit>,
    /// All information about resources in the fortress
    pub resources: EnumMap<FortressResourceType, FortressResource>,
    /// The `last_collectable` variable in `FortressProduction` is NOT
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
    pub attack_target: Option<u32>,
    /// The time at which switching is free again
    pub attack_free_reroll: Option<DateTime<Local>>,
    /// The price in silver re-rolling costs
    pub opponent_reroll_price: u64,
}

/// The price an upgrade, or building something in the fortress costs. These
/// are always for one upgrade/build, which is important for unit builds
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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
    pub(crate) fn parse(data: &ValueArray<4>) -> Result<FortressCost> {
        Ok(FortressCost {
            time: Duration::from_secs(data.u64(0)?),
            silver: data.u64(1)?,
            wood: data.u64(2)?,
            stone: data.u64(3)?,
        })
    }
}

/// Information about one of the three resources, that the fortress can produce.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressResource {
    /// The amount of this resource you have available to spend on upgrades and
    /// recruitment
    pub current: u64,
    /// The maximum amount of this resource, that you can store. If `current ==
    /// limit`, you will not be able to collect resources from buildings
    pub limit: u64,
    /// The maximum amount of this resource that can be stored in the fortress
    /// after upgrading the Fortress main building to the next level
    pub limit_next_level: u64,
    /// Information about the production building, that produces this resource.
    pub production: FortressProduction,
    /// The secret storage, available in the treasury
    pub secret_storage: FortressSecretStorage,
}

/// The secret storage in the treasury where wood and stone are protected.
///
/// Resources obtained outside of regular building production (such as from
/// quests, the Wheel of Fortune, or events) are stored and secured here.
///
/// All resources stored here are safe from enemy attacks during fortress
/// battles and cannot be looted. The capacity is 30% of the main fortress
/// resource storage limit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressSecretStorage {
    /// The current amount of resources stored in secret storage
    pub amount: u64,
    /// The maximum amount of resources that can be protected in secret storage
    /// (30% of the main fortress storage limit)
    pub limit: u64,
}

/// Information about the production of a resource in the fortress.  Note that
/// experience will not have some of these fields
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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

/// The type of resource available in the fortress
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum, EnumCount, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressResourceType {
    /// Wood produced by the Woodcutter's Hut; used for buildings and units
    Wood = 0,
    /// Stone produced by the Quarry; used for buildings and units
    Stone = 1,
    /// Experience produced by the Academy; awarded to the player character
    Experience = 2,
}

/// The type of building that can be built in the fortress
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Enum, EnumCount, EnumIter, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressBuildingType {
    /// The main fortress building; determines the maximum level of all other
    /// buildings and increases resource storage capacity
    Fortress = 0,
    /// Laborer's Quarters; reduces the construction and upgrade duration of all
    /// fortress buildings
    LaborersQuarters = 1,
    /// Woodcutter's Hut; produces wood over time
    WoodcuttersHut = 2,
    /// Quarry; produces stone over time
    Quarry = 3,
    /// Gem Mine; allows searching for attribute and black gems
    GemMine = 4,
    /// Academy; produces experience for the character over time
    Academy = 5,
    /// Archery Guild; trains archers for fortress defense
    ArcheryGuild = 6,
    /// Barracks; trains soldiers for fortress attacks and defense
    Barracks = 7,
    /// Mage's Tower; trains battle mages for fortress defense
    MagesTower = 8,
    /// Treasury; expands backpack/chest capacity and provides secret storage
    Treasury = 9,
    /// Smithy; upgrades the weapon and armor quality of combat units
    Smithy = 10,
    /// Fortress Wall; provides a defensive bonus during enemy attacks
    Wall = 11,
}

impl FortressBuildingType {
    /// Minimum fortress level required to build this building
    #[must_use]
    pub fn required_min_fortress_level(self) -> u16 {
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
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressUnit {
    /// The level this unit has
    pub level: u16,

    /// The amount of this unit available for combat
    pub count: u16,
    /// The amount of this unit currently being trained
    pub in_training: u16,
    /// The maximum `count + in_training` capacity for this unit
    pub limit: u16,
    /// All information about training up new units of this type
    pub training: FortressAction<()>,

    /// The resource cost for the next unit level upgrade
    pub upgrade_cost: FortressCost,
    /// The level this unit will be at when upgraded
    pub upgrade_next_lvl: u64,
}

/// An action that costs resources and finishes after a specific duration.
///
/// The generic parameter `T` specifies the action target:
/// - [`FortressBuildingType`] for building upgrades
///   ([`Fortress::building_upgrade`])
/// - [`GemType`] for gem searches ([`Fortress::gem_search`])
/// - `()` for unit recruitment ([`FortressUnit::training`])
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressAction<T> {
    /// When this action was started
    pub start: Option<DateTime<Local>>,
    /// When this action will finish
    pub finish: Option<DateTime<Local>>,
    /// The amount of resources required for this action
    pub cost: FortressCost,
    /// The specific target of this action, if applicable
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum, EnumCount, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressUnitType {
    /// Soldiers; used for fortress attacks and defense
    Soldier = 0,
    /// Battle mages; used for fortress defense
    Magician = 1,
    /// Archers; used for fortress defense
    Archer = 2,
}

impl FortressUnitType {
    /// The building, that trains this unit type
    #[must_use]
    pub fn training_building(self) -> FortressBuildingType {
        match self {
            FortressUnitType::Archer => FortressBuildingType::ArcheryGuild,
            FortressUnitType::Magician => FortressBuildingType::MagesTower,
            FortressUnitType::Soldier => FortressBuildingType::Barracks,
        }
    }
}

/// Generic information about a building in the fortress. If you want
/// information about a production building, you should look at the resources
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressBuilding {
    /// The current level of this building. If this is 0, it has not yet been
    /// built
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
            && let Some(finish) = self.units[unit_type].training.finish
        {
            return finish > Local::now();
        }

        // Check if gem mining is in progress
        building_type == FortressBuildingType::GemMine
            && self.gem_search.finish.is_some()
    }

    /// Checks whether or not it is possible to build/upgrade a building
    #[must_use]
    pub fn can_build(
        &self,
        building_type: FortressBuildingType,
        available_silver: u64,
    ) -> bool {
        // Checks whether a building is in use, or another upgrade is in
        // progress
        if self.in_use(building_type) || self.building_upgrade.target.is_some()
        {
            return false;
        }

        let fortress_level =
            self.buildings[FortressBuildingType::Fortress].level;

        // Check if required fortress level has been reached
        if building_type.required_min_fortress_level() > fortress_level {
            return false;
        }

        let building_info = &self.buildings[building_type];

        // It is not possible to upgrade a building to a higher level than the
        // fortress level
        if building_type != FortressBuildingType::Fortress
            && building_info.level >= fortress_level
        {
            return false;
        }

        // Smithy can only be built if all combat buildings exist
        if building_type == FortressBuildingType::Smithy {
            let smithy_required_buildings = [
                FortressBuildingType::ArcheryGuild,
                FortressBuildingType::Barracks,
                FortressBuildingType::MagesTower,
                FortressBuildingType::Wall,
            ];
            let can_smithy_be_built = smithy_required_buildings
                .into_iter()
                .all(|b| self.buildings[b].level > 0);
            if !can_smithy_be_built {
                return false;
            }
        }

        let upgrade_cost = building_info.upgrade_cost;
        upgrade_cost.stone
            <= self.resources[FortressResourceType::Stone].current
            && upgrade_cost.wood
                <= self.resources[FortressResourceType::Wood].current
            && upgrade_cost.silver <= available_silver
    }

    /// Updates the fortress resources from a 16-element response chunk.
    pub(crate) fn update_resources(
        &mut self,
        data: &ValueArray<16>,
        server_time: ServerTime,
    ) -> Result<()> {
        for (idx, (typ, resource)) in self.resources.iter_mut().enumerate() {
            resource.production.last_collectable = data.u64(idx)?;
            resource.production.limit = data.u64(3 + idx)?;
            resource.production.per_hour = data.u64(8 + idx)?;

            if typ != FortressResourceType::Experience {
                resource.limit = data.u64(6 + idx)?;
                resource.limit_next_level = data.u64(12 + idx)?;
                resource.secret_storage.limit = data.u64(14 + idx)?;
            }
        }
        self.last_collectable_updated = data.server_time(11, server_time)?;
        Ok(())
    }

    /// Updates unit recruitment costs from a 12-element response chunk.
    pub(crate) fn update_unit_prices(
        &mut self,
        data: &ValueArray<12>,
    ) -> Result<()> {
        for (unit, cost_vals) in
            self.units.values_mut().zip(data.chunks_exact::<4>())
        {
            unit.training.cost = FortressCost::parse(&cost_vals)?;
        }
        Ok(())
    }

    /// Updates unit upgrade requirements from a 9-element response chunk.
    pub(crate) fn update_unit_upgrade_info(
        &mut self,
        data: &ValueArray<9>,
    ) -> Result<()> {
        for (unit, chunk) in
            self.units.values_mut().zip(data.chunks_exact::<3>())
        {
            unit.upgrade_next_lvl = chunk.u64(0)?;
            unit.upgrade_cost.wood = chunk.u64(1)?;
            unit.upgrade_cost.stone = chunk.u64(2)?;
        }
        Ok(())
    }

    /// Updates the fortress unit levels from a 4-element response chunk.
    pub(crate) fn update_levels(&mut self, data: &ValueArray<4>) -> Result<()> {
        data.ignore(0, "might be max unit level");
        for (i, unit) in self.units.values_mut().enumerate() {
            unit.level = data.u16(1 + i)?;
        }
        Ok(())
    }

    /// Updates building upgrade costs and gem search cost.
    pub(crate) fn update_prices(
        &mut self,
        data: &ValueArray<52>,
    ) -> Result<()> {
        let mut chunks = data.chunks_exact::<4>();
        for (building, cost_vals) in
            self.buildings.values_mut().zip(&mut chunks)
        {
            building.upgrade_cost = FortressCost::parse(&cost_vals)?;
        }
        if let Some(gem_cost_vals) = chunks.next() {
            self.gem_search.cost = FortressCost::parse(&gem_cost_vals)?;
        }
        Ok(())
    }

    /// Updates the fortress units from a 15-element response chunk.
    pub(crate) fn update_units(
        &mut self,
        data: &ValueArray<15>,
        server_time: ServerTime,
    ) -> Result<()> {
        for (idx, unit) in self.units.values_mut().enumerate() {
            unit.count = data.u16(idx)?;
            unit.in_training = data.u16(3 + idx)?;
            unit.training.start = data.server_time(6 + idx, server_time)?;
            unit.training.finish = data.server_time(9 + idx, server_time)?;
            data.ignore(12 + idx, "unknown unit value (diverges from limit)");
        }
        Ok(())
    }

    /// Updates the fortress state from a 27-element response chunk.
    pub(crate) fn update(
        &mut self,
        data: &ValueArray<27>,
        server_time: ServerTime,
    ) -> Result<()> {
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
        let raw = "7200/4000/1100/333/7200/2000/1015/308/7200/800/220/133/\
                   7200/1200/660/107/7200/6000/1467/444/765/700/7/9/765/500/\
                   41/7/765/400/20/14/765/600/61/20/7200/10000/1173/356/765/\
                   400/25/8/765/15000/30/13/9180/45000/29/9";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress.update_prices(raw), {});

        let ft =
            fortress.buildings[FortressBuildingType::Fortress].upgrade_cost;
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
        let raw = "19/15/18/18/18/18/15/15/15/18/18/18/3/1786227769/\
                   1785018169/376/7053/1565/1785024031/278315/1459278099/\
                   1784875170/0/0/0/18/348143";
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
        let raw = "19/15/19/18/18/18/15/15/15/18/18/18/0/0/0/377/7069/1583/\
                   1790325015/370262/1459278099/1791987830/13/1790346770/\
                   1790323370/18/521703";
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

    #[test]
    fn test_fortress_update_units() {
        let raw =
            "45/0/0/0/0/0/1790341727/1790327907/1790327907/0/0/0/18/18/18";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress.update_units(raw, ServerTime::default()), {});

        assert_eq!(fortress.units[FortressUnitType::Soldier].count, 45);
        assert_eq!(fortress.units[FortressUnitType::Soldier].in_training, 0);
        assert!(
            fortress.units[FortressUnitType::Soldier]
                .training
                .start
                .is_some()
        );
        assert_eq!(
            fortress.units[FortressUnitType::Soldier].training.finish,
            None
        );

        assert_eq!(fortress.units[FortressUnitType::Magician].count, 0);
        assert_eq!(fortress.units[FortressUnitType::Magician].in_training, 0);
        assert!(
            fortress.units[FortressUnitType::Magician]
                .training
                .start
                .is_some()
        );
        assert_eq!(
            fortress.units[FortressUnitType::Magician].training.finish,
            None
        );

        assert_eq!(fortress.units[FortressUnitType::Archer].count, 0);
        assert_eq!(fortress.units[FortressUnitType::Archer].in_training, 0);
        assert!(
            fortress.units[FortressUnitType::Archer]
                .training
                .start
                .is_some()
        );
        assert_eq!(
            fortress.units[FortressUnitType::Archer].training.finish,
            None
        );
    }

    #[test]
    fn test_fortress_update_levels() {
        let raw = "190/140/140/140";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress.update_levels(raw), {});

        assert_eq!(fortress.units[FortressUnitType::Soldier].level, 140);
        assert_eq!(fortress.units[FortressUnitType::Magician].level, 140);
        assert_eq!(fortress.units[FortressUnitType::Archer].level, 140);
    }

    #[test]
    fn test_fortress_update_unit_upgrade_info() {
        let raw = "145/49500000/37800000/145/132000000/10800000/145/66000000/\
                   32400000";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress.update_unit_upgrade_info(raw), {});

        let soldier = &fortress.units[FortressUnitType::Soldier];
        assert_eq!(soldier.upgrade_next_lvl, 145);
        assert_eq!(soldier.upgrade_cost.wood, 49_500_000);
        assert_eq!(soldier.upgrade_cost.stone, 37_800_000);

        let mage = &fortress.units[FortressUnitType::Magician];
        assert_eq!(mage.upgrade_next_lvl, 145);
        assert_eq!(mage.upgrade_cost.wood, 132_000_000);
        assert_eq!(mage.upgrade_cost.stone, 10_800_000);

        let archer = &fortress.units[FortressUnitType::Archer];
        assert_eq!(archer.upgrade_next_lvl, 145);
        assert_eq!(archer.upgrade_cost.wood, 66_000_000);
        assert_eq!(archer.upgrade_cost.stone, 32_400_000);
    }

    #[test]
    fn test_fortress_update_unit_prices() {
        let raw = "600/0/20250/6750/600/0/15187/8437/300/0/25312/5062";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress.update_unit_prices(raw), {});

        let soldier_cost =
            fortress.units[FortressUnitType::Soldier].training.cost;
        assert_eq!(soldier_cost.time.as_secs(), 600);
        assert_eq!(soldier_cost.wood, 20_250);
        assert_eq!(soldier_cost.stone, 6750);

        let mage_cost =
            fortress.units[FortressUnitType::Magician].training.cost;
        assert_eq!(mage_cost.time.as_secs(), 600);
        assert_eq!(mage_cost.wood, 15_187);
        assert_eq!(mage_cost.stone, 8437);

        let archer_cost =
            fortress.units[FortressUnitType::Archer].training.cost;
        assert_eq!(archer_cost.time.as_secs(), 300);
        assert_eq!(archer_cost.wood, 25_312);
        assert_eq!(archer_cost.stone, 5062);
    }

    #[test]
    fn test_fortress_update_resources() {
        let raw = "614062/136458/4911687/6300000/1350000/4911687/165000000/\
                   54000000/225000/50000/454785/1791911255/300000000/\
                   100000000/49500000/16200000";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress.update_resources(raw, ServerTime::default()), {});

        let wood = &fortress.resources[FortressResourceType::Wood];
        assert_eq!(wood.production.last_collectable, 614_062);
        assert_eq!(wood.production.limit, 6_300_000);
        assert_eq!(wood.production.per_hour, 225_000);
        assert_eq!(wood.limit, 165_000_000);
        assert_eq!(wood.limit_next_level, 300_000_000);
        assert_eq!(wood.secret_storage.limit, 49_500_000);

        let stone = &fortress.resources[FortressResourceType::Stone];
        assert_eq!(stone.production.last_collectable, 136_458);
        assert_eq!(stone.production.limit, 1_350_000);
        assert_eq!(stone.production.per_hour, 50_000);
        assert_eq!(stone.limit, 54_000_000);
        assert_eq!(stone.limit_next_level, 100_000_000);
        assert_eq!(stone.secret_storage.limit, 16_200_000);

        let exp = &fortress.resources[FortressResourceType::Experience];
        assert_eq!(exp.production.last_collectable, 4_911_687);
        assert_eq!(exp.production.limit, 4_911_687);
        assert_eq!(exp.production.per_hour, 454_785);

        assert!(fortress.last_collectable_updated.is_some());
    }
}
