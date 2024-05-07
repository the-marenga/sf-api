use chrono::{DateTime, Local, NaiveTime};
use enum_map::EnumMap;
use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::{
    items::{ItemType, PotionSize, PotionType},
    update_enum_map, ArrSkip, AttributeType, CGet, NormalCost, SFError,
    ServerTime,
};
use crate::misc::{from_sf_string, soft_into, warning_parse};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Informaion about the characters current guild
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
    /// The price to pay to upgrade tyour instructor by one rank
    pub own_instructor_upgrade: NormalCost,

    /// Whether or not the guild is currently raiding
    pub is_raiding: bool,
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

    /// The last time the hydra has been fought
    pub hydra_last_battle: Option<DateTime<Local>>,
    /// The last time the hydra has been seen with full health
    pub hydra_last_full: Option<DateTime<Local>>,
    /// This seems to be last_battle + 30 min. I can only do 1 battle/day, but
    /// I think this should be the next possible fight
    pub hydra_next_battle: Option<DateTime<Local>>,

    pub hydra_current_life: u64,
    pub hydra_max_life: u64,
    pub hydra_attributes: EnumMap<AttributeType, u32>,

    pub guild_portal: GuildPortal,

    member_count: u8,
    pub members: Vec<GuildMemberData>,
    pub chat: Vec<ChatMessage>,
    pub whispers: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The customizeable emblem each guild has
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
pub struct ChatMessage {
    pub user: String,
    pub time: NaiveTime,
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
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        let member_count = soft_into(data[3], "guild member count", 0);
        self.member_count = member_count;
        self.members
            .resize_with(member_count as usize, Default::default);

        for (offset, member) in self.members.iter_mut().enumerate() {
            member.battles_joined =
                FromPrimitive::from_i64(data[64 + offset] / 1000);
            member.level =
                soft_into(data[64 + offset] % 1000, "guild member level", 0);
            member.last_online = server_time
                .convert_to_local(data[114 + offset], "guild last online");
            member.treasure_skill =
                soft_into(data[214 + offset], "guild member treasure skill", 0);
            member.master_skill =
                soft_into(data[264 + offset], "guild member master skill", 0);
            member.guild_rank = match data[314 + offset] {
                1 => GuildRank::Leader,
                2 => GuildRank::Officer,
                3 => GuildRank::Member,
                4 => GuildRank::Invited,
                x => {
                    warn!("Unknown guild rank: {x}");
                    GuildRank::Invited
                }
            };
            member.portal_fought = server_time
                .convert_to_local(data[164 + offset], "portal fought");
            member.guild_pet_lvl =
                soft_into(data[390 + offset], "guild member pet skill", 0);
        }

        self.honor = soft_into(data[13], "guild honor", 0);
        self.id = soft_into(data[0], "guild id", 0);

        self.is_raiding = data[9] != 0;
        self.finished_raids = soft_into(data[8], "finished raids", 0);

        self.attacking = PlanedBattle::parse(
            data.skip(364, "attacking guild")?,
            server_time,
        )?;

        self.defending = PlanedBattle::parse(
            data.skip(366, "attacking guild")?,
            server_time,
        )?;

        self.pet_id = soft_into(data[377], "gpet id", 0);
        self.pet_max_lvl = soft_into(data[378], "gpet max lvl", 0);

        self.hydra_last_battle =
            server_time.convert_to_local(data[382], "hydra pet lb");
        self.hydra_last_full =
            server_time.convert_to_local(data[381], "hydra last defeat");

        self.hydra_current_life =
            soft_into(data[383], "ghydra clife", u64::MAX);
        self.hydra_max_life =
            soft_into(data[384], "ghydra max clife", u64::MAX);

        update_enum_map(&mut self.hydra_attributes, &data[385..]);

        self.guild_portal.life_percentage =
            soft_into(data[6] >> 16, "guild portal life p", 100);
        self.guild_portal.defeated_count =
            soft_into(data[7] >> 16, "guild portal progress", 0);
        Ok(())
    }

    pub(crate) fn update_member_names(&mut self, val: &str) {
        let names: Vec<_> = val.split(',').map(|d| d.to_string()).collect();
        self.members.resize_with(names.len(), Default::default);
        for (member, name) in self.members.iter_mut().zip(names) {
            member.name = name;
        }
    }

    pub(crate) fn update_group_knights(&mut self, val: &str) {
        let data: Vec<i64> = val
            .trim_end_matches(',')
            .split(',')
            .flat_map(|a| a.parse())
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
            Some(ItemType::Potion {
                typ: PotionType::parse(int)?,
                size: PotionSize::parse(int)?,
                expires: None,
            })
        };

        for member in self.members.iter_mut() {
            for i in 0..3 {
                let v = if let Some(x) = data.next() {
                    x
                } else {
                    warn!("Invalid member potion size");
                    0
                };
                member.potions[i] = quick_potion(v);
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

    pub(crate) fn update_group_prices(&mut self, data: &[i64]) {
        self.own_treasure_upgrade.silver =
            soft_into(data[0], "treasure upgr. silver", 0);
        self.own_treasure_upgrade.mushroom =
            soft_into(data[1], "treasure upgr. mush", 0);
        self.own_instructor_upgrade.silver =
            soft_into(data[2], "instr upgr. silver", 0);
        self.own_instructor_upgrade.mushroom =
            soft_into(data[3], "instr upgr. mush", 0);
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlanedBattle {
    /// The guild this battle will be against
    pub other: u32,
    /// The date & time this battle will be at
    pub date: DateTime<Local>,
}

impl PlanedBattle {
    /// Checks if the battle is a raid
    pub fn is_raid(&self) -> bool {
        self.other == 1000000
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
pub struct GuildPortal {
    pub damage_bonus: u8,
    pub defeated_count: u8,
    pub life_percentage: u8,
}
#[derive(Debug, Copy, Clone, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BattlesJoined {
    Attack = 1,
    Defense,
    Both,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GuildMemberData {
    pub name: String,
    pub battles_joined: Option<BattlesJoined>,
    pub level: u16,
    pub last_online: Option<DateTime<Local>>,
    pub treasure_skill: u16,
    pub master_skill: u16,
    pub guild_rank: GuildRank,
    pub portal_fought: Option<DateTime<Local>>,
    pub guild_pet_lvl: u16,
    pub potions: [Option<ItemType>; 3],
    pub knights: u8,
}

#[derive(Debug, Clone, Copy, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GuildRank {
    Leader = 1,
    Officer = 2,
    #[default]
    Member = 3,
    Invited = 4,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GuildSkill {
    Treasure = 0,
    Instructor,
    Pet,
}
