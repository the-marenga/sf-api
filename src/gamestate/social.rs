use std::collections::HashMap;

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::EnumCount;

use super::{
    character::{Mount, Portrait},
    fortress::FortressBuildingType,
    guild::GuildRank,
    items::{Equipment, ItemType},
    unlockables::Mirror,
    AttributeType, Class, Race, ServerTime,
};
use crate::{misc::*, PlayerId};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherPlayers {
    /// The amount of accounts on the server
    pub total_player: u32,
    /// A list of hall of fame players fetched during the last command
    pub hall_of_fame: Vec<HallOfFameEntry>,

    /// The amount of guilds on this server. Will only be set after querying
    /// the guild HOF
    pub total_guilds: Option<u32>,
    /// A list of hall of fame guilds fetched during the last command
    pub guild_hall_of_fame: Vec<HallOfFameGuildEntry>,

    /// The amount of fortresses on this server. Will only be set after
    /// querying the fortress HOF
    pub total_fortresses: Option<u32>,
    /// A list of hall of fame fortresses fetched during the last command
    pub fortress_hall_of_fame: Vec<HallOfFameFortressEntry>,

    /// The amount of players with pets on this server. Will only be set after
    /// querying the pet HOF
    pub total_pet_players: Option<u32>,
    /// A list of hall of fame pet players fetched during the last command
    pub pets_hall_of_fame: Vec<HallOfFamePetsEntry>,

    /// The amount of players with pets on this server. Will only be set after
    /// querying the pet HOF
    pub total_underworld_players: Option<u32>,
    /// A list of hall of fame pet players fetched during the last command
    pub underworld_hall_of_fame: Vec<HallOfFameUnderworldEntry>,

    /// This can be accessed by using the lookup_pid/lookup_name methods
    /// on OtherPlayers
    other_players: HashMap<PlayerId, OtherPlayer>,
    name_lookup: HashMap<String, PlayerId>,

    pub guilds: HashMap<String, OtherGuild>,

    pub combat_log: Vec<CombatLogEntry>,

    pub inbox_capacity: u16,
    pub inbox: Vec<InboxEntry>,
    pub open_msg: Option<String>,

    pub relations: Vec<RelationEntry>,
}

impl OtherPlayers {
    pub(crate) fn insert_lookup(&mut self, other: OtherPlayer) {
        self.name_lookup.insert(other.name.clone(), other.player_id);
        self.other_players.insert(other.player_id, other);
    }

    /// Checks to see if we have queried a player with that player id
    pub fn lookup_pid(&self, pid: PlayerId) -> Option<&OtherPlayer> {
        self.other_players.get(&pid)
    }

    /// Checks to see if we have queried a player with the given name
    pub fn lookup_name(&self, name: &str) -> Option<&OtherPlayer> {
        let other_pos = self.name_lookup.get(name)?;
        self.other_players.get(other_pos)
    }

    /// Removes the information about another player based on their id
    pub fn remove_pid(&mut self, pid: PlayerId) -> Option<OtherPlayer> {
        self.other_players.remove(&pid)
    }

    /// Removes the information about another player based on their name
    pub fn remove_name(&mut self, name: &str) -> Option<OtherPlayer> {
        let other_pos = self.name_lookup.remove(name)?;
        self.other_players.remove(&other_pos)
    }

    pub fn reset_lookups(&mut self) {
        self.other_players = Default::default();
        self.name_lookup = Default::default();
    }

    pub(crate) fn updatete_relation_list(&mut self, val: &str) {
        self.relations.clear();
        for entry in val
            .trim_end_matches(';')
            .split(';')
            .filter(|a| !a.is_empty())
        {
            let mut parts = entry.split(',');
            let (
                Some(id),
                Some(name),
                Some(guild),
                Some(level),
                Some(relation),
            ) = (
                parts.next().and_then(|a| a.parse().ok()),
                parts.next().map(|a| a.to_string()),
                parts.next().map(|a| a.to_string()),
                parts.next().and_then(|a| a.parse().ok()),
                parts.next().and_then(|a| match a {
                    "-1" => Some(Relationship::Ignored),
                    "1" => Some(Relationship::Friend),
                    _ => None,
                }),
            )
            else {
                warn!("bad friendslist entry: {entry}");
                continue;
            };
            self.relations.push(RelationEntry {
                id,
                name,
                guild,
                level,
                relation,
            })
        }
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameEntry {
    pub rank: u32,
    pub name: String,
    pub guild: String,
    pub level: u32,
    pub fame: u32,
    pub class: Class,
    pub flag: String,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameGuildEntry {
    pub rank: u32,
    pub name: String,
    pub leader: String,
    pub member: u32,
    pub honor: u32,
    pub is_attacked: bool,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameFortressEntry {
    pub rank: u32,
    pub name: String,
    pub guild: String,
    pub upgrade: u32,
    pub honor: u32,
    pub unknown: i64,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFamePetsEntry {
    pub rank: u32,
    pub name: String,
    pub guild: String,
    pub collected: u32,
    pub honor: u32,
    pub unknown: i64,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameUnderworldEntry {
    pub rank: u32,
    pub name: String,
    pub guild: String,
    pub upgrade: u32,
    pub honor: u32,
    pub unknown: i64,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherPlayer {
    pub player_id: PlayerId,
    pub name: String,
    pub level: u16,
    pub description: String,
    pub guild_name: String,
    pub mount: Option<Mount>,
    pub portrait: Portrait,

    pub relationship: Relationship,
    pub wall_combat_lvl: u16,

    pub equipment: Equipment,

    pub experience: u64,
    pub next_level_xp: u64,

    pub honor: u32,
    pub rank: u32,
    pub fortress_rank: Option<u32>,
    /// The hp bonus in percent this player has from the personal demon portal
    pub portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    pub portal_dmg_bonus: u32,

    pub base_attributes: EnumMap<AttributeType, u32>,
    pub bonus_attributes: EnumMap<AttributeType, u32>,
    /// This should be the percentage bonus to skills from pets
    pub pet_attribute_bonus_perc: EnumMap<AttributeType, u32>,

    pub class: Class,
    pub race: Race,

    pub mirror: Mirror,

    /// None if they do not have a scrapbook
    pub scrapbook_count: Option<u32>,
    pub active_potions: [Option<ItemType>; 3],
    pub armor: u64,
    pub min_damage_base: u32,
    pub max_damage_base: u32,

    pub fortress: Option<OtherFortress>,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherFortress {
    pub fortress_stone: u64,
    pub fortress_wood: u64,

    pub fortress_archers: u16,
    pub fortress_has_mages: bool,
    pub fortress_soldiers: u16,
    pub fortress_building_level: [u16; FortressBuildingType::COUNT],

    pub wood_in_cutter: u64,
    pub stone_in_quary: u64,
    pub max_wood_in_cutter: u64,
    pub max_stone_in_quary: u64,

    pub fortress_soldiers_lvl: u16,
    pub fortress_mages_lvl: u16,
    pub fortress_archers_lvl: u16,
}

#[derive(Debug, Default, Clone, FromPrimitive, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relationship {
    #[default]
    Ignored = -1,
    Normal = 0,
    Friend = 1,
}

impl OtherPlayer {
    pub(crate) fn update_pet_bonus(&mut self, data: &[u32]) {
        let atr = &mut self.pet_attribute_bonus_perc;
        use crate::command::AttributeType::*;
        // The order of these makes no sense. It is neither pet,
        // nor attribute order
        atr[Constitution] = data[1];
        atr[Dexterity] = data[2];
        atr[Intelligence] = data[3];
        atr[Luck] = data[4];
        atr[Strength] = data[5];
    }

    pub(crate) fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Option<OtherPlayer> {
        let mut op = OtherPlayer::default();
        op.player_id = warning_try_into(data[0], "other player id")?;
        op.level = warning_try_into(data[2], "other level")?;
        op.experience = warning_try_into(data[3], "other exp")?;
        op.next_level_xp = warning_try_into(data[4], "other next lvl exp")?;
        op.honor = warning_try_into(data[5], "other honor")?;
        op.rank = warning_try_into(data[6], "other rank")?;
        op.race = warning_parse(data[18], "other race", |a| {
            FromPrimitive::from_i64(a)
        })?;
        op.portrait = Portrait::parse(&data[8..]).ok()?;
        op.mirror = Mirror::parse(data[19]);
        op.class = FromPrimitive::from_i64(data[20] - 1)?;
        update_enum_map(&mut op.base_attributes, &data[21..]);
        update_enum_map(&mut op.bonus_attributes, &data[26..]);
        op.equipment = Equipment::parse(&data[39..], server_time);
        op.mount = FromPrimitive::from_i64(data[159]);

        if data[163] >= 10000 {
            op.scrapbook_count =
                Some(soft_into(data[163] - 10000, "scrapbook count", 0));
        }

        op.active_potions =
            ItemType::parse_active_potions(&data[194..], server_time);
        op.portal_hp_bonus =
            soft_into(data[252] >> 24, "other portal hp bonus", 0);
        op.portal_dmg_bonus =
            soft_into((data[252] >> 16) & 0xFF, "other portal dmg bonus", 0);

        op.armor = soft_into(data[168], "other armor", 0);
        op.min_damage_base = soft_into(data[169], "other min damage", 0);
        op.max_damage_base = soft_into(data[170], "other max damage", 0);

        if op.level >= 25 {
            let mut fortress = OtherFortress {
                fortress_wood: warning_try_into(data[228], "other s wood")?,
                fortress_stone: warning_try_into(data[229], "other f stone")?,

                fortress_soldiers: soft_into(
                    data[230] & 0xFF,
                    "other f soldiers",
                    0,
                ),
                fortress_has_mages: data[230] >> 16 > 0,
                fortress_archers: soft_into(
                    data[231] & 0xFF,
                    "other f archer",
                    0,
                ),
                wood_in_cutter: soft_into(data[239], "other wood cutter", 0),
                stone_in_quary: soft_into(data[240], "other stone q", 0),
                max_wood_in_cutter: soft_into(data[241], "other max wood c", 0),
                max_stone_in_quary: soft_into(
                    data[242],
                    "other max stone q",
                    0,
                ),
                fortress_soldiers_lvl: soft_into(
                    data[249],
                    "fortress soldiers lvl",
                    0,
                ),
                fortress_mages_lvl: soft_into(
                    data[250],
                    "other f mages lvl",
                    0,
                ),
                fortress_archers_lvl: soft_into(
                    data[251],
                    "other f archer lvl",
                    0,
                ),
                fortress_building_level: Default::default(),
            };
            let end = FortressBuildingType::COUNT;
            for (idx, lvl) in data[208..(end + 208)].iter().enumerate() {
                fortress.fortress_building_level[idx] =
                    soft_into(*lvl, "f build lvl", 0)
            }

            op.fortress = Some(fortress);
        }

        Some(op)
    }
}

#[derive(Debug, Clone, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CombatMessageType {
    Arena = 0,
    Quest = 1,
    GuildFight = 2,
    GuildRaid = 3,
    Dungeon = 4,
    TowerFight = 5,
    LostFight = 6,
    WonFight = 7,
    FortressFight = 8,
    FortressDefense = 9,
    FortressDefenseAlreadyCountered = 109,
    PetAttack = 14,
    PetDefense = 15,
    Underworld = 16,
    GuildFightLost = 26,
    GuildFightWon = 27,
}

#[derive(Debug, Clone, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MessageType {
    Normal,
    GuildInvite,
    GuildKicked,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CombatLogEntry {
    pub msg_id: i64,
    pub player_name: String,
    pub won: bool,
    pub battle_type: CombatMessageType,
    pub time: DateTime<Local>,
}

impl CombatLogEntry {
    pub(crate) fn parse(
        data: &[&str],
        server_time: ServerTime,
    ) -> Option<CombatLogEntry> {
        if data.len() != 6 {
            return None;
        }
        let msg_id = data[0].parse::<i64>().ok()?;
        let battle_t = data[3].parse().ok()?;
        let mt = FromPrimitive::from_i64(battle_t)?;
        let time_stamp = data[4].parse().ok()?;
        let time = server_time.convert_to_local(time_stamp, "combat time")?;

        Some(CombatLogEntry {
            msg_id,
            player_name: data[1].to_string(),
            won: data[2] == "1",
            battle_type: mt,
            time,
        })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InboxEntry {
    pub msg_typ: MessageType,
    pub from: String,
    pub msg_id: i32,
    pub title: String,
    pub date: DateTime<Local>,
    pub read: bool,
}

impl InboxEntry {
    pub(crate) fn parse(
        msg: &str,
        server_time: ServerTime,
    ) -> Option<InboxEntry> {
        let parts = msg.splitn(4, ',').collect::<Vec<_>>();
        if parts.len() != 4 {
            warn!("Bad inbox entry len: {msg:?}");
            return None;
        }
        let Some((title, date)) = parts[3].rsplit_once(',') else {
            warn!("invalid title/date in msg: {msg}");
            return None;
        };

        let msg_typ = match title {
            "3" => MessageType::GuildKicked,
            "5" => MessageType::GuildInvite,
            x if x.chars().all(|a| a.is_ascii_digit()) => {
                warn!("Unknown message typ: {title}");
                return None;
            }
            _ => MessageType::Normal,
        };

        Some(InboxEntry {
            msg_typ,
            date: server_time.convert_to_local(
                warning_from_str(date, "msg date")?,
                "msg date",
            )?,
            from: parts[1].to_string(),
            msg_id: warning_from_str(parts[0], "msg_id")?,
            title: from_sf_string(title.trim_end_matches('\t')),
            read: parts[2] == "1",
        })
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherGuild {
    pub name: String,

    pub attacks: Option<String>,
    pub defends_against: Option<String>,

    pub rank: u16,
    pub attack_cost: u32,
    pub description: String,
    pub emblem: String,
    pub honor: u32,
    pub finished_raids: u16,
    // should just be members.len(), right?
    member_count: u8,
    pub members: Vec<OtherGuildMember>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherGuildMember {
    pub name: String,
    pub instructor_lvl: u16,
    pub treasure_lvl: u16,
    pub rank: GuildRank,
    pub level: u16,
    pub pet_lvl: u16,
    pub last_active: Option<DateTime<Local>>,
}
impl OtherGuild {
    pub(crate) fn update(&mut self, data: &[i64], server_time: ServerTime) {
        self.member_count = soft_into(data[3], "member count", 0);
        let member_count = self.member_count as usize;
        self.finished_raids = soft_into(data[8], "raid count", 0);
        self.honor = soft_into(data[13], "other guild honor", 0);

        self.members.resize_with(member_count, Default::default);

        for (i, member) in &mut self.members.iter_mut().enumerate() {
            member.level =
                soft_into(data[64 + i], "other guild member level", 0);
            member.last_active = server_time
                .convert_to_local(data[114 + i], "other guild member active");
            member.treasure_lvl = soft_into(
                data[214 + i],
                "other guild member treasure levels",
                0,
            );
            member.instructor_lvl = soft_into(
                data[264 + i],
                "other guild member instructor levels",
                0,
            );
            member.rank =
                warning_parse(data[314 + i], "other guild member ranks", |q| {
                    FromPrimitive::from_i64(q)
                })
                .unwrap_or_default();
            member.pet_lvl =
                soft_into(data[390 + i], "other guild pet levels", 0);
        }
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelationEntry {
    pub id: PlayerId,
    pub name: String,
    pub guild: String,
    pub level: u16,
    pub relation: Relationship,
}
