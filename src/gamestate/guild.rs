#![allow(clippy::module_name_repetitions)]
use chrono::{DateTime, Local, NaiveTime};
use enum_map::EnumMap;
use log::warn;
use num_derive::FromPrimitive;

use super::{
    items::{ItemType, PotionSize, PotionType},
    update_enum_map, ArrSkip, AttributeType, CCGet, CFPGet, CGet, CSTGet,
    NormalCost, Potion, SFError, ServerTime,
};
use crate::misc::{from_sf_string, soft_into, warning_parse};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Information about the characters current guild
pub struct Guild {
    /// The internal server id of this guild
    pub id: u32,
    /// The name of the guild
    pub name: String,
    /// The description text of the guild
    pub description: String,
    /// This is guilds emblem. Currently this is unparsed, so you only have
    /// access to the raw string
    pub emblem: Emblem,

    /// The honor this guild has earned
    pub honor: u32,
    /// The rank in the Hall of Fame this guild has
    pub rank: u32,
    /// The date at which the character joined this guild
    pub joined: DateTime<Local>,

    /// The skill you yourself contribute to the guild
    pub own_treasure_skill: u16,
    /// The price to pay to upgrade your treasure by one rank
    pub own_treasure_upgrade: NormalCost,

    /// The skill you yourself contribute to the guild
    pub own_instructor_skill: u16,
    /// The price to pay to upgrade your instructor by one rank
    pub own_instructor_upgrade: NormalCost,

    /// How many raids this guild has completed already
    pub finished_raids: u16,

    /// If the guild is defending against another guild, this will contain
    /// information about the upcoming battle
    pub defending: Option<PlanedBattle>,
    /// If the guild is attacking another guild, this will contain
    /// information about the upcoming battle
    pub attacking: Option<PlanedBattle>,

    /// The id of the pet, that is currently selected as the guild pet
    pub pet_id: u32,
    /// The maximum level, that the pet can be at
    pub pet_max_lvl: u16,
    /// All information about the hydra the guild pet can fight
    pub hydra: GuildHydra,
    /// The thing each player can enter and fight once a day
    pub portal: GuildPortal,

    // This should just be members.len(). I think this is only in the API
    // because they are bad at varsize arrays or smth.
    member_count: u8,
    /// Information about the members of the guild. This includes the player
    pub members: Vec<GuildMemberData>,
    /// The chat messages, that get send in the guild chat
    pub chat: Vec<ChatMessage>,
    /// The whisper messages, that a player can receive
    pub whispers: Vec<ChatMessage>,

    /// A list of guilds which can be fought, must first be fetched by sending
    /// `Command::GuildGetFightableTargets`
    pub fightable_guilds: Vec<FightableGuild>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The hydra, that the guild pet can fight
pub struct GuildHydra {
    /// The last time the hydra has been fought
    pub last_battle: Option<DateTime<Local>>,
    /// The last time the hydra has been seen with full health
    pub last_full: Option<DateTime<Local>>,
    /// This seems to be `last_battle + 30 min`. I can only do 1 battle/day,
    /// but I think this should be the next possible fight
    pub next_battle: Option<DateTime<Local>>,
    /// The amount of times the player can still fight the hydra
    pub remaining_fights: u16,
    /// The current life of the guilds hydra
    pub current_life: u64,
    /// The maximum life the hydra can have
    pub max_life: u64,
    /// The attributes the hydra has
    pub attributes: EnumMap<AttributeType, u32>,
}

/// Contains information about another guild which can be fought.
/// Must first be fetched by sending `Command::GuildGetFightableTargets`
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FightableGuild {
    /// Id of the guild
    pub id: u32,
    /// Name of the guild
    pub name: String,
    /// Emblem of the guild
    pub emblem: Emblem,
    /// Number of members the guild currently has
    pub number_of_members: u8,
    /// The lowest level a member of the guild has
    pub members_min_level: u32,
    /// The highest level a member of the guild has
    pub members_max_level: u32,
    /// The average level of the guild members
    pub members_average_level: u32,
    /// The rank of the guild in the hall of fame
    pub rank: u32,
    /// The amount of honor the guild currently has
    pub honor: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The customizable emblem each guild has
pub struct Emblem {
    raw: String,
}

impl Emblem {
    /// Returns the guild emblem in it's server encoded form
    #[must_use]
    pub fn server_encode(&self) -> String {
        // TODO: Actually parse this
        self.raw.clone()
    }

    pub(crate) fn update(&mut self, str: &str) {
        self.raw.clear();
        self.raw.push_str(str);
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A message, that the player has received, or has send to others via the chat
pub struct ChatMessage {
    /// The user this message originated from. Note that this might not be in
    /// the guild member list in some cases
    pub user: String,
    /// The time at which this message has been sent. I have not checked the
    /// timezone here. Might be UTC/Your TZ/Server TZ
    pub time: NaiveTime,
    /// The actual bessage, that got send
    pub message: String,
}

impl ChatMessage {
    pub(crate) fn parse_messages(data: &str) -> Vec<ChatMessage> {
        data.split('/')
            .filter_map(|msg| {
                let (time, rest) = msg.split_once(' ')?;
                let (name, msg) = rest.split_once(':')?;
                let msg = from_sf_string(msg.trim_start_matches(['§', ' ']));
                let time = NaiveTime::parse_from_str(time, "%H:%M").ok()?;
                Some(ChatMessage {
                    user: name.to_string(),
                    time,
                    message: msg,
                })
            })
            .collect()
    }
}

impl Guild {
    pub(crate) fn update_group_save(
        &mut self,
        val: &str,
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        let data: Vec<_> = val
            .split('/')
            .map(|c| c.trim().parse::<i64>().unwrap_or_default())
            .collect();

        let member_count = data.csiget(3, "member count", 0)?;
        self.member_count = member_count;
        self.members
            .resize_with(member_count as usize, Default::default);

        for (offset, member) in self.members.iter_mut().enumerate() {
            member.battles_joined =
                data.cfpget(445 + offset, "member fights joined", |x| x % 100)?;
            member.level = data.csiget(64 + offset, "member level", 0)?;
            member.last_online =
                data.cstget(114 + offset, "member last online", server_time)?;
            member.treasure_skill =
                data.csiget(214 + offset, "member treasure skill", 0)?;
            member.instructor_skill =
                data.csiget(264 + offset, "member master skill", 0)?;
            member.guild_rank = match data.cget(314 + offset, "member rank")? {
                1 => GuildRank::Leader,
                2 => GuildRank::Officer,
                3 => GuildRank::Member,
                4 => GuildRank::Invited,
                x => {
                    warn!("Unknown guild rank: {x}");
                    GuildRank::Invited
                }
            };
            member.portal_fought =
                data.cstget(164 + offset, "member portal fought", server_time)?;
            member.guild_pet_lvl =
                data.csiget(390 + offset, "member pet skill", 0)?;
        }

        self.honor = data.csiget(13, "guild honor", 0)?;
        self.id = data.csiget(0, "guild id", 0)?;

        self.finished_raids = data.csiget(8, "finished raids", 0)?;

        self.attacking = PlanedBattle::parse(
            data.skip(364, "attacking guild")?,
            server_time,
        )?;

        self.defending = PlanedBattle::parse(
            data.skip(366, "attacking guild")?,
            server_time,
        )?;

        self.pet_id = data.csiget(377, "gpet id", 0)?;
        self.pet_max_lvl = data.csiget(378, "gpet max lvl", 0)?;

        self.hydra.last_battle =
            data.cstget(382, "hydra pet lb", server_time)?;
        self.hydra.last_full =
            data.cstget(381, "hydra last defeat", server_time)?;

        self.hydra.current_life = data.csiget(383, "ghydra clife", u64::MAX)?;
        self.hydra.max_life = data.csiget(384, "ghydra max clife", u64::MAX)?;

        update_enum_map(
            &mut self.hydra.attributes,
            data.skip(385, "hydra attributes")?,
        );

        self.portal.life_percentage =
            data.csimget(6, "guild portal life p", 100, |x| x >> 16)?;
        self.portal.defeated_count =
            data.csimget(7, "guild portal progress", 0, |x| x >> 16)?;
        Ok(())
    }

    pub(crate) fn update_member_names(&mut self, val: &str) {
        let names: Vec<_> = val
            .split(',')
            .map(std::string::ToString::to_string)
            .collect();
        self.members.resize_with(names.len(), Default::default);
        for (member, name) in self.members.iter_mut().zip(names) {
            member.name = name;
        }
    }

    pub(crate) fn update_group_knights(&mut self, val: &str) {
        let data: Vec<i64> = val
            .trim_end_matches(',')
            .split(',')
            .flat_map(str::parse)
            .collect();

        self.members.resize_with(data.len(), Default::default);
        for (member, count) in self.members.iter_mut().zip(data) {
            member.knights = soft_into(count, "guild knight", 0);
        }
    }

    pub(crate) fn update_member_potions(&mut self, val: &str) {
        let data = val
            .trim_end_matches(',')
            .split(',')
            .map(|c| {
                warning_parse(c, "member potion", |a| a.parse::<i64>().ok())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();

        let potions = data.len() / 2;
        let member = potions / 3;
        self.members.resize_with(member, Default::default);

        let mut data = data.into_iter();

        let quick_potion = |int: i64| {
            Some(ItemType::Potion(Potion {
                typ: PotionType::parse(int)?,
                size: PotionSize::parse(int)?,
                expires: None,
            }))
        };

        for member in &mut self.members {
            for potion in &mut member.potions {
                *potion = data
                    .next()
                    .or_else(|| {
                        warn!("Invalid member potion len");
                        None
                    })
                    .and_then(quick_potion);
                _ = data.next();
            }
        }
    }

    pub(crate) fn update_description_embed(&mut self, data: &str) {
        let Some((emblem, description)) = data.split_once('§') else {
            self.description = from_sf_string(data);
            return;
        };

        self.description = from_sf_string(description);
        self.emblem.update(emblem);
    }

    pub(crate) fn update_group_prices(
        &mut self,
        data: &[i64],
    ) -> Result<(), SFError> {
        self.own_treasure_upgrade.silver =
            data.csiget(0, "treasure upgr. silver", 0)?;
        self.own_treasure_upgrade.mushrooms =
            data.csiget(1, "treasure upgr. mush", 0)?;
        self.own_instructor_upgrade.silver =
            data.csiget(2, "instr upgr. silver", 0)?;
        self.own_instructor_upgrade.mushrooms =
            data.csiget(3, "instr upgr. mush", 0)?;
        Ok(())
    }

    #[allow(clippy::indexing_slicing)]
    pub(crate) fn update_fightable_targets(
        &mut self,
        data: &str,
    ) -> Result<(), SFError> {
        const SIZE: usize = 9;

        // Delete any old data
        self.fightable_guilds.clear();

        let entries = data.trim_end_matches('/').split('/').collect::<Vec<_>>();

        let target_counts = entries.len() / SIZE;

        // Check if the data is valid
        if target_counts * SIZE != entries.len() {
            warn!("Invalid fightable targets len");
            return Err(SFError::ParsingError(
                "Fightable targets invalid length",
                data.to_string(),
            ));
        }

        // Reserve space for the new data
        self.fightable_guilds.reserve(entries.len() / SIZE);

        for i in 0..entries.len() / SIZE {
            let offset = i * SIZE;

            self.fightable_guilds.push(FightableGuild {
                id: entries[offset].parse().unwrap_or_default(),
                name: from_sf_string(entries[offset + 1]),
                emblem: Emblem {
                    raw: entries[offset + 2].to_string(),
                },
                number_of_members: entries[offset + 3]
                    .parse()
                    .unwrap_or_default(),
                members_min_level: entries[offset + 4]
                    .parse()
                    .unwrap_or_default(),
                members_max_level: entries[offset + 5]
                    .parse()
                    .unwrap_or_default(),
                members_average_level: entries[offset + 6]
                    .parse()
                    .unwrap_or_default(),
                rank: entries[offset + 7].parse().unwrap_or_default(),
                honor: entries[offset + 8].parse().unwrap_or_default(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A guild battle, that is scheduled to take place at a certain place and time
pub struct PlanedBattle {
    /// The guild this battle will be against
    pub other: u32,
    /// The date & time this battle will be at
    pub date: DateTime<Local>,
}

impl PlanedBattle {
    /// Checks if the battle is a raid
    #[must_use]
    pub fn is_raid(&self) -> bool {
        self.other == 1_000_000
    }

    #[allow(clippy::similar_names)]
    fn parse(
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<Option<Self>, SFError> {
        let other = data.cget(0, "gbattle other")?;
        let other = match other.try_into() {
            Ok(x) if x > 1 => Some(x),
            _ => None,
        };
        let date = data.cget(1, "gbattle time")?;
        let date = server_time.convert_to_local(date, "next guild fight");
        Ok(match (other, date) {
            (Some(other), Some(date)) => Some(Self { other, date }),
            _ => None,
        })
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The portal a guild has
pub struct GuildPortal {
    /// The damage bonus in percent the guild portal gives to its members
    pub damage_bonus: u8,
    /// The amount of times the portal enemy has already been defeated. You can
    /// easily convert this int oct & stage if you want
    pub defeated_count: u8,
    /// The percentage of life the portal enemy still has
    pub life_percentage: u8,
}
#[derive(Debug, Copy, Clone, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Which battles a member will participate in
pub enum BattlesJoined {
    /// The player has only joined the defense of the guild
    Defense = 1,
    /// The player has only joined the offensive attack against another guild
    Attack = 10,
    /// The player has only joined both the offense and defensive battles of
    /// the guild
    Both = 11,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A member of a guild
pub struct GuildMemberData {
    /// The name of the member
    pub name: String,
    /// Which battles this member will participate in
    pub battles_joined: Option<BattlesJoined>,
    /// The level of this member
    pub level: u16,
    /// The last time this player was online (last time they send an update
    /// command)
    pub last_online: Option<DateTime<Local>>,
    /// The level, that this member has upgraded their treasure to
    pub treasure_skill: u16,
    /// The level, that this member has upgraded their instructor to
    pub instructor_skill: u16,
    /// The level of this members guild pet
    pub guild_pet_lvl: u16,

    /// The rank this member has in the guild
    pub guild_rank: GuildRank,
    /// The last time this member has fought the portal. This is basically a
    /// dynamic check if they have fought it today, because today changes
    pub portal_fought: Option<DateTime<Local>>,
    /// The potions this player has active. This will always be potion, no
    /// other item type
    // TODO: make this explicit
    pub potions: [Option<ItemType>; 3],
    /// The level of this members hall of knights
    pub knights: u8,
}

#[derive(Debug, Clone, Copy, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The rank a member can have in a guild
pub enum GuildRank {
    Leader = 1,
    Officer = 2,
    #[default]
    Member = 3,
    Invited = 4,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Something the player can upgrade in the guild
#[allow(missing_docs)]
pub enum GuildSkill {
    Treasure = 0,
    Instructor,
    Pet,
}
