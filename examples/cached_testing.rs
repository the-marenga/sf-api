use clap::Parser;
use regex::Regex;
use sf_api::{gamestate::GameState, session::*, sso::SFAccount};

#[tokio::main]
pub async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();

    let custom_resp: Option<&str> = None;

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

    if let Some(_resp) = custom_resp {
        // Not used in scan mode
    }

    use sf_api::gamestate::character::Class;
    use sf_api::gamestate::social::CombatMessageType;
    use std::collections::BTreeSet;

    // Get arena fight msg_ids from the game state's combat log
    let arena_fights: Vec<u32> = gs
        .mail
        .combat_log
        .iter()
        .filter(|e| matches!(e.battle_type, CombatMessageType::Arena))
        .map(|e| e.msg_id as u32)
        .collect();
    eprintln!("Found {} arena fight msg_ids", arena_fights.len());

    for msg_id in &arena_fights {
        let cmd = sf_api::command::Command::PlayerCombatLogView { msg_id: *msg_id };

        let resp = session.send_command_raw(&cmd).await.unwrap();

        // Check if this response has actual fight data
        let has_fight = resp.values().iter().any(|(key, _val)| {
            let k = *key;
            k.starts_with("fight") && k != "fightresult"
        });
        if !has_fight {
            continue;
        }

        gs.update(resp).unwrap();

        // Collect data from the last fight
        if let Some(fight) = &gs.last_fight {
            for sf in &fight.fights {
                let enemy_name = sf
                    .fighter_b
                    .as_ref()
                    .and_then(|f| f.name.clone())
                    .unwrap_or_default();
                let enemy_class = sf
                    .fighter_b
                    .as_ref()
                    .map(|f| f.class)
                    .unwrap_or(Class::Warrior);

                let mut action_types: BTreeSet<u32> = BTreeSet::new();
                let mut pos1_vals: BTreeSet<i64> = BTreeSet::new();
                let mut pos4_vals: BTreeSet<i64> = BTreeSet::new();

                for action in &sf.actions {
                    // Extract raw action type from the parsed action
                    let raw = match action.action {
                        sf_api::gamestate::arena::FightActionType::Attack => 0,
                        sf_api::gamestate::arena::FightActionType::Crit => 1,
                        sf_api::gamestate::arena::FightActionType::MushroomCatapult => 2,
                        sf_api::gamestate::arena::FightActionType::Summon => 11,
                        sf_api::gamestate::arena::FightActionType::MinionAttack => 12,
                        sf_api::gamestate::arena::FightActionType::BattleMageFireball => 10,
                        sf_api::gamestate::arena::FightActionType::Revive => 14,
                        sf_api::gamestate::arena::FightActionType::AssassinMainHand => 100,
                        sf_api::gamestate::arena::FightActionType::AssassinOffHand => 101,
                        sf_api::gamestate::arena::FightActionType::Unknown(v) => v,
                        _ => 999,
                    };
                    action_types.insert(raw);
                }

                // Helper to get raw state value
                let state_raw = |s: &sf_api::gamestate::arena::FighterState| -> i64 {
                    match s {
                        sf_api::gamestate::arena::FighterState::Normal => 0,
                        sf_api::gamestate::arena::FighterState::BearForm => 10,
                        sf_api::gamestate::arena::FighterState::DefensiveStance => 20,
                        sf_api::gamestate::arena::FighterState::Frenzy => 30,
                        sf_api::gamestate::arena::FighterState::Unknown(v) => *v,
                    }
                };
                for action in &sf.actions {
                    pos1_vals.insert(state_raw(&action.actor_state));
                    pos4_vals.insert(state_raw(&action.defender_state));
                }

                println!(
                    "msg_id={msg_id} enemy={enemy_name:30} class={enemy_class:?}: \
                     actions={:?} pos1={:?} pos4={:?}",
                    action_types.iter().collect::<Vec<_>>(),
                    pos1_vals.iter().collect::<Vec<_>>(),
                    pos4_vals.iter().collect::<Vec<_>>(),
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
