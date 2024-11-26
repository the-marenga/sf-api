use std::time::Duration;

use sf_api::{
    command::Command, gamestate::tavern::GambleResult, session::SimpleSession,
};
use tokio::time::sleep;

#[tokio::main]
pub async fn main() {
    let mut session = login_with_env().await;

    let (mut won, mut lost) = (0, 0);
    loop {
        let gs = session.game_state_mut().unwrap();
        println!(
            "won {won}/{} ({:.2}%)",
            won + lost,
            (won as f32 / (won + lost).max(1) as f32) * 100.0
        );
        if let Some(GambleResult::SilverChange(res)) = gs.tavern.gamble_result {
            if res > 0 {
                won += 1;
            } else {
                lost += 1;
            }
        }

        if gs.character.silver == 0 {
            println!("Character went out of money to gamble with");
            break;
        }

        gs.tavern.gamble_result = None;
        sleep(Duration::from_millis(fastrand::u64(500..1000))).await;
        // There is no actual check how much you gamble, so we just gamble 1
        // silver here
        session
            .send_command(Command::GambleSilver { amount: 1 })
            .await
            .unwrap();
    }
}

pub async fn login_with_env() -> SimpleSession {
    let username = std::env::var("USERNAME").unwrap();
    let password = std::env::var("PASSWORD").unwrap();
    let server = std::env::var("SERVER").unwrap();
    SimpleSession::login(&username, &password, &server)
        .await
        .unwrap()
}
