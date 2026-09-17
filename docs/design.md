# 4-Line PC Setup Finder — Design Document

Planning document. No code has been written under this plan.

## Objective

Build a Rust tool that finds 4-line perfect-clear (PC) setups, with
raw search speed as the primary goal. The tool must:

- Support three setup shapes: regular 3-piece, regular 4-piece, and
  2+2 (two placed groups with one pass-through piece placed between
  them).
- Use one pattern grammar for both the setup and the solve parts of a
  query.
- Support a `save` mode, where success is measured by how often a
  solve ends with the held piece matching a given pattern, instead of
  by how often the board clears.
- Report every setup at or above a stated percent cutoff. With no
  cutoff given, still report every setup at exactly 100%.
- Use its own move generator, built for this project, not a
  dependency on the `rue` project.
- Reuse the existing pattern grammar and parser from the
  `hydra-optimal` project (`src/pattern.rs`) for pattern parsing.
- Search on the fly, per query, rather than building one large
  precomputed graph of all reachable boards up front.

## Current Behavior

This section states facts about existing, related software, read
directly from source rather than recalled from memory.

### The reference tool being replaced

`theoluky/setup-finder` (Java) takes two separate pattern flags,
`-sp` for setup pieces and `-p` for solve pieces. It enumerates every
placement of the setup pieces, and for each resulting field, checks
every unique permutation of the solve pattern one at a time, with no
shared cache between permutations and no shared cache between
candidate fields. Total work is close to
`(setup placements) × (solve permutations) × (one placement search)`.
This is the main cause of its slow runtime.

### Related work: a full precomputed graph

`trueharuu/hydra` is a graph-based solver. It builds one full graph of
every reachable 4-line board and every placement edge between them:
`15,185,706` fields and `109,562,993` directed placement edges. This
number describes the full graph across every possible queue input
(for example, ten unrestricted wildcard piece positions in a row). A
graph built for one specific, narrower query pattern would be a
smaller subgraph of this. This project will not build this graph. It
will search per query instead, per the decision recorded above.

### Related work: fast move generation

`trueharuu/rue` has a fast, SIMD-based board and move generator. Its
board type is a bitboard held in a `Simd<u64, N>` value, one bit per
cell. Its faster move-generation path uses fixed bit-trick operations
and lookup tables rather than a variable-length search per board, which
is a useful shape to copy conceptually. Its slower, definitely-correct
path uses a breadth-first search with a variable number of steps per
board. This project will not depend on `rue`'s code. It will build its
own move generator, taking the bitboard representation and the
bit-trick-over-BFS lesson as background evidence, not as a shared
dependency.

## Proposed Behavior

- Patterns are written and parsed using the existing `hydra-optimal`
  grammar (bags, wildcards, permutations, combinations, filters, and
  `expand()` to a list of concrete piece sequences).
- A setup query names a full setup window as a pattern. Regular 3pc
  and 4pc setups are one contiguous group. A 2+2 setup is written as
  several alternatives joined by `;`, one per way of choosing which
  two of the three pieces go in the first group (see Design, "2+2 as a
  pattern," for a worked example).
- `save` takes its own pattern, separate from the setup and solve
  pattern. For an initial version, a `save` pattern must expand to
  only single-piece sequences (for example `[TI]` expands to `T` and
  `I`, both length 1). A solve counts as a `save` success if the board
  is empty and the piece sitting in hold has a type in that expanded
  set.
- With no `--cutoff` given, the tool reports every setup found at
  exactly 100%. With `--cutoff X` given, it reports every setup at or
  above `X`.

## Design

### Board and piece representation

- Field: one `u64`, one bit per cell, bit index `y * 10 + x`. A 4-row
  board needs 40 bits.
- Piece: a 7-value enum, one byte.
- Universal batch type: `struct Batch<const N: usize>(Simd<u64, N>)`,
  with a single board represented as `Batch<1>`. One set of bitwise
  operations (place, overlap check, full-row detection) is written
  once against `Batch<N>` and used unchanged whether `N` is 1 (a
  single board) or larger (many boards processed together, see
  "Level-batched search" below).
  - `Batch<N>` is kept semantically minimal: `N` lanes of `u64`, with
    elementwise bitwise operations. What a lane represents is decided
    by the call site, not by the type. In the setup and solve
    searches, a lane is one independent board. This should be
    commented at each call site, since the same type could equally
    hold one board tested against `N` different placement masks at
    once, and conflating the two without a comment would be confusing
    later.
  - This mirrors the shape of `rue`'s own `Board<N>` type, which also
    wraps `Simd<u64, N>`, but the meaning of `N` is different there
    and should not be assumed to carry over. In `rue`, `N` lanes are
    `N` vertical bands of the *same* board, used to represent boards
    taller than one `u64` can hold. That use needs some cross-lane
    logic, since a line clear near a band boundary can move bits from
    one band into the next. This project's `N` is `N` *independent*
    boards, which must never interact across lanes. This is safe here
    specifically because a 4-line board always fits in one band: the
    vertical-stacking case that would need cross-lane logic never
    comes up, so every operation this project needs (row-full check,
    overlap, drop) is naturally a pure per-lane elementwise operation
    already (`AND`, `OR`, compare, no cross-lane shift). This should
    still be checked operation by operation as each one is written,
    not assumed from this reasoning alone.
- Kick table: the standard SRS kick table (5 kick tests per rotation).
  This is a public, well-known specification, not something specific
  to `rue` or any other single project.
- No T-spin classification is needed anywhere in this design. The
  search only cares about final board state and legal placement, not
  about scoring or spin naming. This removes a whole category of logic
  a general Tetris engine would otherwise need.

### Move generation

- Built new for this project, not reused from `rue`.
- Design goal, stated plainly since it drives every other choice in
  this section: every legal-placement test must be a fixed, small
  sequence of bitwise operations on a board's `u64`, not a loop whose
  length depends on that specific board's shape. This is what allows
  the same test to later run across many different boards at once
  (see "Level-batched search" below). A breadth-first search with a
  variable number of steps per board would not have this property.
- Recommendation: use column-based bit tricks (shift, mask, compare)
  for drop and tuck reachability, and the fixed kick table for spin and
  wall-kick reachability, following the same general shape `rue`'s
  faster move-generation path uses, implemented independently.

### Pattern grammar and the setup window

- Reuse `hydra-optimal`'s `Pattern` type and its `expand()` method,
  which returns every concrete finite piece sequence a pattern can
  mean, as `Vec<Vec<Piece>>`.
- Use `expand()` for the setup window only. The setup window is short
  (a handful of pieces), so materializing every matching sequence is
  cheap. Do not use `expand()` for the solve phase, since a long
  solve pattern (for example, several wildcard positions) would
  produce a combinatorially large list. Generate solve-phase piece
  choices on the fly, one piece at a time, following normal 7-bag
  rules, instead.

### 2+2 as a pattern, worked example

This was checked directly against the grammar's actual parsing and
expansion rules, not assumed.

- Goal: express "2 of {T, S, Z}, in either order, then whichever of
  the three is left over, then I and J in either order."
- `(TS)!` parses as "permute all elements of the group `(T, S)`,"
  which expands to `TS` and `ST`. The same pattern applies to `(TZ)!`
  and `(SZ)!`.
- The full window is written as three alternatives, joined by `;`,
  one per choice of which two pieces are used first:
  `(TS)!Z(IJ)!;(TZ)!S(IJ)!;(SZ)!T(IJ)!`
- This expands to every valid 2+2 ordering: `TSZIJ`, `STZIJ`,
  `TSZJI`, `STZJI`, and the same pattern for the `TZ`/`S` and `SZ`/`T`
  cases. No new grammar feature is needed for this. It is already
  expressible with what exists.
- This also confirms a point from earlier reasoning: the pass-through
  piece (`Z`, in the first alternative) is written into the pattern
  as a literal, in its real position. It is not skipped. The search
  still has to decide where to place it or whether to hold it, the
  same as any other piece in the window.

### Level-batched search

- Process the setup window and the solve continuation one piece-depth
  at a time, keeping the whole frontier of live board states at that
  depth, rather than a normal one-board-at-a-time depth-first search.
- At each depth, deduplicate the frontier by canonical state before
  moving to the next depth. This is the step that keeps the frontier
  from growing exponentially, since many different placement orders
  reach the same state.
  - In the setup window, the piece type at depth `d` is fixed, since
    it comes from one concrete sequence out of `expand()`. Canonical
    state can be just `(field, hold)`, since the remaining pieces are
    already implied by the depth.
  - In the solve phase, piece type at each depth is not fixed.
    Canonical state needs to be `(field, remaining piece multiset,
    hold)`.
- Once a depth's frontier is deduplicated, testing one specific
  placement (a fixed piece type, rotation, and column) against every
  board in that frontier at once is a uniform operation: the same
  bitwise test, run on many different `u64` values. This is the part
  suited to SIMD, batching many boards into wide vector lanes.
- Recommendation: use Rust's `std::simd` (`portable_simd`), which
  needs the nightly toolchain, since it compiles this kind of
  lane-batched bitwise operation to native vector instructions while
  staying readable. This is a reversible implementation choice — a
  stable-Rust SIMD crate could be substituted later without changing
  anything above this paragraph — recommended now because it fits the
  batched-frontier design directly and the project is not a shared
  library, so a nightly-only requirement carries little cost.
- Turning "which boards in this batch passed the test" into a
  compacted list of new states needs its own small design and testing
  pass. This is a well-known kind of operation in SIMD code generally,
  but it is new work for this project, not something with an existing
  reference to copy.
- Frontier size for a specific query pattern has not been measured.
  The `15,185,706`-field number from `hydra` describes the full
  unrestricted graph, not a typical query's working set, and should
  not be used as a size estimate for this design. Measure actual
  frontier sizes on a prototype before choosing a fixed batch width.

### `save`

- `save` takes its own pattern, parsed and expanded the same way as
  any other pattern in this design.
- For this version, require every expansion of the `save` pattern to
  have length 1 (one piece). Reject the pattern at parse time
  otherwise, with a clear error, rather than silently ignoring extra
  pieces.
- Success test in the solve phase: the board is empty, and the piece
  in hold has a type present in the `save` pattern's expanded set.
- Pruning: once every piece type in the `save` pattern's expanded set
  has been placed, with none left in hold or in the remaining pieces
  for this branch, that branch cannot succeed and can be dropped
  immediately.
- Known limitation, not built in this version: you described a
  further case, saving into a specific followup setup such as
  "`I[SZ]` 5th," where the value of holding a piece also depends on
  what pattern the pieces after it match, not just the held piece's
  type on its own. This is out of scope here. The current design
  supports only "does the held piece's type match this pattern,"
  checked once, at the moment the board clears.

### Cache and hashing

- Pack each canonical state (field, remaining multiset where used,
  hold) into a single fixed-size integer, not a struct holding a
  `Vec` or similar heap-allocated data. This avoids heap allocation on
  the search's hot path and makes state comparison a single machine
  word comparison.
- Use a fast, non-cryptographic hash function for the cache (for
  example, from the `rustc-hash` or `ahash` crate) instead of the
  Rust standard library's default hasher, which is intentionally
  slower to resist an attack that does not apply to a single-user
  command-line tool.

### Cutoff and reporting

- With no `--cutoff` given, the effective cutoff is 100%.
- With `--cutoff X` given, report every setup at or above `X`.
- Per-candidate pruning: stop testing a candidate once enough
  branches have failed that it cannot reach the effective cutoff,
  regardless of what any other candidate scored. This needs no shared
  "current best" value across candidates, so candidates can be tested
  fully independently across threads.

## Alternatives

- A full precomputed reachability graph, as `hydra` builds. Not
  chosen. The tool searches per query instead.
- Depending on `rue_core`/`rue_nav` directly, or vendoring their move
  generator into this project. Not chosen. This project builds its
  own move generator.
- Tracking a single "current best" percent and pruning worse
  candidates against it, instead of a fixed cutoff. Not chosen, since
  a fixed cutoff needs no shared mutable state across threads and
  still gives correct results, given the requirement to always report
  every 100% setup regardless of what else is found.

## Risks

- Rolling a new move generator means this project has no existing,
  already-tested implementation of its own to check correctness
  against. Recommendation: build one deliberately simple, obviously
  correct reference implementation (a plain breadth-first search, no
  bit tricks) purely for testing, and check the fast, batchable
  implementation against it on small hand-picked boards before
  trusting its output.
- The stream-compaction step in the level-batched search (turning a
  pass/fail mask into a compacted list of new states) has no existing
  reference in this project to copy. It should be prototyped and
  measured on its own before the rest of the search is built around
  it.
- Frontier size per query is currently an unmeasured unknown, not a
  fact. The `hydra` graph size describes an upper bound on the full,
  unrestricted state space, not a specific query's cost. A batch
  width chosen before measuring could be wrong in either direction.
- A `save` pattern that expands to sequences longer than one piece
  needs to be rejected clearly at parse time, or the tool's behavior
  in that case is undefined by this design.
- Using `std::simd` requires the nightly Rust toolchain. This is a
  recommendation made in this document, not a requirement handed down
  from elsewhere, since this project no longer depends on `rue`.

## Compatibility

New, standalone tool. No existing behavior, API, data, configuration,
or users are affected.

## Testing

- Correctness: compare the fast, batchable move generator against the
  simple reference implementation (Risks, above) on small hand-picked
  boards, for both single placements and full search results.
- 2+2 case: run the worked pattern
  `(TS)!Z(IJ)!;(TZ)!S(IJ)!;(SZ)!T(IJ)!` and check its percent against
  a hand-worked or independently verified answer.
- `save` case: one pattern matching exactly one piece type, and one
  matching several (for example `[TI]`), checking both the success
  test and the exhaustion-based pruning give correct results.
- Cutoff case: a small query with a known mix of percents, checked
  both with no `--cutoff` (only 100% results expected) and with an
  explicit `--cutoff` value (all results at or above it expected).
- Performance: measure frontier size and wall-clock time by depth for
  a small set of representative patterns (3pc, 4pc, 2+2), both with
  and without SIMD batching enabled, to confirm the batching gives a
  real, measured improvement rather than an assumed one.

## Implementation Steps

1. Bring in `hydra-optimal`'s `Pattern` type and parser for pattern
   parsing and `expand()`.
2. Implement the `u64` bitboard type and the standard SRS kick table.
3. Implement a simple, obviously correct reference move generator, for
   testing only.
4. Implement the fast, branch-light, table-driven move generator meant
   for SIMD batching. Check it against the reference generator on
   small cases.
5. Implement the level-batched, per-depth frontier search: deduplicate
   by canonical state at each depth, then batch the deduplicated
   frontier for the next placement test.
6. Implement the setup-window phase using `Pattern::expand()`, and the
   solve phase using on-the-fly bag generation, sharing the same
   per-piece step logic and the same cache.
7. Implement `save`: parse-time validation that its pattern expands
   only to single pieces, the success test, and the exhaustion-based
   pruning.
8. Implement cutoff reporting, with a 100% default when none is given,
   and per-candidate pruning against the effective cutoff.
9. Implement the packed integer cache key and a fast non-cryptographic
   hash for the transposition cache.
10. Build a benchmarking and instrumentation harness that reports
    frontier size by depth and wall-clock time, and use its output to
    choose a SIMD batch width, rather than guessing one in advance.
11. Run every case in the Testing section.
12. Document any deviation from this plan before the final report.