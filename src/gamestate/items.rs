use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use log::{error, warn};
use num::FromPrimitive;
use num_derive::FromPrimitive;

use super::{
    unlockables::{EquipmentIdent, PetClass},
    ArrSkip, Class, SFError, ServerTime,
};
use crate::{
    command::AttributeType,
    misc::{soft_into, warning_parse, warning_try_into},
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
    pub(crate) fn update_fortress_chest(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) {
        self.fortress_chest = None;
        if data.is_empty() {
            return;
        }
        if data.len() % 12 != 0 {
            error!("Wrong fortess chest response size:  {data:?}");
        }
        self.fortress_chest = Some(
            data.chunks_exact(12)
                .map(|a| Item::parse(a, server_time))
                .collect(),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// All the parts of `ItemPosition`, that are owned by the player
pub enum InventoryType {
    Equipment = 1,
    MainInventory = 2,
    ExtendedInventory = 5,
}

impl InventoryType {
    /// `InventoryType` is a subset of `ItemPosition`. This is a convenient
    /// function to convert between them
    #[must_use]
    pub fn item_position(&self) -> ItemPosition {
        match self {
            InventoryType::Equipment => ItemPosition::Equipment,
            InventoryType::MainInventory => ItemPosition::MainInventory,
            InventoryType::ExtendedInventory => ItemPosition::FortressChest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// All positions, that items can be dragged to excluding companions
pub enum ItemPosition {
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
/// All the equipment a player is weraing
pub struct Equipment(pub EnumMap<EquipmentSlot, Option<Item>>);

impl Equipment {
    /// Expects the input `data` to have items directly at data[0]
    pub(crate) fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Equipment, SFError> {
        let mut res = Equipment::default();
        for (idx, map_val) in res.0.as_mut_slice().iter_mut().enumerate() {
            *map_val =
                Item::parse(data.skip(idx * 12, "equipment")?, server_time);
        }
        Ok(res)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Item {
    /// The type of this  item. May contain further type specific values
    pub typ: ItemType,
    /// Either the price to buy, or sell
    pub price: u32,
    /// The price you would have to pay for this item. Note that this value is
    /// junk for other players and potentially in other cases, where you should
    /// not be able to se a price
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
    /// The stats this item gives, when equiped
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
    pub fn is_unique(&self) -> bool {
        self.typ.is_unique()
    }

    pub fn is_epic(&self) -> bool {
        self.model_id >= 50
    }

    pub fn is_legendary(&self) -> bool {
        self.model_id >= 90
    }

    /// The armor rating of this item. This is just the `effect_val`, if any
    pub fn armor(&self) -> u32 {
        use ItemType::*;
        match self.typ {
            Hat | BreastPlate | Gloves | FootWear | Amulet | Belt | Ring
            | Talisman => self.type_specific_val,
            _ => 0,
        }
    }

    /// Parses an item, that starts at the start of the given data
    pub(crate) fn parse(data: &[i64], server_time: ServerTime) -> Option<Self> {
        if data.len() < 12 {
            warn!("Invalid item length");
            return None;
        }
        let typ = ItemType::parse(data, server_time)?;

        let enchantment = FromPrimitive::from_i64(data[0] >> 24);

        let gem_slot = GemSlot::parse(data[0] >> 16 & 0xF, data[11] >> 16);

        let class = match typ {
            ItemType::Talisman
            | ItemType::Ring
            | ItemType::Amulet
            | ItemType::Shard { .. } => None,
            _ => FromPrimitive::from_i64((data[1] & 0xFFFF) / 1000),
        };
        let mut rune = None;
        let mut attributes: EnumMap<AttributeType, u32> = EnumMap::default();
        if typ.equipment_slot().is_some() {
            for i in 0..3 {
                use AttributeType::*;
                let atr_typ = data[i + 4];
                let Ok(atr_typ) = atr_typ.try_into() else {
                    warn!("Invalid attribute typ: {atr_typ}, {typ:?}");
                    continue;
                };
                let atr_val = data[i + 7];
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
                        attributes[atr_typ] = atr_val;
                    }
                    6 => {
                        attributes.as_mut_array().fill(atr_val);
                    }
                    21 => {
                        for atr in [Strength, Constitution, Luck] {
                            attributes[atr] = atr_val
                        }
                    }
                    22 => {
                        for atr in [Dexterity, Constitution, Luck] {
                            attributes[atr] = atr_val
                        }
                    }
                    23 => {
                        for atr in [Intelligence, Constitution, Luck] {
                            attributes[atr] = atr_val
                        }
                    }
                    rune_typ => {
                        let Some(typ) = FromPrimitive::from_usize(rune_typ)
                        else {
                            warn!(
                                "Unhandled item val: {atr_typ} -> {atr_val} \
                                 for {class:?} {typ:?} price: {}",
                                data[10] / 100
                            );
                            continue;
                        };
                        let Ok(value) = atr_val.try_into() else {
                            warn!("Rune value too big for a u8: {atr_val}");
                            continue;
                        };
                        rune = Some(Rune { typ, value })
                    }
                }
            }
        }
        let model_id = ((data[1] & 0xFFFF) % 1000) as u16;

        let color = match model_id {
            ..=49 if typ != ItemType::Talisman => {
                ((data[2..=9].iter().sum::<i64>() % 5) + 1) as u8
            }
            _ => 1,
        };

        let item = Item {
            typ,
            model_id,
            price: soft_into(data[10], "item price", u32::MAX),
            mushroom_price: soft_into(data[11], "mushroom price", u32::MAX),
            rune,
            type_specific_val: soft_into(data[2], "effect value", 0),
            gem_slot,
            enchantment,
            class,
            attributes,
            color,
        };
        Some(item)
    }
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Enchantment {
    SwordOfVengeance = 11,
    MariosBeard = 32,
    ManyFeetBoots = 41,
    ShadowOfTheCowboy = 51,
    AdventurersArchaeologicalAura = 61,
    ThirstyWanderer = 71,
    UnholyAcquisitiveness = 81,
    TheGraveRobbersPrayer = 91,
    RobberBaronRitual = 101,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rune {
    pub typ: RuneType,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
pub enum GemSlot {
    Filled(Gem),
    Empty,
}

impl GemSlot {
    pub(crate) fn parse(slot_val: i64, gem_pwr: i64) -> Option<GemSlot> {
        if slot_val == 0 {
            return None;
        }
        let Ok(value) = gem_pwr.try_into() else {
            warn!("Invalid gem power {gem_pwr}");
            return None;
        };
        match GemType::parse(slot_val) {
            Some(typ) => Some(GemSlot::Filled(Gem { typ, value })),
            None => Some(GemSlot::Empty),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PotionData {
    pub typ: PotionType,
    pub size: PotionSize,
    pub expires: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    Potion(PotionData),
    Scrapbook,
    DungeonKey {
        id: u32,
        shadow_key: bool,
    },
    Gem,
    PetItem {
        typ: PetItemType,
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

    /// The equipment slot, that this item type can be equiped to
    pub fn equipment_slot(&self) -> Option<EquipmentSlot> {
        use EquipmentSlot::*;
        Some(match self {
            ItemType::Hat => Hat,
            ItemType::BreastPlate => BreastPlate,
            ItemType::Gloves => Gloves,
            ItemType::FootWear => FootWear,
            ItemType::Weapon { .. } => Weapon,
            ItemType::Amulet => Amulet,
            ItemType::Belt => Belt,
            ItemType::Ring => Ring,
            ItemType::Talisman => Talisman,
            ItemType::Shield { .. } => Shield,
            _ => return None,
        })
    }

    pub(crate) fn parse_active_potions(
        data: &[i64],
        server_time: ServerTime,
    ) -> [Option<PotionData>; 3] {
        [0, 1, 2].map(move |i| {
            Some(PotionData {
                typ: PotionType::parse(data[i])?,
                size: PotionSize::parse(data[i])?,
                expires: server_time
                    .convert_to_local(data[3 + i], "potion exp"),
            })
        })
    }

    pub(crate) fn parse(data: &[i64], server_time: ServerTime) -> Option<Self> {
        use ItemType::*;
        Some(match data[0] & 0xFF {
            0 => return None,
            1 => Weapon {
                min_dmg: soft_into(data[2], "weapon min dmg", 0),
                max_dmg: soft_into(data[3], "weapon min dmg", 0),
            },
            2 => Shield {
                block_chance: soft_into(data[2], "shield block chance", 0),
            },
            3 => BreastPlate,
            4 => FootWear,
            5 => Gloves,
            6 => Hat,
            7 => Belt,
            8 => Amulet,
            9 => Ring,
            10 => Talisman,
            11 => {
                let unique_id =
                    warning_try_into(data[1] & 0xFFFF, "unique id")?;
                match unique_id {
                    1..=11 | 17 | 19 | 22 | 69 | 70 => DungeonKey {
                        id: unique_id,
                        shadow_key: false,
                    },
                    20 => ToiletKey,
                    51..=64 | 67..=68 => DungeonKey {
                        id: unique_id,
                        shadow_key: true,
                    },
                    10000 => EpicItemBag,
                    _ => Shard { piece: unique_id },
                }
            }
            12 => {
                if data[1] > 16 {
                    Resource {
                        amount: soft_into(data[7], "resource amount", 0),
                        typ: warning_parse(
                            data[1],
                            "resource type",
                            FromPrimitive::from_i64,
                        )?,
                    }
                } else {
                    Potion(PotionData {
                        typ: warning_parse(
                            data[1],
                            "potion type",
                            PotionType::parse,
                        )?,
                        size: warning_parse(
                            data[1],
                            "potion size",
                            PotionSize::parse,
                        )?,
                        expires: server_time
                            .convert_to_local(data[4], "potion expires"),
                    })
                }
            }
            13 => Scrapbook,
            15 => Gem,
            16 => PetItem {
                typ: warning_parse(
                    data[1] & 0xFFFF,
                    "pet item typ",
                    PetItemType::parse,
                )?,
            },
            17 => {
                if (data[1] & 0xFFFF) == 4 {
                    Gral
                } else {
                    QuickSandGlass
                }
            }
            18 => HeartOfDarkness,
            19 => WheelOfFortune,
            20 => Mannequin,
            x => {
                error!("Unknown item typ id {x}");
                Unknown(x.try_into().unwrap_or(0))
            }
        })
    }

    pub fn raw_id(&self) -> u8 {
        use ItemType::*;
        match self {
            Weapon { .. } => 1,
            Shield { .. } => 2,
            BreastPlate => 3,
            FootWear => 4,
            Gloves => 5,
            Hat => 6,
            Belt => 7,
            Amulet => 8,
            Ring => 9,
            Talisman => 10,
            Shard { .. } | DungeonKey { .. } | ToiletKey | EpicItemBag => 11,
            Potion { .. } | Resource { .. } => 12,
            Scrapbook => 13,
            Gem => 15,
            PetItem { .. } => 16,
            QuickSandGlass | Gral => 17,
            HeartOfDarkness => 18,
            WheelOfFortune => 19,
            Mannequin => 20,
            Unknown(u) => *u,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PotionType {
    Strength,
    Dexterity,
    Intelligence,
    Constitution,
    Luck,
    EternalLife,
}

impl PotionType {
    pub(crate) fn parse(id: i64) -> Option<PotionType> {
        use PotionType::*;
        if id == 0 {
            return None;
        }
        if id == 16 {
            return Some(EternalLife);
        }
        Some(match id % 5 {
            0 => Luck,
            1 => Strength,
            2 => Dexterity,
            3 => Intelligence,
            _ => Constitution,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PotionSize {
    Small,
    Medium,
    Large,
}

impl PotionSize {
    pub(crate) fn parse(id: i64) -> Option<Self> {
        if id == 16 {
            return Some(Large);
        }
        use PotionSize::*;
        Some(match id / 6 {
            0 => Small,
            1 => Medium,
            2 => Large,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResourceType {
    Wood = 17,
    Stone,
    Souls,
    Arcane,
    Metal,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gem {
    pub typ: GemType,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub(crate) fn parse(id: i64) -> Option<GemType> {
        if id == 4 {
            return Some(GemType::Legendary);
        }

        if !(10..=40).contains(&id) {
            return None;
        }

        // NOTE: id / 10 should be the shape
        use GemType::*;
        Some(match id % 10 {
            0 => Strength,
            1 => Dexterity,
            2 => Intelligence,
            3 => Constitution,
            4 => Luck,
            5 => All,
            // Just put this here because it makes sense. I only ever see 4 for
            // these
            6 => Legendary,
            _ => {
                return None;
            }
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub fn raw_id(&self) -> u8 {
        use EquipmentSlot::*;
        match self {
            Weapon => 1,
            Shield => 2,
            BreastPlate => 3,
            FootWear => 4,
            Gloves => 5,
            Hat => 6,
            Belt => 7,
            Amulet => 8,
            Ring => 9,
            Talisman => 10,
        }
    }

    // This is just itemtyp * 10, but whatever
    pub(crate) fn witch_id(&self) -> u32 {
        match self {
            // Wrong, as there are no shield enchantments, but better than
            // panic/erroring I think
            EquipmentSlot::Shield => 10,

            EquipmentSlot::Weapon => 10,
            EquipmentSlot::BreastPlate => 30,
            EquipmentSlot::FootWear => 40,
            EquipmentSlot::Gloves => 50,
            EquipmentSlot::Hat => 60,
            EquipmentSlot::Belt => 70,
            EquipmentSlot::Amulet => 80,
            EquipmentSlot::Ring => 90,
            EquipmentSlot::Talisman => 100,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PetItemType {
    Egg(PetClass),
    SpecialEgg(PetClass),
    GoldenEgg,
    Nest,
    Fruit(PetClass),
}

impl PetItemType {
    pub(crate) fn parse(val: i64) -> Option<Self> {
        use PetItemType::*;
        Some(match val {
            1..=5 => Egg(PetClass::from_typ_id(val)?),
            11..=15 => SpecialEgg(PetClass::from_typ_id(val - 10)?),
            21 => GoldenEgg,
            22 => Nest,
            31..=35 => Fruit(PetClass::from_typ_id(val - 30)?),
            _ => return None,
        })
    }
}
