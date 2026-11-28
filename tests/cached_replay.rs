use sf_api::{
    gamestate::{GameState, rewards::Sale},
    response::Response,
};

#[test]
fn replays_an_anonymized_cached_response() {
    let login: Response =
        serde_json::from_str(include_str!("fixtures/minimal.login")).unwrap();
    let mut state = GameState::new(login).unwrap();

    assert_eq!(state.character.name, "FixtureHero");
    assert_eq!(state.character.level, 42);

    let response: Response =
        serde_json::from_str(include_str!("fixtures/minimal.response"))
            .unwrap();
    state.update(response).unwrap();

    assert_eq!(state.character.mushrooms, 10);
    assert_eq!(state.character.silver, 20);
    assert_eq!(state.tavern.quicksand_glasses, 4);
    assert_eq!(state.specials.wheel.lucky_coins, 3);
    assert_eq!(state.blacksmith.as_ref().unwrap().metal, 7);
    assert!(state.specials.events.sales[Sale::BlackFriday]);

    // Keep the full parsed state serializable as cache-replay tooling evolves.
    serde_json::to_value(state).unwrap();
}
