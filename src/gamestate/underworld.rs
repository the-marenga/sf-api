#![allow(clippy::module_name_repetitions)]
use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use strum::EnumIter;

use super::{Result, ServerTime};
use crate::response::Vals;

/// The number of values in the `owntower` (`towerSave`) response. It is shared
/// between the underworld and the companion parsing.
pub(crate) const TOWER_RESPONSE_LEN: usize = 476;

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

    /// Updates the underworld state from the 476-value `owntower`
    /// (`towerSave`) response.
    ///
    /// That response is shared with the companion parsing in `mod.rs`: every
    /// 148-value block holds a companion at offset 3 (level) and 7-10
    /// (attributes) and an underworld unit at offset 146-149, and the tail
    /// from index 442 holds the underworld buildings, production and upgrade
    /// timers. Everything that is not part of the underworld state is marked
    /// as ignored, so the read tracking does not report it.
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
            let start = 146 + i * 148;
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

        // 446-447 sit between the last unit and the first building, 458
        // between the last building and the soul production and 462 between
        // the soul storage limit and its production. 458 and 462, as well as
        // 446-447, are 0 in every captured response. The remaining ranges are
        // the companion values and per-block data that the underworld does
        // not model.
        data.ignore_unmodelled(
            &[0..146, 150..294, 298..442, 446..448, 458..459, 462..463],
            "tower field that is not part of the underworld state",
        );
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

    /// Asserts that every value of `data` was read or explicitly ignored. The
    /// read tracking only exists in debug builds.
    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

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

    #[test]
    fn test_update() {
        // From `cache/marenga.login`, `owntower.towerSave`.
        let raw =
            "446275/0/100/360/1/0/0/4147/2539/2422/3743/2900/7153/2622/2391/\
             6428/4079/0/0/0/0/0/17534/481/951/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/416/5/56614/600/10/360/2/0/0/2422/2539/4147/\
             3743/2900/508/558/9704/5366/2203/0/0/0/0/0/4235/1244/2734/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/358/4/51450/600/2/\
             360/3/0/0/2422/4147/2539/3743/2900/1003/8878/910/7251/4199/0/0/0/\
             0/0/7117/582/1154/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/368/1/106984/600/0/0/15/15/29/15/15/15/15/15/15/15/0/1188000/\
             1188000/162518400/0/49500/1439888618/1460585058/25182501/\
             1786541817/0/0/0/504/0/2000/2000/80";
        let data = Vals::from(raw);
        assert_eq!(data.len(), TOWER_RESPONSE_LEN);

        let mut uw = Underworld::default();
        uw.update(&data, ServerTime::default()).unwrap();
        assert_all_read(&data);

        assert_eq!(
            uw.buildings[UnderworldBuildingType::HeartOfDarkness].level,
            15
        );
        assert_eq!(uw.buildings[UnderworldBuildingType::GoldPit].level, 29);

        let goblin = &uw.units[UnderworldUnitType::Goblin];
        assert_eq!(goblin.upgraded_amount, 416);
        assert_eq!(goblin.count, 5);
        assert_eq!(goblin.total_attributes, 56_614);
        assert_eq!(goblin.level, 600);

        assert_eq!(uw.souls_limit, 162_518_400);
        assert_eq!(
            uw.production[UnderworldResourceType::Souls].per_hour,
            49_500
        );
        assert_eq!(uw.lure_level, 504);
        assert_eq!(uw.lured_today, 0);

        // No upgrade is running in this capture
        assert_eq!(uw.upgrade_building, None);
        assert_eq!(uw.upgrade_finish, None);
        assert_eq!(uw.upgrade_begin, None);
        assert!(uw.last_collectable_update.is_some());
    }
}
