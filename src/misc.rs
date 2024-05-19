use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use aho_corasick::AhoCorasick;
use chrono::{DateTime, Local};
use enum_map::{Enum, EnumArray, EnumMap};
use log::{error, warn};
use num::FromPrimitive;
use once_cell::sync::Lazy;

use crate::{error::SFError, gamestate::ServerTime};

pub(crate) const HASH_CONST: &str = "ahHoj2woo1eeChiech6ohphoB7Aithoh";

pub(crate) fn sha1_hash(val: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(val.as_bytes());
    let hash = hasher.finalize();
    let mut result = String::with_capacity(hash.len() * 2);
    for byte in &hash {
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

/// Calling `.replace()` a bunch of times is bad, as that generates a bunch of
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

    let (from, to) = if FROM { A.clone() } else { B.clone() };
    let mut wtr = vec![];
    from.try_stream_replace_all(str.as_bytes(), &mut wtr, to)
        .expect("stream_replace_all failed");

    if let Ok(res) = String::from_utf8(wtr) {
        res
    } else {
        error!("replace generated invalid utf8");
        String::new()
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
            func(*a)
                .ok_or_else(|| SFError::ParsingError(name, format!("{data:?}")))
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

fn raw_cget<T: Copy + std::fmt::Debug>(
    val: &[T],
    pos: usize,
    name: &'static str,
) -> Result<T, SFError> {
    val.get(pos)
        .copied()
        .ok_or_else(|| SFError::TooShortResponse {
            name,
            pos,
            array: format!("{val:?}"),
        })
}

pub(crate) trait CGet<T: Copy + std::fmt::Debug> {
    fn cget(&self, pos: usize, name: &'static str) -> Result<T, SFError>;
}

impl<T: Copy + std::fmt::Debug + Display> CGet<T> for [T] {
    fn cget(&self, pos: usize, name: &'static str) -> Result<T, SFError> {
        raw_cget(self, pos, name)
    }
}

pub(crate) trait CCGet<T: Copy + std::fmt::Debug + Display, I: TryFrom<T>> {
    fn csiget(
        &self,
        pos: usize,
        name: &'static str,
        def: I,
    ) -> Result<I, SFError>;
    fn csimget(
        &self,
        pos: usize,
        name: &'static str,
        def: I,
        fun: fn(T) -> T,
    ) -> Result<I, SFError>;
    fn cwiget(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<Option<I>, SFError>;
    fn cwimget(
        &self,
        pos: usize,
        name: &'static str,
        fun: fn(T) -> T,
    ) -> Result<Option<I>, SFError>;
    fn ciget(&self, pos: usize, name: &'static str) -> Result<I, SFError>;
    fn cimget(
        &self,
        pos: usize,
        name: &'static str,
        fun: fn(T) -> T,
    ) -> Result<I, SFError>;
}

impl<T: Copy + std::fmt::Debug + Display, I: TryFrom<T>> CCGet<T, I> for [T] {
    fn csiget(
        &self,
        pos: usize,
        name: &'static str,
        def: I,
    ) -> Result<I, SFError> {
        let raw = raw_cget(self, pos, name)?;
        Ok(soft_into(raw, name, def))
    }

    fn cwiget(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<Option<I>, SFError> {
        let raw = raw_cget(self, pos, name)?;
        Ok(warning_try_into(raw, name))
    }

    fn csimget(
        &self,
        pos: usize,
        name: &'static str,
        def: I,
        fun: fn(T) -> T,
    ) -> Result<I, SFError> {
        let raw = raw_cget(self, pos, name)?;
        let raw = fun(raw);
        Ok(soft_into(raw, name, def))
    }

    fn ciget(&self, pos: usize, name: &'static str) -> Result<I, SFError> {
        let raw = raw_cget(self, pos, name)?;
        raw.try_into()
            .map_err(|_| SFError::ParsingError(name, raw.to_string()))
    }

    fn cimget(
        &self,
        pos: usize,
        name: &'static str,
        fun: fn(T) -> T,
    ) -> Result<I, SFError> {
        let raw = raw_cget(self, pos, name)?;
        let raw = fun(raw);
        raw.try_into()
            .map_err(|_| SFError::ParsingError(name, raw.to_string()))
    }

    fn cwimget(
        &self,
        pos: usize,
        name: &'static str,
        fun: fn(T) -> T,
    ) -> Result<Option<I>, SFError> {
        let raw = raw_cget(self, pos, name)?;
        let raw = fun(raw);
        Ok(warning_try_into(raw, name))
    }
}

pub(crate) trait CSGet<T: FromStr> {
    fn cfsget(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<Option<T>, SFError>;
    fn cfsuget(&self, pos: usize, name: &'static str) -> Result<T, SFError>;
}

impl<T: FromStr> CSGet<T> for [&str] {
    fn cfsget(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<Option<T>, SFError> {
        let raw = raw_cget(self, pos, name)?;
        Ok(warning_from_str(raw, name))
    }

    fn cfsuget(&self, pos: usize, name: &'static str) -> Result<T, SFError> {
        let raw = raw_cget(self, pos, name)?;
        let Some(val) = warning_from_str(raw, name) else {
            return Err(SFError::ParsingError(name, raw.to_string()));
        };
        Ok(val)
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

/// This is a workaround for clippy index warnings for safe index ops. It
/// also is more convenient in some cases to use these fundtions if you want
/// to make sure something is &mut, or &
pub trait EnumMapGet<K, V> {
    /// Gets a normal reference to the value
    fn get(&self, key: K) -> &V;
    /// Gets a mutable reference to the value
    fn get_mut(&mut self, key: K) -> &mut V;
}

impl<K: Enum + EnumArray<V>, V> EnumMapGet<K, V> for EnumMap<K, V> {
    fn get(&self, key: K) -> &V {
        #[allow(clippy::indexing_slicing)]
        &self[key]
    }

    fn get_mut(&mut self, key: K) -> &mut V {
        #[allow(clippy::indexing_slicing)]
        &mut self[key]
    }
}

pub(crate) trait ArrSkip<T: Debug> {
    /// Basically does the equivalent of [pos..], but bounds checked with
    /// correct errors
    fn skip(&self, pos: usize, name: &'static str) -> Result<&[T], SFError>;
}

impl<T: Debug> ArrSkip<T> for [T] {
    fn skip(&self, pos: usize, name: &'static str) -> Result<&[T], SFError> {
        if pos > self.len() {
            return Err(SFError::TooShortResponse {
                name,
                pos,
                array: format!("{self:?}"),
            });
        }
        Ok(self.split_at(pos).1)
    }
}

pub(crate) trait CFPGet<T: Into<i64> + Copy + std::fmt::Debug, R: FromPrimitive>
{
    fn cfpget(
        &self,
        pos: usize,
        name: &'static str,
        fun: fn(T) -> T,
    ) -> Result<Option<R>, SFError>;

    fn cfpuget(
        &self,
        pos: usize,
        name: &'static str,
        fun: fn(T) -> T,
    ) -> Result<R, SFError>;
}

impl<T: Into<i64> + Copy + std::fmt::Debug, R: FromPrimitive> CFPGet<T, R>
    for [T]
{
    fn cfpget(
        &self,
        pos: usize,
        name: &'static str,
        fun: fn(T) -> T,
    ) -> Result<Option<R>, SFError> {
        let raw = raw_cget(self, pos, name)?;
        let raw = fun(raw);
        let t: i64 = raw.into();
        let res = FromPrimitive::from_i64(t);
        if res.is_none() && t != 0 && t != -1 {
            warn!("There might be a new {name} -> {t}");
        }
        Ok(res)
    }

    fn cfpuget(
        &self,
        pos: usize,
        name: &'static str,
        fun: fn(T) -> T,
    ) -> Result<R, SFError> {
        let raw = raw_cget(self, pos, name)?;
        let raw = fun(raw);
        let t: i64 = raw.into();
        FromPrimitive::from_i64(t)
            .ok_or_else(|| SFError::ParsingError(name, t.to_string()))
    }
}

pub(crate) trait CSTGet<T: Copy + Debug + Into<i64>> {
    fn cstget(
        &self,
        pos: usize,
        name: &'static str,
        server_time: ServerTime,
    ) -> Result<Option<DateTime<Local>>, SFError>;
}

impl<T: Copy + Debug + Into<i64>> CSTGet<T> for [T] {
    fn cstget(
        &self,
        pos: usize,
        name: &'static str,
        server_time: ServerTime,
    ) -> Result<Option<DateTime<Local>>, SFError> {
        let val = raw_cget(self, pos, name)?;
        let val = val.into();
        Ok(server_time.convert_to_local(val, name))
    }
}
