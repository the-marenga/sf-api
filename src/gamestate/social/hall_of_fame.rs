use log::warn;

use crate::{
    command::Flag,
    error::{Result, SFError},
    gamestate::character::Class,
    response::Vals,
};

/// The Hall of Fame rankings for players, guilds, pets, fortresses, etc.
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
                SFError::ParsingError(format!(
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

#[cfg(test)]
mod tests {
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
        assert_eq!(player.honor, 157_866);
        assert_eq!(player.class, Class::Mage);
        assert_eq!(player.flag, Flag::parse("de"));
        data.assert_all_read();

        let raw_empty = "2,SoloPlayer,,100,500,1,";
        let data_empty = Vals::by(raw_empty, ',');
        let player2 = HallOfFamePlayer::parse(&data_empty).unwrap();
        assert_eq!(player2.guild, None);
        assert_eq!(player2.flag, None);
        assert_eq!(player2.class, Class::Warrior);
        data_empty.assert_all_read();
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
        assert_eq!(guild.honor, 123_456);
        assert!(guild.is_attacked);
        data.assert_all_read();
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
        data.assert_all_read();
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
        data.assert_all_read();
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
        data.assert_all_read();
    }

    #[test]
    fn test_hall_of_fame_hellevator_parse() {
        let raw = "1,HellMaster,50000";
        let data = Vals::by(raw, ',');
        let hh = HallOfFameHellevator::parse(&data).unwrap();
        assert_eq!(hh.rank, 1);
        assert_eq!(hh.name, "HellMaster");
        assert_eq!(hh.tokens, 50000);
        data.assert_all_read();
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
        players_data.assert_all_read();

        let guilds_raw = "1,Die Hejels,50,157866,10,de";
        let guilds_data = Vals::by(guilds_raw, ';');
        hof.update_guilds(&guilds_data);
        assert_eq!(hof.guilds.len(), 1);
        assert_eq!(hof.guilds[0].name, "Die Hejels");
        guilds_data.assert_all_read();

        let pets_raw = "1,PetMaster,PetGuild,100,157866,0";
        let pets_data = Vals::by(pets_raw, ';');
        hof.update_pets(&pets_data);
        assert_eq!(hof.pets.len(), 1);
        assert_eq!(hof.pets[0].name, "PetMaster");
        pets_data.assert_all_read();

        let ft_raw = "1,FortressMaster,Die Hejels,500,157866,100,de";
        let ft_data = Vals::by(ft_raw, ';');
        hof.update_fortresses(&ft_data);
        assert_eq!(hof.fortresses.len(), 1);
        assert_eq!(hof.fortresses[0].name, "FortressMaster");
        ft_data.assert_all_read();

        let uw_raw = "1,UnderworldMaster,Die Hejels,500,157866,15,de";
        let uw_data = Vals::by(uw_raw, ';');
        hof.update_underworlds(&uw_data);
        assert_eq!(hof.underworlds.len(), 1);
        assert_eq!(hof.underworlds[0].name, "UnderworldMaster");
        uw_data.assert_all_read();

        let hlv_raw = "1,HellevatorHero,1000";
        let hlv_data = Vals::by(hlv_raw, ';');
        hof.update_hellevator(&hlv_data);
        assert_eq!(hof.hellevator.len(), 1);
        assert_eq!(hof.hellevator[0].name, "HellevatorHero");
        assert_eq!(hof.hellevator[0].tokens, 1000);
        hlv_data.assert_all_read();
    }
}
