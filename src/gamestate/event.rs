use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    error::SFError,
    gamestate::{ServerTime, rewards::RewardType},
    misc::{ArrSkip, CCGet, CFPGet, CGet, CSGet, CSTGet},
};

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
    pub(crate) fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Self, SFError> {
        let start = data.cstget(2, "event start", server_time)?;
        let end = data.cstget(3, "event end", server_time)?;
        let extra_end = data.cstget(4, "event claim end", server_time)?;

        let typ = match data.cget(0, "event main type")? {
            1 => SpecialEventType::LegendaryDungeon(
                data.cfpget(1, "legendary dungeon theme", |a| a)?
                    .unwrap_or_default(),
            ),
            2 => SpecialEventType::Hellevator,
            3 => SpecialEventType::DrivingDungeon,
            4 => SpecialEventType::TravelingCircus,
            6 => SpecialEventType::WorldBoss(
                data.cfpget(1, "world boss theme", |a| a)?
                    .unwrap_or_default(),
            ),
            _ => SpecialEventType::Unknown,
        };

        Ok(Self {
            typ,
            start,
            end,
            extra_end,
        })
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
    pub current_segment: WorldBossTowerSegment,
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
    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.catalysts = data.csiget(0, "wb catalysts", 0)?;
        self.medals = data.csiget(1, "wb medals", 0)?;
        self.rank = data.csiget(2, "wb rank", 0)?;
        self.current_segment = data.cfpuget(3, "wb level", |a| a)?;
        // 04 => ???       // 576

        self.attack_timer = data.cstget(5, "wb atk timer", server_time)?;
        self.damage_per_attack = data.csiget(6, "wb dmg atk", 0)?;
        // 07 => ???       // 3470
        // 08 => ???       // 2029
        self.damage_per_hour = data.csiget(9, "wb dmg per hour", 0)?;
        self.loot_per_hour = data.csiget(10, "wb loot per hour", 0)?;
        self.damage_comparative = data.csiget(11, "wb comparative", 0)?;
        self.battle_reward_chests = data.csiget(12, "wb chests", 0)?;
        // 13 => ???       // 0
        self.next_battle_loot_issued =
            data.cstget(14, "wb loot date", server_time)?;
        Ok(())
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
    pub(crate) fn parse(
        data: &[i64],
    ) -> Result<Option<WorldBossProjectile>, SFError> {
        if data.iter().all(|a| *a == 0) {
            return Ok(None);
        }
        Ok(Some(Self {
            typ: data.cfpget(0, "wb ammo typ", |a| a)?.unwrap_or_default(),
            amount: data.csiget(1, "wb ammo cnt", 0)?,
            extra_dmg: data.csiget(2, "wb ammo dmg", 0)?,
        }))
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
    pub(crate) fn parse(data: &[&str]) -> Result<Self, SFError> {
        let mut segments = EnumMap::default();

        for segment in WorldBossTowerSegment::iter() {
            let start = (segment as usize - 1) * 34 + 4;
            let data = data.skip(start, "tower segment")?;
            let mut most_damage_heroes = Vec::new();
            for j in 0..10 {
                let data = data.skip(5 + (j * 2), "wb dmg summary")?;
                most_damage_heroes.push(WorldBossTowerHeroDamageSummary {
                    level: data.cfsuget(0, "wb sum lvl")?,
                    name: data.cget(1, "wb sum name")?.into(),
                });
            }
            let mut most_special_atk_heroes = Vec::new();
            for j in 0..3 {
                let data = data.skip(25 + (j * 3), "wb special summary")?;
                most_special_atk_heroes.push(
                    WorldBossTowerHeroSpecialSummary {
                        level: data.cfsuget(0, "wb sum lvl")?,
                        name: data.cget(1, "wb sum name")?.into(),
                        count: data.cfsuget(2, "wb sum count")?,
                    },
                );
            }

            segments[segment] = WorldBossTowerSegmentInfo {
                signed_up_heroes: data.cfsuget(0, "wb signed up heroes")?,
                joint_damage: data.cfsuget(1, "wb joint damage")?,
                max_health: data.cfsuget(2, "wb max health")?,
                remaining_health: data.cfsuget(3, "wb remaining health")?,
                active_special_attacks: data
                    .cfsuget(4, "wb active special attacks")?,
                most_damage_heroes,
                most_special_atk_heroes,
            };
        }
        let weak_point_hit: i32 = data.cfsuget(3, "wb weak point hit")?;
        Ok(Self {
            boss_nr: data.cfsuget(0, "wb boss nr")?,
            unknown: data.cfsuget(1, "wb unknown wbb")?,
            weak_point: FromPrimitive::from_i32(
                data.cfsuget(2, "wb weak point")?,
            ),
            weak_point_hit: weak_point_hit == 1i32,
            segments,
        })
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
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Option<WorldBossCatapult>, SFError> {
        let Some(breaks) = data.cstget(0, "wb cp breaks", server_time)? else {
            return Ok(None);
        };

        let mut upgrades: [Option<WorldBossCatapultUpgrade>; 4] =
            Default::default();
        for (chunk, upgrade) in data
            .skip(1, "wb catapult")?
            .chunks_exact(4)
            .zip(&mut upgrades)
        {
            if chunk.iter().all(|a| *a == 0) {
                continue;
            }
            *upgrade = Some(WorldBossCatapultUpgrade {
                typ: chunk
                    .cfpget(0, "wb upgrade type", |a| a)?
                    .unwrap_or_default(),
                restriction: chunk.cfpget(1, "wb upgrade restrict", |a| a)?,
                amount: chunk.csiget(2, "wb upgrade amount", 0)?,
                effect_val: chunk.cget(3, "wb upgrade amount")?,
            });
        }

        Ok(Some(WorldBossCatapult { breaks, upgrades }))
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
    pub(crate) fn parse(data: &[i64]) -> Result<ProjectileOffer, SFError> {
        let mut res = Self {
            typ: data.cfpuget(0, "wb po typ", |a| a)?,
            buy_options: EnumMap::default(),
            effect_val: data.cget(3, "wb ev")?,
        };
        res.buy_options[ProjectileOfferQuantity::Small].amount =
            data.csiget(1, "wb po sa", 20)?;
        res.buy_options[ProjectileOfferQuantity::Large].amount =
            data.csiget(2, "wb po la", 50)?;
        res.buy_options[ProjectileOfferQuantity::Small].price =
            data.csiget(4, "wb po sp", 100)?;
        res.buy_options[ProjectileOfferQuantity::Large].price =
            data.csiget(5, "wb po lp", 250)?;
        Ok(res)
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
    pub(crate) fn parse(data: &[i64]) -> Result<UpgradeOffer, SFError> {
        Ok(UpgradeOffer {
            typ: data.cfpget(0, "wb upgrade typ", |a| a)?.unwrap_or_default(),
            restriction: data.cfpget(1, "wb ug restrict", |a| a)?,
            effect_value: data.cget(2, "wb ug effect type")?,
            raw_main_price: data.csiget(3, "wb ug price", u64::MAX)?,
            main_price_type: RewardType::parse(data.cget(4, "wb ug typ")? - 1),
            raw_catalyst_price: data.csiget(5, "wb sec ug price", u64::MAX)?,
            raw_mushroom_price: data.csiget(6, "wb mush ug price", u64::MAX)?,
        })
    }
}
