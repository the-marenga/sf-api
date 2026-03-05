#![allow(clippy::unreadable_literal)]

use std::{collections::HashMap, sync::LazyLock};

use enum_map::EnumMap;

use crate::{
    gamestate::{
        character::Class,
        dungeons::{Dungeon, LightDungeon, ShadowDungeon},
        unlockables::HabitatType,
    },
    simulate::{Element, Monster, MonsterRunes},
};

include!(concat!(env!("OUT_DIR"), "/dungeon_data.rs"));

#[must_use]
pub fn get_dungeon_enemies(dungeon: Dungeon) -> &'static [Monster] {
    match dungeon {
        Dungeon::Light(light) => DUNGEON_MONSTERS_LIGHT[light],
        Dungeon::Shadow(shadow) => DUNGEON_MONSTERS_SHADOW[shadow],
    }
}

pub(crate) static PET_MONSTER: LazyLock<
    HashMap<HabitatType, &'static [Monster]>,
> = LazyLock::new(|| PET_MONSTERS.into_iter().collect());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dungeon_access() {
        let monsters =
            get_dungeon_enemies(LightDungeon::DesecratedCatacombs.into());
        assert!(!monsters.is_empty());
    }

    #[test]
    fn test_access_pet_dungeons() {
        let pet_dungeons = &*PET_MONSTER;
        assert!(!pet_dungeons.is_empty());
    }
}
