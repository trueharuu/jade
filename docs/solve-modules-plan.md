# `jade_solve` module restructuring plan

## Objective

Split `jade_solve` into modules that each hold one concern, and remove the
duplicated decision rules that currently exist in both `solve.rs` and
`percent.rs`.

The crate must keep its current answers. This is a structural change only.

## Current Behavior

`jade_solve` has three modules: `parse`, `solve`, and `percent`.

`parse.rs` (107 lines) holds two unrelated things:

- the fumen codec: `parse_fumen` and `encode_fumen`
- `Saves`, a piece-set bitset with its own `FromStr` and `Display`

`solve.rs` (406 lines) holds the DFS solver and four items that `percent.rs`
also needs:

- `moves`, the `Piece` to const-generic generator dispatch
- `PC_4` and `PC_2`, the goal boards
- `piece_from_index`, the `u8` to `Piece` conversion
- `save_ok`, the leftover-piece check

`percent.rs` imports all four from `solve`. It also writes out its own copies
of the rules that decide an answer:

| Rule | `solve.rs` | `percent.rs` |
|---|---|---|
| prune test | `Node::pruned` (line 353) | `board_pruned` (line 187) |
| play-field bound | `next.0 >> 40 != 0` (line 262) | `n.0 >> 40 != 0` (line 250) |
| final-piece test | inline in `place` (line 234) | `completes` (line 285) |
| count bound `k` | inline twice (lines 149, 242) | inline (lines 40, 47) |

The two final-piece tests are the same function. `solve.rs` lines 232 to 235
are `completes(board, piece, PC_4)`.

`jade_core::header` also declares `PC_4` and `PC_2` as `u64`. Those are dead.
Nothing outside `header.rs` reads them.

## Proposed Behavior

Unchanged answers. New module layout:

```
jade_solve
  fumen.rs   the fumen codec
  saves.rs   Saves
  rules.rs   the rules both solvers use
  moves.rs   the Piece to generator dispatch
  solve.rs   the DFS solver
  percent.rs the queue-set solver
```

`rules.rs` becomes the single home for the shared rules. `solve.rs` and
`percent.rs` both call it. No rule stays written twice.

## Design

### New module `rules`

Holds items that decide an answer and that more than one solver needs.

```
pub const PC_4: Board
pub const PC_2: Board

pub fn pruned(board: Board) -> bool
pub fn outside_playfield(board: Board) -> bool
pub fn pieces_to_goal(cells: u32, two_l: bool) -> usize
pub fn completes_goal(board: Board, piece: Piece, goal: Board) -> bool
pub fn piece_from_index(i: u8) -> Piece
```

`piece_from_index` moves here from `solve.rs`. It is a `Piece` conversion, not
a solver concern, but it is only used by the two solvers, so it does not belong
in `jade_core`.

`pruned` moves here from `percent.rs::board_pruned`. `Node::pruned` in
`solve.rs` calls it. `jade_perft::perft::subtree` keeps its own inline call
until that crate is restructured; it is a third copy, and it is noted here as
known remaining duplication.

`completes_goal` is `percent.rs::completes` unchanged. `solve.rs::place` calls
it in the `cells == 40` branch, replacing the inline test.

`pieces_to_goal` is the count bound. It is the expression that `solve.rs`
writes twice:

```
if two_l && cells < 20 { (20 - cells) / 4 } else { (40 - cells) / 4 }
```

### New module `moves`

Holds the `Piece` dispatch:

```
pub fn moves(board: Board, piece: Piece) -> Moves
```

It moves from `solve.rs` unchanged. It is the only place in the workspace that
expands a `Piece` match into const-generic calls for the fast generator.
`jade_perft::model` has two more copies of this dispatch, one for each
generator; those are noted as known remaining duplication.

### Module `saves`

`Saves` moves from `parse.rs`. It also gains `save_ok`, which currently lives
in `solve.rs` and is used by both solvers. `save_ok` is a rule about a
leftover piece, so it belongs with the type that describes the constraint.

```
pub struct Saves(u8)
pub fn save_ok(saves: Saves, hold: Option<Piece>, active: Option<Piece>) -> bool
```

### Module `fumen`

`parse_fumen` and `encode_fumen` move from `parse.rs`. No other change.

### Deletion in `jade_core`

`header::PC_4` and `header::PC_2` (the `u64` pair) are deleted. They are dead
and they shadow the live `Board` pair under the same names.

### What does not change

The hold schedule stays in two places. `Solver::search` in `solve.rs` and
`walk` in `percent.rs` both encode the three hold options. They are not the
same function and must not be merged: `search` carries a board and a failure
cache, `walk` carries an order code and enumerates without a board. They stay
separate. A comment on each will point at the other so a change to one is
checked against the other.

The pruning call sites stay where they are, because they are not the same
check. `solve.rs` prunes a child before it is entered and before the cache
test. `percent.rs` prunes a child during generation. Merging them would change
which nodes reach the cache. Only the predicate is shared.

## Alternatives

### Keep `parse.rs` and add modules around it

Rejected. `Saves` is in the solver's public signature. It does not belong in a
module named after text parsing.

### Move `Saves` into `jade_core`

Rejected. `Saves` is a solver parameter, not a piece property. Nothing else in
the workspace uses it.

### Move `piece_from_index` into `jade_core::piece`

Rejected for now. `jade_core::piece` already has `Piece::from_u8` returning an
`Option`. `piece_from_index` is the infallible form, valid only for `i < 7`.
Adding a second conversion to the same enum invites confusion. It stays in
`rules` until `jade_core` is restructured.

### Unify the two final-piece tests by changing their order

Rejected. `solve.rs` tests the goal on the child board before the prune test,
because `PC_4` fails `pruned`. `percent.rs` tests it on the parent during the
last expansion step. Swapping either order changes results.

### Move the goal rules into a new crate

Rejected. Both solvers are in `jade_solve`. A crate boundary here costs
`Cargo.toml` entries and re-exports and buys nothing.

## Risks

### The count bound has two guards and they differ

`solve.rs` guards the two-line branch with `cells < 20` and otherwise uses 40.
`percent.rs` guards the two goals separately with `cells <= 40` and
`cells <= 20`. At `cells == 20` the two disagree on which goal is in reach.

Mitigation: `pieces_to_goal` implements the `solve.rs` form only. The
`percent.rs` guards stay at their call sites. The guards are not the same
check and must not be folded in. This is called out because it is the most
likely place for an accidental behavior change.

### `completes_goal` for `PC_2` re-runs `clearshift`

It is `percent.rs::completes` unchanged. The `PC_4` branch compares masks and
does not clear. The difference is intentional and is not being touched.

### Lossy cache interaction

`Solver::seen` is a lossy cache. Sharing `completes_goal` does not change how
many nodes are cached, because the final-piece test happens before the child is
built in both solvers. Still, the solver's answers are the ground truth for
this change and must be checked against the fixtures in `tests/`.

## Compatibility

- **Public API.** `jade_solve::solve::PC_4` and `piece_from_index` move to
  `jade_solve::rules`. `piece_from_index` was `pub(crate)`, so it is not public
  API. `PC_4` and `PC_2` are public and must be re-exported from `solve` or
  their paths updated. `jade_perft` does not use them; `jade_cli` does not use
  them. Only `percent.rs` imports them today.
- **Public API.** `jade_solve::parse_fumen` stays exported from the crate
  root, so `jade_cli` is unaffected.
- **Public API.** `Saves` moves from `jade_solve::parse::Saves` to
  `jade_solve::saves::Saves`. `jade_cli` imports
  `jade_solve::parse::Saves` and must be updated.
- **Public API.** `solve::moves` and `solve::save_ok` are `pub` today and are
  used by `percent.rs`. `moves` is used by nothing outside the crate. Both get
  a re-export from `solve` or their paths updated.
- **Data.** None. No fumen, fixture, or recorded count changes.
- **Configuration.** None.
- **Behavior.** Must be identical. Verified by `tests/check-solve.sh` against
  all three fixtures.
- **Other crates.** `jade_cli` needs one import updated. `jade_perft` and
  `jade_core` need no source change, apart from the `header` deletion.

## Testing

1. `cargo build --workspace` succeeds with no new warnings.
2. `cargo clippy --workspace` reports no new warnings in `jade_solve`.
   Note that `sandbox` still fails this gate; that is a separate item.
3. `cargo test -p jade_solve` passes.
4. `tests/check-solve.sh tests/jaws`, `tests/jigsaw`, and `tests/legs` all
   report `OK`. These are the ground truth. The recorded counts are 5040/5040,
   587/840, and 5016/5040.
5. Unit tests added in `rules.rs` for `pieces_to_goal` at the boundary values
   `cells` 0, 4, 16, 20, 24, 36, 40 with `two_l` both ways, so the `cells == 20`
   disagreement above is pinned down by a test rather than by a comment.
6. A unit test in `fumen.rs` for a decode and re-encode round trip, since the
   module moved.

## Implementation Steps

1. Add `rules.rs` with `PC_4`, `PC_2`, `pruned`, `outside_playfield`,
   `pieces_to_goal`, `completes_goal`, `piece_from_index`, and the boundary
   unit test.
2. Add `moves.rs` with the dispatch, moved from `solve.rs`.
3. Add `saves.rs` with `Saves` and `save_ok`, moved from `parse.rs` and
   `solve.rs`.
4. Rename `parse.rs` to `fumen.rs`, keeping only the codec.
5. Update `solve.rs`: delete the moved items, import from the new modules, keep
   a `pub use` for `PC_4`, `PC_2`, `moves`, and `save_ok` so existing paths
   keep working. Replace the inline `cells == 40` test with `completes_goal`.
   Replace both inline count bounds with `pieces_to_goal`. Make `Node::pruned`
   call `rules::pruned`. Add the cross-reference comment on the hold schedule.
6. Update `percent.rs`: import from `rules` and `moves` instead of `solve`.
   Delete `board_pruned` and `completes`. Replace the two inline count bounds
   with `pieces_to_goal`, keeping the existing `cells <= 40` and `cells <= 20`
   guards. Add the cross-reference comment on `walk`.
7. Update `lib.rs`: declare modules alphabetically, re-export `parse_fumen`,
   re-export `Saves`.
8. Update `jade_cli`'s import of `Saves`.
9. Delete `header::PC_4` and `header::PC_2`.
10. Run the checks in the Testing section.
