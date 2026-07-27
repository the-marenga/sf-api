use clap::Parser;
use regex::Regex;
use sf_api::{gamestate::GameState, session::*, sso::SFAccount};

#[tokio::main]
pub async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();

    let custom_resp: Option<&str> = Some("fightresult.fortresspillagerv1:1/1/0/0/0/1/0/223105/222922/0/0/0/0/0/0/0/0/0/0/0/0/770/6012/1/1/0/0&fightversion:2&fightheader1.fighters:8/0/0/0/1/710/-710/40/133250/133250/650/10/10/650/415/-710/1/1/0/0/0/0/0/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/740/-740/10/33000/33000/200/60/60/600/0/-740/1/1/0/0/0/0/0/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0&fightequipment1:1/15/1/0/0/2/1/1/0/0/1/2/1/0/0/0/0/1/0/0&fightdecoration1:0/0/0/0&externaltoolequipment1:0/0/0/0/0/0/0/0&fight1.r:710/0/1/0/0/133250/22306/0/0/740/0/0/0/0/22306/133103/0/0/710/0/0/0/0/133103/16221/0/0/740/0/0/3/0/16221/133103/0/0/710/0/0/0/0/133103/6203/0/0/740/0/0/0/0/6203/132836/0/0/710/0/1/0/0/132836/-18685/0/0/&winnerid1.s:710&&fightadditionalplayers.r:");

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

    println!("\n=== Fortress Fight ===");

    // Dump fighter info
    if let Some(fight) = &gs.last_fight {
        println!("winner_id: {:?}, has_player_won: {}, extra: {:?}",
            fight.fights.first().map(|f| f.winner_id),
            fight.has_player_won,
            fight.extra,
        );
        for (j, sf) in fight.fights.iter().enumerate() {
            println!("--- SingleFight {j} ---");
            if let Some(fa) = &sf.fighter_a {
                println!("  fighter_a: type={:?} id={} name={:?} level={} life={}",
                    fa.typ, fa.id, fa.name, fa.level, fa.life);
            }
            if let Some(fb) = &sf.fighter_b {
                println!("  fighter_b: type={:?} id={} name={:?} level={} life={}",
                    fb.typ, fb.id, fb.name, fb.level, fb.life);
            }
            for (k, action) in sf.actions.iter().enumerate() {
                println!(
                    "  actions[{k}]: actor={}, action={:?}, outcome={:?}, \
                     target_hp={}, actor_hp={:?}",
                    action.acting_id,
                    action.action,
                    action.outcome,
                    action.other_new_life,
                    action.actor_life,
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
