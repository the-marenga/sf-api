use criterion::{Criterion, criterion_group, criterion_main};
use enum_map::EnumMap;
use sf_api::{
    command::AttributeType,
    gamestate::{character::Class, dungeons::*, items::*},
    simulate::*,
};

fn battle_benchmark(c: &mut Criterion) {
    let cases = vec![(
        "warrior_nordic",
        Class::Warrior,
        Dungeon::from(ShadowDungeon::NordicGods),
        6,
    )];

    let mut group = c.benchmark_group("battle_simulation");
    group.sample_size(10); // Reduce sample size as these might be long running

    for (name, class, dungeon, finished) in cases {
        group.bench_function(name, |b| {
            let progress = DungeonProgress::Open {
                finished: finished - 1,
            };

            let monster =
                Fighter::from(get_dungeon_monster(dungeon, progress).unwrap());

            let squad = init_squad(class, true);
            let mut player_side = if dungeon.is_with_companions() {
                squad
                    .companions
                    .map(|a| a.values().map(Fighter::from).collect())
                    .unwrap_or_default()
            } else {
                vec![]
            };
            player_side.push(Fighter::from(&squad.character));

            let monster_side = vec![monster];

            b.iter(|| {
                simulate_battle(&player_side, &monster_side, 10_000, false)
            })
        });
    }
    group.finish();
}

criterion_group!(benches, battle_benchmark);
criterion_main!(benches);

#[allow(non_snake_case)]
fn init_squad(class: Class, init_companions: bool) -> PlayerFighterSquad {
    let mut account = create_fighter(class, false);

    let companions = if init_companions {
        let comps = EnumMap::from_fn(|companion_class: CompanionClass| {
            let mut companion =
                create_fighter(Class::from(companion_class), true);

            // Copy equipment from account
            for slot in [
                EquipmentSlot::Hat,
                EquipmentSlot::BreastPlate,
                EquipmentSlot::Gloves,
                EquipmentSlot::FootWear,
            ] {
                companion.equipment.0[slot] = account.equipment.0[slot].clone();
            }

            let companion_config = companion.class;
            let armor = (f64::from(companion_config.max_armor_reduction())
                * 100.0
                * f64::from(account.level)
                / companion_config.armor_multiplier()
                * 2.0) as u32;

            companion.equipment.0[EquipmentSlot::Belt] = Some(Item {
                typ: ItemType::Belt,
                type_specific_val: armor,
                // Defaults
                model_id: 1,
                price: 0,
                mushroom_price: 0,
                class: Some(companion.class),
                attributes: EnumMap::default(),
                gem_slot: None,
                rune: None,
                enchantment: None,
                color: 1,
                upgrade_count: 0,
                item_quality: 0,
                is_washed: false,
            });

            companion
        });
        Some(comps)
    } else {
        None
    };

    let armor = (f64::from(class.max_armor_reduction())
        * 100.0
        * f64::from(account.level)
        / class.armor_multiplier()
        * 2.0) as u32;

    if let Some(belt) = &mut account.equipment.0[EquipmentSlot::Hat] {
        belt.type_specific_val = armor;
    }

    PlayerFighterSquad {
        character: account,
        companions,
    }
}

fn create_fighter(class: Class, is_companion: bool) -> UpgradeableFighter {
    let mut attribute_basis = EnumMap::default();
    let dmg_attrs = [
        AttributeType::Strength,
        AttributeType::Dexterity,
        AttributeType::Intelligence,
    ];

    for attr in dmg_attrs {
        if class.main_attribute() == attr {
            attribute_basis[attr] = 100_000;
        } else {
            attribute_basis[attr] = 20_000;
        }
    }
    attribute_basis[AttributeType::Constitution] = 100_000;
    attribute_basis[AttributeType::Luck] = 30_000;

    let mut equipment = Equipment::default();

    equipment.0[EquipmentSlot::Hat] = Some(create_rune_item(
        ItemType::Hat,
        RuneType::FireResistance,
        75,
    ));
    equipment.0[EquipmentSlot::BreastPlate] = Some(create_rune_item(
        ItemType::BreastPlate,
        RuneType::ColdResistence,
        75,
    ));
    equipment.0[EquipmentSlot::Gloves] = Some(create_rune_item(
        ItemType::Gloves,
        RuneType::LightningResistance,
        75,
    ));
    equipment.0[EquipmentSlot::FootWear] = Some(create_rune_item(
        ItemType::FootWear,
        RuneType::ExtraHitPoints,
        15,
    ));

    // Weapon
    let multiplier = class.weapon_multiplier();
    let min_dmg = (700.0 / 2.0 * multiplier) as u32;
    let max_dmg = (2100.0 / 2.0 * multiplier) as u32;

    let weapon = Item {
        typ: ItemType::Weapon { min_dmg, max_dmg },
        rune: Some(Rune {
            typ: RuneType::FireDamage,
            value: 60,
        }),
        enchantment: Some(Enchantment::SwordOfVengeance),
        // Defaults
        model_id: 1,
        price: 0,
        mushroom_price: 0,
        class: Some(class),
        type_specific_val: 0,
        attributes: EnumMap::default(),
        gem_slot: None,
        color: 1,
        upgrade_count: 0,
        item_quality: 0,
        is_washed: false,
    };

    if class == Class::Assassin {
        equipment.0[EquipmentSlot::Shield] = Some(weapon.clone());
    }
    equipment.0[EquipmentSlot::Weapon] = Some(weapon);

    let active_potions = [
        Some(Potion {
            typ: PotionType::EternalLife,
            size: PotionSize::Large,
            expires: None,
        }),
        None,
        None,
    ];

    UpgradeableFighter {
        name: "test".into(),
        is_companion,
        level: 500,
        class,
        attribute_basis,
        pet_attribute_bonus_perc: EnumMap::default(),
        equipment,
        active_potions,
        portal_hp_bonus: 50,
        portal_dmg_bonus: 50,
        gladiator: 15,
    }
}

fn create_rune_item(typ: ItemType, rune_typ: RuneType, value: u8) -> Item {
    Item {
        typ,
        rune: Some(Rune {
            typ: rune_typ,
            value,
        }),
        // Defaults
        model_id: 1,
        price: 0,
        mushroom_price: 0,
        class: None,
        type_specific_val: 0,
        attributes: EnumMap::default(),
        gem_slot: None,
        enchantment: None,
        color: 1,
        upgrade_count: 0,
        item_quality: 0,
        is_washed: false,
    }
}
