use clap::Parser;
use regex::Regex;
use sf_api::{gamestate::GameState, session::*};

#[tokio::main]
pub async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let _args = Args::parse();

    let custom_resp: &str = "fightresult.battlereward:1/1/0/455/0/99/0/20406/20053/0/0/0/0/0/0/0/0/0/0/0/0&battlerewarditem:0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0&ownplayersavecharacter:109244766/11162/0/38/77445/133260/2254/20053/6/102/102/3/102/2/4/7/0/0/6/1/12/0/0/790/43/85/0/53820/0/0/52/341/48/345/122/64/305/26/144/93/0/288/0/288/74/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/1/0/429/10680/0/551/0&fightversion:2&fightheader.fighters:0/0/0/0/1/11162/marenga/38/76284/76284/116/646/74/489/215/6/102/102/3/102/2/4/7/0/0/6/1/12/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/14141/Tomi Lee/31/31488/31488/233/606/233/246/228/1/102/101/2/108/7/2/4/0/0/7/1/12/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0&fightequipment:1/19/4/0/0/0/0/1/0/0/1/24/2/0/0/0/0/1/0/0&fightdecoration:0/0/0/0&externaltoolequipment:43/85/0/0/33/87/0/0&fight.r:14141/0/1/0/0/31488/72628/0/0/11162/0/0/0/0/72628/28146/0/0/14141/0/0/0/0/28146/70696/0/0/11162/0/0/0/0/70696/25787/0/0/14141/0/18/0/0/25787/65080/0/1/3/1/3/11162/0/1/4/0/65080/25787/1/3/1/3/0/14141/0/20/0/0/25787/60644/0/1/3/1/2/14141/0/1/0/0/25787/52216/0/1/3/1/2/11162/0/17/0/0/52216/22145/1/3/1/2/1/3/1/3/14141/0/19/0/0/22145/50381/1/3/1/3/1/3/1/1/14141/0/0/4/0/22145/50381/1/3/1/3/1/3/1/1/11162/0/19/0/0/50381/15584/1/3/1/1/1/3/1/2/11162/0/0/4/0/50381/15584/1/3/1/1/1/3/1/2/14141/0/19/0/0/15584/48811/1/3/1/2/1/3/1/0/14141/0/0/4/0/15584/48811/1/3/1/2/0/11162/0/20/0/0/48811/4895/0/1/3/1/1/11162/0/1/0/0/48811/-10241/0/1/3/1/1/&winnerid:11162&arena:1785178845/1/129117/15500/112937/1/1&dailytasklist:6/1/0/10/1/3/1/10/2/4/0/20/2/3/1/1/2/56/0/3/2/57/0/1/2/4/0/1/2/14/0/1/3/4/20/0/1/4&eventtasklist:77/0/10/1/76/0/10/1/75/0/10/1/57/0/10/1&deeds:0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/1/0/0/1/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0";

    let _commands: Vec<sf_api::command::Command> = vec![];

    // Use cached login data as base, then apply custom fight response
    let login_cache = std::fs::read_to_string("cache/bruhbruh.login").unwrap();
    let login_data: Response = serde_json::from_str(&login_cache).unwrap();
    let mut gs = GameState::new(login_data).unwrap();

    // Overwrite with our custom fight response
    let resp = Response::parse(
        custom_resp.to_string(),
        chrono::Local::now().naive_local(),
    )
    .unwrap();
    gs.update(resp).unwrap();

    // Dump the parsed fight actions
    if let Some(fight) = &gs.last_fight {
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
                     target_hp={}, actor_hp={:?}, effect={:?}/{:?}, \
                     state={:?}/{:?}",
                    action.acting_id,
                    action.action,
                    action.outcome,
                    action.other_new_life,
                    action.actor_life,
                    action.actor_effect,
                    action.opponent_effect,
                    action.actor_state,
                    action.defender_state,
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
