use chrono::{DateTime, Local};
use log::error;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use strum::EnumCount;

use super::*;
use crate::{
    gamestate::{dungeons::*, fortress::*, guild::*, items::*, underworld::*},
    misc::*,
    PlayerId,
};

/// All the aspects of the game you do not have at the start
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Unlockables {
    /// Whether this character has the mirror completed
    pub mirror: Mirror,
    pub scrapbok: Option<ScrapBook>,
    pub scrapbook_count: Option<u32>,
    pub dungeon_timer: Option<DateTime<Local>>,
    pub dungeons: Dungeons,
    pub portal: Option<Portal>,
    /// The companions unlocked from unlocking the tower. Note that the tower
    /// info itself is just handled as a normal light dungeon
    pub companions: Option<Companions>,
    pub underworld: Option<Underworld>,
    pub fortress: Option<Fortress>,
    pub pet_collection: Option<PetCollection>,
    pub blacksmith: Option<Blacksmith>,
    pub witch: Option<Witch>,
    pub hellevator: Option<Hellevator>,
    pub achievements: Achievements,
    pub guild: Option<Guild>,
    pub idle_game: Option<IdleGame>,

    /// Contains the features this is able to unlock right now
    pub pending_unlocks: Vec<Unlockable>,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hellevator {
    pub key_cards: u32,
    pub current_floor: u32,
    pub points: u32,
    pub has_final_reward: bool,
    pub points_today: u32,
    pub event_start: Option<DateTime<Local>>,
    pub event_end: Option<DateTime<Local>>,
    pub collect_time_end: Option<DateTime<Local>>,
    pub next_card_generated: Option<DateTime<Local>>,
    pub next_reset: Option<DateTime<Local>>,
    pub start_contrib_date: Option<DateTime<Local>>,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Witch {
    pub required_item: Option<ItemType>,
    pub cauldron_bubbling: bool,
    pub progress_prec: u32,
    pub enchant_roles: Vec<(ItemType, usize)>,
}

impl Witch {
    pub(crate) fn update(&mut self, data: &[i64], server_time: ServerTime) {
        self.required_item = None;
        if data[5] == 0 {
            self.required_item = ItemType::parse(&data[3..], server_time);
        }
        if self.required_item.is_none() {
            self.cauldron_bubbling = true;
        } else {
            // I would like to offer the raw values here, but the -1 just
            // makes this annoying. A Option<(u32, u32)> is also weird
            if data[1] == -1 || data[2] < 1 {
                self.progress_prec = 100;
            }
            let current = data[1] as f64;
            let target = data[2] as f64;
            self.progress_prec = ((current / target) * 100.0) as u32;
        }

        for i in 0..data[7] as usize {
            let iid = data[9 + 3 * i] - 1;
            if let Some(key) = match iid {
                10 => Some(ItemType::Weapon {
                    min_dmg: 0,
                    max_dmg: 0,
                }),
                30 => Some(ItemType::BreastPlate),
                40 => Some(ItemType::FootWear),
                50 => Some(ItemType::Gloves),
                60 => Some(ItemType::Hat),
                70 => Some(ItemType::Belt),
                80 => Some(ItemType::Amulet),
                90 => Some(ItemType::Ring),
                100 => Some(ItemType::Talisman),
                0 => None,
                x => {
                    warn!("Unknown witch enchant itemtype: {x}");
                    None
                }
            } {
                self.enchant_roles.push((key, i + 1));
            }
        }
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
pub struct PetCollection {
    pub total_collected: u16,
    pub next_free_fight: Option<DateTime<Local>>,
    pub rank: u32,
    pub honor: u32,
    pub max_pet_level: u16,
    pub remaining_pet_battles: u16,

    pub enemy_id: Option<PlayerId>,
    pub enemy_pet_count: u32,
    pub enemy_level_total: u32,

    pub enemy_reroll_date: Option<DateTime<Local>>,
    pub enemy_pet_type: Option<PetClass>,

    pub pets: [[Pet; PETS_PER_HABITAT]; PetClass::COUNT],

    pub explored_pets: [u32; PetClass::COUNT],
    /// The amount of fruits corresponding to that PetClass
    pub fruits: [u16; PetClass::COUNT],
    pub habitat_fights_finished: [bool; PetClass::COUNT],
    pub next_explore_pet_lvl: [u16; PetClass::COUNT],

    pub atr_bonus: Attributes,
    pub dungeon_timer: Option<DateTime<Local>>,
}

impl PetCollection {
    pub(crate) fn update(&mut self, data: &[i64], server_time: ServerTime) {
        for (idx, p) in data[210..215].iter().copied().enumerate() {
            self.explored_pets[idx] = soft_into(p, "pet exp", 0);
        }

        for i in 0..PETS_PER_HABITAT * PetClass::COUNT {
            let pet_id = (i + 1) as u32;
            let element = PetClass::from_pet_id(pet_id as i64).unwrap();
            self.pets[i / 20][i % 20] = Pet {
                index: pet_id,
                level: soft_into(data[i + 2], "pet level", 0),
                fruits_today: soft_into(data[i + 110], "fruits today", 0),
                element,
                can_be_found: data[i + 2] == 0
                    && 3.max(self.explored_pets[element as usize])
                        >= pet_id % 20,
                stats: None,
            };
        }

        self.total_collected = soft_into(data[103], "total pets", 0);

        for i in 0..5 {
            self.habitat_fights_finished[i] = data[223 + i] == 1;
        }

        self.enemy_id = data[231].try_into().ok();
        self.next_free_fight =
            server_time.convert_to_local(data[232], "next free pet fight");
        self.rank = soft_into(data[233], "pet rank", 0);
        self.honor = soft_into(data[234], "pet honor", 0);

        self.enemy_pet_count = soft_into(data[235], "pet enemy count", 0);
        self.enemy_level_total = soft_into(data[236], "pet enemy lvl total", 0);
        self.enemy_reroll_date =
            server_time.convert_to_local(data[237], "pet enemy reroll date");

        for (idx, lvl) in data[238..243].iter().enumerate() {
            self.next_explore_pet_lvl[idx] =
                soft_into(*lvl, "next exp pet lvl", 100)
        }

        self.atr_bonus.update(&data[250..]);
    }

    pub(crate) fn update_pet_stat(&mut self, data: &[i64]) {
        if let Some(ps) = PetStats::parse(data) {
            let idx = ps.index;
            self.pets[idx / 20][idx % 20].stats = Some(ps)
        } else {
            error!("Could not parse pet stats");
        }
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pet {
    pub index: u32,
    pub level: u16,
    /// The amount of fruits this pet got today
    pub fruits_today: u16,
    pub element: PetClass,
    /// This is None until you look at your pets again
    pub stats: Option<PetStats>,
    /// Check if this pet can be found already
    pub can_be_found: bool,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PetStats {
    pub index: usize,
    pub level: u16,
    pub armor: u16,
    pub class: Class,
    pub attributes: Attributes,
    pub bonus_attributes: Attributes,
    pub min_damage: u16,
    pub max_damage: u16,
    pub element: PetClass,
}

#[derive(Debug, Default, Clone, Copy, strum::EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PetClass {
    #[default]
    Water = 0,
    Light = 1,
    Earth = 2,
    Shadow = 3,
    Fire = 4,
}

impl PetClass {
    pub(crate) fn from_pet_id(id: i64) -> Option<Self> {
        use PetClass::*;
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
        use PetClass::*;
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
            index: soft_into(data[0], "pet index", 0),
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
                0 => PetClass::from_pet_id(data[0])?,
                x => PetClass::from_typ_id(x)?,
            },
            ..Default::default()
        };
        s.attributes.update(&data[4..]);
        s.bonus_attributes.update(&data[9..]);
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
    pub(crate) fn update(&mut self, data: &[i64]) {
        let total_count = data.len() / 2;
        if data.len() % 2 != 0 {
            warn!("achievement data has the wrong length: {}", data.len());
            return;
        }

        self.0.clear();
        for i in 0..total_count {
            self.0.push(Achievement {
                achieved: data[i] == 1,
                progress: data[i + total_count],
            });
        }
    }

    /// The amount of achievements, that have been earned
    pub fn owned(&self) -> u32 {
        self.0.iter().map(|a| a.achieved as u32).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Achievement {
    achieved: bool,
    progress: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapBook {
    /// All the items, that this player has already collected. To check if an
    /// item is in this, you should call equipment_ident() on an item and see
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
    pub fn parse(val: &str) -> ScrapBook {
        let text = base64::Engine::decode(
            &base64::engine::general_purpose::URL_SAFE,
            val,
        )
        .unwrap();

        let mut item_index = 0;
        let mut items = HashSet::new();
        let mut monster = HashSet::new();

        for byte in text.into_iter() {
            for bit_pos in (0..=7).rev() {
                item_index += 1;
                let is_owned = ((byte >> bit_pos) & 1) == 1;
                if is_owned {
                    if item_index < 801 {
                        // Monster
                        monster.insert(item_index as u16);
                        continue;
                    } else {
                        // Items
                        if let Some(ident) = parse_scrapbook_item(item_index) {
                            if !items.insert(ident) {
                                error!(
                                    "Two scrapbook positions parsed to the \
                                     same ident"
                                );
                            }
                        }
                    }
                }
            }
        }
        ScrapBook { items, monster }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EquipmentIdent {
    pub class: Option<Class>,
    pub typ: EquipmentSlot,
    pub model_id: u16,
    pub color: u8,
}

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

fn parse_scrapbook_item(index: i64) -> Option<EquipmentIdent> {
    use Class::*;
    use EquipmentSlot::*;
    let slots: [(_, _, _, &[_]); 44] = [
        (801..=905, Amulet, None, &[]),
        (1011..=1028, Amulet, None, &[]),
        (1051..=1130, Ring, None, &[]),
        (1211..=1228, Ring, None, &[]),
        (1251..=1287, Talisman, None, &[]),
        (1325..=1342, Talisman, None, &[]),
        (1365..=1514, Weapon, Some(Warrior), &[]),
        (1665..=1682, Weapon, Some(Warrior), &[]),
        (1705..=1754, Shield, Some(Warrior), &[]),
        (1805..=1822, Shield, Some(Warrior), &[]),
        (1845..=1894, BreastPlate, Some(Warrior), &[]),
        (1945..=1962, BreastPlate, Some(Warrior), &[1954, 1955]),
        (1985..=2034, FootWear, Some(Warrior), &[]),
        (2085..=2102, FootWear, Some(Warrior), &[2094, 2095]),
        (2125..=2174, Gloves, Some(Warrior), &[]),
        (2225..=2242, Gloves, Some(Warrior), &[2234, 2235]),
        (2265..=2314, Hat, Some(Warrior), &[]),
        (2365..=2382, Hat, Some(Warrior), &[2374, 2375]),
        (2405..=2454, Belt, Some(Warrior), &[]),
        (2505..=2522, Belt, Some(Warrior), &[2514, 2515]),
        (2545..=2594, Weapon, Some(Mage), &[]),
        (2645..=2662, Weapon, Some(Mage), &[]),
        (2685..=2734, BreastPlate, Some(Mage), &[]),
        (2785..=2802, BreastPlate, Some(Mage), &[2794, 2795]),
        (2825..=2874, FootWear, Some(Mage), &[]),
        (2925..=2942, FootWear, Some(Mage), &[2934, 2935]),
        (2965..=3014, Gloves, Some(Mage), &[]),
        (3065..=3082, Gloves, Some(Mage), &[3074, 3075]),
        (3105..=3154, Hat, Some(Mage), &[]),
        (3205..=3222, Hat, Some(Mage), &[3214, 3215]),
        (3245..=3294, Belt, Some(Mage), &[]),
        (3345..=3362, Belt, Some(Mage), &[3354, 3355]),
        (3385..=3434, Weapon, Some(Scout), &[]),
        (3485..=3502, Weapon, Some(Scout), &[]),
        (3525..=3574, BreastPlate, Some(Scout), &[]),
        (3625..=3642, BreastPlate, Some(Scout), &[3634, 3635]),
        (3665..=3714, FootWear, Some(Scout), &[]),
        (3765..=3782, FootWear, Some(Scout), &[3774, 3775]),
        (3805..=3854, Gloves, Some(Scout), &[]),
        (3905..=3922, Gloves, Some(Scout), &[3914, 3915]),
        (3945..=3994, Hat, Some(Scout), &[]),
        (4045..=4062, Hat, Some(Scout), &[4054, 4055]),
        (4085..=4134, Belt, Some(Scout), &[]),
        (4185..=4202, Belt, Some(Scout), &[4194, 4195]),
    ];

    let mut epic = true;
    for (range, typ, class, ignore) in slots {
        epic = !epic;
        if !range.contains(&index) {
            continue;
        }
        if ignore.contains(&index) {
            return None;
        }

        let len = range.end() - range.start() + 1;
        let index_max = *range.end();
        use EquipmentSlot::*;

        let model_id = len - (index_max - index);
        let color = match model_id % 10 {
            0 => 5,
            1..=5 => model_id % 10,
            _ => model_id % 10 - 5,
        };

        let model_id = if epic {
            model_id + 49
        } else if typ == Talisman {
            model_id
        } else if model_id % 5 != 0 {
            model_id / 5 + 1
        } else {
            model_id / 5
        };

        return Some(EquipmentIdent {
            class,
            typ,
            model_id: model_id as u16,
            color: if typ == Talisman || epic {
                1
            } else {
                color as u8
            },
        });
    }
    None
}
