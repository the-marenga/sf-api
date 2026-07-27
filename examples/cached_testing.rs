use clap::Parser;
use regex::Regex;
use sf_api::{gamestate::GameState, session::*, sso::SFAccount};

#[tokio::main]
pub async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();

    let custom_resp: Option<&str> = Some("fightversion:2&fightheader.fighters:0/0/0/0/1/1039746/bruhbruh/52/167056/167056/98/73/987/788/439/5/303/301/3/303/1/5/16/0/0/1/1/10/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/784913/haret44 (w35net)/57/210192/210192/1243/161/148/906/216/3/109/103/4/105/4/5/7/9/0/8/1/6/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0&fightequipment:1/1010/5/0/0/0/0/1/0/0/1/24/4/0/0/0/0/1/0/0&fightdecoration:0/0/0/0&externaltoolequipment:194/246/0/0/64/160/0/0&fight.r:784913/30/0/0/0/210192/158354/0/0/784913/0/0/0/0/210192/146572/0/0/1039746/0/0/0/0/146572/198240/0/0/784913/0/0/0/0/198240/128413/0/0/1039746/0/11/0/0/128413/198240/1/2/3/4/0/1039746/0/12/0/0/128413/181542/1/2/3/3/0/784913/30/0/0/0/181542/95209/0/1/2/3/3/784913/0/0/3/0/181542/95209/0/1/2/3/3/1039746/0/0/0/0/95209/160305/1/2/3/3/0/1039746/0/12/0/0/95209/140400/1/2/3/2/0/784913/0/0/0/0/140400/43903/0/1/2/3/2/1039746/0/0/0/0/43903/119032/1/2/3/2/0/1039746/0/12/0/0/43903/92344/1/2/3/1/0/784913/30/0/0/0/92344/9840/0/1/2/3/1/784913/0/0/0/0/92344/-25186/0/1/2/3/1/&winnerid:784913&fightresult.battlereward:0/1/0/0/0/-101/0/199362/202624/0/0/0/0/0/0/0/0/0/0/0/0&battlerewarditem:0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0");

    let commands: Vec<sf_api::command::Command> = vec![];

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

    if let Some(re) = args.search {
        for (&key, value) in login_data.values() {
            if key == "ownplayersave" {
                continue;
            }
            if let Some(key_re) = &args.search_key
                && !key_re.is_match(key)
            {
                continue;
            }
            let values: Vec<_> = value.as_str().split('/').collect();
            for (pos, num) in values.into_iter().enumerate() {
                if re.is_match(num) {
                    println!("{key}[{pos}] = {num}")
                }
            }
        }
    }

    let mut gs = GameState::new(login_data).unwrap();

    if let Some(resp) = custom_resp {
        let resp = Response::parse(
            resp.to_string(),
            chrono::Local::now().naive_local(),
        )
        .unwrap();
        gs.update(resp).unwrap();
    }

    println!("\n=== Fight against Alexander Dybala ===");

    if let Some(fight) = &gs.last_fight {
        for (j, sf) in fight.fights.iter().enumerate() {
            println!("--- SingleFight {j} ---");
            for (k, action) in sf.actions.iter().enumerate() {
                println!(
                    "  actions[{k}]: actor={}, action={:?}, outcome={:?}, \
                     target_hp={}, actor_hp={:?}, minion={:?}/{:?}",
                    action.acting_id,
                    action.action,
                    action.outcome,
                    action.other_new_life,
                    action.actor_life,
                    action.actor_minion,
                    action.opponent_minion,
                );
            }
        }
    }

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

    /// Searches for values, that matches the given regex
    #[arg(long)]
    search: Option<Regex>,

    /// Only print keys during the search, that match this regex
    #[arg(long)]
    search_key: Option<Regex>,
}
