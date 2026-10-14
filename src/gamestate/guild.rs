#![allow(clippy::module_name_repetitions)]
use chrono::{DateTime, Local, NaiveTime};
use enum_map::{Enum, EnumMap};
use log::warn;
use num_derive::FromPrimitive;
use strum::{EnumIter, IntoEnumIterator};

use super::{
    AttributeType, NormalCost, Potion, Result, SFError, ServerTime,
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
    pub defending: Option<PlannedBattle>,
    /// If the guild is attacking another guild, this will contain
    /// information about the upcoming battle
    pub attacking: Option<PlannedBattle>,
    /// The next time the guild can attack another guild.
    pub next_attack_possible: Option<DateTime<Local>>,

    /// The id of the pet, that is currently selected as the guild pet
    pub pet_id: u32,
    /// The level you yourself contribute to the guild pet
    pub own_pet_lvl: u16,
    /// The maximum level that the pet can be at
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
    pub fn server_encode(&self) -> &str {
        // TODO: Actually parse this
        &self.raw
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
    /// Updates the guild from an `owngroupsave` (`groupSave`) response. The
    /// member fields are 50-slot columns, only the first `member_count` used.
    pub(crate) fn update_group_save(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.id = data.u32_or(0, 0);
        let member_count = data.u8_or(3, 0);
        self.member_count = member_count;
        self.members
            .resize_with(usize::from(member_count), Default::default);

        for (offset, member) in self.members.iter_mut().enumerate() {
            // The first member column (base 14) is not modelled
            data.ignore(14 + offset, "unmodelled member column");
            member.battles_joined = data.enum_map(445 + offset, |x| x % 100);
            member.level = data.u16_or(64 + offset, 0);
            member.last_online = data.server_time(114 + offset, server_time)?;
            member.treasure_skill = data.u16_or(214 + offset, 0);
            member.instructor_skill = data.u16_or(264 + offset, 0);
            member.guild_rank = match data.i64(314 + offset)? {
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
                data.server_time(164 + offset, server_time)?;
            member.guild_pet_lvl = data.u16_or(390 + offset, 0);
        }

        self.honor = data.u32_or(13, 0);
        self.finished_raids = data.u16_or(8, 0);

        self.attacking =
            PlannedBattle::parse(&data.window(364..366), server_time)?;
        self.defending =
            PlannedBattle::parse(&data.window(366..368), server_time)?;
        self.next_attack_possible = data.server_time(365, server_time)?;

        self.pet_id = data.u32_or(377, 0);
        self.pet_max_lvl = data.u16_or(378, 0);

        self.hydra.last_battle = data.server_time(382, server_time)?;
        self.hydra.last_full = data.server_time(381, server_time)?;
        self.hydra.current_life = data.u64_or(383, u64::MAX);
        self.hydra.max_life = data.u64_or(384, u64::MAX);

        for (idx, attr) in self.hydra.attributes.values_mut().enumerate() {
            *attr = data.u32_or(385 + idx, 0);
        }

        // Index 6 packs the portal life in the high and the total treasure
        // skill in the low 16 bits, index 7 does the same for the defeated
        // count and the total instructor skill
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

        // Everything from `member_count` to the next column is an unused
        // (always 0) tail; the rest is a field that is not modelled yet
        let members = usize::from(member_count);
        data.ignore(1..3, "the guild emblem strip & an unmodelled timestamp");
        data.ignore(4..6, "always 0 in every captured response");
        data.ignore(9..13, "9 is always 1, the rest is always 0");
        data.ignore(14 + members..64, "unused tail of the first member column");
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

        Ok(())
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

        // One 6-value chunk per member: three `[potion, unused]` pairs
        for (member, chunk) in self.members.iter_mut().zip(data.chunks(6)) {
            for (slot, potion) in member.potions.iter_mut().enumerate() {
                *potion = quick_potion(chunk.i64_or(slot * 2, 0));
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

    /// Updates the skill upgrade prices from a 6-value `groupskillprice`
    /// response: `[silver, mushrooms]` per [`GuildSkill`].
    pub(crate) fn update_group_prices(&mut self, data: &Vals<'_>) {
        for (idx, skill) in GuildSkill::iter().enumerate() {
            let skill = &mut self.upgrade_price[skill];
            skill.silver = data.u64_or(idx * 2, u64::MAX);
            skill.mushrooms = data.u16_or(1 + idx * 2, u16::MAX);
        }
    }

    /// Updates the player's personal guild attributes from a `charactergroup`
    /// response: `own_treasure_skill, own_instructor_skill, hydra_next_battle,
    /// hydra_remaining_fights, own_pet_lvl, joined, ...`.
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
        if data.len() > 6 {
            data.ignore(6, "unknown charactergroup tail");
        }
    }

    /// Parses a `fightablegroups` response: `SIZE` values per guild, with a
    /// runtime-determined number of guilds.
    pub(crate) fn update_fightable_targets(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<()> {
        const SIZE: usize = 9;

        // Check if the data is valid
        if !data.len().is_multiple_of(SIZE) {
            warn!("Invalid fightable targets len");
            return Err(SFError::ParsingError(
                "Fightable targets invalid length",
                data.to_string(),
            ));
        }

        // Delete any old data
        self.fightable_guilds.clear();

        // Reserve space for the new data, then add one guild per chunk
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
pub struct PlannedBattle {
    /// The guild this battle will be against
    pub other: u32,
    /// The date & time this battle will be at
    pub date: DateTime<Local>,
}

impl PlannedBattle {
    /// Checks if the battle is a raid
    #[must_use]
    pub fn is_raid(&self) -> bool {
        self.other == 1_000_000
    }

    #[allow(clippy::similar_names)]
    fn parse(data: &Vals<'_>, server_time: ServerTime) -> Result<Option<Self>> {
        let other = Some(data.u32(0)?).filter(|&x| x > 1);
        let date = data.server_time(1, server_time)?;
        Ok(match (other, date) {
            (Some(other), Some(date)) => Some(Self { other, date }),
            _ => None,
        })
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GuildRank {
    /// The leader of the guild
    Leader = 1,
    /// An officer of the guild
    Officer = 2,
    /// A regular member of the guild
    #[default]
    Member = 3,
    /// Invited to the guild, but not a member yet
    Invited = 4,
}

/// Something the player can upgrade in the guild
// NOTE: The explicit discriminants are wire values, not just declaration
// order: `Command::GuildIncreaseSkill` serializes the skill with
// `*skill as usize`, so they must not be reordered or renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Enum, Eq, EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GuildSkill {
    /// Upgrades the treasure skill of the guild
    Treasure = 0,
    /// Upgrades the instructor skill of the guild
    Instructor = 1,
    /// Upgrades the guild pet
    Pet = 2,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn test_update_group_save() {
        // An `owngroupsave` (`groupSave`) response with 29 members.
        let raw =
            "12345/04120701140F3206060303/1788480001/29/0/0/6488564/1638900/\
             53/1/500000/0/0/1288/500001/500002/500003/500004/500005/500006/\
             500007/500008/500009/500010/500011/500012/500013/500014/500015/\
             500016/500017/500018/500019/500020/500021/500022/500023/500024/\
             500025/500026/500027/500028/500029/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/248/365/360/337/285/189/187/358/345/342/340/335/335/\
             330/330/329/328/325/322/317/314/311/174/166/150/131/106/40/9/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/1788549577/1788120187/\
             1786553505/1788328260/1788054513/1612885231/1544315512/\
             1786323865/1786237638/1785264635/1785264828/1785264778/\
             1785442831/1701799732/1785265129/1785265196/1608702797/\
             1785264806/1785264440/1785265175/1785265005/1785264415/\
             1405532465/1405532459/1380365216/1406365933/1479561108/\
             1785265073/1786458511/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             1788480060/1785255024/1786553614/1772694957/1787883643/\
             1608644333/1544315337/1785255012/1785678215/1785255026/\
             1785255025/1785255042/1785255034/1608640816/1785255013/\
             1785255019/1608643464/1785255040/1785255054/1785255044/\
             1760214974/1785255038/0/0/0/1406365905/1478108248/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/28/28/28/28/28/0/28/28/29/28/28/\
             28/28/28/28/28/28/28/28/28/31/28/28/28/28/28/28/0/3/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/31/31/31/31/31/0/31/31/31/31/31/31/\
             31/31/31/31/31/31/31/31/55/31/31/31/31/31/31/0/3/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/0/0/0/1/2/2/2/2/2/2/3/3/3/3/3/3/3/3/3/3/3/3/\
             3/3/3/3/3/3/3/3/3/3/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/318/17/0/0/3,201,207,6,210,2,3,4,1,0/0/0/36/296/0/0/1/\
             0/199466330/200000000/30461/9572/9110/24992/18637/190/180/255/\
             180/180/15/180/220/285/240/185/180/180/180/185/180/180/200/180/\
             180/180/180/180/180/180/180/180/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/\
             0/0/0/0/0/0/0/0/0/0/242/0/100/000/000/000/000/000/000/000/000/\
             000/000/000/000/000/000/000/000/000/000/000/000/000/000/000/000/\
             000/000/000/000/000/000/000/000/000/000/000/000/000/000/000/000/\
             000/000/000/000/000/000/000/000/000/0/0/0/1/4/2";

        let data = Vals::from(raw);
        let mut guild = Guild::default();
        guild
            .update_group_save(&data, ServerTime::default())
            .unwrap();

        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );

        assert_eq!(guild.id, 12345);
        assert_eq!(guild.honor, 1288);
        assert_eq!(guild.finished_raids, 53);
        assert_eq!(guild.total_treasure_skill, 500);
        assert_eq!(guild.total_instructor_skill, 500);
        assert_eq!(guild.pet_id, 36);
        assert_eq!(guild.pet_max_lvl, 296);

        assert_eq!(guild.members.len(), 29);
        assert!(guild.attacking.is_none());
        assert!(guild.defending.is_none());
        assert!(guild.next_attack_possible.is_none());

        let first = &guild.members[0];
        assert_eq!(first.level, 248);
        assert_eq!(first.treasure_skill, 28);
        assert_eq!(first.instructor_skill, 31);
        assert_eq!(first.guild_pet_lvl, 190);
        assert_eq!(first.guild_rank, GuildRank::Leader);
        assert!(first.last_online.is_some());
        assert!(first.portal_fought.is_some());
        // `battles_joined` is `100 % 100`, which is no variant
        assert_eq!(first.battles_joined, None);

        assert_eq!(guild.portal.life_percentage, 99);
        assert_eq!(guild.portal.defeated_count, 25);
        assert_eq!(guild.hydra.current_life, 199_466_330);
        assert_eq!(guild.hydra.max_life, 200_000_000);
        assert_eq!(guild.hydra.attributes[AttributeType::Strength], 30_461);
    }

    #[test]
    fn test_update_group_prices() {
        // A `groupskillprice(6)` response.
        let data = Vals::from("1000000/10/1000000/10/1250000/2");
        let mut guild = Guild::default();
        guild.update_group_prices(&data);

        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
        let treasure = guild.upgrade_price[GuildSkill::Treasure];
        assert_eq!((treasure.silver, treasure.mushrooms), (1_000_000, 10));
        let pet = guild.upgrade_price[GuildSkill::Pet];
        assert_eq!((pet.silver, pet.mushrooms), (1_250_000, 2));
    }

    #[test]
    fn test_update_fightable_targets() {
        // A `fightablegroups` response: two guilds of 9 values each.
        let resp = crate::response::Response::parse(
            "fightablegroups:5060/Alpha/emblemA/20/10/30/25/5/1000/5061/Beta/\
             emblemB/15/8/22/19/7/900"
                .to_string(),
            Local::now().naive_local(),
        )
        .unwrap();
        let data = resp.values()["fightablegroups"].list();

        let mut guild = Guild::default();
        guild.update_fightable_targets(&data).unwrap();

        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );

        assert_eq!(guild.fightable_guilds.len(), 2);
        let first = &guild.fightable_guilds[0];
        assert_eq!(first.id, 5060);
        assert_eq!(first.name, "Alpha");
        assert_eq!(first.emblem.server_encode(), "emblemA");
        assert_eq!(first.number_of_members, 20);
        assert_eq!(first.members_min_level, 10);
        assert_eq!(first.members_max_level, 30);
        assert_eq!(first.members_average_level, 25);
        assert_eq!(first.rank, 5);
        assert_eq!(first.honor, 1000);

        let second = &guild.fightable_guilds[1];
        assert_eq!(second.id, 5061);
        assert_eq!(second.name, "Beta");
        assert_eq!(second.honor, 900);
    }

    #[test]
    fn test_update_fightable_targets_invalid_len() {
        // A value count that is not a multiple of 9 is rejected.
        let data = Vals::from("5060/Alpha/emblemA");
        let mut guild = Guild::default();
        assert!(guild.update_fightable_targets(&data).is_err());
        data.ignore_all("rejected before parsing");
    }

    #[test]
    fn test_planned_battle_parse() {
        // The `owngroupsave` attacking pair: a raid against the raid id
        // 1_000_000
        let data = Vals::from("1000000/1791924748");
        let battle = PlannedBattle::parse(&data, ServerTime::default())
            .unwrap()
            .unwrap();
        assert_eq!(battle.other, 1_000_000);
        assert!(battle.is_raid());

        // 0/0 is the sentinel for "no planned battle"
        let data = Vals::from("0/0");
        assert!(
            PlannedBattle::parse(&data, ServerTime::default())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_update_member_names() {
        // A `owngroupmember.r` response, first three names
        let resp = crate::response::Response::parse(
            "owngroupmember.r:MemberOne,MemberTwo,MemberThree".to_string(),
            Local::now().naive_local(),
        )
        .unwrap();
        let data = resp.values()["owngroupmember"].list_by(',');

        let mut guild = Guild::default();
        guild.update_member_names(&data);

        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
        let names: Vec<_> = guild.members.iter().map(|m| &m.name).collect();
        assert_eq!(names, ["MemberOne", "MemberTwo", "MemberThree"]);
    }

    #[test]
    fn test_update_group_knights() {
        // A `owngroupknights.r` response, first four members
        let resp = crate::response::Response::parse(
            "owngroupknights.r:13,18,18,0".to_string(),
            Local::now().naive_local(),
        )
        .unwrap();
        let data = resp.values()["owngroupknights"].list_by(',');

        let mut guild = Guild::default();
        guild.update_group_knights(&data);

        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
        let knights: Vec<_> = guild.members.iter().map(|m| m.knights).collect();
        assert_eq!(knights, [13, 18, 18, 0]);
    }

    #[test]
    fn test_update_member_potions() {
        // A `owngrouppotion.r` response, first two members
        let resp = crate::response::Response::parse(
            "owngrouppotion.r:9,15,11,25,12,25,11,25,4,10,12,25".to_string(),
            Local::now().naive_local(),
        )
        .unwrap();
        let data = resp.values()["owngrouppotion"].list_by(',');

        let mut guild = Guild::default();
        guild.update_member_potions(&data);

        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
        assert_eq!(guild.members.len(), 2);
        assert!(guild.members[0].potions[0].is_some());
        assert!(guild.members[1].potions[0].is_some());
    }

    #[test]
    fn test_update_character_group() {
        let raw = "15/20/1600000000/3/10/1500000000/0";
        let data = Vals::from(raw);
        let mut guild = Guild::default();
        guild.update_character_group(&data, ServerTime::default());
        assert_eq!(guild.own_treasure_skill, 15);
        assert_eq!(guild.own_instructor_skill, 20);
        assert!(guild.hydra.next_battle.is_some());
        assert_eq!(guild.hydra.remaining_fights, 3);
        assert_eq!(guild.own_pet_lvl, 10);
        assert!(guild.joined.is_some());
        assert!(
            data.unaccessed_indices().is_empty(),
            "unread values in {data}"
        );
    }
}
