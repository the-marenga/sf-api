use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    gamestate::{Result, ServerTime, rewards::RewardType},
    response::Vals,
};

/// The segment / body part of the world boss
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    EnumIter,
    Hash,
    Default,
    FromPrimitive,
    Enum,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossTowerSegment {
    /// Top section / head
    #[default]
    Top = 1,
    /// Middle section / torso
    Middle,
    /// Bottom section / legs
    Bottom,
}

/// Information about a World Boss event
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossEvent {
    /// The number of catalysts available to spend on catapults, upgrades, and
    /// ammunition
    pub catalysts: u32,
    /// The number of medals earned
    pub medals: u32,
    /// The player's rank on their server
    pub rank: u32,
    /// The current tower segment / body part being attacked.
    ///
    /// Can be changed via
    /// [`Command::WorldBossChangeTarget`](crate::command::Command::WorldBossChangeTarget).
    #[doc(alias = "body_part")]
    #[doc(alias = "target")]
    pub current_segment: Option<WorldBossTowerSegment>,
    /// The amount of battle loot chests available to collect.
    ///
    /// Can be claimed via
    /// [`Command::WorldBossCollectBattleRewards`](crate::command::Command::WorldBossCollectBattleRewards).
    #[doc(alias = "battle_loot")]
    pub battle_reward_chests: u32,
    /// The time until the next attack
    pub attack_timer: Option<DateTime<Local>>,
    /// The next time battle loot will be issued
    pub next_battle_loot_issued: Option<DateTime<Local>>,
    /// The amount of damage dealt per hour
    pub damage_per_hour: u32,
    /// The amount of damage dealt per attack
    #[doc(alias = "attack_damage")]
    pub damage_per_attack: u32,
    /// The damage comparative players do
    pub damage_comparative: u32,
    /// The amount of battle loot earned per hour
    pub loot_per_hour: u32,
    /// Information about the currently ongoing fight
    pub battle: Option<WorldBossBattle>,

    /// The projectiles available in the shop.
    ///
    /// Must have sent
    /// [`Command::WorldBossEnter`](crate::command::Command::WorldBossEnter) to
    /// have this available. Ammunition can be purchased with
    /// [`Command::WorldBossProjectileBuy`](crate::command::Command::WorldBossProjectileBuy).
    pub projectile_offers: Vec<ProjectileOffer>,

    /// The catapult upgrades available in the shop.
    ///
    /// Must have sent
    /// [`Command::WorldBossEnter`](crate::command::Command::WorldBossEnter) to
    /// have this available. Upgrades can be purchased with
    /// [`Command::WorldBossBuyUpgrade`](crate::command::Command::WorldBossBuyUpgrade).
    pub upgrade_offers: Vec<UpgradeOffer>,

    /// Information about the catapult, if active.
    ///
    /// A catapult can be rented via
    /// [`Command::WorldBossBuyCatapult`](crate::command::Command::WorldBossBuyCatapult)
    /// and destroyed via
    /// [`Command::WorldBossRemoveCatapult`](crate::command::Command::WorldBossRemoveCatapult).
    pub catapult: Option<WorldBossCatapult>,
    /// Information about currently loaded projectile ammunition, if any.
    ///
    /// Projectiles can be bought via
    /// [`Command::WorldBossProjectileBuy`](crate::command::Command::WorldBossProjectileBuy).
    #[doc(alias = "ammo")]
    #[doc(alias = "ammunition")]
    pub projectile: Option<WorldBossProjectile>,

    /// The number of ranks available on the "international" tab (number of
    /// servers)
    pub max_international_ranks: Option<u32>,
    /// The number of players available on the "damage" tab
    pub max_world_ranks: Option<u32>,
    /// The number of fighters available on the server ranking tab
    pub max_fighter_ranks: Option<u32>,

    /// The number of chests available for daily collection.
    ///
    /// Can be claimed via
    /// [`Command::WorldBossCollectDailyChests`](crate::command::Command::WorldBossCollectDailyChests).
    #[doc(alias = "daily_chests")]
    pub available_daily_chests: EnumMap<WorldBossChest, u32>,
}

/// The types of daily chests available for collection in the World Boss event
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossChest {
    /// Hourly battle loot chest based on damage dealt
    BattleLoot,
    /// Chest awarded for attacking the boss's designated weak point at least
    /// once
    WeakPointChest,
    /// Chest awarded to the heroes who land the killing blow on a segment (5
    /// per segment)
    KillStealChest,
    /// Chest awarded to the heroes who dealt the most total damage to a segment
    /// (3 per segment)
    DamageMasterChest,
    /// Chest awarded to the heroes who dealt the highest damage in a single hit
    /// to a segment (3 per segment)
    SingleHitChest,
}

impl WorldBossEvent {
    /// Updates the world boss character state from a 15-value `wbcharacter`
    /// response.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.catalysts = data.u32_or(0, 0);
        self.medals = data.u32_or(1, 0);
        self.rank = data.u32_or(2, 0);
        self.current_segment = data.enum_opt(3);
        data.ignore(4, "unknown world boss character field, observed 576");

        self.attack_timer = data.server_time(5, server_time)?;
        self.damage_per_attack = data.u32_or(6, 0);
        data.ignore(
            7..9,
            "unknown world boss damage fields, observed 3470 and 2029",
        );
        self.damage_per_hour = data.u32_or(9, 0);
        self.loot_per_hour = data.u32_or(10, 0);
        self.damage_comparative = data.u32_or(11, 0);
        self.battle_reward_chests = data.u32_or(12, 0);
        data.ignore(13, "unknown world boss field, observed 0");
        self.next_battle_loot_issued = data.server_time(14, server_time)?;
        Ok(())
    }

    /// Updates the upgrade offers in the shop from a response chunked by 7.
    pub(crate) fn update_upgrade_store(&mut self, data: &Vals<'_>) {
        if data.values().iter().all(|&a| a == "0" || a.is_empty()) {
            data.ignore_all("empty upgrade store");
            self.upgrade_offers.clear();
            return;
        }
        self.upgrade_offers = data
            .chunks(7)
            .map(|chunk| UpgradeOffer::parse(&chunk))
            .collect();
    }

    /// Updates the projectile offers in the shop from a response chunked by 6.
    pub(crate) fn update_ammo_store(&mut self, data: &Vals<'_>) {
        if data.values().iter().all(|&a| a == "0" || a.is_empty()) {
            data.ignore_all("empty ammo store");
            self.projectile_offers.clear();
            return;
        }
        self.projectile_offers = data
            .chunks(6)
            .map(|chunk| ProjectileOffer::parse(&chunk))
            .collect();
    }

    /// Updates the available daily chests from a 5-value `wbdailychests`
    /// response.
    pub(crate) fn update_daily_chests(&mut self, data: &Vals<'_>) {
        for (pos, chest) in self.available_daily_chests.values_mut().enumerate()
        {
            *chest = data.u32_or(pos, 0);
        }
    }
}

/// The type of projectile / ammunition used in the World Boss event
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossProjectileType {
    /// Deals extra flat damage on every hit
    Impacting = 1,
    /// Deals extra bonus damage when the boss's HP is below 30%
    Biting = 2,
    /// Deals extra bonus damage when the boss's HP is above 70%
    Mangling = 3,
    /// Unknown projectile type
    #[default]
    Unknown,
}

/// Information about the player's active World Boss projectile ammunition.
///
/// Each attack consumes 1 projectile. Stacks up to 99 shots of the same type;
/// purchasing a different projectile type replaces any existing ammunition.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossProjectile {
    /// The type of this ammunition
    pub typ: WorldBossProjectileType,
    /// The amount of ammunition remaining (stacks up to 99)
    #[doc(alias = "ammo")]
    pub amount: u32,
    /// The extra bonus damage percentage this ammunition provides (e.g. 80
    /// means +80% damage)
    #[doc(alias = "bonus_damage")]
    pub extra_dmg: u32,
}

impl WorldBossProjectile {
    /// Parses world boss projectile ammunition from a 3-value response.
    pub(crate) fn parse(data: &Vals<'_>) -> Option<WorldBossProjectile> {
        if data.values().iter().all(|&a| a == "0" || a.is_empty()) {
            data.ignore_all("empty projectile data");
            return None;
        }
        Some(Self {
            typ: data.enum_or(0, WorldBossProjectileType::Unknown),
            amount: data.u32_or(1, 0),
            extra_dmg: data.u32_or(2, 0),
        })
    }
}

/// Information about an ongoing World Boss battle
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossBattle {
    /// The level/number of the boss in the ongoing boss battles
    pub boss_nr: u32,
    /// Unknown field from the server response
    pub(crate) unknown: i64,
    /// The segment marked as the weak point of the World Boss for this player
    #[doc(alias = "weak_spot")]
    pub weak_point: Option<WorldBossTowerSegment>,
    /// Whether the weak point has already been attacked at least once to
    /// qualify for the [`WorldBossChest::WeakPointChest`]
    #[doc(alias = "weak_spot_hit")]
    pub weak_point_hit: bool,
    /// The three segments / body parts of the boss that can be fought
    #[doc(alias = "body_parts")]
    pub segments: EnumMap<WorldBossTowerSegment, WorldBossTowerSegmentInfo>,
}

impl WorldBossBattle {
    /// Parses the world boss battle information from a 106-value response.
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
        let boss_nr = data.u32_or(0, 0);
        let unknown = data.i64_or(1, 0);
        let weak_point = data.enum_opt(2);
        let weak_point_hit = data.u32_or(3, 0) == 1;

        let mut segments = EnumMap::default();
        for segment in WorldBossTowerSegment::iter() {
            let start = (segment as usize - 1) * 34 + 4;
            let seg_window = data.window(start..start + 34);
            segments[segment] = WorldBossTowerSegmentInfo::parse(&seg_window);
        }

        Self {
            boss_nr,
            unknown,
            weak_point,
            weak_point_hit,
            segments,
        }
    }
}

/// Information about one segment / body part of the World Boss
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossTowerSegmentInfo {
    /// The number of heroes currently targeting this segment
    pub signed_up_heroes: u32,
    /// The total combined damage dealt to this segment by all heroes on the
    /// server
    pub joint_damage: u64,
    /// The maximum health of this segment
    pub max_health: u64,
    /// The remaining health of this segment
    pub remaining_health: u64,
    /// The number of active special attacks/effects on this segment
    pub active_special_attacks: u32,
    /// The top 10 heroes by damage dealt to this segment
    pub most_damage_heroes: Vec<WorldBossTowerHeroDamageSummary>,
    /// The top 3 heroes by special attacks performed on this segment
    pub most_special_atk_heroes: Vec<WorldBossTowerHeroSpecialSummary>,
}

impl WorldBossTowerSegmentInfo {
    /// Parses a 34-value tower segment info window.
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
        let signed_up_heroes = data.u32_or(0, 0);
        let joint_damage = data.u64_or(1, 0);
        let max_health = data.u64_or(2, 0);
        let remaining_health = data.u64_or(3, 0);
        let active_special_attacks = data.u32_or(4, 0);

        let mut most_damage_heroes = Vec::with_capacity(10);
        for j in 0..10 {
            let base = 5 + j * 2;
            most_damage_heroes.push(WorldBossTowerHeroDamageSummary {
                level: data.u16_or(base, 0),
                name: data.str_or(base + 1, "").into(),
            });
        }

        let mut most_special_atk_heroes = Vec::with_capacity(3);
        for j in 0..3 {
            let base = 25 + j * 3;
            most_special_atk_heroes.push(WorldBossTowerHeroSpecialSummary {
                level: data.u16_or(base, 0),
                name: data.str_or(base + 1, "").into(),
                count: data.i64_or(base + 2, 0),
            });
        }

        Self {
            signed_up_heroes,
            joint_damage,
            max_health,
            remaining_health,
            active_special_attacks,
            most_damage_heroes,
            most_special_atk_heroes,
        }
    }
}

/// Summary of damage dealt by a top hero on a World Boss segment
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossTowerHeroDamageSummary {
    /// The name of the hero
    pub name: Box<str>,
    /// The character level of this hero
    pub level: u16,
}

/// Summary of special attacks performed by a top hero on a World Boss segment
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossTowerHeroSpecialSummary {
    /// The name of the hero
    pub name: Box<str>,
    /// The character level of this hero
    pub level: u16,
    /// The number of special attacks performed by this hero
    pub count: i64,
}

/// Information about the player's World Boss catapult.
///
/// Can be rented via
/// [`Command::WorldBossBuyCatapult`](crate::command::Command::WorldBossBuyCatapult)
/// and destroyed via
/// [`Command::WorldBossRemoveCatapult`](crate::command::Command::WorldBossRemoveCatapult).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossCatapult {
    /// The time at which this catapult expires and breaks
    pub breaks: DateTime<Local>,
    /// The upgrades purchased for this catapult (up to 4 slots)
    pub upgrades: [Option<WorldBossCatapultUpgrade>; 4],
}

impl WorldBossCatapult {
    /// Parses world boss catapult information from a 17-value response:
    /// `breaks / 4 * [typ, restriction, amount, effect_val]`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<Option<WorldBossCatapult>> {
        let Some(breaks) = data.server_time(0, server_time)? else {
            data.ignore(1..17, "catapult upgrades when catapult is inactive");
            return Ok(None);
        };

        let mut upgrades: [Option<WorldBossCatapultUpgrade>; 4] =
            Default::default();
        for (chunk, upgrade) in data.window(1..17).chunks(4).zip(&mut upgrades)
        {
            if chunk.values().iter().all(|&a| a == "0" || a.is_empty()) {
                continue;
            }
            *upgrade = Some(WorldBossCatapultUpgrade {
                typ: chunk.enum_or(0, WorldBossCatapultUpgradeType::Unknown),
                restriction: chunk.enum_opt(1),
                amount: chunk.u8_or(2, 0),
                effect_val: chunk.i64_or(3, 0),
            });
        }

        Ok(Some(WorldBossCatapult { breaks, upgrades }))
    }
}

/// An upgrade applied to the World Boss catapult
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossCatapultUpgrade {
    /// The type of upgrade
    pub typ: WorldBossCatapultUpgradeType,
    /// The upgrade level (0..=10)
    pub amount: u8,
    /// The specific segment this upgrade applies to, if restricted
    #[doc(alias = "body_part")]
    #[doc(alias = "target_segment")]
    pub restriction: Option<WorldBossTowerSegment>,
    /// The effect of this upgrade as an integer percentage scaled by 100
    /// (e.g. 700 represents a +7% bonus)
    pub effect_val: i64,
}

/// The type of upgrade for the World Boss catapult
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossCatapultUpgradeType {
    /// Bonus damage (restricted to a specific segment)
    Motor = 1,
    /// Shot speed / attack rate bonus
    Thread = 2,
    /// Bonus critical hit chance
    AimingDevice = 3,
    /// Bonus critical hit damage
    Bowl = 4,
    /// Unknown upgrade type
    #[default]
    Unknown,
}

/// Quantity tier when buying projectiles in the world boss shop
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProjectileOfferQuantity {
    /// Small quantity package
    Small = 1,
    /// Large quantity package
    Large,
}

/// A quantified purchase option for projectiles
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProjectileOfferQuantified {
    /// The number of projectiles included
    pub amount: u32,
    /// The price in the appropriate currency
    pub price: u32,
}

/// An offer to buy projectiles in the World Boss shop.
///
/// Can be purchased via
/// [`Command::WorldBossProjectileBuy`](crate::command::Command::WorldBossProjectileBuy).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProjectileOffer {
    /// The type of projectile offered
    pub typ: WorldBossProjectileType,
    /// The available quantity purchase options
    pub buy_options:
        EnumMap<ProjectileOfferQuantity, ProjectileOfferQuantified>,
    /// The raw bonus effect of this projectile
    pub effect_val: i64,
}

impl ProjectileOffer {
    /// Parses a projectile offer from a 6-value response.
    pub(crate) fn parse(data: &Vals<'_>) -> ProjectileOffer {
        let mut buy_options = EnumMap::default();
        buy_options[ProjectileOfferQuantity::Small] =
            ProjectileOfferQuantified {
                amount: data.u32_or(1, 20),
                price: data.u32_or(4, 100),
            };
        buy_options[ProjectileOfferQuantity::Large] =
            ProjectileOfferQuantified {
                amount: data.u32_or(2, 50),
                price: data.u32_or(5, 250),
            };
        Self {
            typ: data.enum_or(0, WorldBossProjectileType::Unknown),
            buy_options,
            effect_val: data.i64_or(3, 0),
        }
    }
}

/// An offer to upgrade the catapult in the World Boss shop.
///
/// Can be purchased via
/// [`Command::WorldBossBuyUpgrade`](crate::command::Command::WorldBossBuyUpgrade).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UpgradeOffer {
    /// The type of upgrade offered
    pub typ: WorldBossCatapultUpgradeType,
    /// The segment restriction for this upgrade, if any
    pub restriction: Option<WorldBossTowerSegment>,
    /// The effect value of this upgrade
    pub effect_value: i64,
    /// The base price paid in [`main_price_type`](Self::main_price_type).
    /// Scaled down proportionally based on remaining catapult duration
    pub raw_main_price: u64,
    /// The resource required to purchase this upgrade
    pub main_price_type: RewardType,
    /// The base catalyst price required alongside the main payment.
    /// Scaled down proportionally based on remaining catapult duration
    pub raw_catalyst_price: u64,
    /// The alternative mushroom price to purchase this upgrade directly.
    /// Scaled down proportionally based on remaining catapult duration
    pub raw_mushroom_price: u64,
}

impl UpgradeOffer {
    /// Parses an upgrade offer from a 7-value response.
    pub(crate) fn parse(data: &Vals<'_>) -> UpgradeOffer {
        UpgradeOffer {
            typ: data.enum_or(0, WorldBossCatapultUpgradeType::Unknown),
            restriction: data.enum_opt(1),
            effect_value: data.i64_or(2, 0),
            raw_main_price: data.u64_or(3, u64::MAX),
            main_price_type: RewardType::parse(data.i64_or(4, 0)),
            raw_catalyst_price: data.u64_or(5, u64::MAX),
            raw_mushroom_price: data.u64_or(6, u64::MAX),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_boss_event_update() {
        let raw = "10/20/5/1/576/1784829600/5000/3470/2029/1000/200/50/3/0/\
                   1785096000";
        let data = Vals::from(raw);
        let mut event = WorldBossEvent::default();
        event.update(&data, ServerTime::default()).unwrap();
        data.assert_all_read();

        assert_eq!(event.catalysts, 10);
        assert_eq!(event.medals, 20);
        assert_eq!(event.rank, 5);
        assert_eq!(event.current_segment, Some(WorldBossTowerSegment::Top));
        assert!(event.attack_timer.is_some());
        assert_eq!(event.damage_per_attack, 5000);
        assert_eq!(event.damage_per_hour, 1000);
        assert_eq!(event.loot_per_hour, 200);
        assert_eq!(event.damage_comparative, 50);
        assert_eq!(event.battle_reward_chests, 3);
        assert!(event.next_battle_loot_issued.is_some());
    }

    #[test]
    fn test_world_boss_upgrade_store() {
        let mut event = WorldBossEvent::default();

        // Empty store
        let empty_raw = "0/0/0/0/0/0/0";
        let empty_data = Vals::from(empty_raw);
        event.update_upgrade_store(&empty_data);
        empty_data.assert_all_read();
        assert_eq!(event.upgrade_offers, []);

        // Single offer: typ=1 (Motor), restriction=1 (Top), effect=700,
        // price=1000, type=4 (Silver), cat=50, mush=5
        let raw = "1/1/700/1000/4/50/5";
        let data = Vals::from(raw);
        event.update_upgrade_store(&data);
        data.assert_all_read();
        assert_eq!(event.upgrade_offers.len(), 1);
        let offer = &event.upgrade_offers[0];
        assert_eq!(offer.typ, WorldBossCatapultUpgradeType::Motor);
        assert_eq!(offer.restriction, Some(WorldBossTowerSegment::Top));
        assert_eq!(offer.effect_value, 700);
        assert_eq!(offer.raw_main_price, 1000);
        assert_eq!(offer.main_price_type, RewardType::Silver);
        assert_eq!(offer.raw_catalyst_price, 50);
        assert_eq!(offer.raw_mushroom_price, 5);
    }

    #[test]
    fn test_world_boss_ammo_store() {
        let mut event = WorldBossEvent::default();

        // Empty store
        let empty_raw = "0/0/0/0/0/0";
        let empty_data = Vals::from(empty_raw);
        event.update_ammo_store(&empty_data);
        empty_data.assert_all_read();
        assert_eq!(event.projectile_offers, []);

        // Single offer: typ=1 (Impacting), small amount=20, large amount=50,
        // effect=80, small price=100, large price=250
        let raw = "1/20/50/80/100/250";
        let data = Vals::from(raw);
        event.update_ammo_store(&data);
        data.assert_all_read();
        assert_eq!(event.projectile_offers.len(), 1);
        let offer = &event.projectile_offers[0];
        assert_eq!(offer.typ, WorldBossProjectileType::Impacting);
        assert_eq!(
            offer.buy_options[ProjectileOfferQuantity::Small].amount,
            20
        );
        assert_eq!(
            offer.buy_options[ProjectileOfferQuantity::Small].price,
            100
        );
        assert_eq!(
            offer.buy_options[ProjectileOfferQuantity::Large].amount,
            50
        );
        assert_eq!(
            offer.buy_options[ProjectileOfferQuantity::Large].price,
            250
        );
        assert_eq!(offer.effect_val, 80);
    }

    #[test]
    fn test_world_boss_daily_chests() {
        let mut event = WorldBossEvent::default();
        let raw = "1/2/3/4/5";
        let data = Vals::from(raw);
        event.update_daily_chests(&data);
        data.assert_all_read();

        assert_eq!(event.available_daily_chests[WorldBossChest::BattleLoot], 1);
        assert_eq!(
            event.available_daily_chests[WorldBossChest::WeakPointChest],
            2
        );
        assert_eq!(
            event.available_daily_chests[WorldBossChest::KillStealChest],
            3
        );
        assert_eq!(
            event.available_daily_chests[WorldBossChest::DamageMasterChest],
            4
        );
        assert_eq!(
            event.available_daily_chests[WorldBossChest::SingleHitChest],
            5
        );
    }

    #[test]
    fn test_world_boss_projectile_parse() {
        let empty_raw = "0/0/0";
        let empty_data = Vals::from(empty_raw);
        assert!(WorldBossProjectile::parse(&empty_data).is_none());
        empty_data.assert_all_read();

        let raw = "1/15/80";
        let data = Vals::from(raw);
        let projectile = WorldBossProjectile::parse(&data).unwrap();
        data.assert_all_read();
        assert_eq!(projectile.typ, WorldBossProjectileType::Impacting);
        assert_eq!(projectile.amount, 15);
        assert_eq!(projectile.extra_dmg, 80);
    }

    #[test]
    fn test_world_boss_catapult_parse() {
        // Inactive catapult
        let inactive_raw = "0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0";
        let inactive_data = Vals::from(inactive_raw);
        assert!(
            WorldBossCatapult::parse(&inactive_data, ServerTime::default())
                .unwrap()
                .is_none()
        );
        inactive_data.assert_all_read();

        // Active catapult with one upgrade
        let active_raw = "1784829600/1/1/2/700/0/0/0/0/0/0/0/0/0/0/0/0";
        let active_data = Vals::from(active_raw);
        let catapult =
            WorldBossCatapult::parse(&active_data, ServerTime::default())
                .unwrap()
                .unwrap();
        active_data.assert_all_read();

        let upgrade = catapult.upgrades[0].as_ref().unwrap();
        assert_eq!(upgrade.typ, WorldBossCatapultUpgradeType::Motor);
        assert_eq!(upgrade.restriction, Some(WorldBossTowerSegment::Top));
        assert_eq!(upgrade.amount, 2);
        assert_eq!(upgrade.effect_val, 700);

        assert!(catapult.upgrades[1].is_none());
        assert!(catapult.upgrades[2].is_none());
        assert!(catapult.upgrades[3].is_none());
    }

    #[test]
    fn test_world_boss_battle_parse() {
        let mut parts = vec![
            "1".to_string(), // boss_nr
            "40".into(),     // unknown
            "2".into(),      // weak_point (Middle)
            "1".into(),      // weak_point_hit (true)
        ];

        // 3 segments, each 34 values
        for seg_idx in 0..3 {
            parts.push("10".into()); // signed_up_heroes
            parts.push("100000".into()); // joint_damage
            parts.push("500000".into()); // max_health
            parts.push("400000".into()); // remaining_health
            parts.push("2".into()); // active_special_attacks

            // 10 top damage heroes (level, name)
            for h in 0..10 {
                parts.push(format!("{}", 100 + h));
                parts.push(format!("Hero{seg_idx}_{h}"));
            }

            // 3 special atk heroes (level, name, count)
            for s in 0..3 {
                parts.push(format!("{}", 200 + s));
                parts.push(format!("Special{seg_idx}_{s}"));
                parts.push(format!("{}", 5 + s));
            }
        }

        assert_eq!(parts.len(), 106);
        let raw = parts.join("/");
        let data = Vals::from(raw.as_str());
        let battle = WorldBossBattle::parse(&data);
        data.assert_all_read();

        assert_eq!(battle.boss_nr, 1);
        assert_eq!(battle.unknown, 40);
        assert_eq!(battle.weak_point, Some(WorldBossTowerSegment::Middle));
        assert!(battle.weak_point_hit);

        let top_seg = &battle.segments[WorldBossTowerSegment::Top];
        assert_eq!(top_seg.signed_up_heroes, 10);
        assert_eq!(top_seg.joint_damage, 100_000);
        assert_eq!(top_seg.max_health, 500_000);
        assert_eq!(top_seg.remaining_health, 400_000);
        assert_eq!(top_seg.active_special_attacks, 2);
        assert_eq!(top_seg.most_damage_heroes.len(), 10);
        assert_eq!(top_seg.most_damage_heroes[0].level, 100);
        assert_eq!(&*top_seg.most_damage_heroes[0].name, "Hero0_0");
        assert_eq!(top_seg.most_special_atk_heroes.len(), 3);
        assert_eq!(top_seg.most_special_atk_heroes[0].level, 200);
        assert_eq!(&*top_seg.most_special_atk_heroes[0].name, "Special0_0");
        assert_eq!(top_seg.most_special_atk_heroes[0].count, 5);
    }
}
