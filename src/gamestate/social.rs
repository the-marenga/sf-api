use std::collections::HashMap;

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::warn;
use num_derive::FromPrimitive;

use super::{
    AttributeType, Class, Emblem, Flag, Item, Potion, Race, Result, Reward,
    ServerTime,
    character::{Mount, Portrait},
    guild::GuildRank,
    items::{Equipment, ITEM_PARSE_LEN},
    rewards::RewardType,
};
use crate::{PlayerId, error::SFError, misc::from_sf_string, response::Vals};

/// Information about the character's mailbox, combat log, and claimable
/// rewards.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mail {
    /// All the fights, that the character has stored for some reason
    pub combat_log: Vec<CombatLogEntry>,
    /// The amount of messages the inbox can store
    pub inbox_capacity: u16,
    /// Messages and notifications
    pub inbox: Vec<InboxEntry>,
    /// News from the server
    pub news_inbox: Vec<NewsEntry>,
    /// Items and resources from item codes/twitch drops, that you can claim
    pub claimables: Vec<ClaimableMail>,
    /// If you open a message, or news entry (via command), this here will
    /// contain the opened message
    pub open_msg: Option<String>,
    /// A preview of a claimable. You can get this via
    /// `Command::ClaimablePreview`
    pub open_claimable: Option<ClaimablePreview>,
}

impl Mail {
    /// Updates news entries from a `newslist` response.
    pub(crate) fn update_news_inbox(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.news_inbox.clear();
        for msg in data.iter().filter(|a| !a.trim().is_empty()) {
            match NewsEntry::parse(&Vals::by(msg, ','), server_time) {
                Ok(entry) => self.news_inbox.push(entry),
                Err(e) => warn!("Invalid news msg: {msg} {e}"),
            }
        }
    }

    /// Updates inbox messages from an `inbox` response.
    pub(crate) fn update_inbox(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.inbox.clear();
        for msg in data.iter().filter(|a| !a.trim().is_empty()) {
            match InboxEntry::parse(&Vals::by(msg, ','), server_time) {
                Ok(entry) => self.inbox.push(entry),
                Err(e) => warn!("Invalid inbox msg: {msg} {e}"),
            }
        }
    }

    /// Updates combat log entries from a `combatloglist` response.
    pub(crate) fn update_combat_log(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.combat_log.clear();
        for entry in data.iter() {
            if entry.is_empty() || entry.split(',').all(str::is_empty) {
                continue;
            }
            let entry_data = Vals::by(entry, ',');
            match CombatLogEntry::parse(&entry_data, server_time) {
                Ok(cle) => self.combat_log.push(cle),
                Err(e) => {
                    warn!("Unable to parse combat log entry: {entry} - {e}");
                }
            }
        }
    }

    /// Updates claimable deliveries from a `pendingrewards` response.
    pub(crate) fn update_claimables(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.claimables = data
            .chunks(6)
            .filter_map(|chunk| ClaimableMail::parse(&chunk, server_time).ok())
            .collect();
    }
}

/// Contains information about everything involving other players on the server.
/// This mainly revolves around the Hall of Fame
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

    /// The amount of players in the Hellevator ranking on this server, if
    /// queried
    pub hellevator_total: Option<u32>,
    /// A list of Hellevator rankings fetched during the last command
    pub hellevator: Vec<HallOfFameHellevator>,

    /// The amount of players with underworlds on this server. Will only be set
    /// after querying the underworld HOF
    pub underworlds_total: Option<u32>,
    /// A list of hall of fame underworld players fetched during the last
    /// command
    pub underworlds: Vec<HallOfFameUnderworld>,
}

impl HallOfFames {
    /// Updates player ranking from a `ranklistplayer` response.
    pub(crate) fn update_players(&mut self, data: &Vals<'_>) {
        self.players.clear();
        for player in data.iter() {
            if player.is_empty() || player.ends_with(",,,0,0,0,") {
                break;
            }
            match HallOfFamePlayer::parse(&Vals::by(player, ',')) {
                Ok(x) => self.players.push(x),
                Err(e) => warn!("Invalid hall of fame entry: {player} {e}"),
            }
        }
    }

    /// Updates guild ranking from a `ranklistgroup` response.
    pub(crate) fn update_guilds(&mut self, data: &Vals<'_>) {
        self.guilds.clear();
        for guild in data.iter() {
            if guild.is_empty() {
                continue;
            }
            match HallOfFameGuild::parse(&Vals::by(guild, ',')) {
                Ok(x) => self.guilds.push(x),
                Err(e) => warn!("Invalid hall of fame entry: {guild} {e}"),
            }
        }
    }

    /// Updates pet ranking from a `RanklistPets` response.
    pub(crate) fn update_pets(&mut self, data: &Vals<'_>) {
        self.pets.clear();
        for entry in data.iter() {
            if entry.is_empty() {
                continue;
            }
            match HallOfFamePets::parse(&Vals::by(entry, ',')) {
                Ok(x) => self.pets.push(x),
                Err(e) => warn!("Invalid hall of fame entry: {entry} {e}"),
            }
        }
    }

    /// Updates fortress ranking from a `ranklistfortress` response.
    pub(crate) fn update_fortresses(&mut self, data: &Vals<'_>) {
        self.fortresses.clear();
        for fortress in data.iter() {
            if fortress.is_empty() {
                continue;
            }
            match HallOfFameFortress::parse(&Vals::by(fortress, ',')) {
                Ok(x) => self.fortresses.push(x),
                Err(e) => warn!("Invalid hall of fame entry: {fortress} {e}"),
            }
        }
    }

    /// Updates underworld ranking from a `ranklistunderworld` response.
    pub(crate) fn update_underworlds(&mut self, data: &Vals<'_>) {
        self.underworlds.clear();
        for entry in data.iter() {
            if entry.is_empty() {
                continue;
            }
            match HallOfFameUnderworld::parse(&Vals::by(entry, ',')) {
                Ok(x) => self.underworlds.push(x),
                Err(e) => warn!("Invalid hall of fame entry: {entry} {e}"),
            }
        }
    }

    /// Updates hellevator ranking from a `ranklisthellevator` response.
    pub(crate) fn update_hellevator(&mut self, data: &Vals<'_>) {
        self.hellevator = data
            .iter()
            .filter(|a| !a.is_empty())
            .filter_map(|chunk| {
                HallOfFameHellevator::parse(&Vals::by(chunk, ',')).ok()
            })
            .collect();
    }
}

/// A Hellevator Hall of Fame ranking entry
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HallOfFameHellevator {
    /// The rank in the Hellevator Hall of Fame
    pub rank: usize,
    /// The player's name
    pub name: String,
    /// The amount of tokens collected
    pub tokens: u64,
}

impl HallOfFameHellevator {
    /// Parses a single Hellevator Hall of Fame entry `[rank, name, tokens]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(HallOfFameHellevator {
            rank: data.usize(0)?,
            name: data.str(1)?.to_string(),
            tokens: data.u64(2)?,
        })
    }
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
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
    /// Parses a Hall of Fame player entry from a comma-separated record:
    /// `rank, name, guild, level, honor, class, flag`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        let class =
            data.enum_map(5, |raw| raw.wrapping_sub(1)).ok_or_else(|| {
                SFError::ParsingErrorV2(format!(
                    "Invalid hof class: {}",
                    data.str_or(5, "")
                ))
            })?;

        let raw_flag = data.get_str(6).unwrap_or_default();
        let flag = Flag::parse(raw_flag);

        Ok(HallOfFamePlayer {
            rank: data.u32(0)?,
            name: data.str(1)?.to_string(),
            guild: Some(data.str(2)?.to_string()).filter(|a| !a.is_empty()),
            level: data.u32(3)?,
            honor: data.u32(4)?,
            class,
            flag,
        })
    }
}

/// Basic information about one guild on the server. To get more information,
/// you need to query this player via the `ViewGuild` command
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
    /// Parses a Hall of Fame guild entry from a comma-separated record:
    /// `rank, name, leader, member_count, honor, is_attacked`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(HallOfFameGuild {
            rank: data.u32(0)?,
            name: data.str(1)?.to_string(),
            leader: data.str(2)?.to_string(),
            member_count: data.u32(3)?,
            honor: data.u32(4)?,
            is_attacked: data.u8_or(5, 0) == 1,
        })
    }
}

/// Basic information about one players pet collection on the server
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
    /// The amount of honor this pet collection has gained
    pub honor: u32,
    /// For guilds the value at this position is the attacked status, but no
    /// idea, what it means here
    pub unknown: i64,
}

impl HallOfFamePets {
    /// Parses a Hall of Fame pets entry from a comma-separated record:
    /// `rank, name, guild, collected, honor, unknown`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(HallOfFamePets {
            rank: data.u32(0)?,
            name: data.str(1)?.to_string(),
            guild: Some(data.str(2)?.to_string()).filter(|a| !a.is_empty()),
            collected: data.u32(3)?,
            honor: data.u32(4)?,
            unknown: data.i64_or(5, 0),
        })
    }
}

/// Basic information about one fortress on the server
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

impl HallOfFameFortress {
    /// Parses a Hall of Fame fortress entry from a comma-separated record:
    /// `rank, name, guild, upgrade, honor`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(HallOfFameFortress {
            rank: data.u32(0)?,
            name: data.str(1)?.to_string(),
            guild: Some(data.str(2)?.to_string()).filter(|a| !a.is_empty()),
            upgrade: data.u32(3)?,
            honor: data.u32(4)?,
        })
    }
}

/// Basic information about one players underworld on the server
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

impl HallOfFameUnderworld {
    /// Parses a Hall of Fame underworld entry from a comma-separated record:
    /// `rank, name, guild, upgrade, honor, unknown`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(HallOfFameUnderworld {
            rank: data.u32(0)?,
            name: data.str(1)?.to_string(),
            guild: Some(data.str(2)?.to_string()).filter(|a| !a.is_empty()),
            upgrade: data.u32(3)?,
            honor: data.u32(4)?,
            unknown: data.i64_or(5, 0),
        })
    }
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
    #[deprecated = "Since server update v30.500, this field is no longer \
                    available and will be removed from the API in the future"]
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

    /// The current experience points of the player
    pub experience: u64,
    /// The experience required to reach the next level
    pub next_level_xp: u64,

    /// The amount of honor this player has
    pub honor: u32,
    /// The rank of this player in the Hall of Fame
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

/// Information about another player's fortress
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherFortress {
    /// The total amount of upgrades this player has for their fortress
    pub upgrade_count: u32,
    /// The amount of soldiers suggested to use when attacking this players
    /// fortress
    pub soldier_advice: u16,
    /// The level of the fortifications of that player
    pub fortifications_level: u16,
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

/// The relation towards another player
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relationship {
    /// The player is ignored
    #[default]
    Ignored = -1,
    /// Normal relationship status
    Normal = 0,
    /// The player is marked as a friend
    Friend = 1,
}

impl OtherPlayer {
    /// Updates the pet attribute bonus from an `otherplayerpetbonus.petbonus`
    /// response: `[player_id, con, dex, int, luck, str]`.
    pub(crate) fn update_pet_bonus(&mut self, data: &Vals<'_>) -> Result<()> {
        data.ignore(0, "player id");
        // The order of these is neither pet, nor standard attribute order.
        self.attribute_pet_bonus[AttributeType::Constitution] = data.u32(1)?;
        self.attribute_pet_bonus[AttributeType::Dexterity] = data.u32(2)?;
        self.attribute_pet_bonus[AttributeType::Intelligence] = data.u32(3)?;
        self.attribute_pet_bonus[AttributeType::Luck] = data.u32(4)?;
        self.attribute_pet_bonus[AttributeType::Strength] = data.u32(5)?;
        Ok(())
    }

    /// Updates fortress stats from an `otherplayerfortressinfo` response:
    /// `[upgrade_count, fortifications_level, archer_count, mage_count,
    /// lootable_wood, lootable_stone]`.
    pub(crate) fn update_fortress(&mut self, data: &Vals<'_>) -> Result<()> {
        let ft = self.fortress.get_or_insert_default();
        ft.upgrade_count = data.u32(0)?;
        ft.fortifications_level = data.u16(1)?;
        ft.archer_count = data.u16(2)?;
        ft.mage_count = data.u16(3)?;
        ft.lootable_wood = data.u64(4)?;
        ft.lootable_stone = data.u64(5)?;
        Ok(())
    }

    /// Updates the player details from an `otherplayersavecharacter` response.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        data.ignore(0, "always 0");
        self.player_id = data.u32_or(1, 0);
        data.ignore(2, "always 0");
        self.level = (data.u32_or(3, 0) & 0xFFFF) as u16;
        self.experience = data.u64_or(4, 0);
        self.next_level_xp = data.u64_or(5, 0);
        self.honor = data.u32_or(6, 0);
        self.rank = data.u32_or(7, 0);

        self.portrait = Portrait::parse(&data.window(8..20));

        self.race = data.parse_enum(18)?;
        self.class =
            data.enum_map(20, |a| a.wrapping_sub(1)).ok_or_else(|| {
                SFError::ParsingErrorV2(format!(
                    "Unknown character class: {}",
                    data.u32_or(20, 0)
                ))
            })?;
        self.mount = data.enum_map(21, |a| a & 0xFF);
        data.ignore(22, "always 1 in captured responses");
        self.armor = data.u64_or(23, 0);
        self.min_damage = data.u32_or(24, 0);
        self.max_damage = data.u32_or(25, 0);
        self.portal_dmg_bonus = data.u32_or(26, 0);
        data.ignore(27, "unknown, large integer seen in the millions");
        self.portal_hp_bonus = data.u32_or(28, 0);
        self.mount_end = data.server_time(29, server_time)?;

        for (idx, val) in self.attribute_basis.values_mut().enumerate() {
            *val = data.u32_or(30 + idx, 0);
        }
        for (idx, val) in self.attribute_additions.values_mut().enumerate() {
            *val = data.u32_or(35 + idx, 0);
        }
        for (idx, val) in self.attribute_times_bought.values_mut().enumerate() {
            *val = data.u32_or(40 + idx, 0);
        }

        data.ignore(45..66, "unknown/unmodelled character save values");

        let sb_count = data.u32_or(66, 0);
        if sb_count >= 10000 {
            self.scrapbook_count = Some(sb_count - 10000);
        }

        data.ignore(67..69, "unknown");
        self.gladiator_lvl = data.u32_or(69, 0);

        Ok(())
    }

    /// Updates combat unit levels from an `otherplayerunitlevel` response.
    pub(crate) fn update_unit_levels(&mut self, data: &Vals<'_>) {
        self.wall_combat_lvl = data.u16_or(0, 0);
        data.ignore_all("other unit levels handled elsewhere");
    }

    /// Updates the player's fortress rank from an `otherplayerfortressrank`
    /// response.
    pub(crate) fn update_fortress_rank(&mut self, rank: i64) {
        if rank >= 0 {
            self.fortress.get_or_insert_default().rank =
                rank.try_into().unwrap_or(1);
        }
    }
}

/// The type of combat represented in a combat log entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CombatMessageType {
    /// An arena fight against another player
    Arena = 0,
    /// A tavern quest fight
    Quest = 1,
    /// A guild fight
    GuildFight = 2,
    /// A guild raid fight
    GuildRaid = 3,
    /// A dungeon fight
    Dungeon = 4,
    /// A tower fight
    TowerFight = 5,
    /// A lost fight
    LostFight = 6,
    /// A won fight
    WonFight = 7,
    /// An attack against a fortress
    FortressFight = 8,
    /// A fortress defense against an attacker
    FortressDefense = 9,
    /// A shadow world fight
    ShadowWorld = 12,
    /// A pet attack
    PetAttack = 14,
    /// A pet defense against an attacker
    PetDefense = 15,
    /// An underworld fight
    Underworld = 16,
    /// A twister fight
    Twister = 25,
    /// A lost guild fight
    GuildFightLost = 26,
    /// A won guild fight
    GuildFightWon = 27,
    /// A fortress defense where the attacker was already countered
    FortressDefenseAlreadyCountered = 109,
}

/// The type of an inbox message
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MessageType {
    /// A regular message
    #[default]
    Normal,
    /// An invitation to join a guild
    GuildInvite,
    /// A notification that the player was kicked from a guild
    GuildKicked,
}

/// An entry in the combat log
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CombatLogEntry {
    /// The unique message ID of the combat log entry
    pub msg_id: i64,
    /// The opponent's player or monster/guild name
    pub player_name: String,
    /// Whether the fight was won
    pub won: bool,
    /// The type of combat
    pub battle_type: CombatMessageType,
    /// The local timestamp when the battle occurred
    pub time: DateTime<Local>,
}

impl CombatLogEntry {
    /// Parses a combat log entry from a comma-separated record:
    /// `msg_id, player_name, won, battle_type, time, ...`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<CombatLogEntry> {
        let battle_type = data.parse_enum(3)?;
        let time = data.server_time(4, server_time)?.ok_or_else(|| {
            SFError::ParsingErrorV2("missing combat time".to_string())
        })?;

        if data.len() > 5 {
            data.ignore(5, "unknown combat log field");
        }

        Ok(CombatLogEntry {
            msg_id: data.i64(0)?,
            player_name: data.str(1)?.to_string(),
            won: data.str(2)? == "1",
            battle_type,
            time,
        })
    }
}

/// An entry in the player's message inbox
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InboxEntry {
    /// The message type
    pub msg_typ: MessageType,
    /// The sender's name
    pub from: String,
    /// The unique message ID
    pub msg_id: i32,
    /// The subject or title of the message
    pub title: String,
    /// The date and time the message was received
    pub date: DateTime<Local>,
    /// Whether the message has been marked as read
    pub read: bool,
}

impl InboxEntry {
    /// Parses an inbox entry from a comma-separated message string.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<InboxEntry> {
        if data.len() < 5 {
            return Err(SFError::ParsingErrorV2(format!(
                "Inbox entry too short (len: {})",
                data.len()
            )));
        }

        let last_idx = data.len() - 1;
        let time =
            data.server_time(last_idx, server_time)?.ok_or_else(|| {
                SFError::ParsingErrorV2("missing msg date".to_string())
            })?;

        let title_parts = data.slice(3..last_idx);
        let title_raw = title_parts.join(",");

        let msg_typ = match title_raw.as_str() {
            "3" => MessageType::GuildKicked,
            "5" => MessageType::GuildInvite,
            x if !x.is_empty() && x.chars().all(|a| a.is_ascii_digit()) => {
                return Err(SFError::ParsingErrorV2(format!(
                    "Unknown inbox msg type: {title_raw}"
                )));
            }
            _ => MessageType::Normal,
        };

        Ok(InboxEntry {
            msg_typ,
            date: time,
            from: data.str(1)?.to_string(),
            msg_id: data.i32(0)?,
            title: from_sf_string(title_raw.trim_end_matches('\t')),
            read: data.str(2)? != "0",
        })
    }
}

/// A news entry distributed by the server
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewsEntry {
    /// The id used to read the full contents of this message
    pub news_id: i64,
    /// The title of this news entry
    pub title: String,
    /// The time at which this news entry was distributed to players
    pub date: DateTime<Local>,
    /// Has this news entry been opened before?
    pub read: bool,
}

impl NewsEntry {
    /// Parses a news entry from a comma-separated record:
    /// `read, title, timestamp`.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<NewsEntry> {
        let ts = data.i64(2)?;
        let time = data.server_time(2, server_time)?.ok_or_else(|| {
            SFError::ParsingErrorV2(format!("missing news date: {ts}"))
        })?;

        if data.len() > 3 {
            data.ignore(3, "unused news icon");
        }

        Ok(NewsEntry {
            news_id: ts,
            date: time,
            title: from_sf_string(data.str(1)?),
            read: data.str(0)? == "1",
        })
    }
}

/// Information about another guild
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherGuild {
    /// The name of the guild
    pub name: String,

    /// The guild this guild is currently attacking, if any
    pub attacks: Option<String>,
    /// The guild defending against, if any
    pub defends_against: Option<String>,

    /// The rank of the guild in the Hall of Fame
    pub rank: u16,
    /// The cost to attack this guild
    pub attack_cost: u32,
    /// The guild's public description
    pub description: String,
    /// The guild's emblem
    pub emblem: Emblem,
    /// The guild's honor
    pub honor: u32,
    /// The number of finished guild raids
    pub finished_raids: u16,
    // should just be members.len(), right?
    member_count: u8,
    /// The members belonging to the guild
    pub members: Vec<OtherGuildMember>,
}

/// A member of another guild
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OtherGuildMember {
    /// The name of the member
    pub name: String,
    /// The instructor upgrade level of the member
    pub instructor_lvl: u16,
    /// The treasure upgrade level of the member
    pub treasure_lvl: u16,
    /// The rank of the member within the guild
    pub rank: GuildRank,
    /// The character level of the member
    pub level: u16,
    /// The pet level of the member
    pub pet_lvl: u16,
    /// The time the member was last active
    pub last_active: Option<DateTime<Local>>,
}

impl OtherGuild {
    /// Updates guild information from an `othergroup` response.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.member_count = data.u8_or(3, 0);
        let member_count = self.member_count as usize;
        self.finished_raids = data.u16_or(8, 0);
        self.honor = data.u32_or(13, 0);

        self.members.resize_with(member_count, Default::default);

        for (i, member) in self.members.iter_mut().enumerate() {
            member.level = data.u16_or(64 + i, 0);
            member.last_active = data.server_time(114 + i, server_time)?;
            member.treasure_lvl = data.u16_or(214 + i, 0);
            member.instructor_lvl = data.u16_or(264 + i, 0);
            member.rank = data.enum_or(314 + i, GuildRank::default());
            member.pet_lvl = data.u16_or(390 + i, 0);
        }

        data.ignore_all("guild member arrays and unmodelled save offsets");
        Ok(())
    }

    /// Updates member names from an `othergroupmember` response.
    pub(crate) fn update_member_names(&mut self, data: &Vals<'_>) {
        if self.members.len() < data.len() {
            self.members.resize_with(data.len(), Default::default);
        }
        for (m, name) in self.members.iter_mut().zip(data.iter()) {
            m.name = name.to_string();
        }
    }

    /// Updates guild description and emblem from an `othergroupdescription`
    /// response.
    pub(crate) fn update_description_embed(&mut self, data: &Vals<'_>) {
        let (emblem, desc) = match data.len() {
            0 => ("", ""),
            1 => ("", data.str_or(0, "")),
            _ => (data.str_or(0, ""), data.str_or(1, "")),
        };
        self.emblem.update(emblem);
        self.description = from_sf_string(desc);
        data.ignore_all("all description parts consumed");
    }
}

/// An entry representing a relation with another player
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelationEntry {
    /// The player's ID
    pub id: PlayerId,
    /// The player's name
    pub name: String,
    /// The guild name of the player
    pub guild: String,
    /// The character level of the player
    pub level: u16,
    /// The relationship status
    pub relation: Relationship,
}

/// A claimable mail delivery (e.g., promo codes, Twitch drops)
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClaimableMail {
    /// The message identifier
    pub msg_id: i64,
    /// The type of claimable mail
    pub typ: ClaimableMailType,
    /// The status of the claimable item
    pub status: ClaimableStatus,
    /// The name or subject of the claimable delivery
    pub name: String,
    /// The timestamp when the delivery was received
    pub received: Option<DateTime<Local>>,
    /// The expiration time until which the item can be claimed
    pub claimable_until: Option<DateTime<Local>>,
}

/// The claim status of a claimable mail delivery
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClaimableStatus {
    /// The mail has not been viewed yet
    #[default]
    Unread,
    /// The mail has been viewed but not yet claimed
    Read,
    /// The rewards have already been claimed
    Claimed,
}

/// The type of a claimable mail
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClaimableMailType {
    /// A coupon or promotional code reward
    Coupon = 10,
    /// A flying courier / superman delivery
    SupermanDelivery = 11,
    /// A Twitch drop delivery
    TwitchDrop = 12,
    /// A generic delivery
    #[default]
    GenericDelivery,
}

/// A preview of the contents of a claimable mail
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClaimablePreview {
    /// The items contained in the preview
    pub items: Vec<Item>,
    /// The resources or rewards contained in the preview
    pub resources: Vec<Reward>,
}

impl ClaimableMail {
    /// Parses a claimable mail delivery from a 6-element response slice:
    /// `[msg_id, status, type, code_name, start_time, end_time]`.
    ///
    /// # Errors
    /// Returns an error if required fields are missing or invalid.
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<Self> {
        let status = match data.get_str(1) {
            Some("0") => ClaimableStatus::Unread,
            Some("1") => ClaimableStatus::Read,
            Some("2") => ClaimableStatus::Claimed,
            Some(x) => {
                warn!("Unknown claimable status: {x}");
                ClaimableStatus::Claimed
            }
            None => {
                return Err(SFError::ParsingErrorV2(
                    "Missing claimable status".to_string(),
                ));
            }
        };

        Ok(Self {
            msg_id: data.i64(0)?,
            status,
            typ: data.enum_or(2, ClaimableMailType::GenericDelivery),
            name: data.get(3)?.to_string(),
            received: data.server_time(4, server_time)?,
            claimable_until: data.server_time(5, server_time)?,
        })
    }
}

impl ClaimablePreview {
    /// Updates the resources in this preview from a `pendingrewardressources`
    /// response.
    pub(crate) fn update_resources(&mut self, data: &Vals<'_>) {
        self.resources = data
            .chunks(2)
            .map(|chunk| Reward {
                typ: RewardType::parse(chunk.i64_or(0, 0)),
                amount: chunk.u64_or(1, 1),
            })
            .collect();
    }

    /// Updates the items in this preview from a `pendingreward` response.
    pub(crate) fn update_items(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.items = data
            .chunks(ITEM_PARSE_LEN)
            .filter_map(|chunk| Item::parse(&chunk, server_time).ok().flatten())
            .collect();
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::unreadable_literal,
        clippy::len_zero,
        clippy::indexing_slicing
    )]
    use super::*;

    #[test]
    fn test_hall_of_fame_player_parse() {
        let raw = "1,Kiirschsaft,Die Hejels,397,157866,2,de";
        let data = Vals::by(raw, ',');
        let player = HallOfFamePlayer::parse(&data).unwrap();
        assert_eq!(player.rank, 1);
        assert_eq!(player.name, "Kiirschsaft");
        assert_eq!(player.guild.as_deref(), Some("Die Hejels"));
        assert_eq!(player.level, 397);
        assert_eq!(player.honor, 157866);
        assert_eq!(player.class, Class::Mage);
        assert_eq!(player.flag, Flag::parse("de"));
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );

        let raw_empty = "2,SoloPlayer,,100,500,1,";
        let data_empty = Vals::by(raw_empty, ',');
        let player2 = HallOfFamePlayer::parse(&data_empty).unwrap();
        assert_eq!(player2.guild, None);
        assert_eq!(player2.flag, None);
        assert_eq!(player2.class, Class::Warrior);
        assert!(
            data_empty.unaccessed_indices().is_empty(),
            "unread values in {data_empty}"
        );
    }

    #[test]
    fn test_hall_of_fame_guild_parse() {
        let raw = "1,Rift,Gyula240,50,123456,1";
        let data = Vals::by(raw, ',');
        let guild = HallOfFameGuild::parse(&data).unwrap();
        assert_eq!(guild.rank, 1);
        assert_eq!(guild.name, "Rift");
        assert_eq!(guild.leader, "Gyula240");
        assert_eq!(guild.member_count, 50);
        assert_eq!(guild.honor, 123456);
        assert!(guild.is_attacked);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_hall_of_fame_pets_parse() {
        let raw = "5,PetMaster,PetGuild,100,5000,0";
        let data = Vals::by(raw, ',');
        let pets = HallOfFamePets::parse(&data).unwrap();
        assert_eq!(pets.rank, 5);
        assert_eq!(pets.name, "PetMaster");
        assert_eq!(pets.guild.as_deref(), Some("PetGuild"));
        assert_eq!(pets.collected, 100);
        assert_eq!(pets.honor, 5000);
        assert_eq!(pets.unknown, 0);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_hall_of_fame_fortress_parse() {
        let raw = "10,FortMaster,FortGuild,50,9000";
        let data = Vals::by(raw, ',');
        let ft = HallOfFameFortress::parse(&data).unwrap();
        assert_eq!(ft.rank, 10);
        assert_eq!(ft.name, "FortMaster");
        assert_eq!(ft.guild.as_deref(), Some("FortGuild"));
        assert_eq!(ft.upgrade, 50);
        assert_eq!(ft.honor, 9000);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_hall_of_fame_underworld_parse() {
        let raw = "3,UnderMaster,,40,7500,0";
        let data = Vals::by(raw, ',');
        let uw = HallOfFameUnderworld::parse(&data).unwrap();
        assert_eq!(uw.rank, 3);
        assert_eq!(uw.name, "UnderMaster");
        assert_eq!(uw.guild, None);
        assert_eq!(uw.upgrade, 40);
        assert_eq!(uw.honor, 7500);
        assert_eq!(uw.unknown, 0);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_hall_of_fame_hellevator_parse() {
        let raw = "1,HellMaster,50000";
        let data = Vals::by(raw, ',');
        let hh = HallOfFameHellevator::parse(&data).unwrap();
        assert_eq!(hh.rank, 1);
        assert_eq!(hh.name, "HellMaster");
        assert_eq!(hh.tokens, 50000);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_other_player_update_pet_bonus() {
        let raw = "302106/13/15/13/16/16";
        let data = Vals::from(raw);
        let mut player = OtherPlayer::default();
        player.update_pet_bonus(&data).unwrap();
        assert_eq!(player.attribute_pet_bonus[AttributeType::Constitution], 13);
        assert_eq!(player.attribute_pet_bonus[AttributeType::Dexterity], 15);
        assert_eq!(player.attribute_pet_bonus[AttributeType::Intelligence], 13);
        assert_eq!(player.attribute_pet_bonus[AttributeType::Luck], 16);
        assert_eq!(player.attribute_pet_bonus[AttributeType::Strength], 16);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_other_player_update_fortress() {
        let raw = "416/19/30/15/29882513/1088585";
        let data = Vals::from(raw);
        let mut player = OtherPlayer::default();
        player.update_fortress(&data).unwrap();
        let ft = player.fortress.unwrap();
        assert_eq!(ft.upgrade_count, 416);
        assert_eq!(ft.fortifications_level, 19);
        assert_eq!(ft.archer_count, 30);
        assert_eq!(ft.mage_count, 15);
        assert_eq!(ft.lootable_wood, 29882513);
        assert_eq!(ft.lootable_stone, 1088585);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_other_player_update() {
        let raw = "0/302106/0/397/800402824/1500000000/157866/7371/2/106/102/\
                   4/99/8/4/4/0/0/2/2/2/0/1/5170/1330/3142/50/2672172/50/0/\
                   3251/3251/5574/3357/2205/2996/2942/23116/22828/14167/2846/\
                   2849/5163/2962/1803/0/0/0/0/0/0/0/15/22/24/25/17/0/18/0/0/\
                   0/0/0/0/49614/11385/1/15/14";
        let data = Vals::from(raw);
        let mut player = OtherPlayer::default();
        player.update(&data, ServerTime::default()).unwrap();
        assert_eq!(player.player_id, 302106);
        assert_eq!(player.level, 397);
        assert_eq!(player.experience, 800402824);
        assert_eq!(player.next_level_xp, 1500000000);
        assert_eq!(player.honor, 157866);
        assert_eq!(player.rank, 7371);
        assert_eq!(player.race, Race::Elf);
        assert_eq!(player.class, Class::Mage);
        assert_eq!(player.armor, 5170);
        assert_eq!(player.min_damage, 1330);
        assert_eq!(player.max_damage, 3142);
        assert_eq!(player.portal_dmg_bonus, 50);
        assert_eq!(player.portal_hp_bonus, 50);
        assert_eq!(player.scrapbook_count, Some(1385));
        assert_eq!(player.gladiator_lvl, 14);
        assert_eq!(player.attribute_basis[AttributeType::Strength], 3251);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_combat_log_entry_parse() {
        let raw = "94224194,OpponentPlayer,1,0,1600000000";
        let data = Vals::by(raw, ',');
        let entry =
            CombatLogEntry::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(entry.msg_id, 94224194);
        assert_eq!(entry.player_name, "OpponentPlayer");
        assert!(entry.won);
        assert_eq!(entry.battle_type, CombatMessageType::Arena);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );

        let raw_6 = "76078261,Geli123,0,9,1791901430,0";
        let data_6 = Vals::by(raw_6, ',');
        let entry_6 =
            CombatLogEntry::parse(&data_6, ServerTime::default()).unwrap();
        assert_eq!(entry_6.msg_id, 76078261);
        assert_eq!(entry_6.player_name, "Geli123");
        assert!(!entry_6.won);
        assert!(
            data_6.unaccessed_indices().is_empty(),
            "unread values in {data_6}"
        );
    }

    #[test]
    fn test_inbox_entry_parse() {
        let raw = "12345,Admin,0,Important News,1600000000";
        let data = Vals::by(raw, ',');
        let entry = InboxEntry::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(entry.msg_id, 12345);
        assert_eq!(entry.from, "Admin");
        assert!(!entry.read);
        assert_eq!(entry.title, "Important News");
        assert_eq!(entry.msg_typ, MessageType::Normal);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );

        let raw_invite = "12346,GuildLeader,1,5,1600000000";
        let data_invite = Vals::by(raw_invite, ',');
        let invite =
            InboxEntry::parse(&data_invite, ServerTime::default()).unwrap();
        assert_eq!(invite.msg_typ, MessageType::GuildInvite);

        let raw_commas = "12347,Friend,0,Hello,world!,1600000000";
        let data_commas = Vals::by(raw_commas, ',');
        let with_commas =
            InboxEntry::parse(&data_commas, ServerTime::default()).unwrap();
        assert_eq!(with_commas.title, "Hello,world!");
        assert!(
            data_commas.unaccessed_indices().is_empty(),
            "unread values in {data_commas}"
        );
    }

    #[test]
    fn test_news_entry_parse() {
        let raw = "1,Update Announcement,1600000000,0";
        let data = Vals::by(raw, ',');
        let news = NewsEntry::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(news.news_id, 1600000000);
        assert_eq!(news.title, "Update Announcement");
        assert!(news.read);
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }

    #[test]
    fn test_claimable_mail_parse() {
        let raw = "101/0/10/TESTCODE/1600000000/1700000000";
        let data = Vals::from(raw);
        let claimable =
            ClaimableMail::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(claimable.msg_id, 101);
        assert_eq!(claimable.status, ClaimableStatus::Unread);
        assert_eq!(claimable.typ, ClaimableMailType::Coupon);
        assert_eq!(claimable.name, "TESTCODE");
        assert!(claimable.received.is_some());
        assert!(claimable.claimable_until.is_some());
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );

        let mut mail = Mail::default();
        let list_raw = "101/0/10/TESTCODE/1600000000/1700000000/102/1/11/HERO/\
                        1600000000/1700000000";
        let list_data = Vals::from(list_raw);
        mail.update_claimables(&list_data, ServerTime::default());
        assert_eq!(mail.claimables.len(), 2);
        assert_eq!(mail.claimables[1].msg_id, 102);
        assert_eq!(mail.claimables[1].status, ClaimableStatus::Read);
        assert_eq!(mail.claimables[1].typ, ClaimableMailType::SupermanDelivery);
    }

    #[test]
    fn test_claimable_preview_update() {
        let mut preview = ClaimablePreview::default();
        let res_raw = "3/50/4/1000";
        let res_data = Vals::from(res_raw);
        preview.update_resources(&res_data);
        assert_eq!(preview.resources.len(), 2);
        assert_eq!(preview.resources[0].typ, RewardType::Mushrooms);
        assert_eq!(preview.resources[0].amount, 50);
        assert_eq!(preview.resources[1].typ, RewardType::Silver);
        assert_eq!(preview.resources[1].amount, 1000);
        assert!(
            res_data.unaccessed_indices().is_empty(),
            "unread values in {res_data}"
        );
    }

    #[test]
    fn test_mail_updates() {
        let mut mail = Mail::default();
        let news_raw = "1,News One,1600000000,0;2,News Two,1600000010,1";
        let news_data = Vals::by(news_raw, ';');
        mail.update_news_inbox(&news_data, ServerTime::default());
        assert_eq!(mail.news_inbox.len(), 2);
        assert!(
            news_data.unaccessed_indices().is_empty(),
            "unread values in {news_data}"
        );

        let inbox_raw = "10,Sender,0,Title,1600000000";
        let inbox_data = Vals::by(inbox_raw, ';');
        mail.update_inbox(&inbox_data, ServerTime::default());
        assert_eq!(mail.inbox.len(), 1);
        assert_eq!(mail.inbox[0].from, "Sender");
        assert!(
            inbox_data.unaccessed_indices().is_empty(),
            "unread values in {inbox_data}"
        );

        let combat_raw = "1001,OpponentPlayer,1,0,1600000000";
        let combat_data = Vals::by(combat_raw, ';');
        mail.update_combat_log(&combat_data, ServerTime::default());
        assert_eq!(mail.combat_log.len(), 1);
        assert_eq!(mail.combat_log[0].player_name, "OpponentPlayer");
        assert!(
            combat_data.unaccessed_indices().is_empty(),
            "unread values in {combat_data}"
        );
    }

    #[test]
    fn test_other_guild_updates() {
        let mut guild = OtherGuild::default();
        let names_raw = "Leader,Officer,Member";
        let names_data = Vals::by(names_raw, ',');
        guild.update_member_names(&names_data);
        assert_eq!(guild.members.len(), 3);
        assert_eq!(guild.members[0].name, "Leader");
        assert_eq!(guild.members[1].name, "Officer");
        assert_eq!(guild.members[2].name, "Member");
        assert!(
            names_data.unaccessed_indices().is_empty(),
            "unread values in {names_data}"
        );

        let desc_data = Vals::by("010203§Welcome to our guild!", '§');
        guild.update_description_embed(&desc_data);
        assert_eq!(guild.description, "Welcome to our guild!");
        assert!(
            desc_data.unaccessed_indices().is_empty(),
            "unread values in {desc_data}"
        );
    }

    #[test]
    fn test_other_player_unit_levels_and_fortress_rank() {
        let mut player = OtherPlayer::default();
        let unit_raw = "15/10/12/8";
        let unit_data = Vals::from(unit_raw);
        player.update_unit_levels(&unit_data);
        assert_eq!(player.wall_combat_lvl, 15);
        assert!(
            unit_data.unaccessed_indices().is_empty(),
            "unread values in {unit_data}"
        );

        player.update_fortress_rank(42);
        assert_eq!(player.fortress.unwrap().rank, 42);

        player.update_fortress_rank(-1);
        assert_eq!(player.fortress.unwrap().rank, 42);
    }

    #[test]
    fn test_hall_of_fames_updates() {
        let mut hof = HallOfFames::default();
        let players_raw = "1,Kiirschsaft,Die \
                           Hejels,397,157866,2,de;2,PlayerTwo,Guild,350,\
                           140000,1,en";
        let players_data = Vals::by(players_raw, ';');
        hof.update_players(&players_data);
        assert_eq!(hof.players.len(), 2);
        assert_eq!(hof.players[0].name, "Kiirschsaft");
        assert!(
            players_data.unaccessed_indices().is_empty(),
            "unread values in {players_data}"
        );

        let guilds_raw = "1,Die Hejels,50,157866,10,de";
        let guilds_data = Vals::by(guilds_raw, ';');
        hof.update_guilds(&guilds_data);
        assert_eq!(hof.guilds.len(), 1);
        assert_eq!(hof.guilds[0].name, "Die Hejels");
        assert!(
            guilds_data.unaccessed_indices().is_empty(),
            "unread values in {guilds_data}"
        );

        let pets_raw = "1,PetMaster,PetGuild,100,157866,0";
        let pets_data = Vals::by(pets_raw, ';');
        hof.update_pets(&pets_data);
        assert_eq!(hof.pets.len(), 1);
        assert_eq!(hof.pets[0].name, "PetMaster");
        assert!(
            pets_data.unaccessed_indices().is_empty(),
            "unread values in {pets_data}"
        );

        let ft_raw = "1,FortressMaster,Die Hejels,500,157866,100,de";
        let ft_data = Vals::by(ft_raw, ';');
        hof.update_fortresses(&ft_data);
        assert_eq!(hof.fortresses.len(), 1);
        assert_eq!(hof.fortresses[0].name, "FortressMaster");
        assert!(
            ft_data.unaccessed_indices().is_empty(),
            "unread values in {ft_data}"
        );

        let uw_raw = "1,UnderworldMaster,Die Hejels,500,157866,15,de";
        let uw_data = Vals::by(uw_raw, ';');
        hof.update_underworlds(&uw_data);
        assert_eq!(hof.underworlds.len(), 1);
        assert_eq!(hof.underworlds[0].name, "UnderworldMaster");
        assert!(
            uw_data.unaccessed_indices().is_empty(),
            "unread values in {uw_data}"
        );

        let hlv_raw = "1,HellevatorHero,1000";
        let hlv_data = Vals::by(hlv_raw, ';');
        hof.update_hellevator(&hlv_data);
        assert_eq!(hof.hellevator.len(), 1);
        assert_eq!(hof.hellevator[0].name, "HellevatorHero");
        assert_eq!(hof.hellevator[0].tokens, 1000);
        assert!(
            hlv_data.unaccessed_indices().is_empty(),
            "unread values in {hlv_data}"
        );
    }
}
