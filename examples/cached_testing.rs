use std::time::Instant;

use enum_map::EnumMap;
use sf_api::{
    gamestate::{GameState, dungeons::LightDungeon},
    misc::EnumMapGet,
    session::*,
    simulate::{Battle, BattleFighter, BattleSide, PlayerFighterSquad},
    sso::SFAccount,
};
use strum::IntoEnumIterator;

#[tokio::main]
pub async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    const SSO: bool = false;
    const USE_CACHE: bool = true;

    let custom_resp: Option<&str> = None;
    let command = None;

    let username = std::env::var("USERNAME").unwrap();

    let mut session = match SSO {
        true => SFAccount::login(
            std::env::var("SSO_USERNAME").unwrap(),
            std::env::var("PASSWORD").unwrap(),
        )
        .await
        .unwrap()
        .characters()
        .await
        .unwrap()
        .into_iter()
        .flatten()
        .find(|a| a.username() == username)
        .unwrap(),
        false => Session::new(
            &username,
            &std::env::var("PASSWORD").unwrap(),
            ServerConnection::new(&std::env::var("SERVER").unwrap()).unwrap(),
        ),
    };

    _ = std::fs::create_dir("cache");
    let cache_name = format!("cache/{username}.login");

    let login_data = match (USE_CACHE, std::fs::read_to_string(&cache_name)) {
        (true, Ok(s)) => serde_json::from_str(&s).unwrap(),
        _ => {
            let login_data = session.login().await.unwrap();
            let ld = serde_json::to_string_pretty(&login_data).unwrap();
            std::fs::write(&cache_name, ld).unwrap();
            login_data
        }
    };

    let mut gs = GameState::new(login_data).unwrap();

    if let Some(resp) = custom_resp {
        let bruh = Response::parse(
            resp.to_string(),
            chrono::Local::now().naive_local(),
        )
        .unwrap();
        gs.update(bruh).unwrap();
    }

    let Some(command) = command else {
        let js = serde_json::to_string_pretty(&gs).unwrap();
        std::fs::write("character.json", js).unwrap();

        let squad = PlayerFighterSquad::new(&gs);
        let player = BattleFighter::from_upgradeable(&squad.character);
        let mut player_squad = [player];
        for dungeon in LightDungeon::iter() {
            let Some(monster) = gs.dungeons.current_enemy(dungeon) else {
                continue;
            };
            let monster = BattleFighter::from_monster(monster);
            let mut monster = [monster];
            let mut battle = Battle::new(&mut player_squad, &mut monster);
            let mut winners: EnumMap<BattleSide, u32> = EnumMap::default();
            let rounds: usize = 100_000;
            let now = Instant::now();
            for _ in 0..rounds {
                let winner = battle.simulate(&mut ());
                *winners.get_mut(winner) += 1;
            }
            println!(
                "won {:.2}% against {dungeon:?} ({:?}) lvl {} in {:?}",
                (*winners.get(BattleSide::Left) as f32 / rounds as f32) * 100.0,
                monster[0].class,
                monster[0].level,
                now.elapsed()
            );
        }

        return;
    };
    let cache_name = format!(
        "cache/{username}-{}.response",
        serde_json::to_string(&command).unwrap()
    );

    let resp = match (USE_CACHE, std::fs::read_to_string(&cache_name)) {
        (true, Ok(s)) => serde_json::from_str(&s).unwrap(),
        _ => {
            let resp = session.send_command_raw(&command).await.unwrap();
            let ld = serde_json::to_string_pretty(&resp).unwrap();
            std::fs::write(cache_name, ld).unwrap();
            resp
        }
    };

    gs.update(&resp).unwrap();
    let js = serde_json::to_string_pretty(&gs).unwrap();
    std::fs::write("character.json", js).unwrap();
}
