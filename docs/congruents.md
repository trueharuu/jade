# Congruents — Change Plan

## Objective

Add a `jade_cli congruents` subcommand.

The command takes a target field and a queue pattern. It reports every distinct
sequence of playable placements that builds the target field, for each queue the
pattern expands to.

`congruents` answers the opposite question from `solve`. `solve` asks whether a
sequence exists, and answers `yes` or `no`. `congruents` asks which sequences
exist, and lists them.

## Decisions

These are the answers from the project owner. They are fixed inputs to the
design.

| Question | Answer |
| --- | --- |
| Placements | Playable, the same model as `solve`, from `jade_nav::fast::generate` |
| Line format | `{:?}` of a `Vec<Move>`, so `[Move(259), Move(22)]` |
| Hold | Always on, and the path does not record swaps |
| One result | Every distinct placement sequence, reported once |
| Swap duplicates | Merged, by a set |
| Queue length | The path uses as many queue pieces as it needs |
| Output | Flat, one line per path, no headers, no total, no limit |
| No paths | No output, exit code 0 |
| Threads | Single-threaded |
| Target flag | `--field`, the target, which is the opposite of `solve` |
| Empty target | Print `[]` once, exit code 0 |
| Target size | 8 to 20 cells, so 2 to 5 placements, and no size guard |

## Current Behavior

Facts, read from the source in this repository.

- `jade_solve::reachable` and `reachable_save` answer one question: can any legal
  placement sequence fill a goal field. They return `bool` and stop at the first
  path. There is no function that returns the paths.
- The goals in `solve.rs` are the `PC_4` and `PC_2` constants. No goal comes
  from the caller, so no caller-supplied field is a goal today.
- The search in `solve.rs` is a depth-first search over states
  `(level, board, hold)`, where `level` is the index of the next queue piece
  (`crates/jade_solve/src/solve.rs:256`). `Search::visit` returns `bool` up the
  whole recursion, so the first `true` ends the search.
- `solve.rs:313` inserts every state into `Table` and skips a state it has seen.
  A state is `(level, board, hold)`, and the outcome of a state is fixed, so the
  table is sound for a `bool` answer. It keeps one path per state, which is the
  opposite of what `congruents` needs.
- `jade_perft::perft::subtree` already walks a full tree with no dedup
  (`crates/jade_perft/src/perft.rs:34`). It counts leaf nodes and has no path
  buffer.
- `fast::generate` masks each plane with `usable`, `no_land`, and `LANDED_OK`
  (`crates/jade_nav/src/fast.rs:84`). So every placement puts exactly four cells
  on empty cells inside rows 0 to 3. A board reached from the empty field
  therefore always holds a multiple of four cells, and that count is four times
  the number of placements.
- `clearshift` packs the full rows at the bottom and writes the non-full rows
  above them in their original order (`crates/jade_core/src/board.rs:284`). The
  full rows are all equal, so only the order of the non-full rows carries
  information.
- `parse_fumen` reads the first four rows and applies `clearshift`, so a fumen
  argument is always normalized.
- `jade_perft` keeps a private `fast_moves` for the runtime `Piece` dispatch, and
  `jade_solve::solve` keeps its own at `solve.rs:377`. Neither crate shares it
  with the other, because they are separate crates.
- `rustc-hash` is a dependency of `jade_solve` and is not used.

## Proposed Behavior

```
jade congruents -p <pattern> --field <fumen>
```

- `--field` is required. It holds the target field as a fumen.
- A target whose cell count is not a multiple of four is an error. The command
  exits with code 2 and prints the count.
- An empty target is not an error. It prints `[]` once and exits with code 0.
- The pattern expands to queues in the usual way. Every queue is searched.
- Each result is one line, and the order of the lines is the order of the walk.
- Each distinct placement sequence prints once for the whole run, across all
  queue expansions.

`jade_solve` gains one public function and one public type:

```
pub type Paths = FxHashSet<Vec<Move>>

pub fn congruents<F: FnMut(&[Move])>(target: Board, queue: &[Piece], seen: &mut Paths, emit: F)
```

`emit` is called once per distinct placement sequence, with the sequence as a
slice of the walk's own buffer. The callback must not hold the slice.

`seen` is the set of sequences already reported, and the caller owns it. The set
must span every queue of a command, so the caller creates one set and passes it
to each call. The public function cannot create the set itself, because a set
per call would print a sequence once per queue.

The function normalizes the target itself, with `(target & FIELD).clearshift()`,
the same as `solve` does for its start field. The result does not depend on the
caller, so a library caller and the CLI agree.

## Design

### The walk

A depth-first walk of the tree, one route per node, with no state table.

No dedup is possible here. A path is a prefix plus a continuation, so two routes
that reach the same state give two different paths. The `Table` in `solve.rs`
would keep the first route to each state and drop the rest, so it cannot be
used. A separate dead-state table would be sound only for counting, and it
would also cost a lookup and an insert on every node, so it is not used.

The walk copies `solve::expand` (`solve.rs:329`) and `solve::place_all`
(`solve.rs:358`) exactly, so hold works the same way. A swap places one piece
and holds the other. The held piece can still be placed after the queue ends.

The number of placements is fixed. The target holds `P` cells and every
placement adds four, so a path holds exactly `P / 4` placements. A state equal
to the target is therefore terminal, and the target is reachable at one depth
only. The walk does not need a goal set.

The path is one `Vec<Move>` that the walk pushes onto and pops off, so it is
allocated once and reused for every path.

### The alignment prune

This is the one idea here that `solve` does not have, and it is what keeps the
walk small.

`clearshift` writes the full rows at the bottom of the board and the non-full
rows above them, in their original relative order (`board.rs:284`). A board
that is a clearshifted board is a fixed point of `clearshift`, so this holds at
every state of the walk. Two facts follow:

1. A cell's row index is the count of full rows plus its own index among the
   non-full rows.
2. A cell keeps its row index for as long as its row stays non-full. Every new
   full row takes one off its index among the non-full rows and adds one to the
   full row count, so the two cancel.

When the target has no full row, no state can have one either, so no row ever
becomes full, so no cell ever moves. The board must then already be a subset of
the target:

```
if target has no full row and b.0 & !target.0 != 0 -> dead
```

That is the whole test. It is two instructions, and it is strong: it rejects any
cell that is not already where the target needs it.

A target with a full row gets no per-cell test. The first version of this design
tried to add one, and the differential test in `congruents.rs` showed it wrong
twice. A cell can also end inside a full row, and the full rows of a target hold
a cell at every column, so a cell has no column constraint there. What is left
is the full row count:

```
if b has more full rows than the target -> dead
```

Full rows only ever accumulate, so that is sound, and it costs one call to
`markers`.

So the alignment prune is strong for a target with no full row, and absent for a
target with one. The remaining-cells bound is what covers the second case, and
the timings in the Deviations section below show that it is enough for the
target sizes the owner expects.

### Other prunes

- **Remaining cells.** `P - cells <= 4 * (queue.len() - level + has_hold)`. This
  is the bound in `solve.rs:294`, with `P` in place of 40. It rejects the start
  state when the queue is too short, which is the common case for a pattern with
  wildcards.
- **No cell-count test.** `solve` tests `cells & 3 != 0`. It is vacuous here,
  because the walk starts from the empty field and every placement adds exactly
  four cells.
- **No field test.** `solve` tests `b.0 & !FIELD.0 != 0`. It is vacuous here. The
  full rows grow from the bottom, so the residue never rises above row 3.
- **No fillability heuristics.** `is_unfillable`, `has_isolated_cell`, and
  `has_imbalanced_split` all reason about filling all four rows. A target need
  not fill any row at all, so a state can fail those tests and still be on a
  path to the target.
- **No `--limit`.** The decision is to print every path.

### Result dedup

One `FxHashSet<Vec<Move>>` lives for the whole command, not for one queue.

A path can be reachable from two queue expansions, because hold lets a path use
fewer queue pieces than the queue holds. The decision is one line per distinct
placement sequence, so the set is keyed on the whole sequence and is not reset
between queues.

This finally uses the `rustc-hash` dependency of `jade_solve`, which is declared
and unused today. The set holds one `Vec<Move>` per result.

### Interfaces

- New module `crates/jade_solve/src/congruents.rs`, re-exported from
  `crates/jade_solve/src/lib.rs`.
- `solve::fast_moves` becomes `pub(crate)` and is imported, not copied. It is
  already private to the crate, and a third copy of it would be duplication.
  `jade_perft` cannot share it, because that is a different crate.
- `jade_cli` gains one subcommand, `Congruents`. It parses the target with the
  existing `parse_board`, checks the cell count, expands the pattern, calls
  `congruents` per queue, and writes the results to one `BufWriter` over the
  locked standard output.
- The command does not call `process::exit` after printing, so the buffer is
  flushed. The existing commands exit with a status, so this one returns.

## Alternatives

- **Search backward from the target.** Rejected. Inverting `clearshift` is
  ambiguous, because many boards map to one, so a backward walk would need the
  forward walk to enumerate the inverses anyway.
- **Keep the `Table` and store paths in it.** Rejected. A state then keeps one
  path, and every other route to that state is lost.
- **A dead-state table only.** Rejected. It is sound, and it costs a lookup and
  an insert per node, and it is not needed, because the alignment prune already
  removes the dead states that are cheap to find.
- **Store a hash of the path instead of the path.** Rejected. The command prints
  the path, so the path has to be kept to print it, and the same vector is the
  key.
- **Free tiling instead of playable placements.** Rejected by the decision. It
  would be a different question, and it would need a separate mode.
- **A `--limit` flag.** Rejected by the decision.
- **A count or a summary line.** Rejected by the decision.
- **rayon.** Rejected by the decision.
- **A new home for the `Piece` dispatch, in `jade_nav::fast::all`.** Not
  selected. It is the better home in principle, but it widens the change into a
  third crate for 8 lines that `jade_solve` already has.
- **A short flag for `--field`, and a positional target.** Not selected. `-p` has
  a short flag, so `-f` follows, and every other field in the tool is a flag.
- **Reject a large target.** Rejected by the decision. A 40-cell target with a
  long pattern runs for a very long time, and the owner accepted that.

## Risks

- **The alignment prune can cut a valid path.** The prune depends on one fact:
  `clearshift` does not reorder the non-full rows, and a cell keeps its row
  index while its row stays non-full. That fact is read from `board.rs:284` and
  is not asserted anywhere. If a future change lets `clearshift` reorder or drop
  the non-full rows, the prune becomes unsound. The comment on the prune states
  the fact, and the differential test in `congruents.rs` checks the prunes
  against a walk with them off.
- **A target with a full row has no alignment prune.** The walk for such a
  target is larger than the plan assumed. The remaining-cells bound and the
  goal test are all that is left. The measured cost is in the Deviations
  section, and the owner accepted an unbounded walk.
- **The result count is not known.** No limit by decision, and the cost is not
  measured for every target. A target with a full row and a pattern with many
  queues is the slow case.
- **The dedup set grows with the number of results.** It holds one `Vec<Move>`
  per distinct path, which is the same as the output.
- **The output format depends on the derived `Debug` of `Move`.** `Move` is a
  packed `u16` today. Adding a field to it changes the output.
- **The queue prefix rule makes the same path reachable from several queues.**
  That is the intended meaning, and the caller-owned set is what keeps the output
  to one line per path.

## Compatibility

- Additive. A new module, two re-exports, one new subcommand, and `pub(crate)`
  on a private function.
- `reachable`, `reachable_2l`, `reachable_save`, `reachable_save_2l`,
  `is_unfillable`, and `parse_fumen` keep their signatures and their answers.
- The `Solve`, `Percent`, `Move`, and `Perft` subcommands are unchanged, including
  their exit codes.
- Exit code 2 now also means a target whose cell count is not a multiple of
  four. It already means a bad argument, so this adds no new meaning.
- `jade_solve` gains no dependency. `rustc-hash` is already declared, and this
  change is its first use.
- No data and no configuration change.

## Testing

Six unit tests run in `crates/jade_solve/src/congruents.rs`, and two of them are
differential.

- `the_prunes_keep_every_path` walks twenty generated two-placement targets with
  four queues, once with the prunes and once without, and compares the full path
  sets.
- `the_prunes_keep_every_path_into_a_full_row` does the same for up to three
  generated three-placement targets that hold a full row, which is the case the
  alignment prune does not cover.

The walk carries a `const LIMITS: bool` parameter, so the reference walk is the
same code with the prunes compiled out. There is no branch and no cost in the
reported walk.

The reference walk costs about a million states per three-placement target, so
the generated target lists are cut. The tests took 2.6 s in a debug build.

The other four tests cover a single-piece target, a target no queue can build,
the empty target, and one set spanning two queues.

Behavior checks are run by the project owner. The cases:

1. A four-cell target whose cells form a `T` shape. `congruents -p T --field
   <fumen>` must print exactly one path, the `T` that covers those four cells.
   `jade move` on an empty field lists all 34 `T` placements, which checks the
   shape of the output.
2. The same four-cell target with `-p I`. No path, because no `I` covers a `T`
   shape.
3. The same four-cell target with `-p *`. One path for each piece that can cover
   it.
4. A target whose cell count is not a multiple of four. Exit code 2, with the
   count in the message.
5. An empty target. `[]` once, exit code 0.
6. A twelve-cell target with `-p IJL`. The total is checked by hand or against
   an independent count.
7. The swap duplicate case, with a queue such as `T T O` on an eight-cell target
   where the held route and the direct route both work. The path prints once.
8. A fumen that is not already in `clearshift` form. It is normalized, so the
   answer matches the normalized fumen.
9. Consistency with `solve`. For a target of `PC_2` and a queue of exactly five
   pieces, `congruents` must report at least one path exactly when
   `solve <queue> --2l` exits 0. A five-piece queue cannot fill 40 cells, so the
   two answers must agree.
10. A target that no queue can build. No output, exit code 0.
11. A queue shorter than the target needs, such as three pieces against a
    20-cell target. No output, exit code 0.

Cases 1 to 5, 9 and 10 have been run and pass. The results are in the Deviations
section below. Cases 6 to 8 and 11 are for the owner.

Build checks run here: `cargo build --workspace`, `cargo clippy --workspace
--all-targets`, and `cargo fmt --check`. Clippy is clean on the whole workspace.
The workspace sets `missing_const_for_fn = deny` and `perf = deny`, so the new
helpers are `const fn` where they can be.

## Deviations

- **The public function takes the result set as a parameter.** The plan wrote
  `congruents(target, queue, emit)` and also required one set for the whole
  command. A function cannot hold both, so the caller owns the set and the CLI
  passes one set to every queue. The type is public as `Paths`.
- **The alignment prune in the plan was wrong, and the implementation is
  smaller.** The plan shifted the board up by the difference in full row counts
  and required a subset of the target. That drops valid paths, because a cell
  can end inside a full row, and the plan did not model that. The implementation
  uses the plain subset test, and only when the target has no full row. See the
  Design section. The differential tests caught both wrong versions.
- **The prune for a target with a full row is the full row count only.** The
  plan described an alignment prune for every target.
- **`Walk` carries a `const LIMITS: bool`.** The tests walk without the prunes
  using the same code. This is not in the plan.
- **`cargo fmt --check` still fails, on files outside this change.** It reports
  `jade_nav/src/fast.rs`, `jade_nav/src/lib.rs`, `jade_pattern/src/lib.rs`,
  `jade_perft/src/lib.rs`, `jade_perft/src/perft.rs`, and
  `jade_solve/src/parse.rs`. The last one is a whitespace change that came in
  with the save commit. `congruents.rs`, `jade_solve/src/lib.rs` and
  `crates/jade_cli/src/main.rs` are formatted.

Measured results, on a debug build:

| Target | Queue | Result |
| --- | --- | --- |
| 4 cells, `T` shape | `T` | 1 path, 0.0 s |
| 4 cells, `T` shape | `I` | 0 paths |
| 0 cells | `*` | `[]` once, over 7 queues |
| 2 cells | `T` | exit 2, with the count in the message |
| 6 by 2 block, 12 cells, no full row | `OOO` | 6 paths, the 3 orders of 2 each of 2 spots |
| `PC_2`, 20 cells, 2 full rows | `OOOOO` | 120 paths, 5 factorial, 0.15 s |
| `PC_2`, 20 cells | `solve --2l` | exit 0, agrees with the 120 paths |
| 20 cells, no full row | `ITTTT` | 87 paths, 0.65 s |
| 20 cells, no full row | `TTT` | 0 paths, rejected by the remaining-cells bound |

The `PC_2` result is a cross-check that does not depend on the prunes. Five `O`
pieces tile the 10 by 2 field in exactly five spots, and each spot has one
placement, so the only paths are the 5 factorial orders of those five.

## Implementation Steps

1. Write this plan to `docs/congruents.md`.
2. Make `fast_moves` `pub(crate)` in `solve.rs`.
3. Add `congruents.rs` with `markers`, the alignment prune, the walk, the
   remaining-cell bound, the dedup set, and the public `congruents` function.
4. Re-export `congruents` from `lib.rs`.
5. Add the `Congruents` subcommand to `main.rs`, with the cell-count check, the
   per-queue loop, and one `BufWriter`.
6. Run `cargo build --workspace`, `cargo clippy --workspace --all-targets`, and
   `cargo fmt --check`. Revert any reformat of files outside this change.
7. Compare the implementation against this plan and record the deviations above.
8. Hand the test list to the project owner.
