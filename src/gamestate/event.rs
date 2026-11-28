use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    gamestate::{ServerTime, rewards::RewardType},
    response::Vals,
};

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

fn all_numeric_zero(data: &Vals<'_>) -> bool {
    data.values().iter().all(|value| {
        value.is_empty() || value.parse::<i64>().is_ok_and(|raw| raw == 0)
    })
}

/// Information about an upcoming, or currently running special event. This is
/// basically all the information the button above the tavern has in the
/// official GUI. There may be some overlap with information in f.e. Hellevator,
/// but that is just how it is on the server
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventStatus {
    pub typ: SpecialEventType,
    /// The time at which this event is scheduled to start at
    pub start: Option<DateTime<Local>>,
    /// The time until which the event will be active
    pub end: Option<DateTime<Local>>,
    /// If the event has some sort of phase after the main (action) part has
    /// ended, this here will mark the end to that (end of claim period, etc.)
    pub extra_end: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum SpecialEventType {
    LegendaryDungeon(LegendaryDungeonTheme),
    Hellevator,
    DrivingDungeon,
    TravelingCircus,
    WorldBoss(WorldBossTheme),
    Unknown,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum LegendaryDungeonTheme {
    DiabolicalCompanyParty = 1,
    LordOfTheThings,
    FantasticLegendaries,
    ShadyBirthdayBash,
    WinterSpectacle,
    AbyssesOfMadness,
    HuntForTheEasterEgg,
    VileVacation,
    #[default]
    Unknown,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum WorldBossTheme {
    Normal = 1,
    Winter,
    #[default]
    Unknown,
}

impl EventStatus {
    pub(crate) fn parse(data: &Vals<'_>, server_time: ServerTime) -> Self {
        let start = data.server_time_opt(2, server_time);
        let end = data.server_time_opt(3, server_time);
        let extra_end = data.server_time_opt(4, server_time);

        let typ = match data.i64_or(0, 0) {
            1 => {
                let raw = data.i64_or(1, 0);
                let theme = FromPrimitive::from_i64(raw).unwrap_or_else(|| {
                    if !matches!(raw, 0 | -1) {
                        warn!(
                            "There might be a new legendary dungeon theme -> \
                             {raw}"
                        );
                    }
                    LegendaryDungeonTheme::default()
                });
                SpecialEventType::LegendaryDungeon(theme)
            }
            2 => {
                data.ignore(1, "non-legendary event theme");
                SpecialEventType::Hellevator
            }
            3 => {
                data.ignore(1, "non-legendary event theme");
                SpecialEventType::DrivingDungeon
            }
            4 => {
                data.ignore(1, "non-legendary event theme");
                SpecialEventType::TravelingCircus
            }
            6 => {
                let raw = data.i64_or(1, 0);
                let theme = FromPrimitive::from_i64(raw).unwrap_or_else(|| {
                    if !matches!(raw, 0 | -1) {
                        warn!("There might be a new world boss theme -> {raw}");
                    }
                    WorldBossTheme::default()
                });
                SpecialEventType::WorldBoss(theme)
            }
            _ => {
                data.ignore(1, "unknown event theme");
                SpecialEventType::Unknown
            }
        };

        data.ignore(5..data.len(), "extra event status values");
        Self {
            typ,
            start,
            end,
            extra_end,
        }
    }
}

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
#[allow(missing_docs)]
pub enum WorldBossTowerSegment {
    #[default]
    Top = 1,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossEvent {
    pub catalysts: u32,
    pub medals: u32,
    pub rank: u32,
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

    pub catapult: Option<WorldBossCatapult>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossChest {
    BattleLoot,
    WeakPointChest,
    KillStealChest,
    DamageMasterChest,
    SingleHitChest,
}

impl WorldBossEvent {
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        self.catalysts = data.u32_or(0, 0);
        self.medals = data.u32_or(1, 0);
        self.rank = data.u32_or(2, 0);
        self.current_segment = parse_enum_opt(data.i64_or(3, 0), "wb level");
        data.ignore(4, "unknown world boss character field, observed 576");

        self.attack_timer = data.server_time_opt(5, server_time);
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
        self.next_battle_loot_issued = data.server_time_opt(14, server_time);
        data.ignore(15..data.len(), "extra world boss character values");
    }

    pub(crate) fn update_upgrade_store(&mut self, data: &Vals<'_>) {
        if all_numeric_zero(data) {
            data.ignore_all("empty upgrade store");
            self.upgrade_offers.clear();
            return;
        }

        let complete_len = data.len() / 7 * 7;
        self.upgrade_offers = data
            .window(0..complete_len)
            .chunks(7)
            .map(|chunk| UpgradeOffer::parse(&chunk))
            .collect();
        data.ignore(complete_len..data.len(), "partial upgrade offer");
    }

    pub(crate) fn update_ammo_store(&mut self, data: &Vals<'_>) {
        if all_numeric_zero(data) {
            data.ignore_all("empty ammo store");
            self.projectile_offers.clear();
            return;
        }

        let complete_len = data.len() / 6 * 6;
        self.projectile_offers = data
            .window(0..complete_len)
            .chunks(6)
            .map(|chunk| ProjectileOffer::parse(&chunk))
            .collect();
        data.ignore(complete_len..data.len(), "partial projectile offer");
    }

    pub(crate) fn update_daily_chests(&mut self, data: &Vals<'_>) {
        for (pos, chest) in self.available_daily_chests.values_mut().enumerate()
        {
            *chest = data.u32_or(pos, 0);
        }
        data.ignore(5..data.len(), "extra world boss daily chest values");
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Default, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldBossProjectileType {
    // Always active
    Impacting = 1,
    /// Only active below 30% boss health
    Biting = 2,
    /// Only active above 70% boss health
    Mangling = 3,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default)]
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
    pub(crate) fn parse(data: &Vals<'_>) -> Option<Self> {
        if all_numeric_zero(data) {
            data.ignore_all("empty projectile data");
            return None;
        }
        data.ignore(3..data.len(), "extra world boss projectile values");
        Some(Self {
            typ: parse_enum_or(data.i64_or(0, 0), "wb ammo typ"),
            amount: data.u32_or(1, 0),
            extra_dmg: data.u32_or(2, 0),
        })
    }
}

#[derive(Debug, Clone)]
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
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
        let boss_nr = data.u32_or(0, 0);
        let unknown = data.i64_or(1, 0);
        let weak_point = parse_enum_opt(data.i64_or(2, 0), "wb weak point");
        let weak_point_hit = data.u32_or(3, 0) == 1;
        let mut segments = EnumMap::default();

        for segment in WorldBossTowerSegment::iter() {
            let start = (segment as usize - 1) * 34 + 4;
            segments[segment] = WorldBossTowerSegmentInfo::parse(
                &data.window(start..start + 34),
            );
        }
        data.ignore(106..data.len(), "extra world boss tower values");

        Self {
            boss_nr,
            unknown,
            weak_point,
            weak_point_hit,
            segments,
        }
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossTowerSegmentInfo {
    /// The amount of heroes, that are currently fighting on this level of the
    pub signed_up_heroes: u32,
    //// The amount of damage all heroes did combined (I honestly don't know )
    pub joint_damage: u64,
    /// The maximum amount of health this segment of the boss has
    pub max_health: u64,
    /// The amount of health the this segment of the boss still has
    pub remaining_health: u64,
    /// The amount of special effects currently acive
    pub active_special_attacks: u32,
    /// the 10 heroes, that did the most damage
    pub most_damage_heroes: Vec<WorldBossTowerHeroDamageSummary>,
    /// the 3 heroes, that did the most special attacks
    pub most_special_atk_heroes: Vec<WorldBossTowerHeroSpecialSummary>,
}

impl WorldBossTowerSegmentInfo {
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossTowerHeroDamageSummary {
    /// The name of the hero
    pub name: Box<str>,
    /// The level this hero has
    pub level: u16,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldBossCatapult {
    /// The time at which this catapult is going to break
    pub breaks: DateTime<Local>,
    /// The upgrades that have been bought to upgrade the effectiveness of this
    /// catapult
    pub upgrades: [Option<WorldBossCatapultUpgrade>; 4],
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash, Enum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProjectileOfferQuantity {
    Small = 1,
    Large,
}

impl WorldBossCatapult {
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Option<Self> {
        let Some(breaks) = data.server_time_opt(0, server_time) else {
            data.ignore(1..data.len(), "catapult upgrades when inactive");
            return None;
        };

        let mut upgrades: [Option<WorldBossCatapultUpgrade>; 4] =
            Default::default();
        let upgrade_data = data.window(1..17);
        let complete_len = upgrade_data.len() / 4 * 4;
        for (chunk, upgrade) in upgrade_data
            .window(0..complete_len)
            .chunks(4)
            .zip(&mut upgrades)
        {
            if all_numeric_zero(&chunk) {
                chunk.ignore_all("empty catapult upgrade");
                continue;
            }
            *upgrade = Some(WorldBossCatapultUpgrade {
                typ: parse_enum_or(chunk.i64_or(0, 0), "wb upgrade type"),
                restriction: parse_enum_opt(
                    chunk.i64_or(1, 0),
                    "wb upgrade restrict",
                ),
                amount: chunk.u8_or(2, 0),
                effect_val: chunk.i64_or(3, 0),
            });
        }
        upgrade_data.ignore(
            complete_len..upgrade_data.len(),
            "partial catapult upgrade",
        );
        data.ignore(17..data.len(), "extra world boss catapult values");

        Some(Self { breaks, upgrades })
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProjectileOfferQuantified {
    pub amount: u32,
    pub price: u32,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProjectileOffer {
    /// The type of projectile you are buying
    pub typ: WorldBossProjectileType,

    pub buy_options:
        EnumMap<ProjectileOfferQuantity, ProjectileOfferQuantified>,

    /// The raw effect of this projectile
    pub effect_val: i64,
}

impl ProjectileOffer {
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
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
            typ: parse_enum_or(data.i64_or(0, 0), "wb po typ"),
            buy_options,
            effect_val: data.i64_or(3, 0),
        }
    }
}

#[derive(Debug, Clone)]
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
    pub(crate) fn parse(data: &Vals<'_>) -> Self {
        Self {
            typ: parse_enum_or(data.i64_or(0, 0), "wb upgrade typ"),
            restriction: parse_enum_opt(data.i64_or(1, 0), "wb ug restrict"),
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
    #![allow(clippy::indexing_slicing, clippy::unwrap_used)]

    use super::*;

    const WB_CHARACTER: &str =
        "10/20/5/1/576/1784829600/5000/3470/2029/1000/200/50/3/0/1784829600";
    const UPGRADE_OFFER: &str = "1/1/700/1000/4/50/5";
    const AMMO_OFFER: &str = "1/20/50/80/100/250";
    const CATAPULT_BREAKS: i64 = 1_784_829_600;

    #[test]
    fn parses_legendary_event_times_and_ignores_extra_values() {
        let server_time = ServerTime(3_600);
        let data = Vals::from("1/5/1700000000/1700000100/0/extra/malformed");
        let status = EventStatus::parse(&data, server_time);
        data.assert_all_read();

        assert_eq!(
            status.typ,
            SpecialEventType::LegendaryDungeon(
                LegendaryDungeonTheme::WinterSpectacle
            )
        );
        assert_eq!(
            status.start,
            server_time.convert_to_local(1_700_000_000, "event start")
        );
        assert_eq!(
            status.end,
            server_time.convert_to_local(1_700_000_100, "event end")
        );
        assert_eq!(status.extra_end, None);
    }

    #[test]
    fn parses_world_boss_theme() {
        let data = Vals::from("6/2/0/0/0");
        let status = EventStatus::parse(&data, ServerTime::default());
        data.assert_all_read();

        assert_eq!(
            status.typ,
            SpecialEventType::WorldBoss(WorldBossTheme::Winter)
        );
    }

    #[test]
    fn ignores_themes_for_non_theme_events_and_unknown_types() {
        for (raw, expected) in [
            ("2/malformed/0/0/0/extra", SpecialEventType::Hellevator),
            (
                "3/not-a-theme/0/0/0/extra",
                SpecialEventType::DrivingDungeon,
            ),
            ("4/99/0/0/0/extra", SpecialEventType::TravelingCircus),
            ("99/not-a-theme/0/0/0/extra", SpecialEventType::Unknown),
            (
                "malformed/not-a-theme/malformed/malformed/malformed/extra",
                SpecialEventType::Unknown,
            ),
        ] {
            let data = Vals::from(raw);
            let status = EventStatus::parse(&data, ServerTime::default());
            data.assert_all_read();

            assert_eq!(status.typ, expected, "{raw}");
            assert_eq!(status.start, None, "{raw}");
            assert_eq!(status.end, None, "{raw}");
            assert_eq!(status.extra_end, None, "{raw}");
        }
    }

    #[test]
    fn unknown_or_malformed_event_themes_use_defaults() {
        for (raw, expected) in [
            (
                "1/99/0/0/0",
                SpecialEventType::LegendaryDungeon(
                    LegendaryDungeonTheme::Unknown,
                ),
            ),
            (
                "1/malformed/0/0/0",
                SpecialEventType::LegendaryDungeon(
                    LegendaryDungeonTheme::Unknown,
                ),
            ),
            (
                "6/99/0/0/0",
                SpecialEventType::WorldBoss(WorldBossTheme::Unknown),
            ),
            (
                "6/malformed/0/0/0",
                SpecialEventType::WorldBoss(WorldBossTheme::Unknown),
            ),
        ] {
            let data = Vals::from(raw);
            let status = EventStatus::parse(&data, ServerTime::default());
            data.assert_all_read();
            assert_eq!(status.typ, expected, "{raw}");
        }
    }

    #[test]
    fn numeric_zero_sentinel_accepts_empty_and_alternate_spellings() {
        for raw in ["", "0/0", "+0/-0/00", "0//0"] {
            let data = Vals::from(raw);
            assert!(all_numeric_zero(&data), "{raw:?}");
            assert_eq!(
                data.unaccessed_indices(),
                (0..data.len()).collect::<Vec<_>>()
            );
            data.ignore_all("test numeric zero sentinel");
            data.assert_all_read();
        }

        let data = Vals::from("0/malformed");
        assert!(!all_numeric_zero(&data));
        assert_eq!(data.unaccessed_indices(), vec![0, 1]);
        data.ignore_all("test non-zero sentinel");
        data.assert_all_read();
    }

    #[test]
    fn updates_full_world_boss_character_and_ignores_extra_values() {
        let server_time = ServerTime::default();
        let raw = format!("{WB_CHARACTER}/extra/malformed");
        let data = Vals::from(raw.as_str());
        let mut event = WorldBossEvent::default();
        event.update(&data, server_time);
        data.assert_all_read();

        assert_eq!(event.catalysts, 10);
        assert_eq!(event.medals, 20);
        assert_eq!(event.rank, 5);
        assert_eq!(event.current_segment, Some(WorldBossTowerSegment::Top));
        assert_eq!(
            event.attack_timer,
            server_time.convert_to_local(1_784_829_600, "attack timer")
        );
        assert_eq!(event.damage_per_attack, 5_000);
        assert_eq!(event.damage_per_hour, 1_000);
        assert_eq!(event.loot_per_hour, 200);
        assert_eq!(event.damage_comparative, 50);
        assert_eq!(event.battle_reward_chests, 3);
        assert_eq!(
            event.next_battle_loot_issued,
            server_time.convert_to_local(1_784_829_600, "next battle loot")
        );
    }

    #[test]
    fn malformed_world_boss_character_defaults_and_clears_stale_timers() {
        let server_time = ServerTime::default();
        let mut event = WorldBossEvent::default();
        let valid = Vals::from(WB_CHARACTER);
        event.update(&valid, server_time);
        valid.assert_all_read();
        assert!(event.attack_timer.is_some());
        assert!(event.next_battle_loot_issued.is_some());

        let data = Vals::from(
            "malformed/malformed/malformed/99/ignored/malformed/malformed/\
             ignored/ignored/malformed/malformed/malformed/malformed/ignored/\
             malformed/extra",
        );
        event.update(&data, server_time);
        data.assert_all_read();

        assert_eq!(event.catalysts, 0);
        assert_eq!(event.medals, 0);
        assert_eq!(event.rank, 0);
        assert_eq!(event.current_segment, None);
        assert_eq!(event.attack_timer, None);
        assert_eq!(event.damage_per_attack, 0);
        assert_eq!(event.damage_per_hour, 0);
        assert_eq!(event.loot_per_hour, 0);
        assert_eq!(event.damage_comparative, 0);
        assert_eq!(event.battle_reward_chests, 0);
        assert_eq!(event.next_battle_loot_issued, None);
    }

    #[test]
    fn truncated_world_boss_character_defaults_missing_fields() {
        let server_time = ServerTime::default();
        let mut event = WorldBossEvent::default();
        let valid = Vals::from(WB_CHARACTER);
        event.update(&valid, server_time);
        valid.assert_all_read();

        let data = Vals::from("7/8/9/2");
        event.update(&data, server_time);
        data.assert_all_read();

        assert_eq!(event.catalysts, 7);
        assert_eq!(event.medals, 8);
        assert_eq!(event.rank, 9);
        assert_eq!(event.current_segment, Some(WorldBossTowerSegment::Middle));
        assert_eq!(event.attack_timer, None);
        assert_eq!(event.damage_per_attack, 0);
        assert_eq!(event.damage_per_hour, 0);
        assert_eq!(event.loot_per_hour, 0);
        assert_eq!(event.damage_comparative, 0);
        assert_eq!(event.battle_reward_chests, 0);
        assert_eq!(event.next_battle_loot_issued, None);
    }

    #[test]
    fn upgrade_store_handles_empty_zero_and_partial_data() {
        let mut event = WorldBossEvent::default();
        let valid = Vals::from(UPGRADE_OFFER);
        event.update_upgrade_store(&valid);
        valid.assert_all_read();
        assert_eq!(event.upgrade_offers.len(), 1);

        let empty = Vals::from("");
        event.update_upgrade_store(&empty);
        empty.assert_all_read();
        assert!(event.upgrade_offers.is_empty());

        event.update_upgrade_store(&valid);
        valid.assert_all_read();
        let zeros = Vals::from("+0/-0/00/0/0/0/0");
        event.update_upgrade_store(&zeros);
        zeros.assert_all_read();
        assert!(event.upgrade_offers.is_empty());

        let raw = format!(
            "{UPGRADE_OFFER}/0/0/0/0/0/0/0/2/-1/900/2000/3/75/10/1/2/3"
        );
        let data = Vals::from(raw.as_str());
        event.update_upgrade_store(&data);
        data.assert_all_read();

        assert_eq!(event.upgrade_offers.len(), 3);
        let first = &event.upgrade_offers[0];
        assert_eq!(first.typ, WorldBossCatapultUpgradeType::Motor);
        assert_eq!(first.restriction, Some(WorldBossTowerSegment::Top));
        assert_eq!(first.effect_value, 700);
        assert_eq!(first.raw_main_price, 1_000);
        assert_eq!(first.main_price_type, RewardType::Silver);
        assert_eq!(first.raw_catalyst_price, 50);
        assert_eq!(first.raw_mushroom_price, 5);

        let zero_offer = &event.upgrade_offers[1];
        assert_eq!(zero_offer.typ, WorldBossCatapultUpgradeType::Unknown);
        assert_eq!(zero_offer.restriction, None);
        assert_eq!(zero_offer.effect_value, 0);
        assert_eq!(zero_offer.raw_main_price, 0);
        assert_eq!(zero_offer.main_price_type, RewardType::Unknown);
        assert_eq!(zero_offer.raw_catalyst_price, 0);
        assert_eq!(zero_offer.raw_mushroom_price, 0);

        let last = &event.upgrade_offers[2];
        assert_eq!(last.typ, WorldBossCatapultUpgradeType::Thread);
        assert_eq!(last.restriction, None);
        assert_eq!(last.effect_value, 900);
        assert_eq!(last.raw_main_price, 2_000);
        assert_eq!(last.main_price_type, RewardType::Mushrooms);
        assert_eq!(last.raw_catalyst_price, 75);
        assert_eq!(last.raw_mushroom_price, 10);
    }

    #[test]
    fn malformed_upgrade_offer_uses_per_field_defaults() {
        let data = Vals::from("99/99/malformed/-1/malformed");
        let offer = UpgradeOffer::parse(&data);
        data.assert_all_read();

        assert_eq!(offer.typ, WorldBossCatapultUpgradeType::Unknown);
        assert_eq!(offer.restriction, None);
        assert_eq!(offer.effect_value, 0);
        assert_eq!(offer.raw_main_price, u64::MAX);
        assert_eq!(offer.main_price_type, RewardType::Unknown);
        assert_eq!(offer.raw_catalyst_price, u64::MAX);
        assert_eq!(offer.raw_mushroom_price, u64::MAX);
    }

    #[test]
    fn ammo_store_handles_empty_zero_and_partial_data() {
        let mut event = WorldBossEvent::default();
        let valid = Vals::from(AMMO_OFFER);
        event.update_ammo_store(&valid);
        valid.assert_all_read();
        assert_eq!(event.projectile_offers.len(), 1);

        let empty = Vals::from("");
        event.update_ammo_store(&empty);
        empty.assert_all_read();
        assert!(event.projectile_offers.is_empty());

        event.update_ammo_store(&valid);
        valid.assert_all_read();
        let zeros = Vals::from("+0/-0/00/0/0/0");
        event.update_ammo_store(&zeros);
        zeros.assert_all_read();
        assert!(event.projectile_offers.is_empty());

        let raw = format!("{AMMO_OFFER}/0/0/0/0/0/0/2/10/25/40/50/125/1/2/3");
        let data = Vals::from(raw.as_str());
        event.update_ammo_store(&data);
        data.assert_all_read();

        assert_eq!(event.projectile_offers.len(), 3);
        let first = &event.projectile_offers[0];
        assert_eq!(first.typ, WorldBossProjectileType::Impacting);
        assert_eq!(
            first.buy_options[ProjectileOfferQuantity::Small].amount,
            20
        );
        assert_eq!(
            first.buy_options[ProjectileOfferQuantity::Small].price,
            100
        );
        assert_eq!(
            first.buy_options[ProjectileOfferQuantity::Large].amount,
            50
        );
        assert_eq!(
            first.buy_options[ProjectileOfferQuantity::Large].price,
            250
        );
        assert_eq!(first.effect_val, 80);

        let zero_offer = &event.projectile_offers[1];
        assert_eq!(zero_offer.typ, WorldBossProjectileType::Unknown);
        assert_eq!(
            zero_offer.buy_options[ProjectileOfferQuantity::Small].amount,
            0
        );
        assert_eq!(
            zero_offer.buy_options[ProjectileOfferQuantity::Small].price,
            0
        );
        assert_eq!(
            zero_offer.buy_options[ProjectileOfferQuantity::Large].amount,
            0
        );
        assert_eq!(
            zero_offer.buy_options[ProjectileOfferQuantity::Large].price,
            0
        );
        assert_eq!(zero_offer.effect_val, 0);

        let last = &event.projectile_offers[2];
        assert_eq!(last.typ, WorldBossProjectileType::Biting);
        assert_eq!(last.effect_val, 40);
        assert_eq!(last.buy_options[ProjectileOfferQuantity::Large].price, 125);
    }

    #[test]
    fn projectile_offers_parse_valid_and_malformed_values() {
        let valid = Vals::from("3/15/30/80/90/200");
        let offer = ProjectileOffer::parse(&valid);
        valid.assert_all_read();
        assert_eq!(offer.typ, WorldBossProjectileType::Mangling);
        assert_eq!(
            offer.buy_options[ProjectileOfferQuantity::Small].amount,
            15
        );
        assert_eq!(offer.buy_options[ProjectileOfferQuantity::Small].price, 90);
        assert_eq!(
            offer.buy_options[ProjectileOfferQuantity::Large].amount,
            30
        );
        assert_eq!(
            offer.buy_options[ProjectileOfferQuantity::Large].price,
            200
        );
        assert_eq!(offer.effect_val, 80);

        let malformed = Vals::from("99/malformed/-1/malformed/-2/not-a-number");
        let offer = ProjectileOffer::parse(&malformed);
        malformed.assert_all_read();
        assert_eq!(offer.typ, WorldBossProjectileType::Unknown);
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
        assert_eq!(offer.effect_val, 0);
    }

    #[test]
    fn projectile_handles_zero_valid_and_malformed_data() {
        for raw in ["", "0/0/0", "+0/-0/00"] {
            let data = Vals::from(raw);
            assert!(WorldBossProjectile::parse(&data).is_none(), "{raw:?}");
            data.assert_all_read();
        }

        let valid = Vals::from("2/15/80/extra/malformed");
        let projectile = WorldBossProjectile::parse(&valid).unwrap();
        valid.assert_all_read();
        assert_eq!(projectile.typ, WorldBossProjectileType::Biting);
        assert_eq!(projectile.amount, 15);
        assert_eq!(projectile.extra_dmg, 80);

        let malformed = Vals::from("99/malformed/not-a-number");
        let projectile = WorldBossProjectile::parse(&malformed).unwrap();
        malformed.assert_all_read();
        assert_eq!(projectile.typ, WorldBossProjectileType::Unknown);
        assert_eq!(projectile.amount, 0);
        assert_eq!(projectile.extra_dmg, 0);

        let truncated = Vals::from("1");
        let projectile = WorldBossProjectile::parse(&truncated).unwrap();
        truncated.assert_all_read();
        assert_eq!(projectile.typ, WorldBossProjectileType::Impacting);
        assert_eq!(projectile.amount, 0);
        assert_eq!(projectile.extra_dmg, 0);
    }

    #[test]
    fn catapult_handles_inactive_active_zero_and_partial_data() {
        let server_time = ServerTime::default();
        let inactive =
            Vals::from("0/1/2/3/4/5/6/7/8/9/10/11/12/13/14/15/16/17/18/extra");
        assert!(WorldBossCatapult::parse(&inactive, server_time).is_none());
        inactive.assert_all_read();

        let malformed = Vals::from("malformed/1/2/3");
        assert!(WorldBossCatapult::parse(&malformed, server_time).is_none());
        malformed.assert_all_read();

        let active_raw = format!(
            "{CATAPULT_BREAKS}/1/1/2/700/0/0/0/0/3/-1/4/450/0/0/0/0/extra/\
             malformed"
        );
        let active = Vals::from(active_raw.as_str());
        let catapult = WorldBossCatapult::parse(&active, server_time).unwrap();
        active.assert_all_read();
        assert_eq!(
            catapult.breaks,
            server_time
                .convert_to_local(CATAPULT_BREAKS, "catapult breaks")
                .unwrap()
        );
        let motor = catapult.upgrades[0].as_ref().unwrap();
        assert_eq!(motor.typ, WorldBossCatapultUpgradeType::Motor);
        assert_eq!(motor.restriction, Some(WorldBossTowerSegment::Top));
        assert_eq!(motor.amount, 2);
        assert_eq!(motor.effect_val, 700);
        assert!(catapult.upgrades[1].is_none());
        let aiming = catapult.upgrades[2].as_ref().unwrap();
        assert_eq!(aiming.typ, WorldBossCatapultUpgradeType::AimingDevice);
        assert_eq!(aiming.restriction, None);
        assert_eq!(aiming.amount, 4);
        assert_eq!(aiming.effect_val, 450);
        assert!(catapult.upgrades[3].is_none());

        let partial_raw =
            format!("{CATAPULT_BREAKS}/2/1/3/800/0/0/0/0/2/2/extra");
        let partial = Vals::from(partial_raw.as_str());
        let catapult = WorldBossCatapult::parse(&partial, server_time).unwrap();
        partial.assert_all_read();
        let thread = catapult.upgrades[0].as_ref().unwrap();
        assert_eq!(thread.typ, WorldBossCatapultUpgradeType::Thread);
        assert_eq!(thread.restriction, Some(WorldBossTowerSegment::Top));
        assert_eq!(thread.amount, 3);
        assert_eq!(thread.effect_val, 800);
        assert!(catapult.upgrades[1].is_none());
        assert!(catapult.upgrades[2].is_none());
        assert!(catapult.upgrades[3].is_none());
    }

    #[test]
    fn daily_chests_update_all_entries_and_reset_truncated_data() {
        let mut event = WorldBossEvent::default();
        let full = Vals::from("1/2/3/4/5/extra/malformed");
        event.update_daily_chests(&full);
        full.assert_all_read();
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

        let truncated = Vals::from("9/malformed");
        event.update_daily_chests(&truncated);
        truncated.assert_all_read();
        assert_eq!(event.available_daily_chests[WorldBossChest::BattleLoot], 9);
        assert_eq!(
            event.available_daily_chests[WorldBossChest::WeakPointChest],
            0
        );
        assert_eq!(
            event.available_daily_chests[WorldBossChest::KillStealChest],
            0
        );
        assert_eq!(
            event.available_daily_chests[WorldBossChest::DamageMasterChest],
            0
        );
        assert_eq!(
            event.available_daily_chests[WorldBossChest::SingleHitChest],
            0
        );

        let empty = Vals::from("");
        event.update_daily_chests(&empty);
        empty.assert_all_read();
        assert!(
            event
                .available_daily_chests
                .values()
                .all(|value| *value == 0)
        );
    }

    #[test]
    fn parses_full_world_boss_tower_fixture() {
        let mut parts = vec![
            "1".to_string(), // boss_nr
            "40".into(),     // unknown
            "2".into(),      // weak_point (Middle)
            "1".into(),      // weak_point_hit (true)
        ];

        for seg_idx in 0..3 {
            parts.push("10".into());
            parts.push("100000".into());
            parts.push("500000".into());
            parts.push("400000".into());
            parts.push("2".into());

            for hero in 0..10 {
                parts.push(format!("{}", 100 + hero));
                parts.push(format!("Hero{seg_idx}_{hero}"));
            }
            for special in 0..3 {
                parts.push(format!("{}", 200 + special));
                parts.push(format!("Special{seg_idx}_{special}"));
                parts.push(format!("{}", 5 + special));
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
        for segment in WorldBossTowerSegment::iter() {
            let info = &battle.segments[segment];
            assert_eq!(info.most_damage_heroes.len(), 10);
            assert_eq!(info.most_special_atk_heroes.len(), 3);
        }

        let top = &battle.segments[WorldBossTowerSegment::Top];
        assert_eq!(top.signed_up_heroes, 10);
        assert_eq!(top.joint_damage, 100_000);
        assert_eq!(top.max_health, 500_000);
        assert_eq!(top.remaining_health, 400_000);
        assert_eq!(top.active_special_attacks, 2);
        assert_eq!(top.most_damage_heroes[0].level, 100);
        assert_eq!(&*top.most_damage_heroes[0].name, "Hero0_0");
        assert_eq!(top.most_special_atk_heroes[0].level, 200);
        assert_eq!(&*top.most_special_atk_heroes[0].name, "Special0_0");
        assert_eq!(top.most_special_atk_heroes[0].count, 5);
        assert_eq!(
            &*battle.segments[WorldBossTowerSegment::Middle].most_damage_heroes
                [0]
            .name,
            "Hero1_0"
        );
        assert_eq!(
            &*battle.segments[WorldBossTowerSegment::Bottom]
                .most_special_atk_heroes[2]
                .name,
            "Special2_2"
        );

        let with_tail = format!("{raw}/extra/malformed");
        let data = Vals::from(with_tail.as_str());
        let battle = WorldBossBattle::parse(&data);
        data.assert_all_read();
        assert_eq!(battle.unknown, 40);
    }

    #[test]
    fn truncated_world_boss_tower_preserves_summary_lengths() {
        let data = Vals::from(
            "1/40/2/1/7/123/456/789/2/12/PartialHero/13/OtherHero/extra",
        );
        let battle = WorldBossBattle::parse(&data);
        data.assert_all_read();

        assert_eq!(battle.boss_nr, 1);
        assert_eq!(battle.unknown, 40);
        assert_eq!(battle.weak_point, Some(WorldBossTowerSegment::Middle));
        assert!(battle.weak_point_hit);
        for segment in WorldBossTowerSegment::iter() {
            let info = &battle.segments[segment];
            assert_eq!(info.most_damage_heroes.len(), 10);
            assert_eq!(info.most_special_atk_heroes.len(), 3);
        }

        let top = &battle.segments[WorldBossTowerSegment::Top];
        assert_eq!(top.signed_up_heroes, 7);
        assert_eq!(top.joint_damage, 123);
        assert_eq!(top.max_health, 456);
        assert_eq!(top.remaining_health, 789);
        assert_eq!(top.active_special_attacks, 2);
        assert_eq!(top.most_damage_heroes[0].level, 12);
        assert_eq!(&*top.most_damage_heroes[0].name, "PartialHero");
        assert_eq!(top.most_damage_heroes[1].level, 13);
        assert_eq!(&*top.most_damage_heroes[1].name, "OtherHero");
        assert_eq!(top.most_damage_heroes[2].level, 0);
        assert_eq!(&*top.most_damage_heroes[2].name, "");
        assert!(top.most_special_atk_heroes.iter().all(|summary| {
            summary.level == 0 && summary.name.is_empty() && summary.count == 0
        }));

        let middle = &battle.segments[WorldBossTowerSegment::Middle];
        assert_eq!(middle.signed_up_heroes, 0);
        assert_eq!(middle.joint_damage, 0);
        assert_eq!(middle.max_health, 0);
        assert_eq!(middle.remaining_health, 0);
        assert_eq!(middle.active_special_attacks, 0);
        assert!(
            middle.most_damage_heroes.iter().all(|summary| {
                summary.level == 0 && summary.name.is_empty()
            })
        );
        assert!(middle.most_special_atk_heroes.iter().all(|summary| {
            summary.level == 0 && summary.name.is_empty() && summary.count == 0
        }));
    }
}
