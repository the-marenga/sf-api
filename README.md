# S&F API 🧙🏽‍♂️

[![crates.io](https://img.shields.io/crates/v/sf-api.svg)](https://crates.io/crates/sf-api) ![Build Status](https://img.shields.io/github/actions/workflow/status/the-marenga/sf-api/rust.yml?branch=main) ![Licence](https://img.shields.io/crates/l/sf-api) [<img src='https://storage.ko-fi.com/cdn/kofi3.png?v=3' height='20'>](https://ko-fi.com/J3J0ULD4J)

## Overview

This is an unofficial work in progress API to talk to the Shakes & Fidget Servers.

The most basic example on how to use this would be:

```Rust
    // First thing we have to do is login
    let mut session = sf_api::SimpleSession::login(
        "username",
        "password",
        "f1.sfgame.net"
    ).await.unwrap();

    // If everything went well, we are able to look at the current state
    let gs = session.game_state().unwrap();
    println!("Our current description is: {}", gs.character.description);

    //  Lets do something like changing the description as an example
    let new_description = "I love sushi!".to_string();
    let gs = session
        .send_command(Command::SetDescription {
            description: new_description.clone(),
        })
        .await
        .unwrap();
    // After successfully sending a command, the server will return the
    // gamestate so that we can check what changed:
    assert!(gs.character.description == new_description);
    println!("YAY, it worked! 🎉🍣🍣🍣🎉");
```

If you use a single sign-on S&F Account, you can use it like this:

```Rust
    let sessions = SimpleSession::login_sf_account(
        "username",
        "password"
    ).await.unwrap();

    for session in sessions {
        // You can use the sessions, that the account returns like
        // a normal (logged out) session now
        let gs = session.send_command(Command::UpdatePlayer).await.unwrap();
        // ...
    }

```

The `SimpleSession` is not optimal for more complex usecases. For these, have a
look at `Session::new()` & `GameState::new()` to handle session and gamestate
seperately

## Installation

You just need to run the following command in your [Rust](https://rustup.rs/) project:

```
cargo add sf-api
```

## Guidelines

Here are a few things you should note before getting your account banned:

1. Never send commands to the server in an infinite loop without a delay
2. You should send an `UpdatePlayer` command every once in a while.
3. Make sure you have access to the commands you are trying to send.
4. Always check if the thing you expected happened after sending a command.
5. You can assume, that every index in a command starts at 0

## Performance

Performace should not matter to you, as you are not supposed to run this library on a scale, where you have to think about this. Disregarding this fact, this library is build with high scalabillity and low resource usage in mind. Parsing the login gamestate will take < 1ms on my machine with full updates after that taking < 100µs.

Everything is parsed into the exact datatype, that is expected, which also catches weird, or unexpected errors compared to just i64ing every int. A lot of these conversion errors are shown as log warnings and defaulting to some value, instead of returning an error. This way you will not get hard stuck, just because the mushroom price of an item somewhere is negative

## Rust Features

This crate has support for `serde` to (de)serialize the character state and the S&F Account (`sso`) behind the respective feature flags. Note that `sso` depends on the serde crate internally to talk to the server via json.

## Misc.

There are 500+ properties, that get parsed. This is such a huge amount of data, that I have opted to just give you raw access to these fields in the GameState instead of providing get() functions for all of these. This means you can create invalid gamestates with a mutable reference if you want, but as far as I am concerned, that would be your bug, not mine.
