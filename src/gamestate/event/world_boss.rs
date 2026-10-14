#![allow(clippy::module_name_repetitions)]

use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    gamestate::{Result, ServerTime, rewards::RewardType},
    response::Vals,
};

/// The segment / section of the world boss tower
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
    /// Top section of the tower
    #[default]
    Top = 1,
    /// Middle section of the tower
    Middle,
    /// Bottom section of the tower
    Bottom,
}

/// Information about a world boss event
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossEvent {
    /// The number of catalysts available
    pub catalysts: u32,
    /// The number of medals earned
    pub medals: u32,
    /// The player's rank in the event
    pub rank: u32,
    /// The current tower segment being attacked
    pub current_segment: Option<WorldBossTowerSegment>,
    /// The amount of battle loot chests we have available to collect
    pub battle_reward_chests: u32,
    /// The time until the next attack
    pub attack_timer: Option<DateTime<Local>>,
    /// The next time the battle loot will be issued
    pub next_battle_loot_issued: Option<DateTime<Local>>,
    /// The amount of damage we do per hour
    pub damage_per_hour: u32,
    /// The amount of damage a single attack does
    pub damage_per_attack: u32,
    /// The damage comparative players do
    pub damage_comparative: u32,
    /// The amount of loot we get per hour
    pub loot_per_hour: u32,
    /// Information about the currently ongoing fight
    pub battle: Option<WorldBossBattle>,

    /// The projectiles available in the shop.
    /// Must have called `Command::DungeonEnter` to have this available
    pub projectile_offers: Vec<ProjectileOffer>,

    /// The catapult upgrades available in the shop.
    /// Must have called `Command::DungeonEnter` to have this available
    pub upgrade_offers: Vec<UpgradeOffer>,

    /// Information about the catapult, if available
    pub catapult: Option<WorldBossCatapult>,
    /// Information about the current projectile/ammo, if available
    pub projectile: Option<WorldBossProjectile>,

    /// The amount of ranks available on the "international" tab (#servers)
    pub max_international_ranks: Option<u32>,
    /// The amount of players available on the "damage" tab
    pub max_world_ranks: Option<u32>,
    /// Honestly not sure what this is. This seems to be a max rank, such as
    /// the others, but I can't figure out for what.
    pub max_fighter_ranks: Option<u32>,

    /// The amount of chests, that are available for automatic, daily pickup.
    /// You should pick these up before doing any interactions
    pub available_daily_chests: EnumMap<WorldBossChest, u32>,
}

/// The types of daily chests available for collection in the world boss event
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossChest {
    /// Loot chest from battles
    BattleLoot,
    /// Chest awarded for hitting weak points
    WeakPointChest,
    /// Chest awarded for defeating/kill-stealing
    KillStealChest,
    /// Chest awarded for highest damage dealt
    DamageMasterChest,
    /// Chest awarded for single hit achievements
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

/// The type of projectile / ammunition used in the world boss event
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossProjectileType {
    /// Always active
    Impacting = 1,
    /// Only active below 30% boss health
    Biting = 2,
    /// Only active above 70% boss health
    Mangling = 3,
    /// Unknown projectile type
    #[default]
    Unknown,
}

/// Information about the player's world boss projectile ammunition
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossProjectile {
    /// The type this ammunition is of
    pub typ: WorldBossProjectileType,
    /// The amount of this ammo, that we still have left
    pub amount: u32,
    /// The extra amount of damage this ammo does. The value here is a
    /// percentage, so 80 here would mean 80% more dmg
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

/// Information about an ongoing world boss battle
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossBattle {
    /// The level/stage/number of the boss in the ongoing boss battles.
    /// Basically just the number next to "WORLD BOSS" in the official UI
    pub boss_nr: u32,
    /// Honestly, no idea what this is, it was just 40 for me. This is just
    /// public in case, this turns out to be important, but is probably going
    /// to be renamed/removed in a later version
    pub unknown: i64,
    /// The segment marked as the weak point of the world boss
    pub weak_point: Option<WorldBossTowerSegment>,
    /// Have we already hit the hitpoint for the chest?
    pub weak_point_hit: bool,
    /// The three levels of the boss, that you can fight
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

/// Information about one segment of the world boss tower
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossTowerSegmentInfo {
    /// The amount of heroes, that are currently fighting on this level of the
    /// tower
    pub signed_up_heroes: u32,
    /// The amount of damage all heroes did combined
    pub joint_damage: u64,
    /// The maximum amount of health this segment of the boss has
    pub max_health: u64,
    /// The amount of health the this segment of the boss still has
    pub remaining_health: u64,
    /// The amount of special effects currently active
    pub active_special_attacks: u32,
    /// The 10 heroes, that did the most damage
    pub most_damage_heroes: Vec<WorldBossTowerHeroDamageSummary>,
    /// The 3 heroes, that did the most special attacks
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

/// Summary of damage dealt by a top hero in a world boss tower segment
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossTowerHeroDamageSummary {
    /// The name of the hero
    pub name: Box<str>,
    /// The level this hero has
    pub level: u16,
}

/// Summary of special attacks performed by a top hero in a world boss tower
/// segment
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossTowerHeroSpecialSummary {
    /// The name of the hero
    pub name: Box<str>,
    /// The level this hero has
    pub level: u16,
    /// Probably the amount of special attacks this hero has done. This is not
    /// displayed in the official UI, so I am just guessing here
    pub count: i64,
}

/// Information about the player's world boss catapult
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossCatapult {
    /// The time at which this catapult is going to break
    pub breaks: DateTime<Local>,
    /// The upgrades that have been bought to upgrade the effectiveness of this
    /// catapult
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

/// An upgrade applied to the world boss catapult
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossCatapultUpgrade {
    /// The type of upgrade
    pub typ: WorldBossCatapultUpgradeType,
    /// The amount of upgrades we have of this type (0..=10)
    pub amount: u8,
    /// Is only working on this specific level of the tower
    pub restriction: Option<WorldBossTowerSegment>,
    /// The effect this upgrade has. Divide this here by 100.0 to get the
    /// percentage. So 700 here would be a 7% improvement, for whatever the
    /// upgrade type is
    pub effect_val: i64,
}

/// The type of upgrade for the world boss catapult
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossCatapultUpgradeType {
    /// Damage bonus. (Must have restriction)
    Motor = 1,
    /// Shot Speed
    Thread = 2,
    /// Extra crit hit chance
    AimingDevice = 3,
    /// Extra crit damage
    Bowl = 4,
    /// New/Unknown upgrade type
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

/// An offer to buy projectiles in the world boss shop
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProjectileOffer {
    /// The type of projectile you are buying
    pub typ: WorldBossProjectileType,
    /// The available quantity purchase options
    pub buy_options:
        EnumMap<ProjectileOfferQuantity, ProjectileOfferQuantified>,
    /// The raw effect of this projectile
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

/// An offer to upgrade the catapult in the world boss shop
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UpgradeOffer {
    /// The type of upgrade, that we are offered to buy
    pub typ: WorldBossCatapultUpgradeType,
    /// The restiction in terms of which segment this upgrade works on, if any
    pub restriction: Option<WorldBossTowerSegment>,
    /// The effect this item has
    pub effect_value: i64,
    /// The price of the upgrade, which is payed by `main_price_type` as a
    /// currency. This price is scaled down based on how much of the catapult
    /// you still have left, so if you only have 1 out of the 10 hours
    /// left, you only pay 10% of this
    pub raw_main_price: u64,
    /// The resource you have to use to buy this upgrade
    pub main_price_type: RewardType,
    /// The amount of catalysts you have to pay to buy this offer together with
    /// the main payment type. This price is scaled down based on how much of
    /// the catapult you still have left, so if you only have 1 out of the
    /// 10 hours left, you only pay 10% of this
    pub raw_catalyst_price: u64,
    /// The amount of mushrooms this upgrade would cost, if you were to pay
    /// with mushrooms. This price is scaled down based on how much of the
    /// catapult you still have left, so if you only have 1 out of the 10 hours
    /// left, you only pay 10% of this
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
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

    #[test]
    fn test_world_boss_event_update() {
        let raw = "10/20/5/1/576/1784829600/5000/3470/2029/1000/200/50/3/0/\
                   1785096000";
        let data = Vals::from(raw);
        let mut event = WorldBossEvent::default();
        event.update(&data, ServerTime::default()).unwrap();
        assert_all_read(&data);

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
        assert_all_read(&empty_data);
        assert_eq!(event.upgrade_offers, []);

        // Single offer: typ=1 (Motor), restriction=1 (Top), effect=700,
        // price=1000, type=4 (Silver), cat=50, mush=5
        let raw = "1/1/700/1000/4/50/5";
        let data = Vals::from(raw);
        event.update_upgrade_store(&data);
        assert_all_read(&data);
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
        assert_all_read(&empty_data);
        assert_eq!(event.projectile_offers, []);

        // Single offer: typ=1 (Impacting), small amount=20, large amount=50,
        // effect=80, small price=100, large price=250
        let raw = "1/20/50/80/100/250";
        let data = Vals::from(raw);
        event.update_ammo_store(&data);
        assert_all_read(&data);
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
        assert_all_read(&data);

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
        assert_all_read(&empty_data);

        let raw = "1/15/80";
        let data = Vals::from(raw);
        let projectile = WorldBossProjectile::parse(&data).unwrap();
        assert_all_read(&data);
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
        assert_all_read(&inactive_data);

        // Active catapult with one upgrade
        let active_raw = "1784829600/1/1/2/700/0/0/0/0/0/0/0/0/0/0/0/0";
        let active_data = Vals::from(active_raw);
        let catapult =
            WorldBossCatapult::parse(&active_data, ServerTime::default())
                .unwrap()
                .unwrap();
        assert_all_read(&active_data);

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
        assert_all_read(&data);

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
