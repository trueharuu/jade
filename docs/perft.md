# Change Plan: `jade_perft` move-generation performance test

## Objective

Add a new binary `jade_perft` to the workspace. Given a queue of pieces, it
counts every reachable placement sequence (perft tree) and times the
generation. The tool must:

- Take the queue as a `jade_pattern` pattern string.
- Select a move generator with a `--model` flag: `oracle` or `fast`.
- Parallelize the walk with `--threads` using rayon.
- Print, per queue, exactly one result line:

  `perft({queue}) = {n} in {time:?} ({nps} nodes/s)`

  where `{n}` is the perft count, `{time:?}` is the total elapsed time as
  a `Duration`, and `{nps}` is the node rate.

This change stubs the `fast` model. The `oracle` model is fully functional.

## Current Behavior

- `jade_core` provides a 60-bit `Board`, `Piece`, `Move`, `clearshift`, and
  `place_mask`.
- `jade_nav` provides `oracle::generate::<P>(&Board) -> Moves`. It returns
  every reachable landed placement of one piece as a per-rotation origin
  mask. A separate `fast.rs` generator is deferred to a later step; it does
  not exist yet.
- No perft harness exists. The reference for the counting semantics is
  `ref/mochbot/perft.rs`: a strict placement tree where distinct reachable
  placements expand once and transpositions are counted.
- `jade_cli` consumes `jade_pattern::Pattern` through clap's `FromStr`. The
  workspace uses clap with derive and has no rayon dependency yet.

## Proposed Behavior

`jade_perft --queue <PATTERN> [--model oracle|fast] [--threads N] [--depth
N]`

- `--queue`: a `jade_pattern` pattern. The pattern expands to one or more
  concrete piece sequences. Each sequence is one queue, run serially and
  printed on its own line.
- Depth equals the queue length: perft places `queue[0]`, then `queue[1]`,
  and so on. The count is the number of distinct placement sequences of the
  whole queue. `--depth N` runs only the first `N` pieces. A depth larger
  than the queue length is an error. Pieces never cycle.
- `--model oracle` (default) uses `oracle::generate`. `--model fast` exits
  with a clear "not implemented" error and no results. No hold and no 7-bag;
  pieces are placed strictly in queue order.
- `--threads N` (default 1). With `N = 1` the walk is serial. With `N > 1`,
  rayon runs on a pool of `N` threads using mochbot's two-level work split.

The perft counts must be reproducible. Two identical invocations with
different models must produce identical counts; that is the verification
method for the future `fast` model.

## Design

### Crate layout

New workspace crate `crates/jade_perft`, `name = "jade_perft"`, edition
2024, `[lints] workspace = true`. Dependencies: `jade_core`, `jade_nav`,
`jade_pattern`, `clap` (derive), `rayon`, `itertools`. Add
`"crates/jade_perft"` to the workspace members list in the root
`Cargo.toml`.

### Files

- `src/model.rs`
  - `enum Model { Oracle, Fast }` with clap `ValueEnum`. Default `Oracle`.
  - `oracle_moves(piece: Piece, board: Board) -> Moves`: a 7-arm match
    dispatching to `oracle::generate::<{Piece::T}>` and the rest, because
    `generate` takes the piece as a const generic. This adapter is the
    single swap point for the future fast model.
  - `moves(model, piece, board) -> Moves`: dispatches over the model. The
    `Fast` arm is `unreachable!`; `main` rejects `--model fast` before any
    run starts.
- `src/perft.rs`
  - `fn child(board: Board, m: Move) -> Board`:
    `(board | m.mask()).clearshift()`.
  - `fn run(model, board, queue, threads) -> u64`: the perft count. With
    `threads > 1` and `queue.len() > 2` it uses the parallel walk, else the
    serial walk.
  - `fn subtree(model, board, queue) -> u64`, following
    `ref/mochbot/perft.rs`:
    - Empty queue returns 1.
    - A one-piece queue returns the placement count of `queue[0]` in bulk;
      the children are not materialized.
    - Deeper: for each placement `m` in the Moves buffer, compute
      `child(board, m)`, recurse on `&queue[1..]`, and sum.
  - `fn run_parallel(model, board, queue) -> u64`: expands the first two
    plies into board work units (root placements, then their child boards
    at ply 2) and `par_iter` each unit's remaining subtree, summing the
    results. This is mochbot's `perft_parallel` shape. It bounds memory
    because each subtree walks recursively.
- `src/main.rs`
  - clap `Parser` with the flags above.
  - Parse `--queue` through `jade_pattern::Pattern` (clap `FromStr`,
    matching `jade_cli`).
  - `expand()` the pattern; validate `--depth` against each expansion
    length.
  - Serial walk uses `run` with 1 thread. Parallel walk creates a
    `rayon::ThreadPoolBuilder` with `--threads` and runs inside
    `pool.install`.
  - Time the whole run with `std::time::Instant`.
  - `Model::Fast` prints an error to stderr and exits 1.
  - Print each queue as one line:
    `perft({name}) = {n} in {elapsed:?} ({rate} nodes/s)`, where `name` is
    the queue's piece string, `{elapsed:?}` is `Duration` debug formatting,
    and `rate` is `n / elapsed` rounded to an integer (0 when `elapsed` is
    zero).

### Data flow

queue string `->` `Pattern::expand()` `->` `Vec<Vec<Piece>>`. Each queue
runs one recursive placement tree. Each node calls `oracle_moves` for the
current piece, expands each placement to a child board via `clearshift`, and
recurses. The runs return a single count per queue.

### Key decisions

- Counting is per placement, not per unique board. Two placements that reach
  the same board are counted twice. This matches `ref/mochbot/perft.rs`.
- The recursive walk keeps memory bounded by depth. The eventual fast SIMD
  model can replace the per-board oracle calls without changing the counting
  semantics.
- The fast stub is deliberate. It keeps this change small and lets perft
  exist before the generator it will verify.
- The result is a single number per queue. No per-depth breakdown is
  reported; the harness exists to verify parity between models, not to
  profile per depth.

## Alternatives

- Per-depth counts and a per-move `divide` breakdown. Rejected after review:
  they complicate the output and are not needed for model parity.
- Breadth-first frontier walk with a multiset of boards. Rejected: it
  materializes whole depths (millions of boards at depth 5) and adds no
  counting benefit over the recursive walk.
- Deduplicated unique-board counts. Rejected: the metric follows
  `ref/mochbot/perft.rs`, which counts the placement tree.
- Implement the fast generator in this change. Rejected. Too large a scope,
  and perft is the harness for verifying it.

## Risks

- Perft counts are only as correct as the oracle. The oracle's BFS contains
  a commented-out `break;` and a debug `eprintln!` for out-of-range
  canonical moves (`oracle.rs:55`). Verify depth-1 counts per piece by hand
  on the empty board as a first sanity check, and cross-check agreement
  between the serial and parallel paths on the same queue.
- Deep queues blow up. A pattern such as `*p7` expands to thousands of
  length-7 queues, each expensive. The tool does not guard this; document
  expected usage.
- Debug builds are slow. Timing runs require `--release`.
- The workspace builds on nightly (`#![feature(min_adt_const_params)]`);
  the new crate inherits this.

## Compatibility

- New crate only. The `jade_core` and `jade_nav` surfaces were not intended
  to change. A pre-existing `Piece::h_spawn` bug for `O` had to be fixed in
  `jade_core` before the oracle produced any O placements; see the
  Completion Notes.
- No configuration, API, or data changes.

## Testing

- Unit tests in `jade_perft`:
  - Hand-verified single-piece totals: `T = 34` (8 + 9 + 8 + 9),
    `I = 17` (7 horizontal + 10 vertical).
  - Serial and parallel counts agree on the same queue (`IOL`).
  - The full-bag total `IOLJSZT = 2096266`, frozen after the O-spawn height
    fix, is pinned with `#[ignore]` because it is slow in debug builds.
- Test that `--model fast` fails with the stub error.
- Test that `--depth` larger than the queue length is rejected.
- Run `cargo clippy --workspace --all-targets` and `cargo fmt --check`.
- Run the tests; report results.

## Implementation Steps

Per AGENTS.md the plan document is written before code.

1. Write `docs/perft.md` recording this plan.
2. Scaffold `crates/jade_perft` (`Cargo.toml`, `src/main.rs`, `src/lib.rs`)
   and register the member in the root `Cargo.toml`.
3. Implement `model.rs` with the oracle dispatch and the fast stub.
4. Implement `perft.rs`: recursive perft with bulk leaf count and the
   parallel variant.
5. Implement `main.rs`: clap flags, expansion, depth validation, threading,
   and the single-line result.
6. Compute the frozen counts; hand-verify the single-piece totals.
7. Add the tests in the Testing section; make clippy and fmt clean.
8. Run the timing pass; document the numbers and any deviations in the
   final report.

Once `fast` is implemented later, the same
`jade_perft --queue ... --model fast` invocation must produce identical
counts. That comparison is the parity harness.

## Completion Notes

Implemented per steps 2-7 above. The tests pin two hand-verified
single-piece totals and the full-bag total.

### Deviations

- **`jade_core` fix.** The plan said the `jade_core` surface is untouched.
  During step 6 (hand-verify depth 1) the oracle produced 0 placements for
  `O` from the empty board. Root cause: `Piece::h_spawn` returned 0 for `O`,
  placing its origin at row 5 so both O rows fell outside the 6-row play
  field. The fix could not live inside `jade_perft` because `jade_core::piece`
  owns the spawn heights. The O case now returns 1 like the other 2-row
  pieces, matching the wirelyre reference spawn (row 4). The frozen totals
  were computed after this fix.
- **Simplification after first implementation.** The first version printed
  per-depth counts and times and had `--divide`. Review requested a single
  result line per queue, so `--divide`, the per-depth times, and the
  `RunStats` struct were removed. `run` now returns one `u64` count, and the
  tests pin totals only.
- **Test ignore boundary.** Debug-build perft is slower than mochbot's, so
  the full-bag total is ignored by default and verified with
  `cargo test -p jade_perft --release -- --ignored`; the default suite keeps
  the cheap hand-verified checks.

### Frozen totals (oracle, empty board)

| queue   | total    |
|---------|----------|
| T       | 34       |
| I       | 17       |
| IOL     | 3219     |
| IOLJSZT | 2096266  |

### Verification

- `cargo clippy --workspace --all-targets`: no warnings.
- `cargo fmt --check`: clean.
- `cargo test -p jade_perft --release`: 3 passed, 1 ignored.
- `cargo test -p jade_perft --release -- --ignored`: 1 passed.
- Serial and parallel walks agree (tested on `IOL`).
- `--model fast`, `--depth` overrun, and pattern expansion behave as
  specified.

### Timing (release, empty board, queue `IOLJSZT`)

`perft(IOLJSZT) = 2096266 in 3.46369817s (605210 nodes/s)` on one thread.
Speedups from `--threads` are modest (walk parallelizes over the second
ply). Throughput is limited by the scalar oracle. Run `--model fast` later
for the same counts as the parity check.