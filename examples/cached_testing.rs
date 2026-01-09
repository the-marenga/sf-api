use std::time::Instant;

use sf_api::{
    gamestate::{GameState, dungeons::LightDungeon},
    session::*,
    simulate::simulate_dungeon,
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
        let resp = Response::parse(
            resp.to_string(),
            chrono::Local::now().naive_local(),
        )
        .unwrap();
        gs.update(resp).unwrap();
    }

    let Some(command) = command else {
        let js = serde_json::to_string_pretty(&gs).unwrap();
        std::fs::write("character.json", js).unwrap();

        for dungeon in LightDungeon::iter() {
            // if dungeon != LightDungeon::Hemorridor {
            //     continue;
            // }
            let now = Instant::now();
            let Some(res) = simulate_dungeon(&gs, dungeon, 1_000_000) else {
                continue;
            };
            println!(
                "won {:.2}% in {dungeon:?} in {:?}",
                res.win_ratio * 100.0,
                now.elapsed(),
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
