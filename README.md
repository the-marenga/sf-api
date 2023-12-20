# S&F API 🧙🏽‍♂️

## Overview

This is an unofficial work in progress API to talk to the Shakes & Fidget Servers. 

The most basic example on how to use this would be:

```Rust
    // The session is what manages any data relevant to communicating with the 
    // server accross commands
    let mut session = CharacterSession::new(
        "username",
        "password",
        ServerConnection::new("s1.sfgame.de").unwrap(),
    );

    // The login response will contain information about our character and the 
    // server
    let login_respone = session.login().await.unwrap();

    // The game state is what we can use to look at all the data from the 
    // server in a more comprehensible way. 
    let mut game_state = GameState::new(login_respone).unwrap();

    // Now we are all set to do whatever we want.
    let best_description = "I love sushi!".to_string();

    // Just like above we get a response. This time however, it will only 
    // contain a partial response with things, that might have changed
    let response = session
        .send_command(&Command::SetDescription {
            description: best_description.clone(),
        })
        .await
        .unwrap();

    // As such, we should use update on our existing game state
    game_state.update(response).unwrap();

    // Lets make sure the server actually did what we told him
    assert!(game_state.character.description == best_description);

    println!("YAY, it worked! 🎉🍣🍣🍣🎉");
```

If you use a single sign-on S&F Account, you can use it like this:

```Rust
    let account = SFAccount::login(
        "username".to_string(),
        "password".to_string()
    ).await.unwrap();

    for mut session in account.characters().await.unwrap().into_iter().flatten()
    {
        // You can use the sessions, that the account returns like
        // a normal (logged out) session now
        let response = session.login().await.unwrap();
        let mut game_state = GameState::new(response).unwrap();
    }

```

## Installation

You just need to run the following command in your [Rust](https://rustup.rs/) project:

```
cargo add --git https://github.com/the-marenga/sf-api
``` 
### Windows Builds

Windows does not come with the requires openssl libraries installed, that are required to encrypt/decrypt server requests. There are a few ways to fix this, but easiest way should be:
- Install, bootstrap and integrate [vcpkg](https://vcpkg.io/en/getting-started)
- Run `./vcpkg.exe install openssl:x64-windows-static-md` to install the required openssl version.
- You may need to restart your IDE/Terminal, but after that builds should just work

> Note: I may switch to something, that works for windows out of the box later

## Guidelines

Here are a few things you should note before getting your account banned:

1. NEVER run anything that sends commands to the server in an infinite loop without a delay. This will instantly get you the Nr. 1 danger spot for your account/ip on any monitoring system that monitors traffic and depending on your connection and number of accounts might be classified as a DDOS attack. Just put an async sleep somewhere and bail after a set amount of tries. You have been warned
2. The normal web client regularely sends an update command to the server. This is what signals to the server, that your account is still there and that your last active time in the guild screen should be updated. Note that logging in somehow does not update this time on its own. As you might guess, it would be weird to have an account constantly active without sending any update commands, so just send them sometimes.
3. This library does not check to which API calls your charcter should have access to. If you try to equip your not yet unlocked companions from your not yet unlocked fortress chest, that is your fault. The command enum stop you from shooting yourself in the foot by enforcing valid inputs via the rust type system, but any logic above that is not worth the tradeoff in terms of perf/complexity/false positive errors.
4. Similar to the previous point, the parsing of responses does not check if the response is the correct response to your request apart from handling server errors. If the server sends you a hall of fame response, when you send a finish quest command, that is a server error (that I have never seen), which would be silently ignored. Before you then proceed to try and start a new quest for the next 12 hours on loop, you should just sanity check if your command worked. Especially time sensitive stuff accross timezones might surprise you otherwise.
5. The raw S&F API starts indexing at 1 instead of 0. This is pretty unintuitive and error prone, if you want to just use the index of an existing Vec/Array as a command input. As such, I made the decision to hide this fact and manually increment the provided index in commands by 1. This also makes it impossible to provide too low inputs, but Rusts type system is sadly not able to provide an upper bound to values, so you can still provide too large values. Try not to do that, as this will be an error not present in any official release and could easily lead to someone from the dev team taking an interest in your account, if they want to investigate previously unseen errors.

## Performance

Performace should not matter to you, as you are not supposed to run this library on a scale, where you have to think about this. Disregarding this fact, this library is build with high scalabillity and low resource usage in mind. Parsing the login gamestate will take < 1ms on my machine with full updates after that taking < 100µs. 

I have tried to minimize allocations, by reusing previous containers. In addition, `Hashmaps<U, T>`s are largely replaced by `[T;K]`s, where `U as usize` is used to index into the vec (largely abstracted via some get function). 

Responses are just the raw html response body with a ~`HashMap<&str,&str>`, that references into that. This is perfectly safe, saves dozents of allocations per request and keeps everything in one hot place for cache purposes.

Everything is parsed into the exact datatype, that is expected, which also catches weird, or unexpected errors compared to just i64ing every int. Note that I do not expect to maintain this forever (I do not play this game to begin with), so a lot of this is shown as log warnings and defaulting to some value, instead of returning an error. This way you will not get hard stuck, just because the mushroom price of an item somewhere is negative. Feel free to change warn! to panic! in the misc. functions to change this behaviour.

## Rust Features

This crate has support for `serde` to (de)serialize the character state and the S&F Account (`sso`) behind the respective feature flags. Note that `sso` depends on the serde crate internally to talk to the server via json.

## Design

The API has been designed to keep session and character state seperate. Why? Mainly because I can serialize/deserialize responses and "replay" them for unique things and to reduce server requests. 

In addition, this architecture is a bit easier to use, when you have multiple things, that want to use the session, as they do not have to wait for the update to finish before sending again.

> Note that I do not think that this is perfect. Especially for basic usecases, this is more annoying, than helpful. I will likely add a unified version, at some point 

There are roughly ~500 properties, that get parsed. This is such a huge amount of data, that I have opted to just give you raw access to these fields in the GameState instead of providing get() functions for all of these. This means you can create invalid gamestates with a mutable reference if you want, but as far as I am concerned, that would be your bug, not mine. 

> I might consider using something like [derive_getters](https://docs.rs/derive-getters/latest/derive_getters/) in the future, but I dont really care about this issue and it would increase compile times, so for now you just access the fields

## TODO

This is a list of things still on my agenda:
- [X] Make command positions correct/consistent
- [X] All flags 
- [X] Shell player command
- [X] Better pet parsing
- [X] Hall of Fame pets commands
- [X] Scrapbook parsing
- [ ] Add more get() functions for stuff, that is in arrays
- [ ] consider a `into_list_with()` to avoid allocations there
- [ ] Remove the main and replace that with actual tests
- [ ] Better attribute debug
- [ ] Achievement names (enum)
- [ ] Understand portal enemy level
- [ ] Hall of knights parsing
- [ ] Explain some of the interactions/limits of commands
- [ ] Check if register with mail is still possible
