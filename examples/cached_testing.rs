use sf_api::{
    command::Command, gamestate::GameState, session::*, sso::SFAccount,
};

#[tokio::main]
pub async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    const SSO: bool = false;
    const USE_CACHE: bool = true;

    let custom_resp: Option<&str> = None;
    let command = Some(Command::HellevatorPreviewRewards);

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

    std::fs::create_dir("cache").unwrap();
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

    let mut gd = GameState::new(login_data).unwrap();

    if let Some(resp) = custom_resp {
        let bruh = Response::parse(
            resp.to_string(),
            chrono::Local::now().naive_local(),
        )
        .unwrap();
        gd.update(bruh).unwrap();
    }

    let Some(command) = command else {
        let js = serde_json::to_string_pretty(&gd).unwrap();
        std::fs::write("character.json", js).unwrap();
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

    gd.update(&resp).unwrap();
    let js = serde_json::to_string_pretty(&gd).unwrap();
    std::fs::write("character.json", js).unwrap();
}
