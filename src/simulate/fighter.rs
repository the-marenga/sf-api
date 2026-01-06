use enum_map::EnumMap;
use fastrand::Rng;

use crate::{
    command::AttributeType,
    gamestate::character::Class,
    simulate::{
        Element, Monster, Weapon,
        damage::{DamageRange, calculate_damage, calculate_hit_damage, *},
    },
};

#[derive(Debug)]
pub struct InBattleFighter {
    pub fighter: Fighter,

    pub max_health: f64,
    pub damage: DamageRange,
    pub crit_chance: f64,
    pub crit_multiplier: f64,

    pub opponent_is_mage: bool,

    pub class_data: ClassData,
}

#[derive(Debug, Clone)]
pub enum ClassData {
    Warrior {
        block_chance: i32,
    },
    Mage,
    Scout,
    Assassin {
        secondary_damage: DamageRange,
    },
    BattleMage {
        fireball_dmg: f64,
        used_fireball: bool,
    },
    Berserker {
        chain_attack_counter: u32,
    },
    DemonHunter {
        revive_count: u8,
    },
    Druid {
        rage_crit_chance: f64,
        is_in_bear_form: bool,
        has_just_dodged: bool,
        swoop_chance: f64,
    },
    Bard {
        melody_length: i32,
        next_melody_round: i32,
        melody_dmg_multiplier: f64,
    },
    Necromancer {
        base_damage_multi: f64,
        minion_type: NecromancerMinionType,
        minion_rounds: i32,
        skeleton_revives: i32,
    },
    Paladin {
        initial_armor_reduction: f64,
        stance: PaladinStance,
    },
    PlagueDoctor {
        poison_round: usize,
        poison_dmg_multipliers: [f64; 3],
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NecromancerMinionType {
    None,
    Skeleton,
    Hound,
    Golem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaladinStance {
    Initial,
    Defensive,
    Offensive,
}

impl PaladinStance {
    pub(crate) fn damage_multiplier(self) -> f64 {
        match self {
            PaladinStance::Initial => 1.0,
            PaladinStance::Defensive => 1.0 / 0.833 * 0.568,
            PaladinStance::Offensive => 1.0 / 0.833 * 1.253,
        }
    }

    pub(crate) fn block_chance(self) -> i32 {
        match self {
            PaladinStance::Initial => 30,
            PaladinStance::Defensive => 50,
            PaladinStance::Offensive => 25,
        }
    }
}

pub(crate) fn calculate_crit_chance(
    main: &Fighter,
    opponent: &Fighter,
    cap: f64,
    crit_bonus: f64,
) -> f64 {
    (f64::from(main.attributes[AttributeType::Luck]) * 5.0
        / (f64::from(opponent.level) * 2.0)
        / 100.0
        + crit_bonus)
        .min(cap)
}

impl InBattleFighter {
    pub fn is_mage(&self) -> bool {
        self.fighter.class == Class::Mage
    }

    pub fn update_opponent(
        &mut self,
        opponent: &Fighter,
        reduce_gladiator: bool,
    ) {
        let main = &self.fighter;
        self.damage =
            calculate_damage(main.first_weapon.as_ref(), main, opponent, false);
        let mut crit_multiplier = main.crit_multiplier;
        if reduce_gladiator {
            crit_multiplier -=
                f64::from(main.gladiator.min(opponent.gladiator)) * 0.11;
        }
        self.crit_multiplier = crit_multiplier;
        self.crit_chance = calculate_crit_chance(main, opponent, 0.5, 0.0);
        self.class_data.update_opponent(main, opponent);
        self.opponent_is_mage = opponent.class == Class::Mage;
    }

    pub fn attack(
        &mut self,
        target: &mut InBattleFighter,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        match &mut self.class_data {
            ClassData::Assassin { secondary_damage } => {
                let secondary_damage = *secondary_damage;
                *round += 1;
                if target.will_take_attack(rng) {
                    let first_weapon_damage =
                        self.calc_basic_hit_damage(*round, rng);
                    if target.take_attack(first_weapon_damage, round, rng) {
                        return true;
                    }
                }

                *round += 1;

                if !target.will_take_attack(rng) {
                    return false;
                }

                let second_weapon_damage = calculate_hit_damage(
                    &secondary_damage,
                    *round,
                    self.crit_chance,
                    self.crit_multiplier,
                    rng,
                );

                target.take_attack(second_weapon_damage, round, rng)
            }
            ClassData::Druid {
                has_just_dodged,
                rage_crit_chance,
                is_in_bear_form,
                swoop_chance,
            } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }

                if *has_just_dodged {
                    // transform into a bear and attack
                    *is_in_bear_form = true;
                    *has_just_dodged = false;

                    *round += 1;

                    if !target.will_take_attack(rng) {
                        return false;
                    }

                    let crit_multiplier = (2.0 + 40.0) * self.crit_chance / 2.0;

                    let dmg = calculate_hit_damage(
                        &self.damage,
                        *round,
                        *rage_crit_chance,
                        crit_multiplier,
                        rng,
                    );
                    return target.take_attack(dmg, round, rng);
                }

                let swoop_damage_modifier = (1.0 / 3.0 + 0.8) / (1.0 / 3.0);

                *is_in_bear_form = false;

                let will_swoop = rng.f64() < *swoop_chance;

                if will_swoop {
                    *round += 1;
                    *swoop_chance = 0.5f64.min(*swoop_chance + 0.05);
                    if target.will_take_attack(rng) {
                        let swoop_dmg = self.calc_basic_hit_damage(*round, rng)
                            * swoop_damage_modifier;

                        if target.take_attack(swoop_dmg, round, rng) {
                            return true;
                        }
                    }
                }

                self.attack_generic(target, round, rng)
            }
            ClassData::Bard {
                melody_length,
                next_melody_round,
                melody_dmg_multiplier,
            } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }

                if *melody_length == 0 {
                    *melody_dmg_multiplier = 1.0;
                }
                if *melody_length <= 0 && *next_melody_round <= 0 {
                    let (length, multi) = match rng.u32(0..4) {
                        0 | 1 => (3, 1.4),
                        2 => (3, 1.2),
                        _ => (4, 1.6),
                    };
                    *melody_length = length;
                    *melody_dmg_multiplier = multi;
                    *next_melody_round = 4;
                }

                *melody_length -= 1;
                *next_melody_round -= 1;

                if !target.will_take_attack(rng) {
                    return false;
                }

                let dmg = *melody_dmg_multiplier
                    * self.calc_basic_hit_damage(*round, rng);

                target.take_attack(dmg, round, rng)
            }
            ClassData::Necromancer {
                minion_type,
                minion_rounds,
                ..
            } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }
                *round += 1;

                if *minion_type == NecromancerMinionType::None && rng.bool() {
                    // Summon minion
                    let minion_type_chance = rng.i32(1..4);
                    let (new_type, new_rounds) = match minion_type_chance {
                        1 => (NecromancerMinionType::Skeleton, 3),
                        2 => (NecromancerMinionType::Hound, 2),
                        _ => (NecromancerMinionType::Golem, 4),
                    };

                    *minion_type = new_type;
                    *minion_rounds = new_rounds;
                    return self.attack_with_minion(target, round, rng);
                }

                if target.will_take_attack(rng) {
                    let dmg = self.calc_basic_hit_damage(*round, rng);
                    if target.take_attack(dmg, round, rng) {
                        return true;
                    }
                }

                self.attack_with_minion(target, round, rng)
            }
            ClassData::Paladin { stance, .. } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }

                *round += 1;
                if rng.bool() {
                    // change stance
                    *stance = match stance {
                        PaladinStance::Initial => PaladinStance::Defensive,
                        PaladinStance::Defensive => PaladinStance::Offensive,
                        PaladinStance::Offensive => PaladinStance::Initial,
                    };
                }

                let stance_dmg_multi = stance.damage_multiplier();

                if !target.will_take_attack(rng) {
                    return false;
                }

                let dmg =
                    self.calc_basic_hit_damage(*round, rng) * stance_dmg_multi;

                target.take_attack(dmg, round, rng)
            }
            ClassData::PlagueDoctor {
                poison_round,
                poison_dmg_multipliers,
            } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }

                if *poison_round == 0 && rng.bool() {
                    *round += 1;
                    if !target.will_take_attack(rng) {
                        return false;
                    }

                    *poison_round = 3;

                    let poison_multiplier = poison_dmg_multipliers[2];
                    let tincture_throw_dmg = calculate_hit_damage(
                        &(self.damage * poison_multiplier),
                        *round,
                        self.crit_chance,
                        self.crit_multiplier,
                        rng,
                    );

                    return target.take_attack(tincture_throw_dmg, round, rng);
                }

                if *poison_round > 0 {
                    *round += 1;
                    *poison_round -= 1;

                    let poison_multiplier =
                        poison_dmg_multipliers[*poison_round];
                    let poison_dmg = calculate_hit_damage(
                        &(self.damage * poison_multiplier),
                        *round,
                        self.crit_chance,
                        self.crit_multiplier,
                        rng,
                    );

                    if target.take_attack(poison_dmg, round, rng) {
                        return true;
                    }
                }
                self.attack_generic(target, round, rng)
            }
            _ => self.attack_generic(target, round, rng),
        }
    }

    fn attack_generic(
        &mut self,
        target: &mut InBattleFighter,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        *round += 1;

        if !target.will_take_attack(rng) {
            return false;
        }

        let dmg = self.calc_basic_hit_damage(*round, rng);
        target.take_attack(dmg, round, rng)
    }

    pub fn attack_before_fight(
        &mut self,
        target: &mut InBattleFighter,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        match &mut self.class_data {
            ClassData::BattleMage {
                fireball_dmg,
                used_fireball,
            } if !*used_fireball => {
                *round += 1;
                *used_fireball = true;
                target.take_attack(*fireball_dmg, round, rng)
            }
            _ => false,
        }
    }

    pub fn will_skip_round(
        &mut self,
        target: &mut InBattleFighter,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        match &mut self.class_data {
            ClassData::Berserker {
                chain_attack_counter,
            } => {
                if target.fighter.class == Class::Mage {
                    return false;
                }

                if *chain_attack_counter >= 14 {
                    *chain_attack_counter = 0;
                } else if rng.u32(1..=100) > 50 {
                    *round += 1;
                    *chain_attack_counter += 1;
                    return true;
                } else {
                    *chain_attack_counter = 0;
                }
                false
            }
            _ => false,
        }
    }

    pub fn take_attack(
        &mut self,
        damage: f64,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        match &mut self.class_data {
            ClassData::DemonHunter { revive_count } => {
                let health = &mut self.fighter.health;
                *health -= damage;
                if *health > 0.0 {
                    return false;
                }
                if self.opponent_is_mage {
                    return true;
                }

                // revive logic
                let revive_chance = 0.44 - f64::from(*revive_count) * 0.11;
                if revive_chance <= 0.0 || rng.f64() >= revive_chance {
                    return true;
                }

                *round += 1;
                *revive_count += 1;

                true
            }
            ClassData::Paladin {
                stance,
                initial_armor_reduction,
            } => {
                if self.opponent_is_mage {
                    let health = &mut self.fighter.health;
                    *health -= damage;
                    return *health <= 0.0;
                }

                let current_armor_reduction = match stance {
                    PaladinStance::Initial | PaladinStance::Defensive => 1.0,
                    PaladinStance::Offensive => {
                        1.0 / (1.0 - *initial_armor_reduction)
                            * (1.0 - initial_armor_reduction.min(0.20))
                    }
                };

                let actual_damage = damage * current_armor_reduction;
                let current_health = &mut self.fighter.health;

                if *stance == PaladinStance::Defensive
                    && rng.i32(1..101) <= stance.block_chance()
                {
                    let heal_cap = actual_damage * 0.3;
                    *current_health += (self.max_health - *current_health)
                        .clamp(0.0, heal_cap);
                    return false;
                }

                *current_health -= actual_damage;
                *current_health <= 0.0
            }
            _ => {
                let health = &mut self.fighter.health;
                *health -= damage;
                *health <= 0.0
            }
        }
    }

    pub fn will_take_attack(&mut self, rng: &mut Rng) -> bool {
        match &mut self.class_data {
            ClassData::Warrior { block_chance } => {
                rng.i32(1..101) > *block_chance
            }
            ClassData::Scout => rng.i32(1..101) > 50,
            ClassData::Assassin { .. } => rng.u32(1..=100) > 50,
            ClassData::Druid {
                is_in_bear_form,
                has_just_dodged,
                ..
            } => {
                if !*is_in_bear_form && rng.i32(1..101) <= 35 {
                    // evade_chance hardcoded to 35 in original
                    *has_just_dodged = true;
                    return false;
                }
                true
            }
            ClassData::Necromancer { minion_type, .. } => {
                if self.opponent_is_mage {
                    return true;
                }
                if *minion_type != NecromancerMinionType::Golem {
                    return true;
                }
                rng.i32(1..101) > 25
            }
            ClassData::Paladin { stance, .. } => {
                *stance == PaladinStance::Defensive
                    || rng.i32(1..101) > stance.block_chance()
            }
            ClassData::PlagueDoctor { poison_round, .. } => {
                let chance = match poison_round {
                    3 => 65,
                    2 => 50,
                    1 => 35,
                    _ => 20,
                };
                rng.i32(1..101) > chance
            }
            _ => true,
        }
    }

    fn calc_basic_hit_damage(&self, round: u32, rng: &mut Rng) -> f64 {
        calculate_hit_damage(
            &self.damage,
            round,
            self.crit_chance,
            self.crit_multiplier,
            rng,
        )
    }

    fn attack_with_minion(
        &mut self,
        target: &mut InBattleFighter,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        let ClassData::Necromancer {
            minion_type: current_minion,
            minion_rounds,
            skeleton_revives,
            base_damage_multi,
        } = &mut self.class_data
        else {
            // Should not happen
            return false;
        };

        if *current_minion == NecromancerMinionType::None {
            return false;
        }

        *round += 1;

        *minion_rounds -= 1;

        if *minion_rounds == 0
            && *current_minion == NecromancerMinionType::Skeleton
            && *skeleton_revives < 1
        {
            *minion_rounds = 1;
            *skeleton_revives += 1;
        } else if *minion_rounds == 0 {
            *current_minion = NecromancerMinionType::None;
            *skeleton_revives = 0;
        }

        if !target.will_take_attack(rng) {
            return false;
        }

        let mut crit_chance = self.crit_chance;
        let mut crit_multi = self.crit_multiplier;
        if *current_minion == NecromancerMinionType::Hound {
            crit_chance = (crit_chance + 0.1).min(0.6);
            crit_multi = 2.5 * (crit_multi / 2.0);
        }

        let mut dmg = calculate_hit_damage(
            &self.damage,
            *round,
            crit_chance,
            crit_multi,
            rng,
        );

        let base_multi = *base_damage_multi;
        let minion_dmg_multiplier = match current_minion {
            NecromancerMinionType::Skeleton => (base_multi + 0.25) / base_multi,
            NecromancerMinionType::Hound => (base_multi + 1.0) / base_multi,
            NecromancerMinionType::Golem => 1.0,
            NecromancerMinionType::None => 0.0,
        };

        dmg *= minion_dmg_multiplier;
        target.take_attack(dmg, round, rng)
    }
}

#[derive(Debug, Clone)]
pub struct Fighter {
    pub class: Class,
    pub level: u16,
    pub attributes: EnumMap<AttributeType, u32>,
    pub health: f64,
    pub armor: u32,
    pub first_weapon: Option<Weapon>,
    pub second_weapon: Option<Weapon>,
    pub reaction: i32,
    pub crit_multiplier: f64,
    pub resistances: EnumMap<Element, i32>,
    pub guild_portal: f64,

    pub is_companion: bool,

    pub gladiator: i32,
}

impl From<&Monster> for Vec<Fighter> {
    fn from(value: &Monster) -> Self {
        let fighter: Fighter = value.into();
        vec![fighter]
    }
}

impl From<&Monster> for Fighter {
    fn from(monster: &Monster) -> Fighter {
        let mut weapon = Weapon {
            rune_value: 0,
            rune_type: None,
            damage: DamageRange {
                min: f64::from(monster.min_dmg),
                max: f64::from(monster.max_dmg),
            },
        };
        let mut resistances = EnumMap::default();

        if let Some(runes) = &monster.runes {
            resistances = runes.resistances;
            weapon.rune_value = runes.damage;
            weapon.rune_type = Some(runes.damage_type);
        }

        Fighter {
            class: monster.class,
            level: monster.level,
            attributes: monster.attributes,
            health: monster.hp as f64,
            armor: monster.armor,
            first_weapon: Some(weapon),
            second_weapon: None,
            reaction: 0,
            crit_multiplier: 2.0,
            resistances,
            guild_portal: 0.0,
            is_companion: false,
            gladiator: 0,
        }
    }
}

impl InBattleFighter {
    pub(crate) fn new(
        main: &Fighter,
        opponent: &Fighter,
        reduce_gladiator: bool,
    ) -> InBattleFighter {
        let class_data = ClassData::new(main, opponent);

        let mut res = InBattleFighter {
            fighter: main.clone(),
            max_health: main.health,
            damage: DamageRange::default(),
            crit_chance: 0.0,
            crit_multiplier: 0.0,
            opponent_is_mage: false,
            class_data,
        };
        res.update_opponent(opponent, reduce_gladiator);
        res
    }
}

impl ClassData {
    pub fn update_opponent(&mut self, main: &Fighter, opponent: &Fighter) {
        match self {
            ClassData::Bard { .. }
            | ClassData::DemonHunter { .. }
            | ClassData::Mage
            | ClassData::Scout
            | ClassData::Warrior { .. } => {}
            ClassData::Assassin { secondary_damage } => {
                let range = calculate_damage(
                    main.second_weapon.as_ref(),
                    main,
                    opponent,
                    true,
                );
                *secondary_damage = range;
            }
            ClassData::BattleMage { fireball_dmg, .. } => {
                *fireball_dmg = calculate_fire_ball_damage(main, opponent);
            }
            ClassData::Berserker {
                chain_attack_counter,
            } => *chain_attack_counter = 0,
            ClassData::Druid {
                rage_crit_chance, ..
            } => {
                *rage_crit_chance =
                    calculate_crit_chance(main, opponent, 0.75, 0.1);
            }

            ClassData::Necromancer {
                base_damage_multi, ..
            } => {
                *base_damage_multi =
                    calculate_damage_multiplier(main, opponent);
            }
            ClassData::Paladin {
                initial_armor_reduction,
                ..
            } => {
                *initial_armor_reduction =
                    calculate_damage_reduction(opponent, main);
            }
            ClassData::PlagueDoctor {
                poison_dmg_multipliers,
                ..
            } => {
                let base_dmg_multi =
                    calculate_damage_multiplier(main, opponent);

                let dmg_multiplier =
                    Class::PlagueDoctor.get_config().damage_multiplier;
                let class_dmg_multi = base_dmg_multi / dmg_multiplier;

                *poison_dmg_multipliers = [
                    (base_dmg_multi - 0.9 * class_dmg_multi) / base_dmg_multi,
                    (base_dmg_multi - 0.55 * class_dmg_multi) / base_dmg_multi,
                    (base_dmg_multi - 0.2 * class_dmg_multi) / base_dmg_multi,
                ];
                // TODO: Do we reset poison round?
            }
        }
    }

    pub fn new(main: &Fighter, opponent: &Fighter) -> ClassData {
        let mut res = match main.class {
            Class::Warrior if main.is_companion => {
                ClassData::Warrior { block_chance: 0 }
            }
            Class::Warrior => ClassData::Warrior { block_chance: 25 },
            Class::Mage => ClassData::Mage,
            Class::Scout => ClassData::Scout,
            Class::Assassin => ClassData::Assassin {
                secondary_damage: DamageRange::default(),
            },
            Class::BattleMage => ClassData::BattleMage {
                fireball_dmg: 0.0,
                used_fireball: false,
            },
            Class::Berserker => ClassData::Berserker {
                chain_attack_counter: 0,
            },
            Class::DemonHunter => ClassData::DemonHunter { revive_count: 0 },
            Class::Druid => ClassData::Druid {
                rage_crit_chance: 0.0,
                is_in_bear_form: false,
                has_just_dodged: false,
                swoop_chance: 0.15,
            },
            Class::Bard => ClassData::Bard {
                melody_length: -1,
                next_melody_round: 0,
                melody_dmg_multiplier: 1.0,
            },
            Class::Necromancer => ClassData::Necromancer {
                base_damage_multi: 0.0,
                minion_type: NecromancerMinionType::None,
                minion_rounds: 0,
                skeleton_revives: 0,
            },
            Class::Paladin => ClassData::Paladin {
                initial_armor_reduction: 0.0,
                stance: PaladinStance::Initial,
            },
            Class::PlagueDoctor => ClassData::PlagueDoctor {
                poison_round: 0,
                poison_dmg_multipliers: [0.0, 0.0, 0.0],
            },
        };
        res.update_opponent(main, opponent);
        res
    }
}
