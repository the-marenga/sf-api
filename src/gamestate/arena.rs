use chrono::{DateTime, Local};
use num_traits::FromPrimitive;

use super::{items::*, *};
use crate::PlayerId;

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
    UnderworldLure {
        souls: i64,
    },
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
    /// Raw equipment data for fighter_a. Each entry is 19 values (model_id
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

        // Format: 9 values per round, '/' separated
        //   actor_id / 0 / action_type / outcome / 0 / actor_life / target_life / 0 / 0
        let values: Vec<&str> = data.split('/').collect();
        for chunk in values.chunks(9) {
            if chunk.len() < 9 {
                break;
            }
            let acting_id: i64 = chunk[0].parse().map_err(|_| {
                SFError::ParsingError("action pid", chunk[0].to_string())
            })?;

            let action_type: u32 =
                warning_from_str(chunk[2], "fight action").unwrap_or(0);
            let outcome: u32 =
                warning_from_str(chunk[3], "fight outcome").unwrap_or(0);

            // outcome=3 => blocked, outcome=4 => evaded, otherwise use
            // the action type directly. When combined with action_type=5,
            // these are minion-specific variants.
            let action = match (outcome, action_type) {
                (3, 5) => FightActionType::MinionAttackBlocked,
                (4, 5) => FightActionType::MinionAttackEvaded,
                (3, _) => FightActionType::Blocked,
                (4, _) => FightActionType::Evaded,
                _ => FightActionType::parse(action_type),
            };

            let target_life: i64 = chunk[6].parse().map_err(|_| {
                SFError::ParsingError(
                    "action target life",
                    chunk[6].to_string(),
                )
            })?;
            let actor_life: i64 = chunk[5].parse().map_err(|_| {
                SFError::ParsingError(
                    "action actor life",
                    chunk[5].to_string(),
                )
            })?;

            self.actions.push(FightAction {
                acting_id,
                action,
                other_new_life: target_life,
                actor_life: Some(actor_life),
            });
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

        let name = match data.cget(6, "fighter name").ok()?.parse::<i64>() {
            Ok(-770..=-740) => {
                // This range might be too large
                fighter_type = FighterTyp::FortressWall;
                None
            }
            Ok(-712) => {
                fighter_type = FighterTyp::FortressPillager;
                None
            }
            Ok(-732) => {
                fighter_type = FighterTyp::FortressArcher;
                None
            }
            Ok(-722) => {
                fighter_type = FighterTyp::FortressMage;
                None
            }
            Ok(..=-1) => None,
            Ok(0) => {
                let id = data.cget(15, "fighter uwm").ok()?;
                // No idea if this correct
                if ["-910", "-935", "-933", "-924"].contains(&id) {
                    fighter_type = FighterTyp::UnderworldMinion;
                }
                None
            }
            Ok(pid) if pid == id && fighter_type == FighterTyp::Player => {
                fighter_type = FighterTyp::Pet;
                None
            }
            _ => Some(data.cget(6, "fighter name").ok()?.to_string()),
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

/// One round (action) in a fight. This is mostly just one attack
#[derive(Debug, Clone, Copy)]
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
    /// The life of the acting fighter at the time of this action. Only
    /// available in fight_version >= 2
    pub actor_life: Option<i64>,
}

/// An action in a fight. In the official client this determines the animation,
/// that gets played
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FightActionType {
    /// A simple attack with the normal weapon
    Attack,
    /// One shot from a loaded mushroom catapult in a guild battle
    MushroomCatapult,
    /// The last action was blocked
    Blocked,
    /// The last action was evaded
    Evaded,
    /// The summoned minion attacks
    MinionAttack,
    /// The summoned minion blocked the last attack
    MinionAttackBlocked,
    /// The summoned minion evaded the last attack
    MinionAttackEvaded,
    /// The summoned minion was crit
    MinionCrit,
    /// Plays the harp, or summons a friendly minion
    SummonSpecial,
    /// I have not checked all possible battle types, so whatever action I have
    /// missed will be parsed as this
    Unknown,
}

impl FightActionType {
    pub(crate) fn parse(val: u32) -> FightActionType {
        // FIXME: Is this missing crit?
        match val {
            0 | 1 => FightActionType::Attack,
            2 => FightActionType::MushroomCatapult,
            3 => FightActionType::Blocked,
            4 => FightActionType::Evaded,
            5 => FightActionType::MinionAttack,
            6 => FightActionType::MinionAttackBlocked,
            7 => FightActionType::MinionAttackEvaded,
            25 => FightActionType::MinionCrit,
            200..=250 => FightActionType::SummonSpecial,
            _ => FightActionType::Unknown,
        }
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
    /// A pillager in a fortress attack
    FortressPillager,
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
