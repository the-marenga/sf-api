use chrono::{DateTime, Local};
use log::warn;

use crate::{
    error::{Result, SFError},
    gamestate::{ServerTime, items::PetItem},
    response::Vals,
};

/// The type of a reward you can win by spinning the wheel. The wheel can be
/// upgraded, so some rewards may not always be available
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum WheelRewardType {
    /// Mushrooms currency.
    Mushrooms,
    /// Stone fortress resource.
    Stone,
    /// Large stone fortress resource.
    StoneXL,
    /// Wood fortress resource.
    Wood,
    /// Large wood fortress resource.
    WoodXL,
    /// Experience points.
    Experience,
    /// Large experience points.
    ExperienceXL,
    /// Silver coins.
    Silver,
    /// Large silver coins.
    SilverXL,
    /// Arcane splinters blacksmith resource.
    Arcane,
    /// Souls underworld resource.
    Souls,
    /// An equipment item or potion.
    Item,
    /// A pet food or egg item.
    PetItem(PetItem),
    /// An unknown wheel reward type.
    #[default]
    Unknown,
}

/// The thing you won from spinning the wheel
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WheelReward {
    /// The type of item you have won
    pub typ: WheelRewardType,
    /// The amount of the type you have won
    pub amount: i64,
}

impl WheelReward {
    /// Parses a wheel reward from a 2-value window: `[raw_typ, amount]`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        upgraded: bool,
    ) -> Result<WheelReward> {
        let raw_typ = data.i64(0)?;
        let mut amount = data.i64(1)?;
        // NOTE: I have only tested upgraded and inferred not upgraded from that
        let typ = match raw_typ {
            0 => WheelRewardType::Mushrooms,
            1 => {
                if upgraded {
                    WheelRewardType::Arcane
                } else {
                    WheelRewardType::Wood
                }
            }
            2 => WheelRewardType::ExperienceXL,
            3 => {
                if upgraded {
                    let res = WheelRewardType::PetItem(
                        PetItem::parse(amount).ok_or_else(|| {
                            SFError::ParsingError(format!(
                                "Pet wheel reward type: {amount}"
                            ))
                        })?,
                    );
                    amount = 1;
                    res
                } else {
                    WheelRewardType::Stone
                }
            }
            4 => WheelRewardType::SilverXL,
            5 => {
                // The amount does not seem to do anything.
                // 1 => equipment
                // 2 => potion
                amount = 1;
                WheelRewardType::Item
            }
            6 => WheelRewardType::WoodXL,
            7 => WheelRewardType::Experience,
            8 => WheelRewardType::StoneXL,
            9 => {
                if upgraded {
                    WheelRewardType::Souls
                } else {
                    WheelRewardType::Silver
                }
            }
            x => {
                warn!("unknown wheel reward type: {x}");
                WheelRewardType::Unknown
            }
        };
        Ok(WheelReward { typ, amount })
    }
}

/// Dr. Abawuwu's wheel
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Wheel {
    /// The amount of lucky coins you have to spin the wheel
    pub lucky_coins: u32,
    /// The amount of times you have spun the wheel today already (0 -> 20)
    pub spins_today: u8,
    /// The next time you can spin the wheel for free
    pub next_free_spin: Option<DateTime<Local>>,
    /// The result of spinning the wheel
    pub result: Option<WheelReward>,
}

impl Wheel {
    /// Updates the wheel from a 3-value `wheel` response:
    /// `? / spins_today / next_free_spin`.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        data.ignore(0, "unknown wheel state or version");
        self.spins_today = data.u8(1)?;
        self.next_free_spin = data.server_time(2, server_time)?;
        Ok(())
    }

    /// Updates the wheel result from a 2-value `wheelresult` response:
    /// `typ / amount`.
    pub(crate) fn update_result(
        &mut self,
        data: &Vals<'_>,
        upgraded: bool,
    ) -> Result<()> {
        self.result = Some(WheelReward::parse(data, upgraded)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamestate::pets::HabitatType;

    #[test]
    fn test_wheel_reward_parse() {
        let data = Vals::from("0/5");
        let reward = WheelReward::parse(&data, false).unwrap();
        assert_eq!(reward.typ, WheelRewardType::Mushrooms);
        assert_eq!(reward.amount, 5);
        data.assert_all_read();

        let data = Vals::from("1/10");
        let reward = WheelReward::parse(&data, true).unwrap();
        assert_eq!(reward.typ, WheelRewardType::Arcane);
        assert_eq!(reward.amount, 10);
        data.assert_all_read();

        let data = Vals::from("1/10");
        let reward = WheelReward::parse(&data, false).unwrap();
        assert_eq!(reward.typ, WheelRewardType::Wood);
        assert_eq!(reward.amount, 10);
        data.assert_all_read();

        let data = Vals::from("3/1");
        let reward = WheelReward::parse(&data, true).unwrap();
        assert_eq!(
            reward.typ,
            WheelRewardType::PetItem(PetItem::Egg(HabitatType::Shadow))
        );
        assert_eq!(reward.amount, 1);
        data.assert_all_read();
    }

    #[test]
    fn test_wheel_update() {
        let raw = "2/0/1762300801";
        let mut wheel = Wheel::default();
        let server_time = ServerTime::default();
        let data = Vals::from(raw);
        wheel.update(&data, server_time).unwrap();
        assert_eq!(wheel.spins_today, 0);
        assert!(wheel.next_free_spin.is_some());
        data.assert_all_read();
    }

    #[test]
    fn test_wheel_update_result() {
        let mut wheel = Wheel::default();
        let data = Vals::from("0/5");
        wheel.update_result(&data, false).unwrap();
        assert_eq!(
            wheel.result,
            Some(WheelReward {
                typ: WheelRewardType::Mushrooms,
                amount: 5,
            })
        );
        data.assert_all_read();
    }
}
