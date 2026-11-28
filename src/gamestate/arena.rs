use chrono::{DateTime, Local};

use super::{items::*, *};
use crate::{PlayerId, response::Vals};

const FIGHTER_RECORD_LEN: usize = 47;
const ROUND_LEN_PLAIN: usize = 9;
const ROUND_LEN_ONE_EFFECT: usize = 12;
const ROUND_LEN_TWO_EFFECTS: usize = 15;

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

impl Arena {
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        self.next_free_fight = data.server_time_opt(0, server_time);
        self.fights_for_xp = data.u8_or(1, 0);
        for (idx, enemy) in self.enemy_ids.iter_mut().enumerate() {
            *enemy = data.u32_or(2 + idx, 0);
        }
        data.ignore(5, "unknown arena value");
        data.ignore(6, "unknown arena value");
    }
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
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        self.has_player_won = data.i64_or(0, 0) != 0;
        self.silver_change = data.i64_or(2, 0);

        // Underworld lure (fightresult.underworldpillage) — short format
        if data.len() < 20 {
            self.extra = FightExtra::UnderworldLure {
                souls: data.i64_or(3, 0),
            };
            data.ignore(1, "unmodelled fight result flag");
            data.ignore(4..data.len(), "unmodelled lure fields");
            return Ok(());
        }

        self.xp_change = data.u64_or(3, 0);
        self.mushroom_change = data.u8_or(4, 0);
        self.honor_change = data.i64_or(5, 0);

        self.rank_pre_fight = data.u32_or(7, 0);
        self.rank_post_fight = data.u32_or(8, 0);
        let item_end = (9 + ITEM_PARSE_LEN).min(data.len());
        self.item_won = Item::parse(&data.window(9..item_end))?;

        // Extended fortress fight data (fightresult.fortresspillagerv1)
        if data.len() >= 25 {
            self.extra = FightExtra::Fortress {
                soldiers: data.u32_or(24, 0),
                stone: data.i64_or(21, 0),
                wood: data.i64_or(22, 0),
                archers_defeated: data.u32_or(25, 0),
                mages_defeated: data.u32_or(26, 0),
            };
        }

        data.ignore(1, "unmodelled fight result flag");
        data.ignore(6, "unmodelled fight result value");
        data.ignore(28..data.len(), "unmodelled fight result tail");
        Ok(())
    }

    pub(crate) fn update_groups(&mut self, data: &Vals<'_>) {
        let (Some(aid), Some(did), Some(aname), Some(dname)) = (
            data.u32(0).ok(),
            data.u32(1).ok(),
            data.str(2).ok(),
            data.str(3).ok(),
        ) else {
            warn!("Invalid fight group: {data}");
            return;
        };

        self.group_attacker_id = Some(aid);
        self.group_defender_id = Some(did);
        self.group_attacker_name = Some(aname.to_string());
        self.group_defender_name = Some(dname.to_string());
        data.ignore(4..data.len(), "extra fight group values");
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
    /// The action this fight involved
    pub actions: Vec<FightAction>,
}

impl SingleFight {
    pub(crate) fn update_fighters(&mut self, data: &Vals<'_>) {
        if data.len() < 60 {
            self.fighter_a = None;
            self.fighter_b = None;
            warn!("Fighter response too short");
            data.ignore_all("fighter response too short");
            return;
        }

        let second = FIGHTER_RECORD_LEN..FIGHTER_RECORD_LEN * 2;
        self.fighter_a =
            Fighter::parse(&data.window(0..FIGHTER_RECORD_LEN)).ok();
        self.fighter_b = Fighter::parse(&data.window(second)).ok();

        data.ignore(
            FIGHTER_RECORD_LEN * 2..FIGHTER_RECORD_LEN * 2 + 5,
            "trailing fighter header values",
        );
        data.ignore(
            FIGHTER_RECORD_LEN * 2 + 5..data.len(),
            "extra fighter header values",
        );
    }

    pub(crate) fn update_rounds(
        &mut self,
        data: &Vals<'_>,
        fight_version: u32,
    ) {
        self.actions.clear();

        if fight_version != 2 {
            data.ignore_all("unsupported fight version");
            return;
        }

        let mut i = 0;
        while i + ROUND_LEN_PLAIN <= data.len() {
            let extras_first = data.i64_or(i + 7, 0);
            let extras_second = data.i64_or(i + 8, 0);

            let stride = if extras_first == 0 && extras_second == 0 {
                ROUND_LEN_PLAIN
            } else if i + ROUND_LEN_TWO_EFFECTS <= data.len()
                && matches!(data.i64_or(i + ROUND_LEN_ONE_EFFECT, 0), 1..=3)
            {
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
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Fighter, SFError> {
        data.ignore_unmodelled(
            &[0..5, 9..10, 16..27, 28..FIGHTER_RECORD_LEN],
            "fighter field that is not modelled",
        );

        let name_fallback = data.get_str(15).unwrap_or_default();
        let raw_id = data.i64_or(5, 0);
        let mut fighter_type = match raw_id {
            -391 => FighterTyp::Companion(CompanionClass::Warrior),
            -392 => FighterTyp::Companion(CompanionClass::Mage),
            -393 => FighterTyp::Companion(CompanionClass::Scout),
            1.. => FighterTyp::Player,
            x => {
                let monster_id = x
                    .checked_neg()
                    .and_then(|id| u16::try_from(id).ok())
                    .unwrap_or_else(|| {
                        warn!("Invalid monster id: {x}");
                        0
                    });
                FighterTyp::Monster(monster_id)
            }
        };

        let mut attributes = EnumMap::default();
        for (idx, attr) in attributes.values_mut().enumerate() {
            *attr = data.u32_or(10 + idx, 0);
        }

        let class =
            data.enum_map(27, |raw| raw.wrapping_sub(1))
                .ok_or_else(|| {
                    SFError::ParsingError(
                        "fighter class",
                        format!(
                            "Unknown fighter class: {}",
                            data.u32_or(27, 0)
                        ),
                    )
                })?;

        let raw_name = data.str_or(6, "");
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
                if ["-910", "-935", "-933", "-924"].contains(&name_fallback) {
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
            level: data.u32_or(7, 0),
            life: data.u32_or(8, 0),
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
                _ => return None,
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
/// into active effects.
fn parse_active_effect(
    extras: &Vals<'_>,
) -> (Option<ActiveEffect>, Option<ActiveEffect>) {
    if extras.len() < 4 {
        return (None, None);
    }

    if extras.len() >= 8 {
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
        (parse_one_effect(&extras.window(1..4)), None)
    } else {
        (None, parse_one_effect(&extras.window(2..5)))
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    fn fighter_record(id: i64, name: &str, fallback: i64) -> String {
        let mut values = vec!["0".to_string(); FIGHTER_RECORD_LEN];
        values[5] = id.to_string();
        values[6] = name.to_string();
        values[7] = "50".to_string();
        values[8] = "1000".to_string();
        values[10] = "10".to_string();
        values[15] = fallback.to_string();
        values[27] = "1".to_string();
        values.join("/")
    }

    #[test]
    fn updates_arena_and_tolerates_unknown_strings() {
        let data = Vals::from("1785018466/0/111111/222222/333333/1/1");
        let mut arena = Arena::default();
        arena.update(&data, ServerTime::default());
        data.assert_all_read();
        assert_eq!(arena.enemy_ids, [111_111, 222_222, 333_333]);
        assert!(arena.next_free_fight.is_some());

        let data =
            Vals::from("future/future/future/future/future/future/future");
        arena.update(&data, ServerTime::default());
        data.assert_all_read();
        assert_eq!(arena.enemy_ids, [0, 0, 0]);
    }

    #[test]
    fn updates_fighter_records() {
        let first = fighter_record(111_111, "PlayerOne", 0);
        let second = fighter_record(222_222, "PlayerTwo", 0);
        let raw = format!("{first}/{second}/0/0/0/0/0");
        let data = Vals::from(raw.as_str());
        let mut fight = SingleFight::default();
        fight.update_fighters(&data);
        data.assert_all_read();

        let a = fight.fighter_a.unwrap();
        assert_eq!(a.typ, FighterTyp::Player);
        assert_eq!(a.id, 111_111);
        assert_eq!(a.name.as_deref(), Some("PlayerOne"));
        assert_eq!(a.level, 50);
        assert_eq!(a.life, 1000);
        assert_eq!(a.class, Class::Warrior);
        assert_eq!(a.attributes[AttributeType::Strength], 10);

        let b = fight.fighter_b.unwrap();
        assert_eq!(b.id, 222_222);
        assert_eq!(b.name.as_deref(), Some("PlayerTwo"));
    }

    #[test]
    fn parses_special_fighter_types() {
        let cases = [
            (
                -391,
                "-391",
                0,
                FighterTyp::Companion(CompanionClass::Warrior),
            ),
            (712, "-712", 0, FighterTyp::FortressSoldier),
            (111_111, "111111", 0, FighterTyp::Pet),
            (0, "0", 0, FighterTyp::Monster(0)),
            (0, "0", -910, FighterTyp::UnderworldMinion),
        ];
        for (id, name, fallback, expected) in cases {
            let raw = fighter_record(id, name, fallback);
            let data = Vals::from(raw.as_str());
            assert_eq!(Fighter::parse(&data).unwrap().typ, expected);
            data.assert_all_read();
        }
    }

    #[test]
    fn updates_rounds_and_effects() {
        let raw = "111111/0/0/4/0/167268/55080/0/0/222222/0/0/0/0/55080/\
                   166066/0/0/111111/0/1/4/0/166066/55080/0/0/222222/0/0/0/0/\
                   55080/164601/0/0/111111/0/11/0/0/164601/55080/1/2/3/4/0/\
                   111111/0/15/0/0/164601/27830/1/2/3/3/0/222222/0/0/0/0/\
                   27830/162633/0/1/2/3/3/111111/0/1/0/0/162633/-6030/1/2/3/3/\
                   0";
        let data = Vals::from(raw);
        let mut fight = SingleFight::default();
        fight.update_rounds(&data, 2);
        data.assert_all_read();

        assert_eq!(fight.actions.len(), 8);
        assert_eq!(fight.actions[0].outcome, FightOutcome::Evaded);
        assert_eq!(fight.actions[4].action, FightActionType::Summon);
        assert!(matches!(
            fight.actions[4].actor_effect,
            Some(ActiveEffect::Minion {
                minion_type: Minion::Golem,
                remaining_actions: 4,
            })
        ));
        assert_eq!(fight.actions[7].other_new_life, -6030);
    }

    #[test]
    fn updates_double_effect_rounds() {
        let data = Vals::from("2/0/0/0/0/1000/500/1/2/1/3/0/3/7/2");
        let mut fight = SingleFight::default();
        fight.update_rounds(&data, 2);
        data.assert_all_read();

        assert_eq!(fight.actions.len(), 1);
        let action = &fight.actions[0];
        assert!(matches!(
            action.actor_effect,
            Some(ActiveEffect::Minion {
                minion_type: Minion::Skeleton,
                remaining_actions: 3,
            })
        ));
        assert!(matches!(
            action.opponent_effect,
            Some(ActiveEffect::Poison {
                id: 7,
                remaining_rounds: 2,
            })
        ));
    }

    #[test]
    fn unsupported_round_versions_clear_and_ignore_data() {
        let data = Vals::from("1/0/0/0/0/100/50/0/0");
        let mut fight = SingleFight::default();
        fight.update_rounds(&data, 2);
        assert_eq!(fight.actions.len(), 1);
        fight.update_rounds(&data, 1);
        assert_eq!(fight.actions.len(), 0);
        data.assert_all_read();
    }

    #[test]
    fn updates_fight_groups() {
        let data = Vals::by("111111,222222,PlayerOne,PlayerTwo", ',');
        let mut fight = Fight::default();
        fight.update_groups(&data);
        data.assert_all_read();
        assert_eq!(fight.group_attacker_id, Some(111_111));
        assert_eq!(fight.group_defender_id, Some(222_222));
        assert_eq!(fight.group_attacker_name.as_deref(), Some("PlayerOne"));
        assert_eq!(fight.group_defender_name.as_deref(), Some("PlayerTwo"));
    }

    #[test]
    fn parses_a_short_lure_result() {
        let data = Vals::from("1/0/-10/5");
        let mut fight = Fight::default();
        fight.update_result(&data).unwrap();

        assert!(fight.has_player_won);
        assert_eq!(fight.silver_change, -10);
        assert!(matches!(
            fight.extra,
            FightExtra::UnderworldLure { souls: 5 }
        ));
        data.assert_all_read();
    }

    #[test]
    fn malformed_fight_result_numbers_use_defaults() {
        let mut values = vec!["0"; 20];
        values[0] = "1";
        values[2] = "-10";
        values[3] = "invalid";
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut fight = Fight::default();
        fight.update_result(&data).unwrap();
        assert_eq!(fight.xp_change, 0);
        data.assert_all_read();
    }

    #[test]
    fn truncated_fight_result_items_use_defaults() {
        let raw = "1/0/-10/100/2/3/0/4/5/1/0/0/1010/0/149/265/5/4/0/108/72/0/\
                   83940/10/0/0/45";
        let data = Vals::from(raw);
        let mut fight = Fight::default();
        fight.update_result(&data).unwrap();
        let item = fight.item_won.as_ref().unwrap();
        assert_eq!(item.price, 83_940);
        assert!(!item.is_washed);
        data.assert_all_read();
    }

    #[test]
    fn invalid_fortress_counts_default_to_zero() {
        let mut values = vec!["0"; 28];
        values[0] = "1";
        values[24] = "-1";
        values[25] = "-1";
        values[26] = "-1";
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut fight = Fight::default();
        fight.update_result(&data).unwrap();

        let FightExtra::Fortress {
            soldiers,
            archers_defeated,
            mages_defeated,
            ..
        } = fight.extra
        else {
            panic!("expected fortress fight metadata");
        };
        assert_eq!(soldiers, 0);
        assert_eq!(archers_defeated, 0);
        assert_eq!(mages_defeated, 0);
        data.assert_all_read();
    }

    #[test]
    fn parses_the_item_from_a_battle_result() {
        let raw = "1/0/-10/100/2/3/0/4/5/1/0/0/1010/0/149/265/5/4/0/108/72/0/\
                   83940/10/0/0/45/0";
        let data = Vals::from(raw);
        let mut fight = Fight::default();
        fight.update_result(&data).unwrap();

        assert_eq!(fight.xp_change, 100);
        assert_eq!(fight.mushroom_change, 2);
        assert_eq!(fight.honor_change, 3);
        assert_eq!(fight.rank_pre_fight, 4);
        assert_eq!(fight.rank_post_fight, 5);
        assert_eq!(fight.item_won.as_ref().unwrap().price, 83_940);
        data.assert_all_read();
    }
}
