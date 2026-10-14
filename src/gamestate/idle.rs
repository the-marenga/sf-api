use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_bigint::BigInt;
use strum::EnumIter;

use super::ServerTime;
use crate::response::Vals;

/// The idle clicker game where you invest money and get runes by sacrificing
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IdleGame {
    /// The current amount of money the player
    pub current_money: BigInt,
    /// The current amount of runes the player
    pub current_runes: BigInt,
    /// The amount of times the player reset their idle game
    pub resets: u32,
    /// Runes you get, when you sacrifice
    pub sacrifice_runes: BigInt,
    /// The time at which new items will be present in the shop
    pub merchant_new_goods: DateTime<Local>,
    /// I think this the total amount of money you have sacrificed, or the last
    /// plus X, or something related to that. I am not sure and I do not
    /// really care
    pub total_sacrificed: BigInt,
    // Slightly larger than current_money, but I have no Idea why.
    _current_money_2: BigInt,
    /// Information about all the possible buildings
    pub buildings: EnumMap<IdleBuildingType, IdleBuilding>,
}

/// A single building in the idle game
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IdleBuilding {
    /// The current level of this building
    pub level: u32,
    /// The amount of money this earns on the next gather
    pub earning: BigInt,
    /// The time at which this building started it's production cycle. Note
    /// that this is likely to be in the past for quickly producing
    /// buildings
    pub cycle_start: Option<DateTime<Local>>,
    /// The time at which this building will finish it's current production
    /// cycle. Note that this is likely to be in the past for quickly producing
    /// buildings
    pub cycle_end: Option<DateTime<Local>>,
    /// Has the upgrade for this building been bought?
    pub golden: bool,
    /// The price to upgrade this building once
    pub upgrade_cost: BigInt,
    /// The price to upgrade this building 10x
    pub upgrade_cost_10x: BigInt,
    /// The price to upgrade this building 25x
    pub upgrade_cost_25x: BigInt,
    /// The price to upgrade this building 100x
    pub upgrade_cost_100x: BigInt,
}

impl IdleGame {
    /// Parses the idle game state from an `idle` response of at least 118
    /// values.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Option<IdleGame> {
        if data.len() < 118 {
            return None;
        }

        data.ignore(0..2, "idle unknown prefix");
        let resets = data.u32_or(2, 0);
        let merchant_new_goods = data.server_time_opt(63, server_time)?;
        let current_money: BigInt = data.parse_at(72, "current money").ok()?;
        let total_sacrificed: BigInt =
            data.parse_at(73, "total sacrificed").ok()?;
        let current_money_2: BigInt =
            data.parse_at(74, "current money 2").ok()?;
        let sacrifice_runes: BigInt =
            data.parse_at(75, "sacrifice runes").ok()?;
        let current_runes: BigInt = data.parse_at(76, "current runes").ok()?;
        data.ignore(77, "unknown idle field");

        let mut res = IdleGame {
            resets,
            merchant_new_goods,
            current_money,
            total_sacrificed,
            _current_money_2: current_money_2,
            sacrifice_runes,
            current_runes,
            buildings: EnumMap::default(),
        };

        for (pos, building) in
            res.buildings.as_mut_array().iter_mut().enumerate()
        {
            building.level = data.u32_or(pos + 3, 0);
            building.earning = data.parse_at(pos + 13, "earning").ok()?;
            building.cycle_start = data.server_time_opt(pos + 23, server_time);
            building.cycle_end = data.server_time_opt(pos + 33, server_time);
            building.golden = data.get_str(pos + 53) == Some("1");
            building.upgrade_cost =
                data.parse_at(pos + 78, "upgrade cost").ok()?;
            building.upgrade_cost_10x =
                data.parse_at(pos + 88, "upgrade cost 10x").ok()?;
            building.upgrade_cost_25x =
                data.parse_at(pos + 98, "upgrade cost 25x").ok()?;
            building.upgrade_cost_100x =
                data.parse_at(pos + 108, "upgrade cost 100x").ok()?;
        }

        data.ignore(43..53, "idle unused building slot");
        data.ignore(64..72, "idle unknown fields");
        if data.len() > 118 {
            data.ignore(118..data.len(), "trailing idle fields");
        }

        Some(res)
    }
}

/// The type of a building in the idle game
#[derive(Debug, Clone, Copy, Enum, EnumIter, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IdleBuildingType {
    /// The seat building.
    Seat = 1,
    /// The popcorn stand building.
    PopcornStand,
    /// The parking lot building.
    ParkingLot,
    /// The trap building.
    Trap,
    /// The drinks building.
    Drinks,
    /// The deadly trap building.
    DeadlyTrap,
    /// The VIP seat building.
    VIPSeat,
    /// The snacks building.
    Snacks,
    /// The straying monsters building.
    StrayingMonsters,
    /// The toilet building.
    Toilet,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_game_parse() {
        // Construct a synthetic 118-value idle response
        let mut parts = vec!["0".to_string(); 118];
        parts[2] = "5".to_string(); // resets
        for i in 0..10 {
            parts[3 + i] = format!("{}", (i + 1) * 10); // levels
            parts[13 + i] = format!("{}", (i + 1) * 1000); // earnings
            parts[23 + i] = "1600000000".to_string(); // cycle_start
            parts[33 + i] = "1600000100".to_string(); // cycle_end
            parts[53 + i] = if i % 2 == 0 { "1" } else { "0" }.to_string(); // golden
            parts[78 + i] = format!("{}", (i + 1) * 50); // upgrade_cost
            parts[88 + i] = format!("{}", (i + 1) * 500); // upgrade_cost_10x
            parts[98 + i] = format!("{}", (i + 1) * 1250); // upgrade_cost_25x
            parts[108 + i] = format!("{}", (i + 1) * 5000); // upgrade_cost_100x
        }
        parts[63] = "1600003600".to_string(); // merchant_new_goods
        parts[72] = "100000000000".to_string(); // current_money
        parts[73] = "5000000000".to_string(); // total_sacrificed
        parts[74] = "100000000001".to_string(); // _current_money_2
        parts[75] = "250".to_string(); // sacrifice_runes
        parts[76] = "1000".to_string(); // current_runes

        let raw = parts.join("/");
        let data = Vals::from(raw.as_str());
        let idle = IdleGame::parse(&data, ServerTime::default()).unwrap();
        data.assert_all_read();

        assert_eq!(idle.resets, 5);
        assert_eq!(idle.current_money, BigInt::from(100_000_000_000u64));
        assert_eq!(idle.sacrifice_runes, BigInt::from(250));
        assert_eq!(idle.current_runes, BigInt::from(1000));
        assert_eq!(idle.buildings[IdleBuildingType::Seat].level, 10);
        assert_eq!(
            idle.buildings[IdleBuildingType::Seat].earning,
            BigInt::from(1000)
        );
        assert!(idle.buildings[IdleBuildingType::Seat].golden);
        assert!(!idle.buildings[IdleBuildingType::PopcornStand].golden);
    }
}
