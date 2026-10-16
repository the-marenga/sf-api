use chrono::{DateTime, Local};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{
    gamestate::{Result, ServerTime, pets::HabitatType},
    response::Vals,
};

/// Information about the Hellevator event on the server. If it is active, you
/// can get more detailed info via [`status()`](Self::status)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorEvent {
    /// The time the hellevator event was enabled at
    pub start: Option<DateTime<Local>>,
    /// The time the hellevator event will be disabled at
    pub end: Option<DateTime<Local>>,
    /// The deadline (up to 7 days after the event ends) after which the final
    /// ranking reward can no longer be collected
    pub collect_time_end: Option<DateTime<Local>>,
    /// Contains the hellevator. This can be `Some(_)`, even if the event is
    /// not ongoing, so you should use the [`status`](Self::status)
    /// function to get this
    pub(crate) active: Option<Hellevator>,
}

/// The status of the Hellevator event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HellevatorStatus<'a> {
    /// The event is ongoing, but you have to send a
    /// [`Command::HellevatorEnter`](crate::command::Command::HellevatorEnter)
    /// command to start using it
    NotEntered,
    /// The event is currently not available
    NotAvailable,
    /// The event has ended, but you can still claim the final reward with
    /// [`Command::HellevatorClaimFinal`](crate::command::Command::HellevatorClaimFinal).
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
        matches!((self.start, self.end), (Some(start), Some(end)) if (start..end).contains(&now))
    }

    /// If the Hellevator event is active, this returns a reference to the
    /// information about it. Note that you still need to check the level >= 10
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

    /// Updates the event timing from a `gttime` response:
    /// `[event_start, event_end, unknown, claim_time_end, ...]`.
    pub(crate) fn update_time(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.start = data.server_time_opt(0, server_time);
        self.end = data.server_time_opt(1, server_time);
        data.ignore(2, "unknown gttime field");
        self.collect_time_end = data.server_time_opt(3, server_time);
    }
}

/// Information about the player's current Hellevator run and guild raid state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hellevator {
    /// The number of key cards currently available. One card generates every
    /// 30 minutes up to a cap of 15.
    pub key_cards: u32,
    /// The current floor the player is on.
    pub current_floor: u32,
    /// The total points (Hell Tokens) earned in the current Hellevator run.
    #[doc(alias = "tokens")]
    #[doc(alias = "hell_tokens")]
    pub points: u32,
    /// Whether the final reward chest has been claimed via
    /// [`Command::HellevatorClaimFinal`](crate::command::Command::HellevatorClaimFinal).
    pub has_final_reward: bool,

    /// Guild points (Hell Tokens) earned today.
    #[doc(alias = "guild_tokens_today")]
    pub guild_points_today: u32,
    /// The current rank of the player's guild. See
    /// [`Command::HellevatorViewGuildRanking`](crate::command::Command::HellevatorViewGuildRanking)
    /// to refresh or view it.
    pub guild_rank: u32,
    /// Information about each plain (group of 50 floors) in the guild raid.
    /// Attack a plain with
    /// [`Command::HellevatorJoinHellAttack`](crate::command::Command::HellevatorJoinHellAttack).
    #[doc(alias = "plains")]
    #[doc(alias = "hell_attack")]
    pub guild_raid_floors: Vec<HellevatorRaidFloor>,

    /// The time at which signups for the guild raid begin.
    pub guild_raid_signup_start: DateTime<Local>,
    /// The time at which the guild raid battle begins.
    pub guild_raid_start: DateTime<Local>,
    /// Rewards obtained from defeating monsters in the Hellevator.
    pub monster_rewards: Vec<HellevatorMonsterReward>,

    /// The highest floor the player has reached.
    pub own_best_floor: u32,
    /// The three treats currently available in the Hellevator shop. Purchase
    /// with
    /// [`Command::HellevatorBuy`](crate::command::Command::HellevatorBuy) or
    /// reroll
    /// with [`Command::HellevatorRefreshShop`](crate::command::Command::HellevatorRefreshShop).
    pub shop_items: [HellevatorShopTreat; 3],

    /// The treat currently active and granting a buff, if any.
    pub current_treat: Option<HellevatorShopTreat>,

    /// The time when the next free key card will be generated (every 30
    /// minutes up to the 15-card cap).
    pub next_card_generated: Option<DateTime<Local>>,
    /// The time of the next daily reset for the Hellevator (resets current
    /// floor downwards by 20 at midnight server time).
    pub next_reset: Option<DateTime<Local>>,
    /// The date when the player started contributing to the guild in this
    /// event.
    pub start_contrib_date: Option<DateTime<Local>>,

    /// Daily rewards earned yesterday. Claim with
    /// [`Command::HellevatorClaimDailyYesterday`](crate::command::Command::HellevatorClaimDailyYesterday).
    pub rewards_yesterday: Option<HellevatorDailyReward>,
    /// Daily rewards available today. Claim with
    /// [`Command::HellevatorClaimDaily`](crate::command::Command::HellevatorClaimDaily).
    pub rewards_today: Option<HellevatorDailyReward>,
    /// Daily rewards projected for the next day.
    pub rewards_next: Option<HellevatorDailyReward>,

    /// The active daily treat bonus buff (e.g. extra weapon damage added from
    /// day 2 onwards), if any.
    pub daily_treat_bonus: Option<HellevatorTreatBonus>,

    /// The monster currently encountered on this floor, if any. Fight with
    /// [`Command::HellevatorFight`](crate::command::Command::HellevatorFight).
    pub current_monster: Option<HellevatorMonster>,

    /// Hellevator points (Hell Tokens) earned today.
    #[doc(alias = "tokens_today")]
    pub earned_today: u32,
    /// Hellevator points (Hell Tokens) earned yesterday.
    #[doc(alias = "tokens_yesterday")]
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
    /// Hellevator event points (Hell Tokens).
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
            11..=15 => HabitatType::from_typ_id(data - 10).map_or(
                HellevatorMonsterRewardTyp::Unknown,
                HellevatorMonsterRewardTyp::Fruit,
            ),

            _ => HellevatorMonsterRewardTyp::Unknown,
        }
    }
}

/// State of a plain (50-floor section) in the Hellevator guild raid.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorRaidFloor {
    pub(crate) today: i64,
    pub(crate) yesterday: i64,

    /// The Hellevator points (Hell Tokens) rewarded for clearing this plain.
    #[doc(alias = "token_reward")]
    pub point_reward: u32,
    /// The silver rewarded for clearing this plain.
    pub silver_reward: u64,

    /// Names of guild members assigned to this plain today.
    pub today_assigned: Vec<String>,
    /// Names of guild members assigned to this plain yesterday.
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
///
/// Can be purchased via
/// [`Command::HellevatorBuy`](crate::command::Command::HellevatorBuy).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HellevatorShopTreat {
    /// Whether this treat is a special / premium offer.
    pub is_special: bool,
    /// The type of treat.
    pub typ: HellevatorTreatType,
    /// Price in key cards or mushrooms.
    pub price: u32,
    /// Duration of the effect in floors (stacks up to 24 floors).
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
    ///
    /// Claim today's rewards via
    /// [`Command::HellevatorClaimDaily`](crate::command::Command::HellevatorClaimDaily)
    /// or yesterday's rewards via
    /// [`Command::HellevatorClaimDailyYesterday`](crate::command::Command::HellevatorClaimDailyYesterday).
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
    /// Converts the rank of a guild in the Hellevator into the reward bracket
    /// that they would be in (1 to 25). If the rank would gain no rewards, none
    /// is returned here.
    #[must_use]
    pub fn rank_to_rewards_rank(&self, rank: u32) -> Option<u32> {
        if rank == 0 {
            return None;
        }
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
                typ: FromPrimitive::from_i64(c_typ)
                    .unwrap_or(HellevatorTreatType::Unknown),
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
        Ok(())
    }

    /// Updates guild members assigned to raid floors from a
    /// `gtraidparticipants` or `gtraidparticipantsyesterday` response:
    /// pairs of `[player_name, floor_number]`.
    pub(crate) fn update_raid_participants(
        &mut self,
        data: &Vals<'_>,
        target: fn(&mut HellevatorRaidFloor) -> &mut Vec<String>,
    ) {
        for floor in &mut self.guild_raid_floors {
            target(floor).clear();
        }

        for chunk in data.chunks(2) {
            let Some(name) = chunk.get_str(0) else {
                continue;
            };
            let val = chunk.usize_or(1, 0);
            if val > 0 {
                if val > self.guild_raid_floors.len() {
                    self.guild_raid_floors.resize_with(val, Default::default);
                }
                if let Some(floor) = self.guild_raid_floors.get_mut(val - 1) {
                    target(floor).push(name.to_string());
                }
            }
        }
    }

    /// Updates guild raid fights from a `gtraidfights` response:
    /// `[signup_start, attack_start, (unused, today, yesterday, points,
    /// silver)*]`.
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
        if floor_count > self.guild_raid_floors.len() {
            self.guild_raid_floors
                .resize_with(floor_count, Default::default);
        }

        for (chunk, floor) in tail.chunks(5).zip(&mut self.guild_raid_floors) {
            chunk.ignore(0, "unknown floor field");
            floor.today = chunk.i64_or(1, 0);
            floor.yesterday = chunk.i64_or(2, 0);
            floor.point_reward = chunk.u32_or(3, 0);
            floor.silver_reward = chunk.u64_or(4, 0);
        }
    }

    /// Updates defeated monster rewards from a `gtmonsterreward` response:
    /// repeating 3-value chunks `[typ, flag, amount]`.
    pub(crate) fn update_monster_rewards(&mut self, data: &Vals<'_>) {
        self.monster_rewards.clear();
        for chunk in data.chunks(3) {
            let raw_typ = chunk.i64_or(0, 0);
            if raw_typ <= 0 {
                chunk.ignore(1..3, "unused empty reward");
                continue;
            }
            let one = chunk.i64_or(1, 0);
            if one != 0 {
                warn!("hellevator monster t: {one}");
            }
            let typ = HellevatorMonsterRewardTyp::parse(raw_typ);
            let amount = chunk.u64_or(2, 0);
            self.monster_rewards
                .push(HellevatorMonsterReward { typ, amount });
        }
    }

    /// Updates the brackets list from a `gtbracketlist` response.
    pub(crate) fn update_brackets(&mut self, data: &Vals<'_>) {
        self.brackets = data.iter().filter_map(|s| s.parse().ok()).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hellevator_treat_bonus_parse() {
        let data = Vals::from("14/30");
        let bonus = HellevatorTreatBonus::parse(&data).unwrap();
        data.assert_all_read();
        assert_eq!(bonus.typ, HellevatorTreatBonusType::ExtraDamage);
        assert_eq!(bonus.amount, 30);
    }

    #[test]
    fn test_hellevator_monster_parse() {
        let data = Vals::from("123/50/2");
        let monster = HellevatorMonster::parse(&data).unwrap();
        data.assert_all_read();
        assert_eq!(monster.id, 123);
        assert_eq!(monster.level, 50);
        assert_eq!(monster.typ, HellevatorMonsterElement::Cold);
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
        data.assert_all_read();

        let short_data = Vals::from("0/2/0/0");
        assert!(HellevatorDailyReward::parse(&short_data).is_none());
        short_data.assert_all_read();
    }

    #[test]
    fn test_hellevator_update() {
        let raw =
            "0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0";
        let mut hellevator = Hellevator::default();
        let server_time = ServerTime::default();
        let data = Vals::from(raw);
        hellevator.update(&data, server_time).unwrap();
        data.assert_all_read();
    }

    #[test]
    fn test_rank_to_rewards_rank() {
        let hellevator = Hellevator {
            brackets: vec![1, 4, 10], /* Bracket 1: 1, Bracket 2: 2..=5,
                                       * Bracket 3: 6..=15 */
            ..Default::default()
        };
        assert_eq!(hellevator.rank_to_rewards_rank(0), None);
        assert_eq!(hellevator.rank_to_rewards_rank(1), Some(1));
        assert_eq!(hellevator.rank_to_rewards_rank(2), Some(2));
        assert_eq!(hellevator.rank_to_rewards_rank(5), Some(2));
        assert_eq!(hellevator.rank_to_rewards_rank(6), Some(3));
        assert_eq!(hellevator.rank_to_rewards_rank(15), Some(3));
        assert_eq!(hellevator.rank_to_rewards_rank(16), None);
    }

    #[test]
    fn test_monster_reward_type_parse() {
        assert_eq!(
            HellevatorMonsterRewardTyp::parse(1),
            HellevatorMonsterRewardTyp::Points
        );
        assert_eq!(
            HellevatorMonsterRewardTyp::parse(11),
            HellevatorMonsterRewardTyp::Fruit(HabitatType::Shadow)
        );
        assert_eq!(
            HellevatorMonsterRewardTyp::parse(15),
            HellevatorMonsterRewardTyp::Fruit(HabitatType::Water)
        );
        assert_eq!(
            HellevatorMonsterRewardTyp::parse(999),
            HellevatorMonsterRewardTyp::Unknown
        );
    }

    #[test]
    fn test_hellevator_event_update_time() {
        let raw = "1600000000/1700000000/0/1800000000";
        let data = Vals::from(raw);
        let mut event = HellevatorEvent::default();
        event.update_time(&data, ServerTime::default());
        assert!(event.start.is_some());
        assert!(event.end.is_some());
        assert!(event.collect_time_end.is_some());
        data.assert_all_read();
    }

    #[test]
    fn test_hellevator_update_raid_participants() {
        let raw = "Player1/1/Player2/2";
        let data = Vals::from(raw);
        let mut hellevator = Hellevator::default();
        hellevator.update_raid_participants(&data, |f| &mut f.today_assigned);
        assert_eq!(hellevator.guild_raid_floors.len(), 2);
        assert_eq!(
            hellevator.guild_raid_floors[0].today_assigned,
            vec!["Player1"]
        );
        assert_eq!(
            hellevator.guild_raid_floors[1].today_assigned,
            vec!["Player2"]
        );
        data.assert_all_read();

        let raw_yd = "Player3/1";
        let data_yd = Vals::from(raw_yd);
        hellevator
            .update_raid_participants(&data_yd, |f| &mut f.yesterday_assigned);
        assert_eq!(
            hellevator.guild_raid_floors[0].yesterday_assigned,
            vec!["Player3"]
        );
        data_yd.assert_all_read();
    }

    #[test]
    fn test_hellevator_update_raid_fights() {
        let raw = "1600000000/1700000000/0/10/5/100/50000";
        let data = Vals::from(raw);
        let mut hellevator = Hellevator::default();
        hellevator.update_raid_fights(&data, ServerTime::default());
        assert_eq!(hellevator.guild_raid_floors.len(), 1);
        assert_eq!(hellevator.guild_raid_floors[0].today, 10);
        assert_eq!(hellevator.guild_raid_floors[0].yesterday, 5);
        assert_eq!(hellevator.guild_raid_floors[0].point_reward, 100);
        assert_eq!(hellevator.guild_raid_floors[0].silver_reward, 50000);
        data.assert_all_read();
    }

    #[test]
    fn test_hellevator_update_monster_rewards() {
        let raw = "1/0/500/10/0/200/0/0/0";
        let data = Vals::from(raw);
        let mut hellevator = Hellevator::default();
        hellevator.update_monster_rewards(&data);
        assert_eq!(hellevator.monster_rewards.len(), 2);
        assert_eq!(
            hellevator.monster_rewards[0].typ,
            HellevatorMonsterRewardTyp::Points
        );
        assert_eq!(hellevator.monster_rewards[0].amount, 500);
        assert_eq!(
            hellevator.monster_rewards[1].typ,
            HellevatorMonsterRewardTyp::Souls
        );
        assert_eq!(hellevator.monster_rewards[1].amount, 200);
        data.assert_all_read();
    }

    #[test]
    fn test_hellevator_update_brackets() {
        let raw = "1/4/10";
        let data = Vals::from(raw);
        let mut hellevator = Hellevator::default();
        hellevator.update_brackets(&data);
        assert_eq!(hellevator.brackets, vec![1, 4, 10]);
        data.assert_all_read();
    }
}
