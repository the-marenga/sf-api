use std::{collections::HashMap, env, fs, path::Path};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("dungeon_data.rs");

    let dungeons_json =
        fs::read_to_string("src/simulate/dungeons.json").unwrap();
    let pet_dungeons_json =
        fs::read_to_string("src/simulate/pet_dungeons.json").unwrap();

    println!("cargo:rerun-if-changed=src/simulate/dungeons.json");
    println!("cargo:rerun-if-changed=src/simulate/pet_dungeons.json");

    let mut code = String::new();

    let dungeon_data: HashMap<String, DungeonData> =
        serde_json::from_str(&dungeons_json).unwrap();

    let light_dungeons = [
        ("Desecrated_Catacombs", "LightDungeon::DesecratedCatacombs"),
        ("Mines_of_Gloria", "LightDungeon::MinesOfGloria"),
        ("Ruins_of_Gnark", "LightDungeon::RuinsOfGnark"),
        ("Cutthroat_Grotto", "LightDungeon::CutthroatGrotto"),
        ("Emerald_Scale_Altar", "LightDungeon::EmeraldScaleAltar"),
        ("Toxic_Tree", "LightDungeon::ToxicTree"),
        ("Magma_Stream", "LightDungeon::MagmaStream"),
        ("Frost_Blood_Temple", "LightDungeon::FrostBloodTemple"),
        ("Pyramids_of_Madness", "LightDungeon::PyramidsofMadness"),
        ("Black_Skull_Fortress", "LightDungeon::BlackSkullFortress"),
        ("Circus_of_Horror", "LightDungeon::CircusOfHorror"),
        ("Hell", "LightDungeon::Hell"),
        ("The_13th_Floor", "LightDungeon::The13thFloor"),
        ("Osteros", "LightDungeon::Easteros"),
        ("Tower", "LightDungeon::Tower"),
        (
            "Time_Honored_School_of_Magic",
            "LightDungeon::TimeHonoredSchoolofMagic",
        ),
        ("Hemorridor", "LightDungeon::Hemorridor"),
        ("Nordic", "LightDungeon::NordicGods"),
        ("Mount_Olympus", "LightDungeon::MountOlympus"),
        (
            "Tavern_of_the_Dark_Doppelgangers",
            "LightDungeon::TavernoftheDarkDoppelgangers",
        ),
        ("Dragons_Hoard", "LightDungeon::DragonsHoard"),
        ("House_of_Horrors", "LightDungeon::HouseOfHorrors"),
        (
            "The_3rd_League_of_Superheroes",
            "LightDungeon::ThirdLeagueOfSuperheroes",
        ),
        (
            "Dojo_of_Childhood_Heroes",
            "LightDungeon::DojoOfChildhoodHeroes",
        ),
        ("Monster_Grotto", "LightDungeon::MonsterGrotto"),
        ("City_of_Intrigues", "LightDungeon::CityOfIntrigues"),
        (
            "School_of_magic_Express",
            "LightDungeon::SchoolOfMagicExpress",
        ),
        ("Ash_Mountain", "LightDungeon::AshMountain"),
        ("Playa_HQ", "LightDungeon::PlayaGamesHQ"),
        ("Training_Camp", "LightDungeon::TrainingCamp"),
        ("Sandstorm", "LightDungeon::Sandstorm"),
        (
            "Arcade_of_the_Old_Pixel_Icons",
            "LightDungeon::ArcadeOfTheOldPixelIcons",
        ),
        ("The_Server_Room", "LightDungeon::TheServerRoom"),
        (
            "Workshop_of_the_Hunters_of_the_Undead",
            "LightDungeon::WorkshopOfTheHunters",
        ),
        ("Retro_TV_Legends", "LightDungeon::RetroTVLegends"),
        ("The_Meeting_Room", "LightDungeon::MeetingRoom"),
    ];

    let shadow_dungeons = [
        (
            "Shadow_Desecrated_Catacombs",
            "ShadowDungeon::DesecratedCatacombs",
        ),
        ("Shadow_Mines_of_Gloria", "ShadowDungeon::MinesOfGloria"),
        ("Shadow_Ruins_of_Gnark", "ShadowDungeon::RuinsOfGnark"),
        ("Shadow_Cutthroat_Grotto", "ShadowDungeon::CutthroatGrotto"),
        (
            "Shadow_Emerald_Scale_Altar",
            "ShadowDungeon::EmeraldScaleAltar",
        ),
        ("Shadow_Toxic_Tree", "ShadowDungeon::ToxicTree"),
        ("Shadow_Magma_Stream", "ShadowDungeon::MagmaStream"),
        (
            "Shadow_Frost_Blood_Temple",
            "ShadowDungeon::FrostBloodTemple",
        ),
        (
            "Shadow_Pyramids_of_Madness",
            "ShadowDungeon::PyramidsOfMadness",
        ),
        (
            "Shadow_Black_Skull_Fortress",
            "ShadowDungeon::BlackSkullFortress",
        ),
        ("Shadow_Circus_of_Horror", "ShadowDungeon::CircusOfHorror"),
        ("Shadow_Hell", "ShadowDungeon::Hell"),
        ("Shadow_The_13th_Floor", "ShadowDungeon::The13thFloor"),
        ("Shadow_Osteros", "ShadowDungeon::Easteros"),
        ("Twister", "ShadowDungeon::Twister"),
        (
            "Shadow_Time_Honored_School_of_Magic",
            "ShadowDungeon::TimeHonoredSchoolOfMagic",
        ),
        ("Shadow_Hemorridor", "ShadowDungeon::Hemorridor"),
        (
            "Continuous_Loop_of_Idols",
            "ShadowDungeon::ContinuousLoopofIdols",
        ),
        ("Shadow_Nordic", "ShadowDungeon::NordicGods"),
        ("Shadow_Mount_Olympus", "ShadowDungeon::MountOlympus"),
        (
            "Shadow_Tavern_of_the_Dark_Doppelgangers",
            "ShadowDungeon::TavernOfTheDarkDoppelgangers",
        ),
        ("Shadow_Dragons_Hoard", "ShadowDungeon::DragonsHoard"),
        ("Shadow_House_of_Horrors", "ShadowDungeon::HouseOfHorrors"),
        (
            "Shadow_The_3rd_League_of_Superheroes",
            "ShadowDungeon::ThirdLeagueofSuperheroes",
        ),
        (
            "Shadow_Dojo_of_Childhood_Heroes",
            "ShadowDungeon::DojoOfChildhoodHeroes",
        ),
        ("Shadow_Monster_Grotto", "ShadowDungeon::MonsterGrotto"),
        ("Shadow_City_of_Intrigues", "ShadowDungeon::CityOfIntrigues"),
        (
            "Shadow_School_of_magic_Express",
            "ShadowDungeon::SchoolOfMagicExpress",
        ),
        ("Shadow_Ash_Mountain", "ShadowDungeon::AshMountain"),
        ("Shadow_Playa_HQ", "ShadowDungeon::PlayaGamesHQ"),
        (
            "Shadow_Arcade_of_the_Old_Pixel_Icons",
            "ShadowDungeon::ArcadeOfTheOldPixelIcons",
        ),
        ("Shadow_The_Server_Room", "ShadowDungeon::TheServerRoom"),
        (
            "Shadow_Workshop_of_the_Hunters_of_the_Undead",
            "ShadowDungeon::WorkshopOfTheHunters",
        ),
        ("Shadow_Retro_TV_Legends", "ShadowDungeon::RetroTVLegends"),
        ("Shadow_The_Meeting_Room", "ShadowDungeon::MeetingRoom"),
    ];

    for (json_name, enum_variant) in
        light_dungeons.iter().chain(shadow_dungeons.iter())
    {
        if let Some(data) = dungeon_data.get(*json_name) {
            let var_name = json_name.to_uppercase();
            code.push_str(&format!(
                "static {}: [Monster; {}] = [\n",
                var_name, data.levels
            ));
            append_monsters(
                &mut code,
                data,
                json_name.starts_with("Shadow"),
                enum_variant,
            );
            code.push_str("];\n");
        }
    }

    // Pet monsters
    let pet_data: HashMap<String, Vec<PetDungeonEnemy>> =
        serde_json::from_str(&pet_dungeons_json).unwrap();
    for (habitat, monsters) in &pet_data {
        let var_name = format!("PET_{}", habitat.to_uppercase());
        code.push_str(&format!(
            "static {}: [Monster; {}] = [\n",
            var_name,
            monsters.len()
        ));
        for m in monsters {
            code.push_str("    Monster {\n");
            code.push_str(&format!("        name: {:?},\n", m.name));
            code.push_str(&format!("        level: {},\n", m.level));
            code.push_str(&format!("        class: Class::{:?},\n", m.class));
            code.push_str("        attributes: EnumMap::from_array([\n");
            code.push_str(&format!("            {},\n", m.strength));
            code.push_str(&format!("            {},\n", m.dexterity));
            code.push_str(&format!("            {},\n", m.intelligence));
            code.push_str(&format!("            {},\n", m.constitution));
            code.push_str(&format!("            {},\n", m.luck));
            code.push_str("        ]),\n");
            code.push_str(&format!("        hp: {},\n", m.life));
            code.push_str("        min_dmg: 0,\n");
            code.push_str("        max_dmg: 0,\n");
            code.push_str("        armor: 0,\n");
            code.push_str("        runes: None,\n");
            code.push_str("    },\n");
        }
        code.push_str("];\n");
    }

    code.push_str(
        "\npub static DUNGEON_MONSTERS_LIGHT: EnumMap<LightDungeon, &'static \
         [Monster]> = EnumMap::from_array([\n",
    );
    let light_ordered = [
        "DESECRATED_CATACOMBS",
        "MINES_OF_GLORIA",
        "RUINS_OF_GNARK",
        "CUTTHROAT_GROTTO",
        "EMERALD_SCALE_ALTAR",
        "TOXIC_TREE",
        "MAGMA_STREAM",
        "FROST_BLOOD_TEMPLE",
        "PYRAMIDS_OF_MADNESS",
        "BLACK_SKULL_FORTRESS",
        "CIRCUS_OF_HORROR",
        "HELL",
        "THE_13TH_FLOOR",
        "OSTEROS",
        "TOWER",
        "TIME_HONORED_SCHOOL_OF_MAGIC",
        "HEMORRIDOR",
        "NORDIC",
        "MOUNT_OLYMPUS",
        "TAVERN_OF_THE_DARK_DOPPELGANGERS",
        "DRAGONS_HOARD",
        "HOUSE_OF_HORRORS",
        "THE_3RD_LEAGUE_OF_SUPERHEROES",
        "DOJO_OF_CHILDHOOD_HEROES",
        "MONSTER_GROTTO",
        "CITY_OF_INTRIGUES",
        "SCHOOL_OF_MAGIC_EXPRESS",
        "ASH_MOUNTAIN",
        "PLAYA_HQ",
        "TRAINING_CAMP",
        "SANDSTORM",
        "ARCADE_OF_THE_OLD_PIXEL_ICONS",
        "THE_SERVER_ROOM",
        "WORKSHOP_OF_THE_HUNTERS_OF_THE_UNDEAD",
        "RETRO_TV_LEGENDS",
        "THE_MEETING_ROOM",
    ];

    for name in light_ordered {
        code.push_str(&format!("    &{} as &'static [Monster],\n", name));
    }
    code.push_str("]);\n\n");

    code.push_str(
        "pub static DUNGEON_MONSTERS_SHADOW: EnumMap<ShadowDungeon, &'static \
         [Monster]> = EnumMap::from_array([\n",
    );
    let shadow_ordered = [
        "SHADOW_DESECRATED_CATACOMBS",
        "SHADOW_MINES_OF_GLORIA",
        "SHADOW_RUINS_OF_GNARK",
        "SHADOW_CUTTHROAT_GROTTO",
        "SHADOW_EMERALD_SCALE_ALTAR",
        "SHADOW_TOXIC_TREE",
        "SHADOW_MAGMA_STREAM",
        "SHADOW_FROST_BLOOD_TEMPLE",
        "SHADOW_PYRAMIDS_OF_MADNESS",
        "SHADOW_BLACK_SKULL_FORTRESS",
        "SHADOW_CIRCUS_OF_HORROR",
        "SHADOW_HELL",
        "SHADOW_THE_13TH_FLOOR",
        "SHADOW_OSTEROS",
        "TWISTER",
        "SHADOW_TIME_HONORED_SCHOOL_OF_MAGIC",
        "SHADOW_HEMORRIDOR",
        "CONTINUOUS_LOOP_OF_IDOLS",
        "SHADOW_NORDIC",
        "SHADOW_MOUNT_OLYMPUS",
        "SHADOW_TAVERN_OF_THE_DARK_DOPPELGANGERS",
        "SHADOW_DRAGONS_HOARD",
        "SHADOW_HOUSE_OF_HORRORS",
        "SHADOW_THE_3RD_LEAGUE_OF_SUPERHEROES",
        "SHADOW_DOJO_OF_CHILDHOOD_HEROES",
        "SHADOW_MONSTER_GROTTO",
        "SHADOW_CITY_OF_INTRIGUES",
        "SHADOW_SCHOOL_OF_MAGIC_EXPRESS",
        "SHADOW_ASH_MOUNTAIN",
        "SHADOW_PLAYA_HQ",
        "SHADOW_ARCADE_OF_THE_OLD_PIXEL_ICONS",
        "SHADOW_THE_SERVER_ROOM",
        "SHADOW_WORKSHOP_OF_THE_HUNTERS_OF_THE_UNDEAD",
        "SHADOW_RETRO_TV_LEGENDS",
        "SHADOW_THE_MEETING_ROOM",
    ];
    for name in shadow_ordered {
        code.push_str(&format!("    &{} as &'static [Monster],\n", name));
    }
    code.push_str("]);\n\n");

    code.push_str(
        "pub static PET_MONSTERS: [(HabitatType, &[Monster]); 5] = [\n",
    );
    for habitat in ["Shadow", "Light", "Earth", "Fire", "Water"] {
        code.push_str(&format!(
            "    (HabitatType::{}, &PET_{} as &[Monster]),\n",
            habitat,
            habitat.to_uppercase()
        ));
    }
    code.push_str("];\n");

    fs::write(dest_path, code).unwrap();
}

fn append_monsters(
    code: &mut String,
    data: &DungeonData,
    is_shadow: bool,
    enum_variant: &str,
) {
    let mut monsters_count = 0;
    for (idx, m) in data.monsters.iter().enumerate() {
        let class = m.class.as_deref().unwrap_or("Warrior");
        let class_enum = match class {
            "Warrior" => "Class::Warrior",
            "Mage" => "Class::Mage",
            "Scout" => "Class::Scout",
            "Assassin" => "Class::Assassin",
            "WarMage" | "BattleMage" => "Class::BattleMage",
            "Berserker" => "Class::Berserker",
            "DemonHunter" => "Class::DemonHunter",
            "Druid" => "Class::Druid",
            "Bard" => "Class::Bard",
            "Necromancer" => "Class::Necromancer",
            "Paladin" => "Class::Paladin",
            "PlagueDoctor" => "Class::PlagueDoctor",
            _ => panic!("Unknown class: {}", class),
        };
        let level = m.level.unwrap_or(u16::MAX);

        let armor = m.armor.unwrap_or_else(|| {
            let res = (level as u32) * max_armor_reduction(class);
            if !is_shadow
                && enum_variant.contains("TavernoftheDarkDoppelgangers")
            {
                res / 2
            } else if !is_shadow && enum_variant.contains("Tower") {
                ((res as f64) * 1.5) as u32
            } else {
                res
            }
        });

        code.push_str("    Monster {\n");
        let name =
            m.name
                .as_ref()
                .map(|n| n.replace('_', " "))
                .unwrap_or_else(|| {
                    if is_shadow {
                        format!(
                            "Shadow {} monster #{}",
                            enum_variant.split("::").last().unwrap(),
                            idx
                        )
                    } else {
                        format!(
                            "{} monster #{}",
                            enum_variant.split("::").last().unwrap(),
                            idx
                        )
                    }
                });
        code.push_str(&format!("        name: {:?},\n", name));
        code.push_str(&format!("        level: {},\n", level));
        code.push_str(&format!("        class: {},\n", class_enum));
        code.push_str("        attributes: EnumMap::from_array([\n");
        code.push_str(&format!(
            "            {},\n",
            m.strength.unwrap_or(u32::MAX)
        ));
        code.push_str(&format!(
            "            {},\n",
            m.dexterity.unwrap_or(u32::MAX)
        ));
        code.push_str(&format!(
            "            {},\n",
            m.intelligence.unwrap_or(u32::MAX)
        ));
        code.push_str(&format!(
            "            {},\n",
            m.constitution.unwrap_or(u32::MAX)
        ));
        code.push_str(&format!(
            "            {},\n",
            m.luck.unwrap_or(u32::MAX)
        ));
        code.push_str("        ]),\n");
        code.push_str(&format!(
            "        hp: {},\n",
            m.life.unwrap_or(u64::MAX)
        ));
        code.push_str(&format!(
            "        min_dmg: {},\n",
            m.min_dmg.unwrap_or(0)
        ));
        code.push_str(&format!(
            "        max_dmg: {},\n",
            m.max_dmg.unwrap_or(0)
        ));
        code.push_str(&format!("        armor: {},\n", armor));

        if let Some(runes) = &m.runes {
            let typ = match runes.typ {
                40 => "Element::Fire",
                41 => "Element::Cold",
                42 => "Element::Lightning",
                _ => panic!("Unknown rune type: {}", runes.typ),
            };
            code.push_str("        runes: Some(MonsterRunes {\n");
            code.push_str(&format!("            damage_type: {},\n", typ));
            code.push_str(&format!(
                "                damage: {},\n",
                runes.damage
            ));
            code.push_str(&format!(
                "                resistances: EnumMap::from_array([{}, {}, \
                 {}]),\n",
                runes.res[0], runes.res[1], runes.res[2]
            ));
            code.push_str("        }),\n");
        } else {
            code.push_str("        runes: None,\n");
        }

        code.push_str("    },\n");
        monsters_count += 1;
    }

    while monsters_count < data.levels {
        let name = if is_shadow {
            format!(
                "Shadow {} monster #{}",
                enum_variant.split("::").last().unwrap(),
                monsters_count
            )
        } else {
            format!(
                "{} monster #{}",
                enum_variant.split("::").last().unwrap(),
                monsters_count
            )
        };
        code.push_str("    Monster {\n");
        code.push_str(&format!("        name: {:?},\n", name));
        code.push_str("        level: u16::MAX,\n");
        code.push_str("        class: Class::Assassin,\n");
        code.push_str(
            "        attributes: EnumMap::from_array([u32::MAX; 5]),\n",
        );
        code.push_str("        hp: u64::MAX,\n");
        code.push_str("        min_dmg: u32::MAX,\n");
        code.push_str("        max_dmg: u32::MAX,\n");
        code.push_str("        armor: u32::MAX,\n");
        code.push_str("        runes: None,\n");
        code.push_str("    },\n");
        monsters_count += 1;
    }
}

fn max_armor_reduction(class: &str) -> u32 {
    match class {
        "Warrior" | "Berserker" | "Paladin" => 50,
        "Scout" | "Assassin" | "DemonHunter" | "Bard" => 25,
        "Mage" | "BattleMage" | "Druid" | "Necromancer" | "PlagueDoctor" => 10,
        _ => 50, // Default
    }
}

#[derive(serde::Deserialize)]
struct DungeonData {
    levels: usize,
    monsters: Vec<MonsterData>,
}

#[derive(serde::Deserialize)]
struct MonsterData {
    name: Option<String>,
    class: Option<String>,
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

#[derive(serde::Deserialize)]
struct RawMonsterRunes {
    #[serde(rename = "type")]
    typ: u32,
    res: [i32; 3],
    damage: i32,
}

#[derive(serde::Deserialize)]
struct PetDungeonEnemy {
    pub name: String,
    pub class: PetClass,
    pub level: u16,
    pub strength: u32,
    pub dexterity: u32,
    pub intelligence: u32,
    pub constitution: u32,
    pub luck: u32,
    pub life: u64,
}

#[derive(serde::Deserialize, Debug)]
enum PetClass {
    Warrior,
    Mage,
    Scout,
    Assassin,
    BattleMage,
    Berserker,
    DemonHunter,
    Druid,
    Bard,
    Necromancer,
    Paladin,
    PlagueDoctor,
}
