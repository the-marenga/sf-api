use std::time::Duration;

use chrono::{DateTime, Local};
use sf_api::{
    command::{Command, TimeSkip},
    gamestate::tavern::ExpeditionStage,
    SimpleSession,
};
use tokio::time::sleep;

#[tokio::main]
pub async fn main() {
    let mut session = login_with_env().await;

    loop {
        let gs = session.game_state().unwrap();
        let exp = &gs.tavern.expeditions;

        let Some(active) = exp.active() else {
            // We do not currently have an expedition running. Make sure we are
            // idle
            if !gs.tavern.current_action.is_idle() {
                println!(
                    "Waiting/Collection other actions is not part of this \
                     example"
                );
                break;
            }

            // Make sure expeditions can be started
            let now = &Local::now();
            match (&exp.start, &exp.end) {
                (Some(start), Some(end)) if end > now && start < now => {}
                _ => {
                    println!(
                        "Expeditions are currrently not enabled, so we can \
                         not do anything"
                    );
                    break;
                }
            }

            // We would normally have to choose which expedition is the best.
            // For now we just choose the first one though
            let target = exp.available.first().unwrap();

            // Make sure we have enough thirst for adventure to do the
            // expeditions
            if target.thirst_for_adventure_sec
                > gs.tavern.thirst_for_adventure_sec
            {
                println!("We do not have enough thirst for adventure left");
                println!("Buying beer is an option, but not for this example");
                break;
            }

            // We should be all good to start the expedition
            println!("Starting expedition");
            session
                .send_cmd(Command::ExpeditionStart { pos: 0 })
                .await
                .unwrap();
            continue;
        };
        let current = active.current_stage();

        let cmd = match current {
            ExpeditionStage::Boss(_) => {
                println!("Fighting the expedition boss");
                Command::ExpeditionContinue
            }
            ExpeditionStage::Rewards(rewards) => {
                if rewards.is_empty() {
                    panic!("No rewards to choose from");
                }
                println!("Picking reward");
                // We should pick the best reward here
                Command::ExpeditionPickReward { pos: 0 }
            }
            ExpeditionStage::Crossroads(roads) => {
                if roads.is_empty() {
                    panic!("No crossroads to choose from");
                }
                // We should pick the best crossroad here
                println!("Continuing on crossroad");
                Command::ExpeditionChooseStreet { pos: 0 }
            }
            ExpeditionStage::Finished => {
                // Between calling current_stage and now the expedition
                // finished. next time we call active, it will be None
                continue;
            }
            ExpeditionStage::Waiting(until) => {
                let remaining =
                    (until - Local::now()).to_std().unwrap_or_default();
                if remaining.as_secs() > 60 && gs.tavern.quicksand_glasses > 0 {
                    println!("Skipping the {}s wait", remaining.as_secs());
                    Command::ExpeditionSkipWait {
                        typ: TimeSkip::Glass,
                    }
                } else {
                    println!(
                        "Waiting {}s until next expedition step",
                        remaining.as_secs(),
                    );
                    sleep(remaining).await;
                    Command::UpdatePlayer
                }
            }
            ExpeditionStage::Unknown => panic!("unknown expedition stage"),
        };
        sleep(Duration::from_secs(1)).await;
        session.send_cmd(cmd).await.unwrap();
    }
}

pub async fn sleep_until(time: &DateTime<Local>) {
    let duration = *time - Local::now();
    sleep(duration.to_std().unwrap_or_default()).await;
}

pub async fn login_with_env() -> SimpleSession {
    let username = std::env::var("USERNAME").unwrap();
    let password = std::env::var("PASSWORD").unwrap();
    let server = std::env::var("SERVER").unwrap();
    sf_api::SimpleSession::login_normal(&username, &password, &server)
        .await
        .unwrap()
}
