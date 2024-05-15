#![warn(
    clippy::pedantic,
    missing_debug_implementations,
    missing_docs,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::missing_panics_doc
)]
#![allow(
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::field_reassign_with_default
)]

pub mod command;
pub mod error;
pub mod gamestate;
pub mod misc;
pub mod session;
#[cfg(feature = "sso")]
pub mod sso;

/// This is the numerical id of a player on a server. Not that in rare edge
/// cases this might be 0 (you are the first person to unlock the Dungeon. Who
/// do you fight?), but you can almost always expect this to be > 0
/// whereever found
pub type PlayerId = u32;
