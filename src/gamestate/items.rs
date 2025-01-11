use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use log::{error, warn};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::EnumIter;

use super::{
    unlockables::EquipmentIdent, ArrSkip, CFPGet, Class, EnumMapGet,
    HabitatType, SFError, ServerTime,
};
use crate::{
    command::AttributeType,
    gamestate::{CCGet, CGet, CSTGet},
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The basic inventory, that every player has
pub struct Inventory {
    /// The basic 5 item slots, that everybody has
    pub bag: [Option<Item>; 5],
    /// Item slots obtained from the fortress. None means not unlocked, and
    /// `len()` is the amount of slots unlocked
    pub fortress_chest: Option<Vec<Option<Item>>>,
}

impl Inventory {
    /// Returns a place in the inventory, that can store a new item
    #[must_use]
    pub fn free_slot(&self) -> Option<(InventoryType, usize)> {
        if let Some(bag_pos) = self.bag.iter().position(Option::is_none) {
            return Some((InventoryType::MainInventory, bag_pos));
        } else if let Some(e_bag_pos) = self
            .fortress_chest
            .as_ref()
            .and_then(|a| a.iter().position(Option::is_none))
        {
            return Some((InventoryType::ExtendedInventory, e_bag_pos));
        }
        None
    }

    #[must_use]
    pub fn count_free_slots(&self) -> usize {
        let bag_free_slots =
            self.bag.iter().filter(|slot| slot.is_none()).count();
        let fortress_chest_free_slots =
            self.fortress_chest.as_ref().map_or(0, |chest| {
                chest.iter().filter(|slot| slot.is_none()).count()
            });
        bag_free_slots + fortress_chest_free_slots
    }

    pub(crate) fn update_fortress_chest(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.fortress_chest = None;
        if data.is_empty() {
            return Ok(());
        }
        if data.len() % 12 != 0 {
            error!("Wrong fortess chest response size:  {data:?}");
        }
        self.fortress_chest = Some(
            data.chunks_exact(12)
                .map(|a| Item::parse(a, server_time))
                .collect::<Result<Vec<_>, SFError>>()?,
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// All the parts of `ItemPlace`, that are owned by the player
pub enum PlayerItemPlace {
    Equipment = 1,
    MainInventory = 2,
    ExtendedInventory = 5,
}

impl PlayerItemPlace {
    /// `InventoryType` is a subset of `ItemPlace`. This is a convenient
    /// function to convert between them
    #[must_use]
    pub fn item_position(&self) -> ItemPlace {
        match self {
            PlayerItemPlace::Equipment => ItemPlace::Equipment,
            PlayerItemPlace::MainInventory => ItemPlace::MainInventory,
            PlayerItemPlace::ExtendedInventory => ItemPlace::FortressChest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// All the parts of `ItemPlace`, that are owned by the player
pub enum InventoryType {
    MainInventory = 2,
    ExtendedInventory = 5,
}

impl InventoryType {
    /// `InventoryType` is a subset of `ItemPlace`. This is a convenient
    /// function to convert between them
    #[must_use]
    pub fn item_position(&self) -> ItemPlace {
        match self {
            InventoryType::MainInventory => ItemPlace::MainInventory,
            InventoryType::ExtendedInventory => ItemPlace::FortressChest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// All places, that items can be dragged to excluding companions
pub enum ItemPlace {
    /// The stuff a player can wear
    Equipment = 1,
    /// All items in the main 5 inventory slots
    MainInventory = 2,
    /// The items in the weapon slot
    WeaponShop = 3,
    /// The items in the mage slot
    MageShop = 4,
    /// The items in the fortress chest slots
    FortressChest = 5,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// All the equipment a player is wearing
pub struct Equipment(pub EnumMap<EquipmentSlot, Option<Item>>);

impl Equipment {
    #[must_use]
    /// Checks if the character has an item with the enchantment equipped
    pub fn has_enchantment(&self, enchantment: Enchantment) -> bool {
        let item = self.0.get(enchantment.equipment_slot());
        if let Some(item) = item {
            return item.enchantment == Some(enchantment);
        }
        false
    }

    /// Expects the input `data` to have items directly at data[0]
    pub(crate) fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Equipment, SFError> {
        let mut res = Equipment::default();
        for (idx, map_val) in res.0.as_mut_slice().iter_mut().enumerate() {
            *map_val =
                Item::parse(data.skip(idx * 12, "equipment")?, server_time)?;
        }
        Ok(res)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about a single item. This can be anything, that is either in a
/// inventory, in a reward slot, or similar
pub struct Item {
    /// The type of this item. May contain further type specific values
    pub typ: ItemType,
    /// Either the price to buy, or sell
    pub price: u32,
    /// The price you would have to pay for this item. Note that this value is
    /// junk for other players and potentially in other cases, where you should
    /// not be able to see a price
    pub mushroom_price: u32,
    /// The model id of this item
    pub model_id: u16,
    /// The class restriction, that this item has. Will only cover the three
    /// main classes
    pub class: Option<Class>,
    /// Either the armor, weapon dmg, or other. You should be using `armor()`,
    /// or the weapon types damages though, if you want to have a safe
    /// abstraction. This is only public in case I am missing a case here
    pub type_specific_val: u32,
    /// The stats this item gives, when equipped
    pub attributes: EnumMap<AttributeType, u32>,
    /// The gemslot of this item, if any. A gemslot can be filled or empty
    pub gem_slot: Option<GemSlot>,
    /// The rune on this item
    pub rune: Option<Rune>,
    /// The enchantment applied to this item
    pub enchantment: Option<Enchantment>,
    /// This is the color, or other cosmetic variation of an item. There is no
    /// clear 1 => red mapping, so only the raw value here
    pub color: u8,
}

impl Item {
    /// Maps an item to its ident. This is mainly useful, if you want to see,
    /// if a item is already in your scrapbook
    #[must_use]
    pub fn equipment_ident(&self) -> Option<EquipmentIdent> {
        Some(EquipmentIdent {
            class: self.class,
            typ: self.typ.equipment_slot()?,
            model_id: self.model_id,
            color: self.color,
        })
    }

    /// Checks, if this item is unique. Technically they are not always unique,
    /// as the scrapbook/keys can be sold, but it should be clear what this is
    #[must_use]
    pub fn is_unique(&self) -> bool {
        self.typ.is_unique()
    }

    /// Checks if this item is an epic
    #[must_use]
    pub fn is_epic(&self) -> bool {
        self.model_id >= 50
    }

    /// Checks if this item is a legendary
    #[must_use]
    pub fn is_legendary(&self) -> bool {
        self.model_id >= 90
    }

    /// The armor rating of this item. This is just the `effect_val`, if any
    #[must_use]
    pub fn armor(&self) -> u32 {
        #[allow(clippy::enum_glob_use)]
        use ItemType::*;
        match self.typ {
            Hat | BreastPlate | Gloves | FootWear | Amulet | Belt | Ring
            | Talisman => self.type_specific_val,
            _ => 0,
        }
    }

    /// Checks, if this item can be enchanted
    #[must_use]
    pub fn is_enchantable(&self) -> bool {
        self.typ.is_enchantable()
    }

    /// Checks if a companion of the given class can equip this item.
    ///
    /// Returns `true` if the item itself is equipment and this class has the
    /// ability to wear it
    #[must_use]
    pub fn can_be_equipped_by_companion(
        &self,
        class: impl Into<Class>,
    ) -> bool {
        !self.typ.is_shield() && self.can_be_equipped_by(class.into())
    }

    /// Checks if a character of the given class can equip this item. Note that
    /// this only checks the class, so this will make no sense if you use this
    /// for anything that can not equip items at all (monsters, etc.). For
    /// companions you should use `can_companion_equip`
    ///
    /// Returns `true` if the item itself is equipment and this class has the
    /// ability to wear it
    #[must_use]
    pub fn can_be_equipped_by(&self, class: Class) -> bool {
        self.typ.equipment_slot().is_some() && self.can_be_used_by(class)
    }

    /// Checks if a character of the given class can use this item. If you want
    /// to check equipment, you should use `can_be_equipped_by`
    ///
    /// Returns `true` if the item does not have a class requirement, or if the
    /// class requirement matches the given class.
    #[must_use]
    #[allow(clippy::enum_glob_use, clippy::match_same_arms)]
    pub fn can_be_used_by(&self, class: Class) -> bool {
        use Class::*;

        // Without a class requirement any class can use this
        let Some(class_requirement) = self.class else {
            return true;
        };

        // Class requirements
        // Warrior => Weapon: Meele,  Armor: Heavy
        // Scout   => Weapon: Ranged, Armor: Medium
        // Mage    => Weapon: Magic,  Armor: Light
        match (class, class_requirement) {
            // Weapon: Meele, Armor: Heavy
            (Warrior, Warrior) => true,
            (Berserker, Warrior) => !self.typ.is_shield(),
            // Weapon: Ranged, Armor: Medium
            (Scout, Scout) => true,
            // Weapon: Magic, Armor: Light
            (Mage | Necromancer, Mage) => true,
            // Weapon: Meele, Armor: Medium
            (Assassin, Warrior) => self.typ.is_weapon(),
            (Assassin, Scout) => !self.typ.is_weapon(),
            // Weapon: Magic, Armor: Medium
            (Bard | Druid, Mage) => self.typ.is_weapon(),
            (Bard | Druid, Scout) => !self.typ.is_weapon(),
            // Weapon: Meele, Armor: Light
            (BattleMage, Warrior) => self.typ.is_weapon(),
            (BattleMage, Mage) => !self.typ.is_weapon(),
            // Weapon: Ranged, Armor: Heavy
            (DemonHunter, Scout) => self.typ.is_weapon(),
            (DemonHunter, Warrior) => {
                !self.typ.is_weapon() && !self.typ.is_shield()
            }
            _ => false,
        }
    }

    /// Parses an item, that starts at the start of the given data
    pub(crate) fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Option<Self>, SFError> {
        let Some(typ) = ItemType::parse(data, server_time)? else {
            return Ok(None);
        };

        let enchantment = data.cfpget(0, "item enchantment", |a| a >> 24)?;
        let gem_slot_val =
            data.cimget(0, "gem slot val", |a| (a >> 16) & 0xFF)?;
        let gem_pwr = data.cimget(11, "gem pwr", |a| a >> 16)?;
        let gem_slot = GemSlot::parse(gem_slot_val, gem_pwr);

        let class = if typ.is_class_item() {
            data.cfpget(1, "item class", |x| (x & 0xFFFF) / 1000)?
        } else {
            None
        };
        let mut rune = None;
        let mut attributes: EnumMap<AttributeType, u32> = EnumMap::default();
        if typ.equipment_slot().is_some() {
            for i in 0..3 {
                let atr_typ = data.cget(i + 4, "item atr typ")?;
                let Ok(atr_typ) = atr_typ.try_into() else {
                    warn!("Invalid attribute typ: {atr_typ}, {typ:?}");
                    continue;
                };
                let atr_val = data.cget(i + 7, "item atr val")?;
                let Ok(atr_val): Result<u32, _> = atr_val.try_into() else {
                    warn!("Invalid attribute value: {atr_val}, {typ:?}");
                    continue;
                };

                match atr_typ {
                    0 => {}
                    1..=5 => {
                        let Some(atr_typ) = FromPrimitive::from_usize(atr_typ)
                        else {
                            continue;
                        };
                        *attributes.get_mut(atr_typ) = atr_val;
                    }
                    6 => {
                        attributes.as_mut_array().fill(atr_val);
                    }
                    21 => {
                        for atr in [
                            AttributeType::Strength,
                            AttributeType::Constitution,
                            AttributeType::Luck,
                        ] {
                            *attributes.get_mut(atr) = atr_val;
                        }
                    }
                    22 => {
                        for atr in [
                            AttributeType::Dexterity,
                            AttributeType::Constitution,
                            AttributeType::Luck,
                        ] {
                            *attributes.get_mut(atr) = atr_val;
                        }
                    }
                    23 => {
                        for atr in [
                            AttributeType::Intelligence,
                            AttributeType::Constitution,
                            AttributeType::Luck,
                        ] {
                            *attributes.get_mut(atr) = atr_val;
                        }
                    }
                    rune_typ => {
                        let Some(typ) = FromPrimitive::from_usize(rune_typ)
                        else {
                            warn!(
                                "Unhandled item val: {atr_typ} -> {atr_val} \
                                 for {class:?} {typ:?}",
                            );
                            continue;
                        };
                        let Ok(value) = atr_val.try_into() else {
                            warn!("Rune value too big for a u8: {atr_val}");
                            continue;
                        };
                        rune = Some(Rune { typ, value });
                    }
                }
            }
        }
        let model_id: u16 =
            data.cimget(1, "item model id", |x| ((x & 0xFFFF) % 1000))?;

        let color = match model_id {
            ..=49 if typ != ItemType::Talisman => data
                .get(2..=9)
                .map(|a| a.iter().sum::<i64>())
                .map(|a| (a % 5) + 1)
                .and_then(|a| a.try_into().ok())
                .unwrap_or(1),
            _ => 1,
        };

        let item = Item {
            typ,
            model_id,
            price: data.csiget(10, "item price", u32::MAX)?,
            mushroom_price: data.csiget(11, "mushroom price", u32::MAX)?,
            rune,
            type_specific_val: data.csiget(2, "effect value", 0)?,
            gem_slot,
            enchantment,
            class,
            attributes,
            color,
        };
        Ok(Some(item))
    }
}

#[derive(
    Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, EnumIter, Hash, Enum,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A enchantment, that gives a bonus to an aspect, if the item
pub enum Enchantment {
    /// Increased crit damage
    SwordOfVengeance = 11,
    /// Finds more mushrooms
    MariosBeard = 31,
    /// Shortens travel time
    ManyFeetBoots = 41,
    /// Increased reaction score in combat
    ShadowOfTheCowboy = 51,
    /// Extra XP on expeditions
    AdventurersArchaeologicalAura = 61,
    /// Allows an extra beer
    ThirstyWanderer = 71,
    /// Find items at paths edge (expeditions) more often
    UnholyAcquisitiveness = 81,
    /// Find extra gold on expeditions
    TheGraveRobbersPrayer = 91,
    /// Increase the chance of loot against other players
    RobberBaronRitual = 101,
}

impl Enchantment {
    #[must_use]
    pub fn equipment_slot(&self) -> EquipmentSlot {
        match self {
            Enchantment::SwordOfVengeance => EquipmentSlot::Weapon,
            Enchantment::MariosBeard => EquipmentSlot::BreastPlate,
            Enchantment::ManyFeetBoots => EquipmentSlot::FootWear,
            Enchantment::ShadowOfTheCowboy => EquipmentSlot::Gloves,
            Enchantment::AdventurersArchaeologicalAura => EquipmentSlot::Hat,
            Enchantment::ThirstyWanderer => EquipmentSlot::Belt,
            Enchantment::UnholyAcquisitiveness => EquipmentSlot::Amulet,
            Enchantment::TheGraveRobbersPrayer => EquipmentSlot::Ring,
            Enchantment::RobberBaronRitual => EquipmentSlot::Talisman,
        }
    }

    #[must_use]
    pub fn enchant_id(self) -> u32 {
        ((self as u32) / 10) * 10
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A rune, which has both a type and a strength
pub struct Rune {
    /// The type of tune this is
    pub typ: RuneType,
    /// The "strength" of this rune. So a value like 50 here and a typ of
    /// `FireResistance` would mean 50% fire resistance
    pub value: u8,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, EnumIter, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The effect of a rune
pub enum RuneType {
    QuestGold = 31,
    EpicChance,
    ItemQuality,
    QuestXP,
    ExtraHitPoints,
    FireResistance,
    ColdResistence,
    LightningResistance,
    TotalResistence,
    FireDamage,
    ColdDamage,
    LightningDamage,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A gem slot for an item
pub enum GemSlot {
    /// This gemslot has been filled and can only be emptied by the blacksmith
    Filled(Gem),
    /// A gem can be inserted into this item
    Empty,
}

impl GemSlot {
    pub(crate) fn parse(slot_val: i64, gem_pwr: i64) -> Option<GemSlot> {
        match slot_val {
            0 => return None,
            1 => return Some(GemSlot::Empty),
            _ => {}
        }

        let Ok(value) = gem_pwr.try_into() else {
            warn!("Invalid gem power {gem_pwr}");
            return None;
        };

        match GemType::parse(slot_val, value) {
            Some(typ) => Some(GemSlot::Filled(Gem { typ, value })),
            None => Some(GemSlot::Empty),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A potion. This is not just itemtype to make active potions easier
pub struct Potion {
    /// The rtype of potion
    pub typ: PotionType,
    /// The size of potion
    pub size: PotionSize,
    /// The time at which this potion expires. If this is none, the time is not
    /// known. This can happen for other players
    pub expires: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// Identifies a specific item and contains all values related to the specific
/// type. The only thing missing is armor, which can be found as a method on
/// `Item`
pub enum ItemType {
    Hat,
    BreastPlate,
    Gloves,
    FootWear,
    Weapon {
        min_dmg: u32,
        max_dmg: u32,
    },
    Amulet,
    Belt,
    Ring,
    Talisman,
    Shield {
        block_chance: u32,
    },
    Shard {
        piece: u32,
    },
    Potion(Potion),
    Scrapbook,
    DungeonKey {
        id: u32,
        shadow_key: bool,
    },
    Gem(Gem),
    PetItem {
        typ: PetItem,
    },
    QuickSandGlass,
    HeartOfDarkness,
    WheelOfFortune,
    Mannequin,
    Resource {
        amount: u32,
        typ: ResourceType,
    },
    ToiletKey,
    Gral,
    EpicItemBag,
    /// If there is a new item added to the game, this will be the placeholder
    /// to make sure you never think a place is empty somewhere, if it is not
    Unknown(u8),
}

impl ItemType {
    /// Checks if this item type is a weapon.
    #[must_use]
    pub const fn is_weapon(self) -> bool {
        matches!(self, ItemType::Weapon { .. })
    }

    /// Checks if this item type is a shield.
    #[must_use]
    pub const fn is_shield(self) -> bool {
        matches!(self, ItemType::Shield { .. })
    }

    /// Checks if this type can only be worn by only a particular class
    #[must_use]
    pub fn is_class_item(&self) -> bool {
        matches!(
            self,
            ItemType::Hat
                | ItemType::Belt
                | ItemType::Gloves
                | ItemType::FootWear
                | ItemType::Shield { .. }
                | ItemType::Weapon { .. }
                | ItemType::BreastPlate
        )
    }

    /// Checks, if this item type is unique. Technically they are not always
    /// unique, as the scrapbook/keys can be sold, but it should be clear
    /// what this is
    #[must_use]
    pub fn is_unique(&self) -> bool {
        matches!(
            self,
            ItemType::Scrapbook
                | ItemType::HeartOfDarkness
                | ItemType::WheelOfFortune
                | ItemType::Mannequin
                | ItemType::ToiletKey
                | ItemType::Gral
                | ItemType::EpicItemBag
                | ItemType::DungeonKey { .. }
        )
    }

    /// The equipment slot, that this item type can be equipped to
    #[must_use]
    pub fn equipment_slot(&self) -> Option<EquipmentSlot> {
        Some(match self {
            ItemType::Hat => EquipmentSlot::Hat,
            ItemType::BreastPlate => EquipmentSlot::BreastPlate,
            ItemType::Gloves => EquipmentSlot::Gloves,
            ItemType::FootWear => EquipmentSlot::FootWear,
            ItemType::Weapon { .. } => EquipmentSlot::Weapon,
            ItemType::Amulet => EquipmentSlot::Amulet,
            ItemType::Belt => EquipmentSlot::Belt,
            ItemType::Ring => EquipmentSlot::Ring,
            ItemType::Talisman => EquipmentSlot::Talisman,
            ItemType::Shield { .. } => EquipmentSlot::Shield,
            _ => return None,
        })
    }

    /// Checks, if this item type can be enchanted
    #[must_use]
    pub fn is_enchantable(&self) -> bool {
        self.equipment_slot()
            .is_some_and(|e| e.enchantment().is_some())
    }

    pub(crate) fn parse_active_potions(
        data: &[i64],
        server_time: ServerTime,
    ) -> [Option<Potion>; 3] {
        if data.len() < 6 {
            return Default::default();
        }
        #[allow(clippy::indexing_slicing)]
        core::array::from_fn(move |i| {
            Some(Potion {
                typ: PotionType::parse(data[i])?,
                size: PotionSize::parse(data[i])?,
                expires: server_time
                    .convert_to_local(data[3 + i], "potion exp"),
            })
        })
    }

    pub(crate) fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Option<Self>, SFError> {
        let raw_typ: u8 = data.csimget(0, "item type", 255, |a| a & 0xFF)?;
        let unknown_item = |name: &'static str| {
            warn!("Could no parse item of type: {raw_typ}. {name} is faulty");
            Ok(Some(ItemType::Unknown(raw_typ)))
        };
        let sub_ident = data.cget(1, "item sub type")?;

        Ok(Some(match raw_typ {
            0 => return Ok(None),
            1 => ItemType::Weapon {
                min_dmg: data.csiget(2, "weapon min dmg", 0)?,
                max_dmg: data.csiget(3, "weapon min dmg", 0)?,
            },
            2 => ItemType::Shield {
                block_chance: data.csiget(2, "shield block chance", 0)?,
            },
            3 => ItemType::BreastPlate,
            4 => ItemType::FootWear,
            5 => ItemType::Gloves,
            6 => ItemType::Hat,
            7 => ItemType::Belt,
            8 => ItemType::Amulet,
            9 => ItemType::Ring,
            10 => ItemType::Talisman,
            11 => {
                let id = sub_ident & 0xFFFF;
                let Ok(id) = id.try_into() else {
                    return unknown_item("unique sub ident");
                };
                match id {
                    1..=11 | 17 | 19 | 22 | 69 | 70 => ItemType::DungeonKey {
                        id,
                        shadow_key: false,
                    },
                    20 => ItemType::ToiletKey,
                    51..=64 | 67..=68 => ItemType::DungeonKey {
                        id,
                        shadow_key: true,
                    },
                    10000 => ItemType::EpicItemBag,
                    piece => ItemType::Shard { piece },
                }
            }
            12 => {
                let id = sub_ident & 0xFF;

                if id > 16 {
                    let Some(typ) = FromPrimitive::from_i64(id) else {
                        return unknown_item("resource type");
                    };
                    ItemType::Resource {
                        amount: data.csiget(7, "resource amount", 0)?,
                        typ,
                    }
                } else {
                    let Some(typ) = PotionType::parse(id) else {
                        return unknown_item("potion type");
                    };
                    let Some(size) = PotionSize::parse(id) else {
                        return unknown_item("potion size");
                    };
                    ItemType::Potion(Potion {
                        typ,
                        size,
                        expires: data.cstget(
                            4,
                            "potion expires",
                            server_time,
                        )?,
                    })
                }
            }
            13 => ItemType::Scrapbook,
            15 => {
                let pwr = data.csimget(11, "gem pwr", 0, |a| a >> 16)?;
                let Some(typ) = GemType::parse(sub_ident, pwr) else {
                    return unknown_item("gem type");
                };
                let gem = Gem { typ, value: pwr };
                ItemType::Gem(gem)
            }
            16 => {
                let Some(typ) = PetItem::parse(sub_ident & 0xFFFF) else {
                    return unknown_item("pet item");
                };
                ItemType::PetItem { typ }
            }
            17 if (sub_ident & 0xFFFF) == 4 => ItemType::Gral,
            17 => ItemType::QuickSandGlass,
            18 => ItemType::HeartOfDarkness,
            19 => ItemType::WheelOfFortune,
            20 => ItemType::Mannequin,
            _ => {
                return unknown_item("main ident");
            }
        }))
    }

    /// The id, that the server has associated with this item. I honestly forgot
    /// why I have this function public
    #[must_use]
    pub fn raw_id(&self) -> u8 {
        match self {
            ItemType::Weapon { .. } => 1,
            ItemType::Shield { .. } => 2,
            ItemType::BreastPlate => 3,
            ItemType::FootWear => 4,
            ItemType::Gloves => 5,
            ItemType::Hat => 6,
            ItemType::Belt => 7,
            ItemType::Amulet => 8,
            ItemType::Ring => 9,
            ItemType::Talisman => 10,
            ItemType::Shard { .. }
            | ItemType::DungeonKey { .. }
            | ItemType::ToiletKey
            | ItemType::EpicItemBag => 11,
            ItemType::Potion { .. } | ItemType::Resource { .. } => 12,
            ItemType::Scrapbook => 13,
            ItemType::Gem(_) => 15,
            ItemType::PetItem { .. } => 16,
            ItemType::QuickSandGlass | ItemType::Gral => 17,
            ItemType::HeartOfDarkness => 18,
            ItemType::WheelOfFortune => 19,
            ItemType::Mannequin => 20,
            ItemType::Unknown(u) => *u,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The effect, that the potion is going to have
pub enum PotionType {
    Strength,
    Dexterity,
    Intelligence,
    Constitution,
    Luck,
    EternalLife,
}

impl From<AttributeType> for PotionType {
    fn from(value: AttributeType) -> Self {
        match value {
            AttributeType::Strength => PotionType::Strength,
            AttributeType::Dexterity => PotionType::Dexterity,
            AttributeType::Intelligence => PotionType::Intelligence,
            AttributeType::Constitution => PotionType::Constitution,
            AttributeType::Luck => PotionType::Luck,
        }
    }
}

impl PotionType {
    pub(crate) fn parse(id: i64) -> Option<PotionType> {
        if id == 0 {
            return None;
        }
        if id == 16 {
            return Some(PotionType::EternalLife);
        }
        Some(match id % 5 {
            0 => PotionType::Luck,
            1 => PotionType::Strength,
            2 => PotionType::Dexterity,
            3 => PotionType::Intelligence,
            _ => PotionType::Constitution,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The size and with that, the strength, that this potion has
pub enum PotionSize {
    Small,
    Medium,
    Large,
}

impl PotionSize {
    #[must_use]
    pub fn effect(&self) -> f64 {
        match self {
            PotionSize::Small => 0.1,
            PotionSize::Medium => 0.15,
            PotionSize::Large => 0.25,
        }
    }

    pub(crate) fn parse(id: i64) -> Option<Self> {
        Some(match id {
            1..=5 => PotionSize::Small,
            6..=10 => PotionSize::Medium,
            11..=16 => PotionSize::Large,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// Differentiates resource items
pub enum ResourceType {
    Wood = 17,
    Stone,
    Souls,
    Arcane,
    Metal,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A gem, that is either socketed in an item, or in the inventory
pub struct Gem {
    /// The type of gem
    pub typ: GemType,
    /// The strength of this gem
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The type the gam has
pub enum GemType {
    Strength,
    Dexterity,
    Intelligence,
    Constitution,
    Luck,
    All,
    Legendary,
}

impl GemType {
    pub(crate) fn parse(id: i64, debug_value: u32) -> Option<GemType> {
        Some(match id {
            0 | 1 => return None,
            10..=40 => match id % 10 {
                0 => GemType::Strength,
                1 => GemType::Dexterity,
                2 => GemType::Intelligence,
                3 => GemType::Constitution,
                4 => GemType::Luck,
                5 => GemType::All,
                // Just put this here because it makes sense. I only ever
                // see 4 for these
                6 => GemType::Legendary,
                _ => {
                    return None;
                }
            },
            _ => {
                warn!("Unknown gem: {id} - {debug_value}");
                return None;
            }
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Enum, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// Denotes the place, where an item is equipped
pub enum EquipmentSlot {
    Hat = 1,
    BreastPlate,
    Gloves,
    FootWear,
    Amulet,
    Belt,
    Ring,
    Talisman,
    Weapon,
    Shield,
}

impl EquipmentSlot {
    /// The value the game internally uses for these slots. No idea, why this is
    /// pub
    #[must_use]
    pub fn raw_id(&self) -> u8 {
        match self {
            EquipmentSlot::Weapon => 1,
            EquipmentSlot::Shield => 2,
            EquipmentSlot::BreastPlate => 3,
            EquipmentSlot::FootWear => 4,
            EquipmentSlot::Gloves => 5,
            EquipmentSlot::Hat => 6,
            EquipmentSlot::Belt => 7,
            EquipmentSlot::Amulet => 8,
            EquipmentSlot::Ring => 9,
            EquipmentSlot::Talisman => 10,
        }
    }

    /// Returns the corresponding enchantment for this equipment slot, if it
    /// can be enchanted
    #[must_use]
    pub const fn enchantment(&self) -> Option<Enchantment> {
        match self {
            EquipmentSlot::Hat => {
                Some(Enchantment::AdventurersArchaeologicalAura)
            }
            EquipmentSlot::BreastPlate => Some(Enchantment::MariosBeard),
            EquipmentSlot::Gloves => Some(Enchantment::ShadowOfTheCowboy),
            EquipmentSlot::FootWear => Some(Enchantment::ManyFeetBoots),
            EquipmentSlot::Amulet => Some(Enchantment::UnholyAcquisitiveness),
            EquipmentSlot::Belt => Some(Enchantment::ThirstyWanderer),
            EquipmentSlot::Ring => Some(Enchantment::TheGraveRobbersPrayer),
            EquipmentSlot::Talisman => Some(Enchantment::RobberBaronRitual),
            EquipmentSlot::Weapon => Some(Enchantment::SwordOfVengeance),
            EquipmentSlot::Shield => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// An item usable for pets
pub enum PetItem {
    Egg(HabitatType),
    SpecialEgg(HabitatType),
    GoldenEgg,
    Nest,
    Fruit(HabitatType),
}

impl PetItem {
    pub(crate) fn parse(val: i64) -> Option<Self> {
        Some(match val {
            1..=5 => PetItem::Egg(HabitatType::from_typ_id(val)?),
            11..=15 => PetItem::SpecialEgg(HabitatType::from_typ_id(val - 10)?),
            21 => PetItem::GoldenEgg,
            22 => PetItem::Nest,
            31..=35 => PetItem::Fruit(HabitatType::from_typ_id(val - 30)?),
            _ => return None,
        })
    }
}
