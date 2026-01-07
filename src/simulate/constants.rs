#![allow(clippy::unreadable_literal)]

use std::{array, collections::HashMap, sync::LazyLock};

use enum_map::EnumMap;

use crate::{
    gamestate::{
        character::Class,
        dungeons::{Dungeon, LightDungeon, ShadowDungeon},
    },
    misc::EnumMapGet,
    simulate::{Element, Monster, MonsterRunes},
};

static DUNGEONS_MONSTER: LazyLock<DungeonMonsters> =
    LazyLock::new(DungeonMonsters::parse);

#[must_use]
pub fn get_dungeon_enemies(dungeon: Dungeon) -> &'static [Monster] {
    match dungeon {
        Dungeon::Light(light) => DUNGEONS_MONSTER.light.get(light),
        Dungeon::Shadow(shadow) => DUNGEONS_MONSTER.shadow.get(shadow),
    }
}

#[derive(serde::Deserialize)]
struct DungeonData<'a> {
    levels: usize,
    #[serde(borrow)]
    monsters: Vec<MonsterData<'a>>,
}

#[derive(serde::Deserialize, Debug, Default)]
pub struct RawMonsterRunes {
    #[serde(rename = "type")]
    typ: u32,
    res: [i32; 3],
    damage: i32,
}

#[derive(serde::Deserialize)]
struct MonsterData<'a> {
    #[serde(borrow)]
    name: Option<&'a str>,
    class: Option<&'a str>,
    level: Option<u16>,
    strength: Option<u32>,
    dexterity: Option<u32>,
    intelligence: Option<u32>,
    constitution: Option<u32>,
    luck: Option<u32>,
    life: Option<u64>,
    min_dmg: Option<u32>,
    max_dmg: Option<u32>,
    armor: Option<u32>,
    runes: Option<RawMonsterRunes>,
}

#[derive(Debug)]
pub struct DungeonMonsters {
    pub light: EnumMap<LightDungeon, Vec<Monster>>,
    pub shadow: EnumMap<ShadowDungeon, Vec<Monster>>,
}

impl DungeonMonsters {
    #[allow(clippy::unwrap_used, clippy::missing_panics_doc)]
    #[must_use]
    pub fn parse() -> Self {
        let data: HashMap<&'static str, DungeonData<'static>> =
            serde_json::from_str(include_str!("dungeons.json")).unwrap();

        let mut res = Self {
            light: EnumMap::default(),
            shadow: EnumMap::default(),
        };

        for (dungeon, monsters) in &mut res.light {
            let name = get_light_dungeon_name(dungeon);
            let data = data.get(name).unwrap();
            read_dungeon_data(monsters, data, dungeon.into());
        }
        for (dungeon, monsters) in &mut res.shadow {
            let name = get_shadow_dungeon_name(dungeon);
            let data = data.get(name).unwrap();
            read_dungeon_data(monsters, data, dungeon.into());
        }
        res
    }
}

fn read_dungeon_data(
    monsters: &mut Vec<Monster>,
    data: &DungeonData<'_>,
    dungeon: Dungeon,
) {
    let default_name = |idx| {
        match dungeon {
            Dungeon::Light(d) => format!("{d:?} monster #{idx}",),
            Dungeon::Shadow(d) => format!("Shadow {d:?} monster #{idx}",),
        }
        .into()
    };

    for (idx, monster) in data.monsters.iter().enumerate() {
        let class = match monster.class.unwrap_or("Warrior") {
            "Warrior" => Class::Warrior,
            "Mage" => Class::Mage,
            "Scout" => Class::Scout,
            "Assassin" => Class::Assassin,
            "WarMage" | "BattleMage" => Class::BattleMage,
            "Berserker" => Class::Berserker,
            "DemonHunter" => Class::DemonHunter,
            "Druid" => Class::Druid,
            "Bard" => Class::Bard,
            "Necromancer" => Class::Necromancer,
            "Paladin" => Class::Paladin,
            "PlagueDoctor" => Class::PlagueDoctor,
            _ => todo!(),
        };
        let level = monster.level.unwrap_or(u16::MAX);
        let armor = monster.armor.unwrap_or_else(|| {
            let res = u32::from(level) * class.max_armor_reduction();
            match dungeon {
                Dungeon::Light(LightDungeon::TavernoftheDarkDoppelgangers) => {
                    res / 2
                }
                Dungeon::Light(LightDungeon::Tower) => {
                    (f64::from(res) * 1.5) as u32
                }
                _ => res,
            }
        });

        let runes = match &monster.runes {
            Some(runes) => {
                let typ = match runes.typ {
                    40 => Element::Fire,
                    41 => Element::Cold,
                    42 => Element::Lightning,
                    _ => todo!(),
                };
                Some(MonsterRunes {
                    damage_type: typ,
                    damage: runes.damage,
                    resistances: EnumMap::from_array(runes.res),
                })
            }
            None => None,
        };

        let monster = Monster {
            name: monster
                .name
                .as_ref()
                .map(|a| (*a).replace('_', " ").into())
                .unwrap_or(default_name(idx)),
            level,
            class,
            attributes: EnumMap::from_array([
                monster.strength.unwrap_or(u32::MAX),
                monster.dexterity.unwrap_or(u32::MAX),
                monster.intelligence.unwrap_or(u32::MAX),
                monster.constitution.unwrap_or(u32::MAX),
                monster.luck.unwrap_or(u32::MAX),
            ]),
            hp: monster.life.unwrap_or(u64::MAX),
            min_dmg: monster.min_dmg.unwrap_or(0),
            max_dmg: monster.max_dmg.unwrap_or(0),
            armor,
            runes,
        };
        monsters.push(monster);
    }
    while monsters.len() < data.levels {
        monsters.push(Monster {
            name: default_name(monsters.len()),
            level: u16::MAX,
            class: Class::Assassin,
            attributes: EnumMap::from_array(array::from_fn(|_| u32::MAX)),
            hp: u64::MAX,
            min_dmg: u32::MAX,
            max_dmg: u32::MAX,
            armor: u32::MAX,
            runes: None,
        });
    }
}

fn get_light_dungeon_name(dungeon: LightDungeon) -> &'static str {
    match dungeon {
        LightDungeon::DesecratedCatacombs => "Desecrated_Catacombs",
        LightDungeon::MinesOfGloria => "Mines_of_Gloria",
        LightDungeon::RuinsOfGnark => "Ruins_of_Gnark",
        LightDungeon::CutthroatGrotto => "Cutthroat_Grotto",
        LightDungeon::EmeraldScaleAltar => "Emerald_Scale_Altar",
        LightDungeon::ToxicTree => "Toxic_Tree",
        LightDungeon::MagmaStream => "Magma_Stream",
        LightDungeon::FrostBloodTemple => "Frost_Blood_Temple",
        LightDungeon::PyramidsofMadness => "Pyramids_of_Madness",
        LightDungeon::BlackSkullFortress => "Black_Skull_Fortress",
        LightDungeon::CircusOfHorror => "Circus_of_Horror",
        LightDungeon::Hell => "Hell",
        LightDungeon::The13thFloor => "The_13th_Floor",
        LightDungeon::Easteros => "Osteros",
        LightDungeon::Tower => "Tower",
        LightDungeon::TimeHonoredSchoolofMagic => {
            "Time_Honored_School_of_Magic"
        }
        LightDungeon::Hemorridor => "Hemorridor",
        LightDungeon::NordicGods => "Nordic",
        LightDungeon::MountOlympus => "Mount_Olympus",
        LightDungeon::TavernoftheDarkDoppelgangers => {
            "Tavern_of_the_Dark_Doppelgangers"
        }
        LightDungeon::DragonsHoard => "Dragons_Hoard",
        LightDungeon::HouseOfHorrors => "House_of_Horrors",
        LightDungeon::ThirdLeagueOfSuperheroes => {
            "The_3rd_League_of_Superheroes"
        }
        LightDungeon::DojoOfChildhoodHeroes => "Dojo_of_Childhood_Heroes",
        LightDungeon::MonsterGrotto => "Monster_Grotto",
        LightDungeon::CityOfIntrigues => "City_of_Intrigues",
        LightDungeon::SchoolOfMagicExpress => "School_of_magic_Express",
        LightDungeon::AshMountain => "Ash_Mountain",
        LightDungeon::PlayaGamesHQ => "Playa_HQ",
        LightDungeon::TrainingCamp => "Training_Camp",
        LightDungeon::Sandstorm => "Sandstorm",
        LightDungeon::ArcadeOfTheOldPixelIcons => {
            "Arcade_of_the_Old_Pixel_Icons"
        }
        LightDungeon::TheServerRoom => "The_Server_Room",
        LightDungeon::WorkshopOfTheHunters => {
            "Workshop_of_the_Hunters_of_the_Undead"
        }
        LightDungeon::RetroTVLegends => "Retro_TV_Legends",
        LightDungeon::MeetingRoom => "The_Meeting_Room",
    }
}
fn get_shadow_dungeon_name(dungeon: ShadowDungeon) -> &'static str {
    match dungeon {
        ShadowDungeon::DesecratedCatacombs => "Shadow_Desecrated_Catacombs",
        ShadowDungeon::MinesOfGloria => "Shadow_Mines_of_Gloria",
        ShadowDungeon::RuinsOfGnark => "Shadow_Ruins_of_Gnark",
        ShadowDungeon::CutthroatGrotto => "Shadow_Cutthroat_Grotto",
        ShadowDungeon::EmeraldScaleAltar => "Shadow_Emerald_Scale_Altar",
        ShadowDungeon::ToxicTree => "Shadow_Toxic_Tree",
        ShadowDungeon::MagmaStream => "Shadow_Magma_Stream",
        ShadowDungeon::FrostBloodTemple => "Shadow_Frost_Blood_Temple",
        ShadowDungeon::PyramidsOfMadness => "Shadow_Pyramids_of_Madness",
        ShadowDungeon::BlackSkullFortress => "Shadow_Black_Skull_Fortress",
        ShadowDungeon::CircusOfHorror => "Shadow_Circus_of_Horror",
        ShadowDungeon::Hell => "Shadow_Hell",
        ShadowDungeon::The13thFloor => "Shadow_The_13th_Floor",
        ShadowDungeon::Easteros => "Shadow_Osteros",
        ShadowDungeon::Twister => "Twister",
        ShadowDungeon::TimeHonoredSchoolOfMagic => {
            "Shadow_Time_Honored_School_of_Magic"
        }
        ShadowDungeon::Hemorridor => "Shadow_Hemorridor",
        ShadowDungeon::ContinuousLoopofIdols => "Continuous_Loop_of_Idols",
        ShadowDungeon::NordicGods => "Shadow_Nordic",
        ShadowDungeon::MountOlympus => "Shadow_Mount_Olympus",
        ShadowDungeon::TavernOfTheDarkDoppelgangers => {
            "Shadow_Tavern_of_the_Dark_Doppelgangers"
        }
        ShadowDungeon::DragonsHoard => "Shadow_Dragons_Hoard",
        ShadowDungeon::HouseOfHorrors => "Shadow_House_of_Horrors",
        ShadowDungeon::ThirdLeagueofSuperheroes => {
            "Shadow_The_3rd_League_of_Superheroes"
        }
        ShadowDungeon::DojoOfChildhoodHeroes => {
            "Shadow_Dojo_of_Childhood_Heroes"
        }
        ShadowDungeon::MonsterGrotto => "Shadow_Monster_Grotto",
        ShadowDungeon::CityOfIntrigues => "Shadow_City_of_Intrigues",
        ShadowDungeon::SchoolOfMagicExpress => "Shadow_School_of_magic_Express",
        ShadowDungeon::AshMountain => "Shadow_Ash_Mountain",
        ShadowDungeon::PlayaGamesHQ => "Shadow_Playa_HQ",
        ShadowDungeon::ArcadeOfTheOldPixelIcons => {
            "Shadow_Arcade_of_the_Old_Pixel_Icons"
        }
        ShadowDungeon::TheServerRoom => "Shadow_The_Server_Room",
        ShadowDungeon::WorkshopOfTheHunters => {
            "Shadow_Workshop_of_the_Hunters_of_the_Undead"
        }
        ShadowDungeon::RetroTVLegends => "Shadow_Retro_TV_Legends",
        ShadowDungeon::MeetingRoom => "Shadow_The_Meeting_Room",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let _monsters = DungeonMonsters::parse();
    }
}
