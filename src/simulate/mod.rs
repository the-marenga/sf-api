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
    command::AttributeType,
    gamestate::{GameState, character::Class, dungeons::Dungeon},
    simulate::fighter::FighterIdent,
};

pub(crate) mod constants;
mod damage;
mod fighter;
mod upgradeable;

#[derive(Debug, Clone)]
pub struct Weapon {
    pub rune_value: i32,
    pub rune_type: Option<Element>,
    pub damage: DamageRange,
}

#[derive(Debug, Clone, Default)]
pub struct FightSimulationResult {
    pub win_ratio: f64,
    pub won_fights: u32,
}

pub fn simulate_dungeon(
    gs: &GameState,
    dungeon: impl Into<Dungeon> + Copy,
    iterations: u32,
) -> Option<FightSimulationResult> {
    let PlayerFighterSquad {
        character,
        companions,
    } = PlayerFighterSquad::new(gs);
    let dungeon = dungeon.into();
    let mut player_side = if dungeon.is_with_companions() {
        companions
            .map(|a| a.values().map(Fighter::from).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };
    player_side.push(Fighter::from(&character));

    let monster = gs.dungeons.current_enemy(dungeon)?;
    let monster = Fighter::from(monster);

    Some(simulate_battle(&player_side, &[monster], iterations, false))
}

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
