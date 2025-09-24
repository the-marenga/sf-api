use std::{borrow::Borrow, time::Duration};

use chrono::{DateTime, Local};
use sf_api::{
    command::Command, gamestate::legendary_dungeon::LegendaryDungeonStatus,
    session::SimpleSession,
};
use tokio::time::sleep;

#[tokio::main]
pub async fn main() {
    let mut session = login_with_env().await;

    loop {
        sleep(Duration::from_secs(2)).await;
        let gs = session.game_state().unwrap();

        let status = gs.legendary_dungeon.status();

        match status {
            LegendaryDungeonStatus::Unavailable => {
                println!("The event is not ongoing");
                return;
            }
            LegendaryDungeonStatus::NotEntered => {}
            LegendaryDungeonStatus::Ended(stats) => {
                println!("The event has ended. Your stats are: \n{stats:#?}");
                return;
            }
            LegendaryDungeonStatus::Healing { can_continue, .. } => {
                if !can_continue {
                    println!("We are dead. Waiting until we can continue..");
                    sleep(Duration::from_secs(60 * 60)).await;
                    session.send_command(Command::Update).await.unwrap();
                    return;
                }
                todo!("Start a new dungeon run / continue it")
            }
            LegendaryDungeonStatus::PickGem { available_gems, .. } => {
                let best = available_gems
                    .first()
                    .expect("We should always have at least one gem");
                session
                    .send_command(Command::LegendaryDungeonPickGem {
                        gem_type: best.typ,
                    })
                    .await
                    .unwrap();
            }
            LegendaryDungeonStatus::DoorSelect { doors, .. } => {
                println!(
                    "Left door: {:?}{}, right door: {:?}{}",
                    doors[0].typ,
                    doors[0]
                        .trap
                        .map(|a| format!(" (trapped {a:?})"))
                        .unwrap_or_else(String::new),
                    doors[1].typ,
                    doors[1]
                        .trap
                        .map(|a| format!(" (trapped {a:?})"))
                        .unwrap_or_else(String::new),
                );
                // here you should figure out which door is the best. We just
                // pick the left door.
                session
                    .send_command(Command::LegendaryDungeonPickDoor { pos: 0 })
                    .await
                    .unwrap();
            }
            LegendaryDungeonStatus::Unknown => {
                // RIP
                return;
            }
            LegendaryDungeonStatus::Room {
                dungeon,
                status,
                encounter,
                typ,
            } => todo!(),
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

// if gs.character.inventory.free_slot().is_none() {
//     println!(
//         "Inventory is full. This should only matter "
//     );
//     // You should make a free slot at this point
//     break;
// }
