#![warn(
    missing_debug_implementations,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::pedantic,
    // missing_docs
)]
#![allow(
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::field_reassign_with_default
)]
#![deny(unsafe_code)]

use std::{borrow::Borrow, time::Duration};

use command::Command;
use error::SFError;
use gamestate::GameState;
use session::{ServerConnection, Session};

pub mod command;
pub mod error;
pub mod gamestate;
pub mod misc;
pub mod session;
#[cfg(feature = "sso")]
pub mod sso;

/// This is the numerical id of a player on a server. Note that in rare edge
/// cases this might be 0 (you are the first person to unlock the Dungeon. Who
/// do you fight?), but you can almost always expect this to be > 0
/// whereever found
pub type PlayerId = u32;

#[derive(Debug)]
pub struct SimpleSession {
    session: Session,
    gamestate: Option<GameState>,
}

impl SimpleSession {
    async fn short_sleep() {
        tokio::time::sleep(Duration::from_millis(fastrand::u64(1000..2000)))
            .await;
    }

    /// Creates a new `SimpleSession`, by logging in a normal S&F character
    ///
    /// # Errors
    /// Have a look at `send_command` for a full list of possible errors
    pub async fn login(
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<Self, SFError> {
        let connection = ServerConnection::new(server_url)
            .ok_or(SFError::ConnectionError)?;
        let mut session = Session::new(username, password, connection);
        let resp = session.login().await?;
        let gs = GameState::new(resp)?;
        Self::short_sleep().await;
        Ok(Self {
            session,
            gamestate: Some(gs),
        })
    }

    #[cfg(feature = "sso")]
    ///  Creates new `SimpleSession`s, by logging in the S&S SSO account and
    /// returning all the characters associated with the account
    ///
    /// # Errors
    /// Have a look at `send_command` for a full list of possible errors
    pub async fn login_sf_account(
        username: &str,
        password: &str,
    ) -> Result<Vec<Self>, SFError> {
        let acc =
            sso::SFAccount::login(username.to_string(), password.to_string())
                .await?;

        Ok(acc
            .characters()
            .await?
            .into_iter()
            .flatten()
            .map(|a| Self {
                session: a,
                gamestate: None,
            })
            .collect())
    }

    /// Returns a reference to the game state, if this `SimpleSession` is
    /// currently logged in
    #[must_use]
    pub fn game_state(&self) -> Option<&GameState> {
        self.gamestate.as_ref()
    }

    /// Returns a mutable reference to the game state, if this `SimpleSession`
    /// is currently logged in
    #[must_use]
    pub fn game_state_mut(&mut self) -> Option<&mut GameState> {
        self.gamestate.as_mut()
    }

    /// Sends the command and updates the gamestate with the response from the
    /// server. A mutable reference to the gamestate will be returned. If an
    /// error is encountered, the gamestate is cleared and the error will be
    /// returned. If you send a command after that, this function will try to
    /// login this session again, before sending the provided command
    ///
    /// # Errors
    /// - `EmptyResponse`: If the servers response was empty
    /// - `InvalidRequest`: If your response was invalid to send in some way
    /// - `ConnectionError`: If the command could not be send, or the response
    ///   could not successfully be received
    /// - `ParsingError`: If the response from the server was unexpected in some
    ///   way
    /// - `TooShortResponse` Similar to `ParsingError`, but specific to a
    ///   response being too short, which would normaly trigger a out of bound
    ///   panic
    /// - `ServerError`: If the server itself responded with an ingame error
    ///   like "you do not have enough silver to do that"
    #[allow(clippy::unwrap_used, clippy::missing_panics_doc)]
    pub async fn send_command<T: Borrow<Command>>(
        &mut self,
        cmd: T,
    ) -> Result<&mut GameState, SFError> {
        if self.gamestate.is_none() {
            let resp = self.session.login().await?;
            let gs = GameState::new(resp)?;
            self.gamestate = Some(gs);
            Self::short_sleep().await;
        }

        let resp = match self.session.send_command(cmd).await {
            Ok(resp) => resp,
            Err(err) => {
                self.gamestate = None;
                return Err(err);
            }
        };

        if let Some(gs) = &mut self.gamestate {
            if let Err(e) = gs.update(resp) {
                self.gamestate = None;
                return Err(e);
            }
        }

        Ok(self.gamestate.as_mut().unwrap())
    }
}
