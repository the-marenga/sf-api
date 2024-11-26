use chrono::{DateTime, Local};
use enum_map::{Enum, EnumArray, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumCount, EnumIter};

use super::{
    items::Equipment, AttributeType, CCGet, Class, EnumMapGet, Item, SFError,
    ServerTime,
};
use crate::{
    misc::soft_into,
    simulate::{
        constants::{LIGHT_ENEMIES, SHADOW_ENEMIES},
        Monster,
    },
};

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The personal demon portal
pub struct Portal {
    /// The amount of enemies you have fought in the portal already
    pub finished: u16,
    /// If this is true, you can fight the portal via the `FightPortal`
    /// command. You will have to wait until the next day (on the server) and
    /// send an `Update` to make sure this is set correctly
    pub can_fight: bool,
    /// The level of the enemy in the portal. For some reason this is always
    /// wrong by a few levels?
    pub enemy_level: u32,
    /// The amount of health the enemy has left
    pub enemy_hp_percentage: u8,
    /// Percentage boost to the players hp
    pub player_hp_bonus: u16,
}

impl Portal {
    pub(crate) fn update(
        &mut self,
        data: &[i64],
        server_time: ServerTime,
    ) -> Result<(), SFError> {
        self.finished = data.csiget(0, "portal fights", 10_000)?;
        self.enemy_hp_percentage = data.csiget(1, "portal hp", 0)?;

        let current_day = chrono::Datelike::ordinal(&server_time.current());
        let last_portal_day: u32 = data.csiget(2, "portal day", 0)?;
        self.can_fight = last_portal_day != current_day;

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The information about all generic dungeons in the game. Information about
/// special dungeons like the portal
pub struct Dungeons {
    /// The next time you can fight in the dungeons for free
    pub next_free_fight: Option<DateTime<Local>>,
    /// All the light dungeons. Notably tower information is also in here
    pub light: EnumMap<LightDungeon, DungeonProgress>,
    /// All the shadow dungeons. Notably twister & cont. loop of idols is also
    /// in here
    pub shadow: EnumMap<ShadowDungeon, DungeonProgress>,
    pub portal: Option<Portal>,
    /// The companions unlocked from unlocking the tower. Note that the tower
    /// info itself is just handled as a normal light dungeon
    pub companions: Option<EnumMap<CompanionClass, Companion>>,
}

impl Dungeons {
    /// Returns the progress for that dungeon
    pub fn progress(&self, dungeon: impl Into<Dungeon>) -> DungeonProgress {
        let dungeon: Dungeon = dungeon.into();
        match dungeon {
            Dungeon::Light(dungeon) => *self.light.get(dungeon),
            Dungeon::Shadow(dungeon) => *self.shadow.get(dungeon),
        }
    }

    /// Returns the current enemy for that dungeon. Note that the special
    /// "mirrorimage" enemy will be listed as a warrior with 0 stats/lvl/xp/hp.
    // If you care about the actual stats, you should map this to the player
    // stats yourself
    pub fn current_enemy(
        &self,
        dungeon: impl Into<Dungeon> + Copy,
    ) -> Option<&'static Monster> {
        dungeon_enemy(dungeon, self.progress(dungeon))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The current state of a dungeon
pub enum DungeonProgress {
    #[default]
    /// The dungeon has not yet been unlocked
    Locked,
    /// The dungeon is open and can be fought in
    Open {
        /// The amount of enemies already finished
        finished: u16,
    },
    /// The dungeon has been fully cleared and can not be entered anymore
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The category of a dungeon. This is only used internally, so there is no
/// real point for you to use this
pub enum DungeonType {
    Light,
    Shadow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
/// The category of a dungeon. This is only used internally, so there is no
/// real point for you to use this
pub enum Dungeon {
    Light(LightDungeon),
    Shadow(ShadowDungeon),
}

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
#[allow(missing_docs)]
/// All possible light dungeons. They are NOT numbered continuously (17 is
/// missing), so you should use `LightDungeon::iter()`, if you want to iterate
/// these
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
    TrainingCamp = 30,
    Sandstorm = 31,
}

impl From<LightDungeon> for Dungeon {
    fn from(val: LightDungeon) -> Self {
        Dungeon::Light(val)
    }
}

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
#[allow(missing_docs)]
/// All possible shadow dungeons. You can use `ShadowDungeon::iter()`, if you
/// want to iterate these
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

impl From<ShadowDungeon> for Dungeon {
    fn from(val: ShadowDungeon) -> Self {
        Dungeon::Shadow(val)
    }
}

fn update_progress<T: FromPrimitive + EnumArray<DungeonProgress>>(
    data: &[i64],
    dungeons: &mut EnumMap<T, DungeonProgress>,
) {
    for (dungeon_id, progress) in data.iter().copied().enumerate() {
        let Some(dungeon_typ) = FromPrimitive::from_usize(dungeon_id) else {
            continue;
        };
        let dungeon = dungeons.get_mut(dungeon_typ);
        *dungeon = match progress {
            -1 => DungeonProgress::Locked,
            x => {
                let stage = soft_into(x, "dungeon progress", 0);
                if stage == 10 || stage == 100 && dungeon_id == 14 {
                    DungeonProgress::Finished
                } else {
                    DungeonProgress::Open { finished: stage }
                }
            }
        };
    }
}

impl Dungeons {
    /// Check if a specific companion can equip the given item
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

    pub(crate) fn update_progress(
        &mut self,
        data: &[i64],
        dungeon_type: DungeonType,
    ) {
        match dungeon_type {
            DungeonType::Light => update_progress(data, &mut self.light),
            DungeonType::Shadow => {
                update_progress(data, &mut self.shadow);
                for (dungeon, limit) in [
                    (ShadowDungeon::ContinuousLoopofIdols, 21),
                    (ShadowDungeon::Twister, 1000),
                ] {
                    let d = self.shadow.get_mut(dungeon);
                    if let DungeonProgress::Open { finished, .. } = d {
                        if *finished >= limit {
                            *d = DungeonProgress::Finished;
                        }
                    }
                }
            }
        };
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumCount, Enum, EnumIter, Hash,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The class of a companion. There is only 1 companion per class, so this is
/// also a ident of the characters
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

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// All the information about a single companion. The class is not included
/// here, as you access this via a map, where the key will be the class
pub struct Companion {
    /// I can not recall, if I made this signed on purpose, because this should
    /// always be > 0
    pub level: i64,
    /// The equipment this companion is wearing
    pub equipment: Equipment,
    /// The total attributes of this companion
    pub attributes: EnumMap<AttributeType, u32>,
}

pub fn dungeon_enemy(
    dungeon: impl Into<Dungeon>,
    progress: DungeonProgress,
) -> Option<&'static Monster> {
    let stage = match progress {
        DungeonProgress::Open { finished } => finished,
        DungeonProgress::Locked | DungeonProgress::Finished => return None,
    };

    let dungeon: Dungeon = dungeon.into();
    match dungeon {
        Dungeon::Light(dungeon) => {
            LIGHT_ENEMIES.get(dungeon).get(stage as usize)
        }
        Dungeon::Shadow(dungeon) => {
            SHADOW_ENEMIES.get(dungeon).get(stage as usize)
        }
    }
}
