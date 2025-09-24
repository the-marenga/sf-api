use std::{borrow::Borrow, time::Duration};

use chrono::{DateTime, Local};
use log::info;
use sf_api::{
    command::Command, gamestate::legendary_dungeon::*, session::SimpleSession,
};
use tokio::time::sleep;

#[tokio::main]
pub async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();
    let mut session = login_with_env().await;

    loop {
        sleep(Duration::from_secs(30)).await;
        let gs = session.game_state().unwrap();

        let status = gs.legendary_dungeon.status();

        match status {
            LegendaryDungeonStatus::TakeItem { .. } => todo!(),
            LegendaryDungeonStatus::Unavailable => {
                info!("The event is not ongoing");
                return;
            }
            LegendaryDungeonStatus::NotEntered => {}
            LegendaryDungeonStatus::Ended(stats) => {
                info!("The event has ended. Your stats are: \n{stats:#?}");
                return;
            }
            LegendaryDungeonStatus::Healing { can_continue, .. } => {
                if !can_continue {
                    info!("We are dead. Waiting until we can continue..");
                    sleep(Duration::from_secs(60 * 60)).await;
                    session.send_command(Command::Update).await.unwrap();
                    return;
                }
                // MS8w
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
                info!(
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
                status,
                encounter,
                typ,
                dungeon,
            } => {
                match status {
                    RoomStatus::Entered => {
                        // Interact with the room normally, we just entered
                    }
                    RoomStatus::Interacted => {
                        // can lead to 60, or 70 I think. 60 should be
                        // "I defeated the monster" and 70 should be generic
                        // room interaction finished. Not sure though
                        let RoomEncounter::Monster(_) = encounter else {
                            session
                                .send_command(Command::LegendaryDungeon60)
                                .await
                                .unwrap();
                            continue;
                        };
                        // TODO: Is this right?
                        session
                            .send_command(
                                Command::LegendaryDungeonForcedContinue,
                            )
                            .await
                            .unwrap();
                        continue;
                    }
                    RoomStatus::Finished => {
                        session
                            .send_command(
                                Command::LegendaryDungeonForcedContinue,
                            )
                            .await
                            .unwrap();
                        continue;
                    }
                };

                match typ {
                    RoomType::BossRoom => todo!(),
                    RoomType::Generic | RoomType::Encounter => {
                        match encounter {
                            RoomEncounter::BronzeChest
                            | RoomEncounter::SilverChest
                            | RoomEncounter::EpicChest
                            | RoomEncounter::Crate1
                            | RoomEncounter::Crate2
                            | RoomEncounter::Crate3
                            | RoomEncounter::PrizeChest
                            | RoomEncounter::SatedChest
                            | RoomEncounter::FallenWarrior => {
                                session
                                    .send_command(Command::LegendaryDungeonEncounterInteract)
                                    .await
                                    .unwrap();
                                // TODO: Does this 70,70, or do we manually
                                // escpape?
                            }
                            RoomEncounter::SleepingSkeleton
                            | RoomEncounter::MimicChest
                            | RoomEncounter::Barrel
                            | RoomEncounter::SacrificialChest
                            | RoomEncounter::CurseChest => {
                                if dungeon.current_hp >= dungeon.max_hp / 2 {
                                    session
                                        .send_command(Command::LegendaryDungeonEncounterInteract)
                                        .await
                                        .unwrap();
                                } else {
                                    session
                                        .send_command(Command::LegendaryDungeonEncounterEscape)
                                        .await
                                        .unwrap();
                                }
                            }
                            RoomEncounter::Unknown => {
                                // 40 or 41 probably
                                todo!()
                            }
                            RoomEncounter::Monster(_) => {
                                // 20 => attack
                                // 21 => run
                            }
                        }
                    }
                    RoomType::Empty => {
                        // 70 - Leave
                    }
                    RoomType::FountainOfLife
                    | RoomType::SoulBath
                    | RoomType::ArcaneSplintersCave
                    | RoomType::HoleInTheFloor
                    | RoomType::PileOfRocks
                    | RoomType::PileOfWood
                    | RoomType::TheFloorIsLava
                    | RoomType::DungeonNarrator
                    | RoomType::FloodedRoom
                    | RoomType::UnlockedSarcophagus => {
                        // These are all either exclusively good, or forced, so
                        // we just interact with the room
                        session
                            .send_command(Command::LegendaryDungeonRoomInteract)
                            .await
                            .unwrap();
                    }
                    RoomType::Sewers
                    | RoomType::WishingWell
                    | RoomType::AuctionHouse
                    | RoomType::LockerRoom => {
                        if gs.character.inventory.free_slot().is_none() {
                            // I am not sure, if we need this to send the cmd,
                            // but best practice just do it. Otherwise this
                            // could be merged into the above check
                            todo!("Free up an inventory slot");
                        }
                        session
                            .send_command(Command::LegendaryDungeonRoomInteract)
                            .await
                            .unwrap();
                        // We will get a pending item, which we handle next
                        // iteration
                    }
                    RoomType::RockPaperScissors => {
                        if dungeon.current_hp >= dungeon.max_hp / 2 {
                            let choice = RPCChoice::Paper;
                            session
                                .send_command(
                                    Command::LegendaryDungeonPlayRPC { choice },
                                )
                                .await
                                .unwrap();
                        } else {
                            session
                                .send_command(
                                    Command::LegendaryDungeonRoomLeave,
                                )
                                .await
                                .unwrap();
                        }
                        // 90, 70, 70 Rock (forced line)
                        // 91, 70, 70 Paper (forced line)
                        // 92, 70, 70 Scisors (forced line)
                    }
                    RoomType::WheelOfFortune
                    | RoomType::FlyingTube
                    | RoomType::BetaRoom
                    | RoomType::UndeadFiend
                    | RoomType::RainbowRoom
                    | RoomType::PigRoom
                    | RoomType::Valaraukar => {
                        // Rooms that cost some health, and/or inflict a curse
                        // upon the player when something goes wrong. Make sure
                        // we have enough hp to do that
                        if dungeon.current_hp >= dungeon.max_hp / 2 {
                            session
                                .send_command(
                                    Command::LegendaryDungeonRoomInteract,
                                )
                                .await
                                .unwrap();
                        } else {
                            session
                                .send_command(
                                    Command::LegendaryDungeonRoomLeave,
                                )
                                .await
                                .unwrap();
                        }
                    }
                    RoomType::KeyMasterShop => {
                        let available_blessings = dungeon
                            .merchant_offers
                            .iter()
                            .filter(|a| a.keys <= dungeon.keys)
                            .collect::<Vec<_>>();

                        // NOTE: You would want to sort this based on which
                        // type is the best. Could also buy two here I think

                        if let Some(blessing) = available_blessings.first() {
                            session
                                .send_command(
                                    Command::LegendaryDungeonMerchantBuy {
                                        effect: blessing.typ,
                                        keys: blessing.keys,
                                    },
                                )
                                .await
                                .unwrap();
                        } else if 1 == 0 && gs.character.mushrooms > 0 {
                            session
                                .send_command(
                                    Command::LegendaryDungeonMerchantNewGoods,
                                )
                                .await
                                .unwrap();
                            continue;
                        }
                        session
                            .send_command(Command::LegendaryDungeonRoomLeave)
                            .await
                            .unwrap();
                    }
                    RoomType::SpiderWeb => {
                        // NOTE: I have no idea how to read the spider type.
                        // Might be the enounter id. Just to be on the safe
                        // side, we just leave here
                        session
                            .send_command(Command::LegendaryDungeonRoomLeave)
                            .await
                            .unwrap();
                    }
                    RoomType::KeyToFailureShop => {
                        let available_curses = &dungeon.merchant_offers;

                        // NOTE: You would want to sort this based on which
                        // type is the best (the least bad). Could also buy two
                        // here I suppose

                        if let Some(curse) = available_curses.first()
                            && dungeon.current_hp >= dungeon.max_hp / 2
                        {
                            session
                                .send_command(
                                    Command::LegendaryDungeonMerchantBuy {
                                        effect: curse.typ,
                                        keys: curse.keys,
                                    },
                                )
                                .await
                                .unwrap();
                        }
                        session
                            .send_command(Command::LegendaryDungeonRoomLeave)
                            .await
                            .unwrap();
                    }
                    RoomType::Unknown => {
                        // We have no idea what this is, but most rooms can
                        // just be interacted with, or left
                        if session
                            .send_command(Command::LegendaryDungeonRoomInteract)
                            .await
                            .is_err()
                        {
                            session
                                .send_command(
                                    Command::LegendaryDungeonRoomLeave,
                                )
                                .await
                                .unwrap();
                        }
                    }
                }

                // TODO: Check if we need to leave here, or if we can determine
                // that by room status next iteration
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
    SimpleSession::login(&username, &password, &server)
        .await
        .unwrap()
}

// if gs.character.inventory.free_slot().is_none() {
//     info!(
//         "Inventory is full. This should only matter "
//     );
//     // You should make a free slot at this point
//     break;
// }
