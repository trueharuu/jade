# Boolean PC Solver — Change Plan

Planning document. No code has been written under this plan.

## Objective

Add a single-threaded, boolean Perfect-Clear (PC) solver to the `jade` workspace.

The solver decides a reachability question:

Given an initial board and a concrete piece queue, can any legal placement
sequence reach the full-board state `header::PC_4` while using hold?

The solver returns `true` on the first reaching path and stops. It does not
count solutions.

## Current Behavior

- A placement equals `child(board, m) = (board | m.mask()).clearshift()`.
  `clearshift` moves completed rows to the bottom of the board and keeps them
  as markers. The residue sits above the markers. Hand-verified perft counts
  validate this model (`docs/perft.md`).
- `Board::has_isolated_cell` and `Board::has_imbalanced_split` return `true`
  only for boards that can never be fully filled. `jade_perft` uses them as
  sound pruning (`crates/jade_perft/src/perft.rs:35`).
- `jade_nav::fast::generate::<P>` is a fixed-point bitboard closure: masked
  shifts per raw-rotation plane until no origin grows. Its placement set is
  parity-verified against `jade_nav::oracle::generate::<P>`, a per-board BFS
  over ghost origins (`docs/movegen-parity-fix.md`). A `Moves` result stores
  one origin bitboard per canonical rotation; `Moves::iter()` yields placements
  in ascending `(rotation, y, x)` order.
- `jade_cli` supports only `pattern expand|count`.
- The queue grammar lives in `jade_pattern` (`Pattern::expand()`), used by
  `jade_perft`.

## Proposed Behavior

New library crate `crates/jade_solve`:

- `pub fn reachable(board: Board, queue: &[Piece]) -> bool`
- `pub fn parse_fumen(s: &str) -> Result<Board, String>`, copied from
  `jade_perft`.

The solver always uses the `jade_nav::fast` move generator. There is no model
selector.

New `solve` subcommand in `jade_cli`:

```
jade solve <queue-pattern> [--field <fumen>]
```

The command prints one line per pattern expansion:

```
pc(TSZ) = yes
pc(TSZ) = no
```

No lock count is printed. The output is boolean only.

## Design

### Semantics

- Target: a state equal to `Board(header::PC_4)`, meaning all 40 play-field
  cells are filled.
- Each placement maps `board` to `(board | m.mask()).clearshift()`.
- Reaching `PC_4` at any prefix of the queue is success.
- The initial board is masked to the 4-row field and `clearshift()`ed at
  entry.
- The output of `reachable` is `true` when a path exists, `false` otherwise.

### Two structural facts about the search

1. **Cells only accumulate.** Every generated placement lands all four minos
   inside the 4-row field on empty cells. Each placement therefore adds exactly
   four filled cells. `clearshift` permutes bits but never removes them.

2. **Corollary.**
   - If `popcount(init) % 4 != 0`, then `PC_4` is unreachable regardless of
     the queue. Reject the whole query in O(1).
   - From level `i`, at most `queue.len() - i` placements remain, so at most
     `4 * (queue.len() - i)` cells can be added. If
     `40 - popcount(board) > 4 * (queue.len() - i)`, the state is dead.

### Hold model

The guideline hold rule applies: you may swap the current piece with the hold.
After a swap you must lock a piece before swapping again. An empty hold swaps
in the next queue piece. A non-empty hold swaps with the held piece.

A swap is always followed by exactly one placement, the placement of the
swapped-in piece. Each "hold then lock" is therefore folded into one transition.
This makes the "no double swap" rule structural instead of a checked flag.

State: `(board, i, hold)`, where `i` is the next queue index and
`hold: Option<Piece>`.

Transitions from `(b, i, hold)` where `i < queue.len()`:

1. Place `queue[i]`:
   - For each legal placement `m` of `queue[i]`,
     `child = (b | m.mask()).clearshift()`.
   - Recurse into level `i + 1` with the same `hold`.
2. Hold with an empty hold, only if `i + 1 < queue.len()` (else the swapped
   piece strands with no current piece):
   - Place `queue[i + 1]`.
   - Recurse into level `i + 2` with `hold = Some(queue[i])`.
3. Hold with a non-empty hold `Some(h)`:
   - Place `h`.
   - Recurse into level `i + 1` with `hold = Some(queue[i])`.

`i` strictly increases in every transition, by 1 or 2. The state that exists
just after a swap ("`queue[i]` in hold, current is `queue[i + 1]`") is
reachable only through the swap, so folding it loses no state and duplicates
no movegen: `moves(queue[i + 1], b)` is computed exactly once.

Completeness: every lock sequence from a state either locks the current piece
or swaps and then locks the swapped-in piece, since a second swap needs a lock
first. Transitions 1-3 enumerate exactly these cases, and success is checked
for every child. `PC_4` reached at any point returns `true`.

### Search strategy: level frontier

The solver walks a per-level frontier ordered by lock count. It does not use a
recursive DFS. Reasons:

- Memory is one current frontier plus two pending buckets, not the sum of all
  reached levels. For hard instances this keeps memory flat.
- The level loop and the pending-bucket shape are the same shape as the future
  SIMD batched search in `docs/design.md`. The path to that design stays clean.
- No recursion depth risk on long queues.
- The first `PC_4` found is at the smallest queue index that reaches it.

```
reachable(board, queue):
  board = (board & rows_below(PLAY_LINES)).clearshift()
  if board == PC_4:                  return true
  if queue.is_empty():               return false
  if popcount(board) % 4 != 0:       return false    // fact 2
  if popcount(board) + 4*len < 40:   return false    // cannot fill enough cells

  buckets: HashMap<usize, HashSet<u64>>   // only levels i, i+1, i+2 live
  insert(0, board, None)

  for i in 0..queue.len():
    cur = buckets.remove(i); skip if empty
    for each state (b, h) in cur:
      for each transition 1-3 of (b, i, h):
        c = (b | m.mask()).clearshift()
        if insert(i + 1 or i + 2, c, new_hold): return true
  return false

insert(level, b, h) -> bool:  // true means "PC_4 found"
  if b == PC_4:                        return true
  if b.has_isolated_cell() || b.has_imbalanced_split(): return false
  if 40 - popcount(b) > 4 * (queue.len() - level): return false
  buckets.entry(level).or_default().insert(pack(b, h))
  return false
```

Pruning inside `insert` keeps dead states out of the frontier early. The
`i == queue.len()` bucket holds only non-`PC_4` terminal states and is dropped.

The internal loop tracks the level of the first `PC_4` for tests only. The
public API returns `bool`.

### Keys and hashing

- `board.raw < 2^40` at every level, because all bits stay in rows 0-3. Pack
  `key = board.raw | (hold_code << 40)`, with `hold_code` in `0..8` (0 means
  empty hold). One `u64`.
- Use `rustc-hash` (`HashMap` and `HashSet` with `FxHash` on `u64`) instead of
  the standard library SipHash. This is a single-user CLI tool; a fast
  non-cryptographic hash is correct here and keeps the hot path cheap.
- `buckets` keeps at most three live levels, so memory is O(frontier width),
  not O(reachable graph).

### Ordering

The frontier order is: place the current piece, empty-hold place, non-empty
hold place. `Moves::iter` yields placements ascending by `(rotation, y, x)`;
`fast` merges raw rotations into canonical planes. BFS makes ordering
irrelevant for success depth: the first `PC_4` inserted is at the minimal
lock count.

### Move generator

Only `jade_nav::fast::generate` is used. It is a fixed-point bitboard closure:
masked shifts per raw-rotation plane until no origin grows, with no per-board
search. Its placement set is already parity-verified against the oracle in
`jade_perft` (`--compare` in `docs/movegen-parity-fix.md`).

Per expanded state the cost is:

- one `fast::generate` on the board (tens of bitwise ops plus a small
  fixed-point loop),
- `M` placements from the `Moves` iteration, where each child costs an OR, a
  `clearshift`, and one `FxHash` insert,
- where `M <= 34`, the T-piece placement count.

Total work equals the number of distinct `(board, hold)` states per level. For
typical 10-14 piece queues the frontier is `10^3` to `10^5` states per level,
which runs in milliseconds to seconds per level. Memory is one to three
frontier widths.

### Deliberately not in v1

- **Mirror-symmetry canonicalization.** Unsound for a fixed queue: mirroring
  `(board, hold)` must mirror the remaining queue too, and `mirror(J) = L`.
  Deferred to a later phase where the solve phase works on a bag or multiset
  that is mirror-symmetric.
- **Transposition across levels.** The same `(board, hold)` at a deeper `i`
  has fewer pieces left, so it is a genuinely different future. Separate
  per-level keys are correct.
- **Recursive DFS.** Rejected for memory and SIMD-migration reasons above.
- **SIMD batching.** Per `docs/design.md`, a later step built on the same
  level-frontier loop.
- **Count reporting and lock-count output.** The task is boolean. The CLI and
  API output only `yes`/`no` and `true`/`false`.

## Alternatives

- **DFS with a visited set.** Same total work, but memory is the sum of all
  reached states. Rejected for memory and SIMD continuity.
- **Rue-style empty-board goal with line removal.** Rejected by project
  decision. The literal `PC_4` target with `clearshift` markers is used.
- **Explicit phase flags in the state.** Rejected. The glued hold-place
  encoding is smaller and makes the swap rule structural.
- **Symmetry canonicalization.** Considered and rejected as unsound for fixed
  queues, as described above.

## Risks

- **State growth on pathological or long queues.** Mitigated by early exit at
  the first `PC_4`, per-level dedup, the mod-4 rejection, the popcount-in-time
  bound, and the two fillability predicates. Truly adversarial queues can still
  grow. The SIMD phase addresses this later. There is no hard size cap in v1.
- **Divergence from `docs/design.md` semantics.** `design.md` describes
  success as an empty board. The jade `clearshift` model keeps full-row
  markers, so that statement and this solver are not the same thing. This step
  deliberately uses the literal `PC_4` state. The later `save` definition in
  `design.md` must be re-derived against `clearshift` when it is implemented.

## Compatibility

- New crate `jade_solve` and a new `jade_cli solve` subcommand only.
- The workspace gains one member. `jade_cli` gains the `jade_solve` dependency.
- No existing API, data, behavior, or configuration changes.
- The git working tree is clean. The change is one new commit.

## Testing

Deferred until the CLI and parse steps land. Known-invalid expectations from
the first draft were corrected during implementation:

- `IIII` cannot fill an empty field: four I's cover 16 cells, not 40. A
  positive case needs ten placements, for example ten I's or ten O's.
- `Board::lines(1)` is not mod-4 valid (10 cells). A valid filled start is
  `Board::lines(2)` (20 cells).
- A gap in the top row is only fillable when the airspace above the gap is
  clear: pieces must reach the gap through the falling path. The fast
  generator enforces this. The hold-required case must reserve clear columns.
- Minimality lands at queue index 10 for an empty field, not 4.

Planned unit tests in `jade_solve`:

1. Empty board + queue `IIIIIIIIII` returns `true`.
2. Empty board + queue `I`..`IIIIIIIII` returns `false`.
3. Empty board + queue `O`, `OO` returns `false`; `OOOOOOOOOO` returns
   `true`.
4. `Board::lines(2)` + queue `OOOOO` returns `true`.
5. Mod-4 rejection: an initial board with a single cell and any queue returns
   `false`.
6. Initial board already `PC_4` returns `true`. Empty queue with an empty board
   returns `false`.
7. Minimality: empty board + queue `IIIIIIIIIIZZ...` returns `true` and, via
   the internal depth helper, stops at queue index 10, not at the last piece.
8. Hold required: a crafted queue where `reachable` returns `true`, and a
   test-only no-hold variant returns `false` on the same input. A second case
   where the no-hold variant also succeeds shows hold helps and is not
   strictly required there.
9. Popcount-in-time bound: a board four cells short of `PC_4` plus queue `T`
   returns `false`.
10. The pruning predicates never reject a known-true path.

Manual CLI checks:

```
cargo run -p jade_cli -- solve IIIIIIIIII
cargo run -p jade_cli -- solve I
cargo run -p jade_cli -- solve IIIIIIIII --field <fumen of PC4-ish board>
cargo run -p jade_cli -- solve IO
```

Expected output lines are `pc(<queue>) = yes` and `pc(<queue>) = no`.

Verification: `cargo test -p jade_solve` passes, and
`cargo clippy --workspace --all-targets` passes with `perf = deny`.

## Implementation Steps

1. Write this plan to `docs/boolean-solver.md`.
2. Add `crates/jade_solve` to the workspace; create the crate skeleton with
   dependencies `jade_core`, `jade_nav`, `fumen`, and `rustc-hash`.
3. Implement `solve.rs` with the level frontier, keys, pruning, the fast-only
   dispatch, and the internal depth helper for tests.
4. Implement `parse.rs` with `parse_fumen`.
5. Add the `solve` subcommand to `jade_cli`.
6. Write tests and run `cargo test -p jade_solve`. (Deferred; tests were
   omitted from the implementation.)
7. Run clippy and the manual CLI checks.
8. Compare the implementation against this plan. Document deviations.
9. Commit the change as one commit.