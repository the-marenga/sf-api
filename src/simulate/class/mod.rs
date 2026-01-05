#![allow(unused)]
mod asassin;
mod bard;
mod basic;
mod battle_mage;
mod berserker;
mod demon_hunter;
mod druid;
mod necro;
mod paladin;
mod plague_doctor;

pub use asassin::*;
pub use bard::*;
pub use basic::*;
pub use battle_mage::*;
pub use berserker::*;
pub use demon_hunter::*;
pub use druid::*;
use enum_map::EnumMap;
pub use necro::*;
pub use paladin::*;
pub use plague_doctor::*;

use crate::{
    command::AttributeType, gamestate::{GameState, character::Class}, simulate::{Monster, RawWeapon, damage::*}
};

pub trait Fightable {
    fn context(&self) -> &FightContext;
    fn context_mut(&mut self) -> &mut FightContext;

    fn attack(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        self.attack_generic(target, round, rng)
    }

    fn is_mage(&self) -> bool {
        self.context().fighter.class == Class::Mage
    }

    fn attack_generic(
        &mut self,
        target: &mut dyn Fightable,
        round: &mut u32,
        rng: &mut fastrand::Rng,
    ) -> bool {
        *round += 1;

        if !target.will_take_attack(rng) {
            return false;
        }

        let dmg = self.calculate_basic_hit_damage(*round, rng);
        target.take_attack(dmg, round, rng)
    }

    fn attack_before_fight(
        &mut self,
        _target: &mut dyn Fightable,
        _round: &mut u32,
        _rng: &mut fastrand::Rng,
    ) -> bool {
        false
    }
    fn will_skip_round(
        &mut self,
        _target: &mut dyn Fightable,
        _round: &mut u32,
        _rng: &mut fastrand::Rng,
    ) -> bool {
        false
    }

    fn reset_state(&mut self) {
        self.reset_health();
    }

    fn reset_health(&mut self) {
        self.context_mut().fighter.health = self.context_mut().max_health;
    }

    fn take_attack(
        &mut self,
        damage: f64,
        _round: &mut u32,
        _rng: &mut fastrand::Rng,
    ) -> bool {
        let health = &mut self.context_mut().fighter.health;
        *health -= damage;
        *health <= 0.0
    }

    fn will_take_attack(&mut self, _rng: &mut fastrand::Rng) -> bool {
        true
    }

    /// The damage a single normal hit with the main weapon does this round
    fn calculate_basic_hit_damage(
        &self,
        round: u32,
        rng: &mut fastrand::Rng,
    ) -> f64 {
        self.calculate_hit_damage(
            &self.context().damage,
            round,
            self.context().crit_chance,
            self.context().crit_multiplier,
            rng,
        )
    }

    fn calculate_hit_damage(
        &self,
        damage: &DamageRange,
        round: u32,
        crit_chance: f64,
        crit_multiplier: f64,
        rng: &mut fastrand::Rng,
    ) -> f64 {
        let base_damage = rng.f64() * (damage.max - damage.min) + damage.min;
        let mut dmg =
            base_damage * (1.0 + (f64::from(round) - 1.0) * (1.0 / 6.0));

        if rng.f64() < crit_chance {
            dmg *= crit_multiplier;
        }
        dmg
    }
}

#[derive(Debug, Clone)]
pub struct GenericFighter {
    pub class: Class,
    pub level: u32,
    pub attributes: EnumMap<AttributeType, u32>,
    pub health: f64,
    pub armor: i32,
    pub first_weapon: Option<RawWeapon>,
    pub second_weapon: Option<RawWeapon>,
    pub reaction: i32,
    pub crit_multiplier: f64,
    pub lightning_resistance: u32,
    pub fire_resistance: u32,
    pub cold_resistance: u32,
    pub guild_portal: f64,
    pub is_companion: bool,
    pub gladiator: i32,
}

impl GenericFighter {
    pub fn from_monster(monster: &Monster) -> Self {
        // monster.attributes;
        todo!()
    }
}

pub struct PlayerSquad {
    pub player: GenericFighter,
    pub companions: Option<[GenericFighter; 3]>,
}

impl PlayerSquad {
    pub fn new(value: &GameState) -> PlayerSquad {
        todo!()
    }
}

#[derive(Debug)]
pub struct FightContext {
    pub fighter: GenericFighter,

    pub max_health: f64,
    pub damage: DamageRange,
    pub crit_chance: f64,
    pub crit_multiplier: f64,

    pub opponent_is_mage: bool,
}

pub fn create_context(
    main: &GenericFighter,
    opponent: &GenericFighter,
    reduce_gladiator: bool,
) -> Box<dyn Fightable> {
    let context = create_generic_context(main, opponent, reduce_gladiator);

    match main.class {
        Class::Warrior if main.is_companion => {
            Box::new(WarriorFightContext::new(context, 0))
        }
        Class::Warrior => Box::new(WarriorFightContext::new(context, 25)),
        Class::Mage => Box::new(MageFightContext::new(context)),
        Class::Scout => Box::new(ScoutFightContext::new(context)),
        Class::Assassin => {
            let range = calculate_damage(
                main.second_weapon.as_ref(),
                main,
                opponent,
                true,
            );
            Box::new(AssassinFightContext::new(context, range))
        }
        Class::BattleMage => Box::new(BattleMageFightContext::new(
            context,
            calculate_fire_ball_damage(main, opponent),
        )),
        Class::Berserker => Box::new(BerserkerFightContext::new(context)),
        Class::DemonHunter => Box::new(DemonHunterFightContext::new(context)),
        Class::Druid => {
            let rage_crit_chance =
                calculate_crit_chance(main, opponent, 0.75, 0.1);
            Box::new(DruidFightContext::new(context, rage_crit_chance))
        }
        Class::Bard => Box::new(BardFightContext::new(context)),
        Class::Necromancer => {
            let dmg_multi = calculate_damage_multiplier(main, opponent);
            Box::new(NecromancerFightContext::new(context, dmg_multi))
        }
        Class::Paladin => {
            let damage_reduction = calculate_damage_reduction(opponent, main);
            Box::new(PaladinFightContext::new(context, damage_reduction))
        }
        Class::PlagueDoctor => {
            let base_multi = calculate_damage_multiplier(main, opponent);
            Box::new(PlagueDoctorFightContext::new(context, base_multi))
        }
    }
}

fn calculate_crit_chance(
    main: &GenericFighter,
    opponent: &GenericFighter,
    cap: f64,
    crit_bonus: f64,
) -> f64 {
    (main.attributes[AttributeType::Luck] as f64 * 5.0 / (f64::from(opponent.level) * 2.0) / 100.0
        + crit_bonus)
        .min(cap)
}

fn create_generic_context(
    main: &GenericFighter,
    opponent: &GenericFighter,
    reduce_gladiator: bool,
) -> FightContext {
    let range =
        calculate_damage(main.first_weapon.as_ref(), main, opponent, false);
    let mut crit_multiplier = main.crit_multiplier;
    if (reduce_gladiator) {
        crit_multiplier -=
            f64::from(main.gladiator.min(opponent.gladiator)) * 0.11;
    }
    let crit_chance = calculate_crit_chance(main, opponent, 0.5, 0.0);

    FightContext {
        fighter: main.clone(),
        max_health: main.health,
        damage: range,
        crit_chance,
        crit_multiplier,
        opponent_is_mage: opponent.class == Class::Mage,
    }
}
