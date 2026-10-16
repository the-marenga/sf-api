use chrono::{DateTime, Local};
use enum_map::EnumMap;
use num_derive::FromPrimitive;

use crate::{
    PlayerId,
    command::AttributeType,
    error::{Result, SFError},
    gamestate::{
        ServerTime,
        character::{Class, Mount, Portrait, Race},
        items::{Equipment, Potion},
    },
    response::Vals,
};

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
                SFError::ParsingError(format!(
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

impl RelationEntry {
    /// Parses a single relation entry `[id, name, guild, level, relation]`.
    pub(crate) fn parse(data: &Vals<'_>) -> Result<Self> {
        Ok(RelationEntry {
            id: data.u32(0)?,
            name: data.str(1)?.to_string(),
            guild: data.str(2)?.to_string(),
            level: data.u16(3)?,
            relation: data.parse_enum(4)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        data.assert_all_read();
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
        assert_eq!(ft.lootable_wood, 29_882_513);
        assert_eq!(ft.lootable_stone, 1_088_585);
        data.assert_all_read();
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
        assert_eq!(player.player_id, 302_106);
        assert_eq!(player.level, 397);
        assert_eq!(player.experience, 800_402_824);
        assert_eq!(player.next_level_xp, 1_500_000_000);
        assert_eq!(player.honor, 157_866);
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
        data.assert_all_read();
    }

    #[test]
    fn test_other_player_unit_levels_and_fortress_rank() {
        let mut player = OtherPlayer::default();
        let unit_raw = "15/10/12/8";
        let unit_data = Vals::from(unit_raw);
        player.update_unit_levels(&unit_data);
        assert_eq!(player.wall_combat_lvl, 15);
        unit_data.assert_all_read();

        player.update_fortress_rank(42);
        assert_eq!(player.fortress.unwrap().rank, 42);

        player.update_fortress_rank(-1);
        assert_eq!(player.fortress.unwrap().rank, 42);
    }
}
