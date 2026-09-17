# Phase Plan: Move Generation

## Objective

Deliver the move generator for the 4-line board search.

The move generator must:

- Report every reachable landed position of one piece on one 4-row field.
- Be complete by construction, but verified against a second, simpler
  breadth-first search.
- Run as SIMD lane-batched work: `N` independent boards per call.
- Fix two latent bugs found in the board phase.

## Current Behavior

`jade_core` has a 40-bit field (`WIDTH = 10`, `LINES = 4`) and `Batch<N>`
primitives. It has no move generation.

Two bugs sit in the movegen critical path today:

- `Move::canonicalize` (placement.rs) adds the canonical offset.
  Rue and jade's own `place_mask` (data.rs) subtract it.
  The two must agree. `canonicalize` flips to `saturating_sub`.
- `Move.y` has 2 bits (rows 0 to 3).
  The movegen working field has 6 rows (0 to 5).
  Y widens to 3 bits.

## Proposed Behavior

- New workspace crate `crates/jade_nav`.
  It owns a 6-row (60-bit) SIMD plane per board and the whole movegen.
  The level-batched search lands here later.
- The move generator finds all reachable landed positions by structure:
  1. Direct drops: every origin that lands on the current surface.
  2. Reachability fixed point: horizontal tucks, vertical drops, and
     SRS kicks (including 180-degree) over a per-rotation origin plane.
- Contract boundary:
  - Intermediate placements may occupy working rows 4 and 5.
    These are the spawn and buffer rows.
  - A locked placement may never have a cell in rows 4 or 5.
    The movegen reports a lock only when all four cells sit in rows 0 to 3.
  - The caller does not need a post-filter.
- The fast path emits, per rotation group, the plane of legal locked
  origin positions, plus a lock count.
- The reference BFS (oracle) produces the same output from a plain
  wavefront search. It exists to check the fast path.

## Design

### Working field

- One working lane holds one independent 6-row plane per board.
- Rows 0 to 3 are the storable field. Rows 4 and 5 are buffer.
- Bit index is `y * 10 + x`, row 0 at the bottom, column 0 at the left.
  This matches the jade field layout.
- Spawn: `spawn_x = 4`, `spawn_y = 4`.
  The spawn origin sits one row above the visible top.
  The tallest pieces occupy rows 4 and 5 at spawn.
- `force` stays a caller parameter. The search passes 0.

### Components

New crate `crates/jade_nav`. It depends on `jade_core`.

- `header.rs`
  - `LINES = 6`, `BITS = 60`, `MASK = (1 << 60) - 1`.
  - 60-bit column masks.
  - `dx_mask6(dx)`: horizontal shift guard over the 60-bit field.
  - `LANDED_OK: [[u64; 4]; 7]`: allowed origin rows for the lock rule.
- `plane.rs`
  - `Plane<N>(Simd<u64, N>)`: a 60-bit working lane per board.
  - Operators: `&`, `|`, `^`, `!` masked to 60 bits.
  - Methods: `empty`, `splat`, `from_array`, `to_array`, `any`,
    `popcount`, `shifted(dx, dy)`, `get`, `set`.
  - Shifts are pure per-lane operations.
    jade lanes never interact, so no rue-style cross-band shifting.
- `movegen/mod.rs`
- `movegen/op.rs`
  - Ports of the rue building blocks, re-expressed for 60-bit planes:
    - `usable_cell`, `usable_rot`, `usable_map` from `CELLS`.
    - `landable_map`: `usable[r] & !usable[r].shifted(0, 1)`.
    - `vertical_drop` with infinite soft drop fixed on.
    - `sonic_drop`.
    - `vertical_ceiling` (ceiling up to 6, safe in 64 bits).
    - `horizontal_tuck` via SIMD shifts.
    - `env_probe` and `EnvelopeTable` over `KICKS_TJLSZ` and `KICKS_I`.
  - Scalar helpers for the oracle:
    - `check`.
    - `check_fast`.
    - `apply_rotation` (cw, ccw, 180).
  - `unroll!`: compile-time rotation literals for the kicked loop.
- `movegen/buffer.rs`
  - `Moves<N> { piece, per_rot: [Plane<N>; 4] }`.
  - Locked-origin planes indexed by `Rotation`.
  - No spin buckets. jade has no spins.
  - `insert`, `contains`, `iter`.
- `movegen/oracle.rs`
  - Scalar BFS reference.
  - Fixed-capacity ring queue.
  - Expansion: lateral step, soft drop to rest, cw, ccw, and 180
    rotation via `apply_rotation`.
  - Spawn `(4, 4)`, clamped by the same rise-scan as the fast path.
  - A lock is recorded only when `mv.mask().is_some()`.
    `place_mask` enforces "every cell in rows 0 to 3" exactly.
- `movegen/fast.rs`
  - Batched fast generator. The next phase after oracle parity.

### The lock-rule filter

- For each `(piece, canonical rotation)`, the canonical frame has a
  topmost cell at `max_y` rows above the canonical origin.
  A locked origin is valid only for rows `0 ..= 3 - max_y`.
- `LANDED_OK` is a const table of allowed origin rows in the 60-bit
  plane, derived from `CELLS`.
- The fast path ANDs `cands` with this mask before the fixed point.
  Then `missing`, `remaining`, and counts are consistent.
- The oracle records a lock only when `move.mask()` is `Some`.
  This is the same rule, via the shared `place_mask`.

### Rules adopted

- `allow_180 = true`.
  Kicks come from the classic SRS tables at
  `https://kick.rqft.workers.dev/jstris/raw`.
  Those tables are byte-identical to the `KICKS_TJLSZ` and `KICKS_I`
  already in jade.
  The fast path runs kick direction `D = 2` using `kt[R][(R + 2) & 3]`.
  The oracle rotates `cw().cw()`.
  No new kick data. No `KICKS_O`. O never rotates.
- No `Rule` struct.
  `das` and `inf_sdf` only restructure the transition graph, not the
  landed set. Both are fixed to full-reach semantics.
- The 180 kick reach (x and y within +/- 1) is a subset of the cw/ccw
  envelope (+/- 2, +/- 2) and well inside the +/- 3 `env_probe`.
  The envelope needs no extension for 180 rotations.
  Verified during implementation.

### Interfaces

- `fast::movegen<N, P>(planes: Plane<N>, y: i32, force: i32) -> Moves<N>`
- `fast::count_locks<N, P>(plane: Plane<N>, y: i32, force: i32) -> u64`
- `oracle::movegen<P>(board: u64, y: i32, force: i32) -> Vec<Move>`
- The next phase consumes `Moves<N>` planes and expands each origin to
  its 40-bit landed board via `place_mask`.

## Alternatives

- Vendor rue's generator. Rejected.
  The project owns its movegen per `docs/design.md`.
  jade lanes are independent boards; rue lanes are bands of one board.
  The code would not port cheaply.
- BFS only. Rejected.
  Variable-length per-board work does not batch across lanes.
  This is the core motion of `docs/design.md`.
- 4-row working field. Rejected.
  SRS kicks can lift a piece two rows. With no headroom, top-edge
  behavior would diverge from the game for near-top stacks.
- Emit locked boards, not origins. Rejected.
  Per-origin mask expansion is not SIMD on the plane type.
  Emitting origins keeps the hot path vectorized and lets the search
  decide state effects.

## Risks

- Fast/oracle divergence under kicks.
  rue documents divergences for spin tracking.
  This project runs 180 rotations on, beyond rue's own parity scope.
  Mitigation: the lock-rule filter first, then the parity suite.
  Then a failing case is a fixed-point bug, not a rule question.
- The lock-rule filter and the spawn rise-scan must not interact.
  The scan runs before the fixed point. No interaction is expected.
- The `Move` layout change touches existing board-phase tests.
  They are updated in the same change.

## Compatibility

- `jade_core` surface changes:
  - `Move.y` becomes 3 bits.
  - `ROT_SHIFT` moves accordingly.
  - `canonicalize` subtracts instead of adding.
  - `Move` stays 16 bits.
- `jade_core` keeps no movegen. `jade_nav` is the new home.
  The field board type is untouched.
- No rule configuration is exposed.
  Behavior is fixed: spins off, 180 on, buffer spawn.
- Search contract: every locked origin, expanded via `place_mask`,
  fits within rows 0 to 3 by construction.

## Testing

- Port rue's numeric parity harness to jade (fast vs oracle):
  - rue's board shapes: full-row gap, flat stack, staircase, bump.
  - Randomized boards over 6 rows.
  - All 7 pieces.
  - Both set equality and counts.
- Run parity with 180 rotations on.
  This is the differentiating feature.
- Run parity with boards containing cells in rows 4 and 5.
- Lane independence:
  A `Plane<N>` of N random distinct boards must equal N scalar runs.
- `LANDED_OK` versus `place_mask` on every
  `(piece, rotation, origin)` in the 6-row space:
  `LANDED_OK match == place_mask(...).is_some()`.
- `Move` tests updated for the 3-bit Y and the fixed `canonicalize`.

## Implementation Steps

1. `jade_core`: widen `Move.y` to 3 bits.
   Flip `canonicalize` to `saturating_sub`.
   Update tests.
2. `crates/jade_nav` scaffold:
   `Cargo.toml`, `plane.rs`, `header.rs` with `LANDED_OK`.
3. `movegen/op.rs` scalar and vector building blocks. `unroll!`.
4. `movegen/buffer.rs` `Moves<N>`.
5. `movegen/oracle.rs` BFS reference.
6. `movegen/fast.rs` batched generator with emission and counts.
7. Parity, lane-independence, and unit tests. Clippy clean.

Steps 1 to 5 are the current scope. Step 6 and 7 follow.