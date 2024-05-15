use chrono::{DateTime, Local};
use enum_map::Enum;
use log::error;
use num_traits::FromPrimitive;
use strum::EnumIter;

use super::*;
use crate::{gamestate::items::*, misc::*, PlayerId};

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorEvent {
    pub start: Option<DateTime<Local>>,
    pub end: Option<DateTime<Local>>,
    pub collect_time_end: Option<DateTime<Local>>,
    pub active: Option<Hellevator>,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hellevator {
    pub key_cards: u32,
    pub current_floor: u32,
    pub points: u32,
    pub has_final_reward: bool,
    pub points_today: u32,
    pub next_card_generated: Option<DateTime<Local>>,
    pub next_reset: Option<DateTime<Local>>,
    pub start_contrib_date: Option<DateTime<Local>>,
}

impl Hellevator {
    pub(crate) fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Option<Hellevator>, SFError> {
        Ok(Some(Hellevator {
            key_cards: soft_into(data[0], "h key cards", 0),
            next_card_generated: server_time
                .convert_to_local(data[1], "next card"),
            next_reset: server_time.convert_to_local(data[2], "next reset"),
            current_floor: soft_into(data[3], "h current floor", 0),
            points: soft_into(data[4], "h points", 0),
            start_contrib_date: server_time
                .convert_to_local(data[5], "start contrib"),
            has_final_reward: data[6] == 1,
            points_today: soft_into(data[10], "h points today", 0),
        }))
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Witch {
    /// The item type the witch wants today
    pub required_item: Option<EquipmentSlot>,
    /// Whether or not the cauldron is bubbling
    pub cauldron_bubbling: bool,
    /// The enchant role collection progress from 0-100
    pub progress: u32,
    /// Whether or not each enchantment has been unlocked yet
    pub enchantments: EnumMap<Enchantment, bool>,
}

impl Witch {
    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.required_item = None;
        if data.cget(5, "w current item")? == 0 {
            self.required_item =
                ItemType::parse(data.skip(3, "witch item")?, server_time)?
                    .and_then(|a| a.equipment_slot());
        }
        if self.required_item.is_none() {
            self.cauldron_bubbling = true;
        } else {
            // I would like to offer the raw values here, but the -1 just
            // makes this annoying. A Option<(u32, u32)> is also weird
            if data[1] == -1 || data[2] < 1 {
                self.progress = 100;
            } else {
                let current = data[1] as f64;
                let target = data[2] as f64;
                self.progress = ((current / target) * 100.0) as u32;
            }
        }

        for i in 0..data[7] as usize {
            let iid = data[9 + 3 * i] - 1;
            let key = match iid {
                0 => continue,
                10 => Enchantment::SwordOfVengeance,
                30 => Enchantment::MariosBeard,
                40 => Enchantment::ManyFeetBoots,
                50 => Enchantment::ShadowOfTheCowboy,
                60 => Enchantment::AdventurersArchaeologicalAura,
                70 => Enchantment::ThirstyWanderer,
                80 => Enchantment::UnholyAcquisitiveness,
                90 => Enchantment::TheGraveRobbersPrayer,
                100 => Enchantment::RobberBaronRitual,
                x => {
                    warn!("Unknown witch enchant itemtype: {x}");
                    continue;
                }
            };
            *self.enchantments.get_mut(key) = true;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blacksmith {
    pub metal: u64,
    pub arcane: u64,
    pub dismantle_left: u8,
    /// This seems to keep track of when you last dismantled. No idea why
    pub last_dismantled: Option<DateTime<Local>>,
}

const PETS_PER_HABITAT: usize = 20;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pets {
    /// The total amount of pets collected in all habitats
    pub total_collected: u16,
    pub rank: u32,
    pub honor: u32,
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

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Habitat {
    /// The state of the exploration of this habitat
    pub exploration: HabitatExploration,
    /// The amount of fruits you have for this class
    pub fruits: u16,
    /// Has this habitat already fought an opponent today. If so, they can not
    /// do this until the next day
    pub battled_opponent: bool,
    pub pets: [Pet; PETS_PER_HABITAT],
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Represents the current state of the habitat exploration
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

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PetOpponent {
    pub id: PlayerId,
    pub pet_count: u32,
    pub level_total: u32,
    /// The next time a battle against this opponent will cost no mushroom
    pub next_free_battle: Option<DateTime<Local>>,
    /// The time the opponent was chosen
    pub reroll_date: Option<DateTime<Local>>,
    pub habitat: Option<HabitatType>,
}

impl Pets {
    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        let mut pet_id = 0;
        for (element_idx, element) in HabitatType::iter().enumerate() {
            let info = self.habitats.get_mut(element);
            let explored = data.csiget(210 + element_idx, "pet exp", 20)?;
            info.exploration = if explored == 20 {
                HabitatExploration::Finished
            } else {
                let next_lvl =
                    data.csiget(238 + element_idx, "next exp pet lvl", 1_000)?;
                HabitatExploration::Exploring {
                    fights_won: explored,
                    next_fight_lvl: next_lvl,
                }
            };
            for (pet_pos, pet) in info.pets.iter_mut().enumerate() {
                pet_id += 1;
                pet.id = pet_id;
                pet.level =
                    data.csiget((pet_id + 1) as usize, "pet level", 0)?;
                pet.fruits_today =
                    data.csiget((pet_id + 109) as usize, "pet fruits td", 0)?;
                pet.element = element;
                pet.can_be_found =
                    pet.level == 0 && explored as usize >= pet_pos;
            }
            info.battled_opponent =
                1 == data.cget(223 + element_idx, "element ff")?;
        }

        self.total_collected = soft_into(data[103], "total pets", 0);
        self.opponent.id = data[231].try_into().unwrap_or_default();
        self.opponent.next_free_battle =
            server_time.convert_to_local(data[232], "next free pet fight");
        self.rank = soft_into(data[233], "pet rank", 0);
        self.honor = soft_into(data[234], "pet honor", 0);

        self.opponent.pet_count = soft_into(data[235], "pet enemy count", 0);
        self.opponent.level_total =
            soft_into(data[236], "pet enemy lvl total", 0);
        self.opponent.reroll_date =
            server_time.convert_to_local(data[237], "pet enemy reroll date");

        update_enum_map(&mut self.atr_bonus, data.skip(250, "pet atr boni")?);
        Ok(())
    }

    pub(crate) fn update_pet_stat(&mut self, data: &[i64]) {
        if let Some(ps) = PetStats::parse(data) {
            let idx = ps.id;
            if let Some(pet) =
                self.habitats.get_mut(ps.element).pets.get_mut(idx % 20)
            {
                pet.stats = Some(ps);
            }
        } else {
            error!("Could not parse pet stats");
        }
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pet {
    pub id: u32,
    pub level: u16,
    /// The amount of fruits this pet got today
    pub fruits_today: u16,
    pub element: HabitatType,
    /// This is None until you look at your pets again
    pub stats: Option<PetStats>,
    /// Check if this pet can be found already
    pub can_be_found: bool,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PetStats {
    pub id: usize,
    pub level: u16,
    pub armor: u16,
    pub class: Class,
    pub attributes: EnumMap<AttributeType, u32>,
    pub bonus_attributes: EnumMap<AttributeType, u32>,
    pub min_damage: u16,
    pub max_damage: u16,
    pub element: HabitatType,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Enum, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HabitatType {
    #[default]
    Water = 0,
    Light = 1,
    Earth = 2,
    Shadow = 3,
    Fire = 4,
}

impl HabitatType {
    pub(crate) fn from_pet_id(id: i64) -> Option<Self> {
        use HabitatType::*;
        Some(match id {
            1..=20 => Shadow,
            21..=40 => Light,
            41..=60 => Earth,
            61..=80 => Fire,
            81..=100 => Water,
            _ => return None,
        })
    }

    pub(crate) fn from_typ_id(id: i64) -> Option<Self> {
        use HabitatType::*;
        Some(match id {
            1 => Shadow,
            2 => Light,
            3 => Earth,
            4 => Fire,
            5 => Water,
            _ => return None,
        })
    }
}

impl PetStats {
    pub(crate) fn parse(data: &[i64]) -> Option<Self> {
        let mut s = Self {
            id: soft_into(data[0], "pet index", 0),
            level: soft_into(data[1], "pet lvl", 0),
            armor: soft_into(data[2], "pet armor", 0),
            class: warning_parse(
                data[3],
                "pet class",
                FromPrimitive::from_i64,
            )?,
            min_damage: soft_into(data[14], "min damage", 0),
            max_damage: soft_into(data[15], "max damage", 0),

            element: match data[16] {
                0 => HabitatType::from_pet_id(data[0])?,
                x => HabitatType::from_typ_id(x)?,
            },
            ..Default::default()
        };
        update_enum_map(&mut s.attributes, &data[4..]);
        update_enum_map(&mut s.bonus_attributes, &data[9..]);
        Some(s)
    }
}

#[derive(Debug, Clone, Copy, strum::EnumCount, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mirror {
    Pieces {
        amount: u8,
    },
    #[default]
    Full,
}

impl Mirror {
    pub(crate) fn parse(i: i64) -> Mirror {
        if i & (1 << 8) != 0 {
            return Mirror::Full;
        }
        /// Bitmask to cover bits 20 to 32, which is where each bit set is one
        /// mirror piece found
        const MIRROR_PIECES_MASK: i64 = 0xFFF80000;
        Mirror::Pieces {
            amount: (i & MIRROR_PIECES_MASK).count_ones() as u8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Unlockable {
    /// Something like `Dungeon-key`
    pub main_ident: i64,
    /// Would be a specification of the main ident like for which dungeon
    pub sub_ident: i64,
}

impl Unlockable {
    pub(crate) fn parse(data: &[i64]) -> Vec<Unlockable> {
        data.chunks_exact(2)
            .filter(|chunk| chunk[0] != 0)
            .map(|chunk| Unlockable {
                main_ident: chunk[0],
                sub_ident: chunk[1],
            })
            .collect()
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Achievements(pub Vec<Achievement>);

impl Achievements {
    pub(crate) fn update(&mut self, data: &[i64]) -> Result<(), SFError> {
        self.0.clear();
        let total_count = data.len() / 2;
        if data.len() % 2 != 0 {
            warn!("achievement data has the wrong length: {}", data.len());
            return Ok(());
        }

        for i in 0..total_count {
            self.0.push(Achievement {
                achieved: data.cget(i, "achievement achieved")? == 1,
                progress: data.cget(i + total_count, "achievement achieved")?,
            });
        }
        Ok(())
    }

    /// The amount of achievements, that have been earned
    #[must_use]
    pub fn owned(&self) -> u32 {
        self.0.iter().map(|a| u32::from(a.achieved)).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A small challenge you can complete in the game
pub struct Achievement {
    /// Whether or not this achievement has been completed
    pub achieved: bool,
    /// The progress of doing this achievement
    pub progress: i64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Contains all the items & monsters you have found in the scrapbook
pub struct ScrapBook {
    /// All the items, that this player has already collected. To check if an
    /// item is in this, you should call `equipment_ident()` on an item and see
    /// if this item contains that
    pub items: HashSet<EquipmentIdent>,
    /// All the monsters, that the player has seen already. I have only checked
    /// this once, but this should match the tavern monster id.
    // TODO: Dungeon monster ids?
    pub monster: HashSet<u16>,
}

impl ScrapBook {
    // 99% based on Hubert Lipińskis Code
    // https://github.com/HubertLipinski/sfgame-scrapbook-helper
    pub(crate) fn parse(val: &str) -> Option<ScrapBook> {
        let text = base64::Engine::decode(
            &base64::engine::general_purpose::URL_SAFE,
            val,
        )
        .ok()?;
        if text.iter().all(|a| *a == 0) {
            return None;
        }

        let mut index = 0;
        let mut items = HashSet::new();
        let mut monster = HashSet::new();

        for byte in text {
            for bit_pos in (0..=7).rev() {
                index += 1;
                let is_owned = ((byte >> bit_pos) & 1) == 1;
                if !is_owned {
                    continue;
                }
                if index < 801 {
                    // Monster
                    monster.insert(index.try_into().unwrap_or_default());
                } else if let Some(ident) = parse_scrapbook_item(index) {
                    // Items
                    if !items.insert(ident) {
                        error!(
                            "Two scrapbook positions parsed to the same ident"
                        );
                    }
                } else {
                    error!("Owned, but not parsed: {index}");
                }
            }
        }
        Some(ScrapBook { items, monster })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The identification of items in the scrapbook
pub struct EquipmentIdent {
    /// The class the item has and thus the wearer must have
    pub class: Option<Class>,
    /// The position at which the item is worn
    pub typ: EquipmentSlot,
    /// The model id, this is basically the "name"" of the item
    pub model_id: u16,
    /// The color variation of this item
    pub color: u8,
}

#[allow(clippy::to_string_trait_impl)]
impl ToString for EquipmentIdent {
    fn to_string(&self) -> String {
        let item_typ = self.typ.raw_id();
        let model_id = self.model_id;
        let color = self.color;

        if let Some(class) = self.class {
            let ci = class as u8 + 1;
            format!("itm{item_typ}_{model_id}_{color}_{ci}")
        } else {
            format!("itm{item_typ}_{model_id}_{color}")
        }
    }
}

#[allow(clippy::enum_glob_use)]
fn parse_scrapbook_item(index: i64) -> Option<EquipmentIdent> {
    use Class::*;
    use EquipmentSlot::*;
    let slots: [(_, _, _, &[_]); 44] = [
        (801..1011, Amulet, None, &[]),
        (1011..1051, Amulet, None, &[]),
        (1051..1211, Ring, None, &[]),
        (1211..1251, Ring, None, &[]),
        (1251..1325, Talisman, None, &[]),
        (1325..1365, Talisman, None, &[]),
        (1365..1665, Weapon, Some(Warrior), &[]),
        (1665..1705, Weapon, Some(Warrior), &[]),
        (1705..1805, Shield, Some(Warrior), &[]),
        (1805..1845, Shield, Some(Warrior), &[]),
        (1845..1945, BreastPlate, Some(Warrior), &[]),
        (1945..1985, BreastPlate, Some(Warrior), &[1954, 1955]),
        (1985..2085, FootWear, Some(Warrior), &[]),
        (2085..2125, FootWear, Some(Warrior), &[2094, 2095]),
        (2125..2225, Gloves, Some(Warrior), &[]),
        (2225..2265, Gloves, Some(Warrior), &[2234, 2235]),
        (2265..2365, Hat, Some(Warrior), &[]),
        (2365..2405, Hat, Some(Warrior), &[2374, 2375]),
        (2405..2505, Belt, Some(Warrior), &[]),
        (2505..2545, Belt, Some(Warrior), &[2514, 2515]),
        (2545..2645, Weapon, Some(Mage), &[]),
        (2645..2685, Weapon, Some(Mage), &[]),
        (2685..2785, BreastPlate, Some(Mage), &[]),
        (2785..2825, BreastPlate, Some(Mage), &[2794, 2795]),
        (2825..2925, FootWear, Some(Mage), &[]),
        (2925..2965, FootWear, Some(Mage), &[2934, 2935]),
        (2965..3065, Gloves, Some(Mage), &[]),
        (3065..3105, Gloves, Some(Mage), &[3074, 3075]),
        (3105..3205, Hat, Some(Mage), &[]),
        (3205..3245, Hat, Some(Mage), &[3214, 3215]),
        (3245..3345, Belt, Some(Mage), &[]),
        (3345..3385, Belt, Some(Mage), &[3354, 3355]),
        (3385..3485, Weapon, Some(Scout), &[]),
        (3485..3525, Weapon, Some(Scout), &[]),
        (3525..3625, BreastPlate, Some(Scout), &[]),
        (3625..3665, BreastPlate, Some(Scout), &[3634, 3635]),
        (3665..3765, FootWear, Some(Scout), &[]),
        (3765..3805, FootWear, Some(Scout), &[3774, 3775]),
        (3805..3905, Gloves, Some(Scout), &[]),
        (3905..3945, Gloves, Some(Scout), &[3914, 3915]),
        (3945..4045, Hat, Some(Scout), &[]),
        (4045..4085, Hat, Some(Scout), &[4054, 4055]),
        (4085..4185, Belt, Some(Scout), &[]),
        (4185..4225, Belt, Some(Scout), &[4194, 4195]),
    ];

    let mut is_epic = true;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    for (range, typ, class, ignore) in slots {
        is_epic = !is_epic;
        if !range.contains(&index) {
            continue;
        }
        if ignore.contains(&index) {
            return None;
        }

        let relative_pos = index - range.start + 1;

        let color = match relative_pos % 10 {
            _ if typ == Talisman || is_epic => 1,
            0 => 5,
            1..=5 => relative_pos % 10,
            _ => relative_pos % 10 - 5,
        } as u8;

        let model_id = match () {
            _ if is_epic => relative_pos + 49,
            _ if typ == Talisman => relative_pos,
            _ if relative_pos % 5 != 0 => relative_pos / 5 + 1,
            _ => relative_pos / 5,
        } as u16;

        return Some(EquipmentIdent {
            class,
            typ,
            model_id,
            color,
        });
    }
    None
}
