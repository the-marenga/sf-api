use std::num::NonZeroU8;

use chrono::{DateTime, Local};
use enum_map::Enum;
use log::{error, warn};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::EnumIter;

use super::*;
use crate::{PlayerId, gamestate::items::*, misc::*};

fn parse_enum_opt<T: FromPrimitive>(raw: i64, name: &str) -> Option<T> {
    let value = T::from_i64(raw);
    if value.is_none() && !matches!(raw, 0 | -1) {
        warn!("There might be a new {name} -> {raw}");
    }
    value
}

fn parse_enum_or<T: FromPrimitive + Default>(raw: i64, name: &str) -> T {
    parse_enum_opt(raw, name).unwrap_or_default()
}

/// Information about the Hellevator event on the server. If it is active, you
/// can get more detailed info via `active()`
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub(crate) fn update_time(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.start = data.server_time_opt(0, server_time);
        self.end = data.server_time_opt(1, server_time);
        data.ignore(2, "unknown event time value");
        self.collect_time_end = data.server_time_opt(3, server_time);
        data.ignore(4..data.len(), "extra event time values");
    }

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
    pub(crate) fn parse(data: &Vals<'_>) -> Option<Self> {
        let Ok(id) = data.i64(0) else {
            data.ignore_all("empty or malformed monster id");
            return None;
        };

        let level = data.u32_or(1, 0);
        let typ = parse_enum_or(data.i64_or(2, 0), "h monster typ");
        data.ignore(3..data.len(), "extra monster values");
        Some(Self { id, level, typ })
    }
}

impl HellevatorTreatBonus {
    pub(crate) fn parse(data: &Vals<'_>) -> Option<Self> {
        if data.len() < 2 {
            data.ignore_all("incomplete hellevator treat bonus");
            return None;
        }

        let typ = parse_enum_or(data.i64_or(0, 0), "hellevator treat bonus");
        let amount = data.u32_or(1, 0);
        data.ignore(2..data.len(), "extra treat bonus values");
        Some(Self { typ, amount })
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

    pub(crate) fn parse(data: &Vals<'_>) -> Option<HellevatorDailyReward> {
        if data.len() < 10 {
            data.ignore_all("incomplete hellevator daily reward");
            return None;
        }

        let reward = HellevatorDailyReward {
            start_level: data.u16_or(0, 0),
            end_level: data.u16_or(1, 0),
            gold_chests: data.u16_or(2, 0),
            silver: data.u64_or(5, 0),
            fortress_chests: data.u16_or(3, 0),
            wood: data.u64_or(6, 0),
            stone: data.u64_or(7, 0),
            blacksmith_chests: data.u16_or(4, 0),
            arcane: data.u64_or(8, 0),
            metal: data.u64_or(9, 0),
        };
        data.ignore(10..data.len(), "extra daily reward values");
        Some(reward)
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

    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        self.key_cards = data.u32_or(0, 0);
        self.next_card_generated = data.server_time_opt(1, server_time);
        self.next_reset = data.server_time_opt(2, server_time);
        self.current_floor = data.u32_or(3, 0);
        self.points = data.u32_or(4, 0);
        self.start_contrib_date = data.server_time_opt(5, server_time);
        self.has_final_reward = data.i64_or(6, 0) == 1;
        self.own_best_floor = data.u32_or(7, 0);

        for (pos, shop_item) in self.shop_items.iter_mut().enumerate() {
            shop_item.typ =
                parse_enum_or(data.i64_or(8 + pos, 0), "hellevator shop treat");
            shop_item.is_special = data.i64_or(11 + pos, 0) > 0;
            shop_item.price = data.u32_or(14 + pos, u32::MAX);
            shop_item.duration = data.u32_or(17 + pos, 0);
            shop_item.effect_strength = data.u32_or(20 + pos, 0);
        }

        let current_typ = data.i64_or(23, 0);
        self.current_treat = if current_typ > 0 {
            Some(HellevatorShopTreat {
                typ: FromPrimitive::from_i64(current_typ).unwrap_or_default(),
                is_special: data.i64_or(24, 0) > 0,
                price: 0,
                duration: data.u32_or(25, 0),
                effect_strength: data.u32_or(26, 0),
            })
        } else {
            data.ignore(24..27, "unused current treat values");
            None
        };

        self.earned_today = data.u32_or(27, 0);
        data.ignore(28, "rank fall acknowledgement");
        self.earned_yesterday = data.u32_or(29, 0);
        data.ignore(30..32, "unmodelled earned reward fields");
        data.ignore(32..data.len(), "extra hellevator save values");
    }

    pub(crate) fn update_raid_participants(
        &mut self,
        data: &Vals<'_>,
        target: fn(&mut HellevatorRaidFloor) -> &mut Vec<String>,
    ) {
        for floor in &mut self.guild_raid_floors {
            target(floor).clear();
        }

        let pair_count = data.len() / 2;
        for pair in data.window(0..pair_count * 2).chunks(2) {
            let name = pair.get_str(0).unwrap_or_default();
            let floor = pair.usize(1).ok().unwrap_or_default();
            if floor == 0 {
                continue;
            }
            if self.guild_raid_floors.len() < floor {
                self.guild_raid_floors.resize_with(floor, Default::default);
            }
            if let Some(raid_floor) = self.guild_raid_floors.get_mut(floor - 1)
            {
                target(raid_floor).push(name.to_owned());
            }
        }
        data.ignore(
            pair_count * 2..data.len(),
            "odd trailing participant value",
        );
    }

    pub(crate) fn update_raid_fights(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.guild_raid_signup_start =
            data.server_time_opt(0, server_time).unwrap_or_default();
        self.guild_raid_start =
            data.server_time_opt(1, server_time).unwrap_or_default();

        let tail = data.window(2..data.len());
        let floor_count = tail.len() / 5;
        if self.guild_raid_floors.len() < floor_count {
            self.guild_raid_floors
                .resize_with(floor_count, Default::default);
        }
        for (chunk, floor) in tail
            .window(0..floor_count * 5)
            .chunks(5)
            .zip(&mut self.guild_raid_floors)
        {
            chunk.ignore(0, "unknown raid floor value");
            floor.today = chunk.i64_or(1, 0);
            floor.yesterday = chunk.i64_or(2, 0);
            floor.point_reward = chunk.u32_or(3, 0);
            floor.silver_reward = chunk.u64_or(4, 0);
        }
        for floor in self.guild_raid_floors.iter_mut().skip(floor_count) {
            floor.today = 0;
            floor.yesterday = 0;
            floor.point_reward = 0;
            floor.silver_reward = 0;
        }
        tail.ignore(floor_count * 5..tail.len(), "partial raid fight values");
    }

    pub(crate) fn update_monster_rewards(&mut self, data: &Vals<'_>) {
        self.monster_rewards.clear();

        let reward_count = data.len() / 3;
        for chunk in data.window(0..reward_count * 3).chunks(3) {
            let raw_typ = chunk.i64_or(0, 0);
            if raw_typ <= 0 {
                chunk.ignore(1..3, "empty monster reward values");
                continue;
            }
            let flag = chunk.i64_or(1, 0);
            if flag != 0 {
                warn!("hellevator monster t: {flag}");
            }
            let typ = HellevatorMonsterRewardTyp::parse(raw_typ);
            let amount = chunk.u64_or(2, 0);
            self.monster_rewards
                .push(HellevatorMonsterReward { typ, amount });
        }
        data.ignore(
            reward_count * 3..data.len(),
            "partial monster reward values",
        );
    }

    pub(crate) fn update_brackets(&mut self, data: &Vals<'_>) {
        let mut parsed = Vec::with_capacity(data.len());
        let mut valid = true;
        for raw in data.iter() {
            match raw.parse::<u32>() {
                Ok(value) => parsed.push(value),
                Err(_) => valid = false,
            }
        }
        self.brackets = if valid { parsed } else { Vec::new() };
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

/// The S&F server needs a character specific value for enchanting items. This
/// is that value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnchantmentIdent(pub(crate) NonZeroU8);

impl Witch {
    pub(crate) fn update(&mut self, data: &[i64]) -> Result<(), SFError> {
        self.enchantment_price = data.csiget(35, "witch price", u64::MAX)?;
        self.required_item = None;
        if data.cget(33, "w needs more")? == 0 {
            let raw_required = data.cget(34, "w required")?;
            for slot in EquipmentSlot::iter() {
                let id = i64::from(slot.raw_id());
                if id == raw_required {
                    self.required_item = Some(slot);
                    break;
                }
            }
        }
        if self.required_item.is_none() {
            self.cauldron_bubbling = true;
        } else {
            // I would like to offer the raw values here, but the -1 just
            // makes this annoying. A Option<(u32, u32)> is also weird
            let current: i32 = data.ciget(2, "witch current")?;
            let target: i32 = data.ciget(3, "witch target")?;
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            if current < 0 || target <= 0 {
                self.progress = 100;
            } else {
                let current = f64::from(current);
                let target = f64::from(target);
                self.progress = ((current / target) * 100.0) as u32;
            }
        }

        let e_count: u8 = data.ciget(4, "enchant count")?;
        for i in 0..e_count {
            let iid = data.cget(6 + 3 * i as usize, "iid")? - 1;
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

/// Maps the index of the pet in their habitat to their base stats
#[cfg(feature = "simulation")]
static PET_BASE_STAT_ARRAY: [u32; 20] = [
    10, 11, 12, 13, 14, 16, 18, 20, 25, 30, 35, 40, 50, 60, 70, 80, 100, 130,
    160, 160,
];

/// Maps the habitat relativ eindex of the pet to their class
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
    /// Get the current monster we would be fighting, when
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
        let pack_bonus = habitat_pets
            .iter()
            .map(|a| match a.level {
                0 => 0.0,
                _ => 0.05,
            })
            .sum::<f64>();

        let level_bonus = habitat_pets
            .iter()
            .map(|p| match p.level {
                ..100 => 0.0,
                100..150 => 0.05,
                150..200 => 0.75,
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

/// Represents the current state of the habitat exploration
#[derive(Debug, Default, Clone)]
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
    /// The unique id of this pet accross all habitats (1..=101)
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

/// The current progress towards all achievements
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[derive(Debug, Clone)]
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
                            "Two scrapbook positions parsed to the same \
                             ident: {index}"
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

    const GTSAVE_FULL: &str = concat!(
        "5/1700000001/1700000002/7/1234/1700000005/1/9/",
        "1/2/3/1/0/1/100/200/300/",
        "60/120/180/10/20/30/",
        "4/1/45/50/11/ignored/22/ignored/ignored",
    );

    fn full_hellevator() -> Hellevator {
        let data = Vals::from(GTSAVE_FULL);
        let mut hellevator = Hellevator::default();
        hellevator.update(&data, ServerTime::default());
        data.assert_all_read();
        hellevator
    }

    #[test]
    fn event_time_handles_full_truncated_and_extra_values() {
        let server_time = ServerTime::default();
        let expected_start = server_time
            .convert_to_local(1_700_000_000, "test event start")
            .unwrap();
        let expected_end = server_time
            .convert_to_local(1_700_000_100, "test event end")
            .unwrap();
        let expected_collect_end = server_time
            .convert_to_local(1_700_000_200, "test collect end")
            .unwrap();
        let mut event = HellevatorEvent::default();

        let full = Vals::from("1700000000/1700000100/ignored/1700000200");
        event.update_time(&full, server_time);
        full.assert_all_read();
        assert_eq!(event.start.as_ref(), Some(&expected_start));
        assert_eq!(event.end.as_ref(), Some(&expected_end));
        assert_eq!(
            event.collect_time_end.as_ref(),
            Some(&expected_collect_end)
        );

        let truncated = Vals::from("1700000000");
        event.update_time(&truncated, server_time);
        truncated.assert_all_read();
        assert_eq!(event.start.as_ref(), Some(&expected_start));
        assert_eq!(event.end, None);
        assert_eq!(event.collect_time_end, None);

        let extra = Vals::from(
            "1700000000/1700000100/ignored/1700000200/extra/malformed",
        );
        event.update_time(&extra, server_time);
        extra.assert_all_read();
        assert_eq!(event.start.as_ref(), Some(&expected_start));
        assert_eq!(event.end.as_ref(), Some(&expected_end));
        assert_eq!(
            event.collect_time_end.as_ref(),
            Some(&expected_collect_end)
        );
    }

    #[test]
    fn monster_parsing_handles_valid_malformed_and_missing_fields() {
        let valid = Vals::from("42/12/2/extra");
        let monster = HellevatorMonster::parse(&valid).unwrap();
        valid.assert_all_read();
        assert_eq!(monster.id, 42);
        assert_eq!(monster.level, 12);
        assert_eq!(monster.typ, HellevatorMonsterElement::Cold);

        for raw in ["", "malformed/12/2/extra"] {
            let malformed = Vals::from(raw);
            assert!(HellevatorMonster::parse(&malformed).is_none());
            malformed.assert_all_read();
        }

        let missing = Vals::from("42");
        let monster = HellevatorMonster::parse(&missing).unwrap();
        missing.assert_all_read();
        assert_eq!(monster.id, 42);
        assert_eq!(monster.level, 0);
        assert_eq!(monster.typ, HellevatorMonsterElement::Unknown);

        let unknown_type = Vals::from("42/12/999/extra");
        let monster = HellevatorMonster::parse(&unknown_type).unwrap();
        unknown_type.assert_all_read();
        assert_eq!(monster.typ, HellevatorMonsterElement::Unknown);
    }

    #[test]
    fn treat_bonus_parsing_handles_valid_short_and_malformed_values() {
        let valid = Vals::from("14/25/extra");
        let bonus = HellevatorTreatBonus::parse(&valid).unwrap();
        valid.assert_all_read();
        assert_eq!(bonus.typ, HellevatorTreatBonusType::ExtraDamage);
        assert_eq!(bonus.amount, 25);

        let short = Vals::from("14");
        assert!(HellevatorTreatBonus::parse(&short).is_none());
        short.assert_all_read();

        let malformed = Vals::from("999/not-a-number/extra");
        let bonus = HellevatorTreatBonus::parse(&malformed).unwrap();
        malformed.assert_all_read();
        assert_eq!(bonus.typ, HellevatorTreatBonusType::Unknown);
        assert_eq!(bonus.amount, 0);
    }

    #[test]
    fn daily_reward_parsing_handles_full_short_and_malformed_values() {
        let full = Vals::from("10/20/2/3/4/500/600/700/800/900/extra");
        let reward = HellevatorDailyReward::parse(&full).unwrap();
        full.assert_all_read();
        assert_eq!(reward.start_level, 10);
        assert_eq!(reward.end_level, 20);
        assert_eq!(reward.gold_chests, 2);
        assert_eq!(reward.fortress_chests, 3);
        assert_eq!(reward.blacksmith_chests, 4);
        assert_eq!(reward.silver, 500);
        assert_eq!(reward.wood, 600);
        assert_eq!(reward.stone, 700);
        assert_eq!(reward.arcane, 800);
        assert_eq!(reward.metal, 900);
        assert!(reward.claimable());

        let short = Vals::from("10/20/2");
        assert!(HellevatorDailyReward::parse(&short).is_none());
        short.assert_all_read();

        let malformed = Vals::from("bad/20/-1/3/-2/-3/bad/700/800/900/extra");
        let reward = HellevatorDailyReward::parse(&malformed).unwrap();
        malformed.assert_all_read();
        assert_eq!(reward.start_level, 0);
        assert_eq!(reward.end_level, 20);
        assert_eq!(reward.gold_chests, 0);
        assert_eq!(reward.fortress_chests, 3);
        assert_eq!(reward.blacksmith_chests, 0);
        assert_eq!(reward.silver, 0);
        assert_eq!(reward.wood, 0);
        assert_eq!(reward.stone, 700);
        assert_eq!(reward.arcane, 800);
        assert_eq!(reward.metal, 900);
    }

    #[test]
    fn full_hellevator_save_parses_all_modeled_fields() {
        let server_time = ServerTime::default();
        let data = Vals::from(GTSAVE_FULL);
        assert_eq!(data.len(), 32);
        let mut hellevator = Hellevator::default();
        hellevator.update(&data, server_time);
        data.assert_all_read();

        assert_eq!(hellevator.key_cards, 5);
        assert_eq!(hellevator.current_floor, 7);
        assert_eq!(hellevator.points, 1234);
        assert!(hellevator.has_final_reward);
        assert_eq!(hellevator.own_best_floor, 9);
        let expected_next_card = server_time
            .convert_to_local(1_700_000_001, "test next card")
            .unwrap();
        let expected_reset = server_time
            .convert_to_local(1_700_000_002, "test reset")
            .unwrap();
        let expected_contrib = server_time
            .convert_to_local(1_700_000_005, "test contribution start")
            .unwrap();
        assert_eq!(
            hellevator.next_card_generated.as_ref(),
            Some(&expected_next_card)
        );
        assert_eq!(hellevator.next_reset.as_ref(), Some(&expected_reset));
        assert_eq!(
            hellevator.start_contrib_date.as_ref(),
            Some(&expected_contrib)
        );

        assert_eq!(
            hellevator.shop_items[0].typ,
            HellevatorTreatType::ChocolateChilliPepper
        );
        assert_eq!(
            hellevator.shop_items[1].typ,
            HellevatorTreatType::PeppermintChocolate
        );
        assert_eq!(
            hellevator.shop_items[2].typ,
            HellevatorTreatType::Electroshock
        );
        assert!(hellevator.shop_items[0].is_special);
        assert!(!hellevator.shop_items[1].is_special);
        assert!(hellevator.shop_items[2].is_special);
        assert_eq!(hellevator.shop_items[0].price, 100);
        assert_eq!(hellevator.shop_items[1].price, 200);
        assert_eq!(hellevator.shop_items[2].price, 300);
        assert_eq!(hellevator.shop_items[0].duration, 60);
        assert_eq!(hellevator.shop_items[1].duration, 120);
        assert_eq!(hellevator.shop_items[2].duration, 180);
        assert_eq!(hellevator.shop_items[0].effect_strength, 10);
        assert_eq!(hellevator.shop_items[1].effect_strength, 20);
        assert_eq!(hellevator.shop_items[2].effect_strength, 30);

        let current = hellevator.current_treat.as_ref().unwrap();
        assert_eq!(current.typ, HellevatorTreatType::ChillIceCream);
        assert!(current.is_special);
        assert_eq!(current.price, 0);
        assert_eq!(current.duration, 45);
        assert_eq!(current.effect_strength, 50);
        assert_eq!(hellevator.earned_today, 11);
        assert_eq!(hellevator.earned_yesterday, 22);
    }

    #[test]
    fn malformed_hellevator_save_defaults_and_clears_stale_state() {
        let mut hellevator = full_hellevator();
        let raw = vec!["malformed"; 32].join("/");
        let data = Vals::from(raw.as_str());
        hellevator.update(&data, ServerTime::default());
        data.assert_all_read();

        assert_eq!(hellevator.key_cards, 0);
        assert_eq!(hellevator.current_floor, 0);
        assert_eq!(hellevator.points, 0);
        assert!(!hellevator.has_final_reward);
        assert_eq!(hellevator.own_best_floor, 0);
        assert_eq!(hellevator.next_card_generated, None);
        assert_eq!(hellevator.next_reset, None);
        assert_eq!(hellevator.start_contrib_date, None);
        assert_eq!(hellevator.earned_today, 0);
        assert_eq!(hellevator.earned_yesterday, 0);
        assert!(hellevator.current_treat.is_none());
        for shop_item in &hellevator.shop_items {
            assert_eq!(shop_item.typ, HellevatorTreatType::Unknown);
            assert!(!shop_item.is_special);
            assert_eq!(shop_item.price, u32::MAX);
            assert_eq!(shop_item.duration, 0);
            assert_eq!(shop_item.effect_strength, 0);
        }
    }

    #[test]
    fn truncated_hellevator_save_defaults_and_clears_stale_state() {
        let server_time = ServerTime::default();
        let mut hellevator = full_hellevator();
        let data = Vals::from("8/1700000101");
        hellevator.update(&data, server_time);
        data.assert_all_read();

        assert_eq!(hellevator.key_cards, 8);
        let expected_next_card = server_time
            .convert_to_local(1_700_000_101, "test truncated next card")
            .unwrap();
        assert_eq!(
            hellevator.next_card_generated.as_ref(),
            Some(&expected_next_card)
        );
        assert_eq!(hellevator.next_reset, None);
        assert_eq!(hellevator.start_contrib_date, None);
        assert_eq!(hellevator.current_floor, 0);
        assert_eq!(hellevator.points, 0);
        assert!(!hellevator.has_final_reward);
        assert_eq!(hellevator.own_best_floor, 0);
        assert_eq!(hellevator.earned_today, 0);
        assert_eq!(hellevator.earned_yesterday, 0);
        assert!(hellevator.current_treat.is_none());
        for shop_item in &hellevator.shop_items {
            assert_eq!(shop_item.typ, HellevatorTreatType::Unknown);
            assert_eq!(shop_item.price, u32::MAX);
        }
    }

    #[test]
    fn raid_participants_update_today_yesterday_and_ignore_odd_tail() {
        let mut hellevator = Hellevator {
            guild_raid_floors: vec![
                HellevatorRaidFloor {
                    today_assigned: vec!["old-today-0".to_owned()],
                    yesterday_assigned: vec!["old-yesterday-0".to_owned()],
                    ..Default::default()
                },
                HellevatorRaidFloor {
                    today_assigned: vec!["old-today-1".to_owned()],
                    yesterday_assigned: vec!["old-yesterday-1".to_owned()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let today = Vals::from("Alice/2/Bob/1/Carol/4/unpaired");
        hellevator.update_raid_participants(&today, |floor| {
            &mut floor.today_assigned
        });
        today.assert_all_read();
        assert_eq!(hellevator.guild_raid_floors.len(), 4);
        assert_eq!(
            hellevator.guild_raid_floors[0].today_assigned,
            vec!["Bob".to_owned()]
        );
        assert_eq!(
            hellevator.guild_raid_floors[1].today_assigned,
            vec!["Alice".to_owned()]
        );
        assert_eq!(
            hellevator.guild_raid_floors[2].today_assigned,
            Vec::<String>::new()
        );
        assert_eq!(
            hellevator.guild_raid_floors[3].today_assigned,
            vec!["Carol".to_owned()]
        );
        assert_eq!(
            hellevator.guild_raid_floors[0].yesterday_assigned,
            vec!["old-yesterday-0".to_owned()]
        );
        assert_eq!(
            hellevator.guild_raid_floors[1].yesterday_assigned,
            vec!["old-yesterday-1".to_owned()]
        );

        let yesterday = Vals::from("Dave/3/Eve/2/unpaired");
        hellevator.update_raid_participants(&yesterday, |floor| {
            &mut floor.yesterday_assigned
        });
        yesterday.assert_all_read();
        assert_eq!(hellevator.guild_raid_floors.len(), 4);
        assert_eq!(
            hellevator.guild_raid_floors[0].yesterday_assigned,
            Vec::<String>::new()
        );
        assert_eq!(
            hellevator.guild_raid_floors[1].yesterday_assigned,
            vec!["Eve".to_owned()]
        );
        assert_eq!(
            hellevator.guild_raid_floors[2].yesterday_assigned,
            vec!["Dave".to_owned()]
        );
        assert_eq!(
            hellevator.guild_raid_floors[3].yesterday_assigned,
            Vec::<String>::new()
        );
        assert_eq!(
            hellevator.guild_raid_floors[0].today_assigned,
            vec!["Bob".to_owned()]
        );
        assert_eq!(
            hellevator.guild_raid_floors[1].today_assigned,
            vec!["Alice".to_owned()]
        );
    }

    #[test]
    fn raid_fights_parse_full_data_grow_and_ignore_partial_tail() {
        let server_time = ServerTime::default();
        let mut hellevator = Hellevator {
            guild_raid_floors: vec![HellevatorRaidFloor {
                today_assigned: vec!["keep".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let full =
            Vals::from("1700000200/1700000201/99/1/2/10/20/88/3/4/30/40");
        hellevator.update_raid_fights(&full, server_time);
        full.assert_all_read();

        let expected_signup = server_time
            .convert_to_local(1_700_000_200, "test raid signup")
            .unwrap();
        let expected_start = server_time
            .convert_to_local(1_700_000_201, "test raid start")
            .unwrap();
        assert_eq!(hellevator.guild_raid_signup_start, expected_signup);
        assert_eq!(hellevator.guild_raid_start, expected_start);
        assert_eq!(hellevator.guild_raid_floors.len(), 2);
        assert_eq!(hellevator.guild_raid_floors[0].today, 1);
        assert_eq!(hellevator.guild_raid_floors[0].yesterday, 2);
        assert_eq!(hellevator.guild_raid_floors[0].point_reward, 10);
        assert_eq!(hellevator.guild_raid_floors[0].silver_reward, 20);
        assert_eq!(hellevator.guild_raid_floors[1].today, 3);
        assert_eq!(hellevator.guild_raid_floors[1].yesterday, 4);
        assert_eq!(hellevator.guild_raid_floors[1].point_reward, 30);
        assert_eq!(hellevator.guild_raid_floors[1].silver_reward, 40);
        assert_eq!(
            hellevator.guild_raid_floors[0].today_assigned,
            vec!["keep".to_owned()]
        );

        hellevator.guild_raid_floors[1].today_assigned =
            vec!["keep-second".to_owned()];
        let partial = Vals::from("0/0/99/5/6/7/8/9/tail");
        hellevator.update_raid_fights(&partial, server_time);
        partial.assert_all_read();
        assert_eq!(
            hellevator.guild_raid_signup_start,
            DateTime::<Local>::default()
        );
        assert_eq!(hellevator.guild_raid_start, DateTime::<Local>::default());
        assert_eq!(hellevator.guild_raid_floors.len(), 2);
        assert_eq!(hellevator.guild_raid_floors[0].today, 5);
        assert_eq!(hellevator.guild_raid_floors[0].yesterday, 6);
        assert_eq!(hellevator.guild_raid_floors[0].point_reward, 7);
        assert_eq!(hellevator.guild_raid_floors[0].silver_reward, 8);
        assert_eq!(hellevator.guild_raid_floors[1].today, 0);
        assert_eq!(hellevator.guild_raid_floors[1].yesterday, 0);
        assert_eq!(hellevator.guild_raid_floors[1].point_reward, 0);
        assert_eq!(hellevator.guild_raid_floors[1].silver_reward, 0);
        assert_eq!(
            hellevator.guild_raid_floors[1].today_assigned,
            vec!["keep-second".to_owned()]
        );
    }

    #[test]
    fn monster_rewards_skip_empty_types_warn_flags_and_ignore_partial_tail() {
        let mut hellevator = Hellevator {
            monster_rewards: vec![HellevatorMonsterReward {
                typ: HellevatorMonsterRewardTyp::Unknown,
                amount: 1,
            }],
            ..Default::default()
        };
        let data = Vals::from("0/7/100/4/7/250/11/1/300/tail");
        hellevator.update_monster_rewards(&data);
        data.assert_all_read();

        assert_eq!(hellevator.monster_rewards.len(), 2);
        assert!(matches!(
            hellevator.monster_rewards[0].typ,
            HellevatorMonsterRewardTyp::Silver
        ));
        assert_eq!(hellevator.monster_rewards[0].amount, 250);
        assert!(matches!(
            hellevator.monster_rewards[1].typ,
            HellevatorMonsterRewardTyp::Fruit(HabitatType::Shadow)
        ));
        assert_eq!(hellevator.monster_rewards[1].amount, 300);
    }

    #[test]
    fn brackets_fail_closed_on_any_malformed_entry() {
        let mut hellevator = Hellevator {
            brackets: vec![999],
            ..Default::default()
        };
        let valid = Vals::from("10/20/30");
        hellevator.update_brackets(&valid);
        valid.assert_all_read();
        assert_eq!(hellevator.brackets, vec![10, 20, 30]);

        let data = Vals::from("10/bad/20/-1/30/4294967296");
        hellevator.update_brackets(&data);
        data.assert_all_read();
        assert_eq!(hellevator.brackets, Vec::<u32>::new());
    }
}
