use std::{
    fmt::{Debug, Display, Write},
    str::FromStr,
};

use base64::Engine;
use chrono::{DateTime, Local};
use enum_map::{Enum, EnumArray, EnumMap};
use log::warn;
use num_traits::FromPrimitive;

use crate::{error::SFError, gamestate::ServerTime};

pub const HASH_CONST: &str = "ahHoj2woo1eeChiech6ohphoB7Aithoh";
pub const DEFAULT_CRYPTO_KEY: &str = "[_/$VV&*Qg&)r?~g";
pub const DEFAULT_CRYPTO_ID: &str = "0-00000000000000";
pub const DEFAULT_SESSION_ID: &str = "00000000000000000000000000000000";
pub const CRYPTO_IV: &str = "jXT#/vz]3]5X7Jl\\";

#[must_use]
pub fn sha1_hash(val: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(val.as_bytes());
    let hash = hasher.finalize();
    let mut result = String::with_capacity(hash.len() * 2);
    for byte in &hash {
        _ = result.write_fmt(format_args!("{byte:02x}"));
    }
    result
}

/// Converts a raw value into the appropriate type. If that is not possible,
/// a warning will be emitted and the given default returned. This is useful
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

/// Converts a S&F string from the server to their original unescaped
/// representation
#[must_use]
pub fn from_sf_string(val: &str) -> String {
    let mut new = String::with_capacity(val.len());
    let mut is_escaped = false;
    for char in val.chars() {
        if char == '$' {
            is_escaped = true;
            continue;
        }
        let escaped_char = match char {
            x if !is_escaped => x,
            'b' => '\n',
            'c' => ':',
            'P' => '%',
            's' => '/',
            'p' => '|',
            '+' => '&',
            'q' => '"',
            'r' => '#',
            'C' => ',',
            'S' => ';',
            'd' => '$',
            x => {
                warn!("Unkown escape sequence: ${x}");
                x
            }
        };
        new.push(escaped_char);
        is_escaped = false;
    }
    new
}

/// Makes a user controlled string, like a new character description safe to use
/// in a request
#[must_use]
pub fn to_sf_string(val: &str) -> String {
    let mut new = String::with_capacity(val.len());
    for char in val.chars() {
        match char {
            '\n' => new.push_str("$b"),
            ':' => new.push_str("$c"),
            '%' => new.push_str("$P"),
            '/' => new.push_str("$s"),
            '|' => new.push_str("$p"),
            '&' => new.push_str("$+"),
            '"' => new.push_str("$q"),
            '#' => new.push_str("$r"),
            ',' => new.push_str("$C"),
            ';' => new.push_str("$S"),
            '$' => new.push_str("$d"),
            _ => new.push(char),
        }
    }
    new
}

/// This function is designed for reverse engineering encrypted commands from
/// the S&F web client. It expects a login response, which is the ~3KB string
/// response you can see in the network tab of your browser, that starts with
/// `serverversion` after a login. After that, you can take any URL the client
/// sends to the server and have it decoded into the actual string command, that
/// was sent. Note that this function technically only needs the crypto key, not
/// the full response, but it is way easier to just copy paste the full
/// response. The command returned here will be `Command::Custom`
///
/// # Errors
///
/// If either the URL, or the response do not contain the necessary crypto
/// values, an `InvalidRequest` error will be returned, that mentions the part,
/// that is missing or malformed. The same goes for the necessary parts of the
/// decrypted command
#[allow(clippy::missing_errors_doc, deprecated)]
#[deprecated(
    since = "0.2.2",
    note = "S&F requests are no longer encrypted, so this function will be \
            removed in a future update. If you need to decode the arguments \
            to a command yourself, you can just base64 decode them"
)]
pub fn decrypt_url(
    encrypted_url: &str,
    login_resp: Option<&str>,
) -> Result<crate::command::Command, SFError> {
    let crypto_key = if let Some(login_resp) = login_resp {
        login_resp
            .split('&')
            .filter_map(|a| a.split_once(':'))
            .find(|a| a.0 == "cryptokey")
            .ok_or(SFError::InvalidRequest("No crypto key in login resp"))?
            .1
    } else {
        DEFAULT_CRYPTO_KEY
    };

    let encrypted = encrypted_url
        .split_once("req=")
        .ok_or(SFError::InvalidRequest("url does not contain request"))?
        .1
        .rsplit_once("&rnd=")
        .ok_or(SFError::InvalidRequest("url does not contain rnd"))?
        .0;

    let resp = encrypted.get(DEFAULT_CRYPTO_ID.len()..).ok_or(
        SFError::InvalidRequest("encrypted command does not contain crypto id"),
    )?;
    let full_resp = decrypt_server_request(resp, crypto_key)?;

    let (_session_id, command) = full_resp.split_once('|').ok_or(
        SFError::InvalidRequest("decrypted command has no session id"),
    )?;

    let (cmd_name, args) = command
        .split_once(':')
        .ok_or(SFError::InvalidRequest("decrypted command has no name"))?;
    let args: Vec<_> = args
        .trim_end_matches('|')
        .split('/')
        .map(std::string::ToString::to_string)
        .collect();

    Ok(crate::command::Command::Custom {
        cmd_name: cmd_name.to_string(),
        arguments: args,
    })
}

#[allow(clippy::missing_errors_doc)]
#[deprecated(
    since = "0.2.2",
    note = "S&F requests are no longer encrypted, so this function will be \
            removed in a future update. If you need to decode the arguments \
            to a command yourself, you can just base64 decode them"
)]
pub fn decrypt_server_request(
    to_decrypt: &str,
    key: &str,
) -> Result<String, SFError> {
    let text = base64::engine::general_purpose::URL_SAFE
        .decode(to_decrypt)
        .map_err(|_| {
            SFError::InvalidRequest("Value to decode is not base64")
        })?;

    let mut my_key = [0; 16];
    my_key.copy_from_slice(
        key.as_bytes()
            .get(..16)
            .ok_or(SFError::InvalidRequest("Key is not 16 bytes long"))?,
    );

    let mut cipher = libaes::Cipher::new_128(&my_key);
    cipher.set_auto_padding(false);
    let decrypted = cipher.cbc_decrypt(CRYPTO_IV.as_bytes(), &text);

    String::from_utf8(decrypted)
        .map_err(|_| SFError::InvalidRequest("Decrypted value is not UTF8"))
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

#[allow(unused)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_sf_string() {
        let input = "$bHello$cWorld$PThis$sis a test!";
        let expected_output = "\nHello:World%This/is a test!";
        let result = from_sf_string(input);
        assert_eq!(result, expected_output);

        let input = "$$$$$$$$$$$";
        let expected_output = "";
        let result = from_sf_string(input);
        assert_eq!(result, expected_output);

        let input = "$$b$c$P$s$p$+$q$r$C$S$d";
        let expected_output = "\n:%/|&\"#,;$";
        let result = from_sf_string(input);
        assert_eq!(result, expected_output);
    }
    #[test]
    fn test_to_sf_string() {
        let input = "\nHello:World%This/is a test!";
        let expected_output = "$bHello$cWorld$PThis$sis a test!";
        let result = to_sf_string(input);
        assert_eq!(result, expected_output);

        let input = "\n:%/|&\"#,;$";
        let expected_output = "$b$c$P$s$p$+$q$r$C$S$d";
        let result = to_sf_string(input);
        assert_eq!(result, expected_output);
    }
}
