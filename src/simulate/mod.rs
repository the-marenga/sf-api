#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
use std::sync::Arc;

use enum_map::{Enum, EnumMap};
use fastrand::Rng;
use strum::EnumIter;

use crate::{
    command::AttributeType,
    gamestate::{
        GameState, character::Class, dungeons::CompanionClass, items::*,
        social::OtherPlayer, underworld::UnderworldBuildingType,
    },
    misc::EnumMapGet,
};

pub mod constants;

use BattleEvent as BE;

#[derive(Debug, Clone)]
pub struct UpgradeableFighter {
    pub name: Arc<str>,
    is_companion: bool,
    level: u16,
    class: Class,
    /// The base attributes without any equipment, | other boosts
    pub attribute_basis: EnumMap<AttributeType, u32>,
    pet_attribute_bonus_perc: EnumMap<AttributeType, f64>,

    equipment: Equipment,
    active_potions: [Option<Potion>; 3],
    /// This should be the percentage bonus to skills from pets
    /// The hp bonus in percent this player has from the personal demon portal
    portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    portal_dmg_bonus: u32,
    /// The level of the gladiator in the underworld
    gladiator_lvl: u8,
}

impl UpgradeableFighter {
    /// Inserts a gem on the item in the specified slot
    /// If the gem could be inserted the old gem (if any) will be returned
    /// # Errors
    ///
    /// Will return `Err` if the gem could not be inserted. It will contain
    /// the gem you tried to insert
    pub fn insert_gem(
        &mut self,
        gem: Gem,
        slot: EquipmentSlot,
    ) -> Result<Option<Gem>, Gem> {
        let Some(item) = self.equipment.0.get_mut(slot).as_mut() else {
            return Err(gem);
        };
        let Some(gem_slot) = &mut item.gem_slot else {
            return Err(gem);
        };

        let old_gem = match *gem_slot {
            GemSlot::Filled(gem) => Some(gem),
            GemSlot::Empty => None,
        };
        *gem_slot = GemSlot::Filled(gem);
        Ok(old_gem)
    }

    /// Removes the gem at the provided slot and returns the old gem, if
    /// any
    pub fn extract_gem(&mut self, slot: EquipmentSlot) -> Option<Gem> {
        let item = self.equipment.0.get_mut(slot).as_mut()?;
        let gem_slot = &mut item.gem_slot?;

        let old_gem = match *gem_slot {
            GemSlot::Filled(gem) => Some(gem),
            GemSlot::Empty => None,
        };
        *gem_slot = GemSlot::Empty;
        old_gem
    }

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

    /// Removes the potion at the provided slot and returns the old potion, if
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

        if (self.is_companion && !item.can_be_equipped_by_companion(self.class))
            || (!self.is_companion && !item.can_be_equipped_by(self.class))
        {
            return Err(item);
        }

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
            name: other.name.as_str().into(),
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
            // TODO: actually parse and set this here
            gladiator_lvl: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Minion {
    Skeleton { revived: u8 },
    Hound,
    Golem,
}

#[derive(Debug, Clone)]
pub struct BattleFighter {
    pub name: Arc<str>,
    pub is_companion: bool,
    pub level: u16,
    pub class: Class,
    pub attributes: EnumMap<AttributeType, u32>,
    pub max_hp: i64,
    pub current_hp: i64,
    pub equip: EquipmentEffects,
    pub portal_dmg_bonus: f64,
    pub gladiator_lvl: u8,
    pub class_effect: ClassEffect,
}

impl std::hash::Hash for BattleFighter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (
            self.is_companion,
            self.level,
            self.class,
            &self.attributes,
            self.max_hp,
            self.current_hp,
            &self.equip,
            (self.portal_dmg_bonus * 100.0) as u32,
            &self.class_effect,
        )
            .hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    BattleMage,
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
        let mut rune_atk_bonus = None;
        let mut element_res = EnumMap::default();

        if let Some(runes) = &monster.runes {
            rune_atk_bonus = Some((runes.damage_type, runes.damage));
            element_res = runes.resistences;
        }

        Self {
            name: monster.name.clone(),
            is_companion: false,
            level: monster.level,
            class: monster.class,
            attributes: monster.attributes,
            max_hp: monster.hp as i64,
            current_hp: monster.hp as i64,
            equip: EquipmentEffects {
                element_res,
                weapon: Some(Weapon {
                    min_dmg: monster.min_dmg,
                    max_dmg: monster.max_dmg,
                    rune_atk_bonus,
                }),
                offhand: None,
                has_shield: monster.class.can_wear_shield(),
                reaction_boost: false,
                extra_crit_dmg: false,
                armor: monster.armor,
            },
            portal_dmg_bonus: 1.0,
            class_effect: ClassEffect::Normal,
            gladiator_lvl: 0,
        }
    }

    #[must_use]
    pub fn from_upgradeable(char: &UpgradeableFighter) -> Self {
        let attributes = char.attributes();
        let hp = char.hit_points(&attributes);

        let mut equip = EquipmentEffects {
            element_res: EnumMap::default(),
            reaction_boost: false,
            extra_crit_dmg: false,
            armor: 0,
            weapon: None,
            offhand: None,
            has_shield: false,
        };

        for (slot, item) in &char.equipment.0 {
            let Some(item) = item else {
                match slot {
                    EquipmentSlot::Weapon => {
                        let (min, max) =
                            calc_unarmed_base_dmg(slot, char.level, char.class);
                        equip.weapon = Some(Weapon {
                            min_dmg: min,
                            max_dmg: max,
                            rune_atk_bonus: None,
                        });
                    }
                    EquipmentSlot::Shield if char.class == Class::Assassin => {
                        let (min, max) =
                            calc_unarmed_base_dmg(slot, char.level, char.class);
                        equip.offhand = Some(Weapon {
                            min_dmg: min,
                            max_dmg: max,
                            rune_atk_bonus: None,
                        });
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
            }
            let mut rune_atk_bonus = None;
            if let Some(rune) = item.rune {
                use RuneType as RT;

                let mut apply_res = |element| {
                    *equip.element_res.get_mut(element) +=
                        i32::from(rune.value);
                };
                let mut apply_dmg = |element| {
                    rune_atk_bonus = Some((element, i32::from(rune.value)));
                };

                match rune.typ {
                    RT::FireResistance => apply_res(Element::Fire),
                    RT::ColdResistence => apply_res(Element::Cold),
                    RT::LightningResistance => apply_res(Element::Lightning),
                    RT::TotalResistence => {
                        for (_, val) in &mut equip.element_res {
                            *val += i32::from(rune.value);
                        }
                    }
                    RT::FireDamage => apply_dmg(Element::Fire),
                    RT::ColdDamage => apply_dmg(Element::Cold),
                    RT::LightningDamage => apply_dmg(Element::Lightning),
                    _ => {}
                }
            }

            match item.typ {
                ItemType::Weapon { min_dmg, max_dmg } => {
                    let weapon = Some(Weapon {
                        min_dmg,
                        max_dmg,
                        rune_atk_bonus,
                    });
                    match slot {
                        EquipmentSlot::Weapon => equip.weapon = weapon,
                        EquipmentSlot::Shield => equip.offhand = weapon,
                        _ => {}
                    }
                }
                ItemType::Shield { .. } => {
                    equip.has_shield = true;
                }
                _ => (),
            }
        }

        let portal_dmg_bonus = 1.0 + f64::from(char.portal_dmg_bonus) / 100.0;

        BattleFighter {
            name: char.name.clone(),
            is_companion: char.is_companion,
            class: char.class,
            attributes,
            max_hp: hp,
            current_hp: hp,
            equip,
            class_effect: ClassEffect::Normal,
            portal_dmg_bonus,
            level: char.level,
            gladiator_lvl: char.gladiator_lvl,
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
    }
}

#[derive(Debug, Clone, Hash)]
pub struct EquipmentEffects {
    element_res: EnumMap<Element, i32>,

    weapon: Option<Weapon>,
    offhand: Option<Weapon>,
    has_shield: bool,

    /// Shadow of the cowboy
    reaction_boost: bool,
    /// Sword of Vengeance
    extra_crit_dmg: bool,

    armor: u32,
}

#[derive(Debug, Clone, Copy, Hash, Default)]
struct Weapon {
    min_dmg: u32,
    max_dmg: u32,
    rune_atk_bonus: Option<(Element, i32)>,
}

#[derive(Debug, Clone, Copy, Enum, EnumIter, Hash, PartialEq, Eq)]
pub enum Element {
    Fire,
    Cold,
    Lightning,
}

#[derive(Debug)]
pub struct BattleTeam<'a> {
    current_fighter: usize,
    fighters: &'a mut [BattleFighter],
}

#[allow(clippy::extra_unused_lifetimes)]
impl<'a> BattleTeam<'_> {
    #[must_use]
    pub fn current(&self) -> Option<(&BattleFighter, usize)> {
        let idx = self.current_fighter;
        self.fighters.get(idx).map(|a| (a, idx))
    }

    #[must_use]
    pub fn current_mut(&mut self) -> Option<(&mut BattleFighter, usize)> {
        let idx = self.current_fighter;
        self.fighters.get_mut(idx).map(|a| (a, idx))
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
    /// Details about the combat between the two fighters currently fighting.
    /// If they have not started yet, this will be None
    pub current_fight: Option<FightDetails>,
    pub left: BattleTeam<'a>,
    pub right: BattleTeam<'a>,
    pub rng: Rng,
}

#[derive(Debug)]
pub struct FightDetails {
    /// The idx of the fighters in their respective teams. This is used to
    /// invalidate the engagement, once they change
    pub fighter_pos: EnumMap<BattleSide, usize>,
    /// The side, that has started this engagement.
    pub started: BattleSide,
    /// The amount of time the side, that can start attacks have swapped
    pub side_swaps: u32,
    /// Mostly the amount of attacks, that have taken place. This is also
    /// refered to as turns by the game, but turns may be confused with
    /// swapping sides, or the both sides trading blows, depending on what
    /// preconceived notions you have, In addition, stuff like summoning also
    /// wastes "turns", but casting a meteor does not, so this is just called
    /// `rage_lvl`, since it is too far removed from turns to be called that
    pub rage_lvl: u32,
}

impl<'a> Battle<'a> {
    pub fn new(
        left: &'a mut [BattleFighter],
        right: &'a mut [BattleFighter],
    ) -> Self {
        Self {
            left: BattleTeam {
                current_fighter: 0,
                fighters: left,
            },
            right: BattleTeam {
                current_fighter: 0,
                fighters: right,
            },
            rng: fastrand::Rng::default(),
            current_fight: None,
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
        self.current_fight = None;
        self.left.reset();
        self.right.reset();
    }

    /// Simulates one turn (attack) in a battle. If one side is not able
    /// to fight anymore, | is for another reason invalid, the other side is
    /// returned as the winner
    pub fn simulate_turn(
        &mut self,
        logger: &mut impl BattleLogger,
    ) -> Option<BattleSide> {
        use AttackType::{Offhand, Swoop, Weapon};
        use BattleSide::{Left, Right};
        use Class::{
            Assassin, Bard, BattleMage, Berserker, DemonHunter, Druid, Mage,
            Necromancer, Paladin, PlagueDoctor, Scout, Warrior,
        };

        logger.log(BE::TurnUpdate(self));

        let Some((left, left_idx)) = self.left.current_mut() else {
            logger.log(BE::BattleEnd(self, Right));
            return Some(Right);
        };
        let Some((right, right_idx)) = self.right.current_mut() else {
            logger.log(BE::BattleEnd(self, Left));
            return Some(Left);
        };

        let mut start_fight = || {
            // The battle has not yet started. Figure out who side starts
            let starting_side =
                match (right.equip.reaction_boost, left.equip.reaction_boost) {
                    (true, false) => Right,
                    (true, true) | (false, false) if self.rng.bool() => Right,
                    _ => Left,
                };

            FightDetails {
                fighter_pos: EnumMap::from_array([left_idx, right_idx]),
                started: starting_side,
                side_swaps: 0,
                rage_lvl: 0,
            }
        };

        let fight = match &mut self.current_fight {
            Some(fight)
                if fight.fighter_pos.as_slice() == [left_idx, right_idx] =>
            {
                fight.side_swaps += 1;
                fight
            }
            _ => self.current_fight.insert(start_fight()),
        };

        // If We are at the same cycle, as the first turn, the one that
        // started on the first turn starts here. Otherwise the other
        // one
        let attacking_side = match fight.started {
            x if fight.side_swaps % 2 == 0 => x,
            Left => Right,
            Right => Left,
        };

        let (attacker, defender) = match attacking_side {
            Left => (left, right),
            Right => (right, left),
        };

        let rage_lvl = &mut fight.rage_lvl;
        let rng = &mut self.rng;
        match attacker.class {
            Paladin | PlagueDoctor => {
                // TODO: Actually implement stances and stuff
                attack(attacker, defender, rng, Weapon, rage_lvl, logger);
            }
            Warrior | Scout | Mage | DemonHunter => {
                attack(attacker, defender, rng, Weapon, rage_lvl, logger);
            }
            Assassin => {
                attack(attacker, defender, rng, Weapon, rage_lvl, logger);
                attack(attacker, defender, rng, Offhand, rage_lvl, logger);
            }
            Berserker => {
                for _ in 0..15 {
                    attack(attacker, defender, rng, Weapon, rage_lvl, logger);
                    if defender.current_hp <= 0 || rng.bool() {
                        break;
                    }
                }
            }
            BattleMage => {
                if attacker.class_effect == ClassEffect::Normal {
                    attacker.class_effect = ClassEffect::BattleMage;

                    if defender.class == Mage {
                        logger.log(BE::CometRepelled(attacker, defender));
                    } else {
                        let dmg = match defender.class {
                            Mage => 0,
                            Bard => attacker.max_hp / 10,
                            Scout | Assassin | Berserker | Necromancer
                            | DemonHunter | PlagueDoctor => attacker.max_hp / 5,
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
                attack(attacker, defender, rng, Weapon, rage_lvl, logger);
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
                        attack(
                            attacker, defender, rng, Swoop, rage_lvl, logger,
                        );
                        attacker.class_effect = ClassEffect::Druid {
                            bear: false,
                            // max 7 to limit chance to 50%
                            swoops: (swoops + 1).min(7),
                        }
                    }
                }

                attack(attacker, defender, rng, Weapon, rage_lvl, logger);
                // TODO: Does this reset here, | on the start of the next
                // attack?
                attacker.class_effect = ClassEffect::Druid {
                    bear: false,
                    swoops: attacker.class_effect.druid_swoops(),
                };
            }
            Bard => {
                // Start a new melody every 4 turns
                if (fight.side_swaps / 2) % 4 == 0 {
                    let quality = rng.u8(0..4);
                    let (quality, mut remaining) = match quality {
                        0 => (HarpQuality::Bad, 1),
                        1 | 2 => (HarpQuality::Medium, 1),
                        _ => (HarpQuality::Good, 2),
                    };

                    let inteligence =
                        *attacker.attributes.get(AttributeType::Intelligence);
                    let constitution =
                        *attacker.attributes.get(AttributeType::Constitution);

                    if constitution >= inteligence / 2 {
                        remaining += 1;
                    }
                    if constitution >= 3 * inteligence / 4 {
                        remaining += 1;
                    }

                    attacker.class_effect =
                        ClassEffect::Bard { quality, remaining };
                    logger.log(BE::BardPlay(attacker, defender, quality));
                }
                attack(attacker, defender, rng, Weapon, rage_lvl, logger);
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
                        rage_lvl,
                        logger,
                    );
                } else {
                    if has_minion {
                        attack(
                            attacker,
                            defender,
                            rng,
                            AttackType::Minion,
                            rage_lvl,
                            logger,
                        );
                    }
                    attack(attacker, defender, rng, Weapon, rage_lvl, logger);
                }
                if let ClassEffect::Necromancer { remaining, typ } =
                    &mut attacker.class_effect
                    && *remaining > 0
                {
                    let mut has_revived = false;
                    if let Minion::Skeleton { revived } = typ
                        && *revived < 2
                        && self.rng.bool()
                    {
                        *revived += 1;
                        has_revived = true;
                    }
                    if has_revived {
                        // TODO: this revives for one turn, right?
                        *remaining = 1;
                        logger
                            .log(BE::MinionSkeletonRevived(attacker, defender));
                    } else {
                        *remaining -= 1;
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
    rage_lvl: &mut u32,
    logger: &mut impl BattleLogger,
) {
    *rage_lvl += 1;

    if defender.current_hp <= 0 {
        // Skip pointless attacks
        return;
    }

    logger.log(BE::Attack(attacker, defender, typ));
    // Check dodges
    if attacker.class != Class::Mage {
        // Druid has 35% dodge chance
        if defender.class == Class::Druid && rng.f32() <= 0.35 {
            // TODO: is this instant, | does this trigger on start of def.
            // turn?
            defender.class_effect = ClassEffect::Druid {
                bear: true,
                swoops: defender.class_effect.druid_swoops(),
            };
            logger.log(BE::Dodged(attacker, defender));
            return;
        }
        // Scout and assassin have 50% dodge chance
        if (defender.class == Class::Scout || defender.class == Class::Assassin)
            && rng.bool()
        {
            logger.log(BE::Dodged(attacker, defender));
            return;
        }
        if defender.equip.has_shield
            && defender.class.block_chance() > rng.f32()
        {
            logger.log(BE::Blocked(attacker, defender));
            return;
        }
    }

    // FIXME: Is minion damage based on weapon, or unarmed damage?
    let weapon = match typ {
        AttackType::Offhand => attacker.equip.offhand,
        _ => attacker.equip.weapon,
    }
    .unwrap_or_default();

    let mut elemental_bonus = 1.0;
    if let Some((element, atk_bonus)) = weapon.rune_atk_bonus {
        let resistance = f64::from(*defender.equip.element_res.get(element));
        let resistance = 1.0 - resistance.min(70.0) / 100.0;
        let atk_bonus = f64::from(atk_bonus.min(60)) / 100.0;
        elemental_bonus += resistance * atk_bonus;
    }

    let armor_damage_effect = if attacker.class == Class::Mage {
        1.0
    } else {
        let max_dr = defender.class.max_damage_reduction();
        let armor =
            f64::from(defender.equip.armor) * defender.class.armor_factor();
        let raw_dr = armor / f64::from(attacker.level);
        let dr = (raw_dr / 100.0).min(max_dr);
        1.0 - dr
    };

    // The damage bonus you get from some class specific gimmic
    let class_effect_dmg_bonus = match attacker.class_effect {
        ClassEffect::Bard {
            quality,
            remaining: 1..,
        } if defender.class != Class::Mage => match quality {
            HarpQuality::Bad => 1.2,
            HarpQuality::Medium => 1.4,
            HarpQuality::Good => 1.6,
        },
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

    let rage_bonus = 1.0 + (f64::from(rage_lvl.saturating_sub(1)) / 6.0);

    // TODO: Most of this can be reused, as long as the opponent does not
    // change. Should make sure this is correct first though
    let main_atr = attacker.class.main_attribute();

    let attacker_skill = *attacker.attributes.get(main_atr);
    let defender_skill = *defender.attributes.get(main_atr);

    let effective_attacker_skill = (attacker_skill / 2)
        .max(attacker_skill.saturating_sub(defender_skill / 2));

    let attribute_bonus = 1.0 + f64::from(effective_attacker_skill) / 10.0;

    let damage_bonus = 1.0
        * attribute_bonus
        * attacker.portal_dmg_bonus
        * elemental_bonus
        * armor_damage_effect
        * attacker.class.damage_factor(defender.class)
        * rage_bonus
        * class_effect_dmg_bonus;

    let calc_damage =
        |weapon_dmg| (f64::from(weapon_dmg) * damage_bonus).trunc() as i64;

    let min_base_damage = calc_damage(weapon.min_dmg);
    let max_base_damage = calc_damage(weapon.max_dmg);

    let mut damage = rng.i64(min_base_damage..=max_base_damage);

    // Crits
    let luck_mod = attacker.attributes.get(AttributeType::Luck) * 5;
    let raw_crit_chance =
        (f64::from(luck_mod) / (f64::from(defender.level * 2))) / 100.0;
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

    if attacker.equip.extra_crit_dmg {
        crit_dmg_factor += 0.05;
    }
    let gladiator_lvl_diff = attacker
        .gladiator_lvl
        .saturating_sub(defender.gladiator_lvl);

    crit_dmg_factor += 0.11 * f64::from(gladiator_lvl_diff);

    if rng.f64() <= crit_chance {
        logger.log(BE::Crit(attacker, defender, crit_chance, crit_dmg_factor));
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
        }
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

        let gladiator_lvl = gs.underworld.as_ref().map_or(0, |a| {
            a.buildings[UnderworldBuildingType::GladiatorTrainer].level
        });

        let char = &gs.character;
        let character = UpgradeableFighter {
            name: gs.character.name.as_str().into(),
            is_companion: false,
            level: char.level,
            class: char.class,
            attribute_basis: char.attribute_basis,
            equipment: char.equipment.clone(),
            active_potions: char.active_potions,
            pet_attribute_bonus_perc,
            portal_hp_bonus,
            portal_dmg_bonus,
            gladiator_lvl,
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
                    name: "companion".into(),
                    is_companion: true,
                    level: comp.level.try_into().unwrap_or(1),
                    class: class.into(),
                    attribute_basis: comp.attributes,
                    equipment: comp.equipment.clone(),
                    active_potions: char.active_potions,
                    pet_attribute_bonus_perc,
                    portal_hp_bonus,
                    portal_dmg_bonus,
                    gladiator_lvl,
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
        let mut total = i64::from(*attributes.get(AttributeType::Constitution));
        total = (total as f64 * self.class.life_multiplier(self.is_companion))
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

#[derive(Debug)]
#[non_exhaustive]
pub enum BattleEvent<'a, 'b> {
    TurnUpdate(&'a Battle<'b>),
    BattleEnd(&'a Battle<'b>, BattleSide),
    Attack(&'b BattleFighter, &'b BattleFighter, AttackType),
    Dodged(&'b BattleFighter, &'b BattleFighter),
    Blocked(&'b BattleFighter, &'b BattleFighter),
    Crit(&'b BattleFighter, &'b BattleFighter, f64, f64),
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
