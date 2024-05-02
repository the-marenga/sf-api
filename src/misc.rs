use std::{fmt::Display, str::FromStr};

use aho_corasick::AhoCorasick;
use log::error;
use once_cell::sync::Lazy;

use crate::error::SFError;

pub(crate) const HASH_CONST: &str = "ahHoj2woo1eeChiech6ohphoB7Aithoh";

pub(crate) fn sha1_hash(val: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(val.as_bytes());
    let hash = hasher.finalize();
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

pub(crate) trait CGet<T: Copy + std::fmt::Debug> {
    fn cget(&self, pos: usize, name: &'static str) -> Result<T, SFError>;
}

pub(crate) trait CCGet<T: Copy + std::fmt::Debug + Display, I: TryFrom<T>> {
    fn csiget(
        &self,
        pos: usize,
        name: &'static str,
        def: I,
    ) -> Result<I, SFError>;
    fn cwiget(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<Option<I>, SFError>;
}

pub(crate) trait CSGet<T: FromStr> {
    fn cfsget(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<Option<T>, SFError>;
}

impl<T: FromStr> CSGet<T> for [&str] {
    fn cfsget(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<Option<T>, SFError> {
        let raw = self.cget(pos, name)?;
        Ok(warning_from_str(raw, name))
    }
}

impl<T: Copy + std::fmt::Debug + Display, I: TryFrom<T>> CCGet<T, I> for [T] {
    fn csiget(
        &self,
        pos: usize,
        name: &'static str,
        def: I,
    ) -> Result<I, SFError> {
        let raw = self.get(pos).copied().ok_or_else(|| {
            SFError::TooShortResponse {
                name,
                pos,
                array: format!("{:?}", self),
            }
        })?;

        Ok(soft_into(raw, name, def))
    }

    fn cwiget(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<Option<I>, SFError> {
        let raw = self.get(pos).copied().ok_or_else(|| {
            SFError::TooShortResponse {
                name,
                pos,
                array: format!("{:?}", self),
            }
        })?;
        Ok(warning_try_into(raw, name))
    }
}

impl<T: Copy + std::fmt::Debug + Display> CGet<T> for [T] {
    fn cget(&self, pos: usize, name: &'static str) -> Result<T, SFError> {
        self.get(pos)
            .copied()
            .ok_or_else(|| SFError::TooShortResponse {
                name,
                pos,
                array: format!("{:?}", self),
            })
    }
}

pub(crate) fn update_enum_map<
    B: Default + TryFrom<i64>,
    A: enum_map::Enum + enum_map::EnumArray<B>,
>(
    map: &mut enum_map::EnumMap<A, B>,
    vals: &[i64],
) {
    for (map_val, val) in map.as_mut_slice().iter_mut().zip(vals) {
        *map_val = soft_into(*val, "attribute val", B::default());
    }
}
