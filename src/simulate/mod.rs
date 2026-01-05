use std::sync::Arc;

use enum_map::{Enum, EnumMap};
use fastrand::Rng;
use strum::EnumIter;

use crate::{
    command::AttributeType,
    gamestate::{GameState, character::Class, items::RuneType},
    simulate::{
        class::{GenericFighter, PlayerSquad, create_context},
        damage::DamageRange,
    },
};

mod class;
mod config;
mod constants;
mod damage;

pub use class::Fightable;

#[derive(Debug, Clone)]
pub struct RawWeapon {
    pub rune_value: u32,
    pub rune_type: Option<RuneType>,
    pub damage: DamageRange,
}

#[derive(Debug, Clone)]
pub struct FightSimulationResult {
    win_ratio: f64,
    won_fights: u32,
}

#[must_use]
pub fn simulate_dungeon(
    gs: &GameState,
    monster: Monster,
    is_with_companion: bool,
    iterations: u32,
) -> FightSimulationResult {
    let PlayerSquad { player, companions } = PlayerSquad::new(gs);
    let monster = GenericFighter::from_monster(&monster);

    let mut lookup_context: Vec<(Box<dyn Fightable>, Box<dyn Fightable>)> =
        Vec::new();

    if let Some(companions) = &companions
        && is_with_companion
    {
        for companion in companions {
            let companion_ctx = create_context(companion, &monster, false);
            let monster_ctx = create_context(&monster, companion, false);
            lookup_context.push((companion_ctx, monster_ctx));
        }
    }

    let character_context = create_context(&player, &monster, false);
    let dungeon_context = create_context(&monster, &player, false);

    lookup_context.push((character_context, dungeon_context));

    simulate_fight(lookup_context, iterations)
}

//     public FightSimulationResult SimulateArenaFight(Account account,
//         Player opponent,
//         FightSimulationOptions? options = null) {
//         options ??= _defaultOptions;
//         var characterContext =
//             _fightableContextFactory.Create(account, opponent, true);
//         var enemyContext =
//             _fightableContextFactory.Create(opponent, account, true);

//         return SimulateFight([(characterContext, enemyContext)], options);
//     }

fn simulate_fight(
    mut lookup_context: Vec<(Box<dyn Fightable>, Box<dyn Fightable>)>,
    iterations: u32,
) -> FightSimulationResult {
    let mut won_fights = 0;
    for _ in 0..100_000 {
        let fight_result = perform_single_fight(&mut lookup_context);
        if fight_result == FightOutcome::SimulationBroken {
            // The fight took so long, that max iter limit kicked in. We
            // should just break here. Doing so also means wonFights
            // will be low, so this should lead to this thing here scoring
            // badly (never being picked)
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

fn perform_single_fight(
    lookup_context: &mut [(Box<dyn Fightable>, Box<dyn Fightable>)],
) -> FightOutcome {
    let mut rng = Rng::new();
    let mut leftover_health: Option<f64> = None;
    for (char_side, enemy_side) in lookup_context {
        if let Some(remaining) = leftover_health {
            enemy_side.context_mut().fighter.health = remaining;
        }

        let fight_result =
            perform_fight(char_side.as_mut(), enemy_side.as_mut(), &mut rng);
        leftover_health = Some(enemy_side.context().fighter.health);

        // FIXME: This is wrong for guild fights
        char_side.reset_state();
        enemy_side.reset_state();

        if matches!(
            fight_result,
            FightOutcome::LeftSideWin | FightOutcome::SimulationBroken
        ) {
            return fight_result;
        }
    }

    FightOutcome::RightSideWin
}

fn perform_fight<'a>(
    char_side: &'a mut dyn Fightable,
    dungeon_side: &'a mut dyn Fightable,
    rng: &mut Rng,
) -> FightOutcome {
    let char_side_starts = char_side.context().fighter.reaction
        > dungeon_side.context().fighter.reaction
        || rng.bool();

    let (attacker, defender) = if char_side_starts {
        (char_side, dungeon_side)
    } else {
        (dungeon_side, char_side)
    };

    let round = &mut 0u32;

    if attacker.attack_before_fight(defender, round, rng) {
        return OutcomeFromBool(char_side_starts);
    }

    if defender.attack_before_fight(attacker, round, rng) {
        return OutcomeFromBool(!char_side_starts);
    }

    // for sanity we limit max iters to a somewhat reasonable limit, that
    // should never be hit
    for _ in 0..1_000_000 {
        let skip_round = defender.will_skip_round(attacker, round, rng);
        if !skip_round && attacker.attack(defender, round, rng) {
            return OutcomeFromBool(char_side_starts);
        }

        let skip_round = attacker.will_skip_round(defender, round, rng);
        if !skip_round && defender.attack(attacker, round, rng) {
            return OutcomeFromBool(!char_side_starts);
        }
    }
    // TODO: Log
    FightOutcome::SimulationBroken
}

//     public static long GetHealth(ClassType @class, long constitution, int
// level,         double portal, int hpRune, bool hasEternityPotion, bool
// isCompanion) {         var healthMultiplier = @class.Config.HealthMultiplier;
//         if (isCompanion && @class == ClassType.Warrior) {
//             healthMultiplier = 6.1;
//         }
//         var health = constitution * (level + 1D);

//         health *= healthMultiplier;

//         var portalBonus = 1 + portal / 100;
//         var runeBonus = 1 + Math.Min(15, hpRune) / 100D;

//         health = Math.Ceiling(health * portalBonus);
//         health = Math.Ceiling(health * runeBonus);
//         if (hasEternityPotion) {
//             health = Math.Ceiling(health * 1.25D);
//         }

//         return (long)health;
//     }

fn OutcomeFromBool(result: bool) -> FightOutcome {
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
    pub name: Arc<str>,
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
    pub resistences: EnumMap<Element, i32>,
}
