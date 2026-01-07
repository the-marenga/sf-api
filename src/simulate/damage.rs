use fastrand::Rng;

use crate::{
    gamestate::character::Class,
    simulate::{Weapon, fighter::Fighter},
};

#[derive(Debug, Clone, Copy, Default)]
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
    attacker: &Fighter,
    target: &Fighter,
    is_secondary: bool,
) -> DamageRange {
    let weapon = match is_secondary {
        true => &attacker.second_weapon,
        false => &attacker.first_weapon,
    }
    .as_ref();
    let mut damage = get_base_damge(weapon, attacker, is_secondary);

    damage *= 1.0 + attacker.portal_dmg_bonus / 100.0;

    apply_attributes_bonus(attacker, target, &mut damage);
    apply_rune_bonus(weapon, target, &mut damage);

    // Armor reduction
    damage *= 1.0 - calculate_damage_reduction(attacker, target);
    // Class multiplier
    damage *= calculate_damage_multiplier(attacker, target);

    damage
}

fn apply_rune_bonus(
    weapon: Option<&Weapon>,
    target: &Fighter,
    damage: &mut DamageRange,
) {
    let Some(weapon) = weapon else {
        return;
    };
    let Some(element) = weapon.rune_type else {
        return;
    };

    let enemy_rune_resistence = 75.min(target.resistances[element]);

    let mut rune_bonus = f64::from(weapon.rune_value) / 100.0;
    rune_bonus *= (100.0 - f64::from(enemy_rune_resistence)) / 100.0;
    rune_bonus += 1.0;

    *damage *= rune_bonus;
}

fn apply_attributes_bonus(
    attacker: &Fighter,
    target: &Fighter,
    damage: &mut DamageRange,
) {
    let main_attribute = attacker.class.main_attribute();
    let mut attribute = attacker.attributes[main_attribute] / 2;
    attribute = attribute.max(
        attacker.attributes[main_attribute]
            .saturating_sub(target.attributes[main_attribute] / 2),
    );
    let attribute_bonus = 1.0 + f64::from(attribute) / 10.0;
    *damage *= attribute_bonus;
}

pub fn calculate_hit_damage(
    damage: &DamageRange,
    round: u32,
    crit_chance: f64,
    crit_multiplier: f64,
    rng: &mut Rng,
) -> f64 {
    let base_damage = rng.f64() * (1.0 + damage.max - damage.min) + damage.min;
    let mut dmg = base_damage * (1.0 + (f64::from(round) - 1.0) * (1.0 / 6.0));

    if rng.f64() < crit_chance {
        dmg *= crit_multiplier;
    }
    dmg
}

pub fn calculate_swoop_damage(attacker: &Fighter, target: &Fighter) -> f64 {
    let dmg_multiplier = calculate_damage_multiplier(attacker, target);
    let base_dmg_multiplier = Class::Druid.damage_multiplier();
    let class_specific_dmg_multiplier = dmg_multiplier / base_dmg_multiplier;

    (dmg_multiplier / class_specific_dmg_multiplier + 0.8)
        * class_specific_dmg_multiplier
        / dmg_multiplier
}

fn get_base_damge(
    weapon: Option<&Weapon>,
    attacker: &Fighter,
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

fn get_hand_damage(attacker: &Fighter, is_secondary: bool) -> DamageRange {
    if attacker.level <= 10 {
        return DamageRange { min: 1.0, max: 2.0 };
    }

    let mut multiplier = 0.7;
    if attacker.class == Class::Assassin {
        multiplier = if is_secondary { 1.25 } else { 0.875 };
    }

    let weapon_multi = attacker.class.weapon_multiplier();
    let damage: f64 =
        multiplier * (f64::from(attacker.level) - 9.0) * weapon_multi;
    let min = 1.0f64.max((damage * 2.0 / 3.0).ceil());
    let max = 2.0f64.max((damage * 4.0 / 3.0).round());

    DamageRange { min, max }
}

pub fn calculate_fire_ball_damage(attacker: &Fighter, target: &Fighter) -> f64 {
    if target.class == Class::Mage {
        return 0.0;
    }

    let multiplier = target.class.health_multiplier(target.is_companion);

    let dmg = (multiplier * 0.05 * attacker.max_health).ceil();
    (target.max_health / 3.0).ceil().min(dmg)
}

pub fn calculate_damage_reduction(attacker: &Fighter, target: &Fighter) -> f64 {
    // Mage negates enemy armor
    if attacker.class == Class::Mage {
        return 0.0;
    }

    if target.armor == 0 {
        return 0.0;
    }

    let max_dmg_reduction = target.class.max_armor_reduction();

    let damage_reduction = target.class.armor_multiplier()
        * f64::from(target.armor)
        / f64::from(attacker.level)
        / 100.0;

    damage_reduction.min(f64::from(max_dmg_reduction) / 100.0)
}

pub fn calculate_damage_multiplier(
    attacker: &Fighter,
    target: &Fighter,
) -> f64 {
    let base_multi = attacker.class.damage_multiplier();
    match (attacker.class, target.class) {
        (Class::Mage, Class::Paladin) |
        // TODO: Is this right?
        (Class::Paladin, Class::Mage) => base_multi * 1.5,
        (Class::Druid, Class::Mage) => base_multi * 4.0 / 3.0,
        (Class::Druid, Class::DemonHunter) => base_multi * 1.15,
        (Class::Bard, Class::PlagueDoctor) => base_multi * 1.05,
        (Class::Necromancer, Class::DemonHunter) => base_multi + 0.1,
        (Class::PlagueDoctor, Class::DemonHunter) => base_multi * 1.06,
        (_, _) => base_multi,
    }
}
