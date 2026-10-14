# parsingv2: Architecture & Migration Guide

This guide documents the parsing rewrite on the `parsingv2` branch: the view
the ported modules parse through, the conventions those ports established,
and what is left of `src/gamestate/` to move over.

---

## 1. Goals & Philosophy

The original parsing turned every response value into a `Vec<i64>` up front
(`ResponseVal::into_list`) and then read it with loosely checked index
helpers. Those helpers all still live in `src/misc.rs` and carry
`#[deprecated(note = "use Vals instead")]`, so the compiler points at
whatever is left to port:

- `CGet::cget`, `CCGet::csiget` / `cimget` / `csimget`, `CSTGet::cstget`,
  `CSGet::cfsuget`, `CFPGet::cfpget` / `cfpuget`
- `ArrSkip::skip`
- the free functions `soft_into`, `warning_try_into`, `warning_parse`,
  `warning_from_str` and `update_enum_map`
- `Item::parse_slice` and `ItemType::parse_slice` in `items.rs`

They allocated for fields nobody read, deferred type errors to runtime, and
made a renamed or missing field indistinguishable from a zero.

---

## 2. The `Vals<'a>` view

`Vals<'a>` in `src/response.rs` is the one view type the rewrite settled on.
It replaced the fixed-length family the port went through — the `Vals` trait,
`ValueArray<'a, N>`, `ValueList<'a>` and `ValsWindow`.

- It either **owns** the `/`-split values of a response or **borrows a
  sub-range of one as a window**, backed by `Cow`. There is no separate
  owned/borrowed or const/runtime type any more.
- Values stay `&str` until they are read, so the fields you skip cost nothing.
- Every read marks the same shared position in the read log, whether it goes
  through a window or the response it was taken from.
- The **owning** root warns on `Drop` about the positions that were never
  read, which is how an added or renamed server field surfaces.
- Length is a runtime property. A response that is *longer* than expected is
  truncated with a warning, a shorter one is an error.
- Positions past the end of a response are skipped, never a panic.

### 2.1 Entry points

| What you have | What to call |
| --- | --- |
| a fixed number of values | `val.values::<27>()?` |
| a fixed number, other delimiter | `val.values_by::<4>(',')?` |
| a response of unknown length | `val.list()` |
| unknown length, other delimiter | `val.list_by(',')` |
| a sub-range of an already split response | `data.window(47..94)` |
| repeating blocks of equal size | `data.chunks(4)` |
| a literal, e.g. in a test | `Vals::from("1/2/3")` |

`values::<N>()` and `values_by::<N>()` take `self` by value and return
`Result<Vals<'a>>`, so they *are* the boundary check — there is no separate
conversion step. `list()` and `window()` are used inside a module once the
response is already split.

### 2.2 Accessors

Everything is parsed on demand, and every accessor carries `#[track_caller]`,
so the error and the warning message it logs name the call site as
`file:line:column`.

- **Typed, with fallbacks** — `bool`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`,
  `u32`, `u64`, `usize`, each as `data.u32(pos)?` and `data.u32_or(pos, 0)`.
- **Strings** — `data.str(pos)?`, `data.str_or(pos, "")`, plus the
  non-warning `data.get_str(pos)` and `data.get(pos)?`.
- **Enums** — `data.enum_opt(pos)`, `data.enum_or(pos, default)`,
  `data.enum_map(pos, f)` when the raw value needs shifting first, and
  `data.parse_enum(pos)?` when an unknown variant should be an error.
- **Server timestamps** — `data.server_time(idx, server_time)?` returns
  `Option<DateTime<Local>>` (`None` for the "unset" sentinels), or
  `data.server_time_opt(idx, server_time)` when a missing timestamp is fine.
- **Anything else** — `data.parse(pos, "ty")?` / `data.parse_or(pos, d, "ty")`
  for `FromStr`, and `parse_at` / `parse_opt_at` / `parse_soft_at` where the
  name of the field belongs in the message.
- **Wholesale** — `data.values()` (which does *not* mark anything read) as
  opposed to `data.as_slice()`, `data.iter()` and `data.slice(range)` (which
  do).
- **For tests** — `data.unaccessed_indices()`, the same tracker the `Drop`
  warning reports from.

### 2.3 Ignoring fields

An index that is deliberately not used has to be marked, or the `Drop` warning
will report it as unread:

```rust
data.ignore(0, "might be max unit level");
data.ignore(1..3, "an unmodelled region");
data.ignore_all("nothing in this response is modelled yet");
```

`ignore` takes a single position or a half-open range, like `slice::get`, so a
trailing "and everything after this" range is spelled `x..data.len()`.
`ignore_unmodelled` is the shorthand for a list of such ranges sharing one
reason. When you ignore something, write down what you know about it —
"always 0 in every captured response", the value range you have seen, or what
the field looks like it correlates with. A bare "not modelled yet" is the
least useful form of that comment, and the next port is the one that has to
re-derive it.

### 2.4 `_or` accessors vs `?`

Use the fallible accessor (`data.u32(idx)?`) for a field the module needs, and
the `_or` form (`data.u32_or(idx, 0)`) for one where a server change should
degrade a single value instead of failing the whole update. Both leave the
index marked as read, so neither hides the position from the tracking.

---

## 3. Blueprint: How to Port a Gamestate Module

The reference implementations are `src/gamestate/fortress.rs` (fixed-length
responses, repeating chunks zipped with an `EnumMap`) and
`src/gamestate/arena.rs` (responses whose length is only known at runtime,
`window` where a record size is a constant, and effects spread over several
formats).

### Step 1: Update Function Signatures & Types

- Import `Result` and `ServerTime` from `super`, and `Vals` from
  `crate::response`:
  ```rust
  use super::{Result, ServerTime, ...};
  use crate::response::Vals;
  ```
- Methods take `&Vals<'_>` and return the `Result<T>` alias:
  ```rust
  pub(crate) fn update(
      &mut self,
      data: &Vals<'_>,
      server_time: ServerTime,
  ) -> Result<()> {
  ```
- Replace a deprecated `parse_slice` with `parse(data: &Vals<'_>) -> Result<T>`.

### Step 2: Parse with Typed Getters

- Read fields at their real indices instead of shifting a cursor:
  ```rust
  pub(crate) fn parse(data: &Vals<'_>) -> Result<FortressCost> {
      Ok(FortressCost {
          time: Duration::from_secs(data.u64(0)?),
          silver: data.u64(1)?,
          wood: data.u64(2)?,
          stone: data.u64(3)?,
      })
  }
  ```
- **Repeating structures** — `chunks(n)` zipped with the values of an
  `EnumMap` (it marks the whole response as read, and warns about a leftover
  tail):
  ```rust
  for (unit, cost_vals) in self.units.values_mut().zip(data.chunks(4)) {
      unit.training.cost = FortressCost::parse(&cost_vals)?;
  }
  ```
- **Optional values** — filter the sentinel instead of branching on a parse
  error: `Some(data.u32(idx)?).filter(|&x| x != 0)`, or for a 1-based enum
  index `data.u32(idx)?.checked_sub(1).and_then(EnumType::from_u32)`.
- **Fixed-size records inside a longer response** — `data.window(range)` and
  parse the offset-free window:
  ```rust
  Fighter::parse(&data.window(0..FIGHTER_RECORD_LEN))
  ```

### Step 3: Wire into `src/gamestate/mod.rs`

Add an arm to the central `apply_update_key` match, which `GameState::update`
calls once per key of a response:

```rust
"fortress" => {
    self.fortress
        .get_or_insert_default()
        .update(&val.values::<27>()?, server_time)?;
}
"arena" => self.arena.update(&val.values::<7>()?, server_time)?,
```

Keys that arrive under several names are matched with a guard instead
(`x if x.starts_with("fightheader")`, `x if x.starts_with("fight") && x.len()
<= 7`), and that is where a name is turned into an index or a version.

### Step 4: Add Unit Tests with Real Data

Add tests at the bottom of the module in `#[cfg(test)] mod tests`. Real
fixtures come from `cache/`: `cache/*.login` holds the response of a whole
login, `cache/*.response` holds one response per command (the `body` field is
the `key:value&key:value` payload).

`crate::parse_test!` covers the common shapes:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fortress_cost_parse() {
        crate::parse_test!(FortressCost("7200/4000/1100/333"), {
            time: Duration::from_secs(7200),
            silver: 4000,
            wood: 1100,
            stone: 333,
        });
    }

    // `update:` runs a method on a target and then checks fields:
    #[test]
    fn test_fortress_update_prices() {
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress.update_prices("7200/4000/1100/333"), {});
        assert_eq!(fortress.gem_search.cost.wood, 29);
    }
}
```

The arms are `Ty(raw, { fields })`, `opt: Ty(raw, { fields })` for a parser
returning `Option`, `Ty(raw) => expected` for a whole-value comparison, and
`update: target.method(raw, { fields })` (or `update: target(raw, ..)` for
`update` itself).

Two habits worth keeping. Assert that the parser consumed everything, using
the same tracker the `Drop` warning reports from:

```rust
let data = Vals::from(raw);
fight.update_result(&data);
assert!(data.unaccessed_indices().is_empty(), "unread values in {data}");
```

That is what catches an `ignore` range with an off-by-one, or a field the
parser quietly stopped reading. And where a branch has no fixture at all —
the 15-value round stride in `arena.rs` has none in `cache/` — say so in the
test. A synthetic record that documents the assumption is better than no
coverage, but it is not evidence that the format is real.

---

## 4. General Code Quality Standards Established

### 1. `Result<T>` Type Alias

Defined in `src/error.rs` as:
```rust
pub type Result<T, E = SFError> = std::result::Result<T, E>;
```
Re-exported in `src/lib.rs` and `src/gamestate/mod.rs`. Always use `Result<T>`
and `Result<()>` rather than `Result<T, SFError>`.

### 2. Standardized Derived Traits

Derived traits follow a consistent ordering:
- **Structs (Non-Copy)**:
  ```rust
  #[derive(Debug, Default, Clone, PartialEq, Eq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  ```
- **Structs (Copy)**:
  ```rust
  #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  ```
- **Enums**:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Enum, EnumCount, EnumIter, FromPrimitive)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  ```
*(Order: std Debug/Clone/Copy -> std PartialEq/Eq -> Enum -> strum ->
num_derive.)* A type that a downstream crate will match on, or that will grow
variants as the server adds fight or item kinds, gets `#[non_exhaustive]` on
top.

### 3. Enum Methods on `Copy` Types

Methods on `Copy` enums take `self` rather than `&self`:
```rust
pub fn required_min_fortress_level(self) -> u16
```

### 4. `EnumMap` Direct Indexing

`EnumMap` natively implements `Index` and `IndexMut`; the `EnumMapGet`
extension trait is deprecated, so use the indexing form in new code and drop
the import as you port:
```rust
let building = &self.buildings[FortressBuildingType::Fortress];
let current_wood = self.resources[FortressResourceType::Wood].current;
```

### 5. `PlayerId` Type Alias

`PlayerId` in `src/lib.rs` is `pub type PlayerId = u32;`. It belongs in the
public surface (`Arena::enemy_ids`, `SingleFight::winner_id`) where it
documents the meaning; inside a module, plain `u32` avoids the import.

### 6. Guard Clauses & Early Returns

Avoid deeply nested `if / else if / else` blocks in validation functions (like
`Fortress::can_build`). Prefer flat early-exit guards:
```rust
if self.in_use(building_type) || self.building_upgrade.target.is_some() {
    return false;
}
if building_type.required_min_fortress_level() > fortress_level {
    return false;
}
```

### 7. Documentation Integrity

- Never leave `#[allow(missing_docs)]` on public enums or structs. Document
  every variant and field.
- Double-check game mechanics (e.g. Fortress vs Underworld naming like
  Fortress main building vs Heart of Darkness).
- Say what a field *is*, not what it is called: an index that only carries a
  guessed name is worse than one documented as "unknown, seen in the 100-400
  range for players only".

---

## 5. Where the Port Stands

**Ported** (parse through `Vals`, no deprecated helpers left):

| Module | Notes |
| --- | --- |
| `arena.rs` | arena, fights, rounds; variable-length responses |
| `fortress.rs` | the reference implementation |
| `unlockables.rs` | scrapbook, pets, hellevator, blacksmith, witch, achievements |

**Partly ported**: in `items.rs`, `Item::parse` and `ItemType::parse` work on
`Vals`, but the deprecated `parse_slice` twins and their call sites in
`mod.rs` remain.

**Remaining**, by how much deprecated API each file still uses (a snapshot, in
descending order):

| Module | Handles |
| --- | --- |
| `mod.rs` | the `apply_update_key` match itself: character save and attributes, and every key without a home yet |
| `social.rs` | lookups, other players/guilds, fortress and pet bonus views |
| `event.rs` | world boss event and battle, upgrade and projectile offers |
| `legendary_dungeon.rs` | legendary dungeon event, merchant, gems of fate |
| `guild.rs` | members, portal, raid, group prices and saves, chat messages |
| `underworld.rs` | `underworld`, building prices, unit prices and levels, soul extraction |
| `items.rs` | inventory and equipment, plus the `parse_slice` twins |
| `rewards.rs` | reward parser, timed specials, wheel, calendar, tasks |
| `tavern.rs` | quests, toilet state, gambler, encounters |
| `character.rs` | the class/race tables, mount and portrait parsing |
| `dungeons.rs` | light dungeon progress, companion equipment |

---

## 6. Verification Commands

Run these to verify clean compilation, formatting, and tests:
```bash
# Check formatting (the config uses nightly-only options)
cargo +nightly fmt --check

# Run unit tests and doctests
cargo test

# Run clippy (deprecated usage is expected until the port is done)
cargo clippy --tests -- -A deprecated
```

CI runs the same ground from `.github/workflows/rust.yml`: a build with
`--no-default-features`, `cargo test --release`, nightly `rustfmt`, and
`cargo clippy -- -D warnings` on `main`. That last job cannot pass yet — the
crate still has warnings from the unported modules — so treat it as the
target, not a gate, while the migration is open.
