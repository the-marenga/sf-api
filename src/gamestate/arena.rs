use chrono::{DateTime, Local};
use num_traits::FromPrimitive;

use super::{items::*, *};
use crate::{
    PlayerId,
    misc::{ArrSkip, CGet},
};

/// The arena, that a player can fight other players in
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Arena {
    /// The enemies currently available in the arena. You have to fetch the
    /// full player info before fighting them, as you need their name
    pub enemy_ids: [PlayerId; 3],
    /// The time at which the player will be able to fight for free again
    pub next_free_fight: Option<DateTime<Local>>,
    /// The amount of fights this character has already fought today, that
    /// gave xp. 0-10
    pub fights_for_xp: u8,
}

/// A complete fight, which can be between multiple fighters for guild/tower
/// fights
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fight {
    /// The name of the attacking player for pet battles, or the name of the
    /// attacking guild in guild battles
    pub group_attacker_name: Option<String>,
    /// Either the player or guild id depending on pet/guild fight
    pub group_attacker_id: Option<u32>,

    /// The name of the attacking player for pet battles, or the name of the
    /// attacking guild in guild battles
    pub group_defender_name: Option<String>,
    /// Either the player or guild id depending on pet/guild fight
    pub group_defender_id: Option<u32>,

    /// The 1on1 fights within a larger fight, that end with one of the
    /// contestants defeated
    pub fights: Vec<SingleFight>,
    /// Whether the fight was won by the player.
    pub has_player_won: bool,
    /// The amount of money, that changed from a players perspective
    pub silver_change: i64,
    /// The amount of experience, that changed from a players perspective
    pub xp_change: u64,
    /// The amount of mushrooms the player got after this fight
    pub mushroom_change: u8,
    /// How much this fight changed the players honor by
    pub honor_change: i64,
    /// The rank before this fight
    pub rank_pre_fight: u32,
    /// The rank after this fight
    pub rank_post_fight: u32,
    /// The item this fight gave the player (if any)
    pub item_won: Option<Item>,
    /// Extra metadata specific to certain fight types
    pub extra: FightExtra,
}

/// Extra metadata for specific fight types
#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FightExtra {
    /// Default — no special metadata
    #[default]
    None,
    /// Fortress attack or defense details
    Fortress {
        /// Soldiers sent (attack) or deployed by enemy (defense)
        soldiers: u32,
        /// Stone looted or lost in a fortress attack
        stone: i64,
        /// Wood looted or lost in a fortress attack
        wood: i64,
        /// Archers defeated in a fortress defense
        archers_defeated: u32,
        /// Battlemages defeated in a fortress defense
        mages_defeated: u32,
    },
    /// Underworld lure — souls pillaged from another player
    UnderworldLure { souls: i64 },
}

impl Fight {
    pub(crate) fn update_result(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.has_player_won = data.cget(0, "has_player_won")? != 0;
        self.silver_change = data.cget(2, "fight silver change")?;

        // Underworld lure (fightresult.underworldpillage) — short format
        if data.len() < 20 {
            self.extra = FightExtra::UnderworldLure {
                souls: data.csiget(3, "underworld souls", 0)?,
            };
            return Ok(());
        }

        self.xp_change = data.csiget(3, "fight xp", 0)?;
        self.mushroom_change = data.csiget(4, "fight mushrooms", 0)?;
        self.honor_change = data.cget(5, "fight honor")?;

        self.rank_pre_fight = data.csiget(7, "fight rank pre", 0)?;
        self.rank_post_fight = data.csiget(8, "fight rank post", 0)?;
        let item = data.skip(9, "fight item")?;
        self.item_won = Item::parse(item, server_time)?;

        // Extended fortress fight data (fightresult.fortresspillagerv1)
        if data.len() >= 25 {
            self.extra = FightExtra::Fortress {
                soldiers: data.csiget(24, "soldiers", 0)?,
                stone: data.csiget(21, "fortress stone", 0)?,
                wood: data.csiget(22, "fortress wood", 0)?,
                archers_defeated: data.csiget(25, "archers defeated", 0)?,
                mages_defeated: data.csiget(26, "mages defeated", 0)?,
            };
        }

        Ok(())
    }

    pub(crate) fn update_groups(&mut self, val: &str) {
        let mut groups = val.split(',');

        let (Some(aid), Some(did), Some(aname), Some(dname)) = (
            groups.next().and_then(|a| a.parse().ok()),
            groups.next().and_then(|a| a.parse().ok()),
            groups.next(),
            groups.next(),
        ) else {
            warn!("Invalid fight group: {val}");
            return;
        };

        self.group_attacker_id = Some(aid);
        self.group_defender_id = Some(did);
        self.group_attacker_name = Some(aname.to_string());
        self.group_defender_name = Some(dname.to_string());
    }
}

/// This is a single fight between two fighters, which ends when one of them is
/// at <= 0 health
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SingleFight {
    /// The ID of the player, that won.
    pub winner_id: PlayerId,
    /// The stats of the first fighter. Typically the player, if the fight was
    /// started by them
    pub fighter_a: Option<Fighter>,
    /// The stats of the first fighter
    pub fighter_b: Option<Fighter>,
    /// The action this fight involved. Note that this will likely be changed
    /// in the future, as is it hard to interpret
    pub actions: Vec<FightAction>,
    /// Raw equipment data for `fighter_a`. Each entry is 19 values (`model_id`
    /// + item stats). The encoding differs from regular Item format.
    pub equipment: Vec<Vec<i64>>,
}

impl SingleFight {
    pub(crate) fn update_fighters(&mut self, data: &str) {
        let data = data.split('/').collect::<Vec<_>>();
        if data.len() < 60 {
            self.fighter_a = None;
            self.fighter_b = None;
            warn!("Fighter response too short");
            return;
        }
        // Each fighter has the same number of fields (49), but the leading
        // padding before the actual data may differ. The first fighter starts
        // at offset 0. The second fighter starts at an offset that gives it
        // the same number of leading zeros as the first fighter so that
        // Fighter::parse can find the id at index 5.
        //
        // Empirically the data layout is:
        //   Fighter A (49 values) | separator (1) | Fighter B (49 values)
        // With total = 99, split_at(47) gives:
        //   Fighter A: 47 values (indices 0-46) - loses 2 trailing zeros
        //   Fighter B: 52 values (indices 47-98) - gains 5 leading zeros
        //     (2 from Fighter A trailer + 1 separator + 2 Fighter B padding)
        // This makes the id land at index 5 for both fighters.
        let (fighter_a, fighter_b) = data.split_at(47);
        self.fighter_a = Fighter::parse(fighter_a);
        self.fighter_b = Fighter::parse(fighter_b);
    }

    pub(crate) fn update_rounds(
        &mut self,
        data: &str,
        fight_version: u32,
    ) -> Result<(), SFError> {
        self.actions.clear();

        if fight_version != 2 {
            // Unsupported fight version
            return Ok(());
        }
        // Format variants (all values are i64):
        //   9-value  (no effects):
        //     actor / 0 / type / outcome / 0 / actor_hp / target_hp / 0 / 0
        //   12-value (one fighter has an effect):
        //     actor / 0 / type / outcome / 0 / actor_hp / target_hp / [5
        // extras]       Actor's effect:  [who=1,  flag,       id,
        // rem,         trail=0]       Opponent effect: [0,
        // marker=1,   flag,  id,          rem]   15-value (both
        // fighters have effects, or one has two):     actor / 0 / type
        // / outcome / 0 / actor_hp / target_hp / [8 extras]
        //       [who1, eff1_flag, eff1_id, eff1_rem, who2, eff2_flag, eff2_id,
        // eff2_rem]       who={0→opponent, ≠0→actor}
        let raw: Vec<&str> = data.split('/').collect();
        // Parse once to i64 for robust stride detection
        let values: Vec<i64> =
            raw.iter().filter_map(|s| s.parse().ok()).collect();

        let mut i = 0;
        while i + 9 <= values.len() {
            let extras_first = values.cget(i + 7, "extras_first")?; // 0 if none; who=0 → opponent, ≠0 → actor
            let extras_second = values.cget(i + 8, "extras_second")?;

            // Detect stride: 9-value if both effect slots are 0
            let stride = if extras_first == 0 && extras_second == 0 {
                9
            } else if i + 15 <= values.len()
                && matches!(values.cget(i + 12, "stride_p12")?, 1..=3)
            {
                // 15-value: position 12 is the second effect-block's flag
                // (1=Ability, 2=Minion, 3=Poison). In any other format,
                // position 12 is the next action's actor_id (>3 or <0).
                15
            } else if i + 12 <= values.len() {
                12
            } else {
                9
            };

            let acting_id = values.cget(i, "acting_id")?;
            let action_type: u32 =
                u32::try_from(values.cget(i + 2, "action_type")?).unwrap_or(0);
            let outcome_code: u32 =
                u32::try_from(values.cget(i + 3, "outcome")?).unwrap_or(0);

            let action = FightActionType::parse(action_type);
            let outcome = match outcome_code {
                3 => FightOutcome::Blocked,
                4 => FightOutcome::Evaded,
                _ => FightOutcome::Normal,
            };

            let actor_life = values.cget(i + 5, "actor_life")?;
            let target_life = values.cget(i + 6, "target_life")?;

            let actor_state =
                FighterState::from_raw(values.cget(i + 1, "actor_state")?);
            let defender_state =
                FighterState::from_raw(values.cget(i + 4, "defender_state")?);

            let (actor_effect, opponent_effect) = if stride > 9 {
                let extras_start = values.skip(i + 7, "extras")?;
                let extra_vals =
                    extras_start.get(..(stride - 7)).unwrap_or(&[]);
                parse_active_effect(extra_vals)
            } else {
                (None, None)
            };

            self.actions.push(FightAction {
                acting_id,
                action,
                outcome,
                other_new_life: target_life,
                actor_life: Some(actor_life),
                actor_effect,
                opponent_effect,
                actor_state,
                defender_state,
            });

            i += stride;
        }

        if i < values.len() {
            let trailing = raw.get(i..).unwrap_or(&[]);
            warn!(
                "{} trailing unparsed values in fight.r: {:?}",
                values.len() - i,
                trailing,
            );
        }

        Ok(())
    }
}

/// A participant in a fight. Can be anything, that shows up in the battle
/// screen from the player to a fortress Wall
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fighter {
    /// The type of the fighter
    pub typ: FighterTyp,
    /// The raw id of the fighter. This is <= 0 for monsters & companions and
    /// equivalent to the player id for players (>0)
    pub id: i64,
    /// The name of the fighter, if it is a player
    pub name: Option<String>,
    /// The level of the fighter
    pub level: u32,
    /// The amount of hp this fighter has at the start of a battle
    pub life: u32,
    /// The total attributes this fighter has
    pub attributes: EnumMap<AttributeType, u32>,
    /// The class of the fighter
    pub class: Class,
}

impl Fighter {
    // TODO: Make this return Result?
    pub(crate) fn parse(data: &[&str]) -> Option<Fighter> {
        let fighter_typ: i64 = data.cfsget(5, "fighter typ").ok()??;

        let mut fighter_type = match fighter_typ {
            -391 => FighterTyp::Companion(CompanionClass::Warrior),
            -392 => FighterTyp::Companion(CompanionClass::Mage),
            -393 => FighterTyp::Companion(CompanionClass::Scout),
            1.. => FighterTyp::Player,
            x => {
                let monster_id = soft_into(-x, "monster_id", 0);
                FighterTyp::Monster(monster_id)
            }
        };

        let mut attributes = EnumMap::default();
        let raw_atrs =
            parse_vec(data.get(10..15)?, "fighter attributes", |a| {
                a.parse().ok()
            })
            .ok()?;
        update_enum_map(&mut attributes, &raw_atrs);

        let class: i32 = data.cfsget(27, "fighter class").ok().flatten()?;
        let class: Class = FromPrimitive::from_i32(class - 1)?;

        let id = data.cfsget(5, "fighter id").ok()?.unwrap_or_default();

        // Parse the name field, which doubles as fighter-type override for
        // special NPCs (fortress units, underworld minions) and pets.
        let raw_name = data.cget(6, "fighter name").ok()?;
        let name = match raw_name.parse::<i64>() {
            Ok(-719..=-710) => {
                fighter_type = FighterTyp::FortressSoldier;
                None
            }
            Ok(-729..=-720) => {
                fighter_type = FighterTyp::FortressMage;
                None
            }
            Ok(-739..=-730) => {
                fighter_type = FighterTyp::FortressArcher;
                None
            }
            Ok(-799..=-740) => {
                fighter_type = FighterTyp::FortressWall;
                None
            }
            Ok(..=-1) => None,
            Ok(0) => {
                let uwm_id = data.cget(15, "fighter uwm").ok()?;
                if ["-910", "-935", "-933", "-924"].contains(&uwm_id) {
                    fighter_type = FighterTyp::UnderworldMinion;
                }
                None
            }
            Ok(pid) if pid == id && fighter_type == FighterTyp::Player => {
                fighter_type = FighterTyp::Pet;
                None
            }
            _ => Some(raw_name.to_string()),
        };

        Some(Fighter {
            typ: fighter_type,
            id,
            name,
            level: data.cfsget(7, "fighter lvl").ok()??,
            life: data.cfsget(8, "fighter life").ok()??,
            attributes,
            class,
        })
    }
}

/// The outcome of a single round in a fight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FightOutcome {
    /// A normal hit — neither blocked nor evaded
    #[default]
    Normal,
    /// The action was blocked by the defender
    Blocked,
    /// The action was evaded by the defender
    Evaded,
}

/// The type of summoned minion (Necromancer)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Minion {
    #[default]
    Skeleton,
    Hound,
    Golem,
}

/// Decodes a pos1/pos4 raw value into a fighter's active state.
/// These values appear in positions 1 and 4 of the 9-value format and
/// indicate what special state a fighter is in (stance, form, enrage, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FighterState {
    /// No special state
    #[default]
    Normal,
    /// Druid in eagle form
    EagleForm,
    /// Druid in bear form
    BearForm,
    /// Paladin in Defensive stance (value 20)
    DefensiveStance,
    /// Paladin in Offensive stance (value 21)
    OffensiveStance,
    /// Berserker in frenzy mode (value 30)
    Frenzy,
    /// An unrecognized state value (raw value attached for debugging)
    Unknown(i64),
}

impl FighterState {
    pub(crate) fn from_raw(val: i64) -> Self {
        match val {
            0 => FighterState::Normal,
            10 => FighterState::EagleForm,
            11 => FighterState::BearForm,
            20 => FighterState::DefensiveStance,
            21 => FighterState::OffensiveStance,
            30 => FighterState::Frenzy,
            _ => {
                if val != 0 {
                    warn!("Unknown fighter state: {val}");
                }
                FighterState::Unknown(val)
            }
        }
    }
}

/// An active effect on a fighter — either a summoned minion or a class ability
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ActiveEffect {
    /// A summoned minion
    Minion {
        /// The type of minion (Skeleton, Hound, or Golem)
        minion_type: Minion,
        /// How many actions the minion can still take
        remaining_actions: u32,
    },
    /// A poison/debuff effect (`PlagueDoctor`)
    Poison {
        /// The numeric ID of the poison type
        id: u32,
        /// How many rounds the poison is still active for
        remaining_rounds: u32,
    },
    /// A class ability (e.g. Bard melody, Druid bear form)
    Ability {
        /// The numeric ID of the ability
        id: u32,
        /// How many rounds the ability is still active for
        remaining_rounds: u32,
    },
    /// An unknown effect type, with the raw flag and id values
    Unknown {
        /// The raw type flag from the server
        flag: u32,
        /// The raw id from the server
        id: u32,
        /// The remaining rounds/actions from the server
        remaining: u32,
    },
}

/// One round (action) in a fight. This is mostly just one attack
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FightAction {
    /// The id of the fighter, that does the action
    pub acting_id: i64,
    /// The new current life of the fighter, that was hit. Note that this may
    /// be 0 for actions, like spawning minions, that dont have a target
    /// and thus no target health.
    pub other_new_life: i64,
    /// The action, that the active side does
    pub action: FightActionType,
    /// The outcome of this action (blocked, evaded, or normal)
    pub outcome: FightOutcome,
    /// The life of the acting fighter at the time of this action. Only
    /// available in `fight_version` >= 2
    pub actor_life: Option<i64>,
    /// The active effect on the acting fighter, if any (minion or ability)
    pub actor_effect: Option<ActiveEffect>,
    /// The active effect on the opponent, if any (minion or ability)
    pub opponent_effect: Option<ActiveEffect>,
    /// Decoded state of the acting fighter (from position 1 in 9-value
    /// format). Non-zero when the fighter has an active stance/special
    /// ability.
    pub actor_state: FighterState,
    /// Decoded state of the defending fighter (from position 4 in 9-value
    /// format). Non-zero when the fighter has an active stance/special
    /// ability.
    pub defender_state: FighterState,
}

/// An action in a fight. In the official client this determines the animation,
/// that gets played
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FightActionType {
    /// A simple attack with the normal weapon
    Attack,
    /// A critical hit
    Crit,
    /// One shot from a loaded mushroom catapult in a guild battle
    MushroomCatapult,
    /// Summons a minion (Necromancer)
    Summon,
    /// A minion attacks (Necromancer skeleton)
    MinionAttack,
    /// A minion attacks after the main fighter attacked
    MinionAttack2,
    /// `BattleMage`'s opening fireball
    BattleMageFireball,
    /// Assassin's main hand attack
    AssassinMainHand,
    /// Assassin's off hand attack
    AssassinOffHand,
    /// `DemonHunter`'s revive ability
    Revive,
    /// `PlagueDoctor` throws a poison tincture
    ThrowPoison,
    /// `PlagueDoctor`'s poison deals damage over time
    PoisonTick,
    /// I have not checked all possible battle types, so whatever action I have
    /// missed will be parsed as this, with the raw integer value attached
    Unknown(u32),
}

impl FightActionType {
    pub(crate) fn parse(val: u32) -> FightActionType {
        match val {
            0 => FightActionType::Attack,
            1 => FightActionType::Crit,
            2 => FightActionType::MushroomCatapult,
            10 => FightActionType::BattleMageFireball,
            11 => FightActionType::Summon,
            12 => FightActionType::MinionAttack, /* minion acts alone (e.g.
                                                   * after summon) */
            15 => FightActionType::MinionAttack2, /* minion acts after
                                                    * player also attacked */
            14 => FightActionType::Revive,
            17 | 18 => FightActionType::ThrowPoison,
            19 | 20 => FightActionType::PoisonTick,
            100 => FightActionType::AssassinMainHand,
            101 => FightActionType::AssassinOffHand,
            _ => {
                warn!("Unknown fight action type: {val}");
                FightActionType::Unknown(val)
            }
        }
    }
}

/// Safely clamp an `i64` to `u32`, treating negatives as 0.
fn clamp_u32(v: i64) -> u32 {
    u32::try_from(v.max(0)).unwrap_or(0)
}

/// Parse a single active effect from three consecutive `extras` values.
fn parse_one_effect(extras: &[i64], start: usize) -> Option<ActiveEffect> {
    if start + 2 >= extras.len() {
        return None;
    }
    let flag = extras.cget(start, "eff_f").unwrap_or(0);
    let id = extras.cget(start + 1, "eff_id").unwrap_or(0);
    let remaining = extras.cget(start + 2, "eff_rem").unwrap_or(0);
    Some(match flag {
        1 => ActiveEffect::Ability {
            id: clamp_u32(id),
            remaining_rounds: clamp_u32(remaining),
        },
        2 => ActiveEffect::Minion {
            minion_type: match id {
                1 => Minion::Skeleton,
                2 => Minion::Hound,
                3 => Minion::Golem,
                _ => return None,
            },
            remaining_actions: clamp_u32(remaining),
        },
        3 => ActiveEffect::Poison {
            id: clamp_u32(id),
            remaining_rounds: clamp_u32(remaining),
        },
        _ => {
            warn!(
                "Unknown active effect: flag={flag}, id={id}, \
                 remaining={remaining}"
            );
            ActiveEffect::Unknown {
                flag: clamp_u32(flag),
                id: clamp_u32(id),
                remaining: clamp_u32(remaining),
            }
        }
    })
}

/// Parse the 5 (12-value) or 8 (15-value) extra values into active effects.
///
/// 12-value (5 extras):
///   [who=1,    flag, id, rem, trail=0]  → (actor,   None)
///   [0,        marker=1, flag, id, rem] → (None,    opponent)
///
/// 15-value (8 extras):
///   [who1, flag1, id1, rem1, who2, flag2, id2, rem2]
///   who=0      → that block belongs to the **opponent**
///   who≠0      → that block belongs to the **actor**
///   When both blocks belong to the same fighter, only the first
///   is returned (the second is typically an expired sentinel).
fn parse_active_effect(
    extras: &[i64],
) -> (Option<ActiveEffect>, Option<ActiveEffect>) {
    if extras.len() < 4 {
        return (None, None);
    }

    if extras.len() >= 8 {
        // 15-value: two effect blocks, each with an ownership flag
        let who1_actor = extras.first().copied().unwrap_or(0) != 0;
        let who2_actor = extras.get(4).copied().unwrap_or(0) != 0;

        let eff1 = parse_one_effect(extras, 1);
        let eff2 = parse_one_effect(extras, 5);

        let actor_effect = if who1_actor {
            eff1
        } else if who2_actor {
            eff2
        } else {
            None
        };
        let opponent_effect = if !who1_actor {
            eff1
        } else if !who2_actor {
            eff2
        } else {
            None
        };

        (actor_effect, opponent_effect)
    } else if extras.first().copied().unwrap_or(0) != 0 {
        // 12-value, actor's effect: [who=1, flag, id, rem, trail=0]
        (parse_one_effect(extras, 1), None)
    } else {
        // 12-value, opponent's effect: [0, marker=1, flag, id, rem]
        (None, parse_one_effect(extras, 2))
    }
}

/// The type of the participant in a fight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FighterTyp {
    /// Not just the own player, but any player on the server
    #[default]
    Player,
    /// A generic monster, or dungeon boss with its `monster_id`
    Monster(u16),
    /// One of the players companions
    Companion(CompanionClass),
    /// A soldier in a fortress attack
    FortressSoldier,
    /// An archer defending a fortress
    FortressArcher,
    /// A battlemage defending a fortress
    FortressMage,
    /// The wall in a fortress attack
    FortressWall,
    /// A minion in an underworld lure battle
    UnderworldMinion,
    /// A pet
    Pet,
}
