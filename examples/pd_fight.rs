use sf_api::{gamestate::GameState, session::Response};

fn main() {
    let body = "fightversion:2&fightheader.fighters:0/0/0/0/1/11162/marenga/38/76284/76284/116/646/74/489/215/6/102/102/3/102/2/4/7/0/0/6/1/12/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/14141/Tomi Lee/31/31488/31488/233/606/233/246/228/1/102/101/2/108/7/2/4/0/0/7/1/12/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0&fightequipment:1/19/4/0/0/0/0/1/0/0/1/24/2/0/0/0/0/1/0/0&fightdecoration:0/0/0/0&externaltoolequipment:43/85/0/0/33/87/0/0&fight.r:14141/0/1/0/0/31488/72628/0/0/11162/0/0/0/0/72628/28146/0/0/14141/0/0/0/0/28146/70696/0/0/11162/0/0/0/0/70696/25787/0/0/14141/0/18/0/0/25787/65080/0/1/3/1/3/11162/0/1/4/0/65080/25787/1/3/1/3/0/14141/0/20/0/0/25787/60644/0/1/3/1/2/14141/0/1/0/0/25787/52216/0/1/3/1/2/11162/0/17/0/0/52216/22145/1/3/1/2/1/3/1/3/14141/0/19/0/0/22145/50381/1/3/1/3/1/3/1/1/14141/0/0/4/0/22145/50381/1/3/1/3/1/3/1/1/11162/0/19/0/0/50381/15584/1/3/1/1/1/3/1/2/11162/0/0/4/0/50381/15584/1/3/1/1/1/3/1/2/14141/0/19/0/0/15584/48811/1/3/1/2/1/3/1/0/14141/0/0/4/0/15584/48811/1/3/1/2/0/11162/0/20/0/0/48811/4895/0/1/3/1/1/11162/0/1/0/0/48811/-10241/0/1/3/1/1/&winnerid:11162";

    let login_cache = std::fs::read_to_string("cache/bruhbruh.login").unwrap();
    let login_data: Response = serde_json::from_str(&login_cache).unwrap();
    let mut gs = GameState::new(login_data).unwrap();

    let resp = Response::parse(body.to_string(), chrono::Local::now().naive_local()).unwrap();
    gs.update(resp).unwrap();

    if let Some(fight) = &gs.last_fight {
        for (j, sf) in fight.fights.iter().enumerate() {
            println!("--- SingleFight {j} ---");
            for (k, action) in sf.actions.iter().enumerate() {
                println!("  actions[{k}]: actor={}, action={:?}, outcome={:?}, target_hp={}, actor_hp={:?}, effect={:?}/{:?}, state={:?}/{:?}",
                    action.acting_id, action.action, action.outcome, action.other_new_life, action.actor_life,
                    action.actor_effect, action.opponent_effect, action.actor_state, action.defender_state);
            }
        }
    }
}
