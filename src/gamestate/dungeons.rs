use enum_map::{Enum, EnumMap};
use log::warn;
use strum::{EnumCount, EnumIter};

use super::{items::Equipment, AttributeType};
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
    pub(crate) fn update(&mut self, data: &[i64]) {
        self.current = match data[2] {
            0 => 0,
            _ => soft_into(data[0] + 1, "portal progress", 0),
        };
        self.enemy_hp_percentage = soft_into(data[1], "portal hitpoint", 0);
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dungeons {
    light_dungeons: [DungeonProgress; 30],
    shadow_dungeons: [DungeonProgress; 30],
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DungeonType {
    Light,
    Shadow,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumCount, EnumIter,
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
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumCount, EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShadowDungeons {
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

impl Dungeons {
    pub(crate) fn update(&mut self, data: &[i64], dungeon_type: DungeonType) {
        let dungeons = match dungeon_type {
            DungeonType::Light => &mut self.light_dungeons,
            DungeonType::Shadow => &mut self.shadow_dungeons,
        };

        for ((dungeon_id, progress), dungeon) in
            data.iter().copied().enumerate().zip(dungeons)
        {
            let level = match dungeon {
                DungeonProgress::Open { level, .. } => *level,
                _ => 0,
            };

            let progress = match progress {
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
            match dungeon_type {
                DungeonType::Shadow => {
                    if dungeon_id > ShadowDungeons::COUNT {
                        warn!("Unknown shadow dungeon id: {dungeon_id}");
                    }
                }
                DungeonType::Light => {
                    if dungeon_id == 17 {
                        *dungeon = DungeonProgress::Locked;
                        continue;
                    } else if dungeon_id > LightDungeon::COUNT {
                        warn!("Unknown light dungeon id: {dungeon_id}");
                    }
                }
            };
            *dungeon = progress;
        }
    }

    pub(crate) fn update_levels(&mut self, data: &[u16], typ: DungeonType) {
        let dungeons = match typ {
            DungeonType::Light => &mut self.light_dungeons,
            DungeonType::Shadow => &mut self.shadow_dungeons,
        };

        for (dungeon, level) in dungeons.iter_mut().zip(data) {
            let level = *level;
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
    }

    pub fn get_light(&self, typ: LightDungeon) -> DungeonProgress {
        self.light_dungeons[typ as usize]
    }

    pub fn get_shadow(&self, typ: ShadowDungeons) -> DungeonProgress {
        self.shadow_dungeons[typ as usize]
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::EnumCount,
    PartialOrd,
    Ord,
    Enum,
    EnumIter,
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
