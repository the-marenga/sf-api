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
// To elaborate a bit:
// Option<NonZeroU32> -> Is wrong, as a PlayerId is > 0, but would be more
// correct the way it is used currently
//
// NonZeroU32 -> This would be correct, but a pain to work with, as it basically
// eliminates the option to derive Default on anything that uses this
//
// Both options are suboptimal and require this to become a struct. Longterm
// this would be a good thing I think, but for now, I will just keep the 0.01%
// error rate here
pub type PlayerId = u32;
