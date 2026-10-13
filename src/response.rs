// TODO: Remove this once ouroboros is updated:
// https://github.com/someguynamedjosh/ouroboros/issues/140
#![allow(clippy::extra_unused_lifetimes)]

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    str::FromStr,
};

use chrono::NaiveDateTime;
use log::{debug, error, trace, warn};
use num_traits::FromPrimitive;

use crate::error::SFError;

/// A bunch of new information about the state of the server and/or the
/// player
///
/// NOTE: This has a weird syntax to access, because we do not want to create
/// 10000 strings on each request and instead just store the raw response body
/// and references into it. This is faster & uses less memory, but because of
/// rusts borrow checker requires some weird syntax here.
// Technically we could do this safely with an iterator, that parses on demand,
// but send_command() needs to access specific response keys to keep the session
// running, which means a HashMap needs to be constructed no matter what
#[ouroboros::self_referencing]
pub struct Response {
    body: String,
    #[borrows(body)]
    #[covariant]
    resp: HashMap<&'this str, ResponseVal<'this>>,
    /// We store this to make sure the time calculations are still correct, if
    /// this response is held any amount of time before being used to update
    /// character state
    received_at: NaiveDateTime,
}

impl Clone for Response {
    // This is not a good clone..
    #[allow(clippy::expect_used)]
    fn clone(&self) -> Self {
        Self::parse(self.raw_response().to_string(), self.received_at())
            .expect("Invalid response cloned")
    }
}

impl Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.values().iter().map(|a| (a.0, a.1.as_str())))
            .finish()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Response {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Response", 2)?;
        s.serialize_field("body", self.borrow_body())?;
        s.serialize_field("received_at", &self.received_at())?;
        s.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Response {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AVisitor;

        impl<'de> serde::de::Visitor<'de> for AVisitor {
            type Value = Response;

            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str(
                    "struct Response with fields body and received_at",
                )
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut body = None;
                let mut received_at = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "body" => {
                            body = Some(map.next_value()?);
                        }
                        "received_at" => {
                            received_at = Some(map.next_value()?);
                        }
                        _ => {
                            // Ignore unknown fields
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }

                let body: String =
                    body.ok_or_else(|| serde::de::Error::missing_field("q"))?;
                let received_at: NaiveDateTime = received_at
                    .ok_or_else(|| serde::de::Error::missing_field("j"))?;

                Response::parse(body, received_at).map_err(|_| {
                    serde::de::Error::custom("invalid response body")
                })
            }
        }

        deserializer.deserialize_struct(
            "Response",
            &["body", "received_at"],
            AVisitor,
        )
    }
}

impl Response {
    /// Returns a reference to the hashmap, that contains mappings of response
    /// keys to values
    #[must_use]
    pub fn values(&self) -> &HashMap<&str, ResponseVal<'_>> {
        self.borrow_resp()
    }

    /// Returns the raw response from the server. This should only ever be
    /// necessary for debugging, caching, or in case there is ever a new
    /// response format in a response, that is not yet supported. You can of
    /// course also use this to look at how horrible the S&F encoding is..
    #[must_use]
    pub fn raw_response(&self) -> &str {
        self.borrow_body()
    }

    /// Returns the time, at which the response was received
    #[must_use]
    pub fn received_at(&self) -> NaiveDateTime {
        self.with_received_at(|a| *a)
    }

    /// Parses a response body from the server into a usable format
    /// You might want to use this, if you are analyzing responses from the
    /// browsers network tab. If you are trying to store/read responses to/from
    /// disk to cache them, or otherwise, you should use the sso feature to
    /// serialize/deserialize them instead
    ///
    /// # Errors
    /// - `ServerError`: If the server responsed with an error
    /// - `ParsingError`: If the response does not follow the standard S&F
    ///   server response schema
    pub fn parse(
        og_body: String,
        received_at: NaiveDateTime,
    ) -> Result<Response, SFError> {
        // We can not return from the closure below, so we have to do this work
        // twice (sadly)

        // NOTE: I think the trims might actually be completely unnecessary.
        // Pretty sure I mixed them up with command encoding, which is actually
        // '|' padded

        let body = og_body
            .trim_end_matches('|')
            .trim_start_matches(|a: char| !a.is_alphabetic());
        trace!("Received raw response: {body}");

        if !body.contains(':')
            && !body.starts_with("success")
            && !body.starts_with("Success")
        {
            return Err(SFError::ParsingError(
                "unexpected server response",
                body.to_string(),
            ));
        }

        if body.starts_with("error") || body.starts_with("Error") {
            let raw_error = body.split_once(':').unwrap_or_default().1;

            let error_msg = match raw_error {
                "adventure index must be 1-3" => "quest index must be 0-2",
                x => x,
            };

            return Err(SFError::ServerError(error_msg.to_string()));
        }

        let resp = ResponseBuilder {
            body: og_body,
            resp_builder: |body: &String| {
                let mut res = HashMap::new();
                for part in body
                    .trim_start_matches(|a: char| !a.is_alphabetic())
                    .trim_end_matches('|')
                    .split('&')
                    .filter(|a| !a.is_empty())
                {
                    let Some((full_key, value)) = part.split_once(':') else {
                        warn!("weird k/v in resp: {part}");
                        continue;
                    };

                    let (key, sub_key) = match full_key.split_once('.') {
                        Some(x) => {
                            // full_key == key.subkey
                            x
                        }
                        None => {
                            if let Some((k, sk)) = full_key.split_once('(') {
                                // full_key == key(4)
                                (k, sk.trim_matches(')'))
                            } else {
                                // full_key == key
                                (full_key, "")
                            }
                        }
                    };
                    if key.is_empty() {
                        continue;
                    }

                    let old_val =
                        res.insert(key, ResponseVal { value, sub_key });
                    if let Some(old_val) = old_val {
                        let old = old_val.as_str();
                        debug!("Overwrote [{key}]: {old} => {value}");
                    }
                }
                res
            },
            received_at,
        }
        .build();

        Ok(resp)
    }
}

/// This is the raw &str, that the server sent as a value to some key. This
/// often requires extra conversions/parsing to use practically, so we associate
/// the most common parsing functions as methods to this data.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::module_name_repetitions)]
pub struct ResponseVal<'a> {
    value: &'a str,
    sub_key: &'a str,
}

impl<'a> ResponseVal<'a> {
    /// Converts the response value into the required type
    ///
    /// # Errors
    /// If the response value can not be parsed into the output
    /// value, a `ParsingError` will be returned
    pub fn into<T: FromStr>(self, name: &'static str) -> Result<T, SFError> {
        self.value.trim().parse().map_err(|_| {
            error!("Could not convert {name} into target type: {self}");
            SFError::ParsingError(name, self.value.to_string())
        })
    }

    /// Converts the repsponse into a list, by splitting the raw value by '/'
    /// and converting each value into the required type. If any conversion
    /// fails, an error is returned
    ///
    /// # Errors
    /// If any of the values in the string can not be parsed into the output
    /// value, the `ParsingError` for that value will be returned
    #[deprecated]
    pub fn into_list<T: FromStr>(
        self,
        name: &'static str,
    ) -> Result<Vec<T>, SFError> {
        let x = &self.value;
        if x.is_empty() {
            return Ok(Vec::new());
        }
        // Trimming ` ` & `\n` is not required. Might remove this later
        x.trim_matches(|a| ['/', ' ', '\n'].contains(&a))
            .split('/')
            .map(|c| {
                c.trim().parse::<T>().map_err(|_| {
                    error!(
                        "Could not convert {name} into list because of {c}: \
                         {self}"
                    );
                    SFError::ParsingError(name, format!("{c:?}"))
                })
            })
            .collect()
    }

    /// The way keys are parsed will trim some info from the string. The key for
    /// the player save `ownplayersave` is actually `ownplayersave.playerSave`.
    /// As this `.playerSave` is not relevant here and not in most cases, I
    /// decided to trim that off. More common, this is also just `s`, `r`, or a
    /// size hint like `(10)`. In some cases though, this information can be
    /// helpful for parsing. Thus, you can access it here
    #[must_use]
    pub fn sub_key(&self) -> &str {
        self.sub_key
    }

    /// Returns the raw reference to the internal &str, that the server sent
    #[must_use]
    pub fn as_str(&self) -> &'a str {
        self.value
    }

    /// Splits this response value by `'/'` into consecutive non-overlapping chunks of size `CHUNK`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if the response is empty.
    #[track_caller]
    pub fn parse_chunks<const CHUNK: usize>(
        &self,
    ) -> Result<impl Iterator<Item = ResponseVals<'a, CHUNK>>, SFError> {
        self.parse_chunks_delim::<CHUNK>('/')
    }

    /// Splits this response value by `delim` into consecutive non-overlapping chunks of size `CHUNK`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if the response is empty.
    ///
    /// # Panics
    /// Panics if `CHUNK == 0`.
    #[track_caller]
    pub fn parse_chunks_delim<const CHUNK: usize>(
        &self,
        delim: char,
    ) -> Result<impl Iterator<Item = ResponseVals<'a, CHUNK>>, SFError> {
        assert!(CHUNK > 0, "CHUNK size must be greater than 0");
        let trimmed = self
            .value
            .trim_matches(|a| [delim, ' ', '\n', '\r'].contains(&a));
        if trimmed.is_empty() {
            let caller = std::panic::Location::caller();
            return Err(SFError::ParsingErrorV2(format!(
                "Server response value is empty at {}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            )));
        }

        let vals: Vec<&'a str> = trimmed.split(delim).map(str::trim).collect();
        if !vals.len().is_multiple_of(CHUNK) {
            let caller = std::panic::Location::caller();
            warn!(
                "Server response contains {} leftover values when chunked by {CHUNK} at {}:{}:{}",
                vals.len() % CHUNK,
                caller.file(),
                caller.line(),
                caller.column()
            );
        }

        Ok(vals
            .chunks_exact(CHUNK)
            .map(|chunk| {
                let arr: [&'a str; CHUNK] = match chunk.try_into() {
                    Ok(a) => a,
                    Err(_) => unreachable!(),
                };
                ResponseVals {
                    values: Box::new(arr),
                }
            })
            .collect::<Vec<_>>()
            .into_iter())
    }

    /// Parses the response into a compile-time bounded [`ResponseVals<'a, N>`] split by `/`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if the response is empty or has fewer than `N` values.
    #[track_caller]
    pub fn values<const N: usize>(
        self,
    ) -> Result<ResponseVals<'a, N>, SFError> {
        self.values_delim::<N>('/')
    }

    /// Parses the response into a compile-time bounded [`ResponseVals<'a, N>`] split by `/`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if the response is empty or has fewer than `N` values.
    #[track_caller]
    pub fn values_fixed<const N: usize>(
        self,
    ) -> Result<ResponseVals<'a, N>, SFError> {
        self.values_delim::<N>('/')
    }

    /// Parses the response into a compile-time bounded [`ResponseVals<'a, N>`] split by `delim`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if the response is empty or has fewer than `N` values.
    #[track_caller]
    pub fn values_by<const N: usize>(
        self,
        delim: char,
    ) -> Result<ResponseVals<'a, N>, SFError> {
        self.values_delim::<N>(delim)
    }

    /// Parses the response into a compile-time bounded [`ResponseVals<'a, N>`] split by `delim`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if the response is empty or has fewer than `N` values.
    #[track_caller]
    pub fn values_delim<const N: usize>(
        self,
        delim: char,
    ) -> Result<ResponseVals<'a, N>, SFError> {
        const {
            assert!(N > 0, "ResponseVals bound N must be greater than 0");
        };
        let trimmed = self
            .value
            .trim_matches(|a| [delim, ' ', '\n', '\r'].contains(&a));
        if trimmed.is_empty() {
            let caller = std::panic::Location::caller();
            return Err(SFError::ParsingErrorV2(format!(
                "Server response value is empty at {}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            )));
        }

        let mut vals: Vec<_> = trimmed.split(delim).map(str::trim).collect();

        if vals.len() > N {
            let caller = std::panic::Location::caller();
            let extra = vals.len() - N;
            warn!(
                "Server response contains {extra} unexpected values at \
                 {}:{}:{}: {:?}",
                caller.file(),
                caller.line(),
                caller.column(),
                vals.get(N..).unwrap_or_default()
            );
            vals.truncate(N);
        } else if vals.len() < N {
            let caller = std::panic::Location::caller();
            let missing = N - vals.len();
            return Err(SFError::ParsingErrorV2(format!(
                "Server response is missing {missing} values at {}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            )));
        }

        let boxed_slice: Box<[&'a str]> = vals.into_boxed_slice();
        let values: Box<[&'a str; N]> = boxed_slice.try_into().map_err(|_| {
            let caller = std::panic::Location::caller();
            SFError::ParsingErrorV2(format!(
                "Failed to convert response into array of size {N} at {}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            ))
        })?;

        Ok(ResponseVals { values })
    }
}

impl std::fmt::Display for ResponseVal<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.value)
    }
}

/// A sequence of string values from a server response, typically split from a
/// `/`-separated list.
///
/// Guarantees at compile-time that exactly `N` values are present in the response
/// (validated upon construction or conversion), stored efficiently in a heap-allocated
/// fixed-size array `Box<[&'a str; N]>`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)]
pub struct ResponseVals<'a, const N: usize> {
    values: Box<[&'a str; N]>,
}

impl<'a, const N: usize> ResponseVals<'a, N> {
    /// Creates a new [`ResponseVals`] from a fixed-size array.
    #[must_use]
    pub fn from_array(values: [&'a str; N]) -> Self {
        Self {
            values: Box::new(values),
        }
    }

    /// Returns the number of values in this response (always `N`).
    #[must_use]
    pub const fn len(&self) -> usize {
        N
    }

    /// Returns `true` if this response contains no values (always `false` since `N > 0`).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }

    /// Returns a reference to the underlying slice of `&str`.
    #[must_use]
    pub fn as_slice(&self) -> &[&'a str] {
        self.values.as_slice()
    }

    /// Returns the string value at index `pos`, if in bounds.
    #[must_use]
    pub fn get_str(&self, pos: usize) -> Option<&'a str> {
        self.values.get(pos).copied()
    }

    /// Returns the raw string value at `pos`, or an error with caller location if out of bounds.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds.
    #[track_caller]
    pub fn get(&self, pos: usize) -> Result<&'a str, SFError> {
        let Some(val) = self.values.get(pos) else {
            let caller = std::panic::Location::caller();
            return Err(SFError::ParsingErrorV2(format!(
                "Index out of bounds (pos:{pos},len:{}) at {}:{}:{}",
                self.values.len(),
                caller.file(),
                caller.line(),
                caller.column()
            )));
        };
        Ok(val)
    }

    /// Parses the value at index `pos` into type `T` with caller tracking.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or parsing fails.
    #[track_caller]
    pub fn parse<T: FromStr>(
        &self,
        pos: usize,
        ty: &str,
    ) -> Result<T, SFError> {
        let val = self.get(pos)?;
        val.trim().parse().map_err(|_| {
            let caller = std::panic::Location::caller();
            SFError::ParsingErrorV2(format!(
                "Invalid {ty} (pos:{pos},val:{val}) at {}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            ))
        })
    }

    /// Parses the value at index `pos` into type `T`, returning `default` on error.
    #[track_caller]
    #[must_use]
    pub fn parse_or<T: FromStr>(
        &self,
        pos: usize,
        default: T,
        ty: &str,
    ) -> T {
        self.parse(pos, ty).unwrap_or_else(|err| {
            warn!("{err}");
            default
        })
    }

    /// Parses the value at `pos` as `i64`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `i64`.
    #[track_caller]
    pub fn i64(&self, pos: usize) -> Result<i64, SFError> {
        self.parse(pos, "i64")
    }

    /// Parses the value at `pos` as `i64`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn i64_or(&self, pos: usize, default: i64) -> i64 {
        self.parse_or(pos, default, "i64")
    }

    /// Parses the value at `pos` as `u64`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `u64`.
    #[track_caller]
    pub fn u64(&self, pos: usize) -> Result<u64, SFError> {
        self.parse(pos, "u64")
    }

    /// Parses the value at `pos` as `u64`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn u64_or(&self, pos: usize, default: u64) -> u64 {
        self.parse_or(pos, default, "u64")
    }

    /// Parses the value at `pos` as `u32`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `u32`.
    #[track_caller]
    pub fn u32(&self, pos: usize) -> Result<u32, SFError> {
        self.parse(pos, "u32")
    }

    /// Parses the value at `pos` as `u32`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn u32_or(&self, pos: usize, default: u32) -> u32 {
        self.parse_or(pos, default, "u32")
    }

    /// Parses the value at `pos` as `i32`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `i32`.
    #[track_caller]
    pub fn i32(&self, pos: usize) -> Result<i32, SFError> {
        self.parse(pos, "i32")
    }

    /// Parses the value at `pos` as `i32`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn i32_or(&self, pos: usize, default: i32) -> i32 {
        self.parse_or(pos, default, "i32")
    }

    /// Parses the value at `pos` as `usize`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `usize`.
    #[track_caller]
    pub fn usize(&self, pos: usize) -> Result<usize, SFError> {
        self.parse(pos, "usize")
    }

    /// Parses the value at `pos` as `usize`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn usize_or(&self, pos: usize, default: usize) -> usize {
        self.parse_or(pos, default, "usize")
    }

    /// Parses the value at `pos` as `u8`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `u8`.
    #[track_caller]
    pub fn u8(&self, pos: usize) -> Result<u8, SFError> {
        self.parse(pos, "u8")
    }

    /// Parses the value at `pos` as `u8`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn u8_or(&self, pos: usize, default: u8) -> u8 {
        self.parse_or(pos, default, "u8")
    }

    /// Parses the value at `pos` as `i8`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `i8`.
    #[track_caller]
    pub fn i8(&self, pos: usize) -> Result<i8, SFError> {
        self.parse(pos, "i8")
    }

    /// Parses the value at `pos` as `i8`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn i8_or(&self, pos: usize, default: i8) -> i8 {
        self.parse_or(pos, default, "i8")
    }

    /// Parses the value at `pos` as `u16`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `u16`.
    #[track_caller]
    pub fn u16(&self, pos: usize) -> Result<u16, SFError> {
        self.parse(pos, "u16")
    }

    /// Parses the value at `pos` as `u16`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn u16_or(&self, pos: usize, default: u16) -> u16 {
        self.parse_or(pos, default, "u16")
    }

    /// Parses the value at `pos` as `i16`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `i16`.
    #[track_caller]
    pub fn i16(&self, pos: usize) -> Result<i16, SFError> {
        self.parse(pos, "i16")
    }

    /// Parses the value at `pos` as `i16`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn i16_or(&self, pos: usize, default: i16) -> i16 {
        self.parse_or(pos, default, "i16")
    }

    /// Parses the value at `pos` as `bool`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds or cannot be parsed as `bool`.
    #[track_caller]
    pub fn bool(&self, pos: usize) -> Result<bool, SFError> {
        self.parse(pos, "bool")
    }

    /// Parses the value at `pos` as `bool`, returning `default` on failure.
    #[track_caller]
    #[must_use]
    pub fn bool_or(&self, pos: usize, default: bool) -> bool {
        self.parse_or(pos, default, "bool")
    }

    /// Returns the string value at `pos`.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if `pos` is out of bounds.
    #[track_caller]
    pub fn str(&self, pos: usize) -> Result<&'a str, SFError> {
        self.get(pos)
    }

    /// Returns the string value at `pos`, or `default` if out of bounds.
    #[track_caller]
    #[must_use]
    pub fn str_or(&self, pos: usize, default: &'a str) -> &'a str {
        self.get(pos).unwrap_or_else(|err| {
            warn!("{err}");
            default
        })
    }

    /// Parses the value at index `pos` into type `T`.
    ///
    /// # Errors
    /// - [`SFError::TooShortResponse`] if `pos` is out of bounds.
    /// - [`SFError::ParsingError`] if parsing into `T` fails.
    pub fn parse_at<T: FromStr>(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Result<T, SFError> {
        let val = self.values.get(pos).ok_or_else(|| {
            SFError::TooShortResponse {
                name,
                pos,
                array: format!("{:?}", self.values),
            }
        })?;
        val.trim().parse::<T>().map_err(|_| {
            error!("Could not parse {name} from {val:?}");
            SFError::ParsingError(name, (*val).to_string())
        })
    }

    /// Tries to parse the value at index `pos` into `Option<T>`.
    /// If the index is out of bounds or parsing fails, returns `None` with a warning.
    #[must_use]
    pub fn parse_opt_at<T: FromStr>(
        &self,
        pos: usize,
        name: &'static str,
    ) -> Option<T> {
        let val = self.values.get(pos)?;
        val.trim().parse::<T>().ok().or_else(|| {
            warn!("Could not parse optional {name} from {val}");
            None
        })
    }

    /// Parses the value at index `pos` into `T`, returning `default` on failure with a warning.
    #[must_use]
    pub fn parse_soft_at<T: FromStr + Display + Copy>(
        &self,
        pos: usize,
        name: &'static str,
        default: T,
    ) -> T {
        let Some(val) = self.values.get(pos) else {
            warn!("Missing value for {name} at index {pos}");
            return default;
        };
        val.trim().parse::<T>().unwrap_or_else(|_| {
            warn!("Invalid value for {name} at index {pos}: {val}");
            default
        })
    }

    /// Parses an optional enum at `pos` using [`FromPrimitive`].
    ///
    /// Returns `None` if missing, unparseable, or not a recognized variant.
    #[must_use]
    pub fn enum_opt<T: FromPrimitive>(&self, pos: usize) -> Option<T> {
        let val = self.i64(pos).ok()?;
        T::from_i64(val).or_else(|| T::from_u64(val.cast_unsigned()))
    }

    /// Parses an enum at `pos` using [`FromPrimitive`], returning `default` if missing or invalid.
    #[must_use]
    pub fn enum_or<T: FromPrimitive>(&self, pos: usize, default: T) -> T {
        self.enum_opt(pos).unwrap_or(default)
    }

    /// Parses an enum at `pos` using [`FromPrimitive`] after transforming the raw integer with `f`.
    pub fn enum_map<T: FromPrimitive, F: FnOnce(u32) -> u32>(
        &self,
        pos: usize,
        f: F,
    ) -> Option<T> {
        let raw = self.u32_or(pos, 0);
        T::from_u32(f(raw))
    }

    /// Parses a required enum at `pos` using [`FromPrimitive`].
    ///
    /// # Errors
    /// Returns [`SFError::ParsingErrorV2`] if missing, unparseable, or not a recognized variant.
    #[track_caller]
    pub fn parse_enum<T: FromPrimitive>(&self, pos: usize) -> Result<T, SFError> {
        let raw = self.i64(pos)?;
        T::from_i64(raw).or_else(|| T::from_u64(raw.cast_unsigned())).ok_or_else(|| {
            let caller = std::panic::Location::caller();
            warn!(
                "Unknown enum variant {raw} for {} at {}:{}:{}",
                std::any::type_name::<T>(),
                caller.file(),
                caller.line(),
                caller.column()
            );
            SFError::ParsingErrorV2(format!(
                "Unknown enum variant {raw} for {}",
                std::any::type_name::<T>()
            ))
        })
    }

    /// Returns an iterator yielding consecutive non-overlapping chunks of size `CHUNK`.
    ///
    /// # Panics
    /// Panics if `CHUNK == 0`.
    pub fn parse_chunks<const CHUNK: usize>(
        &self,
    ) -> impl Iterator<Item = ResponseVals<'a, CHUNK>> + '_ {
        assert!(CHUNK > 0, "CHUNK size must be greater than 0");
        self.values.chunks_exact(CHUNK).map(|chunk| {
            let arr: [&'a str; CHUNK] = match chunk.try_into() {
                Ok(a) => a,
                Err(_) => unreachable!(),
            };
            ResponseVals {
                values: Box::new(arr),
            }
        })
    }

    /// Returns a direct array reference to the values.
    #[must_use]
    pub fn as_array(&self) -> &[&'a str; N] {
        &self.values
    }

    /// Returns the element at `IDX` with compile-time bounds checking.
    ///
    /// # Panics
    /// Fails to compile if `IDX >= N`.
    #[must_use]
    pub fn get_const<const IDX: usize>(&self) -> &'a str {
        const { assert!(IDX < N, "index out of bounds for ResponseVals") };
        #[allow(clippy::indexing_slicing)]
        self.values[IDX]
    }

    /// Parses the element at `IDX` into `T` with compile-time bounds checking.
    ///
    /// # Errors
    /// Returns [`SFError::ParsingError`] if parsing fails.
    ///
    /// # Panics
    /// Fails to compile if `IDX >= N`.
    pub fn parse_const<T: FromStr, const IDX: usize>(
        &self,
        name: &'static str,
    ) -> Result<T, SFError> {
        const { assert!(IDX < N, "index out of bounds for ResponseVals") };
        let s = self.get_const::<IDX>();
        s.trim().parse::<T>().map_err(|_| {
            SFError::ParsingError(name, s.to_string())
        })
    }

    /// Returns a sub-chunk of `M` elements starting at compile-time `OFFSET`.
    ///
    /// # Panics
    /// Fails to compile if `OFFSET + M > N`.
    #[must_use]
    pub fn sub_chunk<const OFFSET: usize, const M: usize>(
        &self,
    ) -> ResponseVals<'a, M> {
        const {
            assert!(
                OFFSET + M <= N,
                "sub_chunk out of bounds for ResponseVals"
            );
        };
        #[allow(clippy::indexing_slicing)]
        let slice: &[&'a str] = &self.values[OFFSET..OFFSET + M];
        let arr: [&'a str; M] = match slice.try_into() {
            Ok(a) => a,
            Err(_) => unreachable!(),
        };
        ResponseVals {
            values: Box::new(arr),
        }
    }

    /// Returns an iterator yielding consecutive non-overlapping chunks of size `CHUNK`.
    ///
    /// # Panics
    /// Fails to compile if `CHUNK == 0` or if `N % CHUNK != 0`.
    pub fn chunks_exact<const CHUNK: usize>(
        &self,
    ) -> impl Iterator<Item = ResponseVals<'a, CHUNK>> + '_ {
        const {
            assert!(CHUNK > 0, "CHUNK size must be non-zero");
            assert!(
                N.is_multiple_of(CHUNK),
                "N must be an exact multiple of CHUNK size"
            );
        };
        self.values
            .chunks_exact(CHUNK)
            .map(|chunk| {
                let arr: [&'a str; CHUNK] = match chunk.try_into() {
                    Ok(a) => a,
                    Err(_) => unreachable!(),
                };
                ResponseVals {
                    values: Box::new(arr),
                }
            })
    }

    /// Returns an iterator over the string values.
    pub fn iter(&self) -> std::iter::Copied<std::slice::Iter<'_, &'a str>> {
        self.values.iter().copied()
    }
}

impl<'a, const N: usize> std::ops::Deref for ResponseVals<'a, N> {
    type Target = [&'a str; N];

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl<'a, const N: usize> AsRef<[&'a str; N]> for ResponseVals<'a, N> {
    fn as_ref(&self) -> &[&'a str; N] {
        &self.values
    }
}

impl<'a, const N: usize> AsRef<[&'a str]> for ResponseVals<'a, N> {
    fn as_ref(&self) -> &[&'a str] {
        self.values.as_slice()
    }
}

impl<'a, const N: usize> From<[&'a str; N]> for ResponseVals<'a, N> {
    fn from(arr: [&'a str; N]) -> Self {
        Self {
            values: Box::new(arr),
        }
    }
}

impl<'a, const N: usize> From<Box<[&'a str; N]>> for ResponseVals<'a, N> {
    fn from(values: Box<[&'a str; N]>) -> Self {
        Self { values }
    }
}

impl<'a, const N: usize> IntoIterator for &'a ResponseVals<'a, N> {
    type Item = &'a str;
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, &'a str>>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.iter().copied()
    }
}

impl<'a, const N: usize> TryFrom<&'a str> for ResponseVals<'a, N> {
    type Error = SFError;

    #[track_caller]
    fn try_from(raw: &'a str) -> Result<Self, Self::Error> {
        ResponseVal {
            value: raw,
            sub_key: "",
        }
        .values_fixed()
    }
}

impl<'a, const N: usize> TryFrom<&ResponseVals<'a, N>> for ResponseVals<'a, N> {
    type Error = SFError;

    fn try_from(vals: &ResponseVals<'a, N>) -> Result<Self, Self::Error> {
        Ok(vals.clone())
    }
}

impl<'a, const N: usize> TryFrom<ResponseVal<'a>> for ResponseVals<'a, N> {
    type Error = SFError;

    #[track_caller]
    fn try_from(val: ResponseVal<'a>) -> Result<Self, Self::Error> {
        val.values_fixed()
    }
}

impl<'a, const N: usize> TryFrom<&ResponseVal<'a>> for ResponseVals<'a, N> {
    type Error = SFError;

    #[track_caller]
    fn try_from(val: &ResponseVal<'a>) -> Result<Self, Self::Error> {
        (*val).values_fixed()
    }
}

impl<const N: usize> Display for ResponseVals<'_, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for val in &self.values {
            if !first {
                f.write_str("/")?;
            }
            f.write_str(val)?;
            first = false;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn test_from_array_and_try_from() {
        let vals = ResponseVals::from_array(["1", "2", "3", "4"]);
        assert_eq!(vals.len(), 4);
        assert_eq!(vals[0], "1");
        assert_eq!(vals[3], "4");

        let parsed: ResponseVals<4> = "1/2/3/4".try_into().unwrap();
        assert_eq!(parsed, vals);
    }

    #[test]
    fn test_caller_tracking_get_and_typed_parsers() {
        let raw = "42/-10/true/hello/255/1000/2000000000/9000000000000000000";
        let vals: ResponseVals<8> = raw.try_into().unwrap();
        assert_eq!(vals.u32(0).unwrap(), 42);
        assert_eq!(vals.i32(1).unwrap(), -10);
        assert!(vals.bool(2).unwrap());
        assert_eq!(vals.str(3).unwrap(), "hello");
        assert_eq!(vals.u8(4).unwrap(), 255);
        assert_eq!(vals.u16(5).unwrap(), 1000);
        assert_eq!(vals.usize(6).unwrap(), 2_000_000_000);
        assert_eq!(vals.i64(7).unwrap(), 9_000_000_000_000_000_000);

        // Test _or fallbacks
        assert_eq!(vals.u32_or(0, 0), 42);
        assert_eq!(vals.u32_or(99, 999), 999);
        assert_eq!(vals.i64_or(99, -1), -1);
        assert_eq!(vals.str_or(99, "default"), "default");
        assert!(!vals.bool_or(99, false));

        // Out of bounds error should be ParsingErrorV2 with file/line
        let err = vals.get(99).unwrap_err();
        assert!(matches!(err, SFError::ParsingErrorV2(_)));
        assert!(err.to_string().contains("Index out of bounds"));

        // Type parse error should be ParsingErrorV2 with file/line
        let err = vals.u32(3).unwrap_err();
        assert!(matches!(err, SFError::ParsingErrorV2(_)));
        assert!(err.to_string().contains("Invalid u32"));
    }

    #[test]
    fn test_values_tracking_and_truncate() {
        let rval = ResponseVal {
            value: "1/2/3/4/5",
            sub_key: "k",
        };
        // Expected 3, but got 5: should warn and truncate to 3
        let vals: ResponseVals<3> = rval.values().unwrap();
        assert_eq!(vals.len(), 3);
        assert_eq!(vals.u32(0).unwrap(), 1);
        assert_eq!(vals.u32(2).unwrap(), 3);

        // Missing values error
        let err = rval.values::<10>().unwrap_err();
        assert!(matches!(err, SFError::ParsingErrorV2(_)));
        assert!(err.to_string().contains("missing 5 values"));

        // Empty response error
        let empty_val = ResponseVal {
            value: "",
            sub_key: "",
        };
        let err = empty_val.values::<3>().unwrap_err();
        assert!(matches!(err, SFError::ParsingErrorV2(_)));
        assert!(err.to_string().contains("is empty"));
    }

    #[derive(Debug, PartialEq, Eq, num_derive::FromPrimitive)]
    enum TestEnum {
        First = 1,
        Second = 2,
    }

    #[test]
    fn test_parse_chunks_and_delimiters() {
        let rval = ResponseVal {
            value: "1/2/3/4/5/6",
            sub_key: "",
        };
        let chunks: Vec<ResponseVals<2>> = rval.parse_chunks().unwrap().collect();
        assert_eq!(chunks.len(), 3);
        assert_eq!((chunks[0].u32(0).unwrap(), chunks[0].u32(1).unwrap()), (1, 2));
        assert_eq!((chunks[1].u32(0).unwrap(), chunks[1].u32(1).unwrap()), (3, 4));
        assert_eq!((chunks[2].u32(0).unwrap(), chunks[2].u32(1).unwrap()), (5, 6));

        // Test custom delimiter (comma)
        let comma_val = ResponseVal {
            value: "10,20;30,40",
            sub_key: "",
        };
        let semi_chunks: Vec<ResponseVals<1>> =
            comma_val.parse_chunks_delim(';').unwrap().collect();
        assert_eq!(semi_chunks.len(), 2);
        assert_eq!(semi_chunks[0].str(0).unwrap(), "10,20");

        let comma_fixed: ResponseVals<2> = comma_val.values_delim(';').unwrap();
        assert_eq!(comma_fixed.str(0).unwrap(), "10,20");
        assert_eq!(comma_fixed.str(1).unwrap(), "30,40");

        // Test TryFrom for ResponseVal
        let fixed: Result<ResponseVals<6>, _> = rval.try_into();
        assert!(fixed.is_ok());
    }

    #[test]
    fn test_enum_helpers() {
        let raw = "1/2/99/invalid";
        let vals: ResponseVals<4> = raw.try_into().unwrap();

        assert_eq!(vals.enum_opt::<TestEnum>(0), Some(TestEnum::First));
        assert_eq!(vals.enum_opt::<TestEnum>(1), Some(TestEnum::Second));
        assert_eq!(vals.enum_opt::<TestEnum>(2), None);
        assert_eq!(vals.enum_opt::<TestEnum>(3), None);

        assert_eq!(vals.enum_or(2, TestEnum::First), TestEnum::First);

        assert_eq!(vals.parse_enum::<TestEnum>(0).unwrap(), TestEnum::First);
        assert!(vals.parse_enum::<TestEnum>(2).is_err());

        assert_eq!(
            vals.enum_map::<TestEnum, _>(0, |x| x + 1),
            Some(TestEnum::Second)
        );
    }
}
