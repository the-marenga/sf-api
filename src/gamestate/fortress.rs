use std::time::Duration;

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive as _;
use strum::{EnumCount, EnumIter};

use crate::{
    gamestate::{Result, ServerTime, items::GemType},
    response::Vals,
};

/// Information about a character's fortress.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fortress {
    /// All buildings in the fortress (level 0 if unbuilt).
    pub buildings: EnumMap<FortressBuildingType, FortressBuilding>,
    /// Trainable combat units in the fortress.
    pub units: EnumMap<FortressUnitType, FortressUnit>,
    /// Production and storage information for all fortress resources.
    pub resources: EnumMap<FortressResourceType, FortressResource>,
    /// When resource production was last calculated by the server.
    pub last_collectable_updated: Option<DateTime<Local>>,

    /// The maximum level to which buildings can currently be upgraded.
    pub building_max_lvl: u8,
    /// The effective level of the fortress wall in defensive battles.
    pub wall_combat_lvl: u16,

    /// The building currently under construction or upgrade, if any.
    pub building_upgrade: FortressAction<FortressBuildingType>,

    /// The sum of all building levels, as displayed in the Hall of Fame.
    pub upgrades: u16,
    /// The player's honor in the fortress Hall of Fame.
    pub honor: u32,
    /// The player's rank in the fortress Hall of Fame, if ranked.
    pub rank: Option<u32>,

    /// The current gem search, if any. Start with
    /// [`Command::FortressGemStoneSearch`](crate::command::Command::FortressGemStoneSearch).
    pub gem_search: FortressAction<GemType>,

    /// The level of the Hall of Knights. Upgrade with
    /// [`Command::FortressUpgradeHallOfKnights`](crate::command::Command::FortressUpgradeHallOfKnights).
    #[doc(alias = "hok")]
    pub hall_of_knights_level: u16,
    /// The cost to upgrade the Hall of Knights (instant, duration is 0).
    pub hall_of_knights_upgrade_price: FortressCost,

    /// The player ID of the current enemy. Attack with
    /// [`Command::FortressAttack`](crate::command::Command::FortressAttack).
    #[doc(alias = "opponent")]
    #[doc(alias = "enemy")]
    pub attack_target: Option<u32>,
    /// When rerolling the opponent becomes free. Reroll with
    /// [`Command::FortressNewEnemy`](crate::command::Command::FortressNewEnemy).
    #[doc(alias = "opponent_free_reroll")]
    pub attack_free_reroll: Option<DateTime<Local>>,
    /// The silver cost to reroll before
    /// [`attack_free_reroll`](Self::attack_free_reroll).
    pub opponent_reroll_price: u64,
}

/// The resource and time cost for a fortress build or upgrade.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressCost {
    /// The time it takes to complete.
    pub time: Duration,
    /// The wood cost.
    pub wood: u64,
    /// The stone cost.
    pub stone: u64,
    /// The silver cost.
    pub silver: u64,
}

impl FortressCost {
    /// Parses a fortress cost `[time_seconds, silver, wood, stone]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<FortressCost> {
        Ok(FortressCost {
            time: Duration::from_secs(data.u64(0)?),
            silver: data.u64(1)?,
            wood: data.u64(2)?,
            stone: data.u64(3)?,
        })
    }
}

/// Information about a fortress resource (wood, stone, or experience).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressResource {
    /// Amount available to spend.
    pub current: u64,
    /// Maximum storage limit in the fortress.
    pub limit: u64,
    /// Storage limit after upgrading the Fortress building to the next level.
    pub limit_next_level: u64,
    /// Production information for this resource.
    pub production: FortressProduction,
    /// Secret storage in the Treasury (wood and stone only). Collect with
    /// [`Command::FortressGatherSecretStorage`](crate::command::Command::FortressGatherSecretStorage).
    pub secret_storage: FortressSecretStorage,
}

/// Secret storage in the Treasury, safe from enemy attacks (30% of main
/// storage).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressSecretStorage {
    /// Resources currently stored.
    pub amount: u64,
    /// Maximum capacity of the secret storage.
    pub limit: u64,
}

/// Information about the production of a resource in the fortress.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressProduction {
    /// Produced resources ready to collect. Collect with
    /// [`Command::FortressGather`](crate::command::Command::FortressGather).
    pub last_collectable: u64,
    /// Maximum storage before production halts.
    pub limit: u64,
    /// Production per hour.
    pub per_hour: u64,
    /// Production per hour at the next building level (0 for experience).
    pub per_hour_next_lvl: u64,
}

/// The type of resource available in the fortress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum, EnumCount, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressResourceType {
    /// Wood produced by the Woodcutter's Hut; used for buildings and units.
    Wood = 0,
    /// Stone produced by the Quarry; used for buildings and units.
    Stone = 1,
    /// Experience produced by the Academy; awarded to the player character.
    Experience = 2,
}

/// The type of building that can be built in the fortress.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Enum, EnumCount, EnumIter, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressBuildingType {
    /// Determines maximum building levels and resource storage.
    Fortress = 0,
    /// Reduces construction and upgrade durations.
    LaborersQuarters = 1,
    /// Produces wood over time.
    WoodcuttersHut = 2,
    /// Produces stone over time.
    Quarry = 3,
    /// Searches for attribute and black gems.
    GemMine = 4,
    /// Produces experience over time.
    Academy = 5,
    /// Trains archers for fortress defense.
    ArcheryGuild = 6,
    /// Trains soldiers for attacks and defense.
    Barracks = 7,
    /// Trains battle mages for fortress defense.
    MagesTower = 8,
    /// Expands backpack capacity and provides secret storage.
    Treasury = 9,
    /// Upgrades unit level and combat attributes.
    Smithy = 10,
    /// Provides a defensive bonus during attacks.
    Wall = 11,
}

impl FortressBuildingType {
    /// Returns the minimum Fortress level required to construct this building.
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

    /// Returns the unit type trained by this building, or `None` if the
    /// building does not train units.
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

/// Information about a fortress combat unit.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressUnit {
    /// Current unit level. Upgrade with
    /// [`Command::FortressUpgradeUnit`](crate::command::Command::FortressUpgradeUnit).
    pub level: u16,

    /// Units available for combat.
    pub count: u16,
    /// Units currently in training.
    pub in_training: u16,
    /// Maximum unit capacity (`count + in_training`).
    #[doc(alias = "capacity")]
    pub limit: u16,
    /// Recruitment progress. Recruit with
    /// [`Command::FortressBuildUnit`](crate::command::Command::FortressBuildUnit).
    #[doc(alias = "recruitment")]
    pub training: FortressAction<()>,

    /// Resource cost for the next unit level upgrade.
    pub upgrade_cost: FortressCost,
    /// The level this unit will reach when upgraded.
    pub upgrade_next_lvl: u64,
}

/// A timed fortress action (building upgrade, gem search, or unit recruitment).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressAction<T> {
    /// When this action was started, or `None` if idle.
    pub start: Option<DateTime<Local>>,
    /// When this action will finish, or `None` if idle.
    pub finish: Option<DateTime<Local>>,
    /// The cost of this action.
    pub cost: FortressCost,
    /// The target of this action, if applicable.
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

/// The type of a unit usable in the fortress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum, EnumCount, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FortressUnitType {
    /// Soldiers; used for fortress attacks and defense.
    Soldier = 0,
    /// Battle mages; used for fortress defense.
    #[doc(alias = "battle_mage")]
    Magician = 1,
    /// Archers; used for fortress defense.
    Archer = 2,
}

impl FortressUnitType {
    /// Returns the building that trains this unit type.
    #[must_use]
    pub fn training_building(self) -> FortressBuildingType {
        match self {
            FortressUnitType::Archer => FortressBuildingType::ArcheryGuild,
            FortressUnitType::Magician => FortressBuildingType::MagesTower,
            FortressUnitType::Soldier => FortressBuildingType::Barracks,
        }
    }
}

/// Information about a building in the fortress.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FortressBuilding {
    /// Current building level (0 if unbuilt). Upgrade with
    /// [`Command::FortressBuild`](crate::command::Command::FortressBuild).
    pub level: u16,
    /// Cost to upgrade to the next level.
    pub upgrade_cost: FortressCost,
}

impl Fortress {
    /// Checks if the building is currently training units or mining gems.
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

    /// Checks whether a building can currently be built or upgraded.
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
        data: &Vals<'_>,
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
    pub(crate) fn update_unit_prices(&mut self, data: &Vals<'_>) -> Result<()> {
        for (unit, cost_vals) in self.units.values_mut().zip(data.chunks(4)) {
            unit.training.cost = FortressCost::parse(&cost_vals)?;
        }
        Ok(())
    }

    /// Updates unit upgrade requirements from a 9-element response chunk.
    pub(crate) fn update_unit_upgrade_info(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<()> {
        for (unit, chunk) in self.units.values_mut().zip(data.chunks(3)) {
            unit.upgrade_next_lvl = chunk.u64(0)?;
            unit.upgrade_cost.wood = chunk.u64(1)?;
            unit.upgrade_cost.stone = chunk.u64(2)?;
        }
        Ok(())
    }

    /// Updates the fortress unit levels from a 4-element response chunk.
    pub(crate) fn update_levels(&mut self, data: &Vals<'_>) -> Result<()> {
        data.ignore(0, "might be max unit level");
        for (i, unit) in self.units.values_mut().enumerate() {
            unit.level = data.u16(1 + i)?;
        }
        Ok(())
    }

    /// Updates building upgrade costs and gem search cost.
    pub(crate) fn update_prices(&mut self, data: &Vals<'_>) -> Result<()> {
        let mut chunks = data.chunks(4);
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
        data: &Vals<'_>,
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
        data: &Vals<'_>,
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
    use super::*;

    #[test]
    fn test_fortress_cost_parse() {
        let vals = Vals::from("7200/4000/1100/333");
        let cost = FortressCost::parse(&vals).unwrap();
        vals.assert_all_read();
        assert_eq!(cost.time, Duration::from_secs(7200));
        assert_eq!(cost.silver, 4000);
        assert_eq!(cost.wood, 1100);
        assert_eq!(cost.stone, 333);
    }

    #[test]
    fn test_fortress_update_prices() {
        let raw = "7200/4000/1100/333/7200/2000/1015/308/7200/800/220/133/\
                   7200/1200/660/107/7200/6000/1467/444/765/700/7/9/765/500/\
                   41/7/765/400/20/14/765/600/61/20/7200/10000/1173/356/765/\
                   400/25/8/765/15000/30/13/9180/45000/29/9";
        let mut fortress = Fortress::default();
        let vals = Vals::from(raw);
        fortress.update_prices(&vals).unwrap();
        vals.assert_all_read();

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
                   1785018169/376/7053/1565/1785024031/111111/1459278099/\
                   1784875170/0/0/0/18/348143";
        let mut fortress = Fortress::default();
        let vals = Vals::from(raw);
        fortress.update(&vals, ServerTime::default()).unwrap();
        vals.assert_all_read();
        assert_eq!(fortress.upgrades, 376);
        assert_eq!(fortress.honor, 7053);
        assert_eq!(fortress.rank, Some(1565));
        assert_eq!(fortress.attack_target, Some(111_111));
        assert_eq!(fortress.hall_of_knights_level, 18);

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
                   1790325015/222222/1459278099/1791987830/13/1790346770/\
                   1790323370/18/521703";
        let mut fortress = Fortress::default();
        let vals = Vals::from(raw);
        fortress.update(&vals, ServerTime::default()).unwrap();
        vals.assert_all_read();
        assert_eq!(fortress.upgrades, 377);
        assert_eq!(fortress.honor, 7069);
        assert_eq!(fortress.rank, Some(1583));
        assert_eq!(fortress.attack_target, Some(222_222));
        assert_eq!(fortress.hall_of_knights_level, 18);

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
        let vals = Vals::from(raw);
        fortress.update_units(&vals, ServerTime::default()).unwrap();
        vals.assert_all_read();

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
        let vals = Vals::from(raw);
        fortress.update_levels(&vals).unwrap();
        vals.assert_all_read();

        assert_eq!(fortress.units[FortressUnitType::Soldier].level, 140);
        assert_eq!(fortress.units[FortressUnitType::Magician].level, 140);
        assert_eq!(fortress.units[FortressUnitType::Archer].level, 140);
    }

    #[test]
    fn test_fortress_update_unit_upgrade_info() {
        let raw = "145/49500000/37800000/145/132000000/10800000/145/66000000/\
                   32400000";
        let mut fortress = Fortress::default();
        let vals = Vals::from(raw);
        fortress.update_unit_upgrade_info(&vals).unwrap();
        vals.assert_all_read();

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
        let vals = Vals::from(raw);
        fortress.update_unit_prices(&vals).unwrap();
        vals.assert_all_read();

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
        let vals = Vals::from(raw);
        fortress
            .update_resources(&vals, ServerTime::default())
            .unwrap();
        vals.assert_all_read();

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
