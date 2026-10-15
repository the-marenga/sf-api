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

    let dungeon_data: AllDungeons =
        serde_json::from_str(&dungeons_json).unwrap();

    // Ordered matching LightDungeon enum definition in src/gamestate/dungeons.rs
    const LIGHT_DUNGEONS: &[&str] = &[
        "DesecratedCatacombs",
        "MinesOfGloria",
        "RuinsOfGnark",
        "CutthroatGrotto",
        "EmeraldScaleAltar",
        "ToxicTree",
        "MagmaStream",
        "FrostBloodTemple",
        "PyramidsofMadness",
        "BlackSkullFortress",
        "CircusOfHorror",
        "Hell",
        "The13thFloor",
        "Easteros",
        "Tower",
        "TimeHonoredSchoolofMagic",
        "Hemorridor",
        "NordicGods",
        "MountOlympus",
        "TavernoftheDarkDoppelgangers",
        "DragonsHoard",
        "HouseOfHorrors",
        "ThirdLeagueOfSuperheroes",
        "DojoOfChildhoodHeroes",
        "MonsterGrotto",
        "CityOfIntrigues",
        "SchoolOfMagicExpress",
        "AshMountain",
        "PlayaGamesHQ",
        "TrainingCamp",
        "Sandstorm",
        "ArcadeOfTheOldPixelIcons",
        "TheServerRoom",
        "WorkshopOfTheHunters",
        "RetroTVLegends",
        "MeetingRoom",
    ];

    // Ordered matching ShadowDungeon enum definition in src/gamestate/dungeons.rs
    const SHADOW_DUNGEONS: &[&str] = &[
        "DesecratedCatacombs",
        "MinesOfGloria",
        "RuinsOfGnark",
        "CutthroatGrotto",
        "EmeraldScaleAltar",
        "ToxicTree",
        "MagmaStream",
        "FrostBloodTemple",
        "PyramidsOfMadness",
        "BlackSkullFortress",
        "CircusOfHorror",
        "Hell",
        "The13thFloor",
        "Easteros",
        "Twister",
        "TimeHonoredSchoolOfMagic",
        "Hemorridor",
        "ContinuousLoopofIdols",
        "NordicGods",
        "MountOlympus",
        "TavernOfTheDarkDoppelgangers",
        "DragonsHoard",
        "HouseOfHorrors",
        "ThirdLeagueofSuperheroes",
        "DojoOfChildhoodHeroes",
        "MonsterGrotto",
        "CityOfIntrigues",
        "SchoolOfMagicExpress",
        "AshMountain",
        "PlayaGamesHQ",
        "ArcadeOfTheOldPixelIcons",
        "TheServerRoom",
        "WorkshopOfTheHunters",
        "RetroTVLegends",
        "MeetingRoom",
    ];

    for name in LIGHT_DUNGEONS {
        let data = dungeon_data
            .light
            .get(*name)
            .unwrap_or_else(|| panic!("Missing light dungeon in dungeons.json: {name}"));
        let var_name = format!("LIGHT_{}", name.to_uppercase());
        code.push_str(&format!(
            "static {}: [Monster; {}] = [\n",
            var_name, data.levels
        ));
        append_monsters(
            &mut code,
            data,
            false,
            &format!("LightDungeon::{name}"),
        );
        code.push_str("];\n");
    }

    for name in SHADOW_DUNGEONS {
        let data = dungeon_data
            .shadow
            .get(*name)
            .unwrap_or_else(|| panic!("Missing shadow dungeon in dungeons.json: {name}"));
        let var_name = format!("SHADOW_{}", name.to_uppercase());
        code.push_str(&format!(
            "static {}: [Monster; {}] = [\n",
            var_name, data.levels
        ));
        append_monsters(
            &mut code,
            data,
            true,
            &format!("ShadowDungeon::{name}"),
        );
        code.push_str("];\n");
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
    for name in LIGHT_DUNGEONS {
        code.push_str(&format!(
            "    &LIGHT_{} as &'static [Monster],\n",
            name.to_uppercase()
        ));
    }
    code.push_str("]);\n\n");

    code.push_str(
        "pub static DUNGEON_MONSTERS_SHADOW: EnumMap<ShadowDungeon, &'static \
         [Monster]> = EnumMap::from_array([\n",
    );
    for name in SHADOW_DUNGEONS {
        code.push_str(&format!(
            "    &SHADOW_{} as &'static [Monster],\n",
            name.to_uppercase()
        ));
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
struct AllDungeons {
    light: HashMap<String, DungeonData>,
    shadow: HashMap<String, DungeonData>,
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
