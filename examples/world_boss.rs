use std::{borrow::Borrow, time::Duration};

use chrono::{DateTime, Local};
use log::{info, warn};
use sf_api::{
    command::Command,
    error::SFError,
    gamestate::{
        event::*, fortress::FortressResourceType, rewards::RewardType,
    },
    misc::EnumMapGet,
    session::SimpleSession,
};
use tokio::time::sleep;

#[tokio::main]
pub async fn main() -> Result<(), SFError> {
    let mut session = login_with_env().await?;

    'outer_loop: loop {
        sleep(Duration::from_secs(5)).await;
        let gs = session.game_state().unwrap();
        let Some(EventStatus {
            typ: SpecialEventType::WorldBoss(_),
            start: Some(start),
            end: Some(end),
            extra_end: _,
        }) = &gs.special_event
        else {
            info!("World boss event is not available");
            break;
        };

        let now = Local::now();
        if now < *start || now > *end {
            info!("World boss event has not yet started, or is over");
            break;
        }

        let Some(world_boss) = &gs.world_boss else {
            // We do not have any world boss info. that means we have not
            // entered the event.
            info!("Joining the world boss fight");
            session.send_command(Command::WorldBossEnter).await?;
            continue;
        };

        let Some(battle) = &world_boss.battle else {
            // This account is technically already in the world boss event, but
            // we do not have all the relevant information yet. To fetch that
            // in the official UI, we would have to click on the "World Boss"
            // text, which is basically what this here is going to do
            session.send_command(Command::WorldBossEnter).await?;
            continue;
        };

        if world_boss.available_daily_chests.values().any(|a| *a > 0) {
            // Automatically collect all daily chests, since that is what the
            // game also does
            session
                .send_command(Command::WorldBossCollectDailyChests)
                .await?;
            continue;
        }

        if world_boss.battle_reward_chests > 0 {
            // We update the state of the worldboss after each attack, since
            // most things change very infrequently and this is a good
            // benchmark
            session
                .send_command(Command::WorldBossCollectBattleRewards)
                .await?;
            continue;
        }

        if let Some(catapult) =
            world_boss.catapult.as_ref().filter(|a| a.breaks > now)
        {
            // The price of upgrades is scaled by remaining time, so we first
            // calculate that first
            let price_modifier = {
                let remaining_time = time_remaining(catapult.breaks);
                let max_time = Duration::from_hours(10);
                remaining_time.as_secs() as f64 / max_time.as_secs() as f64
            };

            // We always want to keep at least 10 catalysts to buy a new
            // catapult, if necessary. You can obviously ignore this for your
            // code
            let available_catalysts = world_boss.catalysts.saturating_sub(10);

            for (pos, offer) in world_boss.upgrade_offers.iter().enumerate() {
                // figure out if any of them are useful
                let (WorldBossCatapultUpgradeType::Thread
                | WorldBossCatapultUpgradeType::Motor) = offer.typ
                else {
                    // We preted that only thread & motor upgrades are good for
                    // now
                    continue;
                };

                // Note: if we wanted, we could skip the price check here and
                // just pay with mushrooms, if we wanted to
                let catalyst_price = (offer.raw_catalyst_price as f64
                    * price_modifier)
                    .ceil() as u64;
                if catalyst_price > available_catalysts as u64 {
                    continue;
                }

                let main_price = (offer.raw_main_price as f64 * price_modifier)
                    .ceil() as u64;

                // Figure out how much of this we have available
                let available_main = match offer.main_price_type {
                    RewardType::Silver => gs.character.silver,
                    RewardType::LuckyCoins => {
                        gs.specials.wheel.lucky_coins as u64
                    }
                    RewardType::Wood => gs
                        .fortress
                        .as_ref()
                        .map(|a| {
                            a.resources.get(FortressResourceType::Wood).current
                        })
                        .unwrap_or(0)
                        as u64,
                    RewardType::Stone => gs
                        .fortress
                        .as_ref()
                        .map(|a| {
                            a.resources.get(FortressResourceType::Stone).current
                        })
                        .unwrap_or(0)
                        as u64,
                    RewardType::Souls => {
                        gs.underworld
                            .as_ref()
                            .map(|a| a.souls_current)
                            .unwrap_or(0) as u64
                    }
                    RewardType::Metal => {
                        gs.blacksmith.as_ref().map(|a| a.metal).unwrap_or(0)
                    }
                    RewardType::Arcane => {
                        gs.blacksmith.as_ref().map(|a| a.arcane).unwrap_or(0)
                    }
                    RewardType::QuicksandGlass => {
                        gs.tavern.quicksand_glasses as u64
                    }
                    RewardType::Mushrooms => {
                        // You can tweak this to actually return your
                        // mushrooms, but for this example we want to never
                        // touch shrooms
                        0
                    }
                    x => {
                        warn!("Unexpected paymet type: {x:?}");
                        0
                    }
                };
                if available_main < main_price {
                    // We can not afford this
                    continue;
                }
                if let Some(offer_restriction) = offer.restriction {
                    let mut would_mix_restrictions = false;
                    for a in catapult.upgrades.iter().flatten() {
                        if let Some(existing_restriction) = a.restriction {
                            if existing_restriction != offer_restriction {
                                // We already have upgrades for another tower
                                // level and we don't want to mix them
                                would_mix_restrictions = true;
                            }
                            if existing_restriction == offer_restriction {
                                // We already have a restriction of this type,
                                // so this upgrade must be fine
                                would_mix_restrictions = false;
                                break;
                            }
                        }
                    }
                    if would_mix_restrictions {
                        continue;
                    }
                }

                // Make sure we even have space available for this upgrade
                if catapult.upgrades.iter().all(|a| {
                    a.as_ref().is_some_and(|a| {
                        a.typ != offer.typ
                            || a.amount >= 10
                            || a.restriction != offer.restriction
                    })
                }) {
                    // We do not have the space available for this upgrade
                    break;
                }

                // This seems to be a nice upgrade. Buy it.
                session
                    .send_command(Command::WorldBossBuyUpgrade {
                        offer_idx: pos,
                        use_mushrooms: false,
                    })
                    .await?;
                continue 'outer_loop;
            }

            // Try to buy projectiles, if we don't have any yet
            if world_boss.projectile.is_none() {
                for (pos, projectile) in
                    world_boss.projectile_offers.iter().enumerate()
                {
                    if projectile.typ == WorldBossProjectileType::Unknown {
                        continue;
                    }
                    for (offer_size, offer) in &projectile.buy_options {
                        if offer.price > available_catalysts {
                            break;
                        }
                        // If we wanted to, we could now buy these
                        (_, _) = (offer_size, pos);
                        // session.send_command(Command::WorldBossProjectileBuy
                        // {
                        //     offer_idx: pos,
                        //     amount: offer_size,
                        // })
                    }
                }
            }
        } else {
            // We do not have a catapult yet, or it has been broken. Buy a new
            // one, if we have enough resources
            let length = world_boss.catalysts.min(10);
            if length > 0 {
                session
                    .send_command(Command::WorldBossBuyCatapult { length })
                    .await?;
                continue;
            }
        };

        // We will focus the attack on the weak point, if we have not yet
        // gotten the chest yet
        if let Some(weak_point) = battle.weak_point
            && weak_point != world_boss.current_segment
        {
            if !battle.weak_point_hit {
                session
                    .send_command(Command::WorldBossChangeTarget {
                        target: weak_point,
                    })
                    .await?;
                continue;
            }
        } else {
            // TODO: Switch to another level, if it makes more sense for other
            // reasons
        }

        if world_boss.attack_timer.is_some_and(|a| a < now) {
            // We update the state of the worldboss after each attack, since
            // most things change very infrequently and this is a good
            // benchmark
            session.send_command(Command::WorldBossUpdate).await?;
            continue;
        }
    }
    Ok(())
}

pub fn time_remaining<T: Borrow<DateTime<Local>>>(time: T) -> Duration {
    (*time.borrow() - Local::now()).to_std().unwrap_or_default()
}

pub async fn login_with_env() -> Result<SimpleSession, SFError> {
    let username = std::env::var("USERNAME").unwrap();
    let password = std::env::var("PASSWORD").unwrap();
    let server = std::env::var("SERVER").unwrap();
    SimpleSession::login(&username, &password, &server).await
}
