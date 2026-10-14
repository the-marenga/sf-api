use chrono::{DateTime, Local};

use crate::{
    PlayerId,
    error::Result,
    gamestate::{
        ServerTime,
        guild::{Emblem, GuildRank},
    },
    response::{Vals, from_sf_string},
};

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
    /// The unique player ID of the member
    pub id: PlayerId,
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
            member.id = data.u32_or(14 + i, 0);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_all_read(data: &Vals<'_>) {
        let unread = data.unaccessed_indices();
        assert!(unread.is_empty(), "unread values {unread:?} in {data}");
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
        assert_all_read(&names_data);

        let desc_data = Vals::by("010203§Welcome to our guild!", '§');
        guild.update_description_embed(&desc_data);
        assert_eq!(guild.description, "Welcome to our guild!");
        assert_all_read(&desc_data);
    }
}
