use std::{fmt::Display, str::FromStr};

use aho_corasick::AhoCorasick;
use log::error;
use once_cell::sync::Lazy;

use crate::error::SFError;

pub(crate) const HASH_CONST: &str = "ahHoj2woo1eeChiech6ohphoB7Aithoh";

pub(crate) fn sha1_hash(val: &str) -> String {
    let mut sha1 = openssl::sha::Sha1::new();
    sha1.update(val.as_bytes());
    let hash = sha1.finish();
    let mut result = String::with_capacity(hash.len() * 2);
    for byte in hash.iter() {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

/// Converts a raw value into the appropriate type. If that is not possible,
/// a warning will be emmitted and the given default returned. This is useful
/// for stuff, that should not crash everything, when there is a weird value and
/// the silent failure of `as T`, or `unwrap_or_default()` would yield worse
/// results and/or no warning
#[inline]
pub(crate) fn soft_into<B: Display + Copy, T: TryFrom<B>>(
    val: B,
    name: &str,
    default: T,
) -> T {
    val.try_into().unwrap_or_else(|_| {
        log::warn!("Invalid value for {name} in server response: {val}");
        default
    })
}

/// Tries to convert val to T. If that fails a warning is emitted and none is
/// returned
#[inline]
pub(crate) fn warning_try_into<B: Display + Copy, T: TryFrom<B>>(
    val: B,
    name: &str,
) -> Option<T> {
    val.try_into().ok().or_else(|| {
        log::warn!("Invalid value for {name} in server response: {val}");
        None
    })
}

/// Converts the value using the function. If that fails, a warning is emitted
/// and None is returned
#[inline]
pub(crate) fn warning_parse<T, F, V: Display + Copy>(
    val: V,
    name: &str,
    conv: F,
) -> Option<T>
where
    F: Fn(V) -> Option<T>,
{
    conv(val).or_else(|| {
        log::warn!("Invalid value for {name} in server response: {val}");
        None
    })
}

#[inline]
pub(crate) fn warning_from_str<T: FromStr>(val: &str, name: &str) -> Option<T> {
    val.parse().ok().or_else(|| {
        log::warn!("Invalid value for {name} in server response: {val}");
        None
    })
}

/// Converts a  s&f string from the server to their original unescaped
/// representation
pub(crate) fn from_sf_string(val: &str) -> String {
    pattern_replace::<true>(val)
}

/// Makes a user controlled string, like the character description safe to use
/// in a request
pub(crate) fn to_sf_string(val: &str) -> String {
    pattern_replace::<false>(val)
}

/// Calling .replace() a bunch of times is bad, as that generates a bunch of
/// strings. regex!() -> replace_all()  would be better, as that uses cow<>
/// irrc, but we can replace pattern with a linear search an one string, using
/// this extra crate. We call this function a bunch, so optimizing this is
/// probably worth it
fn pattern_replace<const FROM: bool>(str: &str) -> String {
    static A: Lazy<(AhoCorasick, &'static [&'static str; 11])> =
        Lazy::new(|| {
            let l = sf_str_lookups();
            (aho_corasick::AhoCorasick::new(l.0).unwrap(), l.1)
        });

    static B: Lazy<(AhoCorasick, &'static [&'static str; 11])> =
        Lazy::new(|| {
            let l = sf_str_lookups();
            (aho_corasick::AhoCorasick::new(l.1).unwrap(), l.0)
        });

    let (from, to) = match FROM {
        true => A.clone(),
        false => B.clone(),
    };
    let mut wtr = vec![];
    from.try_stream_replace_all(str.as_bytes(), &mut wtr, to)
        .expect("stream_replace_all failed");

    match String::from_utf8(wtr) {
        Ok(res) => res,
        Err(_) => {
            error!("replace generated invalid utf8");
            String::new()
        }
    }
}

pub(crate) fn parse_vec<B: Display + Copy + std::fmt::Debug, T, F>(
    data: &[B],
    name: &'static str,
    func: F,
) -> Result<Vec<T>, SFError>
where
    F: Fn(B) -> Option<T>,
{
    data.iter()
        .map(|a| {
            func(*a).ok_or_else(|| {
                SFError::ParsingError(name, format!("{:?}", data))
            })
        })
        .collect()
}

/// The mappings to convert between a normal and an sf string
const fn sf_str_lookups(
) -> (&'static [&'static str; 11], &'static [&'static str; 11]) {
    (
        &[
            "$b", "$c", "$P", "$s", "$p", "$+", "$q", "$r", "$C", "$S", "$d",
        ],
        &["\n", ":", "%", "/", "|", "&", "\"", "#", ",", ";", "$"],
    )
}
