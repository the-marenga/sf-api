use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::EnumIter;

use super::ServerTime;
use crate::response::Vals;

pub(crate) const TOWER_RESPONSE_LEN: usize = 476;
pub(crate) const TOWER_BLOCK_LEN: usize = 148;

/// The information about a characters underworld
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Underworld {
    /// All the buildings, that the underworld can have. If they are not yet
    /// build, they are level 0
    pub buildings: EnumMap<UnderworldBuildingType, UnderworldBuilding>,
    /// Information about all the buildable units in the underworld
    pub units: EnumMap<UnderworldUnitType, UnderworldUnit>,
    /// All information about the production of resources in the underworld
    pub production: EnumMap<UnderworldResourceType, UnderworldProduction>,
    /// The `last_collectable` value in `UnderworldProduction` is always out of
    /// date. Refer to the `Fortress.last_collectable_updated` for more
    /// information
    pub last_collectable_update: Option<DateTime<Local>>,

    // Both XP&silver are not really resources, so I just have this here,
    // instead of in a resource info struct like in fortress
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

    /// The level of characters you need to lure to get the full reward
    pub lure_level: u16,
    /// The amount of players, that have been lured into the underworld today
    pub lured_today: u16,
    /// The suggested enemy to attack in the underworld. Must be populated with
    /// the `UpdateLureSuggestion` command.
    pub lure_suggestion: Option<LureSuggestion>,
}

/// The ident by which we can fetch more information about the lure enemy using
/// the `ViewLureSuggestion` command.
/// (Don't ask me why this process is so convoluted)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LureSuggestion(pub(crate) u32);

/// The price an upgrade, or building something in the underworld costs. These
/// are always for one upgrade/build, which is important for unit builds
#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderworldCost {
    /// The time it takes to complete one build/upgrade
    pub time: Duration,
    /// The price in silver this costs
    pub silver: u64,
    /// The price in sould this costs
    pub souls: u64,
}

impl UnderworldCost {
    pub(crate) fn parse(data: &Vals<'_>) -> UnderworldCost {
        UnderworldCost {
            time: Duration::from_secs(data.u64_or(0, 0)),
            silver: data.u64_or(1, u64::MAX),
            souls: data.u64_or(2, u64::MAX),
        }
    }
}

impl Underworld {
    pub(crate) fn update_building_prices(&mut self, data: &Vals<'_>) {
        for (pos, building) in self.buildings.values_mut().enumerate() {
            let start = pos * 3;
            building.upgrade_cost =
                UnderworldCost::parse(&data.window(start..start + 3));
        }
    }

    pub(crate) fn update_underworld_unit_prices(&mut self, data: &Vals<'_>) {
        for (pos, unit) in self.units.values_mut().enumerate() {
            let start = pos * 3;
            let chunk = data.window(start..start + 3);
            unit.upgrade_next_lvl = chunk.u16_or(0, 0);
            unit.upgrade_cost.silver = chunk.u64_or(1, 0);
            unit.upgrade_cost.souls = chunk.u64_or(2, 0);
        }
    }

    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        use UnderworldResourceType as Resource;

        for (pos, building) in self.buildings.values_mut().enumerate() {
            building.level = data.u8_or(1 + pos, 0);
        }

        self.production[Resource::Souls].last_collectable = data.u64_or(12, 0);
        self.production[Resource::Souls].limit = data.u64_or(13, 0);
        self.souls_limit = data.u64_or(14, 0);
        self.production[Resource::Souls].per_hour = data.u64_or(16, 0);

        self.production[Resource::Silver].last_collectable = data.u64_or(17, 0);
        self.production[Resource::Silver].limit = data.u64_or(18, 0);
        self.production[Resource::Silver].per_hour = data.u64_or(19, 0);

        self.production[Resource::ThirstForAdventure].last_collectable =
            data.u64_or(26, 0);
        self.production[Resource::ThirstForAdventure].limit =
            data.u64_or(27, 0);
        self.production[Resource::ThirstForAdventure].per_hour =
            data.u64_or(28, 0);

        self.last_collectable_update = data.server_time_opt(20, server_time);
        self.upgrade_building = data
            .i64_or(21, 0)
            .checked_sub(1)
            .and_then(UnderworldBuildingType::from_i64);
        self.upgrade_finish = data.server_time_opt(22, server_time);
        self.upgrade_begin = data.server_time_opt(23, server_time);
        self.lure_level = data.u16_or(24, 0);
        self.lured_today = data.u16_or(25, 0);

        data.ignore(0, "always 0");
        data.ignore(11, "always 0");
        data.ignore(15, "always 0");
    }
}

/// The type of a producible resource in the underworld
#[derive(Debug, Clone, Copy, strum::EnumCount, Enum, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum UnderworldResourceType {
    Silver = 0,
    Souls = 1,
    #[doc(alias = "ALU")]
    ThirstForAdventure = 2,
}

/// Information about the producion of a resource in the fortress.  Note that
/// experience will not have some of these fields
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderworldProduction {
    /// The amount the production building has already produced, that you can
    /// collect. Note that this value will be out of date by some amount of
    /// time. If you need the exact current amount collectable, look at
    /// `last_collectable_update`
    pub last_collectable: u64,
    /// The maximum amount of this resource, that this building can store. If
    /// `building_collectable == building_limit` the production stops
    pub limit: u64,
    /// The amount of this resource the corresponding production building
    /// produces per hour. The adventuromatics amount will be per day here
    pub per_hour: u64,
}

/// The type of building in the underworld
#[derive(
    Debug,
    Clone,
    Copy,
    FromPrimitive,
    strum::EnumCount,
    Enum,
    EnumIter,
    PartialEq,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
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

/// The type of unit in the underworld
#[derive(Debug, Clone, Copy, strum::EnumCount, Enum, EnumIter, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum UnderworldUnitType {
    Goblin = 0,
    Troll = 1,
    Keeper = 2,
}

/// Information about the current building state of a building
#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderworldBuilding {
    /// The current level of this building. If this is 0, it has not yet been
    /// built
    pub level: u8,
    /// The amount of resources it costs to upgrade to the next level
    pub upgrade_cost: UnderworldCost,
}

/// Information about a single type of unit
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl UnderworldUnit {
    pub(crate) fn update(&mut self, data: &Vals<'_>) {
        self.upgraded_amount = data.u16_or(0, 0);
        self.count = data.u16_or(1, 0);
        self.total_attributes = data.u32_or(2, 0);
        self.level = data.u16_or(3, 0);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn parses_costs_and_tolerates_strings() {
        let vals = Vals::from("7200/4000/1100");
        let cost = UnderworldCost::parse(&vals);
        assert_eq!(cost.time, Duration::from_secs(7200));
        assert_eq!(cost.silver, 4000);
        assert_eq!(cost.souls, 1100);
        vals.assert_all_read();

        let vals = Vals::from("future/future/future");
        let cost = UnderworldCost::parse(&vals);
        assert_eq!(cost.time, Duration::ZERO);
        assert_eq!(cost.silver, u64::MAX);
        assert_eq!(cost.souls, u64::MAX);
        vals.assert_all_read();
    }

    #[test]
    fn updates_building_prices() {
        let raw = "0/0/0/0/0/0/1296000/1000000000/162518400/0/0/0/0/0/0/0/0/0/\
                   0/0/0/0/0/0/0/0/0/0/0/0";
        let mut uw = Underworld::default();
        let vals = Vals::from(raw);
        uw.update_building_prices(&vals);
        vals.assert_all_read();

        let gold_pit =
            uw.buildings[UnderworldBuildingType::GoldPit].upgrade_cost;
        assert_eq!(gold_pit.time.as_secs(), 1_296_000);
        assert_eq!(gold_pit.silver, 1_000_000_000);
        assert_eq!(gold_pit.souls, 162_518_400);
    }

    #[test]
    fn updates_unit_prices() {
        let raw = "417/500000/425800/359/500000/340150/369/500000/354450";
        let mut uw = Underworld::default();
        let vals = Vals::from(raw);
        uw.update_underworld_unit_prices(&vals);
        vals.assert_all_read();

        let goblin = &uw.units[UnderworldUnitType::Goblin];
        assert_eq!(goblin.upgrade_next_lvl, 417);
        assert_eq!(goblin.upgrade_cost.silver, 500_000);
        assert_eq!(goblin.upgrade_cost.souls, 425_800);
    }

    #[test]
    fn truncated_prices_apply_defaults() {
        let vals = Vals::from("5");
        let mut uw = Underworld::default();
        uw.update_underworld_unit_prices(&vals);
        vals.assert_all_read();
        assert_eq!(uw.units[UnderworldUnitType::Goblin].upgrade_next_lvl, 5);
        assert_eq!(uw.units[UnderworldUnitType::Troll].upgrade_next_lvl, 0);
    }
}
