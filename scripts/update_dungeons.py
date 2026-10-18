#!/usr/bin/env python3
"""
update_dungeons.py - Updates src/simulate/dungeons.json from sf-tools sources.

Sources:
- Dungeons data: https://raw.githubusercontent.com/HafisCZ/sf-tools/gh-pages/js/sim/data/dungeons.js
- Monsters data: https://raw.githubusercontent.com/HafisCZ/sf-tools/gh-pages/js/playa/lang/en/monsters.json

Outputs standardized format:
{
  "light": {
    "<LightDungeonVariant>": { "levels": N, "monsters": [ ... ] }
  },
  "shadow": {
    "<ShadowDungeonVariant>": { "levels": N, "monsters": [ ... ] }
  }
}
"""

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.request

DEFAULT_BRANCH = "gh-pages"

def make_dungeons_url(branch: str) -> str:
    return f"https://raw.githubusercontent.com/HafisCZ/sf-tools/{branch}/js/sim/data/dungeons.js"

def make_monsters_url(branch: str) -> str:
    return f"https://raw.githubusercontent.com/HafisCZ/sf-tools/{branch}/js/playa/lang/en/monsters.json"


CLASS_MAP = {
    1: "Warrior",
    2: "Mage",
    3: "Scout",
    4: "Assassin",
    5: "BattleMage",
    6: "Berserker",
    7: "DemonHunter",
    8: "Druid",
    9: "Bard",
    10: "Necromancer",
    11: "Paladin",
    12: "PlagueDoctor",
    13: "BloodWeaver",
}

# Ordered exactly matching LightDungeon enum definition in sf-api
LIGHT_DUNGEONS_ORDER = [
    ("DesecratedCatacombs", "1"),
    ("MinesOfGloria", "2"),
    ("RuinsOfGnark", "3"),
    ("CutthroatGrotto", "4"),
    ("EmeraldScaleAltar", "5"),
    ("ToxicTree", "6"),
    ("MagmaStream", "7"),
    ("FrostBloodTemple", "8"),
    ("PyramidsofMadness", "9"),
    ("BlackSkullFortress", "10"),
    ("CircusOfHorror", "11"),
    ("Hell", "12"),
    ("The13thFloor", "13"),
    ("Easteros", "14"),
    ("Tower", "201"),
    ("TimeHonoredSchoolofMagic", "16"),
    ("Hemorridor", "17"),
    ("NordicGods", "19"),
    ("MountOlympus", "20"),
    ("TavernoftheDarkDoppelgangers", "21"),
    ("DragonsHoard", "22"),
    ("HouseOfHorrors", "23"),
    ("ThirdLeagueOfSuperheroes", "24"),
    ("DojoOfChildhoodHeroes", "25"),
    ("MonsterGrotto", "26"),
    ("CityOfIntrigues", "27"),
    ("SchoolOfMagicExpress", "28"),
    ("AshMountain", "29"),
    ("PlayaGamesHQ", "30"),
    ("TrainingCamp", None),  # Preserved from existing data
    ("Sandstorm", "204"),
    ("ArcadeOfTheOldPixelIcons", "31"),
    ("TheServerRoom", "32"),
    ("WorkshopOfTheHunters", "33"),
    ("RetroTVLegends", "34"),
    ("MeetingRoom", "35"),
]

# Ordered exactly matching ShadowDungeon enum definition in sf-api
SHADOW_DUNGEONS_ORDER = [
    ("DesecratedCatacombs", "101"),
    ("MinesOfGloria", "102"),
    ("RuinsOfGnark", "103"),
    ("CutthroatGrotto", "104"),
    ("EmeraldScaleAltar", "105"),
    ("ToxicTree", "106"),
    ("MagmaStream", "107"),
    ("FrostBloodTemple", "108"),
    ("PyramidsOfMadness", "109"),
    ("BlackSkullFortress", "110"),
    ("CircusOfHorror", "111"),
    ("Hell", "112"),
    ("The13thFloor", "113"),
    ("Easteros", "114"),
    ("Twister", "203"),
    ("TimeHonoredSchoolOfMagic", "116"),
    ("Hemorridor", "117"),
    ("ContinuousLoopofIdols", "202"),
    ("NordicGods", "119"),
    ("MountOlympus", "120"),
    ("TavernOfTheDarkDoppelgangers", "121"),
    ("DragonsHoard", "122"),
    ("HouseOfHorrors", "123"),
    ("ThirdLeagueofSuperheroes", "124"),
    ("DojoOfChildhoodHeroes", "125"),
    ("MonsterGrotto", "126"),
    ("CityOfIntrigues", "127"),
    ("SchoolOfMagicExpress", "128"),
    ("AshMountain", "129"),
    ("PlayaGamesHQ", "130"),
    ("ArcadeOfTheOldPixelIcons", "131"),
    ("TheServerRoom", "132"),
    ("WorkshopOfTheHunters", "133"),
    ("RetroTVLegends", "134"),
    ("MeetingRoom", "135"),
]

# Fallback default for Training Camp if not present in existing json
DEFAULT_TRAINING_CAMP = {
    "levels": 10,
    "monsters": [
        {"class": "Mage", "constitution": 10, "dexterity": 5, "id": 1, "intelligence": 10, "level": 1, "life": 50, "luck": 5, "name": "Cardboard Clark", "strength": 5},
        {"class": "Mage", "constitution": 20, "dexterity": 10, "id": 2, "intelligence": 20, "level": 2, "life": 120, "luck": 10, "name": "Scarecrow Steve", "strength": 10},
        {"class": "Scout", "constitution": 30, "dexterity": 30, "id": 3, "intelligence": 15, "level": 3, "life": 480, "luck": 15, "name": "Henry Hobbyhorse", "strength": 15},
        {"class": "Warrior", "constitution": 40, "dexterity": 20, "id": 4, "intelligence": 20, "level": 4, "life": 1000, "luck": 20, "name": "Broken Brian", "strength": 40},
        {"class": "Warrior", "constitution": 50, "dexterity": 25, "id": 5, "intelligence": 25, "level": 5, "life": 1500, "luck": 25, "name": "Tin Can Bob", "strength": 50},
        {"class": "Mage", "constitution": 60, "dexterity": 30, "id": 6, "intelligence": 60, "level": 6, "life": 840, "luck": 30, "name": "Flaming Fred", "strength": 30},
        {"class": "Scout", "constitution": 70, "dexterity": 70, "id": 7, "intelligence": 35, "level": 7, "life": 2240, "luck": 35, "name": "Flying Francis", "strength": 35},
        {"class": "Warrior", "constitution": 80, "dexterity": 40, "id": 8, "intelligence": 40, "level": 8, "life": 3600, "luck": 40, "name": "Dan the Dummy", "strength": 80},
        {"class": "Scout", "constitution": 90, "dexterity": 80, "id": 9, "intelligence": 40, "level": 9, "life": 3600, "luck": 45, "name": "Sarah Saw Blade", "strength": 40},
        {"class": "Mage", "constitution": 100, "dexterity": 40, "id": 10, "intelligence": 80, "level": 10, "life": 2200, "luck": 50, "name": "All-purpose Andy", "strength": 40},
    ],
}


def parse_dungeons_js(js_content: str) -> dict:
    """Parses DUNGEON_DATA from dungeons.js using node if available, or regex fallback."""
    try:
        proc = subprocess.run(
            [
                "node",
                "-e",
                "const fs = require('fs');\n"
                "const code = process.argv[1];\n"
                "eval(code.replace('const DUNGEON_DATA', 'global.DUNGEON_DATA'));\n"
                "process.stdout.write(JSON.stringify(global.DUNGEON_DATA));",
                js_content,
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        return json.loads(proc.stdout)
    except Exception:
        pass

    # Pure Python fallback
    clean = re.sub(r"/\*.*?\*/", "", js_content, flags=re.DOTALL)
    clean = re.sub(r"//.*", "", clean)
    start = clean.find("{")
    end = clean.rfind("}")
    if start == -1 or end == -1:
        raise ValueError("Could not find JavaScript object in dungeons.js")
    content = clean[start : end + 1]

    def repl_str(m):
        return '"' + m.group(1).replace('"', '\\"') + '"'

    content = re.sub(r"'([^'\\]*(?:\\.[^'\\]*)*)'", repl_str, content)
    content = re.sub(r"([{,]\s*)([a-zA-Z_]\w*)\s*:", r'\1"\2":', content)
    content = re.sub(r",\s*([}\]])", r"\1", content)
    return json.loads(content)


def load_content(path_or_url: str) -> str:
    if os.path.exists(path_or_url):
        with open(path_or_url, "r", encoding="utf-8") as f:
            return f.read()
    req = urllib.request.Request(
        path_or_url, headers={"User-Agent": "sf-api-updater"}
    )
    with urllib.request.urlopen(req) as resp:
        return resp.read().decode("utf-8")


def convert_monster(floor_data: dict, monster_name: str, pos: int) -> dict:
    monster = {
        "id": pos,
        "name": monster_name,
    }
    raw_class = floor_data.get("class")
    if raw_class is not None:
        monster["class"] = CLASS_MAP.get(raw_class, "Warrior")
    if "level" in floor_data and floor_data["level"] is not None:
        monster["level"] = floor_data["level"]
    if "str" in floor_data and floor_data["str"] is not None:
        monster["strength"] = floor_data["str"]
    if "dex" in floor_data and floor_data["dex"] is not None:
        monster["dexterity"] = floor_data["dex"]
    if "int" in floor_data and floor_data["int"] is not None:
        monster["intelligence"] = floor_data["int"]
    if "con" in floor_data and floor_data["con"] is not None:
        monster["constitution"] = floor_data["con"]
    if "lck" in floor_data and floor_data["lck"] is not None:
        monster["luck"] = floor_data["lck"]
    if "health" in floor_data and floor_data["health"] is not None:
        monster["life"] = floor_data["health"]
    if "min" in floor_data and floor_data["min"] is not None:
        monster["min_dmg"] = floor_data["min"]
    if "max" in floor_data and floor_data["max"] is not None:
        monster["max_dmg"] = floor_data["max"]
    if "armor" in floor_data and floor_data["armor"] is not None:
        monster["armor"] = floor_data["armor"]
    if "runes" in floor_data and floor_data["runes"] is not None:
        monster["runes"] = floor_data["runes"]

    return monster


def get_existing_training_camp(candidate_paths: list[str]) -> dict:
    """Reads existing Training Camp from candidates or falls back to default."""
    for path in candidate_paths:
        if not path or not os.path.exists(path):
            continue
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
            camp = None
            if "light" in data and "TrainingCamp" in data["light"]:
                camp = data["light"]["TrainingCamp"]
            elif "Training_Camp" in data:
                camp = data["Training_Camp"]

            if camp:
                monsters = []
                for idx, m in enumerate(camp.get("monsters", [])):
                    m_copy = dict(m)
                    # Normalize id to 1-based pos if needed
                    m_copy["id"] = idx + 1
                    if "name" in m_copy and m_copy["name"]:
                        m_copy["name"] = m_copy["name"].replace("_", " ")
                    monsters.append(m_copy)
                return {"levels": camp.get("levels", len(monsters)), "monsters": monsters}
        except Exception as e:
            print(f"Warning: could not read Training Camp from {path}: {e}", file=sys.stderr)
    return DEFAULT_TRAINING_CAMP


def main():
    parser = argparse.ArgumentParser(description="Update sf-api dungeons.json")
    parser.add_argument(
        "--branch",
        default=DEFAULT_BRANCH,
        help=f"Branch to pull from on HafisCZ/sf-tools (default: {DEFAULT_BRANCH})",
    )
    parser.add_argument(
        "--dungeons-src",
        default=None,
        help="Path or URL to dungeons.js (overrides --branch)",
    )
    parser.add_argument(
        "--monsters-src",
        default=None,
        help="Path or URL to monsters.json (overrides --branch)",
    )
    parser.add_argument(
        "--output",
        default="src/simulate/dungeons.json",
        help="Output path for dungeons.json",
    )
    parser.add_argument(
        "--training-camp-src",
        default="src/simulate/dungeons.json",
        help="Path to existing dungeons.json to extract Training Camp from",
    )
    args = parser.parse_args()

    dungeons_src = args.dungeons_src or make_dungeons_url(args.branch)
    monsters_src = args.monsters_src or make_monsters_url(args.branch)

    print(f"Loading dungeons from {dungeons_src}...")
    dungeons_js_raw = load_content(dungeons_src)
    dungeon_data = parse_dungeons_js(dungeons_js_raw)

    print(f"Loading monsters from {monsters_src}...")
    monsters_raw = load_content(monsters_src)

    monsters_map = json.loads(monsters_raw)

    result = {
        "light": {},
        "shadow": {},
    }

    # Process Light Dungeons in defined enum order
    for variant_name, js_key in LIGHT_DUNGEONS_ORDER:
        if js_key is None:
            # Training Camp
            training_camp = get_existing_training_camp([args.training_camp_src, args.output])
            result["light"][variant_name] = training_camp
            continue

        if js_key not in dungeon_data:
            print(f"Warning: Light dungeon key '{js_key}' not in source data", file=sys.stderr)
            continue

        d_info = dungeon_data[js_key]
        floors = d_info.get("floors", {})
        floor_keys = sorted(floors.keys(), key=lambda x: int(x))
        monsters = []
        for fk in floor_keys:
            pos = int(fk)
            fdata = floors[fk]
            enemy_id = fdata.get("id")
            if enemy_id is not None and str(enemy_id) in monsters_map:
                name = monsters_map[str(enemy_id)]
            elif js_key == "9" and pos == 10:
                name = "Yourself"
            else:
                name = f"{variant_name} #{pos}"
            monsters.append(convert_monster(fdata, name, pos))

        result["light"][variant_name] = {
            "levels": len(monsters),
            "monsters": monsters,
        }

    # Process Shadow Dungeons in defined enum order
    for variant_name, js_key in SHADOW_DUNGEONS_ORDER:
        if js_key not in dungeon_data:
            print(f"Warning: Shadow dungeon key '{js_key}' not in source data", file=sys.stderr)
            continue

        d_info = dungeon_data[js_key]
        floors = d_info.get("floors", {})
        floor_keys = sorted(floors.keys(), key=lambda x: int(x))
        monsters = []
        for fk in floor_keys:
            pos = int(fk)
            fdata = floors[fk]
            enemy_id = fdata.get("id")

            # Keys 134 and 135 in sf-tools use enemy id 1..10, so borrow names from Light 34 and 35
            if js_key == "134" and "34" in dungeon_data and fk in dungeon_data["34"].get("floors", {}):
                light_enemy_id = dungeon_data["34"]["floors"][fk].get("id")
                name = monsters_map.get(str(light_enemy_id), f"{variant_name} #{pos}")
            elif js_key == "135" and "35" in dungeon_data and fk in dungeon_data["35"].get("floors", {}):
                light_enemy_id = dungeon_data["35"]["floors"][fk].get("id")
                name = monsters_map.get(str(light_enemy_id), f"{variant_name} #{pos}")
            elif enemy_id is not None and str(enemy_id) in monsters_map:
                name = monsters_map[str(enemy_id)]
            elif js_key == "109" and pos == 10:
                name = "Yourself"
            else:
                name = f"{variant_name} #{pos}"
            monsters.append(convert_monster(fdata, name, pos))

        result["shadow"][variant_name] = {
            "levels": len(monsters),
            "monsters": monsters,
        }

    os.makedirs(os.path.dirname(os.path.abspath(args.output)), exist_ok=True)
    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2, ensure_ascii=False)
        f.write("\n")

    print(
        f"Successfully generated {args.output} with {len(result['light'])} light and {len(result['shadow'])} shadow dungeons."
    )


if __name__ == "__main__":
    main()
