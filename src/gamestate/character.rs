use std::fmt::Debug;

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::warn;
use num_derive::FromPrimitive;

use super::{
    AttributeType, Equipment, Inventory, NormalCost, Potion, RelationEntry,
    Result, SFError, ScrapBook, ServerTime,
};
use crate::{PlayerId, response::Vals};

/// Everything, that can be considered part of the character and not the rest
/// of the world
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// If the character has a mannequin, this will contain all the equipment
    /// stored in it
    pub mannequin: Option<Equipment>,
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

    /// If the scrapbook has been unlocked, it can be found here
    pub scrapbook: Option<ScrapBook>,

    /// A list of other characters, that the set some sort of special relation
    /// to. Either good, or bad
    pub relations: Vec<RelationEntry>,
}

impl Character {
    /// Updates character details from an `ownplayersavecharacter` response.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        data.ignore(0, "creation time? secret id?");
        self.player_id = data.u32_or(1, 0);
        data.ignore(2, "always 0");
        self.level = (data.u32_or(3, 0) & 0xFFFF) as u16;
        self.experience = data.u64_or(4, 0);
        self.next_level_xp = data.u64_or(5, 0);
        self.honor = data.u32_or(6, 0);
        self.rank = data.u32_or(7, 0);

        self.portrait = Portrait::parse(&data.window(8..20));

        self.race = data.parse_enum(18)?;
        self.class =
            data.enum_map(20, |a| a.wrapping_sub(1)).ok_or_else(|| {
                SFError::ParsingErrorV2(format!(
                    "Unknown character class: {}",
                    data.u32_or(20, 0)
                ))
            })?;
        self.mount = data.enum_map(21, |a| a & 0xFF);
        data.ignore(22, "always 0/1 in captured responses");
        self.armor = data.u64_or(23, 0);
        self.min_damage = data.u32_or(24, 0);
        self.max_damage = data.u32_or(25, 0);
        data.ignore(26, "portal dmg bonus, handled in guild");
        data.ignore(27, "unknown, large integer seen in the millions");
        data.ignore(28, "portal hp bonus, handled in dungeons");
        self.mount_end = data.server_time(29, server_time)?;

        for (idx, val) in self.attribute_basis.values_mut().enumerate() {
            *val = data.u32_or(30 + idx, 0);
        }
        for (idx, val) in self.attribute_additions.values_mut().enumerate() {
            *val = data.u32_or(35 + idx, 0);
        }
        for (idx, val) in self.attribute_times_bought.values_mut().enumerate() {
            *val = data.u32_or(40 + idx, 0);
        }

        data.ignore(45..70, "unknown/unmodelled character save values");

        Ok(())
    }

    /// Updates the character's relation entries from a `friendlist` response.
    pub(crate) fn update_relations(&mut self, data: &Vals<'_>) {
        self.relations.clear();
        for entry in data.iter() {
            if entry.is_empty() {
                continue;
            }
            match RelationEntry::parse(&Vals::by(entry, ',')) {
                Ok(x) => self.relations.push(x),
                Err(e) => warn!("bad friendslist entry: {entry} {e}"),
            }
        }
    }
}

/// All the exclusively cosmetic info necessary to build a player image, that is
/// otherwise useless. As these values might change based on each other,
/// some of them are not fully parsed (to a more descriptive enum)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Portrait {
    /// The gender (Female or Male)
    pub gender: Gender,
    /// The hair color index
    pub hair_color: u8,
    /// The hair style index
    pub hair: u8,
    /// The mouth style index
    pub mouth: u8,
    /// The eyebrow style index
    pub brows: u8,
    /// The eye style index
    pub eyes: u8,
    /// The beard style index
    pub beards: u8,
    /// The nose style index
    pub nose: u8,
    /// The ear style index
    pub ears: u8,
    /// The extra facial feature / accessory index
    pub extra: u8,
    /// The horns feature index (e.g. for Demons)
    pub horns: u8,
    /// Influencers get a special portrait. Otherwise this should be 0
    pub special_portrait: i64,
}

impl Portrait {
    /// Parses portrait features from response values.
    #[must_use]
    pub(crate) fn parse(data: &Vals<'_>) -> Portrait {
        let hair_val = data.u32_or(1, 100);
        data.ignore(10, "unused in portrait");
        Self {
            mouth: data.u8_or(0, 1),
            hair_color: ((hair_val / 100) % 100) as u8,
            hair: (hair_val % 100) as u8,
            brows: (data.u32_or(2, 1) % 100) as u8,
            eyes: data.u8_or(3, 1),
            beards: (data.u32_or(4, 1) % 100) as u8,
            nose: data.u8_or(5, 1),
            ears: data.u8_or(6, 1),
            extra: data.u8_or(7, 1),
            horns: (data.u32_or(8, 1) % 100) as u8,
            special_portrait: data.i64_or(9, 0),
            gender: data.enum_map(11, |a| a % 2).unwrap_or_default(),
        }
    }
}

/// The gender of a character
#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Gender {
    /// Female character
    #[default]
    Female = 0,
    /// Male character
    Male,
}

/// The character class
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Class {
    /// Warrior class, uses Strength and can wear shields
    #[default]
    Warrior = 0,
    /// Mage class, uses Intelligence
    Mage,
    /// Scout class, uses Dexterity
    Scout,
    /// Assassin class, uses Dexterity and wields two weapons
    Assassin,
    /// Battle Mage class, uses Strength and casts arcane fireballs
    BattleMage,
    /// Berserker class, uses Strength and may attack multiple times
    Berserker,
    /// Demon Hunter class, uses Dexterity and can revive
    DemonHunter,
    /// Druid class, uses Intelligence and shapeshifts
    Druid,
    /// Bard class, uses Intelligence and plays music
    Bard,
    /// Necromancer class, uses Intelligence and summons minions
    Necromancer,
    /// Paladin class, uses Strength and can wear shields
    Paladin,
    /// Plague Doctor class, uses Dexterity
    PlagueDoctor,
    /// Blood Weaver class, uses Strength
    BloodWeaver,
}

impl Class {
    /// Returns the primary attribute type for this class.
    #[must_use]
    pub fn main_attribute(self) -> AttributeType {
        match self {
            Self::BloodWeaver
            | Self::Paladin
            | Self::BattleMage
            | Self::Berserker
            | Self::Warrior => AttributeType::Strength,
            Self::Assassin
            | Self::DemonHunter
            | Self::Scout
            | Self::PlagueDoctor => AttributeType::Dexterity,
            Self::Druid | Self::Bard | Self::Necromancer | Self::Mage => {
                AttributeType::Intelligence
            }
        }
    }

    // NOTE: Values for Blood Weaver may be guesses, or changes before release

    /// Returns the weapon damage multiplier for this class.
    #[must_use]
    pub fn weapon_multiplier(self) -> f64 {
        match self {
            Self::PlagueDoctor
            | Self::Paladin
            | Self::Warrior
            | Self::Assassin
            | Self::BattleMage
            | Self::Berserker
            | Self::BloodWeaver => 2.0,
            Self::Scout | Self::DemonHunter => 2.5,
            Self::Mage | Self::Druid | Self::Bard | Self::Necromancer => 4.5,
        }
    }

    /// Returns the weapon gem multiplier for this class.
    #[must_use]
    pub fn weapon_gem_multiplier(self) -> i32 {
        match self {
            Class::Warrior | Class::Assassin | Class::Berserker => 1,
            _ => 2,
        }
    }

    /// Returns the weapon attribute multiplier for this class.
    #[must_use]
    pub fn weapon_attribute_multiplier(self) -> i32 {
        match self {
            Class::Warrior
            | Class::BattleMage
            | Class::Berserker
            | Class::Paladin
            | Class::PlagueDoctor
            | Class::BloodWeaver
            | Class::Assassin => 1,
            _ => 2,
        }
    }

    #[cfg(feature = "simulation")]
    #[must_use]
    pub(crate) fn health_multiplier(self, is_companion: bool) -> f64 {
        match self {
            Self::Warrior if is_companion => 6.1,
            Self::Warrior
            | Self::BattleMage
            | Self::Druid
            | Self::BloodWeaver => 5.0,
            Self::Paladin => 6.0,
            Self::PlagueDoctor
            | Self::Scout
            | Self::Assassin
            | Self::Berserker
            | Self::DemonHunter
            | Self::Necromancer => 4.0,
            Self::Mage | Self::Bard => 2.0,
        }
    }

    /// Returns the armor multiplier for this class.
    #[must_use]
    pub fn armor_multiplier(self) -> f64 {
        match self {
            Class::BattleMage => 5.0,
            Class::Bard | Class::Necromancer | Class::PlagueDoctor => 2.0,
            Class::Berserker => 0.5,
            _ => 1.0,
        }
    }

    /// Returns the maximum armor damage reduction percentage for this class.
    #[must_use]
    pub fn max_armor_reduction(self) -> u32 {
        match self {
            Class::Mage => 10,
            Class::Warrior
            | Class::BattleMage
            | Class::DemonHunter
            | Class::Bard => 50,
            Class::Paladin => 45,
            Class::Scout
            | Class::Assassin
            | Class::Berserker
            | Class::BloodWeaver
            | Class::Druid => 25,
            Class::Necromancer | Class::PlagueDoctor => 20,
        }
    }

    /// Returns the damage multiplier for this class.
    #[must_use]
    pub fn damage_multiplier(self) -> f64 {
        match self {
            Class::Assassin => 0.625,
            Class::Berserker | Class::PlagueDoctor => 1.25,
            Class::Druid => 1.0 / 3.0,
            Class::Bard => 1.125,
            Class::Necromancer => 5.0 / 9.0,
            Class::Paladin => 0.833,
            _ => 1.0,
        }
    }

    /// Returns whether this class can equip a shield.
    #[must_use]
    pub fn can_wear_shield(self) -> bool {
        matches!(self, Self::Paladin | Self::Warrior)
    }
}

/// The character race
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy, FromPrimitive, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Race {
    /// Human race
    #[default]
    Human = 1,
    /// Elf race
    Elf,
    /// Dwarf race
    Dwarf,
    /// Gnome race
    Gnome,
    /// Orc race
    Orc,
    /// Dark Elf race
    DarkElf,
    /// Goblin race
    Goblin,
    /// Demon race
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

/// The rented mount
#[derive(Debug, Copy, Clone, FromPrimitive, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mount {
    /// Cow (or pig), provides a 10% quest duration reduction
    Cow = 1,
    /// Horse, provides a 20% quest duration reduction
    Horse = 2,
    /// Tiger (or raptor), provides a 30% quest duration reduction
    Tiger = 3,
    /// Dragon (or griffin), provides a 50% quest duration reduction
    Dragon = 4,
}

impl Mount {
    /// Returns the cost of this mount
    #[must_use]
    pub fn cost(self) -> NormalCost {
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

#[cfg(test)]
mod tests {
    use super::{super::social::Relationship, *};

    fn assert_all_read(data: &Vals<'_>) {
        assert_eq!(
            data.unaccessed_indices(),
            Vec::<usize>::new(),
            "unaccessed indices in test data"
        );
    }

    #[test]
    fn test_portrait_parse() {
        let raw = "4/206/203/2/0/2/7/2/0/0/2/1";
        let vals = Vals::from(raw);
        let portrait = Portrait::parse(&vals);

        assert_eq!(portrait.mouth, 4);
        assert_eq!(portrait.hair_color, 2);
        assert_eq!(portrait.hair, 6);
        assert_eq!(portrait.brows, 3);
        assert_eq!(portrait.eyes, 2);
        assert_eq!(portrait.beards, 0);
        assert_eq!(portrait.nose, 2);
        assert_eq!(portrait.ears, 7);
        assert_eq!(portrait.extra, 2);
        assert_eq!(portrait.horns, 0);
        assert_eq!(portrait.special_portrait, 0);
        assert_eq!(portrait.gender, Gender::Male);
        assert_all_read(&vals);
    }

    #[test]
    fn test_character_update() {
        let raw = "910323886/1039746/0/52/224712/341170/3789/200668/5/303/301/\
                   3/303/1/5/16/0/0/1/1/10/0/0/457/194/246/0/91796/0/0/75/78/\
                   443/433/176/28/0/551/361/268/0/0/366/365/108/0/0/0/0/0/0/0/\
                   0/0/0/0/0/0/0/0/0/0/0/1/0/0/11044/0/500/0";
        let data = Vals::from(raw);
        let mut char = Character::default();
        char.update(&data, ServerTime::default()).unwrap();

        assert_eq!(char.player_id, 1_039_746);
        assert_eq!(char.level, 52);
        assert_eq!(char.experience, 224_712);
        assert_eq!(char.next_level_xp, 341_170);
        assert_eq!(char.honor, 3789);
        assert_eq!(char.rank, 200_668);
        assert_eq!(char.race, Race::Human);
        assert_eq!(char.class, Class::Necromancer);
        assert_eq!(char.mount, None);
        assert_eq!(char.armor, 457);
        assert_eq!(char.min_damage, 194);
        assert_eq!(char.max_damage, 246);
        assert_eq!(char.attribute_basis[AttributeType::Strength], 75);
        assert_eq!(char.attribute_basis[AttributeType::Constitution], 433);
        assert_eq!(char.attribute_additions[AttributeType::Intelligence], 551);
        assert_eq!(
            char.attribute_times_bought[AttributeType::Intelligence],
            366
        );
        assert_eq!(char.portrait.mouth, 5);
        assert_eq!(char.portrait.hair_color, 3);
        assert_eq!(char.portrait.hair, 3);
        assert_eq!(char.portrait.gender, Gender::Male);
        assert_all_read(&data);
    }

    #[test]
    fn test_character_update_relations() {
        let raw = "101,BestFriend,AwesomeGuild,150,1;202,IgnoredPlayer,\
                   EnemyGuild,250,-1";
        let data = Vals::by(raw, ';');
        let mut char = Character::default();
        char.update_relations(&data);

        assert_eq!(char.relations.len(), 2);
        assert_eq!(char.relations[0].id, 101);
        assert_eq!(char.relations[0].name, "BestFriend");
        assert_eq!(char.relations[0].guild, "AwesomeGuild");
        assert_eq!(char.relations[0].level, 150);
        assert_eq!(char.relations[0].relation, Relationship::Friend);

        assert_eq!(char.relations[1].id, 202);
        assert_eq!(char.relations[1].name, "IgnoredPlayer");
        assert_eq!(char.relations[1].guild, "EnemyGuild");
        assert_eq!(char.relations[1].level, 250);
        assert_eq!(char.relations[1].relation, Relationship::Ignored);
        assert_all_read(&data);
    }

    #[test]
    fn test_relation_entry_parse() {
        let raw = "123,TestPlayer,TestGuild,42,1";
        let data = Vals::by(raw, ',');
        let entry = RelationEntry::parse(&data).unwrap();
        assert_eq!(entry.id, 123);
        assert_eq!(entry.name, "TestPlayer");
        assert_eq!(entry.guild, "TestGuild");
        assert_eq!(entry.level, 42);
        assert_eq!(entry.relation, Relationship::Friend);
        assert_all_read(&data);
    }
}
