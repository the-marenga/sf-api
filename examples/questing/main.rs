#![allow(unused)]
use std::time::Duration;

use chrono::{DateTime, Local};
use sf_api::{
    command::Command, gamestate::tavern::CurrentAction, SimpleSession,
};
use sha1::digest::Update;
use tokio::time::sleep;

#[tokio::main]
pub async fn main() {
    let mut session = login_with_env().await;

    loop {
        let gs = session.game_state().unwrap();
        // We do not currently have an expedition running. Make sure we are
        // idle

        match &gs.tavern.current_action {
            CurrentAction::Idle => {
                // TODO: do questing
            }
            CurrentAction::Quest {
                quest_idx,
                busy_until,
            } => {
                sleep_until(busy_until).await;

                session
                    .send_command(Command::FinishQuest { skip: None })
                    .await;
                continue;
            }
            CurrentAction::CityGuard { hours, busy_until } => todo!(),
            _ => {
                println!("Expeditions are not part of this example");
                break;
            }
        }
    }
}

pub async fn sleep_until(time: &DateTime<Local>) {
    let duration = *time - Local::now();
    // We wait a bit longer, because there is always bit of time difference
    // between us and the server
    sleep(duration.to_std().unwrap_or_default() + Duration::from_secs(1)).await;
}

pub async fn login_with_env() -> SimpleSession {
    let username = std::env::var("USERNAME").unwrap();
    let password = std::env::var("PASSWORD").unwrap();
    let server = std::env::var("SERVER").unwrap();
    sf_api::SimpleSession::login(&username, &password, &server)
        .await
        .unwrap()
}
