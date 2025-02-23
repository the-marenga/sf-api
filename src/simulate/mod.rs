#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
use enum_map::{Enum, EnumMap};
use fastrand::Rng;
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    command::AttributeType,
    gamestate::{
        character::Class, dungeons::CompanionClass, items::*,
        social::OtherPlayer, GameState,
    },
    misc::EnumMapGet,
};

pub mod constants;

use BattleEvent as BE;

#[derive(Debug, Clone)]
pub struct UpgradeableFighter {
    is_companion: bool,
    level: u16,
    class: Class,
    /// The base attributes without any equipment, or other boosts
    pub attribute_basis: EnumMap<AttributeType, u32>,
    pet_attribute_bonus_perc: EnumMap<AttributeType, f64>,

    equipment: Equipment,
    active_potions: [Option<Potion>; 3],
    /// This should be the percentage bonus to skills from pets
    /// The hp bonus in percent this player has from the personal demon portal
    portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    portal_dmg_bonus: u32,
}

impl UpgradeableFighter {
    /// Uses a potion in the provided slot and returns the old potion, if any
    pub fn use_potion(
        &mut self,
        potion: Potion,
        slot: usize,
    ) -> Option<Potion> {
        self.active_potions
            .get_mut(slot)
            .and_then(|a| a.replace(potion))
    }

    /// Removed the potion at the provided slot and returns the old potion, if
    /// any
    pub fn remove_potion(&mut self, slot: usize) -> Option<Potion> {
        self.active_potions.get_mut(slot).and_then(|a| a.take())
    }

    /// Equip the provided item.
    /// If the item could be equiped, the previous item will be returned
    /// # Errors
    ///
    /// Will return `Err` if the item could not be equipped. It will contain
    /// the item you tried to insert
    pub fn equip(
        &mut self,
        item: Item,
        slot: EquipmentSlot,
    ) -> Result<Option<Item>, Item> {
        let Some(item_slot) = item.typ.equipment_slot() else {
            return Err(item);
        };

        if item_slot != slot {
            let is_offhand = slot == EquipmentSlot::Shield
                && item_slot == EquipmentSlot::Weapon;
            if !(is_offhand && self.class != Class::Assassin) {
                return Err(item);
            }
        }
        if slot == EquipmentSlot::Shield
            && (!self.class.can_wear_shield() || self.is_companion)
        {
            return Err(item);
        }

        let res = self.unequip(slot);
        *self.equipment.0.get_mut(slot) = Some(item);
        Ok(res)
    }

    /// Unequips the item at the provided slot and returns the old item, if any
    pub fn unequip(&mut self, slot: EquipmentSlot) -> Option<Item> {
        self.equipment.0.get_mut(slot).take()
    }

    #[must_use]
    pub fn from_other(other: &OtherPlayer) -> Self {
        UpgradeableFighter {
            is_companion: false,
            level: other.level,
            class: other.class,
            attribute_basis: other.base_attributes,
            equipment: other.equipment.clone(),
            active_potions: other.active_potions,
            pet_attribute_bonus_perc: other
                .pet_attribute_bonus_perc
                .map(|_, a| f64::from(a) / 100.0),
            portal_hp_bonus: other.portal_hp_bonus,
            portal_dmg_bonus: other.portal_dmg_bonus,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Minion {
    Skeleton { revived: u8 },
    Hound,
    Golem,
}

#[derive(Debug, Clone)]
pub struct BattleFighter {
    pub is_companion: bool,
    pub level: u16,
    pub class: Class,
    pub attributes: EnumMap<AttributeType, u32>,
    pub max_hp: i64,
    pub current_hp: i64,
    pub equip: EquipmentEffects,
    pub portal_dmg_bonus: f64,
    /// The total amount of rounds this fighter has started (tried to do an
    /// attack)
    pub rounds_started: u32,
    /// The amount of turns this player has been in the current 1v1 fight
    pub rounds_in_1v1: u32,
    pub class_effect: ClassEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarpQuality {
    Bad,
    Medium,
    Good,
}

// Modified, but mostly copied from:
// https://github.com/HafisCZ/sf-tools/blob/521c2773098d62fe21ae687de2047c05f84813b7/js/sim/base.js#L746C4-L765C6
fn calc_unarmed_base_dmg(
    slot: EquipmentSlot,
    level: u16,
    class: Class,
) -> (u32, u32) {
    if level <= 10 {
        return (1, 2);
    }
    let dmg_level = f64::from(level - 9);
    let multiplier = match class {
        Class::Assassin if slot == EquipmentSlot::Weapon => 1.25,
        Class::Assassin => 0.875,
        _ => 0.7,
    };

    let base = dmg_level * multiplier * class.weapon_multiplier();
    let min = ((base * 2.0) / 3.0).trunc().max(1.0);
    let max = ((base * 4.0) / 3.0).trunc().max(2.0);
    (min as u32, max as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassEffect {
    Druid {
        /// Has the druid just dodged an attack?
        bear: bool,
        /// The amount of swoops the druid has done so far
        swoops: u8,
    },
    Bard {
        quality: HarpQuality,
        remaining: u8,
    },
    Necromancer {
        typ: Minion,
        remaining: u8,
    },
    DemonHunter {
        revived: u8,
    },
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackType {
    Weapon,
    Offhand,
    Swoop,
    Minion,
}

impl ClassEffect {
    fn druid_swoops(self) -> u8 {
        match self {
            ClassEffect::Druid { swoops, .. } => swoops,
            _ => 0,
        }
    }
}

impl BattleFighter {
    #[must_use]
    pub fn from_monster(monster: &Monster) -> Self {
        // TODO: I assume this is unarmed damage, but I should check
        let weapon = calc_unarmed_base_dmg(
            EquipmentSlot::Weapon,
            monster.level,
            monster.class,
        );

        Self {
            is_companion: false,
            level: monster.level,
            class: monster.class,
            attributes: monster.attributes,
            max_hp: monster.hp as i64,
            current_hp: monster.hp as i64,
            equip: EquipmentEffects {
                element_res: EnumMap::default(),
                element_dmg: EnumMap::default(),
                weapon,
                offhand: (0, 0),
                reaction_boost: false,
                extra_crit_dmg: false,
                armor: 0,
            },
            portal_dmg_bonus: 1.0,
            rounds_started: 0,
            rounds_in_1v1: 0,
            class_effect: ClassEffect::Normal,
        }
    }

    #[must_use]
    pub fn from_upgradeable(char: &UpgradeableFighter) -> Self {
        let attributes = char.attributes();
        let hp = char.hit_points(&attributes);

        let mut equip = EquipmentEffects {
            element_res: EnumMap::default(),
            element_dmg: EnumMap::default(),
            reaction_boost: false,
            extra_crit_dmg: false,
            armor: 0,
            weapon: (0, 0),
            offhand: (0, 0),
        };

        for (slot, item) in &char.equipment.0 {
            let Some(item) = item else {
                match slot {
                    EquipmentSlot::Weapon => {
                        equip.weapon =
                            calc_unarmed_base_dmg(slot, char.level, char.class);
                    }
                    EquipmentSlot::Shield if char.class == Class::Assassin => {
                        equip.offhand =
                            calc_unarmed_base_dmg(slot, char.level, char.class);
                    }
                    _ => {}
                }
                continue;
            };
            equip.armor += item.armor();
            match item.enchantment {
                Some(Enchantment::SwordOfVengeance) => {
                    equip.extra_crit_dmg = true;
                }
                Some(Enchantment::ShadowOfTheCowboy) => {
                    equip.reaction_boost = true;
                }
                _ => {}
            };
            if let Some(rune) = item.rune {
                use RuneType as RT;

                let mut apply = |is_res, element| {
                    let target = if is_res {
                        &mut equip.element_res
                    } else {
                        &mut equip.element_dmg
                    };
                    *target.get_mut(element) += f64::from(rune.value) / 100.0;
                };
                match rune.typ {
                    RT::FireResistance => apply(true, Element::Fire),
                    RT::ColdResistence => apply(true, Element::Cold),
                    RT::LightningResistance => apply(true, Element::Lightning),
                    RT::TotalResistence => {
                        for (_, val) in &mut equip.element_res {
                            *val += f64::from(rune.value) / 100.0;
                        }
                    }
                    RT::FireDamage => apply(false, Element::Fire),
                    RT::ColdDamage => apply(false, Element::Cold),
                    RT::LightningDamage => apply(false, Element::Lightning),
                    _ => {}
                }
            }

            match item.typ {
                ItemType::Weapon { min_dmg, max_dmg } => match slot {
                    EquipmentSlot::Weapon => equip.weapon = (min_dmg, max_dmg),
                    EquipmentSlot::Shield => equip.offhand = (min_dmg, max_dmg),
                    _ => {}
                },
                ItemType::Shield { block_chance } => {
                    equip.offhand = (block_chance, 0);
                }
                _ => (),
            }
        }

        let portal_dmg_bonus = 1.0 + f64::from(char.portal_dmg_bonus) / 100.0;

        BattleFighter {
            is_companion: char.is_companion,
            class: char.class,
            attributes,
            max_hp: hp,
            current_hp: hp,
            equip,
            rounds_started: 0,
            class_effect: ClassEffect::Normal,
            portal_dmg_bonus,
            level: char.level,
            rounds_in_1v1: 0,
        }
    }

    #[must_use]
    pub fn from_squad(squad: &PlayerFighterSquad) -> Vec<Self> {
        let mut res = if let Some(comps) = &squad.companions {
            let mut res = Vec::with_capacity(4);
            for comp in comps.as_array() {
                res.push(Self::from_upgradeable(comp));
            }
            res
        } else {
            Vec::with_capacity(1)
        };
        res.push(BattleFighter::from_upgradeable(&squad.character));
        res
    }

    pub fn reset(&mut self) {
        self.class_effect = ClassEffect::Normal;
        self.current_hp = self.max_hp;
        self.rounds_started = 0;
    }
}

#[derive(Debug, Clone)]
pub struct EquipmentEffects {
    element_res: EnumMap<Element, f64>,
    element_dmg: EnumMap<Element, f64>,

    weapon: (u32, u32),
    /// min,max for weapons | blockchange, 0 for shields
    offhand: (u32, u32),

    /// Shadow of the cowboy
    reaction_boost: bool,
    /// Sword of Vengeance
    extra_crit_dmg: bool,

    armor: u32,
}

#[derive(Debug, Clone, Copy, Enum, EnumIter)]
pub enum Element {
    Lightning,
    Cold,
    Fire,
}

#[derive(Debug)]
pub struct BattleTeam<'a> {
    current_fighter: usize,
    fighters: &'a mut [BattleFighter],
}

#[allow(clippy::extra_unused_lifetimes)]
impl<'a> BattleTeam<'_> {
    #[must_use]
    pub fn current(&self) -> Option<&BattleFighter> {
        self.fighters.get(self.current_fighter)
    }
    #[must_use]
    pub fn current_mut(&mut self) -> Option<&mut BattleFighter> {
        self.fighters.get_mut(self.current_fighter)
    }

    fn reset(&mut self) {
        self.current_fighter = 0;
        for fighter in self.fighters.iter_mut() {
            fighter.reset();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum BattleSide {
    Left,
    Right,
}

#[derive(Debug)]
pub struct Battle<'a> {
    pub round: u32,
    pub started: Option<BattleSide>,
    pub left: BattleTeam<'a>,
    pub right: BattleTeam<'a>,
    pub rng: Rng,
}

impl<'a> Battle<'a> {
    pub fn new(
        left: &'a mut [BattleFighter],
        right: &'a mut [BattleFighter],
    ) -> Self {
        Self {
            round: 0,
            started: None,
            left: BattleTeam {
                current_fighter: 0,
                fighters: left,
            },
            right: BattleTeam {
                current_fighter: 0,
                fighters: right,
            },
            rng: fastrand::Rng::default(),
        }
    }

    /// Simulates a battle between the two sides. Returns the winning side.
    pub fn simulate(&mut self, logger: &mut impl BattleLogger) -> BattleSide {
        self.reset();
        loop {
            if let Some(winner) = self.simulate_turn(logger) {
                return winner;
            }
        }
    }

    pub fn reset(&mut self) {
        self.round = 0;
        self.left.reset();
        self.right.reset();
        self.started = None;
    }

    /// Simulates one turn (attack) in a battle. If one side is not able
    /// to fight anymore, or is for another reason invalid, the other side is
    /// returned as the winner
    pub fn simulate_turn(
        &mut self,
        logger: &mut impl BattleLogger,
    ) -> Option<BattleSide> {
        use AttackType::{Offhand, Swoop, Weapon};
        use BattleSide::{Left, Right};
        use Class::{
            Assassin, Bard, BattleMage, Berserker, DemonHunter, Druid, Mage,
            Necromancer, Paladin, Scout, Warrior,
        };

        logger.log(BE::TurnUpdate(self));

        let Some(left) = self.left.current_mut() else {
            logger.log(BE::BattleEnd(self, Right));
            return Some(Right);
        };
        let Some(right) = self.right.current_mut() else {
            logger.log(BE::BattleEnd(self, Left));
            return Some(Left);
        };

        self.round += 1;

        if left.rounds_in_1v1 != right.rounds_in_1v1 {
            left.rounds_in_1v1 = 0;
            right.rounds_in_1v1 = 0;
        }
        left.rounds_in_1v1 += 1;
        right.rounds_in_1v1 += 1;

        let attacking_side = if let Some(started) = self.started {
            let one_vs_one_round = left.rounds_in_1v1.min(right.rounds_in_1v1);

            // If We are at the same cycle, as the first turn, the one that
            // started on the first turn starts here. Otherwise the other one
            match started {
                _ if one_vs_one_round % 2 == 0 => started,
                Left => Right,
                Right => Left,
            }
        } else {
            // The battle has not yet started. Figure out who side starts
            let attacking_side =
                match (right.equip.reaction_boost, left.equip.reaction_boost) {
                    (true, true) | (false, false) if self.rng.bool() => Right,
                    (true, false) => Right,
                    _ => Left,
                };
            self.started = Some(attacking_side);
            attacking_side
        };

        let (attacker, defender) = match attacking_side {
            Left => (left, right),
            Right => (right, left),
        };

        attacker.rounds_started += 1;
        let turn = self.round;
        let rng = &mut self.rng;
        match attacker.class {
            Paladin => {
                // TODO: Actually implement stances and stuff
                attack(attacker, defender, rng, Weapon, turn, logger);
            }
            Warrior | Scout | Mage | DemonHunter => {
                attack(attacker, defender, rng, Weapon, turn, logger);
            }
            Assassin => {
                attack(attacker, defender, rng, Weapon, turn, logger);
                attack(attacker, defender, rng, Offhand, turn, logger);
            }
            Berserker => {
                for _ in 0..15 {
                    attack(attacker, defender, rng, Weapon, turn, logger);
                    if defender.current_hp <= 0 || rng.bool() {
                        break;
                    }
                }
            }
            BattleMage => {
                if attacker.rounds_started == 1 {
                    if defender.class == Mage {
                        logger.log(BE::CometRepelled(attacker, defender));
                    } else {
                        let dmg = match defender.class {
                            Mage => 0,
                            Bard => attacker.max_hp / 10,
                            Scout | Assassin | Berserker | Necromancer
                            | DemonHunter => attacker.max_hp / 5,
                            Warrior | BattleMage | Druid => attacker.max_hp / 4,
                            Paladin => (attacker.max_hp as f64 / (10.0 / 3.0))
                                .trunc()
                                as i64,
                        };
                        let dmg = dmg.min(defender.max_hp / 3);
                        logger.log(BE::CometAttack(attacker, defender));
                        // TODO: Can you dodge this?
                        do_damage(attacker, defender, dmg, rng, logger);
                    }
                }
                attack(attacker, defender, rng, Weapon, turn, logger);
            }
            Druid => {
                // Check if we do a sweep attack
                if !matches!(
                    attacker.class_effect,
                    ClassEffect::Druid { bear: true, .. }
                ) {
                    let swoops = attacker.class_effect.druid_swoops();
                    let swoop_chance =
                        0.15 + ((f32::from(swoops) * 5.0) / 100.0);
                    if defender.class != Class::Mage
                        && rng.f32() <= swoop_chance
                    {
                        attack(attacker, defender, rng, Swoop, turn, logger);
                        attacker.class_effect = ClassEffect::Druid {
                            bear: false,
                            // max 7 to limit chance to 50%
                            swoops: (swoops + 1).min(7),
                        }
                    }
                }

                attack(attacker, defender, rng, Weapon, turn, logger);
                // TODO: Does this reset here, or on the start of the next
                // attack?
                attacker.class_effect = ClassEffect::Druid {
                    bear: false,
                    swoops: attacker.class_effect.druid_swoops(),
                };
            }
            Bard => {
                // Start a new melody every 4 turns
                if attacker.rounds_started % 4 == 1 {
                    let quality = rng.u8(0..4);
                    let (quality, remaining) = match quality {
                        0 => (HarpQuality::Bad, 3),
                        1 | 2 => (HarpQuality::Medium, 3),
                        _ => (HarpQuality::Good, 4),
                    };
                    attacker.class_effect =
                        ClassEffect::Bard { quality, remaining };
                    logger.log(BE::BardPlay(attacker, defender, quality));
                }
                attack(attacker, defender, rng, Weapon, turn, logger);
                if let ClassEffect::Bard { remaining, .. } =
                    &mut attacker.class_effect
                {
                    *remaining = remaining.saturating_sub(1);
                }
            }
            Necromancer => {
                let has_minion = matches!(
                    attacker.class_effect,
                    ClassEffect::Necromancer { remaining: 1.., .. }
                );
                if !has_minion && defender.class != Class::Mage && rng.bool() {
                    let (typ, rem) = match rng.u8(0..3) {
                        0 => (Minion::Skeleton { revived: 0 }, 3),
                        1 => (Minion::Hound, 2),
                        _ => (Minion::Golem, 4),
                    };
                    attacker.class_effect = ClassEffect::Necromancer {
                        typ,
                        remaining: rem,
                    };
                    logger.log(BE::MinionSpawned(attacker, defender, typ));
                    attack(
                        attacker,
                        defender,
                        rng,
                        AttackType::Minion,
                        turn,
                        logger,
                    );
                } else {
                    if has_minion {
                        attack(
                            attacker,
                            defender,
                            rng,
                            AttackType::Minion,
                            turn,
                            logger,
                        );
                    }
                    attack(attacker, defender, rng, Weapon, turn, logger);
                }
                if let ClassEffect::Necromancer { remaining, typ } =
                    &mut attacker.class_effect
                {
                    if *remaining > 0 {
                        let mut has_revived = false;
                        if let Minion::Skeleton { revived } = typ {
                            if *revived < 2 && self.rng.bool() {
                                *revived += 1;
                                has_revived = true;
                            }
                        }
                        if has_revived {
                            // TODO: this revives for one turn, right?
                            *remaining = 1;
                            logger.log(BE::MinionSkeletonRevived(
                                attacker, defender,
                            ));
                        } else {
                            *remaining -= 1;
                        }
                    }
                }
            }
        }
        if defender.current_hp <= 0 {
            match attacking_side {
                Left => {
                    self.right.current_fighter += 1;
                    logger.log(BE::FighterDefeat(self, Right));
                }
                Right => {
                    self.left.current_fighter += 1;
                    logger.log(BE::FighterDefeat(self, Left));
                }
            }
        }
        None
    }
}

// Does the specified amount of damage to the target. The only special thing
// this does is revive demon hunters
fn do_damage(
    from: &mut BattleFighter,
    to: &mut BattleFighter,
    damage: i64,
    rng: &mut Rng,
    logger: &mut impl BattleLogger,
) {
    to.current_hp -= damage;
    logger.log(BE::DamageReceived(from, to, damage));

    if to.current_hp > 0 {
        return;
    }
    let ClassEffect::DemonHunter { revived } = &mut to.class_effect else {
        return;
    };
    let (chance, hp_restore) = match revived {
        0 => (0.44, 0.9),
        1 => (0.33, 0.8),
        2 => (0.22, 0.7),
        3 => (0.11, 0.6),
        _ => return,
    };

    if rng.f32() >= chance {
        return;
    }

    // The demon hunter revived
    to.current_hp = (hp_restore * to.max_hp as f64) as i64;
    *revived += 1;
    logger.log(BE::DemonHunterRevived(from, to));
}

fn attack(
    attacker: &mut BattleFighter,
    defender: &mut BattleFighter,
    rng: &mut Rng,
    typ: AttackType,
    turn: u32,
    logger: &mut impl BattleLogger,
) {
    if defender.current_hp <= 0 {
        // Skip pointless attacks
        return;
    }

    logger.log(BE::Attack(attacker, defender, typ));
    // Check dodges
    if attacker.class != Class::Mage {
        // Druid has 35% dodge chance
        if defender.class == Class::Druid && rng.f32() <= 0.35 {
            // TODO: is this instant, or does this trigger on start of def.
            // turn?
            defender.class_effect = ClassEffect::Druid {
                bear: true,
                swoops: defender.class_effect.druid_swoops(),
            };
            logger.log(BE::Dodged(attacker, defender));
        }
        // Scout and assassin have 50% dodge chance
        if (defender.class == Class::Scout || defender.class == Class::Assassin)
            && rng.bool()
        {
            logger.log(BE::Dodged(attacker, defender));
            return;
        }
        if defender.class == Class::Warrior
            && !defender.is_companion
            && defender.equip.offhand.0 as f32 / 100.0 > rng.f32()
        {
            // defender blocked
            logger.log(BE::Blocked(attacker, defender));
            return;
        }
    }

    // TODO: Most of this can be reused, as long as the opponent does not
    // change. Should make sure this is correct first though
    let char_damage_modifier = 1.0
        + f64::from(*attacker.attributes.get(attacker.class.main_attribute()))
            / 10.0;

    let mut elemental_bonus = 1.0;
    for element in Element::iter() {
        let plus = attacker.equip.element_dmg.get(element);
        let minus = defender.equip.element_dmg.get(element);

        if plus > minus {
            elemental_bonus += plus - minus;
        }
    }

    let armor = f64::from(defender.equip.armor) * defender.class.armor_factor();
    let max_dr = defender.class.max_damage_reduction();
    // TODO: Is this how mage armor negate works?
    let armor_damage_effect = if attacker.class == Class::Mage {
        1.0
    } else {
        1.0 - (armor / f64::from(attacker.level)).min(max_dr)
    };

    // The damage bonus you get from some class specific gimmic
    let class_effect_dmg_bonus = match attacker.class_effect {
        ClassEffect::Bard { quality, .. } if defender.class != Class::Mage => {
            match quality {
                HarpQuality::Bad => 1.2,
                HarpQuality::Medium => 1.4,
                HarpQuality::Good => 1.6,
            }
        }
        ClassEffect::Necromancer {
            typ: minion_type,
            remaining: 1..,
        } if typ == AttackType::Minion => match minion_type {
            Minion::Skeleton { .. } => 1.25,
            Minion::Hound => 2.0,
            Minion::Golem => 1.0,
        },
        ClassEffect::Druid { .. } if typ == AttackType::Swoop => 1.8,
        _ => 1.0,
    };

    // TODO: Is this the correct formula
    let rage_bonus = 1.0 + (f64::from(turn.saturating_sub(1)) / 6.0);

    let damage_bonus = char_damage_modifier
        * attacker.portal_dmg_bonus
        * elemental_bonus
        * armor_damage_effect
        * attacker.class.damage_factor(defender.class)
        * rage_bonus
        * class_effect_dmg_bonus;

    // FIXME: Is minion damage based on weapon, or unarmed damage?
    let weapon = match typ {
        AttackType::Offhand => attacker.equip.offhand,
        _ => attacker.equip.weapon,
    };

    let calc_damage =
        |weapon_dmg| (f64::from(weapon_dmg) * damage_bonus).trunc() as i64;

    let min_base_damage = calc_damage(weapon.0);
    let max_base_damage = calc_damage(weapon.1);

    let mut damage = rng.i64(min_base_damage..=max_base_damage);

    // Crits

    let luck_mod = attacker.attributes.get(AttributeType::Luck) * 5;
    let raw_crit_chance = f64::from(luck_mod) / f64::from(defender.level);
    let mut crit_chance = raw_crit_chance.min(0.5);
    let mut crit_dmg_factor = 2.0;

    match attacker.class_effect {
        ClassEffect::Druid { bear: true, .. } => {
            crit_chance += 0.1;
            crit_dmg_factor += 2.0;
        }
        ClassEffect::Necromancer {
            typ: Minion::Hound, ..
        } => {
            crit_chance += 0.1;
            crit_dmg_factor += 0.5;
        }
        _ => {}
    }

    if rng.f64() <= crit_chance {
        if attacker.equip.extra_crit_dmg {
            crit_dmg_factor += 0.05;
        };
        logger.log(BE::Crit(attacker, defender));
        damage = (damage as f64 * crit_dmg_factor) as i64;
    }

    do_damage(attacker, defender, damage, rng, logger);
}

#[derive(Debug)]
pub struct PlayerFighterSquad {
    pub character: UpgradeableFighter,
    pub companions: Option<EnumMap<CompanionClass, UpgradeableFighter>>,
}

impl PlayerFighterSquad {
    #[must_use]
    pub fn new(gs: &GameState) -> PlayerFighterSquad {
        let mut pet_attribute_bonus_perc = EnumMap::default();
        if let Some(pets) = &gs.pets {
            for (typ, info) in &pets.habitats {
                let mut total_bonus = 0;
                for pet in &info.pets {
                    total_bonus += match pet.level {
                        0 => 0,
                        1..100 => 100,
                        100..150 => 150,
                        150..200 => 175,
                        200.. => 200,
                    };
                }
                *pet_attribute_bonus_perc.get_mut(typ.into()) =
                    f64::from(total_bonus / 100) / 100.0;
            }
        };
        let portal_hp_bonus = gs
            .dungeons
            .portal
            .as_ref()
            .map(|a| a.player_hp_bonus)
            .unwrap_or_default()
            .into();
        let portal_dmg_bonus = gs
            .guild
            .as_ref()
            .map(|a| a.portal.damage_bonus)
            .unwrap_or_default()
            .into();

        let char = &gs.character;
        let character = UpgradeableFighter {
            is_companion: false,
            level: char.level,
            class: char.class,
            attribute_basis: char.attribute_basis,
            equipment: char.equipment.clone(),
            active_potions: char.active_potions,
            pet_attribute_bonus_perc,
            portal_hp_bonus,
            portal_dmg_bonus,
        };
        let mut companions = None;
        if let Some(comps) = &gs.dungeons.companions {
            let classes = [
                CompanionClass::Warrior,
                CompanionClass::Mage,
                CompanionClass::Scout,
            ];

            let res = classes.map(|class| {
                let comp = comps.get(class);
                UpgradeableFighter {
                    is_companion: true,
                    level: comp.level.try_into().unwrap_or(1),
                    class: class.into(),
                    attribute_basis: comp.attributes,
                    equipment: comp.equipment.clone(),
                    active_potions: char.active_potions,
                    pet_attribute_bonus_perc,
                    portal_hp_bonus,
                    portal_dmg_bonus,
                }
            });
            companions = Some(EnumMap::from_array(res));
        }

        PlayerFighterSquad {
            character,
            companions,
        }
    }
}

impl UpgradeableFighter {
    #[must_use]
    pub fn attributes(&self) -> EnumMap<AttributeType, u32> {
        let mut total = EnumMap::default();

        for equip in self.equipment.0.iter().flat_map(|a| a.1) {
            for (k, v) in &equip.attributes {
                *total.get_mut(k) += v;
            }

            if let Some(GemSlot::Filled(gem)) = &equip.gem_slot {
                use AttributeType as AT;
                let mut value = gem.value;
                if matches!(equip.typ, ItemType::Weapon { .. })
                    && !self.is_companion
                {
                    value *= 2;
                }

                let mut add_atr = |at| *total.get_mut(at) += value;
                match gem.typ {
                    GemType::Strength => add_atr(AT::Strength),
                    GemType::Dexterity => add_atr(AT::Dexterity),
                    GemType::Intelligence => add_atr(AT::Intelligence),
                    GemType::Constitution => add_atr(AT::Constitution),
                    GemType::Luck => add_atr(AT::Luck),
                    GemType::All => {
                        total.iter_mut().for_each(|a| *a.1 += value);
                    }
                    GemType::Legendary => {
                        add_atr(AT::Constitution);
                        add_atr(self.class.main_attribute());
                    }
                }
            }
        }

        let class_bonus: f64 = match self.class {
            Class::BattleMage => 0.1111,
            _ => 0.0,
        };

        let pet_boni = self.pet_attribute_bonus_perc;

        for (k, v) in &mut total {
            let class_bonus = (f64::from(*v) * class_bonus).trunc() as u32;
            *v += class_bonus + self.attribute_basis.get(k);
            if let Some(potion) = self
                .active_potions
                .iter()
                .flatten()
                .find(|a| a.typ == k.into())
            {
                *v += (f64::from(*v) * potion.size.effect()) as u32;
            }

            let pet_bonus = (f64::from(*v) * (*pet_boni.get(k))).trunc() as u32;
            *v += pet_bonus;
        }
        total
    }

    #[must_use]
    #[allow(clippy::enum_glob_use)]
    pub fn hit_points(&self, attributes: &EnumMap<AttributeType, u32>) -> i64 {
        use Class::*;

        let mut total = i64::from(*attributes.get(AttributeType::Constitution));
        total = (total as f64
            * match self.class {
                Warrior if self.is_companion => 6.1,
                Paladin => 6.0,
                Warrior | BattleMage | Druid => 5.0,
                Scout | Assassin | Berserker | DemonHunter | Necromancer => 4.0,
                Mage | Bard => 2.0,
            })
        .trunc() as i64;

        total *= i64::from(self.level) + 1;

        if self
            .active_potions
            .iter()
            .flatten()
            .any(|a| a.typ == PotionType::EternalLife)
        {
            total = (total as f64 * 1.25).trunc() as i64;
        }

        let portal_bonus = (total as f64
            * (f64::from(self.portal_hp_bonus) / 100.0))
            .trunc() as i64;

        total += portal_bonus;

        let mut rune_multi = 0;
        for rune in self
            .equipment
            .0
            .iter()
            .flat_map(|a| a.1)
            .filter_map(|a| a.rune)
        {
            if rune.typ == RuneType::ExtraHitPoints {
                rune_multi += u32::from(rune.value);
            }
        }

        let rune_bonus =
            (total as f64 * (f64::from(rune_multi) / 100.0)).trunc() as i64;

        total += rune_bonus;
        total
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monster {
    pub level: u16,
    pub class: Class,
    pub attributes: EnumMap<AttributeType, u32>,
    pub hp: u64,
    pub xp: u32,
}

impl Monster {
    #[must_use]
    pub const fn new(
        level: u16,
        class: Class,
        attribs: [u32; 5],
        hp: u64,
        xp: u32,
    ) -> Self {
        Monster {
            level,
            class,
            attributes: EnumMap::from_array(attribs),
            hp,
            xp,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum BattleEvent<'a, 'b> {
    TurnUpdate(&'a Battle<'b>),
    BattleEnd(&'a Battle<'b>, BattleSide),
    Attack(&'b BattleFighter, &'b BattleFighter, AttackType),
    Dodged(&'b BattleFighter, &'b BattleFighter),
    Blocked(&'b BattleFighter, &'b BattleFighter),
    Crit(&'b BattleFighter, &'b BattleFighter),
    DamageReceived(&'b BattleFighter, &'b BattleFighter, i64),
    DemonHunterRevived(&'b BattleFighter, &'b BattleFighter),
    CometRepelled(&'b BattleFighter, &'b BattleFighter),
    CometAttack(&'b BattleFighter, &'b BattleFighter),
    MinionSpawned(&'b BattleFighter, &'b BattleFighter, Minion),
    MinionSkeletonRevived(&'b BattleFighter, &'b BattleFighter),
    BardPlay(&'b BattleFighter, &'b BattleFighter, HarpQuality),
    FighterDefeat(&'a Battle<'b>, BattleSide),
}

pub trait BattleLogger {
    fn log(&mut self, event: BattleEvent<'_, '_>);
}

impl BattleLogger for () {
    fn log(&mut self, _event: BattleEvent<'_, '_>) {
    }
}
