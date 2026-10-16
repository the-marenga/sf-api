use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use log::error;
use num_derive::FromPrimitive;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

use super::{AttributeType, ServerTime, character::Class};
#[cfg(feature = "simulation")]
use crate::gamestate::items::Equipment;
use crate::{
    PlayerId,
    error::{Result, SFError},
    response::Vals,
};

/// Information about the player's pet collection and habitats.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pets {
    /// The total amount of pets collected in all habitats
    pub total_collected: u16,
    /// The rank this pet collection achieved in the hall of fame
    pub rank: u32,
    /// The honor this pet collection has gained
    pub honor: u32,
    /// The maximum level any pet can currently reach.
    pub max_pet_level: u16,
    /// Information about the pvp opponent you can attack with your pets
    pub opponent: PetOpponent,
    /// Information about all the different habitats
    pub habitats: EnumMap<HabitatType, Habitat>,
    /// The next time the exploration will be possible without spending a
    /// mushroom
    pub next_free_exploration: Option<DateTime<Local>>,
    /// The bonus the player receives from pets
    pub atr_bonus: EnumMap<AttributeType, u32>,
}

/// Maps the index of the pet in their habitat to their base stats
#[cfg(feature = "simulation")]
static PET_BASE_STAT_ARRAY: [u32; 20] = [
    10, 11, 12, 13, 14, 16, 18, 20, 25, 30, 35, 40, 50, 60, 70, 80, 100, 130,
    160, 160,
];

/// Maps the habitat relative index of the pet to their class
#[cfg(feature = "simulation")]
#[rustfmt::skip]
static PET_CLASS_LOOKUP: EnumMap<HabitatType, [Class; 20]> =
    EnumMap::from_array([
        // Shadow
        [
            Class::Scout,   Class::Warrior, Class::Warrior, Class::Mage,
            Class::Mage,    Class::Mage,    Class::Scout,   Class::Scout,
            Class::Scout,   Class::Warrior, Class::Mage,    Class::Mage,
            Class::Scout,   Class::Scout,   Class::Warrior, Class::Warrior,
            Class::Mage,    Class::Warrior, Class::Warrior, Class::Scout,
        ],
        // Light
        [
            Class::Warrior, Class::Warrior, Class::Mage,    Class::Mage,
            Class::Scout,   Class::Scout,   Class::Mage,    Class::Warrior,
            Class::Warrior, Class::Mage,    Class::Mage,    Class::Scout,
            Class::Scout,   Class::Mage,    Class::Mage,    Class::Warrior,
            Class::Warrior, Class::Warrior, Class::Mage,    Class::Scout,
        ],
        // Earth
        [
            Class::Warrior, Class::Warrior, Class::Scout,   Class::Scout,
            Class::Warrior, Class::Scout,   Class::Mage,    Class::Mage,
            Class::Warrior, Class::Warrior, Class::Scout,   Class::Warrior,
            Class::Scout,   Class::Scout,   Class::Mage,    Class::Mage,
            Class::Mage,    Class::Warrior, Class::Warrior, Class::Warrior,
        ],
        // Fire
        [
            Class::Scout,   Class::Scout,   Class::Warrior, Class::Mage,
            Class::Mage,    Class::Scout,   Class::Scout,   Class::Mage,
            Class::Warrior, Class::Mage,    Class::Mage,    Class::Scout,
            Class::Scout,   Class::Scout,   Class::Scout,   Class::Scout,
            Class::Mage,    Class::Warrior, Class::Mage,    Class::Warrior,
        ],
        // Water
        [   Class::Mage,    Class::Warrior, Class::Warrior, Class::Warrior,
            Class::Warrior, Class::Scout,   Class::Warrior, Class::Scout,
            Class::Scout,   Class::Warrior, Class::Mage,    Class::Mage,
            Class::Mage,    Class::Warrior, Class::Mage,    Class::Mage,
            Class::Warrior, Class::Mage,    Class::Warrior, Class::Scout,
        ],
    ]);

impl Pets {
    /// Returns the enemy monster currently fought in habitat exploration, if
    /// any.
    #[cfg(feature = "simulation")]
    pub fn get_exploration_enemy(
        &self,
        habitat: HabitatType,
    ) -> Option<crate::simulate::Monster> {
        let h = &self.habitats[habitat];
        let stage = match h.exploration {
            HabitatExploration::Finished => return None,
            HabitatExploration::Exploring { fights_won, .. } => fights_won,
        };
        crate::simulate::constants::PET_MONSTER
            .get(&habitat)
            .and_then(|a| a.get((stage) as usize))
            .cloned()
    }

    /// Converts the given player pet into a fighter, usable in the simulation.
    /// The given pet does not need to have stats populated to work, since all
    /// stats will be dynamically calculated
    #[cfg(feature = "simulation")]
    #[must_use]
    pub fn pet_to_fighter(
        &self,
        pet: &Pet,
        gladiator: u32,
    ) -> crate::simulate::Fighter {
        let habitat_pets = &self.habitats[pet.element].pets;
        let unlocked_count =
            u32::try_from(habitat_pets.iter().filter(|a| a.level > 0).count())
                .unwrap_or(0);
        let pack_bonus = f64::from(unlocked_count) * 0.05;

        let level_bonus = habitat_pets
            .iter()
            .map(|p| match p.level {
                ..100 => 0.0,
                100..150 => 0.05,
                150..200 => 0.075,
                200.. => 0.1,
            })
            .sum::<f64>();

        let habitat_idx = habitat_pets
            .iter()
            .position(|a| a.id == pet.id)
            .unwrap_or(0);

        let base_stat =
            PET_BASE_STAT_ARRAY.get(habitat_idx).copied().unwrap_or(0);
        let high_stat = (f64::from(base_stat * (u32::from(pet.level) + 1))
            * (1.0 + pack_bonus + level_bonus))
            .floor();
        let low_stat = (0.5 * high_stat).round();
        let luck = (0.75 * high_stat).round();
        let con = high_stat;

        let class = *PET_CLASS_LOOKUP[pet.element]
            .get(habitat_idx)
            .unwrap_or(&Class::Warrior);

        let (str, dex, int) = match class {
            Class::Warrior => (high_stat, low_stat, low_stat),
            Class::Mage => (low_stat, low_stat, high_stat),
            _ => (low_stat, high_stat, low_stat),
        };

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let pet_fighter = crate::simulate::UpgradeableFighter {
            name: format!(
                "{:?} pet #{} ({}) ",
                pet.element,
                pet.id,
                habitat_idx + 1
            )
            .into(),
            class,
            level: pet.level,
            attribute_basis: EnumMap::from_array([
                str as u32,
                dex as u32,
                int as u32,
                con as u32,
                luck as u32,
            ]),
            is_companion: false,
            pet_attribute_bonus_perc: EnumMap::default(),
            equipment: Equipment::default(),
            active_potions: Default::default(),
            portal_hp_bonus: 0,
            portal_dmg_bonus: 0,
            gladiator,
        };
        (&pet_fighter).into()
    }

    /// Updates pets information from an `ownpets` response.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        data.ignore(0..2, "pet header fields");
        let mut pet_id = 0;
        for (element_idx, element) in HabitatType::iter().enumerate() {
            let info = &mut self.habitats[element];
            let explored = data.u32_or(210 + element_idx, 20);
            let next_lvl = data.u16_or(238 + element_idx, 1_000);
            info.exploration = if explored == 20 {
                HabitatExploration::Finished
            } else {
                HabitatExploration::Exploring {
                    fights_won: explored,
                    next_fight_lvl: next_lvl,
                }
            };
            for (pet_pos, pet) in info.pets.iter_mut().enumerate() {
                pet_id += 1;
                pet.id = pet_id;
                pet.level = data.u16_or(pet_id as usize + 1, 0);
                pet.fruits_today = data.u16_or(pet_id as usize + 109, 0);
                pet.element = element;
                pet.can_be_found =
                    pet.level == 0 && explored as usize >= pet_pos;
            }
            info.battled_opponent = data.u32_or(223 + element_idx, 0) == 1;
        }

        data.ignore(102, "pet 101 level slot");
        self.total_collected = data.u16_or(103, 0);
        data.ignore(104..110, "habitat pet counts or fruits");
        data.ignore(215..223, "unmodelled habitat exploration spacer");
        data.ignore(228..231, "unmodelled opponent spacer");

        self.opponent.id = data.u32_or(231, 0);
        self.opponent.next_free_battle = data.server_time(232, server_time)?;
        self.rank = data.u32_or(233, 0);
        self.honor = data.u32_or(234, 0);

        self.opponent.pet_count = data.u32_or(235, 0);
        self.opponent.level_total = data.u32_or(236, 0);
        self.opponent.reroll_date = data.server_time(237, server_time)?;

        data.ignore(243..250, "unmodelled exploration spacer");
        for (idx, atr) in self.atr_bonus.values_mut().enumerate() {
            *atr = data.u32_or(250 + idx, 0);
        }
        data.ignore(255..265, "unmodelled pet trailer");
        Ok(())
    }

    /// Updates the stats of a single pet from an `ownpetsstats` response.
    pub(crate) fn update_pet_stat(&mut self, data: &Vals<'_>) {
        match PetStats::parse(data) {
            Ok(ps) => {
                let idx = ps.id;
                // TODO: Potential off-by-one indexing bug: `ps.id` is 1-based
                // (1..=100). With `idx % 20`, Pet #1 maps to
                // index 1 (Pet #2), and Pet #20 maps to index 0
                // (Pet #1), placing every pet's stats into the wrong slot.
                // Consider matching by ID instead:
                // `self.habitats[ps.element].pets.iter_mut().find(|p| p.id as
                // usize == ps.id)`
                if let Some(pet) =
                    self.habitats[ps.element].pets.get_mut(idx % 20)
                {
                    pet.stats = Some(ps);
                }
            }
            Err(e) => {
                error!("Could not parse pet stats: {e}");
            }
        }
    }
}

/// A single pet habitat (e.g. Shadow, Light, Earth, Fire, Water).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Habitat {
    /// The state of the exploration of this habitat
    pub exploration: HabitatExploration,
    /// The amount of fruits you have for this class
    pub fruits: u16,
    /// Has this habitat already fought an opponent today. If so, they can not
    /// do this until the next day
    pub battled_opponent: bool,
    /// All the different pets you can collect in this habitat
    pub pets: [Pet; 20],
}

/// Represents the current state of the habitat exploration
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HabitatExploration {
    #[default]
    /// Explored/won all 20 habitat battles. This means you can no longer fight
    /// in the habitat
    Finished,
    /// The habitat has not yet been fully explored
    Exploring {
        /// The amount of pets you have already won fights against (explored)
        /// 0..=19
        fights_won: u32,
        /// The level of the next habitat exploration fight
        next_fight_lvl: u16,
    },
}

/// An opponent player for pet `PvP` battles.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PetOpponent {
    /// The player identifier of the opponent.
    pub id: PlayerId,
    /// The number of pets the opponent has in total.
    pub pet_count: u32,
    /// The total sum of pet levels of the opponent.
    pub level_total: u32,
    /// The next time a battle against this opponent will cost no mushroom
    pub next_free_battle: Option<DateTime<Local>>,
    /// The time the opponent was chosen
    pub reroll_date: Option<DateTime<Local>>,
    /// The element / habitat type defending in this battle, if known.
    pub habitat: Option<HabitatType>,
}

/// A single pet within a habitat.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pet {
    /// The unique id of this pet across all habitats (1..=101)
    pub id: u32,
    /// The level of this pet, or 0 if not yet unlocked.
    pub level: u16,
    /// The amount of fruits this pet got today
    pub fruits_today: u16,
    /// The habitat / element this pet belongs to.
    pub element: HabitatType,
    /// This is None until you look at your pets again
    pub stats: Option<PetStats>,
    /// Check if this pet can be found already
    pub can_be_found: bool,
}

/// Detailed attributes and combat statistics for a specific pet.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PetStats {
    /// The global index / identifier of the pet.
    pub id: usize,
    /// The level of the pet.
    pub level: u16,
    /// Total armor value.
    pub armor: u16,
    /// Combat class of the pet (Warrior, Mage, or Scout).
    pub class: Class,
    /// Primary attributes of the pet.
    pub attributes: EnumMap<AttributeType, u32>,
    /// Bonus attributes granted by the pet pack.
    pub bonus_attributes: EnumMap<AttributeType, u32>,
    /// Minimum attack damage.
    pub min_damage: u16,
    /// Maximum attack damage.
    pub max_damage: u16,
    /// Elemental habitat of the pet.
    pub element: HabitatType,
}

/// The five elemental habitats for pets.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Enum,
    EnumCount,
    EnumIter,
    Hash,
    FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HabitatType {
    /// Shadow element (Constitution).
    #[default]
    Shadow = 0,
    /// Light element (Dexterity).
    Light = 1,
    /// Earth element (Intelligence).
    Earth = 2,
    /// Fire element (Luck).
    Fire = 3,
    /// Water element (Strength).
    Water = 4,
}

impl From<HabitatType> for AttributeType {
    fn from(value: HabitatType) -> Self {
        match value {
            HabitatType::Water => AttributeType::Strength,
            HabitatType::Light => AttributeType::Dexterity,
            HabitatType::Earth => AttributeType::Intelligence,
            HabitatType::Shadow => AttributeType::Constitution,
            HabitatType::Fire => AttributeType::Luck,
        }
    }
}

impl HabitatType {
    pub(crate) fn from_pet_id(id: i64) -> Option<Self> {
        match id {
            1..=20 => Some(HabitatType::Shadow),
            21..=40 => Some(HabitatType::Light),
            41..=60 => Some(HabitatType::Earth),
            61..=80 => Some(HabitatType::Fire),
            81..=100 => Some(HabitatType::Water),
            _ => None,
        }
    }

    pub(crate) fn from_typ_id(id: i64) -> Option<Self> {
        match id {
            1 => Some(HabitatType::Shadow),
            2 => Some(HabitatType::Light),
            3 => Some(HabitatType::Earth),
            4 => Some(HabitatType::Fire),
            5 => Some(HabitatType::Water),
            _ => None,
        }
    }
}

impl PetStats {
    /// Parses pet statistics from a 17-value `ownpetsstats` response.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        let pet_id = data.u32_or(0, 0);
        let element_raw = data.i64(16)?;
        let element = match element_raw {
            0 => HabitatType::from_pet_id(i64::from(pet_id)).ok_or_else(
                || {
                    SFError::ParsingErrorV2(format!(
                        "Could not determine pet habitat type from pet id \
                         {pet_id}"
                    ))
                },
            )?,
            x => HabitatType::from_typ_id(x).ok_or_else(|| {
                SFError::ParsingErrorV2(format!(
                    "Unknown pet element habitat type: {x}"
                ))
            })?,
        };

        let mut attributes = EnumMap::default();
        for (idx, val) in attributes.values_mut().enumerate() {
            *val = data.u32_or(4 + idx, 0);
        }

        let mut bonus_attributes = EnumMap::default();
        for (idx, val) in bonus_attributes.values_mut().enumerate() {
            *val = data.u32_or(9 + idx, 0);
        }

        Ok(Self {
            id: pet_id as usize,
            level: data.u16_or(1, 0),
            armor: data.u16_or(2, 0),
            class: data.parse_enum::<Class>(3)?,
            attributes,
            bonus_attributes,
            min_damage: data.u16_or(14, 0),
            max_damage: data.u16_or(15, 0),
            element,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pets_update() {
        let raw = "446275/0/8/60/18/19/58/23/1/1/52/1/26/66/100/1/1/62/100/7/\
                   0/100/7/70/100/35/71/1/1/1/14/1/1/100/1/1/41/100/21/100/8/\
                   100/15/78/100/30/69/1/1/1/0/99/4/100/1/100/6/0/78/35/0/0/\
                   49/100/14/15/62/1/1/5/1/87/100/1/1/57/91/100/0/0/0/100/11/\
                   77/100/16/44/6/9/1/76/1/100/10/1/85/1/100/1/22/0/100/0/91/\
                   20/22/17/19/21/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                   0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                   0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                   0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/20/20/19/20/20/0/\
                   0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/105012/1760337462/7836/3220/\
                   19/586/1788552826/78/78/75/78/78/0/0/0/0/0/0/0/1424/1482/\
                   10365/6699/4671/0/0/0/0/0/0/0/0/0/3742";
        let mut pets = Pets::default();
        let server_time = ServerTime::default();
        let data = Vals::from(raw);
        pets.update(&data, server_time).unwrap();
        assert_eq!(pets.total_collected, 91);
        assert_eq!(pets.rank, 7836);
        assert_eq!(pets.honor, 3220);
        assert_eq!(pets.opponent.id, 105_012);
        assert_eq!(pets.atr_bonus[AttributeType::Strength], 1424);
        assert_eq!(pets.atr_bonus[AttributeType::Dexterity], 1482);
        assert_eq!(pets.atr_bonus[AttributeType::Intelligence], 10365);
        assert_eq!(pets.atr_bonus[AttributeType::Constitution], 6699);
        assert_eq!(pets.atr_bonus[AttributeType::Luck], 4671);
        data.assert_all_read();
    }

    #[test]
    fn test_pet_stats_parse() {
        let raw = "1/50/200/1/10/20/30/40/50/5/10/15/20/25/100/150/1";
        let data = Vals::from(raw);
        let stats = PetStats::parse(&data).unwrap();
        assert_eq!(stats.id, 1);
        assert_eq!(stats.level, 50);
        assert_eq!(stats.armor, 200);
        assert_eq!(stats.class, Class::Mage);
        assert_eq!(stats.attributes[AttributeType::Strength], 10);
        assert_eq!(stats.bonus_attributes[AttributeType::Strength], 5);
        assert_eq!(stats.min_damage, 100);
        assert_eq!(stats.max_damage, 150);
        assert_eq!(stats.element, HabitatType::Shadow);
        data.assert_all_read();
    }
}
