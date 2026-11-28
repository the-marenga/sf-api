use chrono::{DateTime, Local};
use enum_map::{Enum, EnumMap};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum::{EnumCount, EnumIter};

use super::{
    AttributeType, Class, EnumMapGet, Item, SFError, ServerTime,
    items::{EQUIPMENT_PARSE_LEN, Equipment},
};
use crate::{misc::soft_into, response::Vals};

/// The personal demon portal
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub(crate) fn update(&mut self, data: &Vals<'_>, server_time: ServerTime) {
        self.finished =
            soft_into(data.i64_or(0, 10_000), "portal fights", 10_000);
        self.enemy_hp_percentage = soft_into(data.i64_or(1, 0), "portal hp", 0);

        let current_day = chrono::Datelike::ordinal(&server_time.current());
        let last_portal_day: u32 =
            soft_into(data.i64_or(2, 0), "portal day", 0);
        self.can_fight = last_portal_day != current_day;

        data.ignore(3..data.len(), "extra portal values");
    }
}

/// The information about all generic dungeons in the game. Information about
/// special dungeons like the portal
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    #[cfg(feature = "simulation")]
    pub fn current_enemy(
        &self,
        dungeon: impl Into<Dungeon> + Copy,
    ) -> Option<&'static crate::simulate::Monster> {
        get_dungeon_monster(dungeon, self.progress(dungeon))
    }
}

/// The current state of a dungeon
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DungeonProgress {
    /// The dungeon has not yet been unlocked
    #[default]
    Locked,
    /// The dungeon is open and can be fought in
    Open {
        /// The amount of enemies already finished
        finished: u16,
    },
    /// The dungeon has been fully cleared and can not be entered anymore
    Finished,
}

/// The category of a dungeon. This is only used internally, so there is no
/// real point for you to use this
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum DungeonType {
    Light,
    Shadow,
}

/// The category of a dungeon. This is only used internally, so there is no
/// real point for you to use this
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Dungeon {
    Light(LightDungeon),
    Shadow(ShadowDungeon),
}

impl Dungeon {
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
}

/// All possible light dungeons. They are NOT numbered continuously (17 is
/// missing), so you should use `LightDungeon::iter()`, if you want to iterate
/// these
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
    ArcadeOfTheOldPixelIcons = 32,
    TheServerRoom = 33,
    WorkshopOfTheHunters = 34,
    RetroTVLegends = 35,
    MeetingRoom = 36,
}

impl From<LightDungeon> for Dungeon {
    fn from(val: LightDungeon) -> Self {
        Dungeon::Light(val)
    }
}

/// All possible shadow dungeons. You can use `ShadowDungeon::iter()`, if you
/// want to iterate these
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
    // TrainingDungeon & Sandstorm do not have a shadow version
    ArcadeOfTheOldPixelIcons = 32,
    TheServerRoom = 33,
    WorkshopOfTheHunters = 34,
    RetroTVLegends = 35,
    MeetingRoom = 36,
}

impl From<ShadowDungeon> for Dungeon {
    fn from(val: ShadowDungeon) -> Self {
        Dungeon::Shadow(val)
    }
}

fn update_progress<T: FromPrimitive + enum_map::Enum>(
    data: &Vals<'_>,
    dungeons: &mut EnumMap<T, DungeonProgress>,
) {
    for (dungeon_id, raw) in data.iter().enumerate() {
        // Malformed values are not worth warning about and leave the dungeon
        // progress untouched; iterating still marks every value as read.
        let Ok(x) = raw.parse::<i64>() else {
            continue;
        };
        let Some(dungeon_typ) = FromPrimitive::from_usize(dungeon_id) else {
            continue;
        };
        let dungeon = dungeons.get_mut(dungeon_typ);
        *dungeon = match x {
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
        data: &Vals<'_>,
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
                    if let DungeonProgress::Open { finished, .. } = d
                        && *finished >= limit
                    {
                        *d = DungeonProgress::Finished;
                    }
                }
            }
        }
    }

    pub(crate) fn update_companion_equipment(
        &mut self,
        data: &Vals<'_>,
    ) -> Result<(), SFError> {
        // An empty response does not imply that the player has companions.
        if data.is_empty() {
            return Ok(());
        }
        for (idx, cmp) in self
            .companions
            .get_or_insert_with(Default::default)
            .values_mut()
            .enumerate()
        {
            let window = data.window(idx * EQUIPMENT_PARSE_LEN..data.len());
            cmp.equipment = Equipment::parse(&window)?;
        }
        Ok(())
    }
}

/// The class of a companion. There is only 1 companion per class, so this is
/// also a ident of the characters
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

/// All the information about a single companion. The class is not included
/// here, as you access this via a map, where the key will be the class
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Companion {
    /// I can not recall, if I made this signed on purpose, because this should
    /// always be > 0
    pub level: i64,
    /// The equipment this companion is wearing
    pub equipment: Equipment,
    /// The total attributes of this companion
    pub attributes: EnumMap<AttributeType, u32>,
}

impl Companion {
    pub(crate) fn update(&mut self, data: &Vals<'_>) {
        self.level = data.i64_or(0, 0);
        for (idx, attr) in self.attributes.values_mut().enumerate() {
            *attr = data.u32_or(4 + idx, 0);
        }
        data.ignore(1, "companion index (1, 2, 3)");
        data.ignore(2, "always 0");
        data.ignore(3, "always 0");
        data.ignore(9..14, "2nd attribute map (basis or additions?)");
        data.ignore(14..19, "always 0");
        data.ignore(19..22, "unknown, maybe hp and damage");
        data.ignore(22..143, "old companion equipment, now 0");
        data.ignore(147, "trailing value (10/13, 2, 0)");
    }
}

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
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    fn current_day() -> u32 {
        chrono::Datelike::ordinal(&ServerTime(0).current())
    }

    #[test]
    fn portal_full_values_respect_the_day_and_ignore_the_tail() {
        let raw = format!("5/42/{}/1/2/3", current_day());
        let data = Vals::from(raw.as_str());
        let mut portal = Portal::default();

        portal.update(&data, ServerTime(0));

        data.assert_all_read();
        assert_eq!(portal.finished, 5);
        assert_eq!(portal.enemy_hp_percentage, 42);
        assert!(!portal.can_fight);
    }

    #[test]
    fn portal_truncated_values_use_defaults() {
        let data = Vals::from("7");
        let mut portal = Portal::default();

        portal.update(&data, ServerTime(0));

        data.assert_all_read();
        assert_eq!(portal.finished, 7);
        assert_eq!(portal.enemy_hp_percentage, 0);
        assert_ne!(current_day(), 0);
        assert!(portal.can_fight);
    }

    #[test]
    fn portal_extra_values_do_not_break_can_fight() {
        let raw = format!("3/99/{}/0/0/0/0", current_day() + 1);
        let data = Vals::from(raw.as_str());
        let mut portal = Portal::default();

        portal.update(&data, ServerTime(0));

        data.assert_all_read();
        assert_eq!(portal.finished, 3);
        assert_eq!(portal.enemy_hp_percentage, 99);
        assert!(portal.can_fight);
    }

    #[test]
    fn light_progress_skips_unknown_ids_and_applies_thresholds() {
        let values = [
            "0", "-1", "5", "10", "100", "0", "0", "0", "0", "0", "0", "0",
            "0", "0", "100", "0", "0", "123",
        ];
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut dungeons = Dungeons::default();

        dungeons.update_progress(&data, DungeonType::Light);

        data.assert_all_read();
        assert_eq!(
            dungeons.light.get(LightDungeon::DesecratedCatacombs),
            &DungeonProgress::Open { finished: 0 }
        );
        assert_eq!(
            dungeons.light.get(LightDungeon::MinesOfGloria),
            &DungeonProgress::Locked
        );
        assert_eq!(
            dungeons.light.get(LightDungeon::RuinsOfGnark),
            &DungeonProgress::Open { finished: 5 }
        );
        assert_eq!(
            dungeons.light.get(LightDungeon::CutthroatGrotto),
            &DungeonProgress::Finished
        );
        assert_eq!(
            dungeons.light.get(LightDungeon::EmeraldScaleAltar),
            &DungeonProgress::Open { finished: 100 }
        );
        assert_eq!(
            dungeons.light.get(LightDungeon::Tower),
            &DungeonProgress::Finished
        );
        // Id 17 is not a valid light dungeon and id 18 was not in the data.
        assert_eq!(
            dungeons.light.get(LightDungeon::NordicGods),
            &DungeonProgress::Locked
        );
    }

    #[test]
    fn shadow_progress_finishes_id_14_and_applies_the_post_pass() {
        let mut values = vec!["0"; 18];
        values[14] = "100";
        values[17] = "20";
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut dungeons = Dungeons::default();

        dungeons.update_progress(&data, DungeonType::Shadow);

        data.assert_all_read();
        assert_eq!(
            dungeons.shadow.get(ShadowDungeon::Twister),
            &DungeonProgress::Finished
        );
        assert_eq!(
            dungeons.shadow.get(ShadowDungeon::ContinuousLoopofIdols),
            &DungeonProgress::Open { finished: 20 }
        );

        let mut values = vec!["0"; 18];
        values[14] = "1000";
        values[17] = "21";
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut dungeons = Dungeons::default();

        dungeons.update_progress(&data, DungeonType::Shadow);

        data.assert_all_read();
        assert_eq!(
            dungeons.shadow.get(ShadowDungeon::Twister),
            &DungeonProgress::Finished
        );
        assert_eq!(
            dungeons.shadow.get(ShadowDungeon::ContinuousLoopofIdols),
            &DungeonProgress::Finished
        );
    }

    #[test]
    fn malformed_progress_values_are_skipped_without_unread_values() {
        let data = Vals::from("abc/5/xyz");
        let mut dungeons = Dungeons::default();

        dungeons.update_progress(&data, DungeonType::Light);

        data.assert_all_read();
        assert_eq!(
            dungeons.light.get(LightDungeon::DesecratedCatacombs),
            &DungeonProgress::Locked
        );
        assert_eq!(
            dungeons.light.get(LightDungeon::MinesOfGloria),
            &DungeonProgress::Open { finished: 5 }
        );
        assert_eq!(
            dungeons.light.get(LightDungeon::RuinsOfGnark),
            &DungeonProgress::Locked
        );
    }

    #[test]
    fn companion_equipment_empty_does_not_create_companions() {
        let data = Vals::from("");
        let mut dungeons = Dungeons::default();

        dungeons.update_companion_equipment(&data).unwrap();

        data.assert_all_read();
        assert!(dungeons.companions.is_none());
    }

    #[test]
    fn companion_equipment_one_block_is_read() {
        let raw = ["0"; EQUIPMENT_PARSE_LEN].join("/");
        let data = Vals::from(raw.as_str());
        let mut dungeons = Dungeons::default();

        dungeons.update_companion_equipment(&data).unwrap();

        data.assert_all_read();
        let companions = dungeons.companions.as_ref().unwrap();
        assert!(
            companions.values().all(|cmp| cmp
                .equipment
                .0
                .values()
                .all(Option::is_none))
        );
    }

    #[test]
    fn companion_equipment_two_blocks_are_split_between_companions() {
        let mut values = vec!["0"; EQUIPMENT_PARSE_LEN * 2];
        values[EQUIPMENT_PARSE_LEN] = "1";
        let raw = values.join("/");
        let data = Vals::from(raw.as_str());
        let mut dungeons = Dungeons::default();

        dungeons.update_companion_equipment(&data).unwrap();

        data.assert_all_read();
        let companions = dungeons.companions.as_ref().unwrap();
        assert!(
            companions[CompanionClass::Warrior]
                .equipment
                .0
                .values()
                .all(Option::is_none)
        );
        assert!(
            companions[CompanionClass::Mage]
                .equipment
                .0
                .values()
                .any(Option::is_some)
        );
        assert!(
            companions[CompanionClass::Scout]
                .equipment
                .0
                .values()
                .all(Option::is_none)
        );
    }
}
