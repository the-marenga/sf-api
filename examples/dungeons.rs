use std::{borrow::Borrow, time::Duration};

use chrono::{DateTime, Local};
use enum_map::{EnumArray, EnumMap};
use sf_api::{
    command::Command,
    gamestate::dungeons::{Dungeon, DungeonProgress},
    SimpleSession,
};
use tokio::time::sleep;

#[tokio::main]
pub async fn main() {
    let mut session = login_with_env().await;

    loop {
        sleep(Duration::from_secs(2)).await;
        let gs = session.game_state().unwrap();

        // We might have dungeon keys still waiting to be unlocked, so we
        // should use everything we have
        if let Some(unlockable) = gs.pending_unlocks.first().copied() {
            session
                .send_command(Command::UnlockFeature { unlockable })
                .await
                .unwrap();
            continue;
        }

        if let Some(portal) = &gs.dungeons.portal {
            // TODO: I do not have a char, that has finished the portal, so you
            // should maybe check the finished count against the current
            if portal.can_fight {
                println!("Fighting the player portal");
                session.send_command(Command::FightPortal).await.unwrap();
                continue;
            }
        }

        if gs.character.inventory.free_slot().is_none() {
            println!(
                "Inventory is full. We can not fight in a dungeon like this"
            );
            // You should make a free slot at this point
            break;
        }

        // You should make a better heuristic to find these, but for now we just
        // find the lowest level
        let best_light_dungeon = find_lowest_lvl_dungeon(&gs.dungeons.light);
        let best_shadow_dungeon = find_lowest_lvl_dungeon(&gs.dungeons.shadow);

        let (target_dungeon, target_level) =
            match (best_light_dungeon, best_shadow_dungeon) {
                (Some(x), Some(y)) => {
                    if x.1 < y.1 {
                        x
                    } else {
                        y
                    }
                }
                (Some(x), _) => x,
                (_, Some(x)) => x,
                (None, None) => {
                    println!("There are no dungeons to fight in!");
                    break;
                }
            };

        println!("Chose: {target_dungeon:?} as the best dungeon to fight in");

        let Some(next_fight) = gs.dungeons.next_free_fight else {
            println!("We do not have a time for the next fight");
            break;
        };
        let rem = time_remaining(next_fight);

        if rem > Duration::from_secs(60 * 5)
            && gs.character.mushrooms > 1000
            && target_level <= gs.character.level + 20
        {
            // You should add some better logic on when to skip this
            println!("Using mushrooms to fight in the dungeon");
            session
                .send_command(Command::FightDungeon {
                    dungeon: target_dungeon,
                    use_mushroom: true,
                })
                .await
                .unwrap();
        } else {
            println!("Waiting {rem:?} until we can fight in a dungeon");
            sleep(rem).await;
            session
                .send_command(Command::FightDungeon {
                    dungeon: target_dungeon,
                    use_mushroom: false,
                })
                .await
                .unwrap();
        }
    }
}

fn find_lowest_lvl_dungeon<T: EnumArray<DungeonProgress> + Into<Dungeon>>(
    dungeons: &EnumMap<T, DungeonProgress>,
) -> Option<(Dungeon, u16)> {
    dungeons
        .iter()
        .filter_map(|a| {
            if let DungeonProgress::Open { level, .. } = a.1 {
                Some((a.0.into(), *level))
            } else {
                None
            }
        })
        .min_by_key(|a| {
            a.1
        })
}

pub fn time_remaining<T: Borrow<DateTime<Local>>>(time: T) -> Duration {
    (*time.borrow() - Local::now()).to_std().unwrap_or_default()
}

pub async fn login_with_env() -> SimpleSession {
    let username = std::env::var("USERNAME").unwrap();
    let password = std::env::var("PASSWORD").unwrap();
    let server = std::env::var("SERVER").unwrap();
    sf_api::SimpleSession::login(&username, &password, &server)
        .await
        .unwrap()
}
