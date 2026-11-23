# T-Spin Detection in Fast Movegen

## Objective

Add T-spin detection to `crates/jade_nav/src/fast.rs` so that its `Moves`
result matches `crates/jade_nav/src/oracle.rs` exactly, including `mini` and
`full` strata, while keeping the bitboard implementation.

## Current Behavior

- `oracle::generate` tracks `Spin` per ghost. A T placement is `Full` when its
  rotation arrival satisfies the 3-corner test and either facing corners are
  filled or the last kick index is at least 4. It is `Mini` when the 3-corner
  test holds but facing corners are empty and the kick index is below 4.
  Other pieces are always `None`.
- `oracle` now loses spin if the hard drop moves the piece vertically.
- `fast::generate` only filled `moves.none`. It did not populate `mini` or
  `full`, and did not distinguish spin-carrying placements.

## Proposed Behavior

- `fast::generate` populates `moves.none`, `moves.mini`, and `moves.full`.
- Only `Piece::T` can produce `mini` or `full`.
- A spin is emitted only for a rotation arrival that is already on support
  (landable). A floating spin arrival hard-drops to support and counts as
  `None`.
- Shifts (lateral/soft-drop) reset spin to `None`.
- Spin duals are emitted: the same landed cell may appear in more than one
  stratum.

## Design

Affected file: `crates/jade_nav/src/fast.rs`.

State added per rotation:
- `none_spin`, `mini_spin`, `full_spin`: bitboards of reached cells carrying
  each spin.

Control flow is unchanged (single-pass BFS with `search`/`unsearched`). Spin
state is tracked alongside reachability:

1. Spawn cell is recorded in `none_spin[0]`.
2. `shifts` marks every shift target as `None`, including already-reached cells,
   because movement resets spin.
3. `kick_seq` labels each first-valid rotation arrival:
   - T arrivals satisfying the 3-corner test are `Full` when `n >= 4` or the
     facing corners are filled; otherwise `Mini`.
   - T arrivals failing the 3-corner test are `None`.
   - Non-T arrivals are `None`.
4. Final strata:
   - `moves.none[r]` = landable reached cells carrying `None`, plus
     `project_down` of floating reached cells (spin lost on vertical drop).
   - `moves.mini[r]` = landable reached cells carrying `Mini`.
   - `moves.full[r]` = landable reached cells carrying `Full`.

`project_down` maps each floating cell to the first landable cell at or below
it in the same column, using a per-column 6-row scan.

## Alternatives

- Full spin-state BFS with separate pending queues per spin. Rejected: larger
  state and slower, while the position-only BFS already captures all reachable
  cells.
- Projecting every spin source to support. Rejected after the oracle change:
  vertical drops lose spin, so only landable arrivals keep spin.

## Risks

- Out-of-bounds `Move` coordinates in other pieces can panic in debug
  `Moves::insert`; unrelated to this change.
- `project_down` is O(columns * rows) per rotation, acceptable for T only.

## Compatibility

- `Moves` already has `none`, `mini`, and `full` fields.
- Non-T behavior unchanged.
- Perft counts for T may change on boards with spins.

## Testing

- Randomized parity against `oracle::generate` for T across many boards.
- Existing perft single-piece counts remain valid for the empty board.
EOF
echo done