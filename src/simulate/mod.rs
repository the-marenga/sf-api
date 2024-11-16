#![allow(unused)]
use std::ops::Sub;

use enum_map::{Enum, EnumMap};
use fastrand::Rng;
use log::info;
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    command::AttributeType,
    gamestate::{
        character::{Class, DruidMask},
        dungeons::CompanionClass,
        items::{
            Enchantment, Equipment, EquipmentSlot, GemSlot, GemType, ItemType,
            Potion, PotionType, RuneType,
        },
        GameState,
    },
    misc::EnumMapGet,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BaseClass {
    Warrior = 0,
    Mage,
    Scout,
}

#[allow(clippy::enum_glob_use)]
impl From<Class> for BaseClass {
    fn from(value: Class) -> Self {
        use Class::*;
        match value {
            BattleMage | Berserker | Warrior => BaseClass::Warrior,
            Assassin | DemonHunter | Scout => BaseClass::Scout,
            Druid | Bard | Necromancer | Mage => BaseClass::Mage,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpgradeableFighter {
    is_companion: bool,
    level: u16,
    class: Class,
    /// The base attributes without any equipment, or other boosts
    pub attribute_basis: EnumMap<AttributeType, u32>,
    attributes_bought: EnumMap<AttributeType, u32>,
    pet_attribute_bonus_perc: EnumMap<AttributeType, f64>,

    equipment: Equipment,
    active_potions: [Option<Potion>; 3],
    /// This should be the percentage bonus to skills from pets
    /// The hp bonus in percent this player has from the personal demon portal
    portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    portal_dmg_bonus: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Minion {
    typ: MinionType,
    rounds_remaining: u8,
}

#[derive(Debug, Clone, Copy)]
pub enum MinionType {
    Skeleton,
    Hound,
    Golem,
}

#[derive(Debug, Clone)]
pub struct BattleFighter {
    pub is_companion: bool,
    pub level: u16,
    pub class: Class,
    pub attributes: EnumMap<AttributeType, u32>,
    pub max_hp: i32,
    pub current_hp: i32,
    pub equip: EquipmentEffects,
    pub portal_dmg_bonus: f64,
    pub rounds_in_battle: u32,
    pub class_effect: Option<ClassEffect>,
}

#[derive(Debug, Clone, Copy)]
pub enum HarpQuality {
    Bad,
    Medium,
    Good,
}

#[derive(Debug, Clone, Copy)]
pub enum ClassEffect {
    Druid(DruidMask),
    Bard(HarpQuality),
    Necromancer(Minion),
    DemonHunter { revived: u8 },
}

impl BattleFighter {
    #[must_use]
    pub fn from_upgradeable(char: &UpgradeableFighter) -> Self {
        let attributes = char.attributes();
        let hp = char.hit_points(&attributes) as i32;

        let mut equip = EquipmentEffects {
            element_res: EnumMap::default(),
            element_dmg: EnumMap::default(),
            reaction_boost: false,
            extra_crit_dmg: false,
            armor: 0,
            weapon: (0, 0),
            offhand: (0, 0),
        };

        // Modified, but mostly copied from:
        // https://github.com/HafisCZ/sf-tools/blob/521c2773098d62fe21ae687de2047c05f84813b7/js/sim/base.js#L746C4-L765C6
        let unarmed_dmg = |slot| {
            if char.level <= 10 {
                return (1, 2);
            }
            let dmg_level = f64::from(char.level - 9);
            let multiplier = match char.class {
                Class::Assassin if slot == EquipmentSlot::Weapon => 1.25,
                Class::Assassin => 0.875,
                _ => 0.7,
            };

            let base = dmg_level * multiplier * char.class.weapon_multiplier();
            let min = ((base * 2.0) / 3.0).trunc().max(1.0);
            let max = ((base * 4.0) / 3.0).trunc().max(2.0);
            (min as u32, max as u32)
        };

        for (slot, item) in &char.equipment.0 {
            let Some(item) = item else {
                match slot {
                    EquipmentSlot::Weapon => equip.weapon = unarmed_dmg(slot),
                    EquipmentSlot::Shield if char.class == Class::Assassin => {
                        equip.offhand = unarmed_dmg(slot)
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
            rounds_in_battle: 0,
            class_effect: None,
            portal_dmg_bonus,
            level: char.level,
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

#[derive(Debug, Clone)]
pub struct BattleTeam {
    current_fighter: usize,
    fighters: Vec<BattleFighter>,
}

impl BattleTeam {
    pub fn current(&mut self) -> Option<&mut BattleFighter> {
        self.fighters.get_mut(self.current_fighter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleSide {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct Battle {
    round: u32,
    started: Option<BattleSide>,
    left: BattleTeam,
    right: BattleTeam,
    rng: Rng,
}

impl Battle {
    pub fn new(left: Vec<BattleFighter>, right: Vec<BattleFighter>) -> Self {
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
    /// The result for invalid starting states (dead/damaged/no fighters) is
    /// undefined
    pub fn simulate(&mut self) -> BattleSide {
        loop {
            if let Some(winner) = self.simulate_turn() {
                return winner;
            }
        }
    }

    /// Simulates one turn (attack) in a battle. If one side is not able
    /// to fight anymore, or is for another reason invalid, the other side is
    /// returned as the winner
    fn simulate_turn(&mut self) -> Option<BattleSide> {
        use BattleSide::{Left, Right};
        use Class::{
            Assassin, Bard, BattleMage, Berserker, DemonHunter, Druid, Mage,
            Necromancer, Scout, Warrior,
        };

        let Some(mut left) = self.left.current() else {
            return Some(Right);
        };
        let Some(mut right) = self.right.current() else {
            return Some(Left);
        };

        self.round += 1;
        left.rounds_in_battle += 1;
        right.rounds_in_battle += 1;

        let attacking_side = if let Some(started) = self.started {
            let one_vs_one_round =
                left.rounds_in_battle.min(right.rounds_in_battle);

            // If We are at the same cycle, as the first turn, the one that
            // started on the first turn starts here. Otherwise the other one
            match started {
                _ if one_vs_one_round % 2 == 1 => started,
                Left => Right,
                Right => Left,
            }
        } else {
            // The battle has not yet started. Figure out who side starts
            let mut starter =
                match (right.equip.reaction_boost, left.equip.reaction_boost) {
                    (true, true) | (false, false) if self.rng.bool() => Right,
                    (true, false) => Right,
                    _ => Left,
                };
            self.started = Some(starter);
            starter
        };

        let (attacker, defender) = match attacking_side {
            Left => (left, right),
            Right => (right, left),
        };

        match attacker.class {
            Warrior | Scout | Mage | DemonHunter => {
                weapon_attack(attacker, defender, &mut self.rng, false)
            }
            Assassin => {
                weapon_attack(attacker, defender, &mut self.rng, false);
                weapon_attack(attacker, defender, &mut self.rng, true);
            }
            Berserker => {
                weapon_attack(attacker, defender, &mut self.rng, false);
                for attack_no in 0..14 {
                    if self.rng.bool() {
                        weapon_attack(attacker, defender, &mut self.rng, false);
                    } else {
                        break;
                    }
                }
            }
            BattleMage => {
                if attacker.rounds_in_battle == 1 && defender.class != Mage {
                    let dmg = match defender.class {
                        Mage => 0,
                        Bard => attacker.max_hp as u32 / 10,
                        Scout | Assassin | Berserker | Necromancer
                        | DemonHunter => attacker.max_hp as u32 / 5,
                        Warrior | BattleMage | Druid => {
                            attacker.max_hp as u32 / 4
                        }
                    };
                    if dmg > 0 {
                        do_damage(defender, dmg, &mut self.rng);
                    }
                }
                weapon_attack(attacker, defender, &mut self.rng, false)
            }
            Druid => todo!("All the forms"),
            Bard => todo!("Start Melodies"),
            Necromancer => todo!("Summon minions & do their stuff"),
        }
        if defender.current_hp <= 0 {
            match attacking_side {
                Left => self.right.current_fighter += 1,
                Right => self.left.current_fighter += 1,
            }
        }
        None
    }
}

// Does the specified amount of damage, whilst
fn do_damage(to: &mut BattleFighter, damage: u32, rng: &mut Rng) {
    if to.current_hp <= 0 {
        // Skip pointless attacks
        return;
    }
    to.current_hp -= damage as i32;
    if to.current_hp > 0 {
        return;
    }
    let Some(ClassEffect::DemonHunter { revived }) = &mut to.class_effect
    else {
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
    to.current_hp = (hp_restore * to.max_hp as f64) as i32;
    *revived += 1;
}

fn weapon_attack(
    attacker: &mut BattleFighter,
    defender: &mut BattleFighter,
    rng: &mut Rng,
    offhand: bool,
) {
    if attacker.class != Class::Mage {
        if defender.class == Class::Scout && rng.bool() {
            // defender dodged
            return;
        }
        if defender.class == Class::Warrior
            && !defender.is_companion
            && rng.bool()
        {
            // defender blocked
            return;
        }
    }

    // TODO: Most of this can be reused, as long as the opponent does not
    // change. Should make sure this is correct first though
    let char_damage_modifier = (1.0
        + (*attacker.attributes.get(attacker.class.main_attribute()) as f64)
            / 10.0);

    let mut elemental_bonus = 1.0;
    for element in Element::iter() {
        let plus = attacker.equip.element_dmg.get(element);
        let minus = defender.equip.element_dmg.get(element);

        if plus > minus {
            elemental_bonus += plus - minus;
        }
    }

    let def_reduction = ((defender.equip.armor as f64
        * defender.class.armor_factor())
        / attacker.level as f64)
        .min(defender.class.max_damage_reduction());

    // FIME: Check the order of all of this
    let damage_bonus = char_damage_modifier
        * attacker.portal_dmg_bonus
        * elemental_bonus
        * def_reduction
        * attacker.class.damage_factor();

    let weapon = if offhand {
        attacker.equip.offhand
    } else {
        attacker.equip.weapon
    };

    let calc_damage =
        |weapon_dmg| (weapon_dmg as f64 * damage_bonus).trunc() as u32;

    let min_base_damage = calc_damage(weapon.0);
    let max_base_damage = calc_damage(weapon.1);

    let mut damage = rng.u32(min_base_damage..=max_base_damage);

    let luck_mod = attacker.attributes.get(AttributeType::Luck) * 5;
    let raw_crit_chance = luck_mod as f64 / (defender.level as f64);
    let crit_chance = raw_crit_chance.min(0.5);

    if rng.f64() <= crit_chance {
        if attacker.equip.extra_crit_dmg {
            damage = (damage as f64 * 2.05) as u32;
        } else {
            damage *= 2;
        }
    }
    do_damage(defender, damage, rng);
    // TODO: damage reduction
}

#[derive(Debug)]
pub struct PlayerFighterSquad {
    pub character: UpgradeableFighter,
    pub companions: Option<EnumMap<CompanionClass, UpgradeableFighter>>,
}

#[allow(
    clippy::enum_glob_use,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::missing_panics_doc
)]
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
            attributes_bought: char.attribute_times_bought,
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
                    attributes_bought: EnumMap::default(),
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

#[allow(
    clippy::enum_glob_use,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::missing_panics_doc
)]
impl UpgradeableFighter {
    #[must_use]
    pub fn attributes(&self) -> EnumMap<AttributeType, u32> {
        let mut total = EnumMap::default();

        for equip in self.equipment.0.iter().flat_map(|a| a.1) {
            for (k, v) in &equip.attributes {
                *total.get_mut(k) += v;
            }

            // TODO: HP rune
            if let Some(GemSlot::Filled(gem)) = &equip.gem_slot {
                use AttributeType as AT;
                let mut value = gem.value;
                if matches!(equip.typ, ItemType::Weapon { .. })
                    && !self.is_companion
                {
                    value *= 2;
                }
                let mut add_atr = move |at| *total.get_mut(at) += value;
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

        let class_bonus: f64 = if self.class == Class::BattleMage {
            0.1111
        } else {
            0.0
        };

        let pet_boni = self.pet_attribute_bonus_perc;

        for (k, v) in &mut total {
            info!("{:?} - {:?}", self.class, k);
            info!("\tbase: {}", self.attribute_basis.get(k));
            info!("\tequipment: {}", v);
            let class_bonus = (f64::from(*v) * class_bonus).trunc() as u32;
            info!("\tclass: {}", class_bonus);
            *v += class_bonus + self.attribute_basis.get(k);
            if let Some(potion) = self
                .active_potions
                .iter()
                .flatten()
                .find(|a| a.typ == k.into())
            {
                let potion_bonus =
                    (f64::from(*v) * potion.size.effect()) as u32;
                info!("\tpotion: {}", v);

                *v += potion_bonus;
            }

            let pet_bonus = (f64::from(*v) * (*pet_boni.get(k))).trunc() as u32;
            info!("\tpet: {}", pet_bonus);
            *v += pet_bonus;
            info!("\ttotal: {}", v);
        }
        total
    }

    #[must_use]
    pub fn hit_points(&self, attributes: &EnumMap<AttributeType, u32>) -> u32 {
        use Class::*;

        let mut total = *attributes.get(AttributeType::Constitution);
        total = (f64::from(total)
            * match self.class {
                Warrior if self.is_companion => 6.1,
                Warrior | BattleMage | Druid => 5.0,
                Scout | Assassin | Berserker | DemonHunter | Necromancer => 4.0,
                Mage | Bard => 2.0,
            })
        .trunc() as u32;

        total *= u32::from(self.level) + 1;

        if self
            .active_potions
            .iter()
            .flatten()
            .any(|a| a.typ == PotionType::EternalLife)
        {
            total = (f64::from(total) * 1.25).trunc() as u32;
        }

        let portal_bonus = (f64::from(total)
            * (f64::from(self.portal_hp_bonus) / 100.0))
            .trunc() as u32;

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
            (f64::from(total) * (f64::from(rune_multi) / 100.0)).trunc() as u32;

        total += rune_bonus;
        total
    }
}
