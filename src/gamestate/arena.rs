use chrono::{DateTime, Local};
use num_traits::FromPrimitive;

use super::{items::*, *};
use crate::PlayerId;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The arena, that a player can fight other players in
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

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A complete fight, which can be between mutltiple fighters for guild/tower
/// fights
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

    /// My guess is as good as yours as to what this is
    pub fight_version: u8,
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
}

impl Fight {
    pub(crate) fn update_result(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.has_player_won = data.cget(0, "has_player_won")? != 0;
        self.silver_change = data.cget(2, "fight silver change")?;

        if data.len() < 20 {
            // Skip underworld
            return Ok(());
        }

        self.xp_change = data.csiget(3, "fight xp", 0)?;
        self.mushroom_change = data.csiget(4, "fight mushrooms", 0)?;
        self.honor_change = data.cget(5, "fight honor")?;

        self.rank_pre_fight = data.csiget(7, "fight rank pre", 0)?;
        self.rank_post_fight = data.csiget(8, "fight rank post", 0)?;
        let item = data.skip(9, "fight item")?;
        self.item_won = Item::parse(item, server_time);
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

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// This is a single fight between two fighters, which ends when one of them is
/// at <= 0 health
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
        // FIXME: IIRC this should probably be split(data.len() / 2) instead
        let (fighter_a, fighter_b) = data.split_at(47);
        self.fighter_a = Fighter::parse(fighter_a);
        self.fighter_b = Fighter::parse(fighter_b);
    }

    pub(crate) fn update_rounds(&mut self, data: &str) -> Result<(), SFError> {
        self.actions.clear();
        let mut iter = data.split(',');
        while let (Some(player_id), Some(damage_typ), Some(new_life)) =
            (iter.next(), iter.next(), iter.next())
        {
            let action =
                warning_from_str(damage_typ, "fight action").unwrap_or(0);

            self.actions.push(FightAction {
                acting_id: player_id.parse().map_err(|_| {
                    SFError::ParsingError("action pid", player_id.to_string())
                })?,
                action: FightActionType::parse(action),
                other_new_life: new_life.parse().map_err(|_| {
                    SFError::ParsingError(
                        "action new life",
                        player_id.to_string(),
                    )
                })?,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A participant in a fight. Can be anything, that shows up in the battle
/// screen from the player to a fortress Wall
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

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// One round (action) in a fight. This is mostly just one attack
pub struct FightAction {
    /// The id of the fighter, that does the action
    pub acting_id: i64,
    /// The new current life of the fighter, that was hit. Note that this may
    /// be 0 for actions, like spawning minions, that dont have a target
    /// and thus no target health.
    pub other_new_life: i64,
    /// The action, that the active side does
    pub action: FightActionType,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An action in a fight. In the official client this determines the animation,
/// that gets played
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The type of the participant in a fight
pub enum FighterTyp {
    #[default]
    /// Not just the own player, but any player on the server
    Player,
    /// A generic monster, or dungeon boss with its `monster_id`
    Monster(u16),
    /// One of the players companions
    Companion(CompanionClass),
    /// A pillager in a fortress attack
    FortressPillager,
    /// The wall in a fortress attack
    FortressWall,
    /// A minion in an underworld lure battle
    UnderworldMinion,
    /// A pet
    Pet,
}
