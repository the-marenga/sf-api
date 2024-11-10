use enum_map::EnumMap;

use crate::{
    command::AttributeType,
    gamestate::{
        character::{Class, Race},
        items::{
            Equipment, GemSlot, GemType, ItemType, Potion, PotionType, RuneType,
        },
        social::OtherPlayer,
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
    level: u16,
    class: Class,
    race: Race,
    /// The base attributes without any equipment, or other boosts
    attribute_basis: EnumMap<AttributeType, u32>,
    attributes_bought: EnumMap<AttributeType, u32>,
    pet_attribute_bonus_perc: EnumMap<AttributeType, f64>,

    equipment: Equipment,
    active_potions: [Option<Potion>; 3],
    /// This should be the percentage bonus to skills from pets
    /// The hp bonus in percent this player has from the personal demon portal
    portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    portal_dmg_bonus: u32,

    attribute_additions: EnumMap<AttributeType, u32>,
}

#[derive(Debug, Clone)]
pub struct BattleFighter {
    attributes: EnumMap<AttributeType, u32>,

    class: Class,
    race: Race,

    max_hp: u32,
    hp: u32,

}



#[allow(
    clippy::enum_glob_use,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::missing_panics_doc
)]
impl UpgradeableFighter {
    #[must_use]
    pub fn new(character: impl Into<UpgradeableFighter>) -> Self {
        character.into()
    }

    #[must_use]
    pub fn attributes(&self) -> EnumMap<AttributeType, u32> {
        let mut total = EnumMap::default();

        for equip in self.equipment.0.iter().flat_map(|a| a.1) {
            for (k, v) in &equip.attributes {
                *total.get_mut(k) += v;
            }
            if let Some(GemSlot::Filled(gem)) = &equip.gem_slot {
                let mut value = gem.value;
                if matches!(equip.typ, ItemType::Weapon { .. }) {
                    value *= 2;
                }
                match gem.typ {
                    GemType::Strength => {
                        *total.get_mut(AttributeType::Strength) += value;
                    }
                    GemType::Dexterity => {
                        *total.get_mut(AttributeType::Dexterity) += value;
                    }
                    GemType::Intelligence => {
                        *total.get_mut(AttributeType::Intelligence) += value;
                    }
                    GemType::Constitution => {
                        *total.get_mut(AttributeType::Constitution) += value;
                    }
                    GemType::Luck => {
                        *total.get_mut(AttributeType::Luck) += value;
                    }
                    GemType::All => {
                        total.iter_mut().for_each(|a| *a.1 += value);
                    }
                    GemType::Legendary => {
                        *total.get_mut(AttributeType::Constitution) += value;
                        *total.get_mut(self.class.main_attribute()) += value;
                    }
                }
            }
        }

        let class_bonus: f64 = match self.class {
            Class::BattleMage => 0.1111,
            Class::Warrior => todo!(),
            Class::Mage => todo!(),
            Class::Scout => todo!(),
            Class::Assassin => todo!(),
            Class::Berserker => todo!(),
            Class::DemonHunter => todo!(),
            Class::Druid => todo!(),
            Class::Bard => todo!(),
            Class::Necromancer => todo!(),
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
                *v = (f64::from(*v) * potion.size.effect()) as u32;
            }

            let pet_bonus = (f64::from(*v) * (*pet_boni.get(k))).trunc() as u32;
            *v += pet_bonus;
        }

        let mut expected = self.attribute_basis;
        for n in self.attribute_additions {
            *expected.get_mut(n.0) += n.1;
        }
        assert!(total == expected);
        total
    }

    #[must_use]
    pub fn hit_points(&self) -> u32 {
        use Class::*;

        let attributes = self.attributes();
        let mut total = *attributes.get(AttributeType::Constitution);

        total *= match self.class {
            Warrior | BattleMage | Druid => 5,
            Scout | Assassin | Berserker | DemonHunter | Necromancer => 4,
            Mage | Bard => 2,
        };

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

        let expected = 30_739_506;
        assert!(total == expected, "Got {total} instead of {expected}");

        total
    }
}

impl From<&GameState> for UpgradeableFighter {
    fn from(gs: &GameState) -> Self {
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
                    (total_bonus / 100) as f64 / 100.0;
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
        Self {
            level: char.level,
            class: char.class,
            race: char.race,
            attribute_basis: char.attribute_basis,
            attribute_additions: char.attribute_additions,
            attributes_bought: char.attribute_times_bought,
            equipment: char.equipment.clone(),
            active_potions: char.active_potions,
            pet_attribute_bonus_perc,
            portal_hp_bonus,
            portal_dmg_bonus,
        }
    }
}

impl From<&OtherPlayer> for UpgradeableFighter {
    fn from(value: &OtherPlayer) -> Self {
        todo!()
    }
}
