use std::collections::HashMap;

use chrono::{DateTime, Local};
use enum_map::EnumMap;
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{
    AttributeType, Class, Emblem, Flag, Item, Potion, Race, Reward, RewardType,
    SFError, ServerTime,
    character::{Character, Mount, Portrait},
    guild::GuildRank,
    items::{Equipment, ITEM_PARSE_LEN},
};
use crate::{PlayerId, misc::*, response::Vals};

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
    pub(crate) fn update_news_inbox(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.news_inbox.clear();
        for record in data.iter() {
            if record.is_empty() {
                continue;
            }
            let values = Vals::by(record, ',');
            match NewsEntry::parse(&values, server_time) {
                Ok(entry) => self.news_inbox.push(entry),
                Err(err) => warn!("Invalid msg: {record} {err}"),
            }
        }
    }

    pub(crate) fn update_inbox(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.inbox.clear();
        for record in data.iter() {
            if record.is_empty() {
                continue;
            }
            match InboxEntry::parse(record, server_time) {
                Ok(entry) => self.inbox.push(entry),
                Err(err) => warn!("Invalid msg: {record} {err}"),
            }
        }
    }

    pub(crate) fn update_combat_log(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.combat_log.clear();
        for record in data.iter() {
            let values = Vals::by(record, ',');
            if values.values().iter().all(|value| value.is_empty()) {
                values.ignore_all("empty combat log record");
                continue;
            }
            match CombatLogEntry::parse(&values, server_time) {
                Ok(entry) => self.combat_log.push(entry),
                Err(err) => {
                    warn!(
                        "Unable to parse combat log entry: {values:?} - {err}"
                    );
                }
            }
        }
    }

    pub(crate) fn update_claimables(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.claimables.clear();
        let complete_len = data.len() / 6 * 6;
        for record in data.window(0..complete_len).chunks(6) {
            if let Some(claimable) = ClaimableMail::parse(&record, server_time)
            {
                self.claimables.push(claimable);
            } else {
                warn!("Dropping malformed claimable mail record: {record}");
            }
        }
        data.ignore(complete_len..data.len(), "partial claimable mail record");
    }
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

impl HallOfFameHellevator {
    pub(crate) fn parse(data: &Vals<'_>) -> Option<Self> {
        let (Ok(rank), Ok(name), Ok(tokens)) =
            (data.usize(0), data.str(1), data.u64(2))
        else {
            data.ignore_all("malformed hellevator hall of fame record");
            return None;
        };

        data.ignore(3..data.len(), "extra hall of fame record values");
        Some(Self {
            rank,
            name: name.to_owned(),
            tokens,
        })
    }
}

fn update_hall_of_fame_list<T>(
    data: &Vals<'_>,
    entries: &mut Vec<T>,
    parse: impl Fn(&Vals<'_>) -> Result<T, SFError>,
) {
    entries.clear();
    for record in data.iter() {
        if record.is_empty() {
            continue;
        }
        let values = Vals::by(record, ',');
        match parse(&values) {
            Ok(entry) => entries.push(entry),
            Err(err) => warn!("{err}"),
        }
    }
}

fn hall_of_fame_parse_result<T>(
    data: &Vals<'_>,
    result: Result<T, SFError>,
) -> Result<T, SFError> {
    if result.is_err() {
        data.ignore_all("malformed hall of fame record");
    }
    result
}

impl HallOfFames {
    pub(crate) fn update_players(&mut self, data: &Vals<'_>) {
        self.players.clear();
        for record in data.iter() {
            if record.is_empty() {
                continue;
            }
            if record.ends_with(",,,0,0,0,") {
                data.ignore_all("unused hall of fame player records");
                break;
            }
            let values = Vals::by(record, ',');
            match HallOfFamePlayer::parse(&values) {
                Ok(entry) => self.players.push(entry),
                Err(err) => warn!("{err}"),
            }
        }
    }

    pub(crate) fn update_guilds(&mut self, data: &Vals<'_>) {
        update_hall_of_fame_list(
            data,
            &mut self.guilds,
            HallOfFameGuild::parse,
        );
    }

    pub(crate) fn update_pets(&mut self, data: &Vals<'_>) {
        update_hall_of_fame_list(data, &mut self.pets, HallOfFamePets::parse);
    }

    pub(crate) fn update_fortresses(&mut self, data: &Vals<'_>) {
        update_hall_of_fame_list(
            data,
            &mut self.fortresses,
            HallOfFameFortress::parse,
        );
    }

    pub(crate) fn update_underworlds(&mut self, data: &Vals<'_>) {
        update_hall_of_fame_list(
            data,
            &mut self.underworlds,
            HallOfFameUnderworld::parse,
        );
    }

    pub(crate) fn update_hellevator(&mut self, data: &Vals<'_>) {
        self.hellevator.clear();
        for record in data.iter() {
            if record.is_empty() {
                continue;
            }
            if record.starts_with(',') {
                warn!(
                    "Dropping malformed hellevator hall of fame record: \
                     {record}"
                );
                continue;
            }
            let values = Vals::by(record, ',');
            if let Some(entry) = HallOfFameHellevator::parse(&values) {
                self.hellevator.push(entry);
            } else {
                warn!(
                    "Dropping malformed hellevator hall of fame record: \
                     {record}"
                );
            }
        }
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
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self, SFError> {
        hall_of_fame_parse_result(
            data,
            (|| {
                let rank = data.u32(0)?;
                let name = data.str(1)?.to_owned();
                let guild =
                    Some(data.str(2)?.to_owned()).filter(|g| !g.is_empty());
                let level = data.u32(3)?;
                let honor = data.u32(4)?;
                let raw_class = data.i64(5)?;
                let mapped_class = raw_class.wrapping_sub(1);
                let Some(class) = FromPrimitive::from_i64(mapped_class) else {
                    warn!("Invalid hof class: {raw_class} - {data:?}");
                    return Err(SFError::ParsingError(
                        "hof player class",
                        mapped_class.to_string(),
                    ));
                };

                let raw_flag = data.get_str(6).unwrap_or_default();
                let flag = Flag::parse(raw_flag);
                data.ignore(7..data.len(), "extra hall of fame player values");

                Ok(Self {
                    rank,
                    name,
                    guild,
                    level,
                    honor,
                    class,
                    flag,
                })
            })(),
        )
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
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self, SFError> {
        hall_of_fame_parse_result(
            data,
            (|| {
                let rank = data.u32(0)?;
                let name = data.str(1)?.to_owned();
                let leader = data.str(2)?.to_owned();
                let member_count = data.u32(3)?;
                let honor = data.u32(4)?;
                let attack_status = data.u8(5)?;
                data.ignore(6..data.len(), "extra hall of fame guild values");

                Ok(Self {
                    rank,
                    name,
                    leader,
                    member_count,
                    honor,
                    is_attacked: attack_status == 1,
                })
            })(),
        )
    }
}

impl HallOfFamePets {
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self, SFError> {
        hall_of_fame_parse_result(
            data,
            (|| {
                let rank = data.u32(0)?;
                let name = data.str(1)?.to_owned();
                let guild =
                    Some(data.str(2)?.to_owned()).filter(|g| !g.is_empty());
                let collected = data.u32(3)?;
                let honor = data.u32(4)?;
                let unknown = data.i64(5)?;
                data.ignore(6..data.len(), "extra hall of fame pet values");

                Ok(Self {
                    name,
                    rank,
                    guild,
                    collected,
                    honor,
                    unknown,
                })
            })(),
        )
    }
}

impl HallOfFameFortress {
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self, SFError> {
        hall_of_fame_parse_result(
            data,
            (|| {
                let rank = data.u32(0)?;
                let name = data.str(1)?.to_owned();
                let guild =
                    Some(data.str(2)?.to_owned()).filter(|g| !g.is_empty());
                let upgrade = data.u32(3)?;
                let honor = data.u32(4)?;
                data.ignore(
                    5..data.len(),
                    "extra hall of fame fortress values",
                );

                Ok(Self {
                    name,
                    rank,
                    guild,
                    upgrade,
                    honor,
                })
            })(),
        )
    }
}

impl HallOfFameUnderworld {
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self, SFError> {
        hall_of_fame_parse_result(
            data,
            (|| {
                let rank = data.u32(0)?;
                let name = data.str(1)?.to_owned();
                let guild =
                    Some(data.str(2)?.to_owned()).filter(|g| !g.is_empty());
                let upgrade = data.u32(3)?;
                let honor = data.u32(4)?;
                let unknown = data.i64(5)?;
                data.ignore(
                    6..data.len(),
                    "extra hall of fame underworld values",
                );

                Ok(Self {
                    rank,
                    name,
                    guild,
                    upgrade,
                    honor,
                    unknown,
                })
            })(),
        )
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

#[derive(Debug, Default, Clone, FromPrimitive, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relationship {
    #[default]
    Ignored = -1,
    Normal = 0,
    Friend = 1,
}

fn update_attribute_block<A: enum_map::Enum>(
    map: &mut EnumMap<A, u32>,
    data: &Vals<'_>,
    start: usize,
) -> Result<(), SFError> {
    data.get(start)?;
    for (idx, value) in map.values_mut().enumerate() {
        let pos = start + idx;
        if pos < data.len() {
            *value = soft_into(data.i64(pos)?, "attribute val", 0);
        }
    }
    Ok(())
}

impl OtherPlayer {
    pub(crate) fn update_pet_bonus(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        let parsed = (|| {
            data.ignore(0, "unknown pet bonus value");
            // The order of these makes no sense. It is neither pet,
            // nor attribute order.
            let constitution = data.u32(1)?;
            let dexterity = data.u32(2)?;
            let intelligence = data.u32(3)?;
            let luck = data.u32(4)?;
            let strength = data.u32(5)?;
            data.ignore(6..data.len(), "extra pet bonus values");
            Ok::<_, SFError>((
                constitution,
                dexterity,
                intelligence,
                luck,
                strength,
            ))
        })();
        let (constitution, dexterity, intelligence, luck, strength) =
            match parsed {
                Ok(values) => values,
                Err(err) => {
                    data.ignore_all("malformed pet bonus record");
                    return Err(err);
                }
            };

        let attributes = &mut self.attribute_pet_bonus;
        *attributes.get_mut(AttributeType::Constitution) = constitution;
        *attributes.get_mut(AttributeType::Dexterity) = dexterity;
        *attributes.get_mut(AttributeType::Intelligence) = intelligence;
        *attributes.get_mut(AttributeType::Luck) = luck;
        *attributes.get_mut(AttributeType::Strength) = strength;
        Ok(())
    }

    pub(crate) fn update_fortress(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        let parsed = (|| {
            let upgrades = data.i64(0)?;
            let fortifications = data.i64(1)?;
            let archers = data.i64(2)?;
            let mages = data.i64(3)?;
            let wood = data.i64(4)?;
            let stone = data.i64(5)?;
            data.ignore(6..data.len(), "extra other-player fortress values");
            Ok::<_, SFError>((
                upgrades,
                fortifications,
                archers,
                mages,
                wood,
                stone,
            ))
        })();
        let (upgrades, fortifications, archers, mages, wood, stone) =
            match parsed {
                Ok(values) => values,
                Err(err) => {
                    data.ignore_all("malformed other-player fortress record");
                    return Err(err);
                }
            };

        let mut fortress = self.fortress.clone().unwrap_or_default();
        fortress.upgrade_count = soft_into(upgrades, "other ft upgrades", 0);
        fortress.fortifications_level =
            soft_into(fortifications, "other soldier fortifications", 0);
        fortress.archer_count = soft_into(archers, "other mage count", 0);
        fortress.mage_count = soft_into(mages, "other soldier advice", 0);
        fortress.lootable_wood = soft_into(wood, "other lootable wood", 0);
        fortress.lootable_stone = soft_into(stone, "other lootable stone", 0);
        self.fortress = Some(fortress);
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        let mut updated = self.clone();
        let parsed = (|| -> Result<(), SFError> {
            data.ignore(0, "unknown other-player save value");
            updated.player_id = soft_into(data.i64(1)?, "player id", 0);
            data.ignore(2, "unknown other-player save value");
            updated.level = soft_into(data.i64(3)? & 0xffff, "level", 0);
            updated.experience = soft_into(data.i64(4)?, "experience", 0);
            updated.next_level_xp =
                soft_into(data.i64(5)?, "xp to next lvl", 0);
            updated.honor = soft_into(data.i64(6)?, "honor", 0);
            updated.rank = soft_into(data.i64(7)?, "rank", 0);
            updated.portrait = Portrait::parse_vals(&data.window(8..20));

            let raw_race = data.i64(18)?;
            updated.race =
                FromPrimitive::from_i64(raw_race).ok_or_else(|| {
                    SFError::ParsingError("char race", raw_race.to_string())
                })?;
            let raw_class = data.i64(20)?;
            let mapped_class = raw_class.wrapping_sub(1);
            updated.class =
                FromPrimitive::from_i64(mapped_class).ok_or_else(|| {
                    SFError::ParsingError(
                        "character class",
                        mapped_class.to_string(),
                    )
                })?;

            let raw_mount = data.i64(21)?;
            let masked_mount = raw_mount & 0xff;
            updated.mount = FromPrimitive::from_i64(masked_mount);
            if updated.mount.is_none() && !matches!(masked_mount, 0 | -1) {
                warn!("There might be a new character mount -> {masked_mount}");
            }

            data.ignore(22, "unknown other-player save value");
            updated.armor = soft_into(data.i64(23)?, "total armor", 0);
            updated.min_damage = soft_into(data.i64(24)?, "min damage", 0);
            updated.max_damage = soft_into(data.i64(25)?, "max damage", 0);
            let raw_portal_damage = data.i64(26)?;
            updated.portal_dmg_bonus = u32::try_from(raw_portal_damage)
                .map_err(|_| {
                    SFError::ParsingError(
                        "portal dmg bonus",
                        raw_portal_damage.to_string(),
                    )
                })?;
            data.ignore(27, "unknown other-player save value");
            updated.portal_hp_bonus =
                soft_into(data.i64(28)?, "portal hp bonus", 0);
            updated.mount_end = data.server_time(29, server_time)?;

            update_attribute_block(&mut updated.attribute_basis, data, 30)?;
            update_attribute_block(&mut updated.attribute_additions, data, 35)?;
            update_attribute_block(
                &mut updated.attribute_times_bought,
                data,
                40,
            )?;

            let scrapbook_count = data.i64(66)?;
            if scrapbook_count >= 10000 {
                updated.scrapbook_count = Some(soft_into(
                    scrapbook_count - 10000,
                    "scrapbook count",
                    0,
                ));
            }
            updated.gladiator_lvl =
                soft_into(data.i64(69)?, "gladiator lvl", 0);

            data.ignore(45..66, "unmodelled other-player save values");
            data.ignore(67..69, "unmodelled other-player save values");
            data.ignore(70..data.len(), "extra other-player save values");
            Ok(())
        })();
        if let Err(err) = parsed {
            data.ignore_all("malformed other-player save record");
            return Err(err);
        }
        *self = updated;
        Ok(())
    }

    pub(crate) fn update_unit_levels(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        let parsed = (|| {
            let level = soft_into(data.i64(0)?, "wall_lvl", 0);
            data.ignore(1..data.len(), "extra other-player unit levels");
            Ok::<_, SFError>(level)
        })();
        match parsed {
            Ok(level) => {
                self.wall_combat_lvl = level;
                Ok(())
            }
            Err(err) => {
                data.ignore_all("malformed other-player unit levels");
                Err(err)
            }
        }
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
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<CombatLogEntry, SFError> {
        let result = (|| {
            let msg_id = data.i64(0)?;
            let player_name = data.str(1)?.to_owned();
            let won = data.str(2)? == "1";
            let battle_type = data.i64(3)?;
            let timestamp = data.i64(4)?;
            let time = server_time
                .convert_to_local(timestamp, "combat time")
                .ok_or_else(|| {
                SFError::ParsingError("combat time", timestamp.to_string())
            })?;
            let battle_type =
                FromPrimitive::from_i64(battle_type).ok_or_else(|| {
                    SFError::ParsingError(
                        "combat mt",
                        format!("{battle_type} @ {time:?}"),
                    )
                })?;
            data.ignore(5..data.len(), "extra combat log values");
            Ok(Self {
                msg_id,
                player_name,
                won,
                battle_type,
                time,
            })
        })();
        if result.is_err() {
            data.ignore_all("malformed combat log record");
        }
        result
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
            read: parts.cget(2, "inbox read")? != "0",
        })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewsEntry {
    /// The id used to read the full contents of this message
    pub news_id: i64,
    /// The titel of this news entry
    pub title: String,
    /// The time at which this news entry was distributed to players
    pub date: DateTime<Local>,
    /// Has this news entry been opened before?
    pub read: bool,
    // There is an icon here as well, but we do not care
}

impl NewsEntry {
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<NewsEntry, SFError> {
        let result = (|| {
            let read = data.str(0)? == "1";
            let title = from_sf_string(data.str(1)?);
            let raw_timestamp = data.get(2)?;
            let Ok(timestamp) = raw_timestamp.parse::<i64>() else {
                return Err(SFError::ParsingError(
                    "invalid news ts",
                    raw_timestamp.to_owned(),
                ));
            };
            let published_at = server_time
                .convert_to_local(timestamp, "msg_date")
                .ok_or_else(|| {
                    SFError::ParsingError("msg date", timestamp.to_string())
                })?;
            data.ignore(3..data.len(), "extra news values");
            Ok(Self {
                news_id: timestamp,
                date: published_at,
                title,
                read,
            })
        })();
        if result.is_err() {
            data.ignore_all("malformed news record");
        }
        result
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
fn required_guild_i64(data: &Vals<'_>, pos: usize) -> Result<i64, SFError> {
    Ok(data.str(pos)?.parse().unwrap_or_default())
}

impl OtherGuild {
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        let mut updated = self.clone();
        let parsed = (|| -> Result<(), SFError> {
            updated.member_count =
                soft_into(required_guild_i64(data, 3)?, "member count", 0);
            let member_count = usize::from(updated.member_count);
            updated.finished_raids =
                soft_into(required_guild_i64(data, 8)?, "raid count", 0);
            updated.honor = soft_into(
                required_guild_i64(data, 13)?,
                "other guild honor",
                0,
            );

            updated.members.resize_with(member_count, Default::default);
            for (idx, member) in updated.members.iter_mut().enumerate() {
                member.level = soft_into(
                    required_guild_i64(data, 64 + idx)?,
                    "other guild member level",
                    0,
                );
                member.last_active = server_time.convert_to_local(
                    required_guild_i64(data, 114 + idx)?,
                    "other guild member active",
                );
                member.treasure_lvl = soft_into(
                    required_guild_i64(data, 214 + idx)?,
                    "other guild member treasure levels",
                    0,
                );
                member.instructor_lvl = soft_into(
                    required_guild_i64(data, 264 + idx)?,
                    "other guild member instructor levels",
                    0,
                );
                let raw_rank = required_guild_i64(data, 314 + idx)?;
                member.rank =
                    FromPrimitive::from_i64(raw_rank).unwrap_or_else(|| {
                        if !matches!(raw_rank, 0 | -1) {
                            warn!(
                                "There might be a new other guild member \
                                 ranks -> {raw_rank}"
                            );
                        }
                        GuildRank::Member
                    });
                member.pet_lvl = soft_into(
                    required_guild_i64(data, 390 + idx)?,
                    "other guild pet levels",
                    0,
                );
            }

            data.ignore_unmodelled(
                &[
                    0..3,
                    4..8,
                    9..13,
                    14..64,
                    164..214,
                    364..390,
                    64 + member_count..114,
                    114 + member_count..164,
                    214 + member_count..264,
                    264 + member_count..314,
                    314 + member_count..364,
                    390 + member_count..data.len(),
                ],
                "unmodelled other guild values",
            );
            Ok(())
        })();
        if let Err(err) = parsed {
            data.ignore_all("malformed other guild record");
            return Err(err);
        }
        *self = updated;
        Ok(())
    }

    pub(crate) fn update_member_names(&mut self, data: &Vals<'_>) {
        self.members.resize_with(data.len(), Default::default);
        for (member, name) in self.members.iter_mut().zip(data.iter()) {
            name.clone_into(&mut member.name);
        }
    }
}

impl Character {
    pub(crate) fn update_relations(&mut self, data: &Vals<'_>) {
        self.relations.clear();
        for record in data.iter() {
            if record.is_empty() {
                continue;
            }
            let values = Vals::by(record, ',');
            let parsed = (|| {
                let id = values.u32(0)?;
                let name = values.str(1)?.to_owned();
                let guild = values.str(2)?.to_owned();
                let level = values.u16(3)?;
                let relation = match values.str(4)? {
                    "-1" => Some(Relationship::Ignored),
                    "1" => Some(Relationship::Friend),
                    _ => None,
                };
                values.ignore(5..values.len(), "extra friendslist values");
                Ok::<_, SFError>(relation.map(|relation| RelationEntry {
                    id,
                    name,
                    guild,
                    level,
                    relation,
                }))
            })();
            match parsed {
                Ok(Some(entry)) => self.relations.push(entry),
                Ok(None) => {}
                Err(_) => {
                    values.ignore_all("malformed friendslist record");
                    warn!("bad friendslist entry: {record}");
                }
            }
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

impl ClaimableMail {
    pub(crate) fn parse(
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Option<Self> {
        let (
            Ok(msg_id),
            Ok(raw_status),
            Ok(raw_typ),
            Ok(name),
            Ok(start),
            Ok(end),
        ) = (
            data.i64(0),
            data.str(1),
            data.i64(2),
            data.str(3),
            data.i64(4),
            data.i64(5),
        )
        else {
            data.ignore_all("malformed claimable mail record");
            return None;
        };

        let status = match raw_status {
            "0" => ClaimableStatus::Unread,
            "1" => ClaimableStatus::Read,
            "2" => ClaimableStatus::Claimed,
            x => {
                warn!("Unknown claimable status: {x}");
                ClaimableStatus::Claimed
            }
        };

        data.ignore(6..data.len(), "extra claimable mail values");
        Some(Self {
            msg_id,
            typ: FromPrimitive::from_i64(raw_typ).unwrap_or_default(),
            status,
            name: name.to_owned(),
            received: server_time.convert_to_local(start, "p start"),
            claimable_until: server_time.convert_to_local(end, "p end"),
        })
    }
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

impl ClaimablePreview {
    pub(crate) fn update_resources(&mut self, data: &Vals<'_>) {
        self.resources.clear();
        let complete_len = data.len() / 2 * 2;
        self.resources = data
            .window(0..complete_len)
            .chunks(2)
            .map(|chunk| Reward {
                typ: RewardType::parse(chunk.i64_or(0, 0)),
                amount: u64::try_from(chunk.i64_or(1, 1)).unwrap_or(1),
            })
            .collect();
        data.ignore(complete_len..data.len(), "partial claimable resource");
    }

    pub(crate) fn update_items(&mut self, data: &Vals<'_>) {
        self.items.clear();
        let complete_len = data.len() / ITEM_PARSE_LEN * ITEM_PARSE_LEN;
        self.items = data
            .window(0..complete_len)
            .chunks(ITEM_PARSE_LEN)
            .filter_map(|chunk| Item::parse(&chunk).ok().flatten())
            .collect();
        data.ignore(complete_len..data.len(), "partial claimable item");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use crate::gamestate::character::Gender;

    fn local_time(timestamp: i64) -> DateTime<Local> {
        DateTime::from_timestamp(timestamp, 0)
            .unwrap()
            .naive_utc()
            .and_local_timezone(Local)
            .latest()
            .unwrap()
    }

    fn other_player_values() -> Vec<String> {
        let mut values = vec!["0".to_owned(); 70];
        values[0] = "ignored".to_owned();
        values[1] = "123456".to_owned();
        values[2] = "ignored".to_owned();
        values[3] = "1048618".to_owned();
        values[4] = "9000000000".to_owned();
        values[5] = "12345678".to_owned();
        values[6] = "4321".to_owned();
        values[7] = "17".to_owned();
        values[8] = "4".to_owned();
        values[9] = "206".to_owned();
        values[10] = "103".to_owned();
        values[11] = "2".to_owned();
        values[12] = "104".to_owned();
        values[13] = "5".to_owned();
        values[14] = "6".to_owned();
        values[15] = "7".to_owned();
        values[16] = "108".to_owned();
        values[17] = "9".to_owned();
        values[18] = "3".to_owned();
        values[19] = "1".to_owned();
        values[20] = "4".to_owned();
        values[21] = "260".to_owned();
        values[22] = "ignored".to_owned();
        values[23] = "98765".to_owned();
        values[24] = "123".to_owned();
        values[25] = "456".to_owned();
        values[26] = "12".to_owned();
        values[27] = "ignored".to_owned();
        values[28] = "345".to_owned();
        values[29] = "1700000029".to_owned();
        for (idx, value) in values.iter_mut().enumerate().take(45).skip(30) {
            *value = (idx - 29).to_string();
        }
        values[45..66].fill("ignored".to_owned());
        values[66] = "10042".to_owned();
        values[67..69].fill("ignored".to_owned());
        values[69] = "31".to_owned();
        values
    }

    fn other_guild_values() -> Vec<String> {
        let mut values = vec!["0".to_owned(); 392];
        values[3] = "2".to_owned();
        values[8] = "53".to_owned();
        values[13] = "1288".to_owned();
        values[64] = "248".to_owned();
        values[65] = "365".to_owned();
        values[114] = "1700000001".to_owned();
        values[115] = "1700000002".to_owned();
        values[214] = "28".to_owned();
        values[215] = "31".to_owned();
        values[264] = "31".to_owned();
        values[265] = "55".to_owned();
        values[314] = "1".to_owned();
        values[315] = "2".to_owned();
        values[390] = "190".to_owned();
        values[391] = "180".to_owned();
        values
    }

    #[test]
    fn hellevator_hall_of_fame_parses_complete_records_and_ignores_extras() {
        let data = Vals::by("2,Alice,1234,extra", ',');
        let entry = HallOfFameHellevator::parse(&data).unwrap();
        data.assert_all_read();
        assert_eq!(entry.rank, 2);
        assert_eq!(entry.name, "Alice");
        assert_eq!(entry.tokens, 1234);
    }

    #[test]
    fn malformed_hellevator_hall_of_fame_records_are_dropped_whole() {
        for raw in ["", "1,Alice", "bad,Alice,100", "1,Alice,bad"] {
            let data = Vals::by(raw, ',');
            assert!(HallOfFameHellevator::parse(&data).is_none());
            data.assert_all_read();
        }
    }

    #[test]
    fn hellevator_hall_of_fame_update_replaces_skips_empty_and_drops_malformed()
    {
        let mut hall_of_fames = HallOfFames {
            hellevator: vec![HallOfFameHellevator::default()],
            ..Default::default()
        };
        let data = Vals::by(
            "1,Alice,100;;bad,Bob,200;,1,Leading,400;2,Carol,300;",
            ';',
        );
        hall_of_fames.update_hellevator(&data);
        data.assert_all_read();

        assert_eq!(hall_of_fames.hellevator.len(), 2);
        let alice = hall_of_fames.hellevator.first().unwrap();
        assert_eq!(alice.rank, 1);
        assert_eq!(alice.name, "Alice");
        assert_eq!(alice.tokens, 100);
        let carol = hall_of_fames.hellevator.get(1).unwrap();
        assert_eq!(carol.rank, 2);
        assert_eq!(carol.name, "Carol");
        assert_eq!(carol.tokens, 300);

        let empty = Vals::by("", ';');
        hall_of_fames.update_hellevator(&empty);
        empty.assert_all_read();
        assert!(matches!(hall_of_fames.hellevator.as_slice(), []));
    }

    #[test]
    fn claimable_mail_parses_required_fields_and_ignores_extras() {
        let data = Vals::from("42/1/12/CODE/1700000000/1700000100/extra");
        let mail = ClaimableMail::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(mail.msg_id, 42);
        assert_eq!(mail.status, ClaimableStatus::Read);
        assert_eq!(mail.typ, ClaimableMailType::TwitchDrop);
        assert_eq!(mail.name, "CODE");
        assert_eq!(mail.received, Some(local_time(1_700_000_000)));
        assert_eq!(mail.claimable_until, Some(local_time(1_700_000_100)));
        data.assert_all_read();
    }

    #[test]
    fn claimable_mail_drops_malformed_records_and_defaults_unknown_values() {
        for raw in [
            "",
            "1",
            "1/0/12",
            "bad/0/12/name/0/0",
            "1/0/bad/name/0/0",
            "1/0/12/name/bad/0",
            "1/0/12/name/0/bad",
        ] {
            let data = Vals::from(raw);
            assert!(
                ClaimableMail::parse(&data, ServerTime::default()).is_none()
            );
            data.assert_all_read();
        }

        let data = Vals::from("43/9/999/OTHER/0/0");
        let mail = ClaimableMail::parse(&data, ServerTime::default()).unwrap();
        assert_eq!(mail.status, ClaimableStatus::Claimed);
        assert_eq!(mail.typ, ClaimableMailType::GenericDelivery);
        assert_eq!(mail.received, None);
        assert_eq!(mail.claimable_until, None);
        data.assert_all_read();
    }

    #[test]
    fn mail_update_claimables_drops_a_partial_tail() {
        let mut mail = Mail::default();
        let data = Vals::from("1/0/10/A/0/0/2/1/11/B/0/0/3/0/12");
        mail.update_claimables(&data, ServerTime::default());
        data.assert_all_read();
        assert_eq!(mail.claimables.len(), 2);
        assert_eq!(mail.claimables[0].msg_id, 1);
        assert_eq!(mail.claimables[0].typ, ClaimableMailType::Coupon);
        assert_eq!(mail.claimables[1].msg_id, 2);
        assert_eq!(mail.claimables[1].typ, ClaimableMailType::SupermanDelivery);
    }

    #[test]
    fn claimable_preview_resources_use_default_one_and_ignore_partial_tail() {
        let mut preview = ClaimablePreview::default();
        let data = Vals::from("4/25/5/-1/bad/7/8");
        preview.update_resources(&data);
        data.assert_all_read();
        assert_eq!(preview.resources.len(), 3);
        assert_eq!(preview.resources[0].typ, RewardType::Silver);
        assert_eq!(preview.resources[0].amount, 25);
        assert_eq!(preview.resources[1].typ, RewardType::LuckyCoins);
        assert_eq!(preview.resources[1].amount, 1);
        assert_eq!(preview.resources[2].typ, RewardType::Unknown);
        assert_eq!(preview.resources[2].amount, 7);
    }

    #[test]
    fn claimable_preview_items_filter_empty_slots_and_ignore_partial_tail() {
        use crate::gamestate::items::ItemType;

        let mut preview = ClaimablePreview::default();
        let mut values = vec!["0".to_owned(); ITEM_PARSE_LEN * 2];
        values[0] = "6".to_owned();
        values.extend(["6", "0", "0"].map(str::to_owned));
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        preview.update_items(&data);
        data.assert_all_read();
        assert_eq!(preview.items.len(), 1);
        assert_eq!(preview.items[0].typ, ItemType::Hat);
    }

    #[test]
    fn hall_of_fame_parsers_read_required_fields_and_ignore_tails() {
        let players = Vals::by("1,Alice,,42,99,1,de,extra", ',');
        let player = HallOfFamePlayer::parse(&players).unwrap();
        players.assert_all_read();
        assert_eq!(player.rank, 1);
        assert_eq!(player.name, "Alice");
        assert_eq!(player.guild, None);
        assert_eq!(player.level, 42);
        assert_eq!(player.honor, 99);
        assert_eq!(player.class, Class::Warrior);
        assert_eq!(player.flag, Some(Flag::Germany));

        let players = Vals::by("2,Bob,Guild,12,34,2", ',');
        let player = HallOfFamePlayer::parse(&players).unwrap();
        players.assert_all_read();
        assert_eq!(player.guild.as_deref(), Some("Guild"));
        assert_eq!(player.class, Class::Mage);
        assert_eq!(player.flag, None);

        let guilds = Vals::by("3,Guild,Leader,4,500,1,extra", ',');
        let guild = HallOfFameGuild::parse(&guilds).unwrap();
        guilds.assert_all_read();
        assert_eq!(guild.rank, 3);
        assert_eq!(guild.name, "Guild");
        assert_eq!(guild.leader, "Leader");
        assert_eq!(guild.member_count, 4);
        assert_eq!(guild.honor, 500);
        assert!(guild.is_attacked);

        let pets = Vals::by("4,PetOwner,Guild,20,600,-7,extra", ',');
        let pets_entry = HallOfFamePets::parse(&pets).unwrap();
        pets.assert_all_read();
        assert_eq!(pets_entry.rank, 4);
        assert_eq!(pets_entry.name, "PetOwner");
        assert_eq!(pets_entry.guild.as_deref(), Some("Guild"));
        assert_eq!(pets_entry.collected, 20);
        assert_eq!(pets_entry.honor, 600);
        assert_eq!(pets_entry.unknown, -7);

        let fortresses = Vals::by("5,FortOwner,,30,700,extra", ',');
        let fortress = HallOfFameFortress::parse(&fortresses).unwrap();
        fortresses.assert_all_read();
        assert_eq!(fortress.rank, 5);
        assert_eq!(fortress.name, "FortOwner");
        assert_eq!(fortress.guild, None);
        assert_eq!(fortress.upgrade, 30);
        assert_eq!(fortress.honor, 700);

        let underworlds = Vals::by("6,UWOwner,Guild,40,800,9,extra", ',');
        let underworld = HallOfFameUnderworld::parse(&underworlds).unwrap();
        underworlds.assert_all_read();
        assert_eq!(underworld.rank, 6);
        assert_eq!(underworld.name, "UWOwner");
        assert_eq!(underworld.guild.as_deref(), Some("Guild"));
        assert_eq!(underworld.upgrade, 40);
        assert_eq!(underworld.honor, 800);
        assert_eq!(underworld.unknown, 9);
    }

    #[test]
    fn malformed_hall_of_fame_records_are_consumed_and_rejected_whole() {
        for raw in [
            "",
            "1,Alice",
            "bad,Alice,Guild,42,99,1,de",
            "1,Alice,Guild,bad,99,1,de",
            "1,Alice,Guild,42,bad,1,de",
            "1,Alice,Guild,42,99,bad,de",
            "1,Alice,Guild,42,99,0,de",
        ] {
            let data = Vals::by(raw, ',');
            assert!(HallOfFamePlayer::parse(&data).is_err());
            data.assert_all_read();
        }
        for raw in [
            "",
            "1,Guild,Leader,2,30",
            "1,Guild,Leader,bad,30,1",
            "1,Guild,Leader,2,bad,1",
            "1,Guild,Leader,2,30,256",
        ] {
            let data = Vals::by(raw, ',');
            assert!(HallOfFameGuild::parse(&data).is_err());
            data.assert_all_read();
        }
        for raw in [
            "",
            "1,Alice,Guild,20",
            "1,Alice,Guild,bad,600,1",
            "1,Alice,Guild,20,600,bad",
        ] {
            let data = Vals::by(raw, ',');
            assert!(HallOfFamePets::parse(&data).is_err());
            data.assert_all_read();
        }
        for raw in [
            "",
            "1,Alice,Guild,30",
            "bad,Alice,Guild,30,700",
            "1,Alice,Guild,bad,700",
            "1,Alice,Guild,30,bad",
        ] {
            let data = Vals::by(raw, ',');
            assert!(HallOfFameFortress::parse(&data).is_err());
            data.assert_all_read();
        }
        for raw in [
            "",
            "1,Alice,Guild,40",
            "1,Alice,Guild,bad,800,9",
            "1,Alice,Guild,40,bad,9",
            "1,Alice,Guild,40,800,bad",
        ] {
            let data = Vals::by(raw, ',');
            assert!(HallOfFameUnderworld::parse(&data).is_err());
            data.assert_all_read();
        }

        let data = Vals::by("1,Alice,Guild,42,99,0,de", ',');
        let err = HallOfFamePlayer::parse(&data).unwrap_err();
        data.assert_all_read();
        assert!(matches!(err, SFError::ParsingError(name, value)
            if name == "hof player class" && value == "-1"));
    }

    #[test]
    fn hall_of_fame_player_update_stops_at_terminator_and_drops_bad_records() {
        let mut hof = HallOfFames {
            players: vec![HallOfFamePlayer::default()],
            ..Default::default()
        };
        let data = Vals::by(
            "1,Alice,Guild,42,99,1,de;;bad;2,Bob,,12,34,2;9,Stop,,,,0,0,0,;10,\
             After,,,,0,0,0,",
            ';',
        );
        hof.update_players(&data);
        data.assert_all_read();
        assert_eq!(hof.players.len(), 2);
        assert_eq!(hof.players[0].name, "Alice");
        assert_eq!(hof.players[1].name, "Bob");
    }

    #[test]
    fn all_other_hall_of_fame_updates_replace_and_drop_malformed_records() {
        let mut hof = HallOfFames {
            guilds: vec![HallOfFameGuild::default()],
            pets: vec![HallOfFamePets::default()],
            fortresses: vec![HallOfFameFortress::default()],
            underworlds: vec![HallOfFameUnderworld::default()],
            ..Default::default()
        };

        let guilds = Vals::by(
            "1,Guild,Leader,2,30,1;;bad,Missing,Fields;2,Other,Lead,1,20",
            ';',
        );
        hof.update_guilds(&guilds);
        guilds.assert_all_read();
        assert_eq!(hof.guilds.len(), 1);
        assert_eq!(hof.guilds[0].name, "Guild");

        let pets = Vals::by("1,Alice,Guild,20,600,1;;bad;2,Bob,,10,300", ';');
        hof.update_pets(&pets);
        pets.assert_all_read();
        assert_eq!(hof.pets.len(), 1);
        assert_eq!(hof.pets[0].name, "Alice");

        let fortresses =
            Vals::by("1,Alice,Guild,30,700;;bad,Missing;2,Bob,,20", ';');
        hof.update_fortresses(&fortresses);
        fortresses.assert_all_read();
        assert_eq!(hof.fortresses.len(), 1);
        assert_eq!(hof.fortresses[0].name, "Alice");

        let underworlds =
            Vals::by("1,Alice,Guild,40,800,9;;bad,Missing;2,Bob,,20,500", ';');
        hof.update_underworlds(&underworlds);
        underworlds.assert_all_read();
        assert_eq!(hof.underworlds.len(), 1);
        assert_eq!(hof.underworlds[0].name, "Alice");

        let empty = Vals::by("", ';');
        hof.update_guilds(&empty);
        hof.update_pets(&empty);
        hof.update_fortresses(&empty);
        hof.update_underworlds(&empty);
        empty.assert_all_read();
        assert!(hof.guilds.is_empty());
        assert!(hof.pets.is_empty());
        assert!(hof.fortresses.is_empty());
        assert!(hof.underworlds.is_empty());
    }

    #[test]
    fn friendslist_keeps_only_ignored_and_friend_relations() {
        let mut character = Character {
            relations: vec![RelationEntry::default()],
            ..Default::default()
        };
        let data = Vals::by(
            "1,Alice,Guild,10,-1,extra;;2,Bob,H,11,0;3,Carol,I,12,1;4,Dave,J,\
             13,2;bad,entry",
            ';',
        );
        character.update_relations(&data);
        data.assert_all_read();

        assert_eq!(character.relations.len(), 2);
        assert_eq!(character.relations[0].id, 1);
        assert_eq!(character.relations[0].name, "Alice");
        assert_eq!(character.relations[0].guild, "Guild");
        assert_eq!(character.relations[0].level, 10);
        assert_eq!(character.relations[0].relation, Relationship::Ignored);
        assert_eq!(character.relations[1].id, 3);
        assert_eq!(character.relations[1].relation, Relationship::Friend);
    }

    #[test]
    fn news_entries_parse_timestamps_and_drop_malformed_records() {
        let data = Vals::by("1,Title$CWithComma,1700000000,extra", ',');
        let entry = NewsEntry::parse(&data, ServerTime::default()).unwrap();
        data.assert_all_read();
        assert_eq!(entry.news_id, 1_700_000_000);
        assert_eq!(entry.title, "Title,WithComma");
        assert_eq!(entry.date, local_time(1_700_000_000));
        assert!(entry.read);

        for raw in ["1,Title", "1,Title,bad", "1,Title,0", "Title,1700000000"] {
            let data = Vals::by(raw, ',');
            assert!(NewsEntry::parse(&data, ServerTime::default()).is_err());
            data.assert_all_read();
        }

        let mut mail = Mail {
            news_inbox: vec![NewsEntry {
                news_id: 0,
                title: "old".to_owned(),
                date: local_time(1_700_000_000),
                read: false,
            }],
            ..Default::default()
        };
        let records = Vals::by(
            "0,Good,1700000001;;1,Bad,bad;1,Invalid,0;1,AlsoGood,1700000002",
            ';',
        );
        mail.update_news_inbox(&records, ServerTime::default());
        records.assert_all_read();
        assert_eq!(mail.news_inbox.len(), 2);
        assert_eq!(mail.news_inbox[0].title, "Good");
        assert!(!mail.news_inbox[0].read);
        assert_eq!(mail.news_inbox[1].title, "AlsoGood");
    }

    #[test]
    fn inbox_entries_preserve_comma_titles_spaces_types_and_read_state() {
        let server_time = ServerTime::default();
        let comma = InboxEntry::parse(
            "42,Alice,1,Hello, world,1700000000",
            server_time,
        )
        .unwrap();
        assert_eq!(comma.msg_id, 42);
        assert_eq!(comma.from, "Alice");
        assert_eq!(comma.title, "Hello, world");
        assert!(comma.read);
        assert!(matches!(comma.msg_typ, MessageType::Normal));

        let spaces =
            InboxEntry::parse("43,Bob,0,   ,1700000000", server_time).unwrap();
        assert_eq!(spaces.title, "   ");
        assert!(!spaces.read);
        assert!(matches!(spaces.msg_typ, MessageType::Normal));

        let kicked =
            InboxEntry::parse("44,Carol,2,3,1700000000", server_time).unwrap();
        assert!(matches!(kicked.msg_typ, MessageType::GuildKicked));
        assert!(kicked.read);

        let invited =
            InboxEntry::parse("45,Dave,0,5,1700000000", server_time).unwrap();
        assert!(matches!(invited.msg_typ, MessageType::GuildInvite));
        assert!(!invited.read);

        assert!(
            InboxEntry::parse("46,Erin,1,,1700000000", server_time).is_err()
        );

        let mut mail = Mail {
            inbox: vec![InboxEntry {
                msg_typ: MessageType::Normal,
                from: "old".to_owned(),
                msg_id: 0,
                title: "old".to_owned(),
                date: local_time(1_700_000_000),
                read: false,
            }],
            ..Default::default()
        };
        let records = Vals::by(
            "42,Alice,1,Hello, \
             world,1700000000;;43,Bob,0,,1700000000;44,Carol,0,Valid,\
             1700000001",
            ';',
        );
        mail.update_inbox(&records, server_time);
        records.assert_all_read();
        assert_eq!(mail.inbox.len(), 2);
        assert_eq!(mail.inbox[0].title, "Hello, world");
        assert_eq!(mail.inbox[1].title, "Valid");
    }

    #[test]
    fn combat_log_parses_known_types_and_consumes_all_tail_values() {
        let data = Vals::by("7,Alice,1,2,1700000000,extra,more", ',');
        let entry =
            CombatLogEntry::parse(&data, ServerTime::default()).unwrap();
        data.assert_all_read();
        assert_eq!(entry.msg_id, 7);
        assert_eq!(entry.player_name, "Alice");
        assert!(entry.won);
        assert!(matches!(entry.battle_type, CombatMessageType::GuildFight));
        assert_eq!(entry.time, local_time(1_700_000_000));

        let unknown = Vals::by("8,Bob,0,999,1700000000,tail", ',');
        let err =
            CombatLogEntry::parse(&unknown, ServerTime::default()).unwrap_err();
        unknown.assert_all_read();
        assert!(matches!(err, SFError::ParsingError(name, value)
            if name == "combat mt" && value.contains("999 @")));

        let bad_time = Vals::by("9,Bob,0,2,0,tail", ',');
        let err = CombatLogEntry::parse(&bad_time, ServerTime::default())
            .unwrap_err();
        bad_time.assert_all_read();
        assert!(matches!(err, SFError::ParsingError(name, _)
            if name == "combat time"));
    }

    #[test]
    fn combat_log_update_skips_all_empty_records_and_drops_malformed_ones() {
        let mut mail = Mail {
            combat_log: vec![CombatLogEntry {
                msg_id: 0,
                player_name: "old".to_owned(),
                won: false,
                battle_type: CombatMessageType::Arena,
                time: local_time(1_700_000_000),
            }],
            ..Default::default()
        };
        let empty = Vals::by(",,,,;,,;;;,", ';');
        mail.update_combat_log(&empty, ServerTime::default());
        empty.assert_all_read();
        assert!(mail.combat_log.is_empty());

        let records = Vals::by(
            "7,Alice,1,2,1700000000,extra;;bad;8,Bob,0,999,1700000000;9,Carol,\
             0,3,1700000001,tail",
            ';',
        );
        mail.update_combat_log(&records, ServerTime::default());
        records.assert_all_read();
        assert_eq!(mail.combat_log.len(), 2);
        assert_eq!(mail.combat_log[0].player_name, "Alice");
        assert_eq!(mail.combat_log[1].player_name, "Carol");
    }

    #[test]
    fn other_player_update_parses_a_full_seventy_value_save() {
        let server_time = ServerTime(3600);
        let raw = other_player_values().join("/");
        let data = Vals::from(raw.as_str());
        assert_eq!(data.len(), 70);
        let mut player = OtherPlayer::default();
        player.update(&data, server_time).unwrap();
        data.assert_all_read();

        assert_eq!(player.player_id, 123_456);
        assert_eq!(player.level, 42);
        assert_eq!(player.experience, 9_000_000_000);
        assert_eq!(player.next_level_xp, 12_345_678);
        assert_eq!(player.honor, 4_321);
        assert_eq!(player.rank, 17);
        assert_eq!(player.race, Race::Dwarf);
        assert_eq!(player.class, Class::Assassin);
        assert_eq!(player.mount, Some(Mount::Dragon));
        assert_eq!(player.armor, 98_765);
        assert_eq!(player.min_damage, 123);
        assert_eq!(player.max_damage, 456);
        assert_eq!(player.portal_dmg_bonus, 12);
        assert_eq!(player.portal_hp_bonus, 345);
        assert_eq!(player.scrapbook_count, Some(42));
        assert_eq!(player.gladiator_lvl, 31);
        assert_eq!(
            player.mount_end,
            server_time.convert_to_local(1_700_000_029, "test")
        );
        assert_eq!(
            player.portrait,
            Portrait {
                gender: Gender::Male,
                hair_color: 2,
                hair: 6,
                mouth: 4,
                brows: 3,
                eyes: 2,
                beards: 4,
                nose: 5,
                ears: 6,
                extra: 7,
                horns: 8,
                special_portrait: 9,
            }
        );
        assert_eq!(
            player.attribute_basis,
            EnumMap::from_array([1, 2, 3, 4, 5])
        );
        assert_eq!(
            player.attribute_additions,
            EnumMap::from_array([6, 7, 8, 9, 10])
        );
        assert_eq!(
            player.attribute_times_bought,
            EnumMap::from_array([11, 12, 13, 14, 15])
        );
    }

    #[test]
    fn other_player_race_class_and_portal_errors_do_not_partially_mutate() {
        let server_time = ServerTime::default();
        let mut player = OtherPlayer {
            player_id: 7,
            name: "unchanged".to_owned(),
            level: 9,
            scrapbook_count: Some(3),
            ..Default::default()
        };
        let before = format!("{player:?}");

        for (pos, replacement, expected_name) in [
            (18, "99", "char race"),
            (20, "0", "character class"),
            (26, "-1", "portal dmg bonus"),
        ] {
            let mut values = other_player_values();
            values[pos] = replacement.to_owned();
            let raw = values.join("/");
            let data = Vals::from(raw.as_str());
            let err = player.update(&data, server_time).unwrap_err();
            data.assert_all_read();
            assert!(matches!(err, SFError::ParsingError(name, _)
                if name == expected_name));
            assert_eq!(format!("{player:?}"), before);
        }

        let mut values = other_player_values();
        values[30] = "bad".to_owned();
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        assert!(player.update(&data, server_time).is_err());
        data.assert_all_read();
        assert_eq!(format!("{player:?}"), before);
    }

    #[test]
    fn other_player_attribute_blocks_preserve_missing_trailing_values() {
        let mut attributes: EnumMap<AttributeType, u32> =
            EnumMap::from_array([10, 20, 30, 40, 50]);
        let data = Vals::from("1/-1");
        update_attribute_block(&mut attributes, &data, 0).unwrap();
        data.assert_all_read();
        assert_eq!(attributes, EnumMap::from_array([1, 0, 30, 40, 50]));
    }

    #[test]
    fn other_player_scrapbook_below_ten_thousand_preserves_existing_count() {
        let mut values = other_player_values();
        values[66] = "42".to_owned();
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut player = OtherPlayer {
            scrapbook_count: Some(7),
            ..Default::default()
        };
        player.update(&data, ServerTime::default()).unwrap();
        data.assert_all_read();
        assert_eq!(player.scrapbook_count, Some(7));
    }

    #[test]
    fn other_player_pet_bonus_is_strict_and_all_or_nothing() {
        let data = Vals::from("ignored/1/2/3/4/5/extra");
        let mut player = OtherPlayer::default();
        player.update_pet_bonus(&data).unwrap();
        data.assert_all_read();
        assert_eq!(
            player.attribute_pet_bonus,
            EnumMap::from_array([5, 2, 3, 1, 4])
        );

        let before = format!("{player:?}");
        for raw in ["0/1/2/bad/4/5/tail", "0/1/2/3/4", "0/-1/2/3/4/5"] {
            let data = Vals::from(raw);
            assert!(player.update_pet_bonus(&data).is_err());
            data.assert_all_read();
            assert_eq!(format!("{player:?}"), before);
        }
    }

    #[test]
    fn other_player_fortress_requires_all_fields_and_soft_defaults_ranges() {
        let mut player = OtherPlayer {
            fortress: Some(OtherFortress {
                soldier_advice: 99,
                rank: 7,
                ..Default::default()
            }),
            ..Default::default()
        };
        let data = Vals::from("10/11/12/13/14/15/extra");
        player.update_fortress(&data).unwrap();
        data.assert_all_read();
        let fortress = player.fortress.as_ref().unwrap();
        assert_eq!(fortress.upgrade_count, 10);
        assert_eq!(fortress.fortifications_level, 11);
        assert_eq!(fortress.archer_count, 12);
        assert_eq!(fortress.mage_count, 13);
        assert_eq!(fortress.lootable_wood, 14);
        assert_eq!(fortress.lootable_stone, 15);
        assert_eq!(fortress.soldier_advice, 99);
        assert_eq!(fortress.rank, 7);

        let before = format!("{player:?}");
        for raw in ["1/2/3/bad/5/6/tail", "1/2/3/4/5"] {
            let data = Vals::from(raw);
            assert!(player.update_fortress(&data).is_err());
            data.assert_all_read();
            assert_eq!(format!("{player:?}"), before);
        }

        let data = Vals::from("-1/-1/-1/-1/-1/-1");
        player.update_fortress(&data).unwrap();
        data.assert_all_read();
        let fortress = player.fortress.as_ref().unwrap();
        assert_eq!(fortress.upgrade_count, 0);
        assert_eq!(fortress.fortifications_level, 0);
        assert_eq!(fortress.archer_count, 0);
        assert_eq!(fortress.mage_count, 0);
        assert_eq!(fortress.lootable_wood, 0);
        assert_eq!(fortress.lootable_stone, 0);
    }

    #[test]
    fn other_player_unit_levels_require_field_zero_and_soft_convert_it() {
        let mut player = OtherPlayer {
            wall_combat_lvl: 9,
            ..Default::default()
        };
        let data = Vals::from("42/ignored/also ignored");
        player.update_unit_levels(&data).unwrap();
        data.assert_all_read();
        assert_eq!(player.wall_combat_lvl, 42);

        let data = Vals::from("70000/tail");
        player.update_unit_levels(&data).unwrap();
        data.assert_all_read();
        assert_eq!(player.wall_combat_lvl, 0);

        let data = Vals::from("-1/tail");
        player.update_unit_levels(&data).unwrap();
        data.assert_all_read();
        assert_eq!(player.wall_combat_lvl, 0);

        let before = format!("{player:?}");
        for raw in ["", "bad/tail"] {
            let data = Vals::from(raw);
            assert!(player.update_unit_levels(&data).is_err());
            data.assert_all_read();
            assert_eq!(format!("{player:?}"), before);
        }
    }

    #[test]
    fn other_guild_update_parses_two_members_from_the_full_layout() {
        let raw = other_guild_values().join("/");
        let data = Vals::from(raw.as_str());
        let mut guild = OtherGuild {
            members: vec![OtherGuildMember::default(); 5],
            ..Default::default()
        };
        guild.update(&data, ServerTime::default()).unwrap();
        data.assert_all_read();

        assert_eq!(guild.member_count, 2);
        assert_eq!(guild.members.len(), 2);
        assert_eq!(guild.finished_raids, 53);
        assert_eq!(guild.honor, 1288);
        assert_eq!(guild.members[0].level, 248);
        assert_eq!(guild.members[1].level, 365);
        assert_eq!(
            guild.members[0].last_active,
            Some(local_time(1_700_000_001))
        );
        assert_eq!(
            guild.members[1].last_active,
            Some(local_time(1_700_000_002))
        );
        assert_eq!(guild.members[0].treasure_lvl, 28);
        assert_eq!(guild.members[1].treasure_lvl, 31);
        assert_eq!(guild.members[0].instructor_lvl, 31);
        assert_eq!(guild.members[1].instructor_lvl, 55);
        assert!(matches!(guild.members[0].rank, GuildRank::Leader));
        assert!(matches!(guild.members[1].rank, GuildRank::Officer));
        assert_eq!(guild.members[0].pet_lvl, 190);
        assert_eq!(guild.members[1].pet_lvl, 180);
    }

    #[test]
    fn other_guild_update_tolerates_nonnumeric_tokens_and_defaults_rank() {
        let mut values = other_guild_values();
        values[8] = "future".to_owned();
        values[13] = "future".to_owned();
        values[64] = "future".to_owned();
        values[114] = "future".to_owned();
        values[214] = "future".to_owned();
        values[264] = "-1".to_owned();
        values[314] = "99".to_owned();
        values[390] = "future".to_owned();
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut guild = OtherGuild::default();
        guild.update(&data, ServerTime::default()).unwrap();
        data.assert_all_read();

        assert_eq!(guild.finished_raids, 0);
        assert_eq!(guild.honor, 0);
        assert_eq!(guild.members[0].level, 0);
        assert_eq!(guild.members[0].last_active, None);
        assert_eq!(guild.members[0].treasure_lvl, 0);
        assert_eq!(guild.members[0].instructor_lvl, 0);
        assert!(matches!(guild.members[0].rank, GuildRank::Member));
        assert_eq!(guild.members[0].pet_lvl, 0);
    }

    #[test]
    fn other_guild_missing_member_fields_do_not_partially_mutate() {
        let values = other_guild_values();
        let raw = values[..115].join("/");
        let data = Vals::from(raw.as_str());
        let mut guild = OtherGuild {
            name: "unchanged".to_owned(),
            honor: 9,
            members: vec![OtherGuildMember::default(); 3],
            ..Default::default()
        };
        let before = format!("{guild:?}");
        assert!(guild.update(&data, ServerTime::default()).is_err());
        data.assert_all_read();
        assert_eq!(format!("{guild:?}"), before);
    }

    #[test]
    fn other_guild_member_names_resize_exactly_when_shrinking() {
        let mut guild = OtherGuild {
            members: vec![OtherGuildMember::default(); 3],
            ..Default::default()
        };
        let names = Vals::by("One,Two", ',');
        guild.update_member_names(&names);
        names.assert_all_read();
        assert_eq!(guild.members.len(), 2);
        assert_eq!(guild.members[0].name, "One");
        assert_eq!(guild.members[1].name, "Two");

        let names = Vals::by("Only", ',');
        guild.update_member_names(&names);
        names.assert_all_read();
        assert_eq!(guild.members.len(), 1);
        assert_eq!(guild.members[0].name, "Only");
    }
}
