use enum_map::{Enum, EnumMap};
use fastrand::Rng;
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    command::AttributeType,
    gamestate::{
        character::Class, dungeons::CompanionClass, items::*, GameState,
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
    _attributes_bought: EnumMap<AttributeType, u32>,
    pet_attribute_bonus_perc: EnumMap<AttributeType, f64>,

    equipment: Equipment,
    active_potions: [Option<Potion>; 3],
    /// This should be the percentage bonus to skills from pets
    /// The hp bonus in percent this player has from the personal demon portal
    portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    portal_dmg_bonus: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Minion {
    pub typ: MinionType,
    pub rounds_remaining: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub max_hp: i64,
    pub current_hp: i64,
    pub equip: EquipmentEffects,
    pub portal_dmg_bonus: f64,
    pub rounds_in_battle: u32,
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
    Necromancer(Minion),
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
}

impl ClassEffect {
    pub fn druid_swoops(&self) -> u8 {
        match self {
            ClassEffect::Druid { swoops, .. } => *swoops,
            _ => 0,
        }
    }

    pub fn harp_quality(&self, against: Class) -> Option<HarpQuality> {
        match self {
            ClassEffect::Bard { quality, .. } if against != Class::Mage => {
                Some(*quality)
            }
            _ => None,
        }
    }
}

impl BattleFighter {
    #[must_use]
    pub fn from_monster(monster: &Monster) -> Self {
        // TODO: I assume this is unarmed damage, but I have have to
        // check
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
                element_res: Default::default(),
                element_dmg: Default::default(),
                weapon,
                offhand: (0, 0),
                reaction_boost: false,
                extra_crit_dmg: false,
                armor: 0,
            },
            portal_dmg_bonus: 1.0,
            rounds_in_battle: 0,
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
                            calc_unarmed_base_dmg(slot, char.level, char.class)
                    }
                    EquipmentSlot::Shield if char.class == Class::Assassin => {
                        equip.offhand =
                            calc_unarmed_base_dmg(slot, char.level, char.class)
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
            class_effect: ClassEffect::Normal,
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

    pub fn reset(&mut self) {
        self.class_effect = ClassEffect::Normal;
        self.current_hp = self.max_hp;
        self.rounds_in_battle = 0;
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

impl<'a> BattleTeam<'a> {
    pub fn current(&mut self) -> Option<&mut BattleFighter> {
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
    round: u32,
    started: Option<BattleSide>,
    left: BattleTeam<'a>,
    right: BattleTeam<'a>,
    rng: Rng,
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
    pub fn simulate(&mut self) -> BattleSide {
        self.reset();
        loop {
            if let Some(winner) = self.simulate_turn() {
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
    fn simulate_turn(&mut self) -> Option<BattleSide> {
        use BattleSide::{Left, Right};
        use Class::{
            Assassin, Bard, BattleMage, Berserker, DemonHunter, Druid, Mage,
            Necromancer, Scout, Warrior,
        };

        let Some(left) = self.left.current() else {
            return Some(Right);
        };
        let Some(right) = self.right.current() else {
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

        use AttackType::{Offhand, Swoop, Weapon};

        match attacker.class {
            Warrior | Scout | Mage | DemonHunter => {
                fighter_attack(attacker, defender, &mut self.rng, Weapon)
            }
            Assassin => {
                fighter_attack(attacker, defender, &mut self.rng, Weapon);
                fighter_attack(attacker, defender, &mut self.rng, Offhand);
            }
            Berserker => {
                for _ in 0..15 {
                    fighter_attack(attacker, defender, &mut self.rng, Weapon);
                    if self.rng.bool() {
                        break;
                    }
                }
            }
            BattleMage => {
                if attacker.rounds_in_battle == 1 && defender.class != Mage {
                    let dmg = match defender.class {
                        Mage => 0,
                        Bard => attacker.max_hp / 10,
                        Scout | Assassin | Berserker | Necromancer
                        | DemonHunter => attacker.max_hp / 5,
                        Warrior | BattleMage | Druid => attacker.max_hp / 4,
                    };
                    // TODO: Can you dodge this?
                    do_damage(defender, dmg, &mut self.rng);
                }
                fighter_attack(attacker, defender, &mut self.rng, Weapon)
            }
            Druid => {
                // Check if we do a sweep attack
                if !matches!(
                    attacker.class_effect,
                    ClassEffect::Druid { bear: true, .. }
                ) {
                    let swoops = attacker.class_effect.druid_swoops();
                    let swoop_chance = 0.15 + ((swoops as f32 * 5.0) / 100.0);
                    if defender.class != Class::Mage
                        && self.rng.f32() <= swoop_chance
                    {
                        fighter_attack(
                            attacker,
                            defender,
                            &mut self.rng,
                            Swoop,
                        );
                        attacker.class_effect = ClassEffect::Druid {
                            bear: false,
                            // max 7 to limit chance to 50%
                            swoops: (swoops + 1).min(7),
                        }
                    }
                }

                fighter_attack(attacker, defender, &mut self.rng, Weapon);
                // TODO: Does this reset here, or on the start of the next
                // attack?
                attacker.class_effect = ClassEffect::Druid {
                    bear: false,
                    swoops: attacker.class_effect.druid_swoops(),
                };
            }
            Bard => {
                // Start a new melody every 4 turns
                if attacker.rounds_in_battle % 4 == 0 {
                    let quality = self.rng.u8(0..4);
                    let (quality, remaining) = match quality {
                        0 => (HarpQuality::Bad, 3),
                        1 | 2 => (HarpQuality::Medium, 3),
                        _ => (HarpQuality::Good, 4),
                    };
                    attacker.class_effect =
                        ClassEffect::Bard { quality, remaining }
                }
                fighter_attack(attacker, defender, &mut self.rng, Weapon);
                if let ClassEffect::Bard { remaining, .. } =
                    &mut attacker.class_effect
                {
                    *remaining = remaining.saturating_sub(1);
                }
            }
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
fn do_damage(to: &mut BattleFighter, damage: i64, rng: &mut Rng) {
    // debug!(
    //     "Doing {damage} damage to {:?} with {:.2}% hp",
    //     to.class,
    //     (to.current_hp as f32 / to.max_hp as f32) * 100.0
    // );
    if to.current_hp <= 0 || damage == 0 {
        // Skip pointless attacks
        return;
    }
    to.current_hp -= damage;
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
}

fn fighter_attack(
    attacker: &mut BattleFighter,
    defender: &mut BattleFighter,
    rng: &mut Rng,
    typ: AttackType,
) {
    if attacker.class != Class::Mage {
        // TODO: Different dedge rates (druid 35%)
        if defender.class == Class::Scout && rng.bool() {
            // defender dodged
            if defender.class == Class::Druid {
                // TODO: is this instant, or does this trigger on start of def.
                // turn?
                defender.class_effect = ClassEffect::Druid {
                    bear: true,
                    swoops: defender.class_effect.druid_swoops(),
                };
            }
            return;
        }
        if defender.class == Class::Warrior
            && !defender.is_companion
            && defender.equip.offhand.0 as f32 / 100.0 > rng.f32()
        {
            // defender blocked
            return;
        }
    }

    // TODO: Most of this can be reused, as long as the opponent does not
    // change. Should make sure this is correct first though
    let char_damage_modifier = 1.0
        + (*attacker.attributes.get(attacker.class.main_attribute()) as f64)
            / 10.0;

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

    let swoop_bonus = match typ {
        AttackType::Swoop => 1.8,
        _ => 1.0,
    };
    let harp_bonus = match attacker.class_effect.harp_quality(defender.class) {
        None => 1.0,
        Some(HarpQuality::Bad) => 1.2,
        Some(HarpQuality::Medium) => 1.4,
        Some(HarpQuality::Good) => 1.6,
    };

    // FIME: Check the order of all of this
    let damage_bonus = char_damage_modifier
        * attacker.portal_dmg_bonus
        * elemental_bonus
        * (1.0 - def_reduction)
        * attacker.class.damage_factor(defender.class)
        * swoop_bonus
        * harp_bonus;

    let weapon = if typ == AttackType::Offhand {
        attacker.equip.offhand
    } else {
        attacker.equip.weapon
    };

    let calc_damage =
        |weapon_dmg| (weapon_dmg as f64 * damage_bonus).trunc() as i64;

    let min_base_damage = calc_damage(weapon.0);
    let max_base_damage = calc_damage(weapon.1);

    let mut damage = rng.i64(min_base_damage..=max_base_damage);

    let luck_mod = attacker.attributes.get(AttributeType::Luck) * 5;
    let raw_crit_chance = luck_mod as f64 / (defender.level as f64);

    let is_bear =
        matches!(attacker.class_effect, ClassEffect::Druid { bear: true, .. });
    let mut crit_chance = raw_crit_chance.min(0.5);
    if is_bear {
        crit_chance += 0.1;
    }

    if rng.f64() <= crit_chance {
        let mut crit_dmg_factor = 2.0;
        if attacker.equip.extra_crit_dmg {
            crit_dmg_factor += 0.05;
        };
        if is_bear {
            // TODO: Is this right, or do we set to 4.0?
            crit_dmg_factor += 2.0;
        };
        damage = (damage as f64 * crit_dmg_factor) as i64;
    }
    do_damage(defender, damage.max(1), rng);
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
            _attributes_bought: char.attribute_times_bought,
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
                    _attributes_bought: EnumMap::default(),
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
            let class_bonus = (f64::from(*v) * class_bonus).trunc() as u32;
            *v += class_bonus + self.attribute_basis.get(k);
            if let Some(potion) = self
                .active_potions
                .iter()
                .flatten()
                .find(|a| a.typ == k.into())
            {
                let potion_bonus =
                    (f64::from(*v) * potion.size.effect()) as u32;
                *v += potion_bonus;
            }

            let pet_bonus = (f64::from(*v) * (*pet_boni.get(k))).trunc() as u32;
            *v += pet_bonus;
        }
        total
    }

    #[must_use]
    pub fn hit_points(&self, attributes: &EnumMap<AttributeType, u32>) -> i64 {
        use Class::*;

        let mut total = *attributes.get(AttributeType::Constitution) as i64;
        total = (total as f64
            * match self.class {
                Warrior if self.is_companion => 6.1,
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