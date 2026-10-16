// TODO: Remove this once ouroboros is updated:
// https://github.com/someguynamedjosh/ouroboros/issues/140
#![allow(clippy::extra_unused_lifetimes)]

use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{Debug, Display},
    ops::{
        Bound, Range, RangeBounds, RangeFrom, RangeFull, RangeInclusive,
        RangeTo, RangeToInclusive,
    },
    str::FromStr,
};

use chrono::{DateTime, Local, NaiveDateTime};
use log::{debug, error, trace, warn};
use num_bigint::BigInt;
use num_traits::FromPrimitive;

use crate::{error::SFError, gamestate::ServerTime};

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
            return Err(SFError::ParsingError(format!(
                "Unexpected server response: {body}"
            )));
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

                    let old_val = res.insert(
                        key,
                        ResponseVal {
                            key,
                            value,
                            sub_key,
                        },
                    );
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
pub struct ResponseVal<'a> {
    key: &'a str,
    value: &'a str,
    sub_key: &'a str,
}

/// Helper to lazily format the key context for warnings and errors without
/// allocating when not triggered.
fn key_desc(key: &str) -> impl Display + '_ {
    std::fmt::from_fn(move |f| {
        if key.is_empty() {
            Ok(())
        } else {
            write!(f, " for key {key:?}")
        }
    })
}

impl<'a> ResponseVal<'a> {
    /// Converts the response value into the required type
    ///
    /// # Errors
    /// If the response value can not be parsed into the output
    /// value, a `ParsingError` will be returned
    pub(crate) fn into<T: FromStr>(
        self,
        name: &'static str,
    ) -> Result<T, SFError> {
        self.value.trim().parse().map_err(|_| {
            error!("Could not convert {name} into target type: {self}");
            SFError::ParsingError(format!(
                "Could not convert {name} into target type: {self}"
            ))
        })
    }

    /// Returns the key name this response value is associated with.
    #[must_use]
    pub fn key(&self) -> &str {
        self.key
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

    /// Splits this response value by `/` into a [`Vals`] of any length. Use
    /// this for responses whose number of values is only known at runtime,
    /// like `fight.r`.
    #[must_use]
    pub(crate) fn list(&self) -> Vals<'a> {
        self.list_by('/')
    }

    /// Splits this response value by `delim` into a [`Vals`] of any length.
    /// Like [`list`](Self::list), but for the runtime-length responses that are
    /// separated by something other than `/`, like `owngroupknights` (`,`).
    #[must_use]
    pub(crate) fn list_by(&self, delim: char) -> Vals<'a> {
        Vals::owned(split_values(self.value, delim), self.key)
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
/// A `Vals` either owns the split values — the root, created by
/// [`ResponseVal::values`], [`ResponseVal::list`] or `From<&str>` — or borrows
/// a sub-range of one as a window ([`Vals::window`]). Values are parsed into
/// the requested type on demand; in debug builds every read is tracked, so
/// values that are never read are reported when the root is dropped.
#[derive(Clone)]
pub(crate) struct Vals<'a> {
    key: &'a str,
    values: Cow<'a, [&'a str]>,
    /// Read tracking for the whole response, indexed by `base + pos`. Only
    /// present in debug builds.
    #[cfg(debug_assertions)]
    accessed: Cow<'a, [std::cell::Cell<bool>]>,
    /// The absolute position of `values[0]` within `accessed`.
    #[cfg(debug_assertions)]
    base: usize,
}

/// Splits `raw` by `delim`, trimming the surrounding whitespace and matching
/// edges. An all-whitespace value splits into nothing.
fn split_values(raw: &str, delim: char) -> Vec<&str> {
    let trimmed = raw.trim_matches(|a| [delim, ' ', '\n', '\r'].contains(&a));
    if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed.split(delim).map(str::trim).collect()
    }
}

impl<'a> Vals<'a> {
    /// A new [`Vals`] that owns `values`.
    fn owned(values: Vec<&'a str>, key: &'a str) -> Self {
        #[cfg(debug_assertions)]
        let accessed =
            Cow::Owned(vec![std::cell::Cell::new(false); values.len()]);
        Self {
            key,
            values: Cow::Owned(values),
            #[cfg(debug_assertions)]
            accessed,
            #[cfg(debug_assertions)]
            base: 0,
        }
    }

    /// Returns the response key name this [`Vals`] was created from, if known.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn key(&self) -> &str {
        self.key
    }
}

impl<'a> From<&'a str> for Vals<'a> {
    fn from(raw: &'a str) -> Self {
        Vals::owned(split_values(raw, '/'), "")
    }
}

impl<'a> Vals<'a> {
    /// Creates a new [`Vals`] by splitting `raw` by `delim`.
    #[must_use]
    pub fn by(raw: &'a str, delim: char) -> Self {
        Vals::owned(split_values(raw, delim), "")
    }
}

// Only the root owns its values, so only it warns about unread ones.
#[cfg(debug_assertions)]
impl Drop for Vals<'_> {
    fn drop(&mut self) {
        if matches!(self.values, Cow::Owned(_)) && !std::thread::panicking() {
            let unread = &self.unaccessed_indices();
            if !unread.is_empty() {
                warn!(
                    "Unaccessed response values in Vals{}: indices {unread:?} \
                     (values: {:?})",
                    key_desc(self.key),
                    self.values
                );
            }
        }
    }
}

impl PartialEq for Vals<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

impl Eq for Vals<'_> {
}

impl Debug for Vals<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.values()).finish()
    }
}

impl Display for Vals<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_values(f)
    }
}

/// Implements the trivial `parse`/`parse_or` accessors for the given types.
macro_rules! accessors {
    ($($ty:ty => $name:ident, $or:ident),+ $(,)?) => {
        $(
            #[allow(unused)]
            #[doc = concat!("Parses the value at `pos` as `", stringify!($ty), "`.")]
            ///
            /// # Errors
            /// [`SFError::ParsingError`] if `pos` is out of bounds or
            /// unparsable.
            #[track_caller]
            pub fn $name(&self, pos: usize) -> Result<$ty, SFError> {
                self.parse(pos, stringify!($ty))
            }

            #[allow(unused)]
            #[doc = concat!(
                "Parses the value at `pos` as `", stringify!($ty),
                "`, returning `default` on failure."
            )]
            #[track_caller]
            #[must_use]
            pub fn $or(&self, pos: usize, default: $ty) -> $ty {
                self.parse_or(pos, default, stringify!($ty))
            }
        )+
    };
}

/// An index into a `Vals`: a single position or a range of them, as accepted
/// by `Vals::ignore`.
pub trait IgnoreIndex {
    /// The half-open, `len`-clamped range of positions this index selects.
    fn positions(self, len: usize) -> Range<usize>;
}

impl IgnoreIndex for usize {
    fn positions(self, len: usize) -> Range<usize> {
        self.min(len)..self.saturating_add(1).min(len)
    }
}

/// Converts a range index into a concrete, `len`-clamped [`Range`].
fn bounds_to_range<R: RangeBounds<usize>>(
    range: &R,
    len: usize,
) -> Range<usize> {
    let start = match range.start_bound() {
        Bound::Included(&x) => x,
        Bound::Excluded(&x) => x.saturating_add(1),
        Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        Bound::Included(&x) => x.saturating_add(1),
        Bound::Excluded(&x) => x,
        Bound::Unbounded => len,
    };
    let start = start.min(len);
    start..end.clamp(start, len)
}

macro_rules! impl_ignore_index {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl IgnoreIndex for $ty {
                fn positions(self, len: usize) -> Range<usize> {
                    bounds_to_range(&self, len)
                }
            }
        )+
    };
}

impl_ignore_index!(
    Range<usize>,
    RangeInclusive<usize>,
    RangeTo<usize>,
    RangeToInclusive<usize>,
    RangeFrom<usize>,
    RangeFull,
);

impl<'a> Vals<'a> {
    /// The number of values.
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether there are no values.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The value at `pos`, marking it as read.
    #[must_use]
    pub(crate) fn value(&self, pos: usize) -> Option<&'a str> {
        let val = self.values.get(pos).copied()?;
        #[cfg(debug_assertions)]
        if let Some(cell) = self.accessed.get(self.base + pos) {
            cell.set(true);
        }
        Some(val)
    }

    /// All values, without marking them as read.
    #[must_use]
    pub(crate) fn values(&self) -> &[&'a str] {
        &self.values[..]
    }

    /// Marks every value as read.
    pub(crate) fn mark_all(&self) {
        #[cfg(debug_assertions)]
        if let Some(cells) =
            self.accessed.get(self.base..self.base + self.len())
        {
            for cell in cells {
                cell.set(true);
            }
        }
        #[cfg(not(debug_assertions))]
        {
            _ = self;
        }
    }

    /// The indices of the values that were never read (always empty in
    /// release builds).
    #[must_use]
    #[cfg(any(debug_assertions, test))]
    pub(crate) fn unaccessed_indices(&self) -> Vec<usize> {
        #[cfg(debug_assertions)]
        {
            (0..self.len())
                .filter(|pos| {
                    self.accessed
                        .get(self.base + pos)
                        .is_none_or(|cell| !cell.get())
                })
                .collect()
        }
        #[cfg(not(debug_assertions))]
        {
            Vec::new()
        }
    }

    /// Asserts that all values in this response were read.
    ///
    /// # Panics
    ///
    /// Panics if any value in `self` was not accessed.
    #[cfg(test)]
    #[track_caller]
    pub fn assert_all_read(&self) {
        assert!(
            self.unaccessed_indices().is_empty(),
            "unread values in {self}: {:?}",
            self.unaccessed_indices()
        );
    }

    /// The value at `pos`, marking it as read.
    #[must_use]
    pub fn get_str(&self, pos: usize) -> Option<&'a str> {
        self.value(pos)
    }

    /// The value at `pos`.
    ///
    /// # Errors
    /// [`SFError::ParsingError`] if `pos` is out of bounds.
    #[track_caller]
    pub fn get(&self, pos: usize) -> Result<&'a str, SFError> {
        let Some(val) = self.value(pos) else {
            let caller = std::panic::Location::caller();
            return Err(SFError::ParsingError(format!(
                "Index out of bounds{} (pos:{pos},len:{}) at {}:{}:{}",
                key_desc(self.key),
                self.len(),
                caller.file(),
                caller.line(),
                caller.column()
            )));
        };
        Ok(val)
    }

    /// Parses the value at `pos` into `T`.
    ///
    /// # Errors
    /// [`SFError::ParsingError`] if `pos` is out of bounds or the value does
    /// not parse.
    #[track_caller]
    fn parse<T: FromStr>(&self, pos: usize, ty: &str) -> Result<T, SFError> {
        let val = self.get(pos)?;
        val.trim().parse().map_err(|_| {
            let caller = std::panic::Location::caller();
            SFError::ParsingError(format!(
                "Invalid {ty}{} (pos:{pos},val:{val}) at {}:{}:{}",
                key_desc(self.key),
                caller.file(),
                caller.line(),
                caller.column()
            ))
        })
    }

    /// Parses the value at `pos` into `T`, warning and returning `default` if
    /// that is not possible.
    #[track_caller]
    #[must_use]
    fn parse_or<T: FromStr>(&self, pos: usize, default: T, ty: &str) -> T {
        self.parse(pos, ty).unwrap_or_else(|err| {
            warn!("{err}");
            default
        })
    }

    /// The `range` of values, marking them as read.
    #[must_use]
    pub fn slice(&self, range: Range<usize>) -> &[&'a str] {
        for pos in range.clone() {
            let _ = self.value(pos);
        }
        let end = range.end.min(self.len());
        let start = range.start.min(end);
        self.values().get(start..end).unwrap_or_default()
    }

    /// The `range` of values as a [`Vals`], a window onto this one. Positions
    /// are relative to the window, out of range values are clamped. Reads
    /// through the window mark the same values as read at the source.
    #[must_use]
    pub fn window(&self, range: Range<usize>) -> Vals<'_> {
        let end = range.end.min(self.len());
        let start = range.start.min(end);
        Vals {
            key: self.key,
            values: Cow::Borrowed(
                self.values.get(start..end).unwrap_or_default(),
            ),
            #[cfg(debug_assertions)]
            accessed: Cow::Borrowed(&self.accessed[..]),
            #[cfg(debug_assertions)]
            base: self.base + start,
        }
    }

    /// Consecutive non-overlapping chunks of `n` values, marking all of them as
    /// read.
    ///
    /// # Panics
    /// Panics if `n == 0`.
    pub fn chunks(&self, n: usize) -> impl Iterator<Item = Vals<'_>> + '_ {
        assert!(n > 0, "chunk size must be greater than 0");
        self.mark_all();
        if !self.len().is_multiple_of(n) {
            warn!(
                "Server response{} contains {} leftover values when chunked \
                 by {n}",
                key_desc(self.key),
                self.len() % n
            );
        }
        (0..self.len() / n).map(move |i| self.window(i * n..i * n + n))
    }

    /// An iterator over the values, marking all of them as read.
    #[allow(clippy::iter_without_into_iter)]
    pub fn iter(&self) -> std::iter::Copied<std::slice::Iter<'_, &'a str>> {
        self.mark_all();
        self.values().iter().copied()
    }

    /// Marks the value(s) at `index` as read, so they are not reported as
    /// unaccessed. `index` is a position or a range, like `slice::get`.
    pub fn ignore(&self, index: impl IgnoreIndex, _reason: &str) {
        for pos in index.positions(self.len()) {
            let _ = self.value(pos);
        }
    }

    /// Marks every value as read, so none is reported as unaccessed.
    pub fn ignore_all(&self, _reason: &str) {
        self.mark_all();
    }

    /// Marks every position in every range of `ranges` as read, so none is
    /// reported as unaccessed. Positions past the end of the response are
    /// skipped.
    pub fn ignore_unmodelled(&self, ranges: &[Range<usize>], reason: &str) {
        for range in ranges {
            self.ignore(range.clone(), reason);
        }
    }

    /// Writes the values like they appeared in the response.
    ///
    /// # Errors
    /// If writing to `f` fails.
    pub fn fmt_values(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        for (idx, val) in self.values().iter().enumerate() {
            if idx > 0 {
                f.write_str("/")?;
            }
            f.write_str(val)?;
        }
        Ok(())
    }

    accessors! {
        BigInt => bigint, bigint_or,
        i64 => i64, i64_or,
        u64 => u64, u64_or,
        u32 => u32, u32_or,
        i32 => i32, i32_or,
        usize => usize, usize_or,
        u16 => u16, u16_or,
        u8 => u8, u8_or,
    }

    /// The value at `pos`.
    ///
    /// # Errors
    /// [`SFError::ParsingError`] if `pos` is out of bounds.
    #[track_caller]
    pub fn str(&self, pos: usize) -> Result<&'a str, SFError> {
        self.get(pos)
    }

    /// The value at `pos`, or `default` if it is out of bounds.
    #[track_caller]
    #[must_use]
    pub fn str_or(&self, pos: usize, default: &'a str) -> &'a str {
        self.get(pos).unwrap_or_else(|err| {
            warn!("{err}");
            default
        })
    }

    /// Parses an enum at `pos` via [`FromPrimitive`], or `None` if that is not
    /// possible.
    #[must_use]
    pub fn enum_opt<T: FromPrimitive>(&self, pos: usize) -> Option<T> {
        let val = self.i64(pos).ok()?;
        T::from_i64(val).or_else(|| T::from_u64(val.cast_unsigned()))
    }

    /// Parses an enum at `pos` via [`FromPrimitive`], returning `default` if
    /// that is not possible.
    #[must_use]
    pub fn enum_or<T: FromPrimitive>(&self, pos: usize, default: T) -> T {
        self.enum_opt(pos).unwrap_or(default)
    }

    /// Parses an enum at `pos` via [`FromPrimitive`] from `f(raw)`.
    pub fn enum_map<T: FromPrimitive, F: FnOnce(u32) -> u32>(
        &self,
        pos: usize,
        f: F,
    ) -> Option<T> {
        let raw = self.u32_or(pos, 0);
        T::from_u32(f(raw))
    }

    /// Parses a known enum at `pos` via [`FromPrimitive`].
    ///
    /// # Errors
    /// [`SFError::ParsingError`] if `pos` is out of bounds or the value is
    /// not a variant of `T`.
    #[track_caller]
    pub fn parse_enum<T: FromPrimitive>(
        &self,
        pos: usize,
    ) -> Result<T, SFError> {
        let raw = self.i64(pos)?;
        T::from_i64(raw)
            .or_else(|| T::from_u64(raw.cast_unsigned()))
            .ok_or_else(|| {
                let caller = std::panic::Location::caller();
                warn!(
                    "Unknown enum variant {raw} for {} at {}:{}:{}",
                    std::any::type_name::<T>(),
                    caller.file(),
                    caller.line(),
                    caller.column()
                );
                SFError::ParsingError(format!(
                    "Unknown enum variant {raw} for {}",
                    std::any::type_name::<T>()
                ))
            })
    }

    /// The timestamp at `pos` as local time, `None` if it is unset (e.g. `0`,
    /// `-1`, `1` or `11`).
    ///
    /// # Errors
    /// [`SFError::ParsingError`] if `pos` is out of bounds or unparsable.
    #[track_caller]
    pub fn server_time(
        &self,
        pos: usize,
        server_time: ServerTime,
    ) -> Result<Option<DateTime<Local>>, SFError> {
        let raw = self.i64(pos)?;
        Ok(server_time.convert_to_local_tracked(raw))
    }

    /// The timestamp at `pos` as local time, `None` if it is unset or
    /// unparsable.
    #[track_caller]
    #[must_use]
    pub fn server_time_opt(
        &self,
        pos: usize,
        server_time: ServerTime,
    ) -> Option<DateTime<Local>> {
        self.server_time(pos, server_time).ok().flatten()
    }
}

/// Converts a S&F string from the server to their original unescaped
/// representation.
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

/// Makes a user controlled string, like a new character description, safe to
/// use in a request.
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

    #[test]
    fn test_from_str_and_windows() {
        let vals = Vals::from("1/2/3/4");
        assert_eq!(vals.len(), 4);
        assert_eq!(vals.str(0).unwrap(), "1");
        assert_eq!(vals.str(3).unwrap(), "4");

        let parsed = Vals::from("1/2/3/4");
        assert_eq!(parsed, vals);
    }

    #[test]
    fn test_caller_tracking_get_and_typed_parsers() {
        let raw = "42/-10/-/hello/255/1000/2000000000/9000000000000000000";
        let vals = Vals::from(raw);
        assert_eq!(vals.u32(0).unwrap(), 42);
        assert_eq!(vals.i32(1).unwrap(), -10);
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

        // Out of bounds error should be ParsingError with file/line
        let err = vals.get(99).unwrap_err();
        assert!(matches!(err, SFError::ParsingError(_)));
        assert!(err.to_string().contains("Index out of bounds"));

        // Type parse error should be ParsingError with file/line
        let err = vals.u32(3).unwrap_err();
        assert!(matches!(err, SFError::ParsingError(_)));
        assert!(err.to_string().contains("Invalid u32"));
    }

    #[derive(Debug, PartialEq, Eq, num_derive::FromPrimitive)]
    enum TestEnum {
        First = 1,
        Second = 2,
    }

    #[test]
    fn test_chunks_and_delimiters() {
        let rval = ResponseVal {
            key: "chunk_test",
            value: "1/2/3/4/5/6",
            sub_key: "",
        };
        let list = rval.list();
        assert_eq!(list.key(), "chunk_test");
        let chunks: Vec<_> = list.chunks(2).collect();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].key(), "chunk_test");
        assert_eq!(
            (chunks[0].u32(0).unwrap(), chunks[0].u32(1).unwrap()),
            (1, 2)
        );
        assert_eq!(
            (chunks[1].u32(0).unwrap(), chunks[1].u32(1).unwrap()),
            (3, 4)
        );
        assert_eq!(
            (chunks[2].u32(0).unwrap(), chunks[2].u32(1).unwrap()),
            (5, 6)
        );

        // Test a custom delimiter
        let comma_val = ResponseVal {
            key: "custom_delim",
            value: "10,20;30,40",
            sub_key: "",
        };
        let semi = comma_val.list_by(';');
        assert_eq!(semi.key(), "custom_delim");
        assert_eq!(semi.str(0).unwrap(), "10,20");
        assert_eq!(semi.str(1).unwrap(), "30,40");

        // A list keeps every value, however many there are
        let unbound = rval.list();
        assert_eq!(unbound.len(), 6);
        assert_eq!(unbound.key(), "chunk_test");
    }

    #[test]
    fn test_enum_helpers() {
        let raw = "1/2/99/invalid";
        let vals = Vals::from(raw);

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

    /// Every value response is one `Vals`, so a window has to behave exactly
    /// like the values it came from.
    #[test]
    fn test_vals_accessors() {
        fn check(v: &Vals<'_>) {
            assert_eq!(v.len(), 4);
            assert!(!v.is_empty());
            assert_eq!(v.u32(0).unwrap(), 1);
            assert_eq!(v.bigint(0).unwrap(), BigInt::from(1));
            assert!(v.u32(1).is_err());
            assert_eq!(v.u32_or(1, 7), 7);
            assert_eq!(v.str(1).unwrap(), "abc");
            assert_eq!(v.str_or(9, "?"), "?");
            assert_eq!(v.enum_opt::<TestEnum>(0), Some(TestEnum::First));
            assert!(v.enum_opt::<TestEnum>(2).is_none());
            assert_eq!(v.slice(0..2), ["1", "abc"]);
            assert_eq!(v.slice(9..12), Vec::<&str>::new());
            assert_eq!(v.iter().count(), 4);
            assert_eq!(v.to_string(), "1/abc/3/4");
            let err = v.get(9).unwrap_err();
            assert!(err.to_string().contains("Index out of bounds"));

            #[cfg(debug_assertions)]
            assert_eq!(v.unaccessed_indices(), Vec::<usize>::new());
            v.ignore(2, "test ignore");
            v.ignore_all("test ignore all");
        }

        let vals = Vals::from("1/abc/3/4");
        check(&vals);
        // A whole range window behaves like the values it came from
        check(&vals.window(0..4));
    }

    #[test]
    fn test_exhaustion_tracking() {
        let vals = Vals::from("10/20/30/40/50");

        #[cfg(debug_assertions)]
        assert_eq!(vals.unaccessed_indices(), vec![0, 1, 2, 3, 4]);

        assert_eq!(vals.u32(0).unwrap(), 10);
        #[cfg(debug_assertions)]
        assert_eq!(vals.unaccessed_indices(), vec![1, 2, 3, 4]);

        vals.ignore(1, "test ignore index 1");
        #[cfg(debug_assertions)]
        assert_eq!(vals.unaccessed_indices(), vec![2, 3, 4]);

        // Reading through a window marks the source, creating one does not
        let sub = vals.window(2..4);
        #[cfg(debug_assertions)]
        {
            assert_eq!(sub.unaccessed_indices(), vec![0, 1]);
            assert_eq!(vals.unaccessed_indices(), vec![2, 3, 4]);
        }
        sub.ignore_all("test ignore all sub");
        #[cfg(debug_assertions)]
        {
            assert_eq!(sub.unaccessed_indices(), Vec::<usize>::new());
            assert_eq!(vals.unaccessed_indices(), vec![4]);
        }

        vals.ignore_all("test ignore all parent");
        #[cfg(debug_assertions)]
        assert_eq!(vals.unaccessed_indices(), Vec::<usize>::new());
    }

    /// A window keeps the accessors and shared read tracking of its source.
    #[test]
    fn test_window() {
        let raw = "0/1/2/3/4/5/6/7/8";
        let vals = Vals::from(raw);

        let window = vals.window(2..5);
        assert_eq!(window.len(), 3);
        assert!(!window.is_empty());
        assert_eq!(window.values(), ["2", "3", "4"]);
        assert_eq!(window.to_string(), "2/3/4");
        assert_eq!(window.u32(0).unwrap(), 2);
        assert_eq!(window.u32(2).unwrap(), 4);
        assert!(window.u32(3).is_err());
        assert_eq!(window.u32_or(3, 9), 9);
        assert_eq!(format!("{window:?}"), r#"["2", "3", "4"]"#);

        #[cfg(debug_assertions)]
        {
            // Reads through the window mark the source, the view counts its
            // own unread values
            assert_eq!(vals.unaccessed_indices(), vec![0, 1, 3, 5, 6, 7, 8]);
            assert_eq!(window.unaccessed_indices(), vec![1]);
        }

        window.ignore_all("test ignore all window");
        #[cfg(debug_assertions)]
        {
            assert_eq!(vals.unaccessed_indices(), vec![0, 1, 5, 6, 7, 8]);
            assert_eq!(window.unaccessed_indices(), Vec::<usize>::new());
        }

        // Windows nest and keep the accessors
        let outer = vals.window(0..8);
        let nested = outer.window(2..4);
        assert_eq!(nested.values(), ["2", "3"]);
        assert_eq!(nested.u32(1).unwrap(), 3);

        // Out of range windows are clamped instead of panicking
        assert!(vals.window(9..12).is_empty());
        assert_eq!(vals.window(7..99).len(), 2);
        assert_eq!(vals.window(7..99).values(), ["7", "8"]);
    }

    /// `ignore` takes a single position or any slice-index range, and marks
    /// exactly what it selects (clamped to the response).
    #[test]
    fn test_ignore_index_and_ranges() {
        let vals = Vals::from("0/1/2/3/4/5/6/7/8");
        vals.ignore(0, "single index");
        vals.ignore(2..5, "half-open range");
        vals.ignore(7.., "range to the end");

        #[cfg(debug_assertions)]
        assert_eq!(vals.unaccessed_indices(), vec![1, 5, 6]);

        // Inclusive, full and over-long ranges are all clamped
        let vals = Vals::from("0/1/2/3");
        vals.ignore(.., "full range");
        vals.ignore(2..=99, "over-long inclusive range");
        vals.ignore(99, "out of bounds index");
        #[cfg(debug_assertions)]
        assert_eq!(vals.unaccessed_indices(), Vec::<usize>::new());
    }

    #[test]
    fn test_response_and_vals_key() {
        let resp = Response::parse(
            "charactergroup:28/31/0/1/180/1518563942/2599294/0".to_string(),
            chrono::Local::now().naive_local(),
        )
        .unwrap();

        let rval = resp.values()["charactergroup"];
        assert_eq!(rval.key(), "charactergroup");

        let vals = rval.list();
        assert_eq!(vals.key(), "charactergroup");
        assert_eq!(vals.window(0..3).key(), "charactergroup");

        // Error message includes the key name
        let err = vals.u32(99).unwrap_err();
        assert!(err.to_string().contains("charactergroup"));

        // Standalone Vals defaults to empty key
        let standalone = Vals::from("1/2/3");
        assert_eq!(standalone.key(), "");
    }
}
