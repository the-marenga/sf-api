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
    clippy::redundant_closure_for_method_calls,
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::field_reassign_with_default,
    clippy::match_bool,
    clippy::chunks_exact_to_as_chunks
)]
#![deny(unsafe_code)]

pub mod command;
pub mod error;
pub mod gamestate;
pub mod misc;
pub mod response;
#[cfg(feature = "session")]
pub mod session;
#[cfg(feature = "simulation")]
pub mod simulate;
#[cfg(feature = "sso")]
pub mod sso;

#[cfg(test)]
#[macro_export]
macro_rules! parse_test {
    ($ty:ident ( $raw:expr $(, $arg:expr)* ), { $($field:ident: $expected:expr),+ $(,)? }) => {
        let vals = match $raw.try_into() {
            Ok(v) => v,
            Err(e) => panic!("invalid ResponseVals format: {e}"),
        };
        let parsed = match $ty::parse(&vals $(, $arg)*) {
            Ok(p) => p,
            Err(e) => panic!("parsing failed: {e}"),
        };
        $(
            assert_eq!(parsed.$field, $expected, "mismatch in field '{}'", stringify!($field));
        )+
    };
    (opt: $ty:ident ( $raw:expr $(, $arg:expr)* ), { $($field:ident: $expected:expr),+ $(,)? }) => {
        let vals = match $raw.try_into() {
            Ok(v) => v,
            Err(e) => panic!("invalid ResponseVals format: {e}"),
        };
        let parsed = match $ty::parse(&vals $(, $arg)*) {
            Ok(Some(p)) => p,
            Ok(None) => panic!("expected Some, found None"),
            Err(e) => panic!("parsing failed: {e}"),
        };
        $(
            assert_eq!(parsed.$field, $expected, "mismatch in field '{}'", stringify!($field));
        )+
    };
    ($ty:ident ( $raw:expr $(, $arg:expr)* ) => $expected:expr) => {
        let vals = match $raw.try_into() {
            Ok(v) => v,
            Err(e) => panic!("invalid ResponseVals format: {e}"),
        };
        let parsed = match $ty::parse(&vals $(, $arg)*) {
            Ok(p) => p,
            Err(e) => panic!("parsing failed: {e}"),
        };
        assert_eq!(parsed, $expected);
    };
    (update: $target:ident . $method:ident ( $raw:expr $(, $arg:expr)* ), { $($field:ident: $expected:expr),* $(,)? }) => {
        let vals = match $raw.try_into() {
            Ok(v) => v,
            Err(e) => panic!("invalid ResponseVals format: {e}"),
        };
        if let Err(e) = $target.$method(&vals $(, $arg)*) {
            panic!("update failed: {e}");
        }
        $(
            assert_eq!($target.$field, $expected, "mismatch in field '{}'", stringify!($field));
        )*
    };
    (update: $target:ident ( $raw:expr $(, $arg:expr)* ), { $($field:ident: $expected:expr),* $(,)? }) => {
        let vals = match $raw.try_into() {
            Ok(v) => v,
            Err(e) => panic!("invalid ResponseVals format: {e}"),
        };
        if let Err(e) = $target.update(&vals $(, $arg)*) {
            panic!("update failed: {e}");
        }
        $(
            assert_eq!($target.$field, $expected, "mismatch in field '{}'", stringify!($field));
        )*
    };
}

/// This is the numerical id of a player on a server. Note that in rare edge
/// cases this might be 0 (you are the first person to unlock the Dungeon. Who
/// do you fight?), but you can almost always expect this to be > 0
/// wherever found
pub type PlayerId = u32;

#[cfg(feature = "session")]
pub use session::SimpleSession;
