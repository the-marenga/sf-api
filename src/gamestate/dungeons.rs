use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumCount, EnumIter};

use super::{
    AttributeType, Result, ServerTime,
    character::Class,
    items::{EQUIPMENT_PARSE_LEN, Equipment, Item},
};
use crate::response::Vals;

/// The personal demon portal.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Portal {
    /// Number of enemies defeated in the portal.
    pub finished: u16,
    /// Whether the daily portal fight is available. Fight with
    /// [`Command::FightPortal`](crate::command::Command::FightPortal).
    pub can_fight: bool,
    /// Remaining health percentage of the current portal enemy.
    pub enemy_hp_percentage: u8,
    /// Health bonus percentage granted to the player (1% per defeated enemy).
    #[doc(alias = "hp_bonus")]
    pub player_hp_bonus: u16,
}

impl Portal {
    /// Updates the portal from a `portalprogress` response of 3 values:
    /// `fights / enemy_hp_percentage / last_portal_day`.
    pub(crate) fn update(
        &mut self,
        data: &Vals<'_>,
        server_time: ServerTime,
    ) -> Result<()> {
        self.finished = data.u16(0)?;
        self.enemy_hp_percentage = data.u8(1)?;

        let current_day = chrono::Datelike::ordinal(&server_time.current());
        let last_portal_day: u32 = data.u32(2)?;
        self.can_fight = last_portal_day != current_day;

        Ok(())
    }
}

/// Information about light, shadow, and special dungeons. Refresh with
/// [`Command::UpdateDungeons`](crate::command::Command::UpdateDungeons).
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dungeons {
    /// When the next free dungeon fight is available.
    pub next_free_fight: Option<DateTime<Local>>,
    /// Progress for all light dungeons. Check with
    /// [`progress()`](Self::progress).
    pub light: EnumMap<LightDungeon, DungeonProgress>,
    /// Progress for all shadow dungeons. Check with
    /// [`progress()`](Self::progress).
    pub shadow: EnumMap<ShadowDungeon, DungeonProgress>,
    /// The personal demon portal.
    #[doc(alias = "demon_portal")]
    pub portal: Option<Portal>,
    /// Companions unlocked via the Tower ([`LightDungeon::Tower`]).
    pub companions: Option<EnumMap<CompanionClass, Companion>>,
}

impl Dungeons {
    /// Returns the progress for the specified dungeon.
    pub fn progress(&self, dungeon: impl Into<Dungeon>) -> DungeonProgress {
        let dungeon: Dungeon = dungeon.into();
        match dungeon {
            Dungeon::Light(dungeon) => self.light[dungeon],
            Dungeon::Shadow(dungeon) => self.shadow[dungeon],
        }
    }

    /// Returns the monster for the current floor of the dungeon, if open.
    #[cfg(feature = "simulation")]
    pub fn current_enemy(
        &self,
        dungeon: impl Into<Dungeon> + Copy,
    ) -> Option<&'static crate::simulate::Monster> {
        get_dungeon_monster(dungeon, self.progress(dungeon))
    }
}

/// The current state of a dungeon.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DungeonProgress {
    /// The dungeon is locked.
    #[default]
    Locked,
    /// The dungeon is open and can be entered. Fight with
    /// [`Command::FightDungeon`](crate::command::Command::FightDungeon).
    Open {
        /// Number of enemies defeated so far.
        #[doc(alias = "floor")]
        #[doc(alias = "stage")]
        finished: u16,
    },
    /// The dungeon has been fully cleared.
    Finished,
}

/// Category of dungeon (light or shadow).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DungeonType {
    /// A light dungeon.
    Light,
    /// A shadow dungeon.
    Shadow,
}

/// A light or shadow dungeon identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Dungeon {
    /// A light dungeon.
    Light(LightDungeon),
    /// A shadow dungeon.
    Shadow(ShadowDungeon),
}

impl Dungeon {
    /// Returns whether this dungeon is fought with companions (Tower and
    /// non-Twister shadow dungeons).
    #[must_use]
    #[allow(clippy::match_same_arms)]
    pub fn is_with_companions(self) -> bool {
        match self {
            Dungeon::Light(LightDungeon::Tower) => true,
            Dungeon::Shadow(ShadowDungeon::Twister) => false,
            Dungeon::Light(_) => false,
            Dungeon::Shadow(_) => true,
        }
    }

    /// Returns the total number of floors in this dungeon.
    #[must_use]
    pub fn total_floors(self) -> u16 {
        match self {
            Dungeon::Light(d) => d.total_floors(),
            Dungeon::Shadow(d) => d.total_floors(),
        }
    }
}

/// All light dungeons. Iterate using `LightDungeon::iter()`.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    EnumCount,
    EnumIter,
    Enum,
    FromPrimitive,
    Hash,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LightDungeon {
    /// Desecrated Catacombs
    DesecratedCatacombs = 0,
    /// The Mines of Gloria
    MinesOfGloria = 1,
    /// The Ruins of Gnark
    RuinsOfGnark = 2,
    /// The Cutthroat Grotto
    CutthroatGrotto = 3,
    /// The Emerald Scale Altar
    EmeraldScaleAltar = 4,
    /// The Toxic Tree
    ToxicTree = 5,
    /// The Magma Stream
    MagmaStream = 6,
    /// The Frost Blood Temple
    FrostBloodTemple = 7,
    /// The Pyramids of Madness
    PyramidsofMadness = 8,
    /// Black Skull Fortress
    BlackSkullFortress = 9,
    /// Circus of Horror
    CircusOfHorror = 10,
    /// Hell
    Hell = 11,
    /// The 13th Floor
    The13thFloor = 12,
    /// Easteros
    Easteros = 13,
    /// The Tower
    Tower = 14,
    /// Time-honored School of Magic
    TimeHonoredSchoolofMagic = 15,
    /// Hemorridor
    Hemorridor = 16,
    /// Nordic Gods
    NordicGods = 18,
    /// Mount Olympus
    MountOlympus = 19,
    /// Tavern of the Dark Doppelgangers
    TavernoftheDarkDoppelgangers = 20,
    /// Dragon's Hoard
    DragonsHoard = 21,
    /// House of Horrors
    HouseOfHorrors = 22,
    /// Third League of Superheroes
    ThirdLeagueOfSuperheroes = 23,
    /// Dojo of Childhood Heroes
    DojoOfChildhoodHeroes = 24,
    /// Monster Grotto
    MonsterGrotto = 25,
    /// City of Intrigues
    CityOfIntrigues = 26,
    /// School of Magic Express
    SchoolOfMagicExpress = 27,
    /// Ash Mountain
    AshMountain = 28,
    /// Playa Games HQ
    PlayaGamesHQ = 29,
    /// Training Camp
    TrainingCamp = 30,
    /// Sandstorm
    Sandstorm = 31,
    /// Arcade of the Old Pixel Icons
    ArcadeOfTheOldPixelIcons = 32,
    /// The Server Room
    TheServerRoom = 33,
    /// Workshop of the Hunters of the Undead
    WorkshopOfTheHunters = 34,
    /// Retro TV Legends
    RetroTVLegends = 35,
    /// The Meeting Room
    MeetingRoom = 36,
}

impl LightDungeon {
    /// Returns the total number of floors in this dungeon.
    #[must_use]
    pub fn total_floors(self) -> u16 {
        match self {
            LightDungeon::Tower => 100,
            LightDungeon::Sandstorm => 1000,
            _ => 10,
        }
    }
}

impl From<LightDungeon> for Dungeon {
    fn from(val: LightDungeon) -> Self {
        Dungeon::Light(val)
    }
}

/// All shadow dungeons. Iterate using `ShadowDungeon::iter()`.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    EnumCount,
    EnumIter,
    Enum,
    FromPrimitive,
    Hash,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShadowDungeon {
    /// Desecrated Catacombs
    DesecratedCatacombs = 0,
    /// The Mines of Gloria
    MinesOfGloria = 1,
    /// The Ruins of Gnark
    RuinsOfGnark = 2,
    /// The Cutthroat Grotto
    CutthroatGrotto = 3,
    /// The Emerald Scale Altar
    EmeraldScaleAltar = 4,
    /// The Toxic Tree
    ToxicTree = 5,
    /// The Magma Stream
    MagmaStream = 6,
    /// The Frost Blood Temple
    FrostBloodTemple = 7,
    /// The Pyramids of Madness
    PyramidsOfMadness = 8,
    /// Black Skull Fortress
    BlackSkullFortress = 9,
    /// Circus of Horror
    CircusOfHorror = 10,
    /// Hell
    Hell = 11,
    /// The 13th Floor
    The13thFloor = 12,
    /// Easteros
    Easteros = 13,
    /// The Twister
    Twister = 14,
    /// Time-honored School of Magic
    TimeHonoredSchoolOfMagic = 15,
    /// Hemorridor
    Hemorridor = 16,
    /// Continuous Loop of Idols
    ContinuousLoopofIdols = 17,
    /// Nordic Gods
    NordicGods = 18,
    /// Mount Olympus
    MountOlympus = 19,
    /// Tavern of the Dark Doppelgangers
    TavernOfTheDarkDoppelgangers = 20,
    /// Dragon's Hoard
    DragonsHoard = 21,
    /// House of Horrors
    HouseOfHorrors = 22,
    /// Third League of Superheroes
    ThirdLeagueofSuperheroes = 23,
    /// Dojo of Childhood Heroes
    DojoOfChildhoodHeroes = 24,
    /// Monster Grotto
    MonsterGrotto = 25,
    /// City of Intrigues
    CityOfIntrigues = 26,
    /// School of Magic Express
    SchoolOfMagicExpress = 27,
    /// Ash Mountain
    AshMountain = 28,
    /// Playa Games HQ
    PlayaGamesHQ = 29,
    // TrainingDungeon & Sandstorm do not have a shadow version
    /// Arcade of the Old Pixel Icons
    ArcadeOfTheOldPixelIcons = 32,
    /// The Server Room
    TheServerRoom = 33,
    /// Workshop of the Hunters of the Undead
    WorkshopOfTheHunters = 34,
    /// Retro TV Legends
    RetroTVLegends = 35,
    /// The Meeting Room
    MeetingRoom = 36,
}

impl ShadowDungeon {
    /// Returns the total number of floors in this dungeon.
    #[must_use]
    pub fn total_floors(self) -> u16 {
        match self {
            ShadowDungeon::Twister => 1000,
            ShadowDungeon::ContinuousLoopofIdols => 30,
            _ => 10,
        }
    }
}

impl From<ShadowDungeon> for Dungeon {
    fn from(val: ShadowDungeon) -> Self {
        Dungeon::Shadow(val)
    }
}

fn update_progress<T: FromPrimitive + enum_map::Enum + Copy>(
    data: &Vals<'_>,
    dungeons: &mut EnumMap<T, DungeonProgress>,
    total_floors: impl Fn(T) -> u16,
) -> Result<()> {
    for dungeon_id in 0..data.len() {
        let progress = data.i64(dungeon_id)?;
        let Some(dungeon_typ) = FromPrimitive::from_usize(dungeon_id) else {
            continue;
        };
        dungeons[dungeon_typ] = match progress {
            -1 => DungeonProgress::Locked,
            x => {
                let stage = u16::try_from(x).unwrap_or(0);
                if stage >= total_floors(dungeon_typ) {
                    DungeonProgress::Finished
                } else {
                    DungeonProgress::Open { finished: stage }
                }
            }
        };
    }
    Ok(())
}

impl Dungeons {
    /// Checks whether a companion can equip the given item.
    #[must_use]
    pub fn can_companion_equip(
        &self,
        companion: CompanionClass,
        item: &Item,
    ) -> bool {
        // When we have no companions they can also not equip anything
        if self.companions.is_none() {
            return false;
        }
        item.can_be_equipped_by_companion(companion)
    }

    /// Updates dungeon progress from a `dungeonprogresslight` or
    /// `dungeonprogressshadow` response.
    pub(crate) fn update_progress(
        &mut self,
        data: &Vals<'_>,
        dungeon_type: DungeonType,
    ) -> Result<()> {
        match dungeon_type {
            DungeonType::Light => update_progress(
                data,
                &mut self.light,
                LightDungeon::total_floors,
            ),
            DungeonType::Shadow => update_progress(
                data,
                &mut self.shadow,
                ShadowDungeon::total_floors,
            ),
        }
    }

    /// Updates companion equipment from a `companionequipment` response.
    pub(crate) fn update_companion_equipment(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let companions = self.companions.get_or_insert_default();
        for (chunk, cmp) in data
            .chunks(EQUIPMENT_PARSE_LEN)
            .zip(companions.values_mut())
        {
            cmp.equipment = Equipment::parse(&chunk)?;
        }
        Ok(())
    }
}

/// The class of a companion (Bert, Mark, or Kunigunde).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumCount, Enum, EnumIter, Hash,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CompanionClass {
    /// Bert
    Warrior = 0,
    /// Mark
    Mage = 1,
    /// Kunigunde
    Scout = 2,
}

impl From<CompanionClass> for Class {
    fn from(value: CompanionClass) -> Self {
        match value {
            CompanionClass::Warrior => Class::Warrior,
            CompanionClass::Mage => Class::Mage,
            CompanionClass::Scout => Class::Scout,
        }
    }
}

/// Information about a single companion.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Companion {
    /// The level of this companion.
    pub level: i64,
    /// Equipment currently worn by this companion.
    pub equipment: Equipment,
    /// Total attributes of this companion.
    pub attributes: EnumMap<AttributeType, u32>,
}

impl Companion {
    /// Updates the level and attributes from an `owntower` companion block.
    /// The equipment comes from `companionequipment`; the rest is ignored,
    /// except the underworld unit, which `Underworld::update` reads.
    pub(crate) fn update(&mut self, data: &Vals<'_>) -> Result<()> {
        self.level = data.i64(0)?;
        for (idx, attr) in self.attributes.values_mut().enumerate() {
            *attr = data.u32(4 + idx)?;
        }
        // The 1-based index of this companion in the tower
        data.ignore(1, "companion index (1, 2, 3)");
        data.ignore(2, "always 0");
        data.ignore(3, "always 0");
        // A second attribute map; its largest value sits in this companion's
        // class main attribute slot, so it may include equipment attributes
        data.ignore(9..14, "2nd attribute map (basis or additions?)");
        data.ignore(14..19, "always 0");
        // The first value is an order of magnitude larger than the other two
        data.ignore(19..22, "unknown, maybe hp and damage");
        // The equipment the old code read here; 0 in every capture since it
        // moved to the `companionequipment` response. The last 5 values are
        // the underworld unit and the trailing value after it
        data.ignore(22..143, "old companion equipment, now 0");
        // The value at the end of the block, after the underworld unit
        data.ignore(147, "trailing value (10/13, 2, 0)");
        Ok(())
    }
}

/// Returns the static monster information for the current enemy in the given
/// dungeon at the specified progress.
#[cfg(feature = "simulation")]
pub fn get_dungeon_monster(
    dungeon: impl Into<Dungeon>,
    progress: DungeonProgress,
) -> Option<&'static crate::simulate::Monster> {
    let stage = match progress {
        DungeonProgress::Open { finished } => finished,
        DungeonProgress::Locked | DungeonProgress::Finished => return None,
    };
    crate::simulate::constants::get_dungeon_enemies(dungeon.into())
        .get(stage as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portal_update() {
        let raw = "33/100/54";
        let data = Vals::from(raw);
        let mut portal = Portal::default();
        let server_time = ServerTime::default();
        let current_day = chrono::Datelike::ordinal(&server_time.current());

        portal.update(&data, server_time).unwrap();
        data.assert_all_read();

        assert_eq!(portal.finished, 33);
        assert_eq!(portal.enemy_hp_percentage, 100);
        assert_eq!(portal.can_fight, current_day != 54);
    }

    #[test]
    fn test_light_dungeon_progress() {
        let raw = "10/10/10/10/10/10/10/10/10/10/10/10/10/8/100/8/6/33/2/1/1/\
                   10/6/0/0/-1/-1/-1/-1/0/10/-1/-1/-1/-1/-1/-1";
        let data = Vals::from(raw);
        let mut dungeons = Dungeons::default();

        dungeons.update_progress(&data, DungeonType::Light).unwrap();
        data.assert_all_read();

        assert_eq!(
            dungeons.progress(LightDungeon::DesecratedCatacombs),
            DungeonProgress::Finished
        );
        assert_eq!(
            dungeons.progress(LightDungeon::Easteros),
            DungeonProgress::Open { finished: 8 }
        );
        assert_eq!(
            dungeons.progress(LightDungeon::Tower),
            DungeonProgress::Finished
        );
        assert_eq!(
            dungeons.progress(LightDungeon::TimeHonoredSchoolofMagic),
            DungeonProgress::Open { finished: 8 }
        );
        assert_eq!(
            dungeons.progress(LightDungeon::Hemorridor),
            DungeonProgress::Open { finished: 6 }
        );
        assert_eq!(
            dungeons.progress(LightDungeon::MonsterGrotto),
            DungeonProgress::Locked
        );
        // Test Tower and Sandstorm floor limits
        let mut special_light = Dungeons::default();
        let mut light_vals = vec!["-1"; 37];
        light_vals[14] = "10";
        light_vals[31] = "10";
        let raw_special = light_vals.join("/");
        let data_special = Vals::from(raw_special.as_str());
        special_light
            .update_progress(&data_special, DungeonType::Light)
            .unwrap();
        assert_eq!(
            special_light.progress(LightDungeon::Tower),
            DungeonProgress::Open { finished: 10 }
        );
        assert_eq!(
            special_light.progress(LightDungeon::Sandstorm),
            DungeonProgress::Open { finished: 10 }
        );

        light_vals[31] = "1000";
        let raw_sandstorm_done = light_vals.join("/");
        let data_sandstorm_done = Vals::from(raw_sandstorm_done.as_str());
        special_light
            .update_progress(&data_sandstorm_done, DungeonType::Light)
            .unwrap();
        assert_eq!(
            special_light.progress(LightDungeon::Sandstorm),
            DungeonProgress::Finished
        );
    }

    #[test]
    fn test_shadow_dungeon_progress() {
        let raw = "10/10/10/10/10/10/10/10/10/10/10/10/2/-1/391/0/0/0/0/0/0/\
                   -1/-1/-1/-1/-1/-1/-1/-1/-1/0/0/-1/-1/-1/-1/-1";
        let data = Vals::from(raw);
        let mut dungeons = Dungeons::default();

        dungeons
            .update_progress(&data, DungeonType::Shadow)
            .unwrap();
        data.assert_all_read();

        assert_eq!(
            dungeons.progress(ShadowDungeon::DesecratedCatacombs),
            DungeonProgress::Finished
        );
        assert_eq!(
            dungeons.progress(ShadowDungeon::The13thFloor),
            DungeonProgress::Open { finished: 2 }
        );
        assert_eq!(
            dungeons.progress(ShadowDungeon::Easteros),
            DungeonProgress::Locked
        );
        assert_eq!(
            dungeons.progress(ShadowDungeon::Twister),
            DungeonProgress::Open { finished: 391 }
        );

        // Test Twister at floor 10 and 100
        let mut twister_open = Dungeons::default();
        let mut twister_vals = vec!["-1"; 37];
        twister_vals[14] = "10";
        let raw_twister_10 = twister_vals.join("/");
        let data_twister_10 = Vals::from(raw_twister_10.as_str());
        twister_open
            .update_progress(&data_twister_10, DungeonType::Shadow)
            .unwrap();
        assert_eq!(
            twister_open.progress(ShadowDungeon::Twister),
            DungeonProgress::Open { finished: 10 }
        );

        twister_vals[14] = "100";
        let raw_twister_100 = twister_vals.join("/");
        let data_twister_100 = Vals::from(raw_twister_100.as_str());
        twister_open
            .update_progress(&data_twister_100, DungeonType::Shadow)
            .unwrap();
        assert_eq!(
            twister_open.progress(ShadowDungeon::Twister),
            DungeonProgress::Open { finished: 100 }
        );

        // Test Twister cleared limit
        twister_vals[14] = "1000";
        let raw_twister = twister_vals.join("/");
        let data_twister = Vals::from(raw_twister.as_str());
        let mut twister_done = Dungeons::default();
        twister_done
            .update_progress(&data_twister, DungeonType::Shadow)
            .unwrap();
        assert_eq!(
            twister_done.progress(ShadowDungeon::Twister),
            DungeonProgress::Finished
        );

        // Test Continuous Loop of Idols at floor 10 and cleared limit
        let mut idols = Dungeons::default();
        let mut idols_vals = vec!["-1"; 37];
        idols_vals[17] = "10";
        let raw_idols_10 = idols_vals.join("/");
        let data_idols_10 = Vals::from(raw_idols_10.as_str());
        idols
            .update_progress(&data_idols_10, DungeonType::Shadow)
            .unwrap();
        assert_eq!(
            idols.progress(ShadowDungeon::ContinuousLoopofIdols),
            DungeonProgress::Open { finished: 10 }
        );

        idols_vals[17] = "21";
        let raw_idols_21 = idols_vals.join("/");
        let data_idols_21 = Vals::from(raw_idols_21.as_str());
        idols
            .update_progress(&data_idols_21, DungeonType::Shadow)
            .unwrap();
        assert_eq!(
            idols.progress(ShadowDungeon::ContinuousLoopofIdols),
            DungeonProgress::Open { finished: 21 }
        );

        idols_vals[17] = "30";
        let raw_idols = idols_vals.join("/");
        let data_idols = Vals::from(raw_idols.as_str());
        idols
            .update_progress(&data_idols, DungeonType::Shadow)
            .unwrap();
        assert_eq!(
            idols.progress(ShadowDungeon::ContinuousLoopofIdols),
            DungeonProgress::Finished
        );
    }

    #[test]
    fn test_companion_equipment_update() {
        let mut dungeons = Dungeons::default();

        // Empty data does nothing
        let empty_data = Vals::from("");
        dungeons.update_companion_equipment(&empty_data).unwrap();
        assert!(dungeons.companions.is_none());

        // 3 companion equipment blocks of zeros
        let raw = ["0"; EQUIPMENT_PARSE_LEN * 3].join("/");
        let data = Vals::from(raw.as_str());
        dungeons.update_companion_equipment(&data).unwrap();
        data.assert_all_read();

        let companions = dungeons.companions.as_ref().unwrap();
        for cmp in companions.values() {
            assert!(cmp.equipment.0.values().all(Option::is_none));
        }
    }
}
