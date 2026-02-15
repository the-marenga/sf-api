//! This simulator is based on Ernest Koguc's simulator (<https://github.com/ernest-koguc/sf-simulator>).
//! Minor changes have been done to bridge the gap of the original garbage
//! collected and object oriented design of the original into rust, but
//! otherwise, this is a direct port. As such, all credit for this
//! implementation, goes directly to him.
//! Apart from this, `HafisCZ`'s sf-tools (<https://github.com/HafisCZ/sf-tools>)
//! must also be credited. Sf-tools predates Ernest Koguc's sim and was/is used
//! as a reference, both for results and code. The dungeon data used in the
//! simulations has also been imported and converted directly from that source

#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
use std::sync::Arc;

use enum_map::{Enum, EnumMap};
use fastrand::Rng;
use fighter::InBattleFighter;
use strum::EnumIter;

pub use crate::simulate::{
    damage::DamageRange,
    fighter::Fighter,
    upgradeable::{PlayerFighterSquad, UpgradeableFighter},
};
use crate::{
    command::AttributeType, gamestate::character::Class,
    simulate::fighter::FighterIdent,
};

pub(crate) mod constants;
mod damage;
mod fighter;
mod upgradeable;

/// All the information about a weapon, that is relevant for battle simulations
#[derive(Debug, Clone)]
pub struct Weapon {
    /// The effect amount of this rune
    pub rune_value: i32,
    /// The (battle relevant) type this rune has
    pub rune_type: Option<Element>,
    /// The amount of damage this weapon does
    pub damage: DamageRange,
}

#[derive(Debug, Clone, Default)]
pub struct FightSimulationResult {
    /// The amount percentage of fights, that were won (0.0-1.0)
    pub win_ratio: f64,
    /// The amount of fights, that were won, out of the total amount of
    /// iterations
    pub won_fights: u32,
}

/// Simulates `iterations` many fights between both sides. The returned result
/// will be from the perspective of the left side. A win ratio of 1.0 will mean,
/// the left side win all fights.
///
/// Both sides are `Fighter`'s. These can be derived from `UpgradeableFighter`
/// and `Monster`.
///
/// To obtain an `UpgradeableFighter`, we create a `PlayerFighterSquad`, which
/// can then be turned into a fighter and be used in simulations.
///
/// ```rust
/// use sf_api::{simulate::{Fighter, PlayerFighterSquad, UpgradeableFighter}, gamestate::GameState};
/// let gs: GameState = GameState::default();
/// let squad = PlayerFighterSquad::new(&gs);
/// let player: UpgradeableFighter = squad.character;
/// let fighter: Fighter = Fighter::from(&player);
/// ```
///
/// We go through the `PlayerFighterSquad`, because calculating the stats for
/// player + companion is pretty much as fast, as computing the stats for just
/// the player. Similarely, we use `Fighter`, not `UpgradeableFighter`, because
/// calculating the final stats of any fighter (attributes, rune values, etc)
/// is work, that we would not want to do each time this function is invoked.
///
/// To obtain monsters, we use `current_enemy()` on Dungeons.
///
/// ```rust
/// use sf_api::{simulate::{Fighter, UpgradeableFighter}, gamestate::{dungeons::LightDungeon, GameState}};
/// let gs: GameState = GameState::default();
/// let Some(monster) = gs.dungeons.current_enemy(LightDungeon::MinesOfGloria) else { return };
/// let fighter: Fighter = Fighter::from(monster);
/// ```
#[must_use]
pub fn simulate_battle(
    left: &[Fighter],
    right: &[Fighter],
    iterations: u32,
    is_arena_battle: bool,
) -> FightSimulationResult {
    if left.is_empty() || right.is_empty() {
        return FightSimulationResult::default();
    }

    simulate_fight(left, right, iterations, is_arena_battle)
}

fn simulate_fight(
    left: &[Fighter],
    right: &[Fighter],
    iterations: u32,
    is_arena_battle: bool,
) -> FightSimulationResult {
    let mut cache = InBattleCache(Vec::new());
    let mut won_fights = 0;
    for _ in 0..iterations {
        let fight_result =
            perform_single_fight(left, right, is_arena_battle, &mut cache);
        if fight_result == FightOutcome::SimulationBroken {
            break;
        }
        if fight_result == FightOutcome::LeftSideWin {
            won_fights += 1;
        }
    }

    let win_ratio = f64::from(won_fights) / f64::from(iterations);
    FightSimulationResult {
        win_ratio,
        won_fights,
    }
}

struct InBattleCache(Vec<((FighterIdent, FighterIdent), InBattleFighter)>);

impl InBattleCache {
    pub fn get_or_insert(
        &mut self,
        this: &Fighter,
        other: &Fighter,
        is_arena_battle: bool,
    ) -> InBattleFighter {
        if self.0.len() > 10 {
            return InBattleFighter::new(this, other, is_arena_battle);
        }
        let ident = (this.ident, other.ident);
        if let Some(existing) = self.0.iter().find(|a| a.0 == ident) {
            return existing.1.clone();
        }
        let new = InBattleFighter::new(this, other, is_arena_battle);
        self.0.push((ident, new.clone()));
        new
    }
}

fn perform_single_fight(
    left: &[Fighter],
    right: &[Fighter],
    is_arena_battle: bool,
    cache: &mut InBattleCache,
) -> FightOutcome {
    let mut rng = Rng::new();

    let mut left_side = left.iter().peekable();
    let mut left_in_battle: Option<InBattleFighter> = None;

    let mut right_side = right.iter().peekable();
    let mut right_in_battle: Option<InBattleFighter> = None;

    for _ in 0..500 {
        let Some(left) = left_side.peek_mut() else {
            return FightOutcome::RightSideWin;
        };
        let Some(right) = right_side.peek_mut() else {
            return FightOutcome::LeftSideWin;
        };

        let (left_fighter, right_fighter) =
            match (&mut left_in_battle, &mut right_in_battle) {
                (Some(left), Some(right)) => {
                    // Battle still ongoing between the same two opponents
                    (left, right)
                }
                (None, None) => {
                    // Battle just started
                    (
                        left_in_battle.insert(cache.get_or_insert(
                            left,
                            right,
                            is_arena_battle,
                        )),
                        right_in_battle.insert(cache.get_or_insert(
                            right,
                            left,
                            is_arena_battle,
                        )),
                    )
                }
                (None, Some(r)) => {
                    r.update_opponent(right, left, is_arena_battle);
                    (
                        left_in_battle.insert(cache.get_or_insert(
                            left,
                            right,
                            is_arena_battle,
                        )),
                        r,
                    )
                }
                (Some(l), None) => {
                    l.update_opponent(left, right, is_arena_battle);
                    (
                        l,
                        right_in_battle.insert(cache.get_or_insert(
                            right,
                            left,
                            is_arena_battle,
                        )),
                    )
                }
            };

        // println!("{left_fighter:#?}");
        // println!("{right_fighter:#?}");

        let res = perform_fight(left_fighter, right_fighter, &mut rng);

        match res {
            FightOutcome::LeftSideWin => {
                right_side.next();
                right_in_battle = None;
            }
            FightOutcome::RightSideWin => {
                left_side.next();
                left_in_battle = None;
            }
            FightOutcome::SimulationBroken => {
                return FightOutcome::SimulationBroken;
            }
        }
    }

    FightOutcome::SimulationBroken
}

fn perform_fight<'a>(
    char_side: &'a mut InBattleFighter,
    dungeon_side: &'a mut InBattleFighter,
    rng: &mut Rng,
) -> FightOutcome {
    let char_side_starts =
        char_side.reaction > dungeon_side.reaction || rng.bool();

    let (attacker, defender) = if char_side_starts {
        (char_side, dungeon_side)
    } else {
        (dungeon_side, char_side)
    };

    let round = &mut 0u32;

    if attacker.attack_before_fight(defender, round, rng) {
        return outcome_from_bool(char_side_starts);
    }

    if defender.attack_before_fight(attacker, round, rng) {
        return outcome_from_bool(!char_side_starts);
    }

    // for sanity we limit max iters to a somewhat reasonable limit, that
    // should never be hit
    for _ in 0..1_000_000 {
        let skip_round =
            defender.will_skips_opponent_round(attacker, round, rng);
        if !skip_round && attacker.attack(defender, round, rng) {
            return outcome_from_bool(char_side_starts);
        }

        let skip_round =
            attacker.will_skips_opponent_round(defender, round, rng);
        if !skip_round && defender.attack(attacker, round, rng) {
            return outcome_from_bool(!char_side_starts);
        }
    }
    // TODO: Log
    FightOutcome::SimulationBroken
}

fn outcome_from_bool(result: bool) -> FightOutcome {
    if result {
        FightOutcome::LeftSideWin
    } else {
        FightOutcome::RightSideWin
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FightOutcome {
    LeftSideWin,
    RightSideWin,
    SimulationBroken,
}

#[derive(Debug, Clone, Copy, Enum, EnumIter, Hash, PartialEq, Eq)]
pub enum Element {
    Fire,
    Cold,
    Lightning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monster {
    pub name: &'static str,
    pub level: u16,
    pub class: Class,
    pub attributes: EnumMap<AttributeType, u32>,
    pub hp: u64,
    pub min_dmg: u32,
    pub max_dmg: u32,
    pub armor: u32,
    pub runes: Option<MonsterRunes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonsterRunes {
    pub damage_type: Element,
    pub damage: i32,
    pub resistances: EnumMap<Element, i32>,
}
