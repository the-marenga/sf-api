use std::fmt::Debug;

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::SFError;
use crate::{command::*, gamestate::items::*, misc::*, PlayerId};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharacterState {
    pub player_id: PlayerId,

    pub name: String,
    pub level: u16,
    // The amount of silver a player has. 100 silver = 1 gold
    pub silver: u64,
    pub mushrooms: u32,

    pub class: Class,
    pub druid_mask: Option<DruidMask>,
    pub bard_instrument: Option<BardInstrument>,

    pub race: Race,
    pub portrait: Portrait,
    pub description: String,

    /// The amount of experience already earned in the current level
    pub experience: u64,
    /// The amount of experience required to level up
    pub next_level_xp: u64,
    /// The amount of honor earned through the arena
    pub honor: u32,
    /// The rank in the hall of fame
    pub rank: u32,

    pub inventory: Inventory,
    /// Equiped items
    pub equipment: Equipment,

    /// Equiped items
    pub manequin: Option<Equipment>,
    pub active_potions: [Option<ItemType>; 3],

    /// The total armor of our character. Basically all equiped armor combined
    pub armor: u64,

    /// The min amount of damage the weapon claims it can do without any bonus
    pub min_damage: u32,
    /// The max amount of damage the weapon claims it can do without any bonus
    pub max_damage: u32,

    pub attribute_basis: EnumMap<AttributeType, u32>,
    pub attribute_additions: EnumMap<AttributeType, u32>,
    /// The amount of times an atribute has been bought already
    pub attribute_times_bought: EnumMap<AttributeType, u32>,

    pub mount: Option<Mount>,
    pub mount_end: Option<DateTime<Local>>,
    /// The silver you get for buying a dragon
    pub mount_dragon_refund: u64,

    pub lucky_coins: u32,
    pub wheel_spins_today: u8,
    pub wheel_next_free_spin: Option<DateTime<Local>>,
}

/// All the exclusively cosmetic info necessary to build a player image, that is
/// otherwise useless
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Portrait {
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
            hair_color: data.csiget(1, "hair color", 100)? / 100,
            hair: data.csiget(1, "hair", 1)? % 100,
            brows: data.csiget(2, "brows", 1)? % 100,
            eyes: data.csiget(3, "eyes", 1)?,
            beards: data.csiget(4, "beards", 1)? % 100,
            nose: data.csiget(5, "nose", 1)?,
            ears: data.csiget(6, "ears", 1)?,
            extra: data.csiget(7, "extra", 1)?,
            horns: data.csiget(8, "horns", 1)? % 100,
            special_portrait: data.cget(9, "special portrait")?,
            gender: Gender::from_i64(data.cget(11, "gender")? % 2)
                .unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Gender {
    #[default]
    Female = 0,
    Male,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DruidMask {
    Cat = 4,
    Bear = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BardInstrument {
    Harp = 1,
    Lute,
    Flute,
}

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Debug, Copy, Clone, FromPrimitive, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mount {
    Cow = 1,
    Horse = 2,
    Tiger = 3,
    Dragon = 4,
}
