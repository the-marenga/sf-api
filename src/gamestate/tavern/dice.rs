use chrono::{DateTime, Local};

use crate::{
    command::{DiceReward, DiceType},
    error::{Result, SFError},
    gamestate::ServerTime,
    response::Vals,
};

/// Information about the current state of the dice game
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiceGame {
    /// The amount of dice games you can still play today
    pub remaining: u8,
    /// The next free dice game can be played at this point in time
    pub next_free: Option<DateTime<Local>>,
    /// These are the dices, that are laying on the table after the first
    /// round. The ones you can select to keep from
    pub current_dice: Vec<DiceType>,
    /// Whatever we won in the dice game
    pub reward: Option<DiceReward>,
}

impl DiceGame {
    /// Updates the next free game time and remaining games from a 2-value
    /// `dice` response: `[next_free, remaining]`.
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        self.next_free = data.server_time_opt(0, server_time);
        self.remaining = data.u8_or(1, 0);
    }

    /// Updates the dice currently lying on the table from a `dicestatus`
    /// response.
    pub(crate) fn update_status(&mut self, data: &Vals<'_>) {
        self.current_dice = data
            .iter()
            .filter_map(|x| {
                x.parse::<u8>()
                    .ok()
                    .and_then(num_traits::FromPrimitive::from_u8)
            })
            .collect();
    }

    /// Updates the dice game reward from a 2-value `dicereward` response:
    /// `[win_typ, amount]`.
    pub(crate) fn update_reward(&mut self, data: &Vals<'_>) -> Result<()> {
        let win_typ =
            data.enum_map(0, |a| a.saturating_sub(1)).ok_or_else(|| {
                SFError::ParsingErrorV2(format!(
                    "Invalid dice reward type: {}",
                    data.str_or(0, "")
                ))
            })?;
        let amount = data.u32(1)?;
        self.reward = Some(DiceReward { win_typ, amount });
        Ok(())
    }
}

/// The amount, that you either won or lost gambling. If the value is negative,
/// you lost
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GambleResult {
    /// Silver change from gambling (positive for win, negative for loss).
    SilverChange(i64),
    /// Mushroom change from gambling (positive for win, negative for loss).
    MushroomChange(i32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dice_game_updates() {
        let mut game = DiceGame::default();

        let dice_raw = "1600000000/5";
        let dice_data = Vals::from(dice_raw);
        game.update(&dice_data, ServerTime::default());
        assert!(game.next_free.is_some());
        assert_eq!(game.remaining, 5);
        assert!(
            dice_data.unaccessed_indices().is_empty(),
            "unread values in {dice_data}"
        );

        let status_raw = "1/2/3/4/5";
        let status_data = Vals::from(status_raw);
        game.update_status(&status_data);
        assert_eq!(
            game.current_dice.as_slice(),
            [
                DiceType::Silver,
                DiceType::Stone,
                DiceType::Wood,
                DiceType::Souls,
                DiceType::Arcane,
            ]
        );
        assert!(
            status_data.unaccessed_indices().is_empty(),
            "unread values in {status_data}"
        );

        let reward_raw = "2/50000";
        let reward_data = Vals::from(reward_raw);
        game.update_reward(&reward_data).unwrap();
        assert_eq!(
            game.reward,
            Some(DiceReward {
                win_typ: DiceType::Silver,
                amount: 50000,
            })
        );
        assert!(
            reward_data.unaccessed_indices().is_empty(),
            "unread values in {reward_data}"
        );
    }
}
