use chrono::{DateTime, Local};
use num_traits::FromPrimitive;

use super::{items::*, *};
use crate::PlayerId;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Arena {
    /// The enemies currently available in the arena. You have to fetch the
    /// full player info before fighting them, as you need their name
    pub enemy_ids: [PlayerId; 3],
    /// The time at which the player will be able to fight for free again
    pub next_free_fight: Option<DateTime<Local>>,
    /// The amount of fights this character has already done today, that
    /// gave for xp. 0-10
    pub fights_for_xp: u8,
}

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
        self.has_player_won = data[0] != 0;
        self.silver_change = data[2];

        if data.len() < 20 {
            // Skip underworld
            return Ok(());
        }
        self.xp_change = soft_into(data[3], "fight xp", 0);
        self.mushroom_change = soft_into(data[4], "fight mushrooms", 0);
        self.honor_change = data[5];

        self.rank_pre_fight = soft_into(data[7], "fight rank pre", 0);
        self.rank_post_fight = soft_into(data[8], "fight rank post", 0);

        self.item_won = Item::parse(&data[9..], server_time);
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
    pub rounds: Vec<FightRound>,
}

impl SingleFight {
    pub(crate) fn update_fighters(&mut self, data: &str) {
        let data = data.split('/').collect::<Vec<_>>();
        self.fighter_a = Fighter::parse(&data[..47]);
        self.fighter_b = Fighter::parse(&data[47..]);
    }

    pub(crate) fn update_rounds(&mut self, data: &str) -> Result<(), SFError> {
        self.rounds.clear();
        let mut iter = data.split(',');
        while let (Some(player_id), Some(damage_typ), Some(new_life)) =
            (iter.next(), iter.next(), iter.next())
        {
            let action =
                warning_from_str(damage_typ, "fight action").unwrap_or(0);

            self.rounds.push(FightRound {
                attacking_id: player_id.parse().map_err(|_| {
                    SFError::ParsingError("action pid", player_id.to_string())
                })?,
                action: BattleAction::parse(action),
                defender_new_life: new_life.parse().map_err(|_| {
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
pub struct Fighter {
    pub typ: FighterTyp,
    /// The raw id of the fighter. This is <= 0 for monsters & companions and
    /// equivalent to the player id for players (>0)
    pub id: i64,
    /// The name of the player, if fighting a player
    pub name: Option<String>,
    pub level: u32,
    pub life: u32,
    pub attributes: Attributes,
    pub class: Class,
}

impl Fighter {
    pub(crate) fn parse(data: &[&str]) -> Option<Fighter> {
        if data.len() < 28 {
            warn!("Too short fighter response");
            return None;
        }

        let fighter_typ: i64 = warning_from_str(data[5], "fighter typ")?;
        use FighterTyp::*;

        let mut fighter_type = match fighter_typ {
            -391 => Companion(CompanionClass::Warrior),
            -392 => Companion(CompanionClass::Mage),
            -393 => Companion(CompanionClass::Scout),
            1.. => Player,
            x => {
                let monster_id = soft_into(-x, "monster_id", 0);
                Monster(monster_id)
            }
        };

        let mut attributes = Attributes::default();

        let raw_atrs =
            parse_vec(&data[10..15], "fighter attributes", |a| a.parse().ok())
                .ok()?;
        attributes.update(&raw_atrs);

        let class: i32 = warning_from_str(data[27], "fighter class")?;
        let class: Class = FromPrimitive::from_i32(class - 1)?;

        let id = warning_from_str(data[5], "fighter id").unwrap_or_default();

        let name = match data[6].parse::<i64>() {
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
                // No idea if this correct
                if ["-910", "-935", "-933", "-924"].contains(&data[15]) {
                    fighter_type = FighterTyp::UnderworldMinion;
                }
                None
            }
            Ok(_)
                if data[5] == data[6] && fighter_type == FighterTyp::Player =>
            {
                fighter_type = FighterTyp::Pet;
                None
            }
            _ => Some(data[6].to_string()),
        };

        Some(Fighter {
            typ: fighter_type,
            id,
            name,
            level: warning_from_str(data[7], "fighter lvl")?,
            life: warning_from_str(data[8], "fighter life")?,
            attributes,
            class,
        })
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FightRound {
    /// The id of the player, that is active this round
    pub attacking_id: i64,
    /// The new current life of the person, that was hit. Note that this may be
    /// 0 for actions, like spawning minions, that dont have atarget and thus
    /// no target health
    pub defender_new_life: i64,
    /// The action, that the attacking side does
    pub action: BattleAction,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BattleAction {
    Attack,
    MushroomCatapult,
    Blocked,
    Evaded,
    MinionAttack,
    MinionAttackBlocked,
    MinionAttackEvaded,
    MinionCrit,
    /// Plays the harp, or summons a friendly minion
    SummonSpecial,
    Unknown,
}
impl BattleAction {
    pub(crate) fn parse(val: u32) -> BattleAction {
        match val {
            0 | 1 => BattleAction::Attack,
            2 => BattleAction::MushroomCatapult,
            3 => BattleAction::Blocked,
            4 => BattleAction::Evaded,
            5 => BattleAction::MinionAttack,
            6 => BattleAction::MinionAttackBlocked,
            7 => BattleAction::MinionAttackEvaded,
            25 => BattleAction::MinionCrit,
            200..=250 => BattleAction::SummonSpecial,
            _ => BattleAction::Unknown,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FighterTyp {
    #[default]
    Player,
    Monster(u16),
    Companion(CompanionClass),
    FortressPillager,
    FortressWall,
    UnderworldMinion,
    Pet,
}
