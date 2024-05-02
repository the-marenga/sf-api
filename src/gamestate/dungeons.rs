use enum_map::{Enum, EnumMap};
use num::FromPrimitive;
use num_derive::FromPrimitive;
use strum::{EnumCount, EnumIter};

use super::{items::Equipment, AttributeType, CCGet, CGet, SFError};
use crate::misc::soft_into;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Portal {
    /// The current position in the portal. Starts with 1.
    /// (current - 1) / 10 => act
    pub current: u16,
    /// Supposed to be the level of the enemy, I think. Every time I check,
    /// this was wrong by a bit. No idea why
    pub enemy_level: u16,
    /// The amount of health the enemy has left
    pub enemy_hp_percentage: u8,
    /// Percentage boost to the players hp
    pub player_hp_bonus: u16,
}

impl Portal {
    pub(crate) fn update(&mut self, data: &[i64]) -> Result<(), SFError> {
        self.current = match data.cget(2, "portal unlocked")? {
            0 => 0,
            _ => soft_into(
                data.cget(0, "portal progress")? + 1,
                "portal progress",
                0,
            ),
        };
        self.enemy_hp_percentage = data.csiget(1, "portal hitpoint", 0)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dungeons {
    pub light_dungeons: EnumMap<LightDungeon, DungeonProgress>,
    pub shadow_dungeons: EnumMap<ShadowDungeon, DungeonProgress>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DungeonProgress {
    #[default]
    Locked,
    Open {
        /// The amount of enemies already finished
        finished: u16,
        /// The level of the enemy currently
        level: u16,
    },
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DungeonType {
    Light,
    Shadow,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumCount, EnumIter, Enum, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LightDungeon {
    DesecratedCatacombs = 0,
    MinesOfGloria = 1,
    RuinsOfGnark = 2,
    CutthroatGrotto = 3,
    EmeraldScaleAltar = 4,
    ToxicTree = 5,
    MagmaStream = 6,
    FrostBloodTemple = 7,
    PyramidsofMadness = 8,
    BlackSkullFortress = 9,
    CircusOfHorror = 10,
    Hell = 11,
    The13thFloor = 12,
    Easteros = 13,
    Tower = 14,
    TimeHonoredSchoolofMagic = 15,
    Hemorridor = 16,
    NordicGods = 18,
    MountOlympus = 19,
    TavernoftheDarkDoppelgangers = 20,
    DragonsHoard = 21,
    HouseOfHorrors = 22,
    ThirdLeagueOfSuperheroes = 23,
    DojoOfChildhoodHeroes = 24,
    MonsterGrotto = 25,
    CityOfIntrigues = 26,
    SchoolOfMagicExpress = 27,
    AshMountain = 28,
    PlayaGamesHQ = 29,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumCount, EnumIter, Enum, FromPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShadowDungeon {
    DesecratedCatacombs = 0,
    MinesOfGloria = 1,
    RuinsOfGnark = 2,
    CutthroatGrotto = 3,
    EmeraldScaleAltar = 4,
    ToxicTree = 5,
    MagmaStream = 6,
    FrostBloodTemple = 7,
    PyramidsOfMadness = 8,
    BlackSkullFortress = 9,
    CircusOfHorror = 10,
    Hell = 11,
    The13thFloor = 12,
    Easteros = 13,
    Twister = 14,
    TimeHonoredSchoolOfMagic = 15,
    Hemorridor = 16,
    ContinuousLoopofIdols = 17,
    NordicGods = 18,
    MountOlympus = 19,
    TavernOfTheDarkDoppelgangers = 20,
    DragonsHoard = 21,
    HouseOfHorrors = 22,
    ThirdLeagueofSuperheroes = 23,
    DojoOfChildhoodHeroes = 24,
    MonsterGrotto = 25,
    CityOfIntrigues = 26,
    SchoolOfMagicExpress = 27,
    AshMountain = 28,
    PlayaGamesHQ = 29,
}

macro_rules! update_progress {
    ($data:expr, $dungeons:expr) => {
        for (dungeon_id, progress) in $data.iter().copied().enumerate() {
            let Some(dungeon_typ) = FromPrimitive::from_usize(dungeon_id)
            else {
                continue;
            };
            #[allow(clippy::indexing_slicing)]
            let dungeon = &mut $dungeons[dungeon_typ];
            let level = match dungeon {
                DungeonProgress::Open { level, .. } => *level,
                _ => 0,
            };
            *dungeon = match progress {
                -1 => DungeonProgress::Locked,
                x => {
                    let stage = soft_into(x, "dungeon progress", 0);
                    if stage == 10 {
                        DungeonProgress::Finished
                    } else {
                        DungeonProgress::Open {
                            finished: stage,
                            level,
                        }
                    }
                }
            };
        }
    };
}

macro_rules! update_levels {
    ($dungeons:expr, $data:expr) => {
        for (dungeon_id, level) in $data.iter().copied().enumerate() {
            let Some(dungeon_typ) = FromPrimitive::from_usize(dungeon_id)
            else {
                continue;
            };
            #[allow(clippy::indexing_slicing)]
            let dungeon = &mut $dungeons[dungeon_typ];

            if level < 1 {
                // Either Finished or not unlocked
                continue;
            }

            use DungeonProgress::*;
            let stage = match dungeon {
                Open {
                    finished: stage, ..
                } => *stage,
                _ => 0,
            };

            *dungeon = DungeonProgress::Open {
                finished: stage,
                level,
            }
        }
    };
}

impl Dungeons {
    pub(crate) fn update_progress(
        &mut self,
        data: &[i64],
        dungeon_type: DungeonType,
    ) {
        match dungeon_type {
            DungeonType::Light => {
                update_progress!(data, self.light_dungeons)
            }
            DungeonType::Shadow => {
                update_progress!(data, self.shadow_dungeons)
            }
        };
    }

    pub(crate) fn update_levels(&mut self, data: &[u16], typ: DungeonType) {
        match typ {
            DungeonType::Light => {
                update_levels!(self.light_dungeons, data)
            }
            DungeonType::Shadow => {
                update_levels!(self.shadow_dungeons, data)
            }
        };
    }

    #[deprecated(note = "You should access the dungeon member map directly")]
    pub fn get_light(&self, typ: LightDungeon) -> DungeonProgress {
        #[allow(clippy::indexing_slicing)]
        self.light_dungeons[typ]
    }

    #[deprecated(note = "You should access the dungeon member map directly")]
    pub fn get_shadow(&self, typ: ShadowDungeon) -> DungeonProgress {
        #[allow(clippy::indexing_slicing)]
        self.shadow_dungeons[typ]
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumCount, Enum, EnumIter, Hash,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CompanionClass {
    Warrior = 0,
    Mage = 1,
    Scout = 2,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Companion {
    pub level: i64,
    pub equipment: Equipment,
    pub attributes: EnumMap<AttributeType, u32>,
}
