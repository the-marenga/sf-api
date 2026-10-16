//! Replays every response in `cache/` over every cached login state and
//! reports parsing errors and warnings.
//!
//! ```text
//! cargo run --example replay_cache
//! cargo run --example replay_cache -- --strict
//! ```
//!
//! Without `--strict` only parsing errors fail the run, with it every
//! `WARN`/`ERROR` log does. `cache/` is not in the repository, so without
//! cached data this exits successfully.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use sf_api::{gamestate::GameState, response::Response};

/// Every `WARN`/`ERROR` record of the run, in the order they were logged.
static LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

/// Records the warnings & errors of the run so main can report them.
struct Capture;

impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Some(logs) = LOGS.get()
            && let Ok(mut logs) = logs.lock()
        {
            logs.push(format!("{}: {}", record.level(), record.args()));
        }
    }

    fn flush(&self) {
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strict = args.iter().any(|arg| arg == "--strict");
    let cache_dir = args
        .iter()
        .find(|arg| !arg.starts_with("--"))
        .map_or_else(|| PathBuf::from("cache"), PathBuf::from);

    LOGS.set(Mutex::new(Vec::new())).ok();
    if log::set_boxed_logger(Box::new(Capture)).is_err() {
        eprintln!("could not install the log capture");
        std::process::exit(1);
    }
    log::set_max_level(log::LevelFilter::Warn);

    let logins = files_with_extension(&cache_dir, "login");
    if logins.is_empty() {
        println!(
            "no cached logins in {}, nothing to replay",
            cache_dir.display()
        );
        return;
    }
    let responses = files_with_extension(&cache_dir, "response");
    println!(
        "replaying {} cached responses over {} cached logins from {}",
        responses.len(),
        logins.len(),
        cache_dir.display()
    );

    let mut errors = Vec::new();
    let mut replays = 0_usize;
    for login in &logins {
        let state = match read_cached(login) {
            Ok(state) => state,
            Err(err) => {
                errors.push(format!("{}: {err}", login.display()));
                continue;
            }
        };
        let mut gamestate = match GameState::new(state) {
            Ok(gamestate) => gamestate,
            Err(err) => {
                errors.push(format!("{}: {err}", login.display()));
                continue;
            }
        };

        let mut ok = 0_usize;
        for file in &responses {
            let response = match read_cached(file) {
                Ok(response) => response,
                Err(err) => {
                    errors.push(format!("{}: {err}", file.display()));
                    continue;
                }
            };
            replays += 1;
            match gamestate.update(&response) {
                Ok(()) => ok += 1,
                Err(err) => errors.push(format!(
                    "{} + {}: {err}",
                    login.display(),
                    file.display()
                )),
            }
        }
        println!("  {}: {ok} ok", login.display());
    }

    let warnings = warning_counts();
    let warning_count: usize = warnings.iter().map(|(count, _)| count).sum();
    println!(
        "\n{replays} replays, {} parsing errors, {warning_count} warnings \
         ({warnings_len} kinds)",
        errors.len(),
        warnings_len = warnings.len(),
    );
    for (count, sample) in &warnings {
        println!("  {count:5} x {}", first_chars(sample, 120));
    }
    for error in &errors {
        eprintln!("error: {error}");
    }

    if !errors.is_empty() {
        eprintln!("\nfailed: {} parsing errors", errors.len());
        std::process::exit(1);
    }
    if strict && warning_count > 0 {
        eprintln!("\nfailed: {warning_count} warnings (--strict)");
        std::process::exit(1);
    }
    println!("\nok");
}

/// Reads and deserializes one cached response file.
fn read_cached(path: &Path) -> Result<Response, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("could not read: {err}"))?;
    serde_json::from_str(&raw)
        .map_err(|err| format!("invalid cached response: {err}"))
}

/// All files in `dir` with the given extension, sorted for stable output.
fn files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    files.sort();
    files
}

/// Groups the recorded warnings & errors by their kind, most frequent first.
fn warning_counts() -> Vec<(usize, String)> {
    let mut kinds: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for log in recorded_logs() {
        let entry = kinds.entry(collapse_digits(&log)).or_insert((0, log));
        entry.0 += 1;
    }
    let mut counts: Vec<(usize, String)> = kinds.into_values().collect();
    counts.sort_by_key(|(count, _)| std::cmp::Reverse(*count));
    counts
}

/// The recorded warnings & errors.
fn recorded_logs() -> Vec<String> {
    LOGS.get()
        .and_then(|logs| logs.lock().ok().map(|logs| logs.clone()))
        .unwrap_or_default()
}

/// Replaces every run of digits with a single `#`, so that messages that only
/// differ in an id or a value group together.
fn collapse_digits(message: &str) -> String {
    let mut key = String::with_capacity(message.len());
    let mut in_digits = false;
    for char in message.chars() {
        if char.is_ascii_digit() {
            if !in_digits {
                key.push('#');
                in_digits = true;
            }
        } else {
            in_digits = false;
            key.push(char);
        }
    }
    key
}

/// The first `max` characters of `message`, for one line per warning kind.
fn first_chars(message: &str, max: usize) -> String {
    let mut res: String = message.chars().take(max).collect();
    if res.len() < message.len() {
        res.push('…');
    }
    res
}
