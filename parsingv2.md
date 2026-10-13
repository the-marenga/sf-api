# parsingv2: Architecture & Migration Guide

This guide documents the parsing rewrite in `sf-api` (`parsingv2`), the patterns established during the migration, and actionable instructions for porting remaining modules across the crate.

---

## 1. Goals & Philosophy

The original parsing relied on converting raw response strings to `Vec<i64>` slices (`into_list`), coupled with loose index lookups (`cget`, `csiget`, `ArrSkip::skip`, `parse_slice`). This caused:
- Excessive heap allocations (`Vec<i64>` for every key).
- Inability to detect missing or unparsed fields.
- Type errors deferred to runtime.
- Silent bugs when servers updated array schemas.

### The `ResponseVals<N>` Solution
- **Compile-Time Length Guarantee**: `ResponseVals<const N: usize>` guarantees that exactly `N` values are present in the response slice. If the server sends an unexpected number of values, `try_into()` fails immediately with a clear error.
- **Zero Allocations for Untouched Fields**: Internally backed by `Box<[&'a str; N]>`. Strings are only parsed into typed primitives (`u32`, `u64`, `bool`, etc.) on demand.
- **Exhaustion Tracking**: In debug mode (`cfg(debug_assertions)`), every index tracks whether it was read. If `ResponseVals` is dropped with unread fields, a warning is logged identifying the exact unread indices.
- **Explicit Ignored Fields**: Any field that is intentionally skipped must be marked with `data.ignore(idx, "reason")` with an explanatory comment.

---

## 2. Blueprint: How to Migrate a Gamestate Module

Using `src/gamestate/fortress.rs` as the reference implementation, follow these steps when migrating a module (e.g. `underworld.rs`, `dungeons.rs`, `tavern.rs`, `arena.rs`):

### Step 1: Update Function Signatures & Types
- Import `Result` from `super`:
  ```rust
  use super::{Result, ServerTime, ...};
  use crate::response::ResponseVals;
  ```
- Change internal update/parse methods to take `&ResponseVals<N>` and return the new `Result<()>` or `Result<T>`:
  ```rust
  pub(crate) fn update(
      &mut self,
      data: &ResponseVals<27>,
      server_time: ServerTime,
  ) -> Result<()> {
  ```
- Remove any deprecated `parse_slice` methods in favor of `parse(data: &ResponseVals<N>) -> Result<T>`.

### Step 2: Use Typed Getters & Idiomatic Iteration
- **Typed Accessors**:
  - Unsigned: `data.u16(i)?`, `data.u32(i)?`, `data.u64(i)?`, `data.usize(i)?`
  - Signed: `data.i16(i)?`, `data.i32(i)?`, `data.i64(i)?`
  - Fallbacks: `data.u32_or(i, default)`
  - Strings & Booleans: `data.str(i)?`, `data.bool(i)?`
- **Server Timestamps**:
  ```rust
  self.finish = data.server_time(idx, server_time)?;
  ```
- **Optional Values**:
  - Filter non-zero IDs: `Some(data.u32(idx)?).filter(|&x| x != 0)`
  - 1-based enum indices (0 = None):
    ```rust
    target: data.u32(idx)?.checked_sub(1).and_then(EnumType::from_u32),
    ```
- **Fixed Chunks**:
  When iterating repeating structures (e.g. costs or units), use `chunks_exact::<M>()` zipped with enum maps or slices:
  ```rust
  for (unit, cost_vals) in self.units.values_mut().zip(data.chunks_exact::<4>()) {
      unit.training.cost = FortressCost::parse(&cost_vals)?;
  }
  ```
- **Ignore Unused / Unknown Fields**:
  ```rust
  data.ignore(0, "might be max unit level");
  data.ignore(20, "unknown server timestamp");
  ```

### Step 3: Wire into `src/gamestate/mod.rs`
In the central `update_gamestate` match statement in `src/gamestate/mod.rs`:
```rust
"fortress" => {
    let data: ResponseVals<27> = val.try_into()?;
    self.fortress.get_or_insert_default().update(&data, server_time)?;
}
```

### Step 4: Add Unit Tests with Real Data
Add tests at the bottom of the module in `#[cfg(test)] mod tests` using `parse_test!`. Real test data can be retrieved from cached account logs in `cache/*.login`.
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

    #[test]
    fn test_fortress_update_prices() {
        let raw = "7200/4000/1100/333/...";
        let mut fortress = Fortress::default();
        crate::parse_test!(update: fortress.update_prices(raw), {});
        assert_eq!(fortress.gem_search.cost.wood, 29);
    }
}
```

---

## 3. General Code Quality Standards Established

### 1. `Result<T>` Type Alias
Defined in `src/error.rs` as:
```rust
pub type Result<T, E = SFError> = std::result::Result<T, E>;
```
Re-exported in `src/lib.rs` and `src/gamestate/mod.rs`. Always use `Result<T>` and `Result<()>` rather than `Result<T, SFError>`.

### 2. Standardized Derived Traits
Derived traits must follow a consistent, standardized ordering:
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
*(Order: std Debug/Clone/Copy -> std PartialEq/Eq -> Enum -> strum -> num_derive).*

### 3. Enum Methods on `Copy` Types
Methods on `Copy` enums should take `self` rather than `&self`:
```rust
pub fn required_min_level(self) -> u16
```

### 4. `EnumMap` Direct Indexing
Do NOT use the custom extension trait `EnumMapGet` (`.get()` / `.get_mut()`). `EnumMap` natively implements `Index` and `IndexMut`:
```rust
let building = &self.buildings[FortressBuildingType::Fortress];
let current_wood = self.resources[FortressResourceType::Wood].current;
```

### 5. `PlayerId` Type Alias
`PlayerId` in `src/lib.rs` is `pub type PlayerId = u32;`. In internal gamestate modules, use `u32` directly (e.g. `Option<u32>`) to avoid unnecessary imports.

### 6. Guard Clauses & Early Returns
Avoid deeply nested `if / else if / else` blocks in validation functions (like `can_build` or `can_upgrade`). Prefer flat early-exit guards:
```rust
if self.in_use(building_type) || self.building_upgrade.target.is_some() {
    return false;
}
if building_type.required_min_fortress_level() > fortress_level {
    return false;
}
```

### 7. Documentation Integrity
- Never leave `#[allow(missing_docs)]` on public enums or structs. Document every variant and field.
- Double-check game mechanics (e.g. Fortress vs Underworld naming like Fortress main building vs Heart of Darkness).

---

## 4. Modules Remaining to Port

The following modules in `src/gamestate/` still contain deprecated slice-parsing methods or are partially migrated:

1. **`underworld.rs`**:
   - `underworld` response string.
   - Building levels, production rates, soul extraction, and underworld units (Goblins, Troll, Keeper).
2. **`dungeons.rs`**:
   - `iadungeon`, `iadungeonstats`, `iadungeontime`, `iadungeonstatstotal`.
3. **`tavern.rs`**:
   - Quests parsing (`q1`, `q2`, `q3`), toilet state, gambler/dice game, beer count.
4. **`arena.rs`**:
   - Arena opponent lookups, fights, and cooldowns.
5. **`guild.rs`**:
   - Guild members, portal status, raid progress, guild chat/log parsing.
6. **`character.rs` & `src/gamestate/mod.rs`**:
   - Core character save (`char save`), attributes (`char attr basis`, `char attr adds`, `char attr tb`), mount durations, and equipment parsing.
7. **`items.rs`**:
   - `Item::parse` already exists for `ResponseVals`, but several places in `src/gamestate/mod.rs` still invoke deprecated `Item::parse_slice`.

---

## 5. Verification Commands

Run these to verify clean compilation, formatting, and tests:
```bash
# Check formatting
cargo +nightly fmt --check

# Run unit tests and doctests
cargo test

# Run clippy on migrated code (ignoring expected deprecated warnings from unmigrated modules)
cargo clippy --tests -- -A deprecated
```
