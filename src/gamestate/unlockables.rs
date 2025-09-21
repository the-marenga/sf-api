use std::num::NonZeroU8;

use chrono::{DateTime, Local};
use enum_map::Enum;
use log::error;
use num_derive::FromPrimitive;
use strum::EnumIter;

use super::*;
use crate::{PlayerId, gamestate::items::*, misc::*};

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the Hellevator event on the server. If it is active, you
/// can get more detailed info via `active()`
pub struct HellevatorEvent {
    /// The time the hellevator event was enabled at
    pub start: Option<DateTime<Local>>,
    /// The time the hellevator event will be disabled at
    pub end: Option<DateTime<Local>>,
    /// The time at which you will no longer be able to collect things for the
    /// hellevator
    pub collect_time_end: Option<DateTime<Local>>,
    /// Contains the hellevator. This can be some(x), even if the event is not
    /// going, so you should use the `active()` functions to get this
    pub(crate) active: Option<Hellevator>,
}

#[derive(Debug)]
pub enum HellevatorStatus<'a> {
    /// The event is ongoing, but you have to send a `HellevatorEnter` command
    /// to start using it
    NotEntered,
    /// The event is currently not available
    NotAvailable,
    /// The event has ended, but you can still claim the final reward
    RewardClaimable,
    /// A reference to the
    Active(&'a Hellevator),
}

impl HellevatorEvent {
    /// Checks if the event has started and not yet ended compared to the
    /// current time
    #[must_use]
    pub fn is_event_ongoing(&self) -> bool {
        let now = Local::now();
        matches!((self.start, self.end), (Some(start), Some(end)) if end > now && start < now)
    }

    /// If the Hellevator event is active, this returns a reference to the
    /// Information about it. Note that you still need to check the level >= 10
    /// requirement yourself
    #[must_use]
    pub fn status(&self) -> HellevatorStatus<'_> {
        match self.active.as_ref() {
            None => HellevatorStatus::NotAvailable,
            Some(h) if !self.is_event_ongoing() => {
                if let Some(cend) = self.collect_time_end
                    && !h.has_final_reward
                    && Local::now() < cend
                {
                    return HellevatorStatus::RewardClaimable;
                }
                HellevatorStatus::NotAvailable
            }
            Some(h) if h.current_floor == 0 => HellevatorStatus::NotEntered,
            Some(h) => HellevatorStatus::Active(h),
        }
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hellevator {
    pub key_cards: u32,
    pub current_floor: u32,
    pub points: u32,
    pub has_final_reward: bool,

    pub guild_points_today: u32,
    pub guild_rank: u32,
    pub guild_raid_floors: Vec<HellevatorRaidFloor>,

    pub guild_raid_signup_start: DateTime<Local>,
    pub guild_raid_start: DateTime<Local>,
    pub monster_rewards: Vec<HellevatorMonsterReward>,

    pub own_best_floor: u32,
    pub shop_items: [HellevatorShopTreat; 3],

    pub current_treat: Option<HellevatorShopTreat>,

    pub next_card_generated: Option<DateTime<Local>>,
    pub next_reset: Option<DateTime<Local>>,
    pub start_contrib_date: Option<DateTime<Local>>,

    pub rewards_yesterday: Option<HellevatorDailyReward>,
    pub rewards_today: Option<HellevatorDailyReward>,
    pub rewards_next: Option<HellevatorDailyReward>,

    pub daily_treat_bonus: Option<HellevatorTreatBonus>,

    pub current_monster: Option<HellevatorMonster>,

    pub earned_today: u32,
    pub earned_yesterday: u32,

    pub(crate) brackets: Vec<u32>,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorTreatBonus {
    pub typ: HellevatorTreatBonusType,
    pub amount: u32,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorMonster {
    pub id: i64,
    pub level: u32,
    pub typ: HellevatorMonsterElement,
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HellevatorMonsterElement {
    Fire = 1,
    Cold = 2,
    Lightning = 3,
    #[default]
    Unknown = 240,
}

impl HellevatorMonster {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        Ok(HellevatorMonster {
            id: data.cget(0, "h monster id")?,
            level: data.csiget(1, "h monster level", 0)?,
            typ: data.cfpget(2, "h monster typ", |a| a)?.unwrap_or_default(),
        })
    }
}

impl HellevatorTreatBonus {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        Ok(HellevatorTreatBonus {
            typ: data
                .cfpget(0, "hellevator treat bonus", |a| a)?
                .unwrap_or_default(),
            amount: data.csiget(1, "hellevator treat bonus a", 0)?,
        })
    }
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HellevatorTreatBonusType {
    ExtraDamage = 14,
    #[default]
    Unknown = 240,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorMonsterReward {
    pub typ: HellevatorMonsterRewardTyp,
    pub amount: u64,
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HellevatorMonsterRewardTyp {
    Points,
    Tickets,
    Mushrooms,
    Silver,
    LuckyCoin,
    Wood,
    Stone,
    Arcane,
    Metal,
    Souls,
    Fruit(HabitatType),

    #[default]
    Unknown,
}

impl HellevatorMonsterRewardTyp {
    pub(crate) fn parse(data: i64) -> HellevatorMonsterRewardTyp {
        match data {
            1 => HellevatorMonsterRewardTyp::Points,
            2 => HellevatorMonsterRewardTyp::Tickets,
            3 => HellevatorMonsterRewardTyp::Mushrooms,
            4 => HellevatorMonsterRewardTyp::Silver,
            5 => HellevatorMonsterRewardTyp::LuckyCoin,
            6 => HellevatorMonsterRewardTyp::Wood,
            7 => HellevatorMonsterRewardTyp::Stone,
            8 => HellevatorMonsterRewardTyp::Arcane,
            9 => HellevatorMonsterRewardTyp::Metal,
            10 => HellevatorMonsterRewardTyp::Souls,
            11 => HellevatorMonsterRewardTyp::Fruit(HabitatType::Shadow),
            12 => HellevatorMonsterRewardTyp::Fruit(HabitatType::Light),
            13 => HellevatorMonsterRewardTyp::Fruit(HabitatType::Earth),
            14 => HellevatorMonsterRewardTyp::Fruit(HabitatType::Fire),
            15 => HellevatorMonsterRewardTyp::Fruit(HabitatType::Water),

            _ => HellevatorMonsterRewardTyp::Unknown,
        }
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorRaidFloor {
    pub(crate) today: i64,
    pub(crate) yesterday: i64,

    pub point_reward: u32,
    pub silver_reward: u64,

    pub today_assigned: Vec<String>,
    pub yesterday_assigned: Vec<String>,
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash, FromPrimitive)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HellevatorTreatType {
    ChocolateChilliPepper = 1,
    PeppermintChocolate = 2,
    Electroshock = 3,
    ChillIceCream = 4,
    CracklingChewingGum = 5,
    PeppermintChewingGum = 6,
    BeerBiscuit = 7,
    GingerBreadHeart = 8,
    FortuneCookie = 9,
    CannedSpinach = 10,
    StoneBiscuit = 11,
    OrganicGranolaBar = 12,
    ChocolateGoldCoin = 13,
    #[default]
    Unknown = 230,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorShopTreat {
    pub is_special: bool,
    pub typ: HellevatorTreatType,
    pub price: u32,
    pub duration: u32,
    pub effect_strength: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorDailyReward {
    // TODO: What is the purpose of these fields?
    pub(crate) start_level: u16,
    pub(crate) end_level: u16,

    pub gold_chests: u16,
    pub silver: u64,

    pub fortress_chests: u16,
    pub wood: u64,
    pub stone: u64,

    pub blacksmith_chests: u16,
    pub arcane: u64,
    pub metal: u64,
}

impl HellevatorDailyReward {
    /// Returns `true` if the daily reward can be claimed
    #[must_use]
    pub fn claimable(&self) -> bool {
        self.gold_chests > 0
            || self.fortress_chests > 0
            || self.blacksmith_chests > 0
    }

    pub(crate) fn parse(data: &[i64]) -> Option<HellevatorDailyReward> {
        if data.len() < 10 {
            return None;
        }

        Some(HellevatorDailyReward {
            start_level: data.csiget(0, "start level", 0).unwrap_or(0),
            end_level: data.csiget(1, "end level", 0).unwrap_or(0),
            gold_chests: data.csiget(2, "gold chests", 0).unwrap_or(0),
            silver: data.csiget(5, "silver reward", 0).unwrap_or(0),
            fortress_chests: data.csiget(3, "ft chests", 0).unwrap_or(0),
            wood: data.csiget(6, "wood reward", 0).unwrap_or(0),
            stone: data.csiget(7, "stone reward", 0).unwrap_or(0),
            blacksmith_chests: data.csiget(4, "bs chests", 0).unwrap_or(0),
            arcane: data.csiget(8, "arcane reward", 0).unwrap_or(0),
            metal: data.csiget(9, "metal reward", 0).unwrap_or(0),
        })
    }
}

impl Hellevator {
    /// Converts the rank of a guild in the Hellevator into the reward bracket,
    /// that they would be in (1 to 25). If the rank would gain no rewards, none
    /// is returned here
    #[must_use]
    pub fn rank_to_rewards_rank(&self, rank: u32) -> Option<u32> {
        let mut rank_limit = 0;
        let mut bracket = 0;
        for bracket_len in &self.brackets {
            bracket += 1;
            rank_limit += *bracket_len;
            if rank <= rank_limit {
                return Some(bracket);
            }
        }
        None
    }

    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.key_cards = data.csiget(0, "h key cards", 0)?;
        self.next_card_generated = data.cstget(1, "next card", server_time)?;
        self.next_reset = data.cstget(2, "h next reset", server_time)?;
        self.current_floor = data.csiget(3, "h current floor", 0)?;
        self.points = data.csiget(4, "h points", 0)?;
        self.start_contrib_date =
            data.cstget(5, "start contrib", server_time)?;
        self.has_final_reward = data.cget(6, "hellevator final")? == 1;
        self.own_best_floor = data.csiget(7, "hellevator best rank", 0)?;

        for (pos, shop_item) in self.shop_items.iter_mut().enumerate() {
            let start = data.skip(8 + pos, "shop item start")?;
            shop_item.typ = start
                .cfpget(0, "hellevator shop treat", |a| a)?
                .unwrap_or_default();
            // FIXME: This is wrong
            shop_item.is_special =
                start.cget(3, "hellevator shop special")? > 0;
            shop_item.price =
                start.csiget(6, "hellevator shop price", u32::MAX)?;
            shop_item.duration =
                start.csiget(9, "hellevator shop duration", 0)?;
            shop_item.effect_strength =
                start.csiget(12, "hellevator effect str", 0)?;
        }

        let c_typ = data.cget(23, "current ctyp")?;
        self.current_treat = if c_typ > 0 {
            Some(HellevatorShopTreat {
                typ: FromPrimitive::from_i64(c_typ).unwrap_or_default(),
                is_special: data.cget(24, "current item special")? > 0,
                price: 0,
                duration: data.csiget(25, "current item remaining", 0)?,
                effect_strength: data.csiget(26, "current item effect", 0)?,
            })
        } else {
            None
        };

        self.earned_today = data.csiget(27, "points earned today", 0)?;
        // 28 => probably a "has acknowledged rank fall" msg
        self.earned_yesterday = data.csiget(29, "points earned yd", 0)?;
        // 30 => fallen to rank
        // 31 => ???
        Ok(())
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
    /// The price in silver to enchant an item
    pub enchantment_price: u64,
    /// Contains the ident to use when you want to apply the enchantment. If
    /// this is `None`, the enchantment has not been unlocked yet
    pub enchantments: EnumMap<Enchantment, Option<EnchantmentIdent>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The S&F server needs a character specific value for enchanting items. This
/// is that value
pub struct EnchantmentIdent(pub(crate) NonZeroU8);

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
            let current: i32 = data.ciget(1, "witch current")?;
            let target: i32 = data.ciget(2, "witch target")?;
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            if current < 0 || target <= 0 {
                self.progress = 100;
            } else {
                let current = f64::from(current);
                let target = f64::from(target);
                self.progress = ((current / target) * 100.0) as u32;
            }
        }

        let e_count: u8 = data.ciget(7, "enchant count")?;
        for i in 0..e_count {
            let iid = data.cget(9 + 3 * i as usize, "iid")? - 1;
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
                *self.enchantments.get_mut(key) = Some(EnchantmentIdent(val));
            }
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
    /// The rank this pet collection achieved in the hall of fame
    pub rank: u32,
    /// The honor this pet collection has gained
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
    /// All the different pets you can collect in this habitat
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
        for (element_idx, element) in [
            HabitatType::Shadow,
            HabitatType::Light,
            HabitatType::Earth,
            HabitatType::Fire,
            HabitatType::Water,
        ]
        .into_iter()
        .enumerate()
        {
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

        self.total_collected = data.csiget(103, "total pets", 0)?;
        self.opponent.id = data.csiget(231, "pet opponent id", 0)?;
        self.opponent.next_free_battle =
            data.cstget(232, "next free pet fight", server_time)?;
        self.rank = data.csiget(233, "pet rank", 0)?;
        self.honor = data.csiget(234, "pet honor", 0)?;

        self.opponent.pet_count = data.csiget(235, "pet enemy count", 0)?;
        self.opponent.level_total =
            data.csiget(236, "pet enemy lvl total", 0)?;
        self.opponent.reroll_date =
            data.cstget(237, "pet enemy reroll date", server_time)?;

        update_enum_map(&mut self.atr_bonus, data.skip(250, "pet atr boni")?);
        Ok(())
    }

    pub(crate) fn update_pet_stat(&mut self, data: &[i64]) {
        match PetStats::parse(data) {
            Ok(ps) => {
                let idx = ps.id;
                if let Some(pet) =
                    self.habitats.get_mut(ps.element).pets.get_mut(idx % 20)
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Enum, EnumIter, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HabitatType {
    #[default]
    Shadow = 0,
    Light = 1,
    Earth = 2,
    Fire = 3,
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
        Some(match id {
            1..=20 => HabitatType::Shadow,
            21..=40 => HabitatType::Light,
            41..=60 => HabitatType::Earth,
            61..=80 => HabitatType::Fire,
            81..=100 => HabitatType::Water,
            _ => return None,
        })
    }

    pub(crate) fn from_typ_id(id: i64) -> Option<Self> {
        Some(match id {
            1 => HabitatType::Shadow,
            2 => HabitatType::Light,
            3 => HabitatType::Earth,
            4 => HabitatType::Fire,
            5 => HabitatType::Water,
            _ => return None,
        })
    }
}

impl PetStats {
    pub(crate) fn parse(data: &[i64]) -> Result<Self, SFError> {
        let pet_id: u32 = data.csiget(0, "pet index", 0)?;
        let mut s = Self {
            id: pet_id as usize,
            level: data.csiget(1, "pet lvl", 0)?,
            armor: data.csiget(2, "pet armor", 0)?,
            class: data.cfpuget(3, "pet class", |a| a)?,
            min_damage: data.csiget(14, "min damage", 0)?,
            max_damage: data.csiget(15, "max damage", 0)?,

            element: match data.cget(16, "pet element")? {
                0 => HabitatType::from_pet_id(i64::from(pet_id)).ok_or_else(
                    || SFError::ParsingError("det pet typ", pet_id.to_string()),
                )?,
                x => HabitatType::from_typ_id(x).ok_or_else(|| {
                    SFError::ParsingError("det pet typ", x.to_string())
                })?,
            },
            ..Default::default()
        };
        update_enum_map(&mut s.attributes, data.skip(4, "pet attrs")?);
        update_enum_map(&mut s.bonus_attributes, data.skip(9, "pet bonus")?);
        Ok(s)
    }
}

#[derive(Debug, Clone, Copy, strum::EnumCount, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The current state of the mirror
pub enum Mirror {
    /// The player is still collecting the mirror pieces
    Pieces {
        /// The amount of pieces the character has found
        amount: u8,
    },
    /// The player has found all mirror pieces and thus has a working mirror
    #[default]
    Full,
}

impl Mirror {
    pub(crate) fn parse(i: i64) -> Mirror {
        /// Bitmask to cover bits 20 to 32, which is where each bit set is one
        /// mirror piece found
        const MIRROR_PIECES_MASK: i64 = 0xFFF8_0000;

        if i & (1 << 8) != 0 {
            return Mirror::Full;
        }
        Mirror::Pieces {
            amount: (i & MIRROR_PIECES_MASK)
                .count_ones()
                .try_into()
                .unwrap_or(0),
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
    pub(crate) fn parse(data: &[i64]) -> Result<Vec<Unlockable>, SFError> {
        data.chunks_exact(2)
            .filter(|chunk| chunk.first().copied().unwrap_or_default() != 0)
            .map(|chunk| {
                Ok(Unlockable {
                    main_ident: chunk.cget(0, "unlockable ident")?,
                    sub_ident: chunk.cget(1, "unlockable sub ident")?,
                })
            })
            .collect()
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The current progress towards all achievements
pub struct Achievements(pub Vec<Achievement>);

impl Achievements {
    pub(crate) fn update(&mut self, data: &[i64]) -> Result<(), SFError> {
        self.0.clear();
        let total_count = data.len() / 2;
        if !data.len().is_multiple_of(2) {
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
