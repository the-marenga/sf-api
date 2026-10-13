#![allow(clippy::module_name_repetitions)]
use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use strum::EnumIter;

use super::{Result, ServerTime};
use crate::response::Vals;

/// The number of values in the `owntower` (`towerSave`) response.
pub(crate) const TOWER_RESPONSE_LEN: usize = 476;
/// The size of one `owntower` block: a companion and an underworld unit.
pub(crate) const TOWER_BLOCK_LEN: usize = 148;
/// The offset of the underworld unit at the end of its `owntower` block.
pub(crate) const TOWER_UNIT_OFFSET: usize = 146;

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
    /// Parses an underworld cost `[time_seconds, silver, souls]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<UnderworldCost> {
        Ok(UnderworldCost {
            time: Duration::from_secs(data.u64(0)?),
            silver: data.u64(1)?,
            souls: data.u64(2)?,
        })
    }
}

impl Underworld {
    /// Updates the building upgrade costs from a 30-value response: 10
    /// `[time_seconds, silver, souls]` costs, one per building type.
    pub(crate) fn update_building_prices(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<()> {
        for (building, cost_vals) in
            self.buildings.values_mut().zip(data.chunks(3))
        {
            building.upgrade_cost = UnderworldCost::parse(&cost_vals)?;
        }
        Ok(())
    }

    /// Updates the unit upgrade costs from a 9-value response: 3
    /// `[next_level, silver, souls]` costs, one per unit type.
    pub(crate) fn update_underworld_unit_prices(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<()> {
        for (unit, chunk) in self.units.values_mut().zip(data.chunks(3)) {
            unit.upgrade_next_lvl = chunk.u16(0)?;
            unit.upgrade_cost.silver = chunk.u64(1)?;
            unit.upgrade_cost.souls = chunk.u64(2)?;
        }
        Ok(())
    }

    /// Updates the underworld state from the `owntower` (`towerSave`) response.
    /// The companion and unknown values in it are handled elsewhere.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        use UnderworldResourceType as Resource;

        for (pos, building) in self.buildings.values_mut().enumerate() {
            building.level = data.u8(448 + pos)?;
        }

        for (i, unit) in self.units.values_mut().enumerate() {
            // The unit sits at the end of its block
            let start = TOWER_UNIT_OFFSET + i * TOWER_BLOCK_LEN;
            unit.upgraded_amount = data.u16(start)?;
            unit.count = data.u16(start + 1)?;
            unit.total_attributes = data.u32(start + 2)?;
            unit.level = data.u16(start + 3)?;
        }

        self.production[Resource::Souls].last_collectable = data.u64(459)?;
        self.production[Resource::Souls].limit = data.u64(460)?;
        self.souls_limit = data.u64(461)?;
        self.production[Resource::Souls].per_hour = data.u64(463)?;

        self.production[Resource::Silver].last_collectable = data.u64(464)?;
        self.production[Resource::Silver].limit = data.u64(465)?;
        self.production[Resource::Silver].per_hour = data.u64(466)?;

        self.production[Resource::ThirstForAdventure].last_collectable =
            data.u64(473)?;
        self.production[Resource::ThirstForAdventure].limit = data.u64(474)?;
        self.production[Resource::ThirstForAdventure].per_hour =
            data.u64(475)?;

        self.last_collectable_update = data.server_time(467, server_time)?;
        self.upgrade_building = data.enum_map(468, |raw| raw.wrapping_sub(1));
        self.upgrade_finish = data.server_time(469, server_time)?;
        self.upgrade_begin = data.server_time(470, server_time)?;
        self.lure_level = data.u16(471)?;
        self.lured_today = data.u16(472)?;
        Ok(())
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_underworld_cost_parse() {
        crate::parse_test!(UnderworldCost("7200/4000/1100"), {
            time: Duration::from_secs(7200),
            silver: 4000,
            souls: 1100,
        });
    }

    #[test]
    fn test_update_building_prices() {
        // From `cache/marenga.login`, `underworldprice.underworldPrice(10)`.
        // Only the gold pit has a price so far, the rest is still 0.
        let raw = "0/0/0/0/0/0/1296000/1000000000/162518400/0/0/0/0/0/0/0/0/0/\
                   0/0/0/0/0/0/0/0/0/0/0/0";
        let mut uw = Underworld::default();
        crate::parse_test!(update: uw.update_building_prices(raw), {});

        let gold_pit =
            uw.buildings[UnderworldBuildingType::GoldPit].upgrade_cost;
        assert_eq!(gold_pit.time.as_secs(), 1_296_000);
        assert_eq!(gold_pit.silver, 1_000_000_000);
        assert_eq!(gold_pit.souls, 162_518_400);

        let heart =
            uw.buildings[UnderworldBuildingType::HeartOfDarkness].upgrade_cost;
        assert_eq!(
            (heart.time.as_secs(), heart.silver, heart.souls),
            (0, 0, 0)
        );
    }

    #[test]
    fn test_update_underworld_unit_prices() {
        // From `cache/marenga.login`, `underworldupgradeprice`.
        let raw = "417/500000/425800/359/500000/340150/369/500000/354450";
        let mut uw = Underworld::default();
        crate::parse_test!(
            update: uw.update_underworld_unit_prices(raw),
            {}
        );

        let goblin = &uw.units[UnderworldUnitType::Goblin];
        assert_eq!(goblin.upgrade_next_lvl, 417);
        assert_eq!(goblin.upgrade_cost.silver, 500_000);
        assert_eq!(goblin.upgrade_cost.souls, 425_800);

        let keeper = &uw.units[UnderworldUnitType::Keeper];
        assert_eq!(keeper.upgrade_next_lvl, 369);
        assert_eq!(keeper.upgrade_cost.souls, 354_450);
    }
}
