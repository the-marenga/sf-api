use std::collections::HashMap;

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{
    AttributeType, Class, Emblem, Flag, Item, Potion, Race, Reward, SFError,
    ServerTime,
    character::{Mount, Portrait},
    guild::GuildRank,
    items::Equipment,
};
use crate::{PlayerId, misc::*};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mail {
    /// All the fights, that the character has stored for some reason
    pub combat_log: Vec<CombatLogEntry>,
    /// The amount of messages the inbox can store
    pub inbox_capacity: u16,
    /// Messages and notifications
    pub inbox: Vec<InboxEntry>,
    /// Items and resources from item codes/twitch drops, that you can claim
    pub claimables: Vec<ClaimableMail>,
    /// If you open a message (via command), this here will contain the opened
    /// message
    pub open_msg: Option<String>,
    /// A preview of a claimable. You can get this via
    /// `Command::ClaimablePreview`
    pub open_claimable: Option<ClaimablePreview>,
}

/// Contains information about everything involving other players on the server.
/// This mainly revolves around the Hall of Fame
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFames {
    /// The amount of accounts on the server
    pub players_total: u32,
    /// A list of hall of fame players fetched during the last command
    pub players: Vec<HallOfFamePlayer>,

    /// The amount of guilds on this server. Will only be set after querying
    /// the guild Hall of Fame, or looking at your own guild
    pub guilds_total: Option<u32>,
    /// A list of hall of fame guilds fetched during the last command
    pub guilds: Vec<HallOfFameGuild>,

    /// The amount of fortresses on this server. Will only be set after
    /// querying the fortress HOF
    pub fortresses_total: Option<u32>,
    /// A list of hall of fame fortresses fetched during the last command
    pub fortresses: Vec<HallOfFameFortress>,

    /// The amount of players with pets on this server. Will only be set after
    /// querying the pet HOF
    pub pets_total: Option<u32>,
    /// A list of hall of fame pet players fetched during the last command
    pub pets: Vec<HallOfFamePets>,

    pub hellevator_total: Option<u32>,
    pub hellevator: Vec<HallOfFameHellevator>,

    /// The amount of players with underworlds on this server. Will only be set
    /// after querying the pet HOF
    pub underworlds_total: Option<u32>,
    /// A list of hall of fame pet players fetched during the last command
    pub underworlds: Vec<HallOfFameUnderworld>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameHellevator {
    pub rank: usize,
    pub name: String,
    pub tokens: u64,
}

/// Contains the results of `ViewGuild` & `ViewPlayer` commands. You can access
/// the player info via functions and the guild data directly
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lookup {
    /// This can be accessed by using the `lookup_pid()`/`lookup_name()`
    /// methods on `Lookup`
    players: HashMap<PlayerId, OtherPlayer>,
    name_to_id: HashMap<String, PlayerId>,

    /// Guild that the character has looked at
    pub guilds: HashMap<String, OtherGuild>,
}

impl Lookup {
    pub(crate) fn insert_lookup(&mut self, other: OtherPlayer) {
        if other.name.is_empty() || other.player_id == 0 {
            warn!("Skipping invalid player insert");
            return;
        }
        self.name_to_id.insert(other.name.clone(), other.player_id);
        self.players.insert(other.player_id, other);
    }

    /// Checks to see if we have queried a player with that player id
    #[must_use]
    pub fn lookup_pid(&self, pid: PlayerId) -> Option<&OtherPlayer> {
        self.players.get(&pid)
    }

    /// Checks to see if we have queried a player with the given name
    #[must_use]
    pub fn lookup_name(&self, name: &str) -> Option<&OtherPlayer> {
        let other_pos = self.name_to_id.get(name)?;
        self.players.get(other_pos)
    }

    /// Removes the information about another player based on their id
    #[allow(clippy::must_use_unit)]
    pub fn remove_pid(&mut self, pid: PlayerId) -> Option<OtherPlayer> {
        self.players.remove(&pid)
    }

    /// Removes the information about another player based on their name
    #[allow(clippy::must_use_unit)]
    pub fn remove_name(&mut self, name: &str) -> Option<OtherPlayer> {
        let other_pos = self.name_to_id.remove(name)?;
        self.players.remove(&other_pos)
    }

    /// Clears out all players, that have previously been queried
    pub fn reset_lookups(&mut self) {
        self.players = HashMap::default();
        self.name_to_id = HashMap::default();
    }
}

/// Basic information about one character on the server. To get more
/// information, you need to query this player via the `ViewPlayer` command
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFamePlayer {
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
    pub honor: u32,
    /// The class of this player
    pub class: Class,
    /// The Flag of this player, if they have set any
    pub flag: Option<Flag>,
}

impl HallOfFamePlayer {
    pub(crate) fn parse(val: &str) -> Result<Self, SFError> {
        let data: Vec<_> = val.split(',').collect();
        let rank = data.cfsuget(0, "hof player rank")?;
        let name = data.cget(1, "hof player name")?.to_string();
        let guild = Some(data.cget(2, "hof player guild")?.to_string())
            .filter(|a| !a.is_empty());
        let level = data.cfsuget(3, "hof player level")?;
        let honor = data.cfsuget(4, "hof player fame")?;
        let class: i64 = data.cfsuget(5, "hof player class")?;
        let Some(class) = FromPrimitive::from_i64(class - 1) else {
            warn!("Invalid hof class: {class} - {data:?}");
            return Err(SFError::ParsingError(
                "hof player class",
                class.to_string(),
            ));
        };

        let raw_flag = data.get(6).copied().unwrap_or_default();
        let flag = Flag::parse(raw_flag);

        Ok(HallOfFamePlayer {
            rank,
            name,
            guild,
            level,
            honor,
            class,
            flag,
        })
    }
}

/// Basic information about one guild on the server. To get more information,
/// you need to query this player via the `ViewGuild` command
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameGuild {
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

impl HallOfFameGuild {
    pub(crate) fn parse(val: &str) -> Result<Self, SFError> {
        let data: Vec<_> = val.split(',').collect();
        let rank = data.cfsuget(0, "hof guild rank")?;
        let name = data.cget(1, "hof guild name")?.to_string();
        let leader = data.cget(2, "hof guild leader")?.to_string();
        let member = data.cfsuget(3, "hof guild member")?;
        let honor = data.cfsuget(4, "hof guild fame")?;
        let attack_status: u8 = data.cfsuget(5, "hof guild atk")?;

        Ok(HallOfFameGuild {
            rank,
            name,
            leader,
            member_count: member,
            honor,
            is_attacked: attack_status == 1u8,
        })
    }
}

impl HallOfFamePets {
    pub(crate) fn parse(val: &str) -> Result<Self, SFError> {
        let data: Vec<_> = val.split(',').collect();
        let rank = data.cfsuget(0, "hof pet rank")?;
        let name = data.cget(1, "hof pet player")?.to_string();
        let guild = Some(data.cget(2, "hof pet guild")?.to_string())
            .filter(|a| !a.is_empty());
        let collected = data.cfsuget(3, "hof pets collected")?;
        let honor = data.cfsuget(4, "hof pets fame")?;
        let unknown = data.cfsuget(5, "hof pets uk")?;

        Ok(HallOfFamePets {
            name,
            rank,
            guild,
            collected,
            honor,
            unknown,
        })
    }
}

impl HallOfFameFortress {
    pub(crate) fn parse(val: &str) -> Result<Self, SFError> {
        let data: Vec<_> = val.split(',').collect();
        let rank = data.cfsuget(0, "hof ft rank")?;
        let name = data.cget(1, "hof ft player")?.to_string();
        let guild = Some(data.cget(2, "hof ft guild")?.to_string())
            .filter(|a| !a.is_empty());
        let upgrade = data.cfsuget(3, "hof ft collected")?;
        let honor = data.cfsuget(4, "hof ft fame")?;

        Ok(HallOfFameFortress {
            name,
            rank,
            guild,
            upgrade,
            honor,
        })
    }
}

impl HallOfFameUnderworld {
    pub(crate) fn parse(val: &str) -> Result<Self, SFError> {
        let data: Vec<_> = val.split(',').collect();
        let rank = data.cfsuget(0, "hof ft rank")?;
        let name = data.cget(1, "hof ft player")?.to_string();
        let guild = Some(data.cget(2, "hof ft guild")?.to_string())
            .filter(|a| !a.is_empty());
        let upgrade = data.cfsuget(3, "hof ft collected")?;
        let honor = data.cfsuget(4, "hof ft fame")?;
        let unknown = data.cfsuget(5, "hof pets uk")?;

        Ok(HallOfFameUnderworld {
            rank,
            name,
            guild,
            upgrade,
            honor,
            unknown,
        })
    }
}

/// Basic information about one guild on the server
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameFortress {
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

/// Basic information about one players pet collection on the server
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFamePets {
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

/// Basic information about one players underworld on the server
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameUnderworld {
    /// The rank this underworld has
    pub rank: u32,
    /// The name of the player, that owns this underworld
    pub name: String,
    /// If the player, that owns this underworld is in a guild, this will
    /// contain the guild name
    pub guild: Option<String>,
    /// The amount of upgrades this underworld has
    pub upgrade: u32,
    /// The amount of honor this underworld has
    pub honor: u32,
    /// For guilds the value at this position is the attacked status, but no
    /// idea, what it means here
    pub unknown: i64,
}

/// All information about another player, that was queried via the `ViewPlayer`
/// command
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherPlayer {
    /// The id of this player. This is mainly just useful to lookup this player
    /// in `Lookup`, if you do not know the name
    pub player_id: PlayerId,
    /// The name of the player
    pub name: String,
    /// The level of the player
    pub level: u16,
    /// The description this player has set for themselves
    pub description: String,
    /// If the player is in a guild, this will contain the name
    pub guild: Option<String>,
    /// The time at which this player joined their guild, if any
    #[deprecated = "v29.500 overhauled the parsing of normal & other players. \
                    This field is not longer available in the new data. As \
                    such, this field may become unavailable at any point, \
                    once the old data is on longer served by the server"]
    pub guild_joined: Option<DateTime<Local>>,
    /// The mount the player currently ahs rented
    pub mount: Option<Mount>,
    /// The time at which the others mount will expire
    pub mount_end: Option<DateTime<Local>>,
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
    /// The hp bonus in percent this player has from the personal demon portal
    pub portal_hp_bonus: u32,
    /// The damage bonus in percent this player has from the guild demon portal
    pub portal_dmg_bonus: u32,
    /// The base level of attributes, if no armor & other bonuses are
    /// considered
    pub attribute_basis: EnumMap<AttributeType, u32>,
    /// The amount of bonus attribuets from equipment & other things
    pub attribute_additions: EnumMap<AttributeType, u32>,
    /// The amount of times the player has manually bought an attribute
    pub attribute_times_bought: EnumMap<AttributeType, u32>,
    /// The bonus to attributes from pets
    pub attribute_pet_bonus: EnumMap<AttributeType, u32>,
    /// The class of this player
    pub class: Class,
    /// The race this player is of
    pub race: Race,
    /// None if they do not have a scrapbook
    pub scrapbook_count: Option<u32>,
    /// The potions this player has currently equipped
    pub active_potions: [Option<Potion>; 3],
    /// The total amount of armor
    pub armor: u64,
    /// The minimum base damage (from their weapon)
    pub min_damage: u32,
    /// The maximum base damage (from their weapon)
    pub max_damage: u32,
    /// All available data about their fortress, if any
    pub fortress: Option<OtherFortress>,
    /// The level of their gladiator in the underworld
    pub gladiator_lvl: u32,
    /// Is the player considered to be a VIP by the game
    pub is_vip: bool,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherFortress {
    /// The total amount of upgrades this player has for their fortress
    pub upgrade_count: u32,
    /// The amount of soldiers suggested to use when attacking this players
    /// fortress
    pub soldier_advice: u16,
    /// The amount of stone we are expected to gain from raiding this players
    /// fortress
    pub lootable_wood: u64,
    /// The amount of stone we are expected to gain from raiding this players
    /// fortress
    pub lootable_stone: u64,
    /// The amount of archers defending this players fortress
    pub archer_count: u16,
    /// The amount of mages defending this players fortress
    pub mage_count: u16,
    /// The rank this player has achieved in the fortress
    pub rank: u32,
}

#[derive(Debug, Default, Clone, FromPrimitive, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relationship {
    #[default]
    Ignored = -1,
    Normal = 0,
    Friend = 1,
}

impl OtherPlayer {
    pub(crate) fn update_pet_bonus(
        &mut self,
        data: &[u32],
    ) -> Result<(), SFError> {
        let atr = &mut self.attribute_pet_bonus;
        // The order of these makes no sense. It is neither pet,
        // nor attribute order
        *atr.get_mut(AttributeType::Constitution) = data.cget(1, "pet con")?;
        *atr.get_mut(AttributeType::Dexterity) = data.cget(2, "pet dex")?;
        *atr.get_mut(AttributeType::Intelligence) = data.cget(3, "pet int")?;
        *atr.get_mut(AttributeType::Luck) = data.cget(4, "pet luck")?;
        *atr.get_mut(AttributeType::Strength) = data.cget(5, "pet str")?;
        Ok(())
    }

    pub(crate) fn update_fortress(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        let ft = self.fortress.get_or_insert_default();
        ft.upgrade_count = data.csiget(0, "other ft upgrades", 0)?;
        ft.soldier_advice = data.csiget(1, "other soldier advice", 0)?;
        ft.mage_count = data.csiget(2, "other mage count", 0)?;
        ft.archer_count = data.csiget(3, "other soldier advice", 0)?;
        ft.lootable_wood = data.csiget(4, "other lootable wood", 0)?;
        ft.lootable_stone = data.csiget(5, "other lootable stone", 0)?;
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        // 0
        self.player_id = data.csiget(1, "player id", 0)?;
        // 0
        self.level = data.csimget(3, "level", 0, |a| a & 0xFFFF)?;
        self.experience = data.csiget(4, "experience", 0)?;
        self.next_level_xp = data.csiget(5, "xp to next lvl", 0)?;
        self.honor = data.csiget(6, "honor", 0)?;
        self.rank = data.csiget(7, "rank", 0)?;
        self.portrait =
            Portrait::parse(data.skip(8, "portrait")?).unwrap_or_default();
        //////// portrait
        // 4
        // 206
        // 203
        // 2
        // 0
        // 2
        // 7
        // 2
        // 0
        // 0
        self.race = data.cfpuget(18, "char race", |a| a)?;
        // 2
        //////
        self.class = data.cfpuget(20, "character class", |a| a - 1)?;
        self.mount = data.cfpget(21, "character mount", |a| a & 0xFF)?;
        // 3
        // 0
        self.armor = data.csiget(23, "total armor", 0)?;
        self.min_damage = data.csiget(24, "min damage", 0)?;
        self.max_damage = data.csiget(25, "max damage", 0)?;
        self.portal_dmg_bonus = data.cimget(26, "portal dmg bonus", |a| a)?;
        // 4280492      // ???
        self.portal_hp_bonus = data.csimget(28, "portal hp bonus", 0, |a| a)?;
        self.mount_end = data.cstget(29, "mount end", server_time)?;
        update_enum_map(
            &mut self.attribute_basis,
            data.skip(30, "char attr basis")?,
        );
        update_enum_map(
            &mut self.attribute_additions,
            data.skip(35, "char attr adds")?,
        );
        update_enum_map(
            &mut self.attribute_times_bought,
            data.skip(40, "char attr tb")?,
        );
        // 0
        // 0
        // 0
        // 0
        // 0
        // 17
        // 0
        // 0
        // 0
        // 66
        // 0
        // 7
        // 18
        // 0
        // 0
        // 0
        // 0
        // 0
        // 0
        // 0

        // 80315 // guild id
        let sb_count = data.cget(66, "scrapbook count")?;
        if sb_count >= 10000 {
            self.scrapbook_count =
                Some(soft_into(sb_count - 10000, "scrapbook count", 0));
        }
        // 0
        // 31
        self.gladiator_lvl = data.csiget(69, "gladiator lvl", 0)?;

        Ok(())
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
    ShadowWorld = 12,
    FortressDefenseAlreadyCountered = 109,
    PetAttack = 14,
    PetDefense = 15,
    Underworld = 16,
    Twister = 25,
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
    ) -> Result<CombatLogEntry, SFError> {
        let msg_id = data.cfsuget(0, "combat msg_id")?;
        let battle_t: i64 = data.cfsuget(3, "battle t")?;
        let time_stamp: i64 = data.cfsuget(4, "combat log time")?;
        let time = server_time
            .convert_to_local(time_stamp, "combat time")
            .ok_or_else(|| {
                SFError::ParsingError("combat time", time_stamp.to_string())
            })?;

        let mt = FromPrimitive::from_i64(battle_t).ok_or_else(|| {
            SFError::ParsingError("combat mt", format!("{battle_t} @ {time:?}"))
        })?;

        Ok(CombatLogEntry {
            msg_id,
            player_name: data.cget(1, "clog player")?.to_string(),
            won: data.cget(2, "clog won")? == "1",
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
    ) -> Result<InboxEntry, SFError> {
        let parts = msg.splitn(4, ',').collect::<Vec<_>>();
        let Some((title, date)) =
            parts.cget(3, "msg title/date")?.rsplit_once(',')
        else {
            return Err(SFError::ParsingError(
                "title/msg comma",
                msg.to_string(),
            ));
        };

        let msg_typ = match title {
            "3" => MessageType::GuildKicked,
            "5" => MessageType::GuildInvite,
            x if x.chars().all(|a| a.is_ascii_digit()) => {
                return Err(SFError::ParsingError(
                    "msg typ",
                    title.to_string(),
                ));
            }
            _ => MessageType::Normal,
        };

        let Some(date) = date
            .parse()
            .ok()
            .and_then(|a| server_time.convert_to_local(a, "msg_date"))
        else {
            return Err(SFError::ParsingError("msg date", date.to_string()));
        };

        Ok(InboxEntry {
            msg_typ,
            date,
            from: parts.cget(1, "inbox from")?.to_string(),
            msg_id: parts.cfsuget(0, "msg_id")?,
            title: from_sf_string(title.trim_end_matches('\t')),
            read: parts.cget(2, "inbox read")? == "1",
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
    pub emblem: Emblem,
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
        val: &str,
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        let data: Vec<_> = val
            .split('/')
            .map(|c| c.trim().parse::<i64>().unwrap_or_default())
            .collect();

        self.member_count = data.csiget(3, "member count", 0)?;
        let member_count = self.member_count as usize;
        self.finished_raids = data.csiget(8, "raid count", 0)?;
        self.honor = data.csiget(13, "other guild honor", 0)?;

        self.members.resize_with(member_count, Default::default);

        for (i, member) in &mut self.members.iter_mut().enumerate() {
            member.level =
                data.csiget(64 + i, "other guild member level", 0)?;
            member.last_active =
                data.cstget(114 + i, "other guild member active", server_time)?;
            member.treasure_lvl =
                data.csiget(214 + i, "other guild member treasure levels", 0)?;
            member.instructor_lvl = data.csiget(
                264 + i,
                "other guild member instructor levels",
                0,
            )?;
            member.rank = data
                .cfpget(314 + i, "other guild member ranks", |q| q)?
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClaimableMail {
    pub msg_id: i64,
    pub typ: ClaimableMailType,
    pub status: ClaimableStatus,
    pub name: String,
    pub received: Option<DateTime<Local>>,
    pub claimable_until: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClaimableStatus {
    Unread,
    Read,
    Claimed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClaimableMailType {
    Coupon = 10,
    SupermanDelivery = 11,
    TwitchDrop = 12,
    #[default]
    GenericDelivery,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClaimablePreview {
    pub items: Vec<Item>,
    pub resources: Vec<Reward>,
}
