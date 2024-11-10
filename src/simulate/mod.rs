use enum_map::EnumMap;

use crate::{
    command::AttributeType,
    gamestate::{
        character::{Class, Race},
        items::{Equipment, Potion},
        social::OtherPlayer,
        GameState,
    },
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
    class: Class,
    race: Race,
    /// The base attributes without any equipment, or other boosts
    attribute_basis: EnumMap<AttributeType, u32>,
    attributes_bought: EnumMap<AttributeType, u32>,

    equipment: Equipment,
    active_potions: [Option<Potion>; 3],
    /// This should be the percentage bonus to skills from pets
    pub pet_attribute_bonus_perc: EnumMap<AttributeType, u32>,
    /// The hp bonus in percent this player has from the personal demon portal
    pub portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    pub portal_dmg_bonus: u32,

    attribute_additions: EnumMap<AttributeType, u32>,
}

impl UpgradeableFighter {
    pub fn new(character: impl Into<UpgradeableFighter>) -> Self {
        character.into()
    }
}

impl From<&GameState> for UpgradeableFighter {
    fn from(gs: &GameState) -> Self {
        let pet_attribute_bonus_perc =
            gs.pets.as_ref().map(|a| a.atr_bonus).unwrap_or_default();
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
