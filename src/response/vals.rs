//! Lazy, typed access to slash-separated response values.
//!
//! This module is introduced before the parser migrations. Until the first
//! consumers move over, the migration seam is intentionally unused in normal
//! library builds.
#![allow(dead_code)]

#[cfg(any(debug_assertions, test))]
use std::cell::Cell;
use std::{
    borrow::Cow,
    fmt::{Debug, Display, Formatter},
    ops::{
        Bound, Range, RangeBounds, RangeFrom, RangeFull, RangeInclusive,
        RangeTo, RangeToInclusive,
    },
    str::FromStr,
};

use chrono::{DateTime, Local};
use log::{trace, warn};
use num_bigint::BigInt;
use num_traits::FromPrimitive;

use crate::{error::SFError, gamestate::ServerTime};

/// A sequence of string values from a server response.
///
/// A root `Vals` owns the split values. A [`window`](Self::window) borrows a
/// sub-range and shares read tracking with its root. Values are parsed on
/// demand; debug and test builds report values that were never read or
/// explicitly ignored.
#[derive(Clone)]
pub(crate) struct Vals<'a> {
    key: &'a str,
    values: Cow<'a, [&'a str]>,
    #[cfg(any(debug_assertions, test))]
    accessed: Cow<'a, [Cell<bool>]>,
    #[cfg(any(debug_assertions, test))]
    base: usize,
}

/// Formats optional response-key context without allocating when absent.
fn key_desc(key: &str) -> impl Display + '_ {
    std::fmt::from_fn(move |f| {
        if key.is_empty() {
            Ok(())
        } else {
            write!(f, " for key {key:?}")
        }
    })
}

/// Splits `raw`, trimming surrounding whitespace and matching delimiters.
fn split_values(raw: &str, delim: char) -> Vec<&str> {
    let trimmed = raw.trim_matches(|c: char| c == delim || c.is_whitespace());
    if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed.split(delim).map(str::trim).collect()
    }
}

impl<'a> Vals<'a> {
    /// Creates a root `Vals` from a response value.
    pub(super) fn new(raw: &'a str, key: &'a str, delim: char) -> Self {
        Self::owned(split_values(raw, delim), key)
    }

    fn owned(values: Vec<&'a str>, key: &'a str) -> Self {
        #[cfg(any(debug_assertions, test))]
        let len = values.len();
        Self {
            key,
            values: Cow::Owned(values),
            #[cfg(any(debug_assertions, test))]
            accessed: Cow::Owned(vec![Cell::new(false); len]),
            #[cfg(any(debug_assertions, test))]
            base: 0,
        }
    }

    /// Returns the response key this value list came from.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn key(&self) -> &str {
        self.key
    }
}

impl<'a> From<&'a str> for Vals<'a> {
    fn from(raw: &'a str) -> Self {
        Self::new(raw, "", '/')
    }
}

impl<'a> Vals<'a> {
    /// Creates a root `Vals` using a custom delimiter.
    #[must_use]
    pub(crate) fn by(raw: &'a str, delim: char) -> Self {
        Self::new(raw, "", delim)
    }
}

// Only roots own their values, so only roots warn about unread values.
#[cfg(any(debug_assertions, test))]
impl Drop for Vals<'_> {
    fn drop(&mut self) {
        if matches!(self.values, Cow::Owned(_)) && !std::thread::panicking() {
            let unread = self.unaccessed_indices();
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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.values()).finish()
    }
}

impl Display for Vals<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.fmt_values(f)
    }
}

/// Implements the trivial typed accessors.
macro_rules! accessors {
    ($($ty:ty => $name:ident, $or:ident),+ $(,)?) => {
        $(
            #[doc = concat!(
                "Parses the value at `pos` as `",
                stringify!($ty),
                "`."
            )]
            ///
            /// # Errors
            /// [`SFError::ParsingError`] if `pos` is out of bounds or the
            /// value cannot be parsed.
            #[track_caller]
            pub(crate) fn $name(&self, pos: usize) -> Result<$ty, SFError> {
                self.parse(pos, stringify!($ty))
            }

            #[doc = concat!(
                "Parses the value at `pos` as `",
                stringify!($ty),
                "`, returning `default` if the position is out of bounds or the value cannot be parsed."
            )]
            #[track_caller]
            #[must_use]
            pub(crate) fn $or(&self, pos: usize, default: $ty) -> $ty {
                self.parse_or(pos, default, stringify!($ty))
            }
        )+
    };
}

/// An index accepted by [`Vals::ignore`].
pub(crate) trait IgnoreIndex {
    /// The half-open, length-clamped range selected by this index.
    fn positions(self, len: usize) -> Range<usize>;
}

impl IgnoreIndex for usize {
    fn positions(self, len: usize) -> Range<usize> {
        self.min(len)..self.saturating_add(1).min(len)
    }
}

/// Converts a range index into a concrete, length-clamped range.
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
        let value = self.values.get(pos).copied()?;
        #[cfg(any(debug_assertions, test))]
        if let Some(cell) = self.accessed.get(self.base + pos) {
            cell.set(true);
        }
        Some(value)
    }

    /// All values, without marking them as read.
    #[must_use]
    pub(crate) fn values(&self) -> &[&'a str] {
        &self.values[..]
    }

    /// Marks every value as read.
    pub(crate) fn mark_all(&self) {
        #[cfg(any(debug_assertions, test))]
        if let Some(cells) =
            self.accessed.get(self.base..self.base + self.len())
        {
            for cell in cells {
                cell.set(true);
            }
        }
    }

    /// Returns the relative indices of values that were never read.
    #[must_use]
    #[cfg(any(debug_assertions, test))]
    pub(crate) fn unaccessed_indices(&self) -> Vec<usize> {
        (0..self.len())
            .filter(|pos| {
                self.accessed
                    .get(self.base + pos)
                    .is_none_or(|cell| !cell.get())
            })
            .collect()
    }

    /// Asserts that every value was read or explicitly ignored.
    ///
    /// # Panics
    /// Panics if any value remains unread.
    #[cfg(test)]
    #[track_caller]
    pub(crate) fn assert_all_read(&self) {
        assert!(
            self.unaccessed_indices().is_empty(),
            "unread values in {self}: {:?}",
            self.unaccessed_indices()
        );
    }

    /// The value at `pos`, marking it as read.
    #[must_use]
    pub(crate) fn get_str(&self, pos: usize) -> Option<&'a str> {
        self.value(pos)
    }

    /// The value at `pos`.
    ///
    /// # Errors
    /// [`SFError::ParsingError`] if `pos` is out of bounds.
    #[track_caller]
    pub(crate) fn get(&self, pos: usize) -> Result<&'a str, SFError> {
        let Some(value) = self.value(pos) else {
            let caller = std::panic::Location::caller();
            return Err(SFError::ParsingError(
                "response value",
                format!(
                    "Index out of bounds{} (pos:{pos},len:{}) at {}:{}:{}",
                    key_desc(self.key),
                    self.len(),
                    caller.file(),
                    caller.line(),
                    caller.column()
                ),
            ));
        };
        Ok(value)
    }

    /// Parses the value at `pos` into `T`.
    #[track_caller]
    fn parse<T: FromStr>(&self, pos: usize, ty: &str) -> Result<T, SFError> {
        let caller = std::panic::Location::caller();
        let value = self.get(pos)?;
        value.trim().parse().map_err(|_| {
            SFError::ParsingError(
                "response value",
                format!(
                    "Invalid {ty}{} (pos:{pos},val:{value}) at {}:{}:{}",
                    key_desc(self.key),
                    caller.file(),
                    caller.line(),
                    caller.column()
                ),
            )
        })
    }

    /// Parses a value, warning and returning `default` on failure.
    #[track_caller]
    #[must_use]
    fn parse_or<T: FromStr>(&self, pos: usize, default: T, ty: &str) -> T {
        self.parse(pos, ty).unwrap_or_else(|err| {
            warn!("{err}");
            default
        })
    }

    /// The selected values, marking them as read.
    #[must_use]
    pub(crate) fn slice(&self, range: Range<usize>) -> &[&'a str] {
        let end = range.end.min(self.len());
        let start = range.start.min(end);
        for pos in start..end {
            let _ = self.value(pos);
        }
        self.values().get(start..end).unwrap_or_default()
    }

    /// A window onto `range`, with positions relative to the window.
    ///
    /// Out-of-range bounds are clamped. Reads through the window mark the
    /// same values as read in the source.
    #[must_use]
    pub(crate) fn window(&self, range: Range<usize>) -> Vals<'_> {
        let end = range.end.min(self.len());
        let start = range.start.min(end);
        Vals {
            key: self.key,
            values: Cow::Borrowed(
                self.values.get(start..end).unwrap_or_default(),
            ),
            #[cfg(any(debug_assertions, test))]
            accessed: Cow::Borrowed(&self.accessed[..]),
            #[cfg(any(debug_assertions, test))]
            base: self.base + start,
        }
    }

    /// Consecutive non-overlapping chunks of `n` values.
    ///
    /// Creating or collecting chunks does not mark them read. Only values read
    /// through the returned windows are tracked. Leftover values are warned
    /// about and remain unread.
    ///
    /// # Panics
    /// Panics if `n == 0`.
    pub(crate) fn chunks(
        &self,
        n: usize,
    ) -> impl Iterator<Item = Vals<'_>> + '_ {
        assert!(n > 0, "chunk size must be greater than 0");
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

    /// Iterates over values, marking each one read only when yielded.
    #[allow(clippy::iter_without_into_iter)]
    pub(crate) fn iter(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.values().iter().enumerate().map(move |(pos, value)| {
            let _ = self.value(pos);
            *value
        })
    }

    /// Marks the selected value(s) read and records the rationale at trace
    /// level. `index` is a position or a slice-style range.
    pub(crate) fn ignore(&self, index: impl IgnoreIndex, reason: &str) {
        let range = index.positions(self.len());
        trace!("Ignoring {range:?} in Vals{}: {reason}", key_desc(self.key));
        for pos in range {
            let _ = self.value(pos);
        }
    }

    /// Marks every value read and records the rationale at trace level.
    pub(crate) fn ignore_all(&self, reason: &str) {
        trace!(
            "Ignoring all values in Vals{}: {reason}",
            key_desc(self.key)
        );
        self.mark_all();
    }

    /// Marks every position in `ranges` read.
    pub(crate) fn ignore_unmodelled(
        &self,
        ranges: &[Range<usize>],
        reason: &str,
    ) {
        for range in ranges {
            self.ignore(range.clone(), reason);
        }
    }

    /// Writes the values like they appeared in the response.
    ///
    /// # Errors
    /// Returns an error if writing to the formatter fails.
    pub(crate) fn fmt_values(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (idx, value) in self.values().iter().enumerate() {
            if idx > 0 {
                f.write_str("/")?;
            }
            f.write_str(value)?;
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
    pub(crate) fn str(&self, pos: usize) -> Result<&'a str, SFError> {
        self.get(pos)
    }

    /// The value at `pos`, or `default` if it is out of bounds.
    #[track_caller]
    #[must_use]
    pub(crate) fn str_or(&self, pos: usize, default: &'a str) -> &'a str {
        self.get(pos).unwrap_or_else(|err| {
            warn!("{err}");
            default
        })
    }

    /// Parses an enum at `pos`, returning `None` if it is out of bounds,
    /// unparsable, or not a known variant.
    #[must_use]
    pub(crate) fn enum_opt<T: FromPrimitive>(&self, pos: usize) -> Option<T> {
        let value = self.i64(pos).ok()?;
        T::from_i64(value).or_else(|| T::from_u64(value.cast_unsigned()))
    }

    /// Parses an enum at `pos`, returning `default` if it is out of bounds,
    /// unparsable, or not a known variant.
    #[must_use]
    pub(crate) fn enum_or<T: FromPrimitive>(
        &self,
        pos: usize,
        default: T,
    ) -> T {
        self.enum_opt(pos).unwrap_or(default)
    }

    /// Parses an enum at `pos` after applying `f` to its raw `u32` value.
    pub(crate) fn enum_map<T: FromPrimitive, F: FnOnce(u32) -> u32>(
        &self,
        pos: usize,
        f: F,
    ) -> Option<T> {
        let raw = self.u32(pos).ok()?;
        T::from_u32(f(raw))
    }

    /// Parses a known enum at `pos`.
    ///
    /// # Errors
    /// [`SFError::ParsingError`] if `pos` is out of bounds, the value cannot
    /// be parsed, or it is not a variant of `T`.
    #[track_caller]
    pub(crate) fn parse_enum<T: FromPrimitive>(
        &self,
        pos: usize,
    ) -> Result<T, SFError> {
        let caller = std::panic::Location::caller();
        let raw = self.i64(pos)?;
        T::from_i64(raw)
            .or_else(|| T::from_u64(raw.cast_unsigned()))
            .ok_or_else(|| {
                let message = format!(
                    "Unknown enum variant {raw} for {}{} at {}:{}:{}",
                    std::any::type_name::<T>(),
                    key_desc(self.key),
                    caller.file(),
                    caller.line(),
                    caller.column()
                );
                warn!("{message}");
                SFError::ParsingError("response value", message)
            })
    }

    /// Parses a server timestamp as local time.
    ///
    /// # Errors
    /// [`SFError::ParsingError`] if `pos` is out of bounds or unparsable.
    #[track_caller]
    pub(crate) fn server_time(
        &self,
        pos: usize,
        server_time: ServerTime,
    ) -> Result<Option<DateTime<Local>>, SFError> {
        let raw = self.i64(pos)?;
        let name = if self.key.is_empty() {
            "response timestamp"
        } else {
            self.key
        };
        Ok(server_time.convert_to_local(raw, name))
    }

    /// Parses a server timestamp, returning `None` on any failure.
    #[track_caller]
    #[must_use]
    pub(crate) fn server_time_opt(
        &self,
        pos: usize,
        server_time: ServerTime,
    ) -> Option<DateTime<Local>> {
        self.server_time(pos, server_time).ok().flatten()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    #[derive(Debug, PartialEq, Eq, num_derive::FromPrimitive)]
    enum TestEnum {
        First = 1,
        Second = 2,
    }

    #[track_caller]
    fn parse_u32(vals: &Vals<'_>, pos: usize) -> Result<u32, SFError> {
        vals.u32(pos)
    }

    #[test]
    fn splits_and_trims_values() {
        let vals = Vals::from(" \t/1/\t2/\r\n ");
        assert_eq!(vals.values(), ["1", "2"]);
        assert_eq!(vals.to_string(), "1/2");
        let same = Vals::from("1/2");
        assert_eq!(vals, same);
        vals.ignore_all("test values");
        same.ignore_all("test equal values");
        assert!(Vals::from(" \t\r\n ").is_empty());
        let custom = Vals::by("10,20;30", ',');
        assert_eq!(custom.values(), ["10", "20;30"]);
        custom.ignore_all("test custom delimiter");
    }

    #[test]
    fn typed_accessors_and_defaults() {
        let vals = Vals::from(
            "42/-10/hello/255/1000/2000000000/9000000000000000000/\
             9007199254740993",
        );
        assert_eq!(vals.u32(0).unwrap(), 42);
        assert_eq!(vals.i32(1).unwrap(), -10);
        assert_eq!(vals.str(2).unwrap(), "hello");
        assert_eq!(vals.u8(3).unwrap(), 255);
        assert_eq!(vals.u16(4).unwrap(), 1_000);
        assert_eq!(vals.usize(5).unwrap(), 2_000_000_000);
        assert_eq!(vals.i64(6).unwrap(), 9_000_000_000_000_000_000);
        assert_eq!(
            vals.bigint(7).unwrap(),
            BigInt::from(9_007_199_254_740_993_u64)
        );

        assert_eq!(vals.u32_or(0, 0), 42);
        assert_eq!(vals.u32_or(2, 7), 7);
        assert_eq!(vals.u32_or(99, 9), 9);
        assert_eq!(vals.str_or(99, "default"), "default");
    }

    #[test]
    fn errors_include_key_and_caller() {
        let vals = Vals::new("not-a-number", "resources", '/');
        let expected_line = line!() + 1;
        let err = parse_u32(&vals, 0).unwrap_err();
        assert!(matches!(err, SFError::ParsingError(_, _)));
        assert!(err.to_string().contains("resources"));
        assert!(err.to_string().contains(&format!(":{expected_line}:")));

        let err = vals.u32(1).unwrap_err();
        assert!(err.to_string().contains("Index out of bounds"));
        assert!(err.to_string().contains(file!()));

        let enums = Vals::new("99", "enums", '/');
        let expected_line = line!() + 1;
        let err = enums.parse_enum::<TestEnum>(0).unwrap_err();
        assert!(err.to_string().contains("Unknown enum variant"));
        assert!(err.to_string().contains("enums"));
        assert!(err.to_string().contains(&format!(":{expected_line}:")));
    }

    #[test]
    fn enum_helpers() {
        let vals = Vals::from("1/2/99/invalid");
        assert_eq!(vals.enum_opt::<TestEnum>(0), Some(TestEnum::First));
        assert_eq!(vals.enum_opt::<TestEnum>(1), Some(TestEnum::Second));
        assert_eq!(vals.enum_opt::<TestEnum>(2), None);
        assert_eq!(vals.enum_or(2, TestEnum::First), TestEnum::First);
        assert_eq!(
            vals.enum_map::<TestEnum, _>(0, |raw| raw + 1),
            Some(TestEnum::Second)
        );
        assert_eq!(vals.enum_map::<TestEnum, _>(3, |raw| raw - 1), None);
        assert_eq!(vals.enum_map::<TestEnum, _>(99, |raw| raw - 1), None);
        assert_eq!(vals.parse_enum::<TestEnum>(0).unwrap(), TestEnum::First);
        assert!(vals.parse_enum::<TestEnum>(2).is_err());
    }

    #[test]
    fn windows_share_read_tracking() {
        let vals = Vals::from("0/1/2/3/4/5");
        assert_eq!(vals.unaccessed_indices(), vec![0, 1, 2, 3, 4, 5]);

        let window = vals.window(2..5);
        assert_eq!(window.values(), ["2", "3", "4"]);
        assert_eq!(window.u32(0).unwrap(), 2);
        assert_eq!(vals.unaccessed_indices(), vec![0, 1, 3, 4, 5]);
        assert_eq!(window.unaccessed_indices(), vec![1, 2]);

        window.ignore_all("test window");
        assert_eq!(vals.unaccessed_indices(), vec![0, 1, 5]);
        assert!(vals.window(99..100).is_empty());
        vals.ignore_all("test remaining values");
    }

    #[test]
    fn slices_and_nested_windows_mark_the_source() {
        let vals = Vals::from("0/1/2/3/4/5");
        assert_eq!(vals.slice(1..3), ["1", "2"]);
        assert_eq!(vals.unaccessed_indices(), vec![0, 3, 4, 5]);

        let outer = vals.window(3..6);
        let nested = outer.window(1..3);
        assert_eq!(nested.u32(0).unwrap(), 4);
        assert_eq!(vals.unaccessed_indices(), vec![0, 3, 5]);
        vals.ignore_all("test remaining values");

        let all = Vals::from("0/1");
        assert_eq!(all.slice(0..usize::MAX), ["0", "1"]);
        all.assert_all_read();
    }

    #[test]
    fn chunks_only_mark_values_actually_read() {
        let vals = Vals::from("1/2/3/4/5");
        let chunks: Vec<_> = vals.chunks(2).collect();
        assert_eq!(chunks.len(), 2);
        assert_eq!(vals.unaccessed_indices(), vec![0, 1, 2, 3, 4]);

        assert_eq!(chunks[0].u32(0).unwrap(), 1);
        assert_eq!(vals.unaccessed_indices(), vec![1, 2, 3, 4]);
        assert_eq!(chunks[1].u32(1).unwrap(), 4);
        assert_eq!(vals.unaccessed_indices(), vec![1, 2, 4]);

        vals.ignore(1..3, "unread chunk fields");
        vals.ignore(4, "leftover");
        vals.assert_all_read();
    }

    #[test]
    fn iterator_marks_values_as_they_are_yielded() {
        let vals = Vals::from("1/2/3");
        let mut iter = vals.iter();
        assert_eq!(iter.next(), Some("1"));
        assert_eq!(vals.unaccessed_indices(), vec![1, 2]);
        assert_eq!(iter.next(), Some("2"));
        assert_eq!(vals.unaccessed_indices(), vec![2]);
        drop(iter);
        assert_eq!(vals.unaccessed_indices(), vec![2]);
        vals.ignore(2, "not consumed");
    }

    #[test]
    fn ignore_accepts_positions_and_ranges() {
        let vals = Vals::from("0/1/2/3/4/5/6/7/8");
        vals.ignore(0, "single");
        vals.ignore(2..5, "half-open");
        vals.ignore(7.., "to end");
        assert_eq!(vals.unaccessed_indices(), vec![1, 5, 6]);

        vals.ignore(.., "everything else");
        vals.assert_all_read();

        let vals = Vals::from("0/1/2/3");
        vals.ignore(2..=99, "clamped inclusive");
        vals.ignore(99, "past end");
        vals.ignore_unmodelled(&[0..2, 2..4], "remaining ranges");
        vals.assert_all_read();
    }

    #[test]
    fn server_time_accessors() {
        let vals = Vals::from("0/1700000000/invalid");
        let server_time = ServerTime::default();
        assert_eq!(vals.server_time(0, server_time).unwrap(), None);
        assert!(vals.server_time(1, server_time).unwrap().is_some());
        assert!(vals.server_time(2, server_time).is_err());
        assert_eq!(vals.server_time_opt(2, server_time), None);
    }

    #[test]
    fn key_is_preserved() {
        let vals = Vals::new("1/2", "example", '/');
        assert_eq!(vals.key(), "example");
        assert_eq!(vals.window(1..2).key(), "example");
        vals.ignore_all("test key");
    }
}
