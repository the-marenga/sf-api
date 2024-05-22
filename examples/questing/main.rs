#![allow(unused)]
use std::{borrow::Borrow, time::Duration};

use chrono::{DateTime, Local};
use sf_api::{
    command::{Command, ExpeditionSetting, TimeSkip},
    gamestate::{
        items::{Enchantment, EquipmentSlot},
        tavern::{AvailableTasks, CurrentAction},
    },
    misc::EnumMapGet,
    SimpleSession,
};
use sha1::digest::Update;
use tokio::time::sleep;

#[tokio::main]
pub async fn main() {
    let mut session = login_with_env().await;

    loop {
        sleep(Duration::from_secs(2)).await;
        let gs = session.game_state().unwrap();
        match &gs.tavern.current_action {
            CurrentAction::Idle => match gs.tavern.available_tasks() {
                AvailableTasks::Quests(q) => {
                    // You should pick the best quest here, but this is an
                    // example
                    let best_quest = q.first().unwrap();

                    if best_quest.base_length
                        > gs.tavern.thirst_for_adventure_sec
                    {
                        let has_extra_beer = gs
                            .character
                            .equipment
                            .has_enchantment(Enchantment::ThirstyWanderer);

                        if gs.character.mushrooms > 0
                            && gs.tavern.beer_drunk
                                < (10 + has_extra_beer as u8)
                        {
                            println!("Buying beer");
                            session
                                .send_command(Command::BuyBeer)
                                .await
                                .unwrap();
                            continue;
                        } else {
                            println!("Starting city guard");
                            session
                                .send_command(Command::StartWork { hours: 10 })
                                .await
                                .unwrap();
                            break;
                        }
                    }
                    println!("Starting the next quest");

                    if best_quest.item.is_some()
                        && gs.character.inventory.free_slot().is_none()
                    {
                        println!("Inventory is full. Stopping!");
                        // You should sell/use/throw away/equip items at this
                        // point
                        break;
                    }

                    session
                        .send_command(Command::StartQuest {
                            quest_pos: 0,
                            overwrite_inv: true,
                        })
                        .await
                        .unwrap();
                    continue;
                }
                AvailableTasks::Expeditions(_) => {
                    if !gs.tavern.can_change_questing_preference() {
                        println!(
                            "We can not do quests, because we have done \
                             expeditions today already"
                        );
                        break;
                    }
                    println!("Changing questing setting");
                    session
                        .send_command(Command::SetQuestsInsteadOfExpeditions {
                            value: ExpeditionSetting::PreferQuests,
                        })
                        .await
                        .unwrap();
                    continue;
                }
            },
            CurrentAction::Quest {
                quest_idx,
                busy_until,
            } => {
                let remaining = time_remaining(busy_until);
                let mut skip = None;

                if remaining > Duration::from_secs(60) {
                    if gs.tavern.quicksand_glasses > 0 {
                        skip = Some(TimeSkip::Glass);
                    } else if gs.character.mushrooms > 0
                        && gs.tavern.mushroom_skip_allowed
                    {
                        skip = Some(TimeSkip::Mushroom);
                    }
                }
                if let Some(skip) = skip {
                    println!(
                        "Skipping the remaining {remaining:?} with a {skip:?}"
                    );
                    session
                        .send_command(Command::FinishQuest { skip: Some(skip) })
                        .await
                        .unwrap();
                } else {
                    println!(
                        "Waiting {remaining:?} until the quest is finished"
                    );
                    sleep(remaining);
                    session
                        .send_command(Command::FinishQuest { skip })
                        .await
                        .unwrap();
                }
            }
            CurrentAction::CityGuard { hours, busy_until } => {
                let remaining = time_remaining(busy_until);
                if remaining > Duration::from_secs(60 * 60) {
                    // You should not do this is practice. This should at least
                    // check if you can quest anymore
                    println!("Canceling the city guard job");
                    session.send_command(Command::CancelWork).await;
                } else {
                    println!(
                        "Waiting {remaining:?} until the city guard is \
                         finished"
                    );
                    sleep(time_remaining(busy_until)).await;
                    session.send_command(Command::FinishWork).await;
                }
                continue;
            }
            _ => {
                println!("Expeditions are not part of this example");
                break;
            }
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
    sf_api::SimpleSession::login(&username, &password, &server)
        .await
        .unwrap()
}
