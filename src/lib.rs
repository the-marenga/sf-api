#![warn(
    missing_debug_implementations,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::pedantic,
    clippy::missing_panics_doc
)]
#![allow(
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::field_reassign_with_default
)]
#![deny(unsafe_code)]
// clippy::pedantic,
//     missing_docs,

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

pub struct SimpleSession {
    session: Session,
    gamestate: Option<GameState>,
}

impl SimpleSession {
    async fn short_sleep() {
        tokio::time::sleep(Duration::from_millis(fastrand::u64(1000..2000)))
            .await;
    }

    pub async fn login_normal(
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

    pub fn game_state(&self) -> Option<&GameState> {
        self.gamestate.as_ref()
    }

    pub fn game_state_mut(&mut self) -> Option<&mut GameState> {
        self.gamestate.as_mut()
    }

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

        #[allow(clippy::unwrap_used)]
        let gs = self.gamestate.as_mut().unwrap();
        Ok(gs)
    }
}
