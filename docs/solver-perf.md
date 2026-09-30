# Solver Performance and Debugging Removal — Change Plan

Planning document. Phase 1 is done. See the Deviations section.

**The solver is single-threaded.** `crates/jade_solve` has no thread pool, no
atomics, and no `rayon` dependency. This is a decision of the owner, not a
deferred item.

## Objective

Two goals, in this order.

1. Remove all debugging from `crates/jade_solve`. The crate must contain only
   solving code.
2. Reduce the wall time of the solver on 10-piece queues such as
   `TZSIJLOIOJ`.

Goal 1 runs first. It removes 12 clock reads and about 20 counter writes from
the hot path, and it gives the goal 2 work a clean base to measure.

The boolean answers from `reachable` and `reachable_2l` must not change. The
`jade_nav` move generator must keep its results.

## Evidence

This section separates facts from inferences, and lists the unknowns. Read it
before acting on the Design section.

### One measurement was taken

The prebuilt binary `target/debug/jade_cli` was run once on the input
`TZSIJLOIOJ`. The binary is a debug build from 2026-09-24 14:28. It is older
than the current `solve.rs`, which was last changed at 15:24 the same day, so
it does not match the source.

```
JADE_SOLVE_STATS=1 ./target/debug/jade_cli solve TZSIJLOIOJ   ->  true
real 1m6.3s   user 1m6.2s
```

The `JADE_SOLVE_STATS` line did not print, which is consistent with the binary
predating the counters. No release build was measured. The owner reports about
15 seconds from a release build. A ratio of 4.4 between the two is the normal
debug-to-release factor for bit-twiddling code, so the two numbers are
consistent. No other measurement has been taken.

### Facts about the current code

1. An empty play field has 40 cells. Every placement adds exactly 4 cells. So a
   solution for an empty field needs exactly 10 placements.
2. A state with a non-empty hold can only be reached by a swap. A swap with an
   empty box advances the queue index by 2 and places 1 piece. A swap with a
   non-empty box advances the index by 1 and places 1 piece. Only transitions 2
   and 3 set a non-empty hold, and transitions 1 and 3 keep whatever hold the
   state already has. So once the hold is non-empty it never becomes empty,
   except at the final release. Therefore a path uses at most one empty-box
   swap, and 10 placements need at least 10 indices. On a 10-piece queue the
   goal is reachable at level 10 only, and levels 0 to 8 run to exhaustion. No
   early exit applies.
3. `jade_perft` records 3,390,732 tree nodes for 7 pieces from an empty board,
   before any deduplication. The 8-piece and 9-piece frontiers are wider. The
   solver explores those frontiers in full.
4. `merge` at `crates/jade_solve/src/solve.rs:702` calls `set.reserve` once per
   worker. `walk_par` calls `merge` once per worker at `solve.rs:371`.
   `hashbrown::HashSet::reserve(additional)` resizes, and a resize reinserts
   every element, when `items + additional` passes the table capacity. The set
   is at its capacity limit whenever it is merged into, so each worker's
   `reserve` resizes and rehashes the whole level set. Summing the set size over
   all workers gives `workers * states / 2` reinserts, all on one thread.
5. The `i == queue.len()` branch at `solve.rs:310` allocates two `Vec` values
   and calls `merge` once per state. Together with item 4 this is quadratic in
   the size of that bucket.
6. `Instant::now()` is called 12 times in the hot path, 2 per movegen call, at
   `solve.rs:93`, `391`, `393`, `642`, `644`, `655`, `657`, `667`, `669`, `676`,
   `678`, `687`, `689`. An expanded state runs 1 to 3 movegens, so a state pays
   2 to 6 clock reads. One `clock_gettime` call costs about 20 to 25 ns on
   Linux. A neighbouring `fast::generate` call costs tens of ns.
7. For each child the code builds a `Move` at `solve.rs:77` of
   `jade_nav/src/buffer.rs`, unpacks it, and calls `Move::mask`, which runs
   `canonical_offset` and `canonical_rotation` from
   `jade_core/src/piece.rs:125` and `jade_core/src/piece.rs:72`, reads `PMASK`
   from `jade_core/src/data.rs:49`, and does two variable shifts. The child
   loop also calls `has_full_row`, `clearshift`, `has_isolated_cell`, and
   `has_imbalanced_split`.
8. `clearshift` at `jade_core/src/board.rs:241` is straight-line code of about
   70 to 80 word operations over 6 rows. `place` at `solve.rs:592` calls
   `has_full_row` first, then `clearshift`, so the 6 rows are read twice.
9. `has_isolated_cell` at `jade_core/src/board.rs:162` is branchless code of
   about 15 word operations.
10. `has_imbalanced_split` at `jade_core/src/board.rs:205` runs a 7-iteration
    loop. The loop body computes `b | (b >> 1)` again on every iteration,
    although it does not depend on the loop variable. A seam is complete only
    when all 4 play-field rows have a filled cell at the seam. On a typical
    residue the seam test fails, so the loop reaches column 7 without its early
    return. The function is called on every child.
11. `crates/jade_solve` declared `rayon` in its manifest but used no rayon API.
    `walk_par` used `slice::Chunks` with `Iterator::map`, which runs on the
    calling thread, so the "parallel" walk was serial in fact. `found_atomic`
    was an `AtomicUsize` used by one thread. The plan text that follows
    describes this as parallel; that was wrong.
12. `reachable_debug` at `solve.rs:42` and `reachable_2l_debug` at `solve.rs:49`
    are re-exported at `lib.rs:13` and `lib.rs:14`. No code in the workspace
    calls them. No `--debug` flag exists in `jade_cli`. `docs/boolean-solver.md:50`
    and `docs/boolean-solver.md:57` document a flag that was never written.
13. `crates/jade_solve` has no test module. The tests listed in
    `docs/boolean-solver.md:295` were deferred, per `docs/boolean-solver.md:347`.
14. `allow_hold` is `true` at all four call sites of `solve_inner`. The
    no-hold variant wanted by `docs/boolean-solver.md:308` was never written.
15. `walk_seq` is the only caller of `gen_moves`, `play`, and `insert`. `play` is
    the only user of the `Insert` enum, `hex_board`, and `insert_out`. The
    `debug` flag and `JADE_SOLVE_SEQ` exist only to select `walk_seq`.
16. `walk_par` is the only walk that runs today, because nothing sets `debug`
    and nothing sets `JADE_SOLVE_SEQ` in the workspace.
17. `ref/jigsaw.txt` holds 209 entries of the form `SEQ,BOOL`. 54 are `true`.
    Every entry is a 3-piece queue with a known answer.
18. The workspace `Cargo.toml` has no `[profile.dev]` section, so `cargo run`
    builds at `opt-level = 0`. In that mode rustc does not honour `#[inline]`,
    so every `Board` method is a real call, and `Board::shifted` at
    `jade_core/src/board.rs:63` runs the 6-iteration `cols_below` loop at
    runtime.

### Inferences

These follow from the facts but were not measured.

1. Items 4 and 6 are the most likely cause of most of the 15 seconds. Item 4
   makes the merge quadratic in the workers, and item 6 puts a 20 to 25 ns cost
   on a 30 to 60 ns operation. This is an inference. The split between them is
   not known.
2. Items 7 to 10 set the per-child constant factor. They matter because the
   frontier, item 3, is large.
3. Item 11 means there was no thread pool to begin with, so this item is void.
   The claim that a thread pool bought less than its width was based on a
   misreading of the code. The per-worker cost in items 4 and 5 was real,
   because the chunked `Vec` staging ran on one thread.

### Unknowns

1. The level widths for `TZSIJLOIOJ`. The counters that would report them are
   in the code today, but no run has printed them.
2. The share of wall time in `hashbrown::grow` and `rehash_in_place`. This
   decides whether Phase 2 or Phase 4 comes first.
3. The size of the largest frontier. The upper bound from the cell count
   argument is large enough to be useless.
4. The effect of stronger pruning. It is the only change that reduces the
   search space, and its cost per child is also higher. Its net effect cannot be
   predicted.

## Current Behavior

`reachable(board, queue)` and `reachable_2l(board, queue)` both call
`solve_inner`, which walks a level frontier of `(board, hold)` states. The walk
is breadth-first by queue index. Each state expands into up to 3 movegen calls
and 3 sets of children. Each child costs a mask, an OR, an optional
`clearshift`, 4 pruning tests, and a hash insert. Keys are packed into one
`u64` as the 40 board bits plus a 3-bit hold code.

`walk` at `solve.rs:188` selects `walk_seq` at `solve.rs:198` when `debug` is
set or `JADE_SOLVE_SEQ` is present, and `walk_par` at `solve.rs:285` otherwise.
The comment at `solve.rs:279` claims the two walks produce the same buckets and
the same returned level. The claim is untested.

`crates/jade_solve` has 779 lines. About 340 of them are diagnostics or exist
only for the trace.

## Proposed Behavior

One walk, returning `bool`. No counters, no clock, no environment variables, no
per-placement trace, no debug entry points. The public API loses 2 functions.
The per-child cost drops. The serial merge stops rehash amplification. All 209
answers in `ref/jigsaw.txt` and the answer for `TZSIJLOIOJ` stay the same.

## Design

Work is split into 7 phases. Each phase is one commit, and each phase is
measured before the next one starts.

### Phase 1: Remove debugging and concurrency from `jade_solve`

**Status.** Done. `crates/jade_solve/src/solve.rs` went from 799 to 275 lines.
`crates/jade_solve/src/lib.rs` lost 2 re-exports. `crates/jade_solve/Cargo.toml`
lost the `rayon` dependency.

**Note on the plan below.** The original text of this phase kept the parallel
walk and deleted the sequential one. That was reversed. The owner decided the
solver must be single-threaded, so `walk_seq` was promoted to `walk` and
`walk_par` was deleted. Phase 6 and Phase 7a are withdrawn for the same reason.
The original reasoning is kept below where it is still correct.

**Components.** `crates/jade_solve/src/solve.rs`, `crates/jade_solve/src/lib.rs`,
`crates/jade_solve/Cargo.toml`, `docs/boolean-solver.md`.

**Step 1.1. Record the baseline before deleting anything.** The counters and
`JADE_SOLVE_SEQ` still exist. Run all 209 entries of `ref/jigsaw.txt` twice:
once normally, once with `JADE_SOLVE_SEQ=1`. Save both boolean vectors, and
save the `JADE_SOLVE_STATS` output for a few of them. This proves the two walks
agree and gives the reference for every later phase. Commit the file.

**Step 1.2. Delete the diagnostics.** Remove the `Stats` struct at
`solve.rs:102`, its `add` and `report` at `solve.rs:119`, the `stats` field on
`Solve` at `solve.rs:182` and on `WorkerOut` at `solve.rs:518`, the 20 counter
write sites, and the `stats` parameter on `rate`, `place`, `expand_state`, and
`merge`. `merge` becomes `merge(buckets, next1, keys1, next2, keys2)`.

**Step 1.3. Delete the clock reads.** Remove all 12 `Instant::now` pairs listed
in Fact 6, and the `started` and `elapsed` locals in `solve_inner`.

**Step 1.4. Delete the debug walk and its exclusive helpers.** Remove the
`walk` dispatch at `solve.rs:188`, `walk_seq` at `solve.rs:198`,
`Solve::gen_moves` at `solve.rs:389`, `Solve::play` at `solve.rs:402`,
`Solve::insert` at `solve.rs:446`, the `Insert` enum at `solve.rs:158`,
`hex_board` at `solve.rs:730`, and `insert_out` at `solve.rs:735`. Rename
`walk_par` to `walk`. Remove the `debug` parameter of `solve_inner` and the
`debug` field of `Solve`.

**Step 1.5. Collapse the result to `bool`.** `solve_inner` returns `bool`.
`FOUND_NONE` at `solve.rs:20` goes. `AtomicUsize` becomes `AtomicBool`, and
`fetch_min` at `solve.rs:352` becomes `store(true)`. `expand_state` returns
`bool`. This is internal only, because the two public functions already discard
the level. The comment at `solve.rs:346` about minimal lock count becomes
false and must be rewritten.

**Step 1.6. Delete the debug entry points.** Remove `reachable_debug` and
`reachable_2l_debug`, and the re-exports at `lib.rs:13` and `lib.rs:14`.

**Step 1.7. Keep `allow_hold` and `Rated` unchanged.** After `walk_seq` goes,
`Solve::allow_hold` is read once, at `solve.rs:291`, to build the `Frame`. It
becomes a pass-through, which no lint reports. `Rated` at `solve.rs:534` keeps
its 3 variants. `rate` must keep them: `place` pushes a child to the sink only
for `Rated::Recorded`, so a two-state return would push pruned children and
lose the pruning.

**Step 1.8. Update the docs.** Remove the `--debug` line at
`docs/boolean-solver.md:50` and the paragraph at `docs/boolean-solver.md:57`.
Add an entry to the Deviations section of `docs/boolean-solver.md` recording
what was removed, and stating that `allow_hold` is now always `true` and exists
for the planned no-hold test rather than for a reachable mode.

**Resulting call graph.**

```
reachable / reachable_2l
  -> solve_inner(board, queue, lines, allow_hold) -> bool
       Solve { goals, n_goals, two_l, queue, allow_hold, buckets }
       Solve::walk() -> bool                        (was walk_par)
         expand_state(frame, piece, level, key, next1, next2) -> bool
           fast_moves
           place(frame, level, mv, b, hold, sink) -> bool
             rate(frame, level, b, hold) -> Rated
         merge(buckets, next1, keys1, next2, keys2)
       pack / unpack / fast_moves                   (unchanged)
```

### Phase 2: Fix the merge rehash amplification

**Components.** `crates/jade_solve/src/solve.rs`.

**Step 2.1.** In `walk`, sum the `next1` and `next2` lengths over all worker
results. Reserve once per successor level before the merge loop. Remove the
`reserve` call from `merge`.

**Step 2.2.** In the `i == queue.len()` branch, hoist the two `Vec` values out
of the per-state loop and call `merge` once after the loop.

This is the smallest phase in the plan. It changes 4 lines. Fact 4 gives the
current cost as `workers * states / 2` reinserts; this makes it `states`.

### Phase 3: Cheaper fillability tests in `jade_core`

**Components.** `crates/jade_core/src/board.rs`.

**Step 3.1. Hoist the invariant out of the seam loop.** In
`has_imbalanced_split`, compute `u = b | (b >> 1)` and `holes = FIELD & !u` once,
before the loop. The existing `b >> 1` shifts column 0 of row `r` into column 9
of row `r - 1`. That garbage is never read today, because `col_bits` only ever
covers columns 1 to 7. The rewrite must keep that property, or mask column 0
out of `u` before the complement.

**Step 3.2. Test the seam with one constant mask.** For column `c` from 1 to 7,
the seam is incomplete when `holes & (ROW_MASK << (10 * c)) != 0`. Test that
first, and compute `left.count_ones() % 4` only when the seam is complete. Keep
the range 1 to 7, as the doc comment at `board.rs:201` explains.

This replaces about 6 word operations and a `popcount` per iteration with about
2, and removes a recomputed `b | (b >> 1)`. It does not change any answer.

**Step 3.3. Fuse the row test into `clearshift`.** `place` at `solve.rs:592`
calls `has_full_row` and then `clearshift`, so `clearshift` reads the 6 rows
twice. Give `clearshift` the row test, and have `place` call only the new
method. The method returns the same board as today.

**Tests for Phase 3.** A differential test that compares old and new
`has_imbalanced_split` over random boards and over structured boards, including
every residue with 4, 8, 12, and 16 empty cells. A test that compares the fused
method against `clearshift` for every board with 0, 4, 8, 12, 16, 20, 24, 28,
32, 36, and 40 filled cells in the field. `jade_perft`'s frozen counts must not
change, because both rewrites preserve behaviour.

### Phase 4: Remove the `Move` round trip from the child loop

**Components.** `crates/jade_core/src/data.rs`, `crates/jade_solve/src/solve.rs`.

**Step 4.1. Add the table.** Build `pub const SHAPE: [[u64; 4]; 7]` in a `const`
block in `jade_core/src/data.rs`. `SHAPE[p][r]` holds the 4 cells of piece `p`
in canonical rotation `r`, relative to the raw origin `(0, 0)`, that is, with
the raw origin at bit 0.

`place_mask` at `jade_core/src/data.rs:115` builds the same result through
`canonical_offset`, `canonical_rotation`, a `PMASK` read, and 2 variable
shifts. The combined net shift of a cell is `((cell_dy - offset_y) * 10) +
(cell_dx - offset_x)`. `SHAPE` is the `PMASK` mask pre-shifted by that amount
for each of the 4 cells, so the whole sequence becomes one variable shift.

**Step 4.2. Rewrite the child loop.** Replace iteration over `MovesIter` with a
loop over the 4 rotation planes. For each plane, walk the set bits:

```
for r in 0..4 {
    let shape = SHAPE[p as usize][r];
    let mut plane = moves.mask[r].0;
    while plane != 0 {
        let tz = plane.trailing_zeros();
        plane &= plane - 1;
        let child = b | ((shape << tz) & MASK);
        ...
    }
}
```

Planes at or above `groups()` are zero, because `Moves::empty` and
`fast::generate` never set them, so all 4 run without a branch. `tz` is at most
39, so the shift count never reaches 64. Bits above 59 are dropped, which is
what `place_mask` does at `jade_core/src/data.rs:132`. The identity is
`(SHAPE[p][r] << tz) & MASK == place_mask(p, r, tz % 10, tz / 10)`.

**Test for Phase 4.** An exhaustive test over all 7 pieces, 4 rotations, 10
columns, and 6 rows, which is 1680 cases. It must compare `SHAPE` against
`place_mask` for every case. This test is required and must not be removed.

**Interfaces.** `jade_core` gains 1 public `const` item. `jade_nav` is
unchanged. `Move::mask` stays for `jade_perft` and `jade_cli`.

### Phase 5: A purpose-built frontier table

**Components.** `crates/jade_solve/src/solve.rs`.

**Step 5.1.** Add an internal type with 2 fields. `slots` is a `Vec<u64>` whose
length is a power of two, at least twice the expected state count, and it uses
`u64::MAX` as the empty marker, which no real key can equal because a key has at
most 43 significant bits. `keys` is a `Vec<u64>` that holds the keys in
insertion order. Insertion uses Fibonacci hashing,
`(key * 0x9E37_79B9_7F4A_7C15) >> (64 - log2 len)`, then linear probing.

**Step 5.2.** Replace the `FxHashMap<usize, FxHashSet<u64>>` with one such
table per level. Because `keys` holds insertion order, the walk iterates
contiguous memory, and the per-level `cur.into_iter().collect()` at
`solve.rs:328` goes. Only 3 levels are live at once, so keep 3 tables and
reuse them in a round robin. Clearing is one `fill`, which is about 2 stores
per key inserted.

**Step 5.3.** Reuse buffers across levels and across `reachable` calls, so a
wide level does not allocate a new `Vec` per worker per level.

**Test for Phase 5.** Compare the new table against `FxHashSet<u64>` on a
random key stream of 10 million keys, checking that the inserted set is equal
and that duplicate reports match.

### Phase 6: rayon tuning — WITHDRAWN

The solver is single-threaded by decision of the owner. There is no thread pool
to tune. The original phase text is kept below for the record only. Do not
implement it.

**Components.** `crates/jade_solve/src/solve.rs`.

**Original text, no longer valid.**

**Step 6.1.** Replace `map` with `map_init` or with `fold` and `reduce`, so the
two candidate `Vec` values live once per thread instead of once per task. With
`LEVEL_CHUNK` of 2048, a level of 10 million states allocates about 10,000
`Vec` pairs today.

**Step 6.2.** Check `found_atomic` once per chunk instead of once per state at
`solve.rs:338`. The current load puts all workers on one cache line on every
expanded state.

**Step 6.3.** Make the chunk length `max(256, len / (threads * 8))`, so a short
level does not pay 2048-state task granularity.

**Trade-off.** `fold` and `reduce` make an early exit weaker. Fact 2 shows the
early exit has no value on inputs of this shape, because the goal is at the last
level. The trade-off belongs in a comment.

### Phase 7: Optional, decided by measurement

Both items run only if the measurement after Phase 6 shows the limit is still
untouched. They are listed so the plan is complete, not because they are
recommended now.

**7a. Parallel dedup. WITHDRAWN.** The solver is single-threaded, so a
parallel merge is not possible. The original text follows.

**7a, original text.** Fact 11 says the merge is serial. If the merge is still
more than about 25% of the time, split it into 2 phases. Phase 1 is parallel:
each worker writes candidates to a private `Vec` and removes its own duplicates
with a private table. Phase 2 is parallel: split the candidates into 8 to 64
slices by hash prefix, and give each task one slice and one shard table. No
locks and no shared writes.

**7b. Exact empty-space pruning.** Unknown 4 says the effect cannot be
predicted. Add a test in `jade_core` that flood fills the empty field cells
from row 3, rejects a state where an empty cell is unreachable, then requires
every 4-connected component of the empty cells to have a cell count that is a
multiple of 4. A tetromino is 4-connected and enters from above, so both
conditions are necessary. The test is strictly stronger than the pair it
replaces. It costs about 60 to 120 word operations per child, against about 45
to 70 now. Compare the level widths and the wall time before and after, and
adopt it only if the frontier shrinks by more than the added per-child cost.
This is the only change that alters `jade_perft`'s frozen node count, because
`jade_perft` uses the same predicates.

## Alternatives

- **Keep `Stats` behind a cargo feature.** Rejected. A feature that is off by
  default is still debugging code in the solver, and the goal is complete
  removal.
- **Keep `reachable_debug`.** Rejected. It has no callers, and `jade_cli` never
  had a `--debug` flag.
- **Keep `walk_seq`.** **Selected.** The owner requires a single-threaded
  solver. `walk_seq` was promoted to `walk` and is now the only walk. The
  original reason for rejecting it, that its only purpose was the trace, does
  not apply now that the trace is gone and the parallel walk is gone.
- **Keep the target level for minimality tests.** Rejected for now. Nothing
  reads it, and the minimality claim is already not guaranteed, because a child
  of level `i` is inserted at `i + 1` or `i + 2` and the walk returns whichever
  comes first.
- **Collapse `Rated` to 2 states.** Rejected. `place` pushes only for
  `Rated::Recorded`, so a 2-state return would push pruned children and lose the
  pruning.
- **Depth-first search.** Rejected. Fact 2 shows the goal is at the deepest
  level, so a depth-first walk must exhaust the space, and it cannot use a
  thread pool.
- **Mirror symmetry across the queue.** Rejected. Mirroring swaps `J` and `L`,
  so it does not preserve a fixed queue. `docs/boolean-solver.md:240` gives the
  same reason.
- **Transposition across levels.** Rejected. A state at a deeper level has fewer
  pieces left, so it has a different future.
- **Keep `hashbrown` and only reserve once.** Valid and smaller than Phase 5.
  Take it if Phase 5 proves too large, after Phase 2 has been measured.
- **Move the search to the batched SIMD form of `docs/simd-boards.md`.** Not
  selected. The defects in Facts 4 and 6 to 10 cut the same work at lower cost
  and with less risk.

## Risks

- **Deleting `walk_seq` could change an answer** if the two walks differ. The
  comment at `solve.rs:279` claims they agree but nothing tests it. Step 1.1
  checks 209 cases before anything is deleted.
- **A custom table in Phase 5 can be slower than `hashbrown`** if the load
  factor is too high. Keep the load factor at or below 0.5 and measure.
- **`SHAPE` in Phase 4 can disagree with `place_mask`.** The 1680-case test is
  required.
- **The Phase 3 rewrites can change which states are pruned.** The differential
  tests and the frozen `jade_perft` counts are the checks.
- **Collapsing to `bool` in Step 1.5 removes the target level**, so nothing can
  check which level found the goal. `docs/boolean-solver.md:156` promises
  minimality. That promise becomes untestable until the planned tests land. Noted
  and accepted.
- **The counters are the only per-level visibility.** Phase 1 deletes them, so
  the measurement must live outside the solver. Unknowns 1 and 2 then need
  `perf record`, which is a sampling profiler and gives no counters.
- **Phase 7b can change `jade_perft`'s frozen counts** at
  `crates/jade_perft/src/perft.rs:102`. A sound prune can only lower the count,
  so the value must be updated when that phase lands.
- **`fold` and `reduce` in Phase 6 weaken the early exit.** Noted, and Fact 2
  says the early exit has no value on these inputs.
- **An unused import or parameter is the likely build failure** in Phase 1. The
  workspace denies `clippy::perf` and `missing_const_for_fn`.

## Compatibility

- `jade_solve` loses 2 public functions, `reachable_debug` and
  `reachable_2l_debug`. No code in the workspace calls them. This is a breaking
  change for any outside user.
- `JADE_SOLVE_STATS` and `JADE_SOLVE_SEQ` stop working. Neither is set anywhere
  in the workspace.
- `jade_cli` is unchanged. It never had a `--debug` flag, so `docs/boolean-solver.md:50`
  described a flag that was never written.
- The answers from `reachable` and `reachable_2l` do not change. Step 1.1 and
  the Testing section are the checks.
- `jade_core` gains 1 public `const` item, `SHAPE`, in Phase 4, and changed but
  behaviour-preserving `has_imbalanced_split` and `clearshift` in Phase 3.
  `jade_nav` is unchanged in every phase.
- `jade_perft` node counts do not change until Phase 7b.
- Debug builds stay slow. Fact 18. This is recorded as a note in Phase 8, not
  changed, because the release build is the one that matters.
- Each phase is one commit, so a phase can be reverted on its own.

## Testing

### The measurement harness

`JADE_SOLVE_STATS` is gone after Phase 1, so the harness must not depend on the
solver. It uses `ref/jigsaw.txt`, which gives 209 queues with known answers.

For each `SEQ,BOOL` line, run `jade_cli solve SEQ` and compare the printed
boolean to `BOOL`. Report any mismatch and the total wall time. Each 3-piece
queue has one expansion in `Pattern::expand`, so one process per line is enough.

`perf record -g` on `jade_cli solve TZSIJLOIOJ` gives the hot functions. It is
a sampling profiler, so it reports shares, not counts. The owner has used
`perf` in this repository before, as `perf.data` and `flamegraph.svg` in the
repository root show.

No test in the workspace may call removed solver code, and no new code goes into
`jade_solve` to support measurement.

### Checks that apply to every phase

1. All 209 answers in `ref/jigsaw.txt` match the baseline.
2. `TZSIJLOIOJ` answers `true`.
3. `cargo clippy --workspace --all-targets` is clean.
4. `cargo test --workspace` passes.
5. Wall time over the 209 cases is recorded before and after.

### New tests, with the phase that adds them

1. Phase 3: differential test for `has_imbalanced_split`, and a test that
   compares the fused `clearshift` against `clearshift`.
2. Phase 4: the 1680-case `SHAPE` identity test.
3. Phase 5: the new table against `FxHashSet<u64>` on 10 million random keys.
4. Later, the tests already listed at `docs/boolean-solver.md:295`. They are not
   in this plan, because they are not needed to validate it. Note that test 8
   there needs a no-hold entry point, which Phase 1 does not add.

## Implementation Steps

1. Commit this plan.
2. Phase 1, Step 1.1: record the baseline from both walks. Commit the file.
3. Phase 1, Steps 1.2 and 1.3: delete the diagnostics and the clock reads.
   Run the checks. Commit.
4. Phase 1, Steps 1.4 to 1.6: delete the debug walk, collapse to `bool`, delete
   the entry points. Run the checks. Commit.
5. Phase 1, Steps 1.7 and 1.8: confirm `allow_hold` and `Rated` are unchanged,
   update the docs. Run the checks. Commit.
6. Phase 2: fix the merge reserve and the per-state merge. Run the checks.
   Record the wall time. Commit.
7. `perf record` on the current build. Read the share in `hashbrown::grow` and
   `rehash_in_place`. If the share is small, do Phase 4 before Phase 3.
8. Phase 3. Run the checks. Record the wall time. Commit.
9. Phase 4. Run the checks. Record the wall time. Commit.
10. Phase 5. Run the checks. Record the wall time. Commit.
11. Phase 6. Run the checks. Record the wall time. Commit.
12. Decide on Phase 7 from the measurement. Run whichever parts are chosen, with
    the checks after each. Commit.
13. Add the note about `opt-level = 0` to `docs/boolean-solver.md`.
14. Compare the result against this plan. Record the deviations, and the final
    wall times for `TZSIJLOIOJ` and for the 209 cases.

## Decisions

These came from the owner and are already applied above.

1. The 15 seconds is from a release build, so the build profile is not a cause
   and Phase 8 is a note.
2. `jade_core`, `jade_nav`, and `jade_perft` may change, so `SHAPE` and the
   improved prune tests are shared rather than duplicated.
3. Phase 7b is decided by measurement, not scheduled.
4. `reachable_debug` and `reachable_2l_debug` are removed.
5. `allow_hold` is kept.
6. `solve_inner` returns `bool`, so the target level is gone.
7. Measurement is the 209-case harness plus `perf record`, both outside the
   solver.
8. **The solver is single-threaded.** No thread pool, no atomics, no `rayon`.
   This supersedes Steps 1.4 and Phase 6, and it withdraws Phase 7a.

## Deviations

### Phase 1 is done, and it differs from the plan in three ways

1. **The parallel walk was deleted, not kept.** The plan kept `walk_par` and
   renamed it `walk`. The owner requires a single-threaded solver, so `walk_seq`
   was promoted instead. `walk_par`, `Frame`, `WorkerOut`, `Rated`, `rate`,
   `place`, `expand_state`, `merge`, and `Stats::add` are all gone. `FOUND_NONE`
   and the `AtomicUsize` import are gone with them.
2. **`Stats` and the clock reads are gone, as planned.** `solve_inner` returns
   `bool`. `insert` returns `bool` instead of `Insert`, so the `Insert` enum,
   `hex_board`, and `insert_out` are gone. `play` lost its `mode` and `piece`
   parameters, because the trace was their only consumer.
3. **`rayon` was removed from `crates/jade_solve/Cargo.toml`.** The plan did not
   list this. The crate declared `rayon` but called no rayon API, so the
   dependency was dead weight.

### Corrected facts in this document

- Fact 11 and Inference 3 claimed a thread pool existed in `jade_solve`. It did
  not. `walk_par` used `Iterator::map`, not a rayon parallel iterator, so it ran
  on one thread. The plan's performance reasoning about "workers" described a
  serial loop.
- Fact 4 and Fact 5 remain correct. The per-chunk `Vec` staging and the
  per-worker `set.reserve` were real costs, paid on one thread.

### Verification actually performed

- `cargo build -p jade_solve` passes with no warnings.
- `cargo clippy --workspace --all-targets` reports 3 warnings, all in files this
  phase did not touch: `jade_core/src/piece.rs`, `jade_solve/src/parse.rs`, and
  `jade_cli/src/main.rs`.
- `cargo test --workspace` passes. 3 passed, 1 ignored, 0 failed. The crate has
  no test module, so no test covers the solver directly.

### Known risk, not closed

Step 1.1, the 209-case answer baseline, was **not** run. The owner chose build
and lint only. The claim that `walk_seq` and `walk_par` returned the same
answers was never tested, and `walk_seq` is now the only walk. An answer
regression would not be caught by the current checks. The 209 cases in
`ref/jigsaw.txt` are the reference if this needs to be closed later.
