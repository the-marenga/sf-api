use std::{borrow::Borrow, time::Duration};

use chrono::{DateTime, Local};
use sf_api::{
    command::Command,
    gamestate::dungeons::{Dungeon, LightDungeon},
    session::SimpleSession,
    simulate::Monster,
};
use strum::IntoEnumIterator;
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

        let mut best: Option<(Dungeon, &'static Monster)> = None;
        // TODO: ShadowDungeons
        for l in LightDungeon::iter() {
            let Some(current) = gs.dungeons.current_enemy(l) else {
                continue;
            };
            // You should make a better heuristic to find these, but for now we
            // just find the lowest level
            if best.map_or(true, |old| old.1.level > current.level) {
                best = Some((l.into(), current))
            }
        }

        let Some((target_dungeon, target_monster)) = best else {
            println!("There are no more enemies left to fight");
            break;
        };

        println!("Chose: {target_dungeon:?} as the best dungeon to fight in");

        let Some(next_fight) = gs.dungeons.next_free_fight else {
            println!("We do not have a time for the next fight");
            break;
        };
        let rem = time_remaining(next_fight);

        if rem > Duration::from_secs(60 * 5)
            && gs.character.mushrooms > 1000
            && target_monster.level <= gs.character.level + 20
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
            println!("Waiting {rem:?} until we can fight in the dungeon again");
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

pub fn time_remaining<T: Borrow<DateTime<Local>>>(time: T) -> Duration {
    (*time.borrow() - Local::now()).to_std().unwrap_or_default()
}

pub async fn login_with_env() -> SimpleSession {
    let username = std::env::var("USERNAME").unwrap();
    let password = std::env::var("PASSWORD").unwrap();
    let server = std::env::var("SERVER").unwrap();
    SimpleSession::login(&username, &password, &server)
        .await
        .unwrap()
}
