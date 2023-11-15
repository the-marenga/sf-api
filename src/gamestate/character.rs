use std::fmt::Debug;

use chrono::{DateTime, Local};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::EnumCount;

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

    pub attribute_basis: Attributes,
    pub attribute_additions: Attributes,
    /// The amount of times an atribute has been bought already
    pub attribute_times_bought: Attributes,

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
    pub special_portrait: u32,
}

impl Portrait {
    pub(crate) fn update(&mut self, data: &[i64]) {
        self.mouth = soft_into(data[0], "mouth", 1);
        self.hair_color = soft_into(data[1] / 100, "hair color", 1);
        self.hair = soft_into(data[1] % 100, "hair", 1);
        self.brows = soft_into(data[2] % 100, "brows", 1);
        self.eyes = soft_into(data[3], "eyes", 1);
        self.beards = soft_into(data[4] % 100, "beards", 1);
        self.nose = soft_into(data[5], "nose", 1);
        self.ears = soft_into(data[6], "ears", 1);
        // Check what 24..=25 are
        self.special_portrait = soft_into(data[9], "special", 1);
        self.gender = FromPrimitive::from_i64(data[11] % 2).unwrap_or_default();
    }
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Gender {
    #[default]
    Female = 0,
    Male,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DruidMask {
    Cat = 4,
    Bear = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BardInstrument {
    Harp = 1,
    Lute,
    Flute,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Attributes(pub(crate) [u32; AttributeType::COUNT]);

impl Attributes {
    pub fn get(&self, attribute: AttributeType) -> u32 {
        self.0[attribute as usize - 1]
    }
    pub fn get_mut(&mut self, attribute: AttributeType) -> &mut u32 {
        self.0.get_mut(attribute as usize - 1).unwrap()
    }
    pub fn set(&mut self, attribute: AttributeType, val: u32) {
        self.0[attribute as usize - 1] = val
    }
    pub(crate) fn update(&mut self, data: &[i64]) {
        self.0.iter_mut().zip(data).for_each(|(old, new)| {
            *old = soft_into(*new, "attribute value", 0)
        });
    }
}

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy, FromPrimitive)]
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

#[derive(Debug, Copy, Clone, FromPrimitive, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mount {
    Cow = 1,
    Horse = 2,
    Tiger = 3,
    Dragon = 4,
}
