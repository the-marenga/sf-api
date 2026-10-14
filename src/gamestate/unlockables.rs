use std::{collections::HashSet, fmt::Display, num::NonZeroU8};

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use log::{error, warn};
use num_derive::FromPrimitive;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

use super::{AttributeType, Class, EquipmentSlot, Result, ServerTime};
use crate::{
    PlayerId,
    error::SFError,
    gamestate::items::{Enchantment, Equipment},
    response::Vals,
};

/// Information about the Hellevator event on the server. If it is active, you
/// can get more detailed info via `active()`
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

/// The status of the Hellevator event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HellevatorStatus<'a> {
    /// The event is ongoing, but you have to send a `HellevatorEnter` command
    /// to start using it
    NotEntered,
    /// The event is currently not available
    NotAvailable,
    /// The event has ended, but you can still claim the final reward
    RewardClaimable,
    /// A reference to the active Hellevator state.
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

/// Information about the player's current Hellevator run and guild raid state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hellevator {
    /// The number of key cards currently available.
    pub key_cards: u32,
    /// The current floor the player is on.
    pub current_floor: u32,
    /// The total points earned in the current Hellevator run.
    pub points: u32,
    /// Whether the final reward chest has been claimed.
    pub has_final_reward: bool,

    /// Guild points earned today.
    pub guild_points_today: u32,
    /// The current rank of the player's guild.
    pub guild_rank: u32,
    /// Information about each floor in the guild raid.
    pub guild_raid_floors: Vec<HellevatorRaidFloor>,

    /// The time at which signups for the guild raid begin.
    pub guild_raid_signup_start: DateTime<Local>,
    /// The time at which the guild raid battle begins.
    pub guild_raid_start: DateTime<Local>,
    /// Rewards obtained from defeating monsters in the Hellevator.
    pub monster_rewards: Vec<HellevatorMonsterReward>,

    /// The highest floor the player has reached.
    pub own_best_floor: u32,
    /// The three treats currently available in the Hellevator shop.
    pub shop_items: [HellevatorShopTreat; 3],

    /// The treat currently active and granting a buff, if any.
    pub current_treat: Option<HellevatorShopTreat>,

    /// The time when the next free key card will be generated.
    pub next_card_generated: Option<DateTime<Local>>,
    /// The time of the next daily reset for the Hellevator.
    pub next_reset: Option<DateTime<Local>>,
    /// The date when the player started contributing to the guild in this
    /// event.
    pub start_contrib_date: Option<DateTime<Local>>,

    /// Daily rewards earned yesterday.
    pub rewards_yesterday: Option<HellevatorDailyReward>,
    /// Daily rewards available today.
    pub rewards_today: Option<HellevatorDailyReward>,
    /// Daily rewards projected for the next day.
    pub rewards_next: Option<HellevatorDailyReward>,

    /// The active daily treat bonus buff, if any.
    pub daily_treat_bonus: Option<HellevatorTreatBonus>,

    /// The monster currently encountered on this floor, if any.
    pub current_monster: Option<HellevatorMonster>,

    /// Hellevator points earned today.
    pub earned_today: u32,
    /// Hellevator points earned yesterday.
    pub earned_yesterday: u32,

    pub(crate) brackets: Vec<u32>,
}

/// A daily treat bonus active during the Hellevator event.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorTreatBonus {
    /// The type of treat bonus granted.
    pub typ: HellevatorTreatBonusType,
    /// The strength or amount of the bonus.
    pub amount: u32,
}

/// A monster encountered on a Hellevator floor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorMonster {
    /// The unique monster identifier.
    pub id: i64,
    /// The level of the monster.
    pub level: u32,
    /// The elemental type of the monster.
    pub typ: HellevatorMonsterElement,
}

/// Elemental damage type of a Hellevator monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HellevatorMonsterElement {
    /// Fire element.
    Fire = 1,
    /// Cold element.
    Cold = 2,
    /// Lightning element.
    Lightning = 3,
    /// Unknown or unclassified element.
    #[default]
    Unknown = 240,
}

impl HellevatorMonster {
    /// Parses a monster from a 3-value Hellevator monster response `[id, level,
    /// typ]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(HellevatorMonster {
            id: data.i64(0)?,
            level: data.u32_or(1, 0),
            typ: data.enum_or(2, HellevatorMonsterElement::Unknown),
        })
    }
}

impl HellevatorTreatBonus {
    /// Parses a treat bonus from a 2-value response `[typ, amount]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(HellevatorTreatBonus {
            typ: data.enum_or(0, HellevatorTreatBonusType::Unknown),
            amount: data.u32(1)?,
        })
    }
}

/// The type of Hellevator daily treat bonus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HellevatorTreatBonusType {
    /// Bonus percentage damage against monsters.
    ExtraDamage = 14,
    /// An unknown bonus type.
    #[default]
    Unknown = 240,
}

/// A reward granted for defeating a Hellevator monster.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorMonsterReward {
    /// The kind of reward granted.
    pub typ: HellevatorMonsterRewardTyp,
    /// The amount of the reward.
    pub amount: u64,
}

/// The category of reward dropped by a Hellevator monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HellevatorMonsterRewardTyp {
    /// Hellevator event points.
    Points,
    /// Beer / thirst tickets.
    Tickets,
    /// Premium currency (mushrooms).
    Mushrooms,
    /// Gold/silver currency.
    Silver,
    /// Lucky coins for the wheel of fortune.
    LuckyCoin,
    /// Fortress wood resource.
    Wood,
    /// Fortress stone resource.
    Stone,
    /// Blacksmith arcane splinters resource.
    Arcane,
    /// Blacksmith metal resource.
    Metal,
    /// Underworld souls resource.
    Souls,
    /// Pet food fruit of a specific habitat.
    Fruit(HabitatType),

    /// An unknown reward type.
    #[default]
    Unknown,
}

impl HellevatorMonsterRewardTyp {
    /// Parses the monster reward type from a raw numeric identifier.
    #[must_use]
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

/// State of a floor in the Hellevator guild raid.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorRaidFloor {
    pub(crate) today: i64,
    pub(crate) yesterday: i64,

    /// The Hellevator points rewarded for clearing this raid floor.
    pub point_reward: u32,
    /// The silver rewarded for clearing this raid floor.
    pub silver_reward: u64,

    /// Names of guild members assigned to this floor today.
    pub today_assigned: Vec<String>,
    /// Names of guild members assigned to this floor yesterday.
    pub yesterday_assigned: Vec<String>,
}

/// The kinds of treats available in the Hellevator shop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, FromPrimitive)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HellevatorTreatType {
    /// Chocolate chilli pepper treat.
    ChocolateChilliPepper = 1,
    /// Peppermint chocolate treat.
    PeppermintChocolate = 2,
    /// Electroshock treat.
    Electroshock = 3,
    /// Chill ice cream treat.
    ChillIceCream = 4,
    /// Crackling chewing gum treat.
    CracklingChewingGum = 5,
    /// Peppermint chewing gum treat.
    PeppermintChewingGum = 6,
    /// Beer biscuit treat.
    BeerBiscuit = 7,
    /// Ginger bread heart treat.
    GingerBreadHeart = 8,
    /// Fortune cookie treat.
    FortuneCookie = 9,
    /// Canned spinach treat.
    CannedSpinach = 10,
    /// Stone biscuit treat.
    StoneBiscuit = 11,
    /// Organic granola bar treat.
    OrganicGranolaBar = 12,
    /// Chocolate gold coin treat.
    ChocolateGoldCoin = 13,
    /// An unknown treat type.
    #[default]
    Unknown = 230,
}

/// A treat available for purchase in the Hellevator shop or currently active.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorShopTreat {
    /// Whether this treat is a special / premium offer.
    pub is_special: bool,
    /// The type of treat.
    pub typ: HellevatorTreatType,
    /// Price in key cards or mushrooms.
    pub price: u32,
    /// Duration of the effect in floors or time.
    pub duration: u32,
    /// The strength or magnitude of the treat's effect.
    pub effect_strength: u32,
}

/// Daily rewards available from the Hellevator based on guild performance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorDailyReward {
    // TODO: What is the purpose of these fields?
    pub(crate) start_level: u16,
    pub(crate) end_level: u16,

    /// Number of gold chests rewarded.
    pub gold_chests: u16,
    /// Amount of silver rewarded.
    pub silver: u64,

    /// Number of fortress chests rewarded.
    pub fortress_chests: u16,
    /// Amount of wood rewarded.
    pub wood: u64,
    /// Amount of stone rewarded.
    pub stone: u64,

    /// Number of blacksmith chests rewarded.
    pub blacksmith_chests: u16,
    /// Amount of arcane splinters rewarded.
    pub arcane: u64,
    /// Amount of metal rewarded.
    pub metal: u64,
}

impl HellevatorDailyReward {
    /// Returns `true` if the daily reward can be claimed.
    #[must_use]
    pub fn claimable(&self) -> bool {
        self.gold_chests > 0
            || self.fortress_chests > 0
            || self.blacksmith_chests > 0
    }

    /// Parses daily rewards from a 10-value response.
    pub(crate) fn parse(data: &Vals<'_>) -> Option<HellevatorDailyReward> {
        if data.len() < 10 {
            data.ignore_all("short daily reward response");
            return None;
        }

        Some(HellevatorDailyReward {
            start_level: data.u16_or(0, 0),
            end_level: data.u16_or(1, 0),
            gold_chests: data.u16_or(2, 0),
            fortress_chests: data.u16_or(3, 0),
            blacksmith_chests: data.u16_or(4, 0),
            silver: data.u64_or(5, 0),
            wood: data.u64_or(6, 0),
            stone: data.u64_or(7, 0),
            arcane: data.u64_or(8, 0),
            metal: data.u64_or(9, 0),
        })
    }
}

impl Hellevator {
    /// Converts the rank of a guild in the Hellevator into the reward bracket,
    /// that they would be in (1 to 25). If the rank would gain no rewards, none
    /// is returned here.
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

    /// Updates the Hellevator state from a 32-value `gtsave` response.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.key_cards = data.u32_or(0, 0);
        self.next_card_generated = data.server_time(1, server_time)?;
        self.next_reset = data.server_time(2, server_time)?;
        self.current_floor = data.u32_or(3, 0);
        self.points = data.u32_or(4, 0);
        self.start_contrib_date = data.server_time(5, server_time)?;
        self.has_final_reward = data.u32_or(6, 0) == 1;
        self.own_best_floor = data.u32_or(7, 0);

        for (pos, shop_item) in self.shop_items.iter_mut().enumerate() {
            shop_item.typ = data.enum_or(8 + pos, HellevatorTreatType::Unknown);
            // FIXME: This is wrong
            shop_item.is_special = data.u32_or(11 + pos, 0) > 0;
            shop_item.price = data.u32_or(14 + pos, u32::MAX);
            shop_item.duration = data.u32_or(17 + pos, 0);
            shop_item.effect_strength = data.u32_or(20 + pos, 0);
        }

        let c_typ = data.i64_or(23, 0);
        let is_special = data.u32_or(24, 0) > 0;
        let duration = data.u32_or(25, 0);
        let effect_strength = data.u32_or(26, 0);
        self.current_treat = if c_typ > 0 {
            Some(HellevatorShopTreat {
                typ: data.enum_or(23, HellevatorTreatType::Unknown),
                is_special,
                price: 0,
                duration,
                effect_strength,
            })
        } else {
            None
        };

        self.earned_today = data.u32_or(27, 0);
        data.ignore(28, "probably a 'has acknowledged rank fall' msg");
        self.earned_yesterday = data.u32_or(29, 0);
        data.ignore(30, "fallen to rank");
        data.ignore(31, "unknown hellevator field");
        if data.len() > 32 {
            data.ignore(32..data.len(), "trailing hellevator fields");
        }
        Ok(())
    }
}

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
        if data.len() > 36 {
            data.ignore(36..data.len(), "trailing witch fields");
        }
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

    /// Updates pets information from an `ownpets` response.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        data.ignore(0..2, "pet header fields");
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
        if data.len() > 255 {
            data.ignore(255..data.len(), "pet trailer");
        }
        Ok(())
    }

    /// Updates the stats of a single pet from an `ownpetsstats` response.
    pub(crate) fn update_pet_stat(&mut self, data: &Vals<'_>) {
        match PetStats::parse(data) {
            Ok(ps) => {
                let idx = ps.id;
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
    /// The unique id of this pet accross all habitats (1..=101)
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
    /// Parses pet statistics from a 17-value `ownpetsstats` response.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        let pet_id = data.u32_or(0, 0);
        let element_raw = data.i64(16)?;
        let element = match element_raw {
            0 => HabitatType::from_pet_id(i64::from(pet_id)).ok_or_else(|| {
                SFError::ParsingErrorV2(format!(
                    "Could not determine pet habitat type from pet id {pet_id}"
                ))
            })?,
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
            if data.len() > 1 {
                data.ignore(1..data.len(), "trailing scrapbook fields");
            }
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
        if data.len() > 1 {
            data.ignore(1..data.len(), "trailing scrapbook fields");
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
    fn test_hellevator_treat_bonus_parse() {
        crate::parse_test!(HellevatorTreatBonus("14/30"), {
            typ: HellevatorTreatBonusType::ExtraDamage,
            amount: 30,
        });
    }

    #[test]
    fn test_hellevator_monster_parse() {
        crate::parse_test!(HellevatorMonster("123/50/2"), {
            id: 123,
            level: 50,
            typ: HellevatorMonsterElement::Cold,
        });
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
    fn test_hellevator_daily_reward_parse() {
        let raw = "1/10/2/3/4/1000/500/300/100/50";
        let data = Vals::from(raw);
        let reward = HellevatorDailyReward::parse(&data).unwrap();
        assert_eq!(reward.gold_chests, 2);
        assert_eq!(reward.fortress_chests, 3);
        assert_eq!(reward.blacksmith_chests, 4);
        assert_eq!(reward.silver, 1000);
        assert_eq!(reward.wood, 500);
        assert_eq!(reward.stone, 300);
        assert_eq!(reward.arcane, 100);
        assert_eq!(reward.metal, 50);
        assert!(reward.claimable());
        assert_all_read(&data);

        let short_data = Vals::from("0/2/0/0");
        assert!(HellevatorDailyReward::parse(&short_data).is_none());
        assert_all_read(&short_data);
    }

    #[test]
    fn test_hellevator_update() {
        let raw =
            "0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0";
        let mut hellevator = Hellevator::default();
        let server_time = ServerTime::default();
        let data = Vals::from(raw);
        hellevator.update(&data, server_time).unwrap();
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
        assert_all_read(&data);
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
        assert_all_read(&data);
    }
}
