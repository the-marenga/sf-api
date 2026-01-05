use num_traits::real::Real;

use crate::{
    command::AttributeType,
    gamestate::{character::Class, items::RuneType},
    simulate::{RawWeapon, class::GenericFighter},
};

#[derive(Debug, Clone, Copy)]
pub struct DamageRange {
    pub min: f64,
    pub max: f64,
}

impl std::ops::Mul<f64> for DamageRange {
    type Output = DamageRange;

    fn mul(self, rhs: f64) -> DamageRange {
        DamageRange {
            min: self.min * rhs,
            max: self.max * rhs,
        }
    }
}

impl std::ops::MulAssign<f64> for DamageRange {
    fn mul_assign(&mut self, rhs: f64) {
        self.min *= rhs;
        self.max *= rhs;
    }
}

pub fn calculate_damage(
    weapon: Option<&RawWeapon>,
    attacker: &GenericFighter,
    target: &GenericFighter,
    is_secondary: bool,
) -> DamageRange {
    let mut damage = get_base_damge(weapon, attacker, is_secondary);

    damage *= 1.0 + attacker.guild_portal / 100.0;

    proc_attributes_bonus(attacker, target, &mut damage);
    proc_rune_bonus(weapon, target, &mut damage);
    proc_armor_damage_reduction(attacker, target, &mut damage);
    proc_class_modifiers(attacker, target, &mut damage);

    damage
}

pub fn proc_class_modifiers(
    attacker: &GenericFighter,
    target: &GenericFighter,
    damage: &mut DamageRange,
) {
    *damage *= calculate_damage_multiplier(attacker, target);
}

fn proc_armor_damage_reduction(
    attacker: &GenericFighter,
    target: &GenericFighter,
    damage: &mut DamageRange,
) {
    *damage *= 1.0 - calculate_damage_reduction(attacker, target);
}

fn proc_rune_bonus(
    weapon: Option<&RawWeapon>,
    target: &GenericFighter,
    damage: &mut DamageRange,
) {
    let Some(weapon) = weapon else {
        return;
    };
    let Some(rune_type) = weapon.rune_type else {
        return;
    };
    let enemy_rune_resistance = match rune_type {
        RuneType::LightningDamage => 75.min(target.lightning_resistance),
        RuneType::ColdDamage => 75.min(target.cold_resistance),
        RuneType::FireDamage => 75.min(target.fire_resistance),
        _ => 0,
    };

    let mut rune_bonus = f64::from(weapon.rune_value) / 100.0;
    rune_bonus *= (100.0 - f64::from(enemy_rune_resistance)) / 100.0;
    rune_bonus += 1.0;

    *damage *= rune_bonus;
}

fn proc_attributes_bonus(
    attacker: &GenericFighter,
    target: &GenericFighter,
    damage: &mut DamageRange,
) {
    let main_attribute = attacker.class.get_config().main_attribute;
    let attribute = (f64::from(attacker.attributes[main_attribute]) / 2.0).max(
        f64::from(
            attacker.attributes[main_attribute]
                - target.attributes[main_attribute],
        ) / 2.0,
    );

    let attribute_bonus = 1.0 + attribute / 10.0;

    *damage *= attribute_bonus;
}

fn get_base_damge(
    weapon: Option<&RawWeapon>,
    attacker: &GenericFighter,
    is_secondary: bool,
) -> DamageRange {
    let hand_damage = get_hand_damage(attacker, is_secondary);

    let Some(weapon) = weapon else {
        return hand_damage;
    };

    if weapon.damage.min < hand_damage.min
        && weapon.damage.max < hand_damage.max
    {
        return hand_damage;
    }

    weapon.damage
}

fn get_hand_damage(
    attacker: &GenericFighter,
    is_secondary: bool,
) -> DamageRange {
    if attacker.level <= 10 {
        return DamageRange { min: 1.0, max: 2.0 };
    }

    let mut multiplier = 0.7;
    if attacker.class == Class::Assassin {
        multiplier = if is_secondary { 1.25 } else { 0.875 };
    }

    let weapon_multi = attacker.class.get_config().weapon_multiplier;
    let damage: f64 =
        multiplier * (f64::from(attacker.level) - 9.0) * weapon_multi;
    let min = 1.0.max((damage * 2.0 / 3.0).ceil());
    let max = 2.0.max((damage * 4.0 / 3.0).round());

    DamageRange { min, max }
}

pub fn calculate_fire_ball_damage(
    attacker: &GenericFighter,
    target: &GenericFighter,
) -> f64 {
    if target.class == Class::Mage {
        return 0.0;
    }

    let multiplier = target.class.get_config().health_multiplier;

    let dmg = (multiplier * 0.05 * attacker.health).ceil();
    (target.health / 3.0).ceil().min(dmg)
}

pub fn calculate_damage_reduction(
    attacker: &GenericFighter,
    target: &GenericFighter,
) -> f64 {
    // Mage negates enemy armor
    if attacker.class == Class::Mage {
        return 0.0;
    }

    if target.armor <= 0 {
        return 0.0;
    }

    let class_config = target.class.get_config();
    let max_dmg_reduction = class_config.max_armor_reduction;

    let damage_reduction = class_config.armor_multiplier
        * f64::from(target.armor)
        / f64::from(attacker.armor)
        / 100.0;

    damage_reduction.min(max_dmg_reduction)
}

pub fn calculate_damage_multiplier(
    attacker: &GenericFighter,
    target: &GenericFighter,
) -> f64 {
    let base_multi = attacker.class.get_config().damage_multiplier;
    match (attacker.class, target.class) {
        (Class::Mage, Class::Paladin) => base_multi * 1.5,
        (Class::Druid, Class::Mage) => base_multi * 4.0 / 3.0,
        (Class::Druid, Class::DemonHunter) => base_multi * 1.15,
        (Class::Bard, Class::PlagueDoctor) => base_multi * 1.05,
        (Class::Necromancer, Class::DemonHunter) => base_multi + 0.1,
        (Class::Paladin, Class::Mage) => base_multi * 1.5,
        (Class::PlagueDoctor, Class::DemonHunter) => base_multi * 1.065,
        (_, _) => base_multi,
    }
}
