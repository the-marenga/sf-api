use enum_map::EnumMap;

use crate::{
    command::AttributeType,
    gamestate::{
        GameState, character::Class, dungeons::CompanionClass, items::*,
        social::OtherPlayer, underworld::UnderworldBuildingType,
    },
    misc::EnumMapGet,
};

#[derive(Debug, Clone)]
pub struct UpgradeableFighter {
    pub(crate) is_companion: bool,
    pub(crate) level: u16,
    pub(crate) class: Class,
    /// The base attributes without any equipment, or other boosts
    pub attribute_basis: EnumMap<AttributeType, u32>,
    pet_attribute_bonus_perc: EnumMap<AttributeType, f64>,

    pub(crate) equipment: Equipment,
    active_potions: [Option<Potion>; 3],
    /// This should be the percentage bonus to skills from pets
    /// The hp bonus in percent this player has from the personal demon portal
    portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    pub(crate) portal_dmg_bonus: u32,
    // The level of the gladiator in the underworld
    pub(crate) gladiator: u32,
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
            // TODO:
            gladiator: 0,
        }
    }

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
        total = (total as f64 * self.class.health_multiplier(self.is_companion))
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

        let gladiator = match &gs.underworld {
            Some(uw) => uw.buildings[UnderworldBuildingType::GladiatorTrainer]
                .level
                .into(),
            None => 0,
        };

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
            gladiator,
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
                    gladiator,
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
