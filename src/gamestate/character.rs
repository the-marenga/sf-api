use std::fmt::Debug;

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{Mirror, NormalCost, RelationEntry, SFError, ScrapBook};
use crate::{PlayerId, command::*, gamestate::items::*, misc::*};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Everything, that can be considered part of the character and not the rest
/// of the world
pub struct Character {
    /// This is the unique identifier of this character. Can be used to compare
    /// against places, that also have `player_ids` to make sure a Hall of
    /// Fame entry or similar is not the player
    pub player_id: PlayerId,
    /// The name of this character
    pub name: String,
    /// The current level of this character
    pub level: u16,
    /// The amount of silver a player has. 100 silver = 1 gold
    pub silver: u64,
    /// The amount of moshrooms a player has
    pub mushrooms: u32,

    /// The class of this character
    pub class: Class,

    /// The race of this character. Has some effects on attributes, which is
    /// why this is not in portrait
    pub race: Race,
    /// Everything that determines the players looks except for the race
    pub portrait: Portrait,
    /// The description of this character
    pub description: String,

    /// The amount of experience already earned in the current level
    pub experience: u64,
    /// The amount of experience required to level up.
    /// `next_level_xp - experience` is the amount of xp missing to level up
    pub next_level_xp: u64,
    /// The amount of honor earned through the arena
    pub honor: u32,
    /// The rank in the hall of fame
    pub rank: u32,

    /// All the items this character has stored. These are all the slots right
    /// next to the portrait in the web ui
    pub inventory: Inventory,
    /// All items the character has currently equipped (on the body)
    pub equipment: Equipment,

    /// If the character has a manequin, this will contain all the equipment
    /// stored in it
    pub manequin: Option<Equipment>,
    /// The potions currently active
    pub active_potions: [Option<Potion>; 3],

    /// The total armor of our character. Basically all equipped armor combined
    pub armor: u64,

    /// The min amount of damage the weapon claims it can do without any bonus
    pub min_damage: u32,
    /// The max amount of damage the weapon claims it can do without any bonus
    pub max_damage: u32,

    /// The base attributes without any equipment, or other boosts
    pub attribute_basis: EnumMap<AttributeType, u32>,
    /// All bonus attributes from equipment/pets/potions
    pub attribute_additions: EnumMap<AttributeType, u32>,
    /// The amount of times an attribute has been bought already.
    /// Important to calculate the price of the next attribute to buy
    pub attribute_times_bought: EnumMap<AttributeType, u32>,

    /// The mount this character has rented
    pub mount: Option<Mount>,
    /// The point at which the mount will end. Note that this might be None,
    /// whilst mount is Some
    pub mount_end: Option<DateTime<Local>>,
    /// The silver you get for buying a dragon
    pub mount_dragon_refund: u64,

    /// Whether this character has the mirror completed, or is still collecting
    /// pieces
    pub mirror: Mirror,
    /// If the scrapbook has been unlocked, it can be found here
    pub scrapbook: Option<ScrapBook>,

    /// A list of other characters, that the set some sort of special relation
    /// to. Either good, or bad
    pub relations: Vec<RelationEntry>,
}

/// All the exclusively cosmetic info necessary to build a player image, that is
/// otherwise useless. As these values might change their based on each other,
/// some of them are not fully parsed (to a more descriptive enum)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub struct Portrait {
    /// The gender (m/w)
    pub gender: Gender,
    pub hair_color: u8,
    pub hair: u8,
    pub mouth: u8,
    pub brows: u8,
    pub eyes: u8,
    pub beards: u8,
    pub nose: u8,
    pub ears: u8,
    pub extra: u8,
    pub horns: u8,
    /// Influencers get a special portrait. Otherwise this should be 0
    pub special_portrait: i64,
}

impl Portrait {
    pub(crate) fn parse(data: &[i64]) -> Result<Portrait, SFError> {
        Ok(Self {
            mouth: data.csiget(0, "mouth", 1)?,
            hair_color: data.csimget(1, "hair color", 100, |a| a / 100)?,
            hair: data.csimget(1, "hair", 1, |a| a % 100)?,
            brows: data.csimget(2, "brows", 1, |a| a % 100)?,
            eyes: data.csiget(3, "eyes", 1)?,
            beards: data.csimget(4, "beards", 1, |a| a % 100)?,
            nose: data.csiget(5, "nose", 1)?,
            ears: data.csiget(6, "ears", 1)?,
            extra: data.csiget(7, "extra", 1)?,
            horns: data.csimget(8, "horns", 1, |a| a % 100)?,
            special_portrait: data.cget(9, "special portrait")?,
            gender: Gender::from_i64(data.csimget(11, "gender", 1, |a| a % 2)?)
                .unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Gender {
    #[default]
    Female = 0,
    Male,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Class {
    #[default]
    Warrior = 0,
    Mage,
    Scout,
    Assassin,
    BattleMage,
    Berserker,
    DemonHunter,
    Druid,
    Bard,
    Necromancer,
    Paladin,
    PlagueDoctor,
}

#[allow(clippy::enum_glob_use)]
impl Class {
    #[must_use]
    #[allow(clippy::enum_glob_use)]
    pub fn main_attribute(&self) -> AttributeType {
        use Class::*;
        match self {
            Paladin | BattleMage | Berserker | Warrior => {
                AttributeType::Strength
            }
            Assassin | DemonHunter | Scout | PlagueDoctor => {
                AttributeType::Dexterity
            }
            Druid | Bard | Necromancer | Mage => AttributeType::Intelligence,
        }
    }

    #[must_use]
    pub(crate) fn weapon_multiplier(self) -> f64 {
        use Class::*;
        match self {
            PlagueDoctor | Paladin | Warrior | Assassin | BattleMage
            | Berserker => 2.0,
            Scout => 2.5,
            Mage | DemonHunter | Druid | Bard | Necromancer => 4.5,
        }
    }

    #[must_use]
    pub(crate) fn monster_armor_multiplier(self) -> u32 {
        match self {
            Class::Warrior | Class::Berserker | Class::DemonHunter => 50,
            Class::Paladin => 45,
            // TODO: Plague doctor
            Class::Scout
            | Class::Assassin
            | Class::Druid
            | Class::Bard
            | Class::PlagueDoctor => 25,
            Class::Mage | Class::BattleMage | Class::Necromancer => 10,
        }
    }

    #[must_use]
    pub(crate) fn life_multiplier(self, is_companion: bool) -> f64 {
        use Class::*;

        match self {
            Warrior if is_companion => 6.1,
            Paladin => 6.0,
            Warrior | BattleMage | Druid => 5.0,
            PlagueDoctor | Scout | Assassin | Berserker | DemonHunter
            | Necromancer => 4.0,
            Mage | Bard => 2.0,
        }
    }

    #[must_use]
    pub(crate) fn block_chance(self) -> f32 {
        match self {
            Class::Warrior => 0.25,
            Class::Paladin => 0.3,
            _ => 0.0,
        }
    }

    pub(crate) fn max_damage_reduction_val(self) -> f64 {
        match self {
            Class::Mage | Class::BattleMage | Class::Necromancer => 10.0,
            Class::Scout
            | Class::Assassin
            | Class::Druid
            | Class::Bard
            | Class::PlagueDoctor => 25.0,
            Class::Paladin => 45.0,
            Class::DemonHunter | Class::Berserker | Class::Warrior => 50.0,
        }
    }

    #[must_use]
    pub(crate) fn max_damage_reduction_multiplier(self) -> f64 {
        use Class::*;
        match self {
            Berserker => 0.5,
            Warrior | Mage | Scout | Assassin | DemonHunter | Druid
            | Paladin | PlagueDoctor => 1.0,
            Bard | Necromancer => 2.0,
            BattleMage => 5.0,
        }
    }

    #[must_use]
    pub fn can_wear_shield(self) -> bool {
        matches!(self, Self::Paladin | Self::Warrior)
    }

    #[must_use]
    pub(crate) fn damage_factor(self, against: Class) -> f64 {
        use Class::*;
        match self {
            Druid if against == Class::DemonHunter => (1.0 / 3.0) * 1.15,
            Druid if against == Class::Mage => (1.0 / 3.0) * (4.0 / 3.0),
            Druid => 1.0 / 3.0,
            Necromancer if against == Class::DemonHunter => 0.56 + 0.1,
            Necromancer => 0.56,
            Assassin => 0.625,
            Paladin => 0.83,
            Warrior | Mage | Scout | BattleMage | DemonHunter => 1.0,
            Bard => 1.125,
            Berserker | PlagueDoctor => 1.25,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Race {
    #[default]
    Human = 1,
    Elf,
    Dwarf,
    Gnome,
    Orc,
    DarkElf,
    Goblin,
    Demon,
}

impl Race {
    /// These are the boni the game claims to give to certain races. As far as I
    /// can tell though, these are actually irrellevant. Changing the race mid
    /// game does nothing and the calcs without it are linig up perfectly. That
    /// means these values here have no reason to exist
    #[must_use]
    pub fn stat_modifiers(self) -> EnumMap<AttributeType, i32> {
        let raw = match self {
            Race::Human => [0, 0, 0, 0, 0],
            Race::Elf => [-1, 2, 0, -1, 0],
            Race::Dwarf => [0, -2, -1, 2, 1],
            Race::Gnome => [-2, 3, -1, -1, 1],
            Race::Orc => [1, 0, -1, 0, 0],
            Race::DarkElf => [-2, 2, 1, -1, 0],
            Race::Goblin => [-2, 2, 0, -1, 1],
            Race::Demon => [3, -1, 0, 1, -3],
        };
        EnumMap::from_array(raw)
    }
}

#[derive(Debug, Copy, Clone, FromPrimitive, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Mount {
    Cow = 1,
    Horse = 2,
    Tiger = 3,
    Dragon = 4,
}

impl Mount {
    /// Returns the cost of this mount
    #[must_use]
    pub fn cost(&self) -> NormalCost {
        match self {
            Mount::Cow => NormalCost {
                silver: 100,
                mushrooms: 0,
            },
            Mount::Horse => NormalCost {
                silver: 500,
                mushrooms: 0,
            },
            Mount::Tiger => NormalCost {
                silver: 1000,
                mushrooms: 1,
            },
            Mount::Dragon => NormalCost {
                silver: 0,
                mushrooms: 25,
            },
        }
    }
}
