use std::{collections::HashSet, fmt::Display, num::NonZeroU8};

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::warn;
use strum::IntoEnumIterator;

use super::{Result, ServerTime, character::Class, items::EquipmentSlot};
pub use super::{hellevator::*, pets::*};
use crate::{gamestate::items::Enchantment, response::Vals};

/// Information about the witch cauldron and unlocked enchantments.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Witch {
    /// The item type the witch wants today
    pub required_item: Option<EquipmentSlot>,
    /// Whether or not the cauldron is bubbling
    pub cauldron_bubbling: bool,
    /// The enchant role collection progress from 0-100
    pub progress: u32,
    /// The price in silver to enchant an item
    pub enchantment_price: u64,
    /// Contains the ident to use when you want to apply the enchantment. If
    /// this is `None`, the enchantment has not been unlocked yet
    pub enchantments: EnumMap<Enchantment, Option<EnchantmentIdent>>,
}

/// The S&F server needs a character specific value for enchanting items. This
/// is that value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnchantmentIdent(pub(crate) NonZeroU8);

impl Witch {
    /// Updates the witch state from a `witchshop` response.
    pub(crate) fn update(&mut self, data: &Vals<'_>) {
        data.ignore(0..2, "unknown witch timestamps");
        self.enchantment_price = data.u64_or(35, u64::MAX);
        self.required_item = None;
        if data.i64_or(33, 1) == 0 {
            let raw_required = data.i64_or(34, 0);
            self.required_item = EquipmentSlot::iter()
                .find(|slot| i64::from(slot.raw_id()) == raw_required);
        } else {
            data.ignore(34, "required item slot when cauldron is full");
        }

        let current = data.i32_or(2, 0);
        let target = data.i32_or(3, 0);
        if self.required_item.is_none() {
            self.cauldron_bubbling = true;
            self.progress = 100;
        } else if current < 0 || target <= 0 {
            self.progress = 100;
        } else {
            let current = current as u64;
            let target = target as u64;
            self.progress = ((current * 100) / target).min(100) as u32;
        }

        let e_count = data.u8_or(4, 0);
        for i in 0..e_count {
            let base = 5 + 3 * usize::from(i);
            data.ignore(base, "enchantment slot / index");
            let raw_iid = data.i64_or(base + 1, 0);
            data.ignore(base + 2, "enchantment unlock timestamp");
            if raw_iid == 0 {
                continue;
            }
            let Some(key) = num_traits::FromPrimitive::from_i64(raw_iid) else {
                warn!("Unknown witch enchant itemtype: {raw_iid}");
                continue;
            };
            if let Some(val) = NonZeroU8::new(i + 1) {
                self.enchantments[key] = Some(EnchantmentIdent(val));
            }
        }
        let enchants_end = 5 + 3 * usize::from(e_count);
        if enchants_end < 32 {
            data.ignore(enchants_end..32, "unlocked enchantment slots");
        }
        data.ignore(32, "unknown witch field");
    }
}

/// Information about the blacksmith resources and daily dismantling limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blacksmith {
    /// The amount of metal resource currently stored.
    pub metal: u64,
    /// The amount of arcane splinters resource currently stored.
    pub arcane: u64,
    /// The number of item dismantles remaining today.
    pub dismantle_left: u8,
    /// This seems to keep track of when you last dismantled. No idea why
    pub last_dismantled: Option<DateTime<Local>>,
}

impl Blacksmith {
    /// Updates the blacksmith from a `smith` response:
    /// `[dismantles_left, last_dismantled, ...]`.
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        self.dismantle_left = data.u8_or(0, 0);
        self.last_dismantled = data.server_time_opt(1, server_time);
        if data.len() > 2 {
            data.ignore(2, "unknown smith field");
        }
    }
}

/// A pending feature unlock received from the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Unlockable {
    /// Something like `Dungeon-key`
    pub main_ident: i64,
    /// Would be a specification of the main ident like for which dungeon
    pub sub_ident: i64,
}

impl Unlockable {
    /// Parses a list of unlockable feature pairs `[main_ident, sub_ident]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Vec<Unlockable>> {
        let mut unlocks = Vec::new();
        for chunk in data.chunks(2) {
            let main_ident = chunk.i64(0)?;
            let sub_ident = chunk.i64(1)?;
            if main_ident != 0 {
                unlocks.push(Unlockable {
                    main_ident,
                    sub_ident,
                });
            }
        }
        Ok(unlocks)
    }
}

/// The current progress towards all achievements
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Achievements(pub Vec<Achievement>);

impl Achievements {
    /// Updates the achievements from an `achievement` response.
    pub(crate) fn update(&mut self, data: &Vals<'_>) {
        self.0.clear();
        let total_count = data.len() / 2;
        if !data.len().is_multiple_of(2) {
            warn!("achievement data has the wrong length: {}", data.len());
            data.ignore_all("wrong length achievement data");
            return;
        }

        for i in 0..total_count {
            self.0.push(Achievement {
                achieved: data.u32_or(i, 0) == 1,
                progress: data.i64_or(i + total_count, 0),
            });
        }
    }

    /// The amount of achievements, that have been earned
    #[must_use]
    pub fn owned(&self) -> u32 {
        self.0.iter().filter(|a| a.achieved).count() as u32
    }
}

impl std::ops::Deref for Achievements {
    type Target = [Achievement];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for Achievements {
    type Item = Achievement;
    type IntoIter = std::vec::IntoIter<Achievement>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Achievements {
    type Item = &'a Achievement;
    type IntoIter = std::slice::Iter<'a, Achievement>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// A small challenge you can complete in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Achievement {
    /// Whether or not this achievement has been completed
    pub achieved: bool,
    /// The progress of doing this achievement
    pub progress: i64,
}

/// Contains all the items & monsters you have found in the scrapbook
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Parses scrapbook collected item and monster IDs from a base64 encoded
    /// response.
    // 99% based on Hubert Lipińskis Code
    // https://github.com/HubertLipinski/sfgame-scrapbook-helper
    pub(crate) fn parse(data: &Vals<'_>) -> Option<ScrapBook> {
        let val = data.str(0).ok()?;
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
                        warn!(
                            "Two scrapbook positions parsed to the same \
                             ident: {index}"
                        );
                    }
                } else {
                    warn!("Owned, but not parsed: {index}");
                }
            }
        }
        Some(ScrapBook { items, monster })
    }
}

/// The identification of items in the scrapbook
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl Display for EquipmentIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let item_typ = self.typ.raw_id();
        let model_id = self.model_id;
        let color = self.color;

        if let Some(class) = self.class {
            let ci = class as u8 + 1;
            write!(f, "itm{item_typ}_{model_id}_{color}_{ci}")
        } else {
            write!(f, "itm{item_typ}_{model_id}_{color}")
        }
    }
}

const SCRAPBOOK_SLOTS: [(i64, EquipmentSlot, Option<Class>, bool); 44] = {
    use EquipmentSlot as ES;
    [
        (1011, ES::Amulet, None, false),
        (1051, ES::Amulet, None, false),
        (1211, ES::Ring, None, false),
        (1251, ES::Ring, None, false),
        (1325, ES::Talisman, None, false),
        (1365, ES::Talisman, None, false),
        (1665, ES::Weapon, Some(Class::Warrior), false),
        (1705, ES::Weapon, Some(Class::Warrior), false),
        (1805, ES::Shield, Some(Class::Warrior), false),
        (1845, ES::Shield, Some(Class::Warrior), false),
        (1945, ES::BreastPlate, Some(Class::Warrior), false),
        (1985, ES::BreastPlate, Some(Class::Warrior), true),
        (2085, ES::FootWear, Some(Class::Warrior), false),
        (2125, ES::FootWear, Some(Class::Warrior), true),
        (2225, ES::Gloves, Some(Class::Warrior), false),
        (2265, ES::Gloves, Some(Class::Warrior), true),
        (2365, ES::Hat, Some(Class::Warrior), false),
        (2405, ES::Hat, Some(Class::Warrior), true),
        (2505, ES::Belt, Some(Class::Warrior), false),
        (2545, ES::Belt, Some(Class::Warrior), true),
        (2645, ES::Weapon, Some(Class::Mage), false),
        (2685, ES::Weapon, Some(Class::Mage), false),
        (2785, ES::BreastPlate, Some(Class::Mage), false),
        (2825, ES::BreastPlate, Some(Class::Mage), true),
        (2925, ES::FootWear, Some(Class::Mage), false),
        (2965, ES::FootWear, Some(Class::Mage), true),
        (3065, ES::Gloves, Some(Class::Mage), false),
        (3105, ES::Gloves, Some(Class::Mage), true),
        (3205, ES::Hat, Some(Class::Mage), false),
        (3245, ES::Hat, Some(Class::Mage), true),
        (3345, ES::Belt, Some(Class::Mage), false),
        (3385, ES::Belt, Some(Class::Mage), true),
        (3485, ES::Weapon, Some(Class::Scout), false),
        (3525, ES::Weapon, Some(Class::Scout), false),
        (3625, ES::BreastPlate, Some(Class::Scout), false),
        (3665, ES::BreastPlate, Some(Class::Scout), true),
        (3765, ES::FootWear, Some(Class::Scout), false),
        (3805, ES::FootWear, Some(Class::Scout), true),
        (3905, ES::Gloves, Some(Class::Scout), false),
        (3945, ES::Gloves, Some(Class::Scout), true),
        (4045, ES::Hat, Some(Class::Scout), false),
        (4085, ES::Hat, Some(Class::Scout), true),
        (4185, ES::Belt, Some(Class::Scout), false),
        (4225, ES::Belt, Some(Class::Scout), true),
    ]
};

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_scrapbook_item(item_idx: i64) -> Option<EquipmentIdent> {
    let mut start = 801;
    for (pos, (end, typ, class, has_ignore)) in
        SCRAPBOOK_SLOTS.iter().enumerate()
    {
        if item_idx >= *end {
            start = *end;
            continue;
        }
        if item_idx < start {
            return None;
        }
        let relative_pos = item_idx - start + 1;
        if *has_ignore && matches!(relative_pos, 10 | 11) {
            return None;
        }

        let is_epic = pos % 2 == 1;

        let color = if *typ == EquipmentSlot::Talisman || is_epic {
            1
        } else {
            match relative_pos % 10 {
                0 => 5,
                1..=5 => relative_pos % 10,
                _ => relative_pos % 10 - 5,
            }
        } as u8;

        let model_id = if is_epic {
            relative_pos + 49
        } else if *typ == EquipmentSlot::Talisman {
            relative_pos
        } else {
            (relative_pos + 4) / 5
        } as u16;

        return Some(EquipmentIdent {
            class: *class,
            typ: *typ,
            model_id,
            color,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

    #[test]
    fn test_unlockable_parse() {
        let data = Vals::from("1/2/3/4");
        let parsed = Unlockable::parse(&data).unwrap();
        assert_eq!(
            parsed,
            vec![
                Unlockable {
                    main_ident: 1,
                    sub_ident: 2,
                },
                Unlockable {
                    main_ident: 3,
                    sub_ident: 4,
                },
            ]
        );
        assert_all_read(&data);
    }

    #[test]
    fn test_witch_update() {
        let raw = "1788480000/1391734661/-1/-1/9/6/51/1386180831/7/11/\
                   1387304577/0/61/1387815766/2/41/1388352596/8/101/\
                   1389068503/1/71/1389696713/3/91/1390336047/4/81/1391019206/\
                   5/31/1391734731/9/0/9/238037800";
        let mut witch = Witch::default();
        let data = Vals::from(raw);
        witch.update(&data);
        assert_eq!(witch.enchantment_price, 238_037_800);
        assert_eq!(witch.required_item, Some(EquipmentSlot::Ring));
        assert!(!witch.cauldron_bubbling);
        assert!(witch.enchantments[Enchantment::SwordOfVengeance].is_some());
        assert!(witch.enchantments[Enchantment::ShadowOfTheCowboy].is_some());
        assert_all_read(&data);
    }

    #[test]
    fn test_scrapbook_parse() {
        let raw = concat!(
            "vXx122GYRUavcz8hEZv7spxFkShAgAAAAAAAAAAAAAAAAAAAAAAA_8AAAAAA",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAAAAAAAP1__7v0OKV9gAAAAAAAAAAAAAAAAAAAAAAAHgIAAAA-_3nv",
            "-_dgAAAAAAAAAAAAAAAAAD4AAAAAP_6EAAAAAAAADYAEAAAAAAKABCAACWSk",
            "DsxhAQAwLAAAAAAAAAAAAAAAAAAAAAAAAAAAAIAAAAAAAIAyP6kAAAAAAAAA",
            "CIAAAAAAIgVAzmGEAAAAAAAACAAAAAAgARbzkAgAAAAAAAAEAAAAAAAANtSs",
            "cAAAAAAAAAAIAAAAAACJhPLgAEAAAAAAAAcAAAAAAAgIbdigEAAAAAAAAFAA",
            "AAAAAgtP-FgAAAAAAAAABAAAAAABCJ_9_SAAAAAAAAAAmAAAAACHn-3rdAAA",
            "AAAAAAAIgAAAAAQD_2bwQAAAAAAAAAD4AAAAAAYefuuQAAAAAAAAAA-AAAAA",
            "AAx5Z2ZQAAAAAAAAAOgEAAAAADCJMCMAAAAAAAAAAAAAAAAAAIbapBAAAAAA",
            "AAAAkAAAAAAAEdjcAgAAAAAAAAAAgAAAAAADAWKkAAAAAAAAAAAwAAAAAAAS",
            "2SoEAAAAAAAAAAgAAAAAAADFXAqAAAAAAAAAAAgAAAAAAA==",
        );
        let data = Vals::from(raw);
        let scrapbook = ScrapBook::parse(&data).unwrap();
        assert_eq!(scrapbook.monster.len(), 95);
        assert_eq!(scrapbook.items.len(), 460);
        assert_all_read(&data);

        let empty_data = Vals::from("AAAAAAAAAA==");
        assert!(ScrapBook::parse(&empty_data).is_none());
        assert_all_read(&empty_data);
    }

    #[test]
    fn test_achievements_deref_and_iter() {
        let achievements = Achievements(vec![
            Achievement {
                achieved: true,
                progress: 10,
            },
            Achievement {
                achieved: false,
                progress: 5,
            },
        ]);
        assert_eq!(achievements.len(), 2);
        assert!(!achievements.is_empty());
        assert_eq!(achievements[0].progress, 10);
        assert_eq!(achievements.owned(), 1);
        let count = (&achievements).into_iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_parse_scrapbook_item() {
        assert!(parse_scrapbook_item(800).is_none());
        assert!(parse_scrapbook_item(4225).is_none());

        let first = parse_scrapbook_item(801).unwrap();
        assert_eq!(first.typ, EquipmentSlot::Amulet);
        assert_eq!(first.class, None);
        assert_eq!(first.model_id, 1);

        // Ignored epic items in warrior breastplate range (1945..1985)
        assert!(parse_scrapbook_item(1953).is_some());
        assert!(parse_scrapbook_item(1954).is_none());
        assert!(parse_scrapbook_item(1955).is_none());
        assert!(parse_scrapbook_item(1956).is_some());
    }

    #[test]
    fn test_blacksmith_update() {
        let raw = "5/1600000000/0";
        let data = Vals::from(raw);
        let mut bs = Blacksmith::default();
        bs.update(&data, ServerTime::default());
        assert_eq!(bs.dismantle_left, 5);
        assert!(bs.last_dismantled.is_some());
        assert_all_read(&data);
    }
}
