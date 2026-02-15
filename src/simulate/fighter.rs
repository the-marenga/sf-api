use std::hash::Hash;

use enum_map::EnumMap;
use fastrand::Rng;

use crate::{
    command::AttributeType,
    gamestate::{character::Class, items::*},
    misc::EnumMapGet,
    simulate::{damage::*, upgradeable::UpgradeableFighter, *},
};

/// Contains all informations, that are necessary for battles to be simulated.
/// It is derived by converting any of the things that can fight (player,
/// companion, etc.) to a fighter through the From<T> traits.
/// Contains all informations, that are necessary for battles to be simulated.
/// It is derived by converting any of the things that can fight (player,
/// companion, etc.) to a fighter through the From<T> traits.
///
/// ## Example
/// To create a `Fighter` from a monster:
///
/// ```rust,ignore
/// let monster: &Monster = get_dungeon_monster(..);
/// // Convert this to a Fighter
/// let monster_fighter: Fighter = monster.into();
/// // or
/// let monster_fighter = Fighter::from(monster);
/// ```
#[derive(Debug, Clone)]
pub struct Fighter {
    pub ident: FighterIdent,
    /// The name, or alternative identification of this fighter. Only used for
    /// display purposes, does not affect combat.
    pub name: std::sync::Arc<str>,
    /// The class of the fighter (e.g., Warrior, Mage).
    pub class: Class,
    /// The level of the fighter.
    pub level: u16,
    /// The attributes of the fighter
    pub attributes: EnumMap<AttributeType, u32>,
    /// The health the fighter has before going into battle.
    pub max_health: f64,
    /// The armor value that reduces incoming damage. Sum of all equipment.
    pub armor: u32,
    /// The fighter's first weapon, if equipped.
    pub first_weapon: Option<Weapon>,
    /// The fighter's second weapon, if the fighter is an assassin. Shields are
    /// not tracked
    pub second_weapon: Option<Weapon>,
    /// Check if this fighter has the enchantment to take the first action
    pub has_reaction_enchant: bool,
    /// The critical hit multiplier for the fighter.
    pub crit_dmg_multi: f64,
    /// The resistances of the fighter to various elements from runes.
    pub resistances: EnumMap<Element, i32>,
    /// The damage bonus the fighter receives from guild portal.
    pub portal_dmg_bonus: f64,
    /// Indicates whether the fighter is a companion.
    pub is_companion: bool,
    /// The level of the gladiator building in the underworld.
    pub gladiator_lvl: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FighterIdent(u32);

impl FighterIdent {
    pub fn new() -> Self {
        FighterIdent(fastrand::u32(..))
    }
}

impl Default for FighterIdent {
    fn default() -> Self {
        Self::new()
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

        // TODO: is this real?
        let second_weapon =
            (monster.class == Class::Assassin).then(|| weapon.clone());

        Fighter {
            ident: FighterIdent::new(),
            name: std::sync::Arc::from(monster.name),
            class: monster.class,
            level: monster.level,
            attributes: monster.attributes,
            max_health: monster.hp as f64,
            armor: monster.armor,
            second_weapon,
            first_weapon: Some(weapon),
            has_reaction_enchant: false,
            crit_dmg_multi: 2.0,
            resistances,
            portal_dmg_bonus: 0.0,
            is_companion: false,
            gladiator_lvl: 0,
        }
    }
}

impl From<&UpgradeableFighter> for Fighter {
    fn from(char: &UpgradeableFighter) -> Self {
        use RuneType as RT;

        let attributes = char.attributes();
        let health = char.hit_points(&attributes) as f64;

        let mut resistances = EnumMap::default();
        let mut has_reaction = false;
        let mut extra_crit_dmg = 0.0;
        let mut armor = 0;
        let mut weapon = None;
        let mut offhand = None;

        for (slot, item) in &char.equipment.0 {
            let Some(item) = item else {
                continue;
            };
            armor += item.armor();
            match item.enchantment {
                Some(Enchantment::SwordOfVengeance) => {
                    extra_crit_dmg = 0.05;
                }
                Some(Enchantment::ShadowOfTheCowboy) => {
                    has_reaction = true;
                }
                _ => {}
            }

            if let Some(rune) = item.rune {
                let mut apply = |element| {
                    *resistances.get_mut(element) += i32::from(rune.value);
                };
                match rune.typ {
                    RT::FireResistance => apply(Element::Fire),
                    RT::ColdResistence => apply(Element::Cold),
                    RT::LightningResistance => apply(Element::Lightning),
                    RT::TotalResistence => {
                        for val in &mut resistances.values_mut() {
                            *val += i32::from(rune.value);
                        }
                    }
                    _ => {}
                }
            }

            match item.typ {
                ItemType::Weapon { min_dmg, max_dmg } => {
                    let mut res = Weapon {
                        rune_value: 0,
                        rune_type: None,
                        damage: DamageRange {
                            min: f64::from(min_dmg),
                            max: f64::from(max_dmg),
                        },
                    };
                    if let Some(rune) = item.rune {
                        res.rune_type = match rune.typ {
                            RT::FireDamage => Some(Element::Fire),
                            RT::ColdDamage => Some(Element::Cold),
                            RT::LightningDamage => Some(Element::Lightning),
                            _ => None,
                        };
                        res.rune_value = rune.value.into();
                    }
                    match slot {
                        EquipmentSlot::Weapon => weapon = Some(res),
                        EquipmentSlot::Shield => offhand = Some(res),
                        _ => {}
                    }
                }
                ItemType::Shield { block_chance: _ } => {
                    // TODO: What about the block chance of this?
                    // Should this not be used?
                }
                _ => (),
            }
        }

        let crit_multiplier =
            2.0 + extra_crit_dmg + f64::from(char.gladiator) * 0.11;

        Fighter {
            ident: FighterIdent::new(),
            name: char.name.clone(),
            class: char.class,
            level: char.level,
            attributes,
            max_health: health,
            armor,
            first_weapon: weapon,
            second_weapon: offhand,
            has_reaction_enchant: has_reaction,
            crit_dmg_multi: crit_multiplier,
            resistances,
            portal_dmg_bonus: f64::from(char.portal_dmg_bonus),
            is_companion: char.is_companion,
            gladiator_lvl: char.gladiator,
        }
    }
}

// TODO: Impl From OtherPlayer / Pet

/// Contains all relevant information about a fighter, that has entered combat
/// against another fighter, that are relevant to resolve this 1on1 battle.
/// If this fighter has won a 1on1 battle and is matched up with another enemy,
/// the stats must be updated using `update_opponent()`.
#[derive(Debug, Clone)]
pub(crate) struct InBattleFighter {
    /// The name, or alternative identification of this fighter. Only used for
    /// display purposes, does not affect combat.
    #[allow(unused)]
    pub name: Arc<str>,
    /// The class of the fighter (e.g., Warrior, Mage).
    pub class: Class,
    /// The amount of health this fighter has started the battle with
    pub max_health: f64,
    /// The amount of health this fighter currently has. May be negative, or
    /// zero
    pub health: f64,
    /// The amount of damage this fighter can do with a normal (weapon 1)
    /// attack on the first turn
    pub damage: DamageRange,
    /// The reaction speed of the fighter, affecting turn order. `1` if this
    /// fighter has an item with the relevant enchantment
    pub reaction: u8,
    /// The chance to land a critical hit against the opponent
    pub crit_chance: f64,
    /// The amount of damage a crit does compared to a normal attack
    pub crit_dmg_multi: f64,
    /// Just a flag that stores if the opponent is a mage. We could also store
    /// the class of the opponent here, but we only ever really care about
    /// mage.
    pub opponent_is_mage: bool,

    /// All the metadata a fighter needs to keep track of during a fight, that
    /// is unique to their class.
    pub class_data: ClassData,
}

/// The class specific metadata a fighter needs to keep track of during a fight.
#[derive(Debug, Clone)]
pub(crate) enum ClassData {
    Warrior {
        /// The chance to block an attack with the shield
        block_chance: i32,
    },
    Mage,
    Scout,
    Assassin {
        /// The weapon damage from the secondary weapon
        secondary_damage: DamageRange,
    },
    BattleMage {
        /// The damage a fireball does against the enemy on the first turn
        fireball_dmg: f64,
        /// Has the fireball already been used?
        used_fireball: bool,
    },
    Berserker {
        /// The amount of times the berserker has attacked consecutively in
        /// a frenzy
        frenzy_attacks: u32,
    },
    DemonHunter {
        /// The amount of times the demon hunter has revived
        revive_count: u32,
    },
    Druid {
        /// Is this character currently in bear form
        is_in_bear_form: bool,
        /// The chance to crit whilst in rage (bear form)
        rage_crit_chance: f64,
        /// Have we just an enemies attack, which would lead us to transform
        /// into a bear on our next turn?
        has_just_dodged: bool,
        /// The chance to do a swoop attack
        swoop_chance: f64,
        /// The amount of damage a swoop attack does compared to a normal
        /// attack
        swoop_dmg_multi: f64,
    },
    Bard {
        /// The amount of turns the melody is still active for
        melody_remaining_rounds: i32,
        /// The amount of turns until we can start playing a new melody
        melody_cooldown_rounds: i32,
        /// The amount of damage an attack does based on the current melody
        /// compared to a generic attack
        melody_dmg_multi: f64,
    },
    Necromancer {
        // TODO: When exactly is this applied
        damage_multi: f64,
        /// The type of minion, that we have summoned, if any
        minion: Option<Minion>,
        /// The amount of rounds the minion is going to remain active for
        minion_remaining_rounds: i32,
        /// The amount of times the skeleton has revived
        skeleton_revived: i32,
    },
    Paladin {
        // TODO: What exactly is this? Is this a damage bonus? Why is it named
        // this?
        initial_armor_reduction: f64,
        /// The current stance, that the paladin is in
        stance: Stance,
    },
    PlagueDoctor {
        /// The amount of rounds the current tincture is still active for
        poison_remaining_round: usize,
        /// The damage multipliers the three turns of poison inflict extra on
        /// the opponent
        poison_dmg_multis: [f64; 3],
    },
}

/// The type of minion a necromancer can summon
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Minion {
    Skeleton,
    Hound,
    Golem,
}

/// The stance a paladin can enter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stance {
    Regular,
    Defensive,
    Offensive,
}

impl Stance {
    pub(crate) fn damage_multiplier(self) -> f64 {
        match self {
            Stance::Regular => 1.0,
            Stance::Defensive => 1.0 / 0.833 * 0.568,
            Stance::Offensive => 1.0 / 0.833 * 1.253,
        }
    }

    pub(crate) fn block_chance(self) -> u8 {
        match self {
            Stance::Regular => 30,
            Stance::Defensive => 50,
            Stance::Offensive => 25,
        }
    }
}
/// Calculates for `main` to crit `opponent`
pub(crate) fn calculate_crit_chance(
    main: &Fighter,
    opponent: &Fighter,
    cap: f64,
    crit_bonus: f64,
) -> f64 {
    let luck_factor = f64::from(main.attributes[AttributeType::Luck]) * 5.0;
    let opponent_level_factor = f64::from(opponent.level) * 2.0;
    let crit_chance = luck_factor / opponent_level_factor / 100.0 + crit_bonus;
    crit_chance.min(cap)
}

impl InBattleFighter {
    /// Shorthand to check if this fighter is a mage
    pub fn is_mage(&self) -> bool {
        self.class == Class::Mage
    }

    /// Update the stats that are affected by the opponent with a new oponent
    /// without resetting persistent data points
    pub fn update_opponent(
        &mut self,
        main: &Fighter,
        opponent: &Fighter,
        reduce_gladiator: bool,
    ) {
        self.damage = calculate_damage(main, opponent, false);

        let mut crit_dmg_multi = main.crit_dmg_multi;
        if reduce_gladiator {
            let glad_lvl = main.gladiator_lvl.min(opponent.gladiator_lvl);
            crit_dmg_multi -= f64::from(glad_lvl) * 0.11;
        }
        self.crit_dmg_multi = crit_dmg_multi;
        self.crit_chance = calculate_crit_chance(main, opponent, 0.5, 0.0);

        self.class_data.update_opponent(main, opponent);
        self.opponent_is_mage = opponent.class == Class::Mage;
    }

    /// Does a full attack turn for this fighter against the target. Returns
    /// true, if the opponent has won
    pub fn attack(
        &mut self,
        target: &mut InBattleFighter,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        match &mut self.class_data {
            ClassData::Assassin { secondary_damage } => {
                let secondary_damage = *secondary_damage;

                // Main hand attack
                *round += 1;
                if target.will_take_attack(rng) {
                    let first_weapon_damage =
                        self.calc_basic_hit_damage(*round, rng);
                    if target.take_attack_dmg(first_weapon_damage, round, rng) {
                        return true;
                    }
                }

                // Second hand attack
                *round += 1;
                if !target.will_take_attack(rng) {
                    return false;
                }

                let second_weapon_damage = calculate_hit_damage(
                    &secondary_damage,
                    *round,
                    self.crit_chance,
                    self.crit_dmg_multi,
                    rng,
                );

                target.take_attack_dmg(second_weapon_damage, round, rng)
            }
            ClassData::Druid {
                has_just_dodged,
                rage_crit_chance,
                is_in_bear_form,
                swoop_chance,
                swoop_dmg_multi,
            } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }

                if *has_just_dodged {
                    // transform into a bear and attack with rage
                    *is_in_bear_form = true;
                    *has_just_dodged = false;

                    *round += 1;

                    if !target.will_take_attack(rng) {
                        return false;
                    }

                    let rage_crit_multi = 6.0 * self.crit_dmg_multi / 2.0;
                    let dmg = calculate_hit_damage(
                        &self.damage,
                        *round,
                        *rage_crit_chance,
                        rage_crit_multi,
                        rng,
                    );
                    return target.take_attack_dmg(dmg, round, rng);
                }

                *is_in_bear_form = false;

                // eagle form

                let do_swoop_attack = rng.f64() < *swoop_chance;
                if do_swoop_attack {
                    *round += 1;
                    *swoop_chance = (*swoop_chance + 0.05).min(0.5);

                    if target.will_take_attack(rng) {
                        let swoop_dmg_multi = *swoop_dmg_multi;
                        let swoop_dmg = self.calc_basic_hit_damage(*round, rng)
                            * swoop_dmg_multi;

                        if target.take_attack_dmg(swoop_dmg, round, rng) {
                            return true;
                        }
                    }
                }

                self.attack_generic(target, round, rng)
            }
            ClassData::Bard {
                melody_remaining_rounds,
                melody_cooldown_rounds,
                melody_dmg_multi,
            } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }

                if *melody_remaining_rounds <= 0 && *melody_cooldown_rounds <= 0
                {
                    // Start playing a new melody
                    let (length, multi) = match rng.u32(0..4) {
                        0 | 1 => (3, 1.4),
                        2 => (3, 1.2),
                        _ => (4, 1.6),
                    };
                    *melody_remaining_rounds = length;
                    *melody_dmg_multi = multi;
                    *melody_cooldown_rounds = 4;
                } else if *melody_remaining_rounds == 0 {
                    // Stop a melody effect, that has elapsed
                    *melody_dmg_multi = 1.0;
                }

                *melody_remaining_rounds -= 1;
                *melody_cooldown_rounds -= 1;

                if !target.will_take_attack(rng) {
                    return false;
                }

                let dmg_multi = *melody_dmg_multi;
                let dmg = self.calc_basic_hit_damage(*round, rng) * dmg_multi;
                target.take_attack_dmg(dmg, round, rng)
            }
            ClassData::Necromancer {
                minion,
                minion_remaining_rounds: minion_rounds,
                ..
            } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }
                *round += 1;

                if minion.is_none() && rng.bool() {
                    // Summon a new minion and have it attack
                    let (new_type, new_rounds) = match rng.u8(0..3) {
                        0 => (Minion::Skeleton, 3),
                        1 => (Minion::Hound, 2),
                        _ => (Minion::Golem, 4),
                    };

                    *minion = Some(new_type);
                    *minion_rounds = new_rounds;
                    return self.attack_with_minion(target, round, rng);
                }

                if target.will_take_attack(rng) {
                    // Do a normal attack before minion attack
                    let dmg = self.calc_basic_hit_damage(*round, rng);
                    if target.take_attack_dmg(dmg, round, rng) {
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
                        Stance::Regular => Stance::Defensive,
                        Stance::Defensive => Stance::Offensive,
                        Stance::Offensive => Stance::Regular,
                    };
                }

                if !target.will_take_attack(rng) {
                    return false;
                }

                let dmg_multi = stance.damage_multiplier();
                let dmg = self.calc_basic_hit_damage(*round, rng) * dmg_multi;
                target.take_attack_dmg(dmg, round, rng)
            }
            ClassData::PlagueDoctor {
                poison_remaining_round,
                poison_dmg_multis,
            } => {
                if target.is_mage() {
                    return self.attack_generic(target, round, rng);
                }

                if *poison_remaining_round == 0 && rng.bool() {
                    // Throw a new tincture and attack
                    *round += 1;
                    if !target.will_take_attack(rng) {
                        return false;
                    }

                    *poison_remaining_round = 3;

                    let dmg_multi = poison_dmg_multis[2];
                    let dmg =
                        self.calc_basic_hit_damage(*round, rng) * dmg_multi;
                    return target.take_attack_dmg(dmg, round, rng);
                }

                if *poison_remaining_round > 0 {
                    // Apply damage tick from the tincture that we currently
                    // have in effect
                    *round += 1;
                    *poison_remaining_round -= 1;

                    #[allow(clippy::indexing_slicing)]
                    let dmg_multi = poison_dmg_multis[*poison_remaining_round];
                    let dmg =
                        self.calc_basic_hit_damage(*round, rng) * dmg_multi;

                    if target.class == Class::Paladin {
                        // Paladin can not block this
                        target.health -= dmg;
                        if target.health <= 0.0 {
                            return true;
                        }
                    } else if target.take_attack_dmg(dmg, round, rng) {
                        return true;
                    }
                }
                self.attack_generic(target, round, rng)
            }
            ClassData::Mage => {
                // Mage attacks to not check will_take_attack
                let dmg = self.calc_basic_hit_damage(*round, rng);
                target.take_attack_dmg(dmg, round, rng)
            }
            _ => self.attack_generic(target, round, rng),
        }
    }

    /// The most generic type of attack. Just a swing/stab/shot with the main
    /// weapon. Increases turn timer and checks for target dodges.
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
        target.take_attack_dmg(dmg, round, rng)
    }

    /// Any kind of attack, that happens at the start of a 1v1 fight
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
                target.take_attack_dmg(*fireball_dmg, round, rng)
            }
            _ => false,
        }
    }

    /// Do we deny the opponents next turn?
    pub fn will_skips_opponent_round(
        &mut self,
        target: &mut InBattleFighter,
        _round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        match &mut self.class_data {
            ClassData::Berserker { frenzy_attacks } => {
                if target.class == Class::Mage {
                    return false;
                }

                if *frenzy_attacks < 14 && rng.bool() {
                    *frenzy_attacks += 1;
                    return true;
                }

                *frenzy_attacks = 0;
                false
            }
            _ => false,
        }
    }

    /// Applies the given damage to this fighter. The damage will be reduced,
    /// if possible and if applicable this fighter may revive. If the fighter
    /// ends up dead, this will return true.
    pub fn take_attack_dmg(
        &mut self,
        damage: f64,
        round: &mut u32,
        rng: &mut Rng,
    ) -> bool {
        match &mut self.class_data {
            ClassData::DemonHunter { revive_count } => {
                let health = &mut self.health;
                *health -= damage;
                if *health > 0.0 {
                    return false;
                }
                if self.opponent_is_mage {
                    return true;
                }

                // revive logic
                let revive_chance = 0.44 - (f64::from(*revive_count) * 0.11);
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
                let current_armor_reduction = match stance {
                    Stance::Regular | Stance::Defensive => 1.0,
                    Stance::Offensive => {
                        1.0 / (1.0 - *initial_armor_reduction)
                            * (1.0 - initial_armor_reduction.min(0.20))
                    }
                };
                let actual_damage = damage * current_armor_reduction;
                let health = &mut self.health;

                if self.opponent_is_mage {
                    *health -= actual_damage;
                    return *health <= 0.0;
                }

                if *stance == Stance::Defensive
                    && rng.u8(1..=100) <= stance.block_chance()
                {
                    let heal_cap = actual_damage * 0.3;
                    *health += (self.max_health - *health).clamp(0.0, heal_cap);
                    return false;
                }

                *health -= actual_damage;
                *health <= 0.0
            }
            _ => {
                let health = &mut self.health;
                *health -= damage;
                *health <= 0.0
            }
        }
    }

    /// Checks if this fighter manages to block/dodge the enemies attack
    pub fn will_take_attack(&mut self, rng: &mut Rng) -> bool {
        match &mut self.class_data {
            ClassData::Warrior { block_chance } => {
                rng.i32(1..=100) > *block_chance
            }
            ClassData::Assassin { .. } | ClassData::Scout => rng.bool(),
            ClassData::Druid {
                is_in_bear_form,
                has_just_dodged,
                ..
            } => {
                if !*is_in_bear_form && rng.u8(1..=100) <= 35 {
                    // evade_chance hardcoded to 35 in original
                    *has_just_dodged = true;
                    return false;
                }
                true
            }
            ClassData::Necromancer { minion, .. } => {
                if self.opponent_is_mage {
                    return true;
                }
                if *minion != Some(Minion::Golem) {
                    return true;
                }
                rng.u8(1..=100) > 25
            }
            ClassData::Paladin { stance, .. } => {
                *stance == Stance::Defensive
                    || rng.u8(1..=100) > stance.block_chance()
            }
            ClassData::PlagueDoctor {
                poison_remaining_round,
                ..
            } => {
                let chance = match poison_remaining_round {
                    3 => 65,
                    2 => 50,
                    1 => 35,
                    _ => 20,
                };
                rng.u8(1..=100) > chance
            }
            _ => true,
        }
    }

    fn calc_basic_hit_damage(&self, round: u32, rng: &mut Rng) -> f64 {
        calculate_hit_damage(
            &self.damage,
            round,
            self.crit_chance,
            self.crit_dmg_multi,
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
            minion,
            minion_remaining_rounds,
            skeleton_revived,
            damage_multi,
        } = &mut self.class_data
        else {
            // Should not happen
            return false;
        };

        if minion.is_none() {
            return false;
        }

        *round += 1;

        *minion_remaining_rounds -= 1;

        // NOTE: Currently skeleton can revive only once per fight but this is
        // a bug
        if *minion_remaining_rounds == 0
            && *minion == Some(Minion::Skeleton)
            && *skeleton_revived < 1
            && rng.bool()
        {
            *minion_remaining_rounds = 1;
            *skeleton_revived += 1;
        } else if *minion_remaining_rounds == 0 {
            *minion = None;
            *skeleton_revived = 0;
        }

        if !target.will_take_attack(rng) {
            return false;
        }

        let mut crit_chance = self.crit_chance;
        let mut crit_multi = self.crit_dmg_multi;
        if *minion == Some(Minion::Hound) {
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

        let base_multi = *damage_multi;
        let minion_dmg_multiplier = match minion {
            Some(Minion::Skeleton) => (base_multi + 0.25) / base_multi,
            Some(Minion::Hound) => (base_multi + 1.0) / base_multi,
            Some(Minion::Golem) => 1.0,
            None => 0.0,
        };
        dmg *= minion_dmg_multiplier;

        target.take_attack_dmg(dmg, round, rng)
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
            name: main.name.clone(),
            class: main.class,
            health: main.max_health,
            max_health: main.max_health,
            reaction: u8::from(main.has_reaction_enchant),
            damage: DamageRange::default(),
            crit_chance: 0.0,
            crit_dmg_multi: 0.0,
            opponent_is_mage: false,
            class_data,
        };
        res.update_opponent(main, opponent, reduce_gladiator);
        res
    }
}

impl ClassData {
    pub(crate) fn update_opponent(
        &mut self,
        main: &Fighter,
        opponent: &Fighter,
    ) {
        // TODO: Should we reset stuff like melody / druid form etc. when
        // the opponent becomes a mage?
        match self {
            ClassData::Bard { .. }
            | ClassData::DemonHunter { .. }
            | ClassData::Mage
            | ClassData::Scout
            | ClassData::Warrior { .. } => {}
            ClassData::Assassin { secondary_damage } => {
                let range = calculate_damage(main, opponent, true);
                *secondary_damage = range;
            }
            ClassData::BattleMage { fireball_dmg, .. } => {
                *fireball_dmg = calculate_fire_ball_damage(main, opponent);
            }
            ClassData::Berserker {
                frenzy_attacks: chain_attack_counter,
            } => *chain_attack_counter = 0,
            ClassData::Druid {
                rage_crit_chance,
                swoop_dmg_multi,
                ..
            } => {
                *rage_crit_chance =
                    calculate_crit_chance(main, opponent, 0.75, 0.1);
                *swoop_dmg_multi = calculate_swoop_damage(main, opponent);
            }

            ClassData::Necromancer { damage_multi, .. } => {
                *damage_multi = calculate_damage_multiplier(main, opponent);
            }
            ClassData::Paladin {
                initial_armor_reduction,
                ..
            } => {
                *initial_armor_reduction =
                    calculate_damage_reduction(opponent, main);
            }
            ClassData::PlagueDoctor {
                poison_dmg_multis, ..
            } => {
                let base_dmg_multi =
                    calculate_damage_multiplier(main, opponent);

                let dmg_multiplier = Class::PlagueDoctor.damage_multiplier();
                let class_dmg_multi = base_dmg_multi / dmg_multiplier;

                *poison_dmg_multis = [
                    (base_dmg_multi - 0.9 * class_dmg_multi) / base_dmg_multi,
                    (base_dmg_multi - 0.55 * class_dmg_multi) / base_dmg_multi,
                    (base_dmg_multi - 0.2 * class_dmg_multi) / base_dmg_multi,
                ];
                // TODO: Do we reset poison round?
            }
        }
    }

    pub(crate) fn new(main: &Fighter, opponent: &Fighter) -> ClassData {
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
            Class::Berserker => ClassData::Berserker { frenzy_attacks: 0 },
            Class::DemonHunter => ClassData::DemonHunter { revive_count: 0 },
            Class::Druid => ClassData::Druid {
                rage_crit_chance: 0.0,
                is_in_bear_form: false,
                has_just_dodged: false,
                swoop_chance: 0.15,
                swoop_dmg_multi: 0.0,
            },
            Class::Bard => ClassData::Bard {
                melody_remaining_rounds: -1,
                melody_cooldown_rounds: 0,
                melody_dmg_multi: 1.0,
            },
            Class::Necromancer => ClassData::Necromancer {
                damage_multi: 0.0,
                minion: None,
                minion_remaining_rounds: 0,
                skeleton_revived: 0,
            },
            Class::Paladin => ClassData::Paladin {
                initial_armor_reduction: 0.0,
                stance: Stance::Regular,
            },
            Class::PlagueDoctor => ClassData::PlagueDoctor {
                poison_remaining_round: 0,
                poison_dmg_multis: [0.0, 0.0, 0.0],
            },
        };
        res.update_opponent(main, opponent);
        res
    }
}
