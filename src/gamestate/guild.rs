use chrono::{DateTime, Local, NaiveTime};
use enum_map::{Enum, EnumMap};
use log::warn;
use num_derive::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use super::{
    AttributeType, NormalCost, Potion, SFError, ServerTime,
    items::{ItemType, PotionSize, PotionType},
};
use crate::{misc::from_sf_string, response::Vals};

/// Information about the characters current guild
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub joined: Option<DateTime<Local>>,

    /// The skill you yourself contribute to the guild
    pub own_treasure_skill: u16,
    /// The total amount of treasure skill the guild has
    pub total_treasure_skill: u16,
    /// The skill you yourself contribute to the guild
    pub own_instructor_skill: u16,
    /// The total amount of instructor skill the guild has
    pub total_instructor_skill: u16,

    /// The price to pay to upgrade the given skill
    pub upgrade_price: EnumMap<GuildSkill, NormalCost>,

    /// How many raids this guild has completed already
    pub finished_raids: u16,

    /// If the guild is defending against another guild, this will contain
    /// information about the upcoming battle
    pub defending: Option<PlanedBattle>,
    /// If the guild is attacking another guild, this will contain
    /// information about the upcoming battle
    pub attacking: Option<PlanedBattle>,
    /// The next time the guild can attack another guild.
    pub next_attack_possible: Option<DateTime<Local>>,

    /// The id of the pet, that is currently selected as the guild pet
    pub pet_id: u32,
    /// The level of your guild pet
    pub own_pet_lvl: u16,
    /// The level of your guild pet
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
    /// The chat messages, that get sent in the guild chat
    pub chat: Vec<ChatMessage>,
    /// The whisper messages, that a player can receive
    pub whispers: Vec<ChatMessage>,
    /// A list of guilds which can be fought, must first be fetched by sending
    /// `Command::GuildGetFightableTargets`
    pub fightable_guilds: Vec<FightableGuild>,
}

/// The hydra, that the guild pet can fight
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// The customizable emblem each guild has
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Emblem {
    raw: String,
}

impl Emblem {
    /// Returns the guild emblem in its server encoded form
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

/// A message, that the player has received, or has sent to others via the chat
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChatMessage {
    /// The user this message originated from. Note that this might not be in
    /// the guild member list in some cases
    pub user: String,
    /// The time at which this message has been sent. I have not checked the
    /// timezone here. Might be UTC/Your TZ/Server TZ
    pub time: NaiveTime,
    /// The actual message, that got sent
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
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.id = data.u32_or(0, 0);
        let member_count = data.u8_or(3, 0);
        self.member_count = member_count;
        self.members
            .resize_with(usize::from(member_count), Default::default);

        for (offset, member) in self.members.iter_mut().enumerate() {
            member.battles_joined = data.enum_map(445 + offset, |x| x % 100);
            member.level = data.u16_or(64 + offset, 0);
            member.last_online =
                data.server_time_opt(114 + offset, server_time);
            member.treasure_skill = data.u16_or(214 + offset, 0);
            member.instructor_skill = data.u16_or(264 + offset, 0);
            member.guild_rank = match data.i64_or(314 + offset, 0) {
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
                data.server_time_opt(164 + offset, server_time);
            member.guild_pet_lvl = data.u16_or(390 + offset, 0);
        }

        self.honor = data.u32_or(13, 0);
        self.finished_raids = data.u16_or(8, 0);
        self.attacking =
            PlanedBattle::parse(&data.window(364..366), server_time);
        self.defending =
            PlanedBattle::parse(&data.window(366..368), server_time);
        self.next_attack_possible = data.server_time_opt(365, server_time);
        self.pet_id = data.u32_or(377, 0);
        self.pet_max_lvl = data.u16_or(378, 0);

        self.hydra.last_battle = data.server_time_opt(382, server_time);
        self.hydra.last_full = data.server_time_opt(381, server_time);
        self.hydra.current_life = data.u64_or(383, u64::MAX);
        self.hydra.max_life = data.u64_or(384, u64::MAX);
        for (idx, attr) in self.hydra.attributes.values_mut().enumerate() {
            *attr = data.u32_or(385 + idx, 0);
        }

        let treasure = data.u32_or(6, 0);
        self.total_treasure_skill =
            u16::try_from(treasure & 0xFFFF).unwrap_or(0);
        self.portal.life_percentage =
            u8::try_from(treasure >> 16).unwrap_or(100);

        let instructor = data.u32_or(7, 0);
        self.total_instructor_skill =
            u16::try_from(instructor & 0xFFFF).unwrap_or(0);
        self.portal.defeated_count =
            u8::try_from(instructor >> 16).unwrap_or(0);

        let members = usize::from(member_count);
        data.ignore(1..3, "the guild emblem strip & an unmodelled timestamp");
        data.ignore(4..6, "always 0 in every captured response");
        data.ignore(9..13, "9 is always 1, the rest is always 0");
        data.ignore(14..64, "member id column not modelled");
        data.ignore(64 + members..114, "unused tail of the level column");
        data.ignore(114 + members..164, "unused tail of the online column");
        data.ignore(164 + members..214, "unused tail of the portal column");
        data.ignore(214 + members..264, "unused tail of the treasure column");
        data.ignore(264 + members..314, "unused tail of the instructor column");
        data.ignore(314 + members..364, "unused tail of the rank column");
        data.ignore(368..377, "370, 371 and 374 are set, the rest is 0");
        data.ignore(379..381, "always 0 in every captured response");
        data.ignore(390 + members..445, "unused tail of the pet level column");
        data.ignore(
            445 + members..518,
            "unused tail of the battles column & the trailer",
        );
        data.ignore(518..data.len(), "extra guild save values");
    }

    pub(crate) fn update_member_names(&mut self, data: &Vals<'_>) {
        self.members.resize_with(data.len(), Default::default);
        for (member, name) in self.members.iter_mut().zip(data.iter()) {
            member.name = name.to_string();
        }
    }

    pub(crate) fn update_group_knights(&mut self, data: &Vals<'_>) {
        self.members.resize_with(data.len(), Default::default);
        for (idx, member) in self.members.iter_mut().enumerate() {
            member.knights = data.u8_or(idx, 0);
        }
    }

    pub(crate) fn update_member_potions(&mut self, data: &Vals<'_>) {
        self.members.resize_with(data.len() / 6, Default::default);

        let quick_potion = |int: i64| {
            Some(ItemType::Potion(Potion {
                typ: PotionType::parse(int)?,
                size: PotionSize::parse(int)?,
                expires: None,
            }))
        };

        for (member, chunk) in self.members.iter_mut().zip(data.chunks(6)) {
            for (slot, potion) in member.potions.iter_mut().enumerate() {
                *potion = quick_potion(chunk.i64_or(slot * 2, 0));
                chunk.ignore(slot * 2 + 1, "unused member potion value");
            }
        }
        let full = data.len() / 6 * 6;
        data.ignore(full..data.len(), "partial member potion data");
    }

    pub(crate) fn update_description_embed(&mut self, data: &str) {
        let Some((emblem, description)) = data.split_once('§') else {
            self.description = from_sf_string(data);
            return;
        };

        self.description = from_sf_string(description);
        self.emblem.update(emblem);
    }

    pub(crate) fn update_group_prices(&mut self, data: &Vals<'_>) {
        for (idx, skill) in GuildSkill::iter().enumerate() {
            let skill = &mut self.upgrade_price[skill];
            skill.silver = data.u64_or(idx * 2, u64::MAX);
            skill.mushrooms = data.u16_or(1 + idx * 2, u16::MAX);
        }
    }

    pub(crate) fn update_character_group(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) {
        self.own_treasure_skill = data.u16_or(0, 0);
        self.own_instructor_skill = data.u16_or(1, 0);
        self.hydra.next_battle = data.server_time_opt(2, server_time);
        self.hydra.remaining_fights = data.u16_or(3, 0);
        self.own_pet_lvl = data.u16_or(4, 0);
        self.joined = data.server_time_opt(5, server_time);
        data.ignore(6..data.len(), "unknown charactergroup tail");
    }

    pub(crate) fn update_fightable_targets(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        const SIZE: usize = 9;

        self.fightable_guilds.clear();
        if !data.len().is_multiple_of(SIZE) {
            warn!("Invalid fightable targets len");
            data.ignore_all("invalid fightable target length");
            return Err(SFError::ParsingError(
                "Fightable targets invalid length",
                data.to_string(),
            ));
        }

        self.fightable_guilds.reserve(data.len() / SIZE);
        self.fightable_guilds.extend(data.chunks(SIZE).map(|chunk| {
            FightableGuild {
                id: chunk.u32_or(0, 0),
                name: from_sf_string(chunk.str_or(1, "")),
                emblem: Emblem {
                    raw: chunk.str_or(2, "").to_string(),
                },
                number_of_members: chunk.u8_or(3, 0),
                members_min_level: chunk.u32_or(4, 0),
                members_max_level: chunk.u32_or(5, 0),
                members_average_level: chunk.u32_or(6, 0),
                rank: chunk.u32_or(7, 0),
                honor: chunk.u32_or(8, 0),
            }
        }));

        Ok(())
    }
}

/// A guild battle, that is scheduled to take place at a certain place and time
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
    #[must_use]
    pub fn is_raid(&self) -> bool {
        self.other == 1_000_000
    }

    #[allow(clippy::similar_names)]
    fn parse(data: &Vals<'_>, server_time: ServerTime) -> Option<Self> {
        let other = u32::try_from(data.i64_or(0, 0)).ok().filter(|&x| x > 1);
        let date = data.server_time_opt(1, server_time);
        match (other, date) {
            (Some(other), Some(date)) => Some(Self { other, date }),
            _ => None,
        }
    }
}

/// The portal a guild has
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GuildPortal {
    /// The damage bonus in percent the guild portal gives to its members
    pub damage_bonus: u8,
    /// The amount of times the portal enemy has already been defeated. You can
    /// easily convert this int oct & stage if you want
    pub defeated_count: u8,
    /// The percentage of life the portal enemy still has
    pub life_percentage: u8,
}

/// Which battles a member will participate in
#[derive(Debug, Copy, Clone, FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BattlesJoined {
    /// The player has only joined the defense of the guild
    Defense = 1,
    /// The player has only joined the offensive attack against another guild
    Attack = 10,
    /// The player has only joined both the offense and defensive battles of
    /// the guild
    Both = 11,
}

/// A member of a guild
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GuildMemberData {
    /// The name of the member
    pub name: String,
    /// Which battles this member will participate in
    pub battles_joined: Option<BattlesJoined>,
    /// The level of this member
    pub level: u16,
    /// The last time this player was online (last time they sent an update
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

/// The rank a member can have in a guild
#[derive(Debug, Clone, Copy, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum GuildRank {
    Leader = 1,
    Officer = 2,
    #[default]
    Member = 3,
    Invited = 4,
}

/// Something the player can upgrade in the guild
#[derive(Debug, Clone, Copy, PartialEq, Enum, Eq, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum GuildSkill {
    Treasure = 0,
    Instructor,
    Pet,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn updates_group_save_and_tolerates_strings() {
        let mut values = vec!["0".to_string(); 518];
        values[0] = "12345".to_string();
        values[1] = "future-emblem".to_string();
        values[3] = "2".to_string();
        values[6] = 0x0063_01F4_u64.to_string();
        values[7] = 0x0019_01F4_u64.to_string();
        values[8] = "53".to_string();
        values[13] = "1288".to_string();
        values[14] = "500001".to_string();
        values[15] = "500002".to_string();
        values[64] = "248".to_string();
        values[65] = "365".to_string();
        values[114] = "1788480001".to_string();
        values[115] = "1788480002".to_string();
        values[164] = "1788480003".to_string();
        values[165] = "1788480004".to_string();
        values[214] = "28".to_string();
        values[215] = "31".to_string();
        values[264] = "31".to_string();
        values[265] = "55".to_string();
        values[314] = "1".to_string();
        values[315] = "2".to_string();
        values[377] = "36".to_string();
        values[378] = "296".to_string();
        values[381] = "1788480005".to_string();
        values[382] = "1788480006".to_string();
        values[383] = "199466330".to_string();
        values[384] = "200000000".to_string();
        values[385] = "30461".to_string();
        values[390] = "190".to_string();
        values[391] = "180".to_string();
        values[445] = "100".to_string();
        values[446] = "0".to_string();

        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut guild = Guild::default();
        guild.update_group_save(&data, ServerTime::default());
        data.assert_all_read();

        assert_eq!(guild.id, 12345);
        assert_eq!(guild.honor, 1288);
        assert_eq!(guild.total_treasure_skill, 500);
        assert_eq!(guild.total_instructor_skill, 500);
        assert_eq!(guild.portal.life_percentage, 99);
        assert_eq!(guild.portal.defeated_count, 25);
        assert_eq!(guild.members.len(), 2);
        assert_eq!(guild.members[0].level, 248);
        assert!(matches!(guild.members[0].guild_rank, GuildRank::Leader));
        assert!(guild.members[0].battles_joined.is_none());
        assert_eq!(guild.hydra.current_life, 199_466_330);
        assert_eq!(guild.hydra.attributes[AttributeType::Strength], 30_461);
    }

    #[test]
    fn updates_group_prices() {
        let data = Vals::from("1000000/10/1000000/10/1250000/2");
        let mut guild = Guild::default();
        guild.update_group_prices(&data);
        data.assert_all_read();
        assert_eq!(guild.upgrade_price[GuildSkill::Treasure].silver, 1_000_000);
        assert_eq!(guild.upgrade_price[GuildSkill::Pet].silver, 1_250_000);
    }

    #[test]
    fn updates_fightable_targets() {
        let data = Vals::from(
            "5060/Alpha/emblemA/20/10/30/25/5/1000/5061/Beta/emblemB/15/8/22/\
             19/7/900",
        );
        let mut guild = Guild::default();
        guild.update_fightable_targets(&data).unwrap();
        data.assert_all_read();
        assert_eq!(guild.fightable_guilds.len(), 2);
        assert_eq!(guild.fightable_guilds[0].name, "Alpha");
        assert_eq!(guild.fightable_guilds[1].honor, 900);
    }

    #[test]
    fn invalid_fightable_target_lengths_are_consumed() {
        let data = Vals::from("5060/Alpha/emblemA");
        let mut guild = Guild::default();
        assert!(guild.update_fightable_targets(&data).is_err());
        data.assert_all_read();
    }

    #[test]
    fn parses_planned_battles() {
        let data = Vals::from("1000000/1791924748");
        let battle = PlanedBattle::parse(&data, ServerTime::default()).unwrap();
        assert!(battle.is_raid());
        data.assert_all_read();

        let data = Vals::from("0/0");
        assert!(PlanedBattle::parse(&data, ServerTime::default()).is_none());
        data.assert_all_read();
    }

    #[test]
    fn updates_member_lists() {
        let names = Vals::by("MemberOne,MemberTwo,MemberThree", ',');
        let mut guild = Guild::default();
        guild.update_member_names(&names);
        names.assert_all_read();
        assert_eq!(guild.members.len(), 3);

        let knights = Vals::by("13,18,18,0", ',');
        guild.update_group_knights(&knights);
        knights.assert_all_read();
        assert_eq!(guild.members[0].knights, 13);

        let potions = Vals::by("9,15,11,25,12,25,11,25,4,10,12,25", ',');
        guild.update_member_potions(&potions);
        potions.assert_all_read();
        assert_eq!(guild.members.len(), 2);
        assert!(guild.members[0].potions[0].is_some());
    }

    #[test]
    fn updates_character_group() {
        let data = Vals::from("15/20/1600000000/3/10/1500000000/0");
        let mut guild = Guild::default();
        guild.update_character_group(&data, ServerTime::default());
        data.assert_all_read();
        assert_eq!(guild.own_treasure_skill, 15);
        assert_eq!(guild.own_instructor_skill, 20);
        assert_eq!(guild.hydra.remaining_fights, 3);
        assert!(guild.joined.is_some());
    }
}
