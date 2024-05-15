use std::collections::HashMap;

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::IntoEnumIterator;

use super::{
    character::{Mount, Portrait},
    fortress::FortressBuildingType,
    guild::GuildRank,
    items::{Equipment, ItemType},
    unlockables::Mirror,
    AttributeType, Class, Flag, Potion, Race, SFError, ServerTime,
};
use crate::{misc::*, PlayerId};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mail {
    /// All the fights, that the character has stored for some reason
    pub combat_log: Vec<CombatLogEntry>,
    /// The amount of messages the  inbo xcan store
    pub inbox_capacity: u16,
    /// A meessages and notifications
    pub inbox: Vec<InboxEntry>,
    /// If you open a message (via command), this here will contain the openeed
    /// message
    pub open_msg: Option<String>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Contains information about everything involving other players on the server.
/// This mainly revolves around the Hall of Fame
pub struct HallOfFames {
    /// The amount of accounts on the server
    pub total_players: u32,
    /// A list of hall of fame players fetched during the last command
    pub player_hall_of_fame: Vec<HallOfFameEntry>,

    /// The amount of guilds on this server. Will only be set after querying
    /// the guild HoF, or looking at your own guild
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

    /// This can be accessed by using the `lookup_pid()`/`lookup_name()`
    /// methods on `OtherPlayers`
    other_players: HashMap<PlayerId, OtherPlayer>,
    name_lookup: HashMap<String, PlayerId>,

    /// Guild that the character has looked at
    pub other_guilds: HashMap<String, OtherGuild>,
}

impl HallOfFames {
    pub(crate) fn insert_lookup(&mut self, other: OtherPlayer) {
        self.name_lookup.insert(other.name.clone(), other.player_id);
        self.other_players.insert(other.player_id, other);
    }

    /// Checks to see if we have queried a player with that player id
    #[must_use]
    pub fn lookup_pid(&self, pid: PlayerId) -> Option<&OtherPlayer> {
        self.other_players.get(&pid)
    }

    /// Checks to see if we have queried a player with the given name
    #[must_use]
    pub fn lookup_name(&self, name: &str) -> Option<&OtherPlayer> {
        let other_pos = self.name_lookup.get(name)?;
        self.other_players.get(other_pos)
    }

    /// Removes the information about another player based on their id
    #[allow(clippy::must_use_unit)]
    pub fn remove_pid(&mut self, pid: PlayerId) -> Option<OtherPlayer> {
        self.other_players.remove(&pid)
    }

    /// Removes the information about another player based on their name
    #[allow(clippy::must_use_unit)]
    pub fn remove_name(&mut self, name: &str) -> Option<OtherPlayer> {
        let other_pos = self.name_lookup.remove(name)?;
        self.other_players.remove(&other_pos)
    }

    /// Clears out all players, that have previously been queried
    pub fn reset_lookups(&mut self) {
        self.other_players = HashMap::default();
        self.name_lookup = HashMap::default();
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Basic information about one character on the server. To get more
/// information, you need to query this player via the `ViewPlayer` command
pub struct HallOfFameEntry {
    /// The rank of this player
    pub rank: u32,
    /// The name of this player. Used to query more information
    pub name: String,
    /// The guild this player is currently in. If this is None, the player is
    /// not in a guild
    pub guild: Option<String>,
    /// The level of this player
    pub level: u32,
    /// The amount of fame this player has
    pub fame: u32,
    /// The class of this player
    pub class: Class,
    /// The Flag of this player, if they have set any
    pub flag: Option<Flag>,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Basic information about one guild on the server. To get more information,
/// you need to query this player via the `ViewGuild` command
pub struct HallOfFameGuildEntry {
    /// The name of the guild
    pub name: String,
    /// The rank of the guild
    pub rank: u32,
    /// The leader of the guild
    pub leader: String,
    /// The amount of members this guild has
    pub member_count: u32,
    /// The amount of honor this guild has
    pub honor: u32,
    /// Whether or not this guild is already being attacked
    pub is_attacked: bool,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Basic information about one guild on the server
pub struct HallOfFameFortressEntry {
    /// The name of the person, that owns this fort
    pub name: String,
    /// The rank of this fortress in the fortress Hall of Fame
    pub rank: u32,
    /// If the player, that owns this fort is in a guild, this will contain the
    /// guild name
    pub guild: Option<String>,
    /// The amount of upgrades, that have been built in this fortress
    pub upgrade: u32,
    /// The amount of honor this fortress has gained
    pub honor: u32,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Basic information about one players pet collection on the server
pub struct HallOfFamePetsEntry {
    /// The name of the player, that has these pets
    pub name: String,
    /// The rank of this players pet collection
    pub rank: u32,
    /// If the player, that owns these pets is in a guild, this will contain
    /// the guild name
    pub guild: Option<String>,
    /// The amount of pets collected
    pub collected: u32,
    /// The amount of honro this pet collection has gained
    pub honor: u32,
    /// For guilds the value at this position is the attacked status, but no
    /// idea, what it means here
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
/// All information about another player, that was queried via the `ViewPlayer`
/// command
pub struct OtherPlayer {
    /// The id of this player. This is mainly just useful to lookup this player
    /// in `OtherPlayers`, if you do not know the name
    pub player_id: PlayerId,
    /// The name of the player
    pub name: String,
    /// The level of the player
    pub level: u16,
    /// The description this player has set for themselfes
    pub description: String,
    /// If the player is in a guild, this will contain the name
    pub guild: Option<String>,
    /// The mount the player currently ahs rented
    pub mount: Option<Mount>,
    /// Information about the players visual apperarence
    pub portrait: Portrait,
    /// The relation the own character has set towards this player
    pub relationship: Relationship,
    /// The level their fortress wall would have in combat
    pub wall_combat_lvl: u16,
    /// The equipment this player is currently wearing
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
    pub active_potions: [Option<Potion>; 3],
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
    pub fortress_building_level: EnumMap<FortressBuildingType, u16>,

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
    ) -> Result<OtherPlayer, SFError> {
        let mut op = OtherPlayer::default();
        op.player_id = data.ciget(0, "other player id")?;
        op.level = data.ciget(2, "other level")?;
        op.experience = data.ciget(3, "other exp")?;
        op.next_level_xp = data.ciget(4, "other next lvl exp")?;
        op.honor = data.ciget(5, "other honor")?;
        op.rank = data.ciget(6, "other rank")?;
        op.race = data.cfpuget(18, "other race", |a| a)?;
        op.portrait = Portrait::parse(data.skip(8, "other portrait")?)?;
        op.mirror = Mirror::parse(data.cget(19, "other mirror")?);
        op.class = data.cfpuget(20, "other class", |a| a - 1)?;
        update_enum_map(
            &mut op.base_attributes,
            data.skip(21, "other base attrs")?,
        );
        update_enum_map(
            &mut op.bonus_attributes,
            data.skip(26, "other base attrs")?,
        );
        op.equipment =
            Equipment::parse(data.skip(39, "other equipment")?, server_time)?;
        op.mount = data.cfpget(159, "other mount", |x| x)?;

        let sb_count = data.cget(163, "scrapbook count")?;
        if sb_count >= 10000 {
            op.scrapbook_count =
                Some(soft_into(sb_count - 10000, "scrapbook count", 0));
        }

        op.active_potions = ItemType::parse_active_potions(
            data.skip(194, "other potions")?,
            server_time,
        );
        op.portal_hp_bonus =
            data.csimget(252, "other portal hp bonus", 0, |a| a >> 24)?;
        op.portal_dmg_bonus =
            data.csimget(252, "other portal dmg bonus", 0, |a| {
                (a >> 16) & 0xFF
            })?;

        op.armor = data.csiget(168, "other armor", 0)?;
        op.min_damage_base = data.csiget(169, "other min damage", 0)?;
        op.max_damage_base = data.csiget(170, "other max damage", 0)?;

        if op.level >= 25 {
            let mut fortress = OtherFortress {
                fortress_wood: data.ciget(228, "other s wood")?,
                fortress_stone: data.ciget(229, "other f stone")?,
                fortress_soldiers: data.csimget(
                    230,
                    "other f soldiers",
                    0,
                    |a| a & 0xFF,
                )?,
                fortress_has_mages: data[230] >> 16 > 0,
                fortress_archers: soft_into(
                    data[231] & 0xFF,
                    "other f archer",
                    0,
                ),
                wood_in_cutter: data.csiget(239, "other wood cutter", 0)?,
                stone_in_quary: data.csiget(240, "other stone q", 0)?,
                max_wood_in_cutter: data.csiget(241, "other max wood c", 0)?,
                max_stone_in_quary: data.csiget(242, "other max stone q", 0)?,
                fortress_soldiers_lvl: data.csiget(
                    249,
                    "fortress soldiers lvl",
                    0,
                )?,
                fortress_mages_lvl: data.csiget(250, "other f mages lvl", 0)?,
                fortress_archers_lvl: data.csiget(
                    251,
                    "other f archer lvl",
                    0,
                )?,
                fortress_building_level: EnumMap::default(),
            };

            for (pos, typ) in FortressBuildingType::iter().enumerate() {
                *fortress.fortress_building_level.get_mut(typ) =
                    data.csiget(208 + pos, "o f building lvl", 0)?;
            }
            op.fortress = Some(fortress);
        }

        Ok(op)
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
    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.member_count = data.csiget(3, "member count", 0)?;
        let member_count = self.member_count as usize;
        self.finished_raids = data.csiget(8, "raid count", 0)?;
        self.honor = data.csiget(13, "other guild honor", 0)?;

        self.members.resize_with(member_count, Default::default);

        for (i, member) in &mut self.members.iter_mut().enumerate() {
            member.level =
                data.csiget(64 + i, "other guild member level", 0)?;
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
                data.csiget(390 + i, "other guild pet levels", 0)?;
        }
        Ok(())
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
