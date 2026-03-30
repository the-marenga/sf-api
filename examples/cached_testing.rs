use clap::Parser;
use sf_api::{gamestate::GameState, session::*, sso::SFAccount};

#[tokio::main]
pub async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let args = Args::parse();

    let custom_resp: Option<&str> = None;
    let command = None;

    let username = args.username;

    let mut session = match args.sso {
        true => SFAccount::login(
            args.sso_username
                .expect("SSO_USERNAME or --sso-username is required for SSO"),
            args.password,
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
            &args.password,
            ServerConnection::new(
                &args
                    .server
                    .expect("SERVER or --server is required for non-SSO"),
            )
            .unwrap(),
        ),
    };

    _ = std::fs::create_dir("cache");
    let cache_name = format!("cache/{username}.login");

    let login_data = match (args.cache, std::fs::read_to_string(&cache_name)) {
        (_, Ok(s)) if args.diff => {
            let old: Response = serde_json::from_str(&s).unwrap();
            let new = session.login().await.unwrap();
            // TODO: Diff the two values
            for (&key, new_val) in new.values() {
                if key.ends_with("id")
                    || key == "timestamp"
                    || key == "expeditionevent"
                    || key == "idle"
                {
                    continue;
                }
                let Some(old_val) = old.values().get(key) else {
                    println!("New key: {key}");
                    continue;
                };
                let old_val: Vec<_> = old_val.as_str().split("/").collect();
                let new_val: Vec<_> = new_val.as_str().split("/").collect();
                for (idx, (new, old)) in
                    new_val.into_iter().zip(old_val).enumerate()
                {
                    if new.starts_with("17") && new.len() == "1774765933".len()
                    {
                        continue;
                    }
                    if key == "ownplayersave" && idx == 478 {
                        continue;
                    }
                    if new != old {
                        println!("{key}[{idx}] {old} => {new}");
                    }
                }
            }
            return;
        }
        (true, Ok(s)) => serde_json::from_str(&s).unwrap(),
        _ => {
            let login_data = session.login().await.unwrap();
            let ld = serde_json::to_string_pretty(&login_data).unwrap();
            std::fs::write(&cache_name, ld).unwrap();
            login_data
        }
    };

    // for (key, value) in login_data.values() {
    //     if !key.starts_with("event") {
    //         continue;
    //     }
    //     println!("resp.start_section(\"{key}\");");
    //     let values:Vec<i64> = value.into_list("test").unwrap();
    //     for num in values {
    //         println!("resp.add_val({num});");
    //     }
    // }

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
        return;
    };
    let cache_name = format!(
        "cache/{username}-{}.response",
        serde_json::to_string(&command).unwrap()
    );

    let resp = match (args.cache, std::fs::read_to_string(&cache_name)) {
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Whether to use SSO login
    #[arg(short, long)]
    sso: bool,

    /// Whether to use cached responses
    #[arg(short, long)]
    cache: bool,

    #[arg(short, long)]
    diff: bool,

    /// Character username
    #[arg(short, long, env = "USERNAME")]
    username: String,

    /// Character password
    #[arg(short, long, env = "PASSWORD")]
    password: String,

    /// Game server (required if not using SSO)
    #[arg(long, env = "SERVER")]
    server: Option<String>,

    /// SSO username / Email (required if using SSO)
    #[arg(long, env = "SSO_USERNAME")]
    sso_username: Option<String>,
}
