#![warn(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_debug_implementations,
    clippy::pedantic,
    // missing_docs
)]
#![allow(
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::field_reassign_with_default,
    clippy::match_bool
)]
#![deny(unsafe_code)]

pub mod command;
pub mod error;
pub mod gamestate;
pub mod misc;
pub mod response;
#[cfg(feature = "session")]
pub mod session;
pub mod simulate;
#[cfg(feature = "sso")]
pub mod sso;

/// This is the numerical id of a player on a server. Note that in rare edge
/// cases this might be 0 (you are the first person to unlock the Dungeon. Who
/// do you fight?), but you can almost always expect this to be > 0
/// wherever found
pub type PlayerId = u32;

#[cfg(feature = "session")]
pub use session::SimpleSession;
