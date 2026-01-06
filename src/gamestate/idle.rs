#![allow(clippy::module_name_repetitions)]
use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_bigint::BigInt;
use strum::EnumIter;

use super::ServerTime;

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
    /// The time at which this building will gather resources
    pub next_gather: Option<DateTime<Local>>,
    /// The next time at which this building will gather resources
    pub next_next_gather: Option<DateTime<Local>>,
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
    pub(crate) fn parse_idle_game(
        data: &[BigInt],
        server_time: ServerTime,
    ) -> Option<IdleGame> {
        if data.len() < 118 {
            return None;
        }

        let mut res = IdleGame {
            resets: data.get(2)?.try_into().ok()?,
            merchant_new_goods: server_time.convert_to_local(
                data.get(63)?.try_into().ok()?,
                "trader time",
            )?,
            current_money: data.get(72)?.clone(),
            total_sacrificed: data.get(73)?.clone(),
            _current_money_2: data.get(74)?.clone(),
            sacrifice_runes: data.get(75)?.clone(),
            current_runes: data.get(76)?.clone(),
            buildings: EnumMap::default(),
        };

        // We could do this above.. but I do not want to..
        for (pos, building) in
            res.buildings.as_mut_array().iter_mut().enumerate()
        {
            building.level = data.get(pos + 3)?.try_into().ok()?;
            building.earning.clone_from(data.get(pos + 13)?);
            building.next_gather = server_time.convert_to_local(
                data.get(pos + 23)?.try_into().ok()?,
                "next gather time",
            );
            building.next_next_gather = server_time.convert_to_local(
                data.get(pos + 33)?.try_into().ok()?,
                "next next gather time",
            );
            building.golden = data.get(pos + 53)? == &1.into();
            building.upgrade_cost.clone_from(data.get(pos + 78)?);
            building.upgrade_cost_10x.clone_from(data.get(pos + 88)?);
            building.upgrade_cost_25x.clone_from(data.get(pos + 98)?);
            building.upgrade_cost_100x.clone_from(data.get(pos + 108)?);
        }
        Some(res)
    }
}

/// The type of a building in the idle game
#[derive(Debug, Clone, Copy, Enum, EnumIter, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum IdleBuildingType {
    Seat = 1,
    PopcornStand,
    ParkingLot,
    Trap,
    Drinks,
    DeadlyTrap,
    VIPSeat,
    Snacks,
    StrayingMonsters,
    Toilet,
}
