# Piece queue migration plan

## Objective

Replace every `Vec<Piece>`-based piece queue with the one fixed-capacity queue
type in `jade_core::queue::Queue`.

The change must do two things:

* Make the piece queue cheap. A `Queue<Piece>` holds its pieces inline. A
  `Vec<Piece>` holds a pointer, a length, a capacity, and a heap block.
* Make the piece queue say what it is. A queue is a fixed-order list of at most
  11 pieces. One type states that limit. `Vec` does not.

`jade_nav::queue::Queue` stays as it is. It is a worklist of `Move` values for
the oracle BFS, not a piece queue. Both types are named `Queue` in different
modules, which already compiles.

## Current Behavior

### Two unrelated types are named `Queue`

`jade_nav::queue::Queue` is a ring buffer over `MaybeUninit`. It holds `CAP = 4096`
values, uses `idx & MASK` indexing, and has `new`, `push_back`, `pop_front`, and
`is_empty`. Only `jade_nav::oracle::generate` uses it. It stores `Move`, not
`Piece`.

`jade_core::queue::Queue` is `[u8; 11]`. It stores each piece as `Piece as u8 + 1`,
so `0` means no piece. It has `new`, `len`, and `is_empty` only. It has no
`push_back`, no accessor, and no iterator. **It has zero call sites.** No crate
outside `jade_core` names it.

### Piece queues are `Vec<Piece>` at fourteen sites

| Site | Today |
|---|---|
| `jade_pattern::Pattern::expand` | `BTreeSet<Vec<Piece>>` |
| `jade_pattern::Segment::expand` | `Vec<Vec<Piece>>` plus one `Vec<Piece>` per variant |
| `jade_pattern::matches_at` | `pieces: &[Piece]` |
| `jade_pattern::occurrences` | `pieces: &[Piece]` |
| `jade_pattern::count_occurrences` | `pieces: &[Piece]` |
| `jade_perft::model::parse_queue` | `Vec<Piece>` |
| `jade_perft::perft::run` | `queue: &[Piece]` |
| `jade_perft::perft::subtree` | `queue: &[Piece]` |
| `jade_perft::perft::run_parallel` | `queue: &[Piece]` |
| `jade_perft::main::depth_for` | `queue: &[Piece]` |
| `jade_solve::solve::reachable` | `queue: &[Piece]` |
| `jade_solve::solve::Solver::reachable` and `Ctx` | `queue: &[Piece]` |
| `jade_solve::percent::assert_matches_solver` | `queues: &[Vec<Piece>]` |
| `jade_solve::setup::setups` | `Vec<(Board, Vec<Piece>)>` |
| `jade_cli::bin::check::Row` | `queue: Result<Vec<Piece>, String>` |

### The limit of 11 is written in four places

* `jade_core::queue`, as the array length.
* `jade_solve::solve`, as `const MAX_QUEUE: usize = 11`.
* `jade_cli::bin::check`, as `const MAX_QUEUE: usize = 11`.
* The doc comment on `jade_core::queue`, as 10 pieces plus 1 hold.

`solve::reachable` asserts the limit. `check::parse_queue` rejects a longer
queue. `Pattern` has no limit at all, because `Vec` grows.

### Two observable behaviours depend on the current types

`Pattern::expand` returns a `BTreeSet`, so its result order is the `Ord` order of
the element. `jade_cli pattern expand` prints that order. A new element type with
a different `Ord` changes that output.

`percent::percent` takes `I: IntoIterator<Item = Q>, Q: AsRef<[Piece]>`. That
bound already accepts any piece container, so the queue type can replace `Vec`
behind it without a signature change.

## Proposed Behavior

Every piece queue in the tree is a `jade_core::queue::Queue`.

`Queue` holds `[Piece; 11]` and a `len: u8`. It has no sentinel, no
`MaybeUninit`, and no unsafe code. `Piece` is `Copy`, so the array is fully
initialized and `len` marks the live prefix. The type is `Copy`, so passing a
queue costs one register-sized copy and needs no `Drop`.

The length limit of 11 is written once, as `jade_core::queue::CAP`. The two
`MAX_QUEUE` constants are removed.

A pattern that can name more than 11 pieces is rejected by `Pattern::parse`
instead of producing a longer `Vec`. Every expansion of an accepted pattern
fits in a `Queue`.

Answers do not change. Node counts, percentages, and solver results stay as they
are. The one user-visible change is that some over-long patterns now fail to
parse.

## Design

### Components affected

```
jade_core::queue      the queue type, rewritten
jade_pattern          expand, and a new length check in parse
jade_perft            parse_queue, the perft walk, the binary
jade_solve            solve, percent, setup
jade_cli              main, bin/check
jade_nav              unchanged
```

No crate dependency changes. `jade_pattern` already depends on `jade_core`, so
the type is reachable from every crate that needs it.

### The queue interface

The type lives in `jade_core::queue`. Its shape:

```rust
pub const CAP: usize = 11;

pub struct Queue {
    pieces: [Piece; CAP],
    len: u8,
}
```

The workspace sets `missing_const_for_fn = "deny"`, so every method that can be
`const` is `const`. The new interface, and the reason each item is needed:

| Item | Reason a call site needs it |
|---|---|
| `new`, `Default` | build sites |
| `push_back(Piece)` | every producer. Asserts at `CAP` |
| `len`, `is_empty` | already present. `is_empty` becomes `len == 0` |
| `get(usize) -> Option<Piece>` | `solve.rs` reads `queue.get(d)` |
| `first()` | `solve::reachable` reads `queue.first()` |
| `prefix(usize) -> &[Piece]` | the `--depth` cut, the `keep` cut in `percent` |
| `Deref<Target = [Piece]>` | gives `Index`, `iter`, `for`, and the `&Queue` to `&[Piece]` coercion |
| `AsRef<[Piece]>` | keeps the existing bound in `percent` unchanged |
| `iter() -> slice::Iter<'_, Piece>` | an explicit iterator for a non-`const` call site |
| `FromIterator<Piece>`, `Extend<Piece>` | `Segment::expand` collects into a queue |
| `Copy`, `Clone`, `Debug` | derived from the two fields |
| `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash` | hand-written over `pieces[..len]` |
| `Display` | replaces `Itertools::join("")` in two binaries |
| `Default` | matches the workspace style |

`Deref` is the item that keeps the change small. `&Queue` coerces to `&[Piece]`,
so `solve::reachable` and `percent::percent` keep their signatures and the
`solve` internals keep `queue[d]` and `queue.get(d)`.

`Deref` does not give range slicing. `&queue[1..]` and `&queue[..depth]` do not
compile through `Deref`, which is why `prefix` exists.

`Ord` and `Hash` are hand-written rather than derived. A derive would read the
whole array, including the unused tail, and would require an `Ord` bound on
`Piece` that the derive cannot avoid. Both compare only `pieces[..len]`.

### Ordering

`Ord` on `Queue` must return the same result as `Ord` on `Vec<Piece>` for the
same piece list. It compares element by element over the live prefix, then by
length. `Hash` hashes the same elements in the same order.

This is a requirement, not a preference. `Pattern::expand` returns a `BTreeSet`,
so the set order is the `Ord` order of the element. `jade_cli pattern expand`
prints that order. A different `Ord` changes the output of that command and
changes nothing else, so no solver test will report it.

### The pattern length check

The cap is fixed at 11, so a pattern that can name more than 11 pieces must fail
at parse time. `Vec` grew silently where a fixed queue cannot.

One new private function in `jade_pattern` computes the length of the longest
expansion of a segment:

```rust
fn max_len(segment: &Segment) -> usize
```

The rules, per variant:

| Variant | Longest expansion |
|---|---|
| `Single`, `Wildcard` | 1 |
| `Group`, `Filter` | the inner length |
| `Sequence` | the sum of the term lengths, saturating |
| `Bag` | the largest item length |
| `Except` | 1. Each item is an exclusion, so every result is one piece |
| `Choose`, `Permute` | `min(inner, n)` |
| `All` | the inner length |

`Pattern::parse` calls `max_len` on each top-level sequence and returns
`PatternError::InvalidPattern` when the sum exceeds `CAP`. The sum saturates, so
a long source cannot overflow.

The check runs in `parse`, not in `expand`. `expand` is also reachable through
`Segment::expand` and through `FromStr`, and a parser error carries a position.

`push_back` still asserts. The parser check covers patterns, but `Queue` is a
public type and other code builds queues directly.

### Data flow

`Pattern::parse` reads a source string and returns a `Pattern` or an error. It
now also rejects a source whose longest expansion is over `CAP`.

`Pattern::expand` returns `BTreeSet<Queue>`. Each `Segment::expand` builds a
queue by pushing one piece at a time. `Permute`, `Choose`, and `All` consume
`itertools` iterators of owned sequences, so each collects into a `Queue` and
relies on the `push_back` assert.

`percent` receives `IntoIterator<Item = Q>, Q: AsRef<[Piece]>`. A `Queue`
satisfies that bound, so `percent` takes queues without a signature change. It
reads `q.prefix(keep)` instead of `q[..n]`. It still encodes each queue into a
`u64` key and sorts the keys.

`check::parse_queue` builds a `Queue` and rejects a source longer than `CAP`.

### Control flow

No search algorithm changes. The oracle BFS keeps the `jade_nav` ring.
`Solver::search` and `percent::walk` keep their recursion and their `queue.get(d)`
reads. `perft::subtree` and `perft::run_parallel` keep their structure and swap
`&queue[1..]` for `prefix(1)` and `&queue[2..]` for `prefix(2)`.

The only new control flow is `max_len`, which runs once per parse.

### Dependencies

None added. None removed. `jade_pattern` already depends on `jade_core`, which
is what makes `jade_core::queue` the right home for the type.

### Design decisions

**The type lives in `jade_core`.** Every crate that needs a piece queue depends
on `jade_core`. Putting the type in `jade_nav` would give `jade_pattern` a new
dependency on `jade_nav` for no gain.

**Storage is `[Piece; CAP]` plus a `len` field, not the `u8` sentinel.** The
sentinel version is one byte smaller. It makes `is_empty` and `len` scan for a
zero byte, so both stay loops. The `len` field makes both one comparison and
keeps every method `const`. Twelve bytes is not the constraint here; the number
of scans is.

**The capacity is a fixed 11, not a const parameter.** A const parameter would
let a later call site choose a larger capacity without a second type. The 11
piece limit comes from the field size, not from the container, so a larger
capacity has no use today.

**Over-long patterns fail in the parser, not in `expand`.** A parser error names
the source and the limit. A panic in `expand` does not, and `expand` has no
source text.

**`percent` keeps its `u64` code key.** The key is what the parallel stages
sort and group. Sorting 8-byte keys is cheaper than sorting 12-byte queues, and
the key already carries the length, so the queue does not need to.

**`jade_nav::queue::Queue` is untouched.** It is a worklist of `Move` values.
Changing it is a separate concern.

## Alternatives

**Make `jade_nav::queue::Queue` generic and use it for both purposes.** One ring
type would serve the oracle and the piece queues. It was not selected because
`jade_pattern` would need a new dependency on `jade_nav`, and because a
4096-entry ring of `MaybeUninit<Piece>` is 4096 bytes per queue. `expand`
produces up to 5040 queues.

**Keep the `u8` sentinel storage.** One byte smaller per queue. It was not
selected because `len` and `is_empty` then scan, and those two calls sit in the
perft walk.

**Add a const capacity parameter.** It was not selected because the 11 piece
limit comes from the field size, not from the container.

**Reject an over-long pattern in `expand`.** It was not selected because
`expand` has no source text for the error, and because a panic is a worse
failure than an error.

**Leave `percent` taking `AsRef<[Piece]>`.** This was selected. The bound is
already container-agnostic, so no signature change is needed at that call site.

**Give `Queue` a `Drop` implementation.** It was not selected. `Copy` and `Drop`
cannot both apply to one type, and `Copy` is what lets a queue pass through
`&[Piece]`-based signatures without a `clone`.

## Risks

**Some patterns that parsed now fail.** A source whose longest expansion exceeds
11 pieces is rejected. Concretely: a 12-piece sequence, `[TTTTTTTT]!`,
`T*T*T*T*T*p11`. This is the intended trade, but it is a user-visible change to
`jade_cli` and to any fixture that uses such a pattern.

**A wrong `Ord` changes the output order of `jade_cli pattern expand`.** No
solver test reports it. It needs its own test against `Vec`.

**A hand-written `Ord` or `Hash` that reads past `len` compares unused tail
bytes.** Two queues with the same live prefix would then compare unequal. The
test in the Testing section covers this.

**`max_len` is a second description of the grammar.** It must agree with
`Segment::expand`. A new `Segment` variant that is not added to `max_len` would
let a too-long queue reach `push_back` and panic. The `Segment` match in
`max_len` is exhaustive, so the compiler reports a missing variant.

**`itertools` collectors must fit in a `Queue`.** `Permute`, `Choose`, and `All`
collect from an iterator of owned sequences. A sequence longer than `CAP` now
panics in `push_back` where `Vec` grew. The parser check rejects those sources
first, so the panic is not reachable from a parsed pattern.

**`&queue[a..]` does not compile through `Deref`.** Three sites use it. Each
becomes a `prefix` call. A missed site is a compile error, not a fault.

## Compatibility

**Existing behavior.** Node counts, percentages, and solver results do not
change. The `check` fixture must still cross-check `percent` against `reachable`.

**APIs.** These change:

* `jade_pattern::Pattern::expand` returns `BTreeSet<Queue>`.
* `jade_pattern::Segment::expand` returns `Vec<Queue>`.
* `jade_pattern::max_len` is new and private.
* `jade_perft::model::parse_queue` returns `Queue`.
* `jade_solve::setup::setups` returns `Vec<(Board, Queue)>`. The body is
  `todo!()`, so only the signature changes.
* `jade_cli::bin::check::Row::queue` holds a `Queue`.
* `jade_core::queue::Queue` changes its fields, its storage, and its method set.

These do not change:

* `jade_solve::solve::reachable` and `Solver::reachable` keep `&[Piece]`. A
  `&Queue` coerces at the call site.
* `jade_solve::percent::percent` keeps `Q: AsRef<[Piece]>`.
* Every public function in `jade_nav`.

**Data.** No file format changes. The `check` fixture format is unchanged.

**Configuration.** `CAP` is a compile-time constant. There is no runtime
setting.

**Users.** A pattern longer than 11 pieces now fails to parse. The error names
the source and the limit. No other user-visible change is expected, except the
`pattern expand` order if `Ord` is wrong, which the tests cover.

**Other components.** `jade_nav` is unaffected.

## Testing

### New tests in `jade_core::queue`

* Push order, and the empty queue.
* `push_back` panics at `CAP` and accepts the eleventh piece.
* `get`, `first`, and `prefix` at every boundary, including an empty queue and
  `prefix(0)`.
* `Display` prints the live prefix in order.
* `Deref` gives the right element and length.
* `Ord` and `Hash` agree with `Vec` on the same piece list. The table covers an
  empty list against a one-element list, two lists with the same prefix and
  different lengths, two lists of equal length that differ at the last piece,
  and equal lists.
* `Ord` and `Hash` ignore the unused tail. This is the test for the risk above.

### New tests in `jade_pattern`

* `max_len` for each `Segment` variant.
* A source of exactly 11 pieces is accepted. A source of 12 is rejected.
* `[TTTTTTTT]!` and `T*T*T*T*T*p11` are rejected. A 7-piece `*` bag is accepted.
* The existing `expand` expectations still hold.

### Golden output

Before any call site moves, record the output of `jade_cli pattern expand` for a
fixed set of patterns that stay under the limit. The output must be identical
afterwards, in the same order. This is the only check on `Ord`.

### Existing suites

These must pass unchanged:

* `cargo test --workspace`
* the `check` fixture, which cross-checks `percent` against `reachable` for
  every queue

### Performance

There is no `benches` target in the tree. The measurement is the printed nodes
per second from `jade_perft` and the wall time of `jade_cli percent`, taken
before and after the change.

## Implementation Steps

1. Rewrite `jade_core::queue` with `[Piece; CAP]` plus `len`, the full method
   set, the hand-written `Ord` and `Hash`, and its tests. This step alone
   changes no call site and breaks nothing, because the old type has no users.

2. Add `max_len` to `jade_pattern`, with tests. No behavior changes yet.

3. Move `jade_pattern` to `Queue`. Check the golden output from `pattern
   expand` against the recording from step 1's order of work.

4. Move `jade_perft`: `parse_queue`, then `run`, `subtree`, and `run_parallel`,
   then `depth_for`, `report`, and `report_compare`.

5. Move `jade_solve`: `solve` and `Solver::reachable`, then `percent` and
   `assert_matches_solver`, then the `setups` signature.

6. Move `jade_cli`: `main`, then `bin/check`.

7. Remove `MAX_QUEUE` from `jade_solve::solve` and `jade_cli::bin::check`. Both
   use `jade_core::queue::CAP`.

8. Run the full test suite, then take the performance numbers.

Steps 1 and 2 do not depend on each other, and neither depends on any later
step. They can be done in parallel.