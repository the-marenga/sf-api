//! A simple API to send commands to the Shakes & Fidget servers and parse their
//! responses into characters.

#![warn(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_debug_implementations,
    clippy::pedantic,
    missing_docs
)]
#![allow(clippy::module_name_repetitions, clippy::too_many_lines)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::indexing_slicing))]
#![deny(unsafe_code)]

/// Commands that can be sent to Shakes & Fidget game servers.
pub mod command;
/// Error and Result types used across this crate.
pub mod error;
/// Types representing the parsed game state (character, inventory, dungeons, etc.).
pub mod gamestate;
/// Parsing and decoding utilities for raw server response strings.
pub mod response;

/// Miscellaneous helper functions kept for backwards compatibility.
pub mod misc {
    pub use crate::response::{from_sf_string, to_sf_string};
}

pub use error::{Result, SFError};
/// Network session handling for communicating with game servers.
#[cfg(feature = "session")]
pub mod session;
/// Battle simulation for characters, dungeons, and pets.
#[cfg(feature = "simulation")]
pub mod simulate;
/// Single Sign-On (SSO) authentication and account management.
#[cfg(feature = "sso")]
pub mod sso;



/// This is the numerical id of a player on a server. Note that in rare edge
/// cases this might be 0 (you are the first person to unlock the Dungeon. Who
/// do you fight?), but you can almost always expect this to be > 0
/// wherever found
pub type PlayerId = u32;

#[cfg(feature = "session")]
pub use session::SimpleSession;
