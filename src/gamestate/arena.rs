use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::warn;

use super::{AttributeType, Class, CompanionClass, Item, Result, ServerTime};
use crate::{PlayerId, error::SFError, response::Vals};

/// The number of values a single fighter record of a `fightheader*.fighters`
/// response takes up.
const FIGHTER_RECORD_LEN: usize = 47;

/// The number of values of a fight round without any active effect:
/// `actor / state / type / outcome / state / actor_life / target_life / 0 / 0`.
const ROUND_LEN_PLAIN: usize = 9;
/// The number of values of a fight round with a single effect block.
const ROUND_LEN_ONE_EFFECT: usize = 12;
/// The number of values of a fight round with two effect blocks.
const ROUND_LEN_TWO_EFFECTS: usize = 15;

/// The arena, that a player can fight other players in
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

impl Arena {
    /// Updates the arena from an `arena` response of 7 values:
    /// `next_free_fight / fights_for_xp / 3 enemy ids / ? / ?`.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.next_free_fight = data.server_time(0, server_time)?;
        self.fights_for_xp = data.u8(1)?;
        for (idx, enemy) in self.enemy_ids.iter_mut().enumerate() {
            *enemy = data.u32(2 + idx)?;
        }
        data.ignore(5, "always 1 so far");
        data.ignore(6, "always 1 so far");
        Ok(())
    }
}

/// A complete fight, which can be between multiple fighters for guild/tower
/// fights
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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
    UnderworldLure {
        /// The amount of souls gained (or lost) in this fight
        souls: i64,
    },
}

impl Fight {
    /// Updates this fight from a `fightresult` response. `battlereward` (21
    /// values) and `fortresspillagerv1` (27) share their first 9 values, the
    /// shorter `underworldpillage` has the pillaged souls at index 3.
    pub(crate) fn update_result(&mut self, data: &Vals<'_>) {
        self.has_player_won = data.i64_or(0, 0) != 0;
        self.silver_change = data.i64_or(2, 0);

        // Underworld lure (fightresult.underworldpillage) — short format
        if data.len() <= 19 {
            self.extra = FightExtra::UnderworldLure {
                souls: data.i64_or(3, 0),
            };
            data.ignore_unmodelled(
                &[1..2, 4..data.len()],
                "underworld lure field that is not modelled",
            );
            return;
        }

        self.xp_change = data.u64_or(3, 0);
        self.mushroom_change = data.u8_or(4, 0);
        self.honor_change = data.i64_or(5, 0);

        self.rank_pre_fight = data.u32_or(7, 0);
        self.rank_post_fight = data.u32_or(8, 0);

        // Extended fortress fight data (fightresult.fortresspillagerv1)
        if data.len() >= 27 {
            self.extra = FightExtra::Fortress {
                soldiers: data.u32_or(24, 0),
                stone: data.i64_or(21, 0),
                wood: data.i64_or(22, 0),
                archers_defeated: data.u32_or(25, 0),
                mages_defeated: data.u32_or(26, 0),
            };
            // 1 & 6 as above, 9-20 & 23 are not modelled yet
            data.ignore_unmodelled(
                &[1..2, 6..7, 9..21, 23..24, 27..data.len()],
                "fortress fight result field that is not modelled",
            );
        } else {
            // 1 is a 0/1 flag that does not follow the win, 6 is 0 so far and
            // 9-20 are always 0. The won item used to be parsed from index 9,
            // it now comes from the `battlerewarditem` response
            data.ignore_unmodelled(
                &[1..2, 6..7, 9..data.len()],
                "fight result field that is not modelled",
            );
        }
    }

    /// Updates the group ids & names from a `fightgroups` response:
    /// `attacker_id,defender_id,attacker_name,defender_name`.
    pub(crate) fn update_groups(&mut self, data: &Vals<'_>) {
        self.group_attacker_id = Some(data.u32_or(0, 0));
        self.group_defender_id = Some(data.u32_or(1, 0));
        self.group_attacker_name = Some(data.str_or(2, "").to_string());
        self.group_defender_name = Some(data.str_or(3, "").to_string());
    }
}

/// This is a single fight between two fighters, which ends when one of them is
/// at <= 0 health
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SingleFight {
    /// The ID of the player, that won.
    pub winner_id: PlayerId,
    /// The stats of the first fighter. Typically the player, if the fight was
    /// started by them
    pub fighter_a: Option<Fighter>,
    /// The stats of the second fighter
    pub fighter_b: Option<Fighter>,
    /// The action this fight involved
    pub actions: Vec<FightAction>,
}

impl SingleFight {
    /// Updates both fighters from a `fightheader*.fighters` response: two
    /// [`FIGHTER_RECORD_LEN`] value fighter records followed by 5 unknown
    /// values, both records keep the fighter id at index 5 of their own
    /// record.
    pub(crate) fn update_fighters(&mut self, data: &Vals<'_>) {
        let second = FIGHTER_RECORD_LEN..FIGHTER_RECORD_LEN * 2;

        self.fighter_a = Fighter::parse(&data.window(0..FIGHTER_RECORD_LEN))
            .map_err(|err| warn!("Failed to parse fighter A: {err}"))
            .ok();
        self.fighter_b = Fighter::parse(&data.window(second))
            .map_err(|err| warn!("Failed to parse fighter B: {err}"))
            .ok();

        // The 5 values after the two records are always 0 in every captured
        // response
        let trailing = FIGHTER_RECORD_LEN * 2..data.len();
        data.ignore_unmodelled(
            &[trailing],
            "fight header field that is not modelled",
        );
    }

    /// Updates the rounds of this fight from a `fight*.r` response. Unlike the
    /// other fight responses it has no fixed number of values, every round is
    /// [`ROUND_LEN_PLAIN`] (no effects), [`ROUND_LEN_ONE_EFFECT`] (one effect)
    /// or [`ROUND_LEN_TWO_EFFECTS`] (two effects) values long: always 7 base
    /// values (`actor / state / type / outcome / state / actor_life /
    /// target_life`) plus 2, 5 or 8 more.
    pub(crate) fn update_rounds(
        &mut self,
        data: &Vals<'_>,
        fight_version: u32,
    ) {
        self.actions.clear();

        if fight_version != 2 {
            // Unsupported fight version
            return;
        }

        let mut i = 0;
        while i + ROUND_LEN_PLAIN <= data.len() {
            let extras_first = data.i64_or(i + 7, 0);
            let extras_second = data.i64_or(i + 8, 0);

            // Detect stride: plain if both effect slots are 0
            let stride = if extras_first == 0 && extras_second == 0 {
                ROUND_LEN_PLAIN
            } else if i + ROUND_LEN_TWO_EFFECTS <= data.len()
                && matches!(data.i64_or(i + ROUND_LEN_ONE_EFFECT, 0), 1..=3)
            {
                // Two effects: position 12 is the second effect-block's flag
                // (1=Ability, 2=Minion, 3=Poison). In any other format,
                // position 12 is the next action's actor_id (>3 or <0). No
                // captured response has used this stride so far, every effect
                // round in the logs is 12 values
                ROUND_LEN_TWO_EFFECTS
            } else if i + ROUND_LEN_ONE_EFFECT <= data.len() {
                ROUND_LEN_ONE_EFFECT
            } else {
                ROUND_LEN_PLAIN
            };

            let acting_id = data.i64_or(i, 0);
            let action_type = data.u32_or(i + 2, 0);
            let outcome_code = data.u32_or(i + 3, 0);

            let action = FightActionType::parse(action_type);
            let outcome = match outcome_code {
                3 => FightOutcome::Blocked,
                4 => FightOutcome::Evaded,
                _ => FightOutcome::Normal,
            };

            let actor_life = data.i64_or(i + 5, 0);
            let target_life = data.i64_or(i + 6, 0);

            let actor_state = FighterState::from_raw(data.i64_or(i + 1, 0));
            let defender_state = FighterState::from_raw(data.i64_or(i + 4, 0));

            let (actor_effect, opponent_effect) = if stride > ROUND_LEN_PLAIN {
                // A single effect block has an unused trailing slot, a double
                // one does not
                if stride == ROUND_LEN_ONE_EFFECT {
                    data.ignore(
                        i + ROUND_LEN_ONE_EFFECT - 1,
                        "unused effect trailer slot",
                    );
                }
                parse_active_effect(&data.window(i + 7..i + stride))
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

        if i < data.len() {
            warn!(
                "{} trailing unparsed values in fight.r: {:?}",
                data.len() - i,
                data.slice(i..data.len()),
            );
        }
    }
}

/// A participant in a fight. Can be anything, that shows up in the battle
/// screen from the player to a fortress Wall
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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
    /// The maximum amount of hp this fighter has, i.e. its hp at the start of
    /// a battle
    pub life: u32,
    /// The amount of hp this fighter has left when the fight header was sent.
    /// This is its start of battle hp for the first header of a fight and is
    /// lower from the second one on
    pub current_life: u32,
    /// The total attributes this fighter has
    pub attributes: EnumMap<AttributeType, u32>,
    /// The class of the fighter
    pub class: Class,
}

impl Fighter {
    /// Parses one [`FIGHTER_RECORD_LEN`] value fighter record of a
    /// `fightheader*.fighters` response. Index 5 is the raw id and doubles as
    /// the fighter type (see [`FighterTyp`]), index 6 the name, 7 the level, 8
    /// the max hp, 9 the remaining hp, 10-14 the attributes and 27 the class.
    /// Index 15 is a name fallback that marks underworld minions.
    ///
    /// # Errors
    /// [`SFError::ParsingErrorV2`] if the record is too short, contains an
    /// unparsable number or has an unknown class.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Fighter> {
        // The fields of the record that are still unmodelled:
        // - 0:     the category of the own side (2 player, 3, 8 fortress
        //   soldier, 12 companion, 14 pet), always 0 for the opponent
        // - 1, 2:  always 0 in every captured record
        // - 4:     the side, 1 for fighter_a and 0 for fighter_b
        // - 16-26: values in the 100-400 range with small counters for players,
        //   0 for companions and pets. They do not match the ids of the
        //   `fightequipment` response
        // - 28-46: always 0 in every captured record
        data.ignore_unmodelled(
            &[0..5, 16..27, 28..FIGHTER_RECORD_LEN],
            "fighter field that is not modelled",
        );

        // Index 15 is only meaningful for underworld minions, but is read
        // unconditionally so records without one are not reported as unread.
        let name_fallback = data.get_str(15).unwrap_or_default();

        let raw_id = data.i64(5)?;
        let mut fighter_type = match raw_id {
            -391 => FighterTyp::Companion(CompanionClass::Warrior),
            -392 => FighterTyp::Companion(CompanionClass::Mage),
            -393 => FighterTyp::Companion(CompanionClass::Scout),
            1.. => FighterTyp::Player,
            x => FighterTyp::Monster(u16::try_from(-x).unwrap_or_else(|_| {
                warn!("Invalid monster id: {x}");
                0
            })),
        };

        let mut attributes = EnumMap::default();
        for (idx, attr) in attributes.values_mut().enumerate() {
            *attr = data.u32(10 + idx)?;
        }

        // The class is 1 based, 0 means no class and maps out of range
        let class =
            data.enum_map(27, |raw| raw.wrapping_sub(1))
                .ok_or_else(|| {
                    SFError::ParsingErrorV2(format!(
                        "Unknown fighter class: {}",
                        data.u32_or(27, 0)
                    ))
                })?;

        // Parse the name field, which doubles as fighter-type override for
        // special NPCs (fortress units, underworld minions) and pets.
        let raw_name = data.str(6)?;
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
                // Index 15 doubles as the underworld minion marker
                let minion_names = ["-910", "-935", "-933", "-924"];
                if minion_names.contains(&name_fallback) {
                    fighter_type = FighterTyp::UnderworldMinion;
                }
                None
            }
            Ok(pid) if pid == raw_id && fighter_type == FighterTyp::Player => {
                fighter_type = FighterTyp::Pet;
                None
            }
            _ => Some(raw_name.to_string()),
        };

        Ok(Fighter {
            typ: fighter_type,
            id: raw_id,
            name,
            level: data.u32(7)?,
            life: data.u32(8)?,
            current_life: data.u32(9)?,
            attributes,
            class,
        })
    }
}

/// The outcome of a single round in a fight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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
#[non_exhaustive]
pub enum Minion {
    /// A skeleton
    #[default]
    Skeleton,
    /// A hound
    Hound,
    /// A golem
    Golem,
}

/// Decodes a pos1/pos4 raw value into a fighter's active state.
/// These values appear in positions 1 and 4 of the 9-value format and
/// indicate what special state a fighter is in (stance, form, enrage, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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
                warn!("Unknown fighter state: {val}");
                FighterState::Unknown(val)
            }
        }
    }
}

/// An active effect on a fighter — either a summoned minion or a class ability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// A minion attacks (Necromancer)
    MinionAttack,
    /// A minion's critical hit
    MinionAttackCrit,
    /// `Druid`'s eagle swoop attack
    Swoop,
    /// `Druid`'s eagle swoop critical hit
    SwoopCrit,
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
            12 => FightActionType::MinionAttack,
            13 => FightActionType::Swoop,
            14 => FightActionType::Revive,
            15 => FightActionType::MinionAttackCrit,
            16 => FightActionType::SwoopCrit,
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

/// Parse a single active effect from an effect block: `flag / id / remaining`.
fn parse_one_effect(block: &Vals<'_>) -> Option<ActiveEffect> {
    let flag = block.u32_or(0, 0);
    let id = block.u32_or(1, 0);
    let remaining = block.u32_or(2, 0);
    Some(match flag {
        1 => ActiveEffect::Ability {
            id,
            remaining_rounds: remaining,
        },
        2 => ActiveEffect::Minion {
            minion_type: match id {
                1 => Minion::Skeleton,
                2 => Minion::Hound,
                3 => Minion::Golem,
                _ => {
                    warn!("Unknown minion type: {id}");
                    return None;
                }
            },
            remaining_actions: remaining,
        },
        3 => ActiveEffect::Poison {
            id,
            remaining_rounds: remaining,
        },
        _ => {
            warn!(
                "Unknown active effect: flag={flag}, id={id}, \
                 remaining={remaining}"
            );
            ActiveEffect::Unknown {
                flag,
                id,
                remaining,
            }
        }
    })
}

/// Parses the 5 (single effect round) or 8 (double effect round) extra values
/// into active effects. Positions are relative to `extras`.
///
/// Single effect (5 extras):
///   `[who=1, flag, id, rem, trailer=0]` → (actor, None)
///   `[0, marker=1, flag, id, rem]`      → (None, opponent)
///
/// Double effect (8 extras):
///   `[who1, flag1, id1, rem1, who2, flag2, id2, rem2]`
///   `who=0` → that block belongs to the **opponent**
///   `who≠0` → that block belongs to the **actor**
///   When both blocks belong to the same fighter, only the first is returned
///   (the second is typically an expired sentinel).
fn parse_active_effect(
    extras: &Vals<'_>,
) -> (Option<ActiveEffect>, Option<ActiveEffect>) {
    // A leading flag plus at least one effect block is required
    if extras.len() < 4 {
        return (None, None);
    }

    if extras.len() >= 8 {
        // Double effect: two blocks, each with an ownership flag
        let who1_actor = extras.i64_or(0, 0) != 0;
        let who2_actor = extras.i64_or(4, 0) != 0;

        let eff1 = parse_one_effect(&extras.window(1..4));
        let eff2 = parse_one_effect(&extras.window(5..8));

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
    } else if extras.i64_or(0, 0) != 0 {
        // Single effect, the actor's: [who=1, flag, id, rem, trailer=0]
        (parse_one_effect(&extras.window(1..4)), None)
    } else {
        // Single effect, the opponent's: [0, marker=1, flag, id, rem]. The
        // marker is not enforced: an unexpected block is still attributed to
        // the opponent rather than silently dropped.
        if extras.i64_or(1, 0) != 1 {
            warn!("Unexpected opponent effect marker: {extras:?}");
        }
        (None, parse_one_effect(&extras.window(2..5)))
    }
}

/// The type of the participant in a fight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use chrono::Local;

    use super::*;
    use crate::response::Vals;

    /// Asserts that every value of `data` was read or explicitly ignored. The
    /// read tracking only exists in debug builds.
    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
    }

    /// Parses a single [`FIGHTER_RECORD_LEN`] value fighter record.
    fn parse_fighter(record: &str) -> Fighter {
        let data = Vals::from(record);
        assert_eq!(data.len(), FIGHTER_RECORD_LEN);
        Fighter::parse(&data).unwrap()
    }

    #[test]
    fn test_arena_update() {
        let mut arena = Arena::default();
        let data = Vals::from("1785018466/0/111111/222222/333333/1/1");
        arena.update(&data, ServerTime::default()).unwrap();
        assert_all_read(&data);

        assert_eq!(arena.enemy_ids, [111_111, 222_222, 333_333]);
        assert_eq!(arena.fights_for_xp, 0);
        assert!(arena.next_free_fight.is_some());
    }

    #[test]
    fn test_update_fighters() {
        let raw = "0/0/0/0/1/111111/PlayerOne/50/236232/236232/95/73/757/1158/\
                   428/5/303/301/3/303/1/5/16/0/0/1/1/10/0/0/0/0/0/0/0/0/0/0/\
                   0/0/0/0/0/0/0/0/0/0/0/0/0/0/222222/PlayerTwo \
                   (s1de)/51/164736/164736/815/253/253/792/808/1/204/202/4/\
                   205/4/4/2/0/0/3/1/6/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                   0/0/0/0/0";
        let mut fight = SingleFight::default();
        let data = Vals::from(raw);
        fight.update_fighters(&data);
        assert_all_read(&data);

        let a = fight.fighter_a.unwrap();
        assert_eq!(a.typ, FighterTyp::Player);
        assert_eq!(a.id, 111_111);
        assert_eq!(a.name.as_deref(), Some("PlayerOne"));
        assert_eq!(a.level, 50);
        assert_eq!(a.life, 236_232);
        assert_eq!(a.current_life, 236_232);
        assert_eq!(a.class, Class::Necromancer);
        assert_eq!(a.attributes[AttributeType::Strength], 95);
        assert_eq!(a.attributes[AttributeType::Luck], 428);

        let b = fight.fighter_b.unwrap();
        assert_eq!(b.id, 222_222);
        assert_eq!(b.name.as_deref(), Some("PlayerTwo (s1de)"));
        assert_eq!(b.level, 51);
        assert_eq!(b.life, 164_736);
        assert_eq!(b.current_life, 164_736);
    }

    #[test]
    fn test_fighter_types() {
        // Fighter records covering each kind. The underworld minion reuses one
        // of them with the documented marker at index 15, as no underworld
        // lure capture exists.
        let cases: [(&str, FighterTyp); 10] = [
            // companion warrior (-391)
            (
                "12/0/0/68/1/-391/-391/365/34711556/34711556/18016/6842/4588/\
                 10824/6673/-391/0/0/0/0/0/0/0/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0/0/0/0/0/0/0",
                FighterTyp::Companion(CompanionClass::Warrior),
            ),
            // companion mage (-392)
            (
                "12/0/0/68/1/-392/-392/365/10888295/10888295/5774/5777/15094/\
                 11184/7198/-392/0/0/0/0/0/0/0/0/0/0/0/2/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0/0/0/0/0/0/0",
                FighterTyp::Companion(CompanionClass::Mage),
            ),
            // companion scout (-393)
            (
                "12/0/0/68/1/-393/-393/365/35739679/35739679/5388/18464/4206/\
                 15961/9880/-393/0/0/0/0/0/0/0/0/0/0/0/3/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0/0/0/0/0/0/0",
                FighterTyp::Companion(CompanionClass::Scout),
            ),
            // fortress soldier, mage, archer & wall (named -710 to -799)
            (
                "8/0/0/0/1/712/-712/150/1736500/1736500/2300/10/10/2300/1515/\
                 -712/1/41/0/0/0/0/0/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0/0",
                FighterTyp::FortressSoldier,
            ),
            (
                "0/0/0/0/0/722/-722/140/606300/606300/10/10/2150/2150/1415/\
                 -722/1/-1/0/0/0/0/0/0/0/0/0/2/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0/0",
                FighterTyp::FortressMage,
            ),
            (
                "0/0/0/0/0/732/-732/140/1212600/1212600/10/2150/10/2150/1415/\
                 -732/1/30/0/0/0/0/0/0/0/0/0/3/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0/0",
                FighterTyp::FortressArcher,
            ),
            (
                "0/0/0/0/0/756/-756/185/7881750/7881750/2825/935/935/8475/0/\
                 -756/1/1/0/0/0/0/0/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0",
                FighterTyp::FortressWall,
            ),
            // a pet repeats the player id in its name
            (
                "14/0/0/0/1/111111/111111/1/328/328/20/41/20/41/30/-800/0/0/0/\
                 0/0/0/0/0/0/0/0/3/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0",
                FighterTyp::Pet,
            ),
            // an id of 0 stays a monster
            (
                "0/0/0/0/0/0/0/222/80000000/80000000/90000/2000/2000/71749/\
                 5000/-1320/0/0/0/0/0/0/0/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0/0/0/0",
                FighterTyp::Monster(0),
            ),
            // index 15 marks an underworld minion
            (
                "0/0/0/0/0/0/0/222/80000000/80000000/90000/2000/2000/71749/\
                 5000/-910/0/0/0/0/0/0/0/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/\
                 0/0/0/0/0/0",
                FighterTyp::UnderworldMinion,
            ),
        ];

        for (record, expected) in cases {
            assert_eq!(parse_fighter(record).typ, expected, "in {record}");
        }
    }

    #[test]
    fn test_fighter_current_life() {
        // A companion dungeon, where the same fighter is
        // carried into later fights at 10% and 1% of its maximum hp, and a
        // fighter that entered a fight at 94%
        let cases = [
            (
                "0/0/0/0/0/333333/PlayerOne \
                 (s1de)/239/3076684/307668/3173/3126/7763/5723/5249/6/101/102/\
                 3/0/5/7/3/0/0/2/2/2/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0",
                3_076_684,
                307_668,
            ),
            (
                "0/0/0/0/0/333333/PlayerOne \
                 (s1de)/239/3076684/28374/3173/3126/7763/5723/5249/6/101/102/\
                 3/0/5/7/3/0/0/2/2/2/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0",
                3_076_684,
                28_374,
            ),
            (
                "2/0/0/0/1/444444/PlayerTwo/257/18549297/17422335/3400/13083/\
                 3400/12290/8286/1/302/303/4/308/1/6/7/1/0/4/1/3/0/0/0/0/0/0/\
                 0/0/0/0/0/0/0/0/0/0/0/0/0",
                18_549_297,
                17_422_335,
            ),
        ];

        for (record, life, current_life) in cases {
            let fighter = parse_fighter(record);
            assert_eq!(fighter.life, life, "in {record}");
            assert_eq!(fighter.current_life, current_life, "in {record}");
        }
    }

    #[test]
    fn test_fighter_state_from_raw() {
        assert_eq!(FighterState::from_raw(0), FighterState::Normal);
        assert_eq!(FighterState::from_raw(10), FighterState::EagleForm);
        assert_eq!(FighterState::from_raw(11), FighterState::BearForm);
        assert_eq!(FighterState::from_raw(20), FighterState::DefensiveStance);
        assert_eq!(FighterState::from_raw(21), FighterState::OffensiveStance);
        assert_eq!(FighterState::from_raw(30), FighterState::Frenzy);
        assert_eq!(FighterState::from_raw(99), FighterState::Unknown(99));
    }

    #[test]
    fn test_update_rounds() {
        let raw = "111111/0/0/4/0/167268/55080/0/0/222222/0/0/0/0/55080/\
                   166066/0/0/111111/0/1/4/0/166066/55080/0/0/222222/0/0/0/0/\
                   55080/164601/0/0/111111/0/11/0/0/164601/55080/1/2/3/4/0/\
                   111111/0/15/0/0/164601/27830/1/2/3/3/0/222222/0/0/0/0/\
                   27830/162633/0/1/2/3/3/111111/0/1/0/0/162633/-6030/1/2/3/3/\
                   0";
        let mut fight = SingleFight::default();
        let data = Vals::from(raw);
        fight.update_rounds(&data, 2);
        assert_all_read(&data);

        assert_eq!(fight.actions.len(), 8);
        assert_eq!(fight.actions[0].outcome, FightOutcome::Evaded);
        assert_eq!(fight.actions[1].acting_id, 222_222);
        assert_eq!(fight.actions[4].action, FightActionType::Summon);
        assert_eq!(
            fight.actions[4].actor_effect,
            Some(ActiveEffect::Minion {
                minion_type: Minion::Golem,
                remaining_actions: 4,
            })
        );
        assert!(fight.actions[4].opponent_effect.is_none());
        assert_eq!(
            fight.actions[6].opponent_effect,
            Some(ActiveEffect::Minion {
                minion_type: Minion::Golem,
                remaining_actions: 3,
            })
        );
        assert!(fight.actions[6].actor_effect.is_none());
        assert_eq!(fight.actions[7].other_new_life, -6030);
    }

    #[test]
    fn test_update_rounds_double_effect() {
        // A single round with two effect blocks: the actor summons a skeleton
        // while the opponent is poisoned. The flag of the second block at
        // index 12 is what distinguishes it from the 12 value stride.
        let raw = "2/0/0/0/0/1000/500/1/2/1/3/0/3/7/2";
        let data = Vals::from(raw);
        let mut fight = SingleFight::default();
        fight.update_rounds(&data, 2);
        assert_all_read(&data);

        assert_eq!(fight.actions.len(), 1);
        let action = &fight.actions[0];
        assert_eq!(action.acting_id, 2);
        assert_eq!(action.actor_life, Some(1000));
        assert_eq!(action.other_new_life, 500);
        assert_eq!(
            action.actor_effect,
            Some(ActiveEffect::Minion {
                minion_type: Minion::Skeleton,
                remaining_actions: 3,
            })
        );
        assert_eq!(
            action.opponent_effect,
            Some(ActiveEffect::Poison {
                id: 7,
                remaining_rounds: 2,
            })
        );
    }

    #[test]
    fn test_update_rounds_unsupported_version() {
        let data = Vals::from("1/0/0/0/0/100/50/0/0");
        let mut fight = SingleFight::default();

        fight.update_rounds(&data, 2);
        assert_eq!(fight.actions.len(), 1);

        // An unsupported version must not keep the previous rounds around
        fight.update_rounds(&data, 1);
        assert_eq!(fight.actions.len(), 0, "stale rounds kept around");
        assert_all_read(&data);
    }

    #[test]
    fn test_update_result_battle_reward() {
        let mut fight = Fight::default();
        let raw = "1/1/0/822/0/100/0/204025/200284/0/0/0/0/0/0/0/0/0/0/0/0";
        let data = Vals::from(raw);
        fight.update_result(&data);
        assert_all_read(&data);

        assert!(fight.has_player_won);
        assert_eq!(fight.xp_change, 822);
        assert_eq!(fight.honor_change, 100);
        assert_eq!(fight.rank_pre_fight, 204_025);
        assert_eq!(fight.rank_post_fight, 200_284);
        assert_eq!(fight.extra, FightExtra::None);
    }

    #[test]
    fn test_update_result_underworld_lure() {
        let mut fight = Fight::default();
        // No capture of `fightresult.underworldpillage` exists,
        // this is built from its documented layout: a short response with the
        // pillaged souls at index 3.
        let data = Vals::from("1/1/0/42/0/0/0/0/0/0/0/0");
        fight.update_result(&data);
        assert_all_read(&data);

        assert!(fight.has_player_won);
        assert_eq!(fight.extra, FightExtra::UnderworldLure { souls: 42 });
        // The battle reward fields are not part of the short format
        assert_eq!(fight.xp_change, 0);
        assert_eq!(fight.rank_pre_fight, 0);
    }

    #[test]
    fn test_update_result_fortress_pillage() {
        let mut fight = Fight::default();
        let raw = "1/1/0/0/0/1/0/1565/1565/0/0/0/0/0/0/0/0/0/0/0/0/1129390/\
                   3438403/1/45/0/0";
        let data = Vals::from(raw);
        fight.update_result(&data);
        assert_all_read(&data);

        assert!(fight.has_player_won);
        let FightExtra::Fortress {
            soldiers,
            stone,
            wood,
            archers_defeated,
            mages_defeated,
        } = fight.extra
        else {
            panic!("expected fortress extras, got {:?}", fight.extra);
        };
        assert_eq!(soldiers, 45);
        assert_eq!(stone, 1_129_390);
        assert_eq!(wood, 3_438_403);
        assert_eq!(archers_defeated, 0);
        assert_eq!(mages_defeated, 0);
    }

    #[test]
    fn test_update_result_fortress_defense_units() {
        let mut fight = Fight::default();
        // A fortress defense, where the units at index 25 & 26 are set
        let raw =
            "1/1/0/0/0/0/0/1533/1533/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/41/30/13";
        let data = Vals::from(raw);
        fight.update_result(&data);
        assert_all_read(&data);

        let FightExtra::Fortress {
            soldiers,
            stone,
            wood,
            archers_defeated,
            mages_defeated,
        } = fight.extra
        else {
            panic!("expected fortress extras, got {:?}", fight.extra);
        };
        assert_eq!(soldiers, 41);
        assert_eq!(stone, 0);
        assert_eq!(wood, 0);
        assert_eq!(archers_defeated, 30);
        assert_eq!(mages_defeated, 13);
    }

    #[test]
    fn test_update_groups() {
        let resp = crate::response::Response::parse(
            "fightgroups.r:111111,222222,PlayerOne,PlayerTwo".to_string(),
            Local::now().naive_local(),
        )
        .unwrap();
        let data = resp.values()["fightgroups"].values_by::<4>(',').unwrap();

        let mut fight = Fight::default();
        fight.update_groups(&data);

        assert_eq!(fight.group_attacker_id, Some(111_111));
        assert_eq!(fight.group_defender_id, Some(222_222));
        assert_eq!(fight.group_attacker_name.as_deref(), Some("PlayerOne"));
        assert_eq!(fight.group_defender_name.as_deref(), Some("PlayerTwo"));
    }
}
