use chrono::{DateTime, Local};
use num_bigint::{BigInt, ToBigInt};
use strum::EnumCount;

use super::ServerTime;

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IdleGame {
    pub current_money: BigInt,
    pub current_runes: BigInt,
    pub resets: u32,
    /// Runes you get, when you sacrifice
    pub sacrifice_runes: BigInt,
    /// The time at which new items will be present in the shop
    pub merchant_new_goods: DateTime<Local>,
    /// I think this the total amount of money you have sacrificed, or the last
    /// + X, or something related  to that. I am not sure and I do not really
    /// care
    pub total_sacrificed: BigInt,
    // Slightly larger than current_money, but I have no Idea why.
    _current_money_2: BigInt,
    pub buildings: [IdleBuilding; IdleBuildingType::COUNT],
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IdleBuilding {
    pub level: u32,
    pub earning: BigInt,
    pub next_gather: Option<DateTime<Local>>,
    pub next_next_gather: Option<DateTime<Local>>,
    /// Has the upgrade for this building been bought?
    pub golden: bool,
    pub upgrade_cost: BigInt,
    pub upgrade_cost_10x: BigInt,
    pub upgrade_cost_25x: BigInt,
    pub upgrade_cost_100x: BigInt,
}

impl IdleGame {
    pub(crate) fn parse_idle_game(
        data: Vec<BigInt>,
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
            buildings: Default::default(),
        };

        // We could do this above.. but I do not want to..
        for (pos, building) in res.buildings.iter_mut().enumerate() {
            building.level = data.get(pos + 3)?.try_into().ok()?;
            building.earning = data.get(pos + 13)?.clone();
            building.next_gather = server_time.convert_to_local(
                data.get(pos + 23)?.try_into().ok()?,
                "next gather time",
            );
            building.next_next_gather = server_time.convert_to_local(
                data.get(pos + 33)?.try_into().ok()?,
                "next next gather time",
            );
            building.golden = data.get(pos + 53)? == &1.to_bigint().unwrap();
            building.upgrade_cost = data.get(pos + 78)?.clone();
            building.upgrade_cost_10x = data.get(pos + 88)?.clone();
            building.upgrade_cost_25x = data.get(pos + 98)?.clone();
            building.upgrade_cost_100x = data.get(pos + 108)?.clone();
        }
        Some(res)
    }
}

#[derive(Debug, Clone, Copy, strum::EnumCount)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
