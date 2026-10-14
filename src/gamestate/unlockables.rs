use std::{collections::HashSet, fmt::Display, num::NonZeroU8};

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::{error, warn};
use strum::IntoEnumIterator;

use super::{Class, EquipmentSlot, Result};
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
            for slot in EquipmentSlot::iter() {
                let id = i64::from(slot.raw_id());
                if id == raw_required {
                    self.required_item = Some(slot);
                    break;
                }
            }
        } else {
            data.ignore(34, "required item slot when cauldron is full");
        }

        let current = data.i32_or(2, 0);
        let target = data.i32_or(3, 0);
        if self.required_item.is_none() {
            self.cauldron_bubbling = true;
        } else {
            // I would like to offer the raw values here, but the -1 just
            // makes this annoying. A Option<(u32, u32)> is also weird
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            if current < 0 || target <= 0 {
                self.progress = 100;
            } else {
                let current = f64::from(current);
                let target = f64::from(target);
                self.progress = ((current / target) * 100.0) as u32;
            }
        }

        let e_count = data.u8_or(4, 0);
        for i in 0..e_count {
            let base = 5 + 3 * usize::from(i);
            data.ignore(base, "enchantment slot / index");
            let iid = data.i64_or(base + 1, 0) - 1;
            data.ignore(base + 2, "enchantment unlock timestamp");
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
        self.0.iter().map(|a| u32::from(a.achieved)).sum()
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

#[allow(clippy::enum_glob_use)]
fn parse_scrapbook_item(item_idx: i64) -> Option<EquipmentIdent> {
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

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    for (pos, (range, typ, class, ignore)) in slots.into_iter().enumerate() {
        if !range.contains(&item_idx) {
            continue;
        }
        if ignore.contains(&item_idx) {
            return None;
        }

        let is_epic = pos % 2 == 1;
        let relative_pos = item_idx - range.start + 1;

        let color = match relative_pos % 10 {
            _ if typ == Talisman || is_epic => 1,
            0 => 5,
            1..=5 => relative_pos % 10,
            _ => relative_pos % 10 - 5,
        } as u8;

        let model_id = match () {
            () if is_epic => relative_pos + 49,
            () if typ == Talisman => relative_pos,
            () if relative_pos % 5 != 0 => relative_pos / 5 + 1,
            () => relative_pos / 5,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
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
        let scrapbook =
            ScrapBook::parse(&data).expect("scrapbook should parse");
        assert_eq!(scrapbook.monster.len(), 95);
        assert_eq!(scrapbook.items.len(), 460);
        assert_all_read(&data);

        let empty_data = Vals::from("AAAAAAAAAA==");
        assert!(ScrapBook::parse(&empty_data).is_none());
        assert_all_read(&empty_data);
    }
}
