# Save Mode — Change Plan

## Objective

Add a `--save <pattern>` option to `jade_cli solve` and `jade_cli percent`.

A query with `--save` succeeds only when both conditions hold at the same
moment:

1. The placement sequence fills the goal field.
2. The hold box holds a piece whose type is in the pattern's expanded set.

A query without `--save` keeps its current meaning and its current answers.

## Current Behavior

Facts, read from the source in this repository.

- `jade_solve::reachable` and `jade_solve::reachable_2l` answer one question:
  can any legal placement sequence fill the goal field. They take
  `(board, queue)`. They have no save parameter.
- The search is a depth-first search over states `(level, board, hold)`, where
  `level` is the index of the next queue piece
  (`crates/jade_solve/src/solve.rs:139`).
- A goal test at the top of `visit` accepts a state whose board equals
  `Board::lines(4)`, and also `Board::lines(2)` when the two-line mode is on
  (`solve.rs:185`). The test runs before every other check, so it applies to
  the start state and to every child state.
- The hold of a state is the hold box content *after* the transition that
  produced the state. Placing the held piece empties the hold
  (`solve.rs:248`).
- The hold of a state at a deeper level is always either the current hold or a
  piece from `queue[level..]`. A swap is always followed by a placement, so
  `expand` only ever sets the hold to the current hold, to `queue[i]`, or to
  `None` (`solve.rs:242`).
- The full 4-row field is a fixed point of `clearshift`, and the two-line field
  is a fixed point too. No piece fits on the full field, so a full field has no
  outgoing transition.
- `jade_cli` declares `save: Option<Pattern>` on both `Solve` and `Percent`
  (`crates/jade_cli/src/main.rs:33` and `main.rs:57`). Both fields are bound
  and never read. The build emits an `unused_variables` warning for each.
- `docs/design.md:222` records an earlier `save` design. It defines success as
  "the board is empty and the piece in hold has a type in the save set", and it
  requires every expansion of the pattern to be one piece long. The
  empty-board test does not apply here. `docs/boolean-solver.md:265` records
  that the `save` definition must be re-derived against the `clearshift` model
  before it is implemented. This plan is that re-derivation.

## Proposed Behavior

`jade_solve` gains two public functions:

```
pub fn reachable_save(board: Board, queue: &[Piece], save: &[Piece]) -> bool
pub fn reachable_save_2l(board: Board, queue: &[Piece], save: &[Piece]) -> bool
```

`save` is the set of piece types that meet the save requirement. The result is
`true` when the goal field is reachable and some path fills it while hold holds
a piece whose type is in `save`.

`jade_cli` passes the `--save` pattern through to these functions. Both
subcommands accept `--save`, and `--2l` composes with it in both.

`--save` patterns that do not expand to single pieces are an error. The command
exits with code 2 and prints the reason. A pattern that expands to nothing is
also an error, because it can never match a hold.

An empty hold never meets a save requirement. A save means the piece is kept in
hold, so a state with an empty hold box has kept nothing.

## Design

### Save set representation

The hold is `Option<Piece>`, so a save set must cover the empty hold as well as
the seven piece types. One `u8` holds the whole set:

- bit 0: the empty hold
- bit `p + 1`: `Piece::p`

The empty-hold bit is only ever set by the constant `SAVE_ANY`, which is
`0xFF` and means "no save required". `reachable` and `reachable_2l` pass
`SAVE_ANY`, so the hold test in the goal is always true for them and the
existing answers do not change.

A save set built from `save` never sets bit 0, so a set from pieces alone never
accepts an empty hold. An empty `save` slice gives the set `0x00`, which
matches no hold value, so every search on it fails. This is the correct answer
for that input, and the CLI rejects the input earlier with a clear message.

One function, `hold_code`, maps a hold value to its index. The key packer, the
goal test, and the set builder all use it, so the numbering has one definition.

### Goal test

The goal test becomes two cases:

```
if b == PC_4                          -> return holds(saves, hold)
if two_l && b == PC_2 && holds(...)   -> return true
```

The first case is a decision, not a test. The full field has no room for a
piece, so no transition leaves it, so the hold can never change. A full field
therefore answers on its own, and the search does not expand it. Without this,
a full field with a hold that fails the save test would run the cell checks,
spend a table slot, run the fill heuristics, and call the move generator before
it learns the state is dead.

The second case stays a test. The two-line field has two empty rows above it,
so a path may continue from it toward the full field, even when the hold at the
two-line field does not meet the save.

Without `--save`, `holds` is true for every hold, so both cases reduce to the
old `goals.contains(&b)` and return true. The answers of `reachable` and
`reachable_2l` do not change.

### Prune

`docs/design.md:232` asks for a prune: drop a branch when no piece type in the
save set is left in hold or in the remaining queue. The same rule is derived
here from the hold transition rule:

The hold of any state in the subtree below `(level, b, hold)` is the current
hold or a piece from `queue[level..]`, and no other value. If none of those
values is in the save set, no state in the subtree can be a successful goal,
because every goal needs a savable hold. So the state is dead.

The test is one load, one OR, one AND, one compare:

```
if saves != SAVE_ANY && (hold_bit(hold) | suffix[level]) & saves == 0 -> dead
```

`suffix[level]` is a precomputed `u8` set of the hold codes of the pieces in
`queue[level..]`. It is built once per query, in `O(queue.len())`. It has
`queue.len() + 2` entries, because the held piece can still be placed after the
queue ends, which reaches level `queue.len() + 1`.

The set is only built when a save is required. `SAVE_ANY` queries allocate
nothing, as before.

The prune is the reason a save query is usually faster than a plain one, not
slower. A query with `--save I` and no `I` in the queue dies at the start
state. A query where the only `I` is late in the queue loses most of the tree
before the search reaches it.

### Interfaces

- `jade_solve::reachable`, `reachable_2l`, `is_unfillable`, `parse_fumen` keep
  their current signatures and answers.
- `jade_solve::reachable_save` and `reachable_save_2l` are added.
- `solve_inner` gains a `saves: u8` parameter. The `goals: [Board; 2]` and
  `n_goals: usize` fields of `Search` are removed and replaced by the `PC_4` and
  `PC_2` constants, which the two goal cases and the cell bound both need.
- `jade_cli` gains one helper, `parse_save`, and one more parameter on
  `has_solve`.

## Alternatives

- **Add a `save: Option<&[Piece]>` parameter to `reachable` and
  `reachable_2l`.** Rejected. It changes two published signatures to express
  what two new functions express, and it puts an `Option` test on the path of
  every query that has no save.
- **Run a second search that only looks for the save.** Rejected. It repeats the
  whole tree search for each piece type in the save set. The set is one `u8`
  test, so the whole set is checked at no extra cost.
- **Encode the save set in the packed key.** Rejected. The key has 3 spare bits
  and they hold the level. The set is a property of the query, not of a state,
  so it belongs in the `Search`, not in the key.
- **Treat a two-line field that fails the save test as a dead state.** Rejected.
  It is wrong. A path through the two-line field can still reach the full field
  with a savable hold.
- **Let `--save` accept multi-piece expansions and match a sequence.** Rejected
  for this version. A goal leaves exactly one piece in hold, so a longer
  sequence can never match. `docs/design.md:226` already requires the pattern to
  be rejected in that case, rather than silently ignored.
- **Let an empty hold match a wildcard save such as `--save '*'`.** Rejected. A
  save means the piece is kept, so a state with an empty hold box has kept
  nothing. This also means `reachable_save` with all seven piece types is not
  the same query as `reachable`. The two functions differ only when the start
  field is already the goal field.

## Risks

- **The goal test can now return `false` for a full field, where it used to
  return `true`.** This is the intended change, and only when a save is
  required. `SAVE_ANY` is passed by both existing functions, and `holds` is
  true for every hold, so their answers do not change.
- **A wrong prune loses a solution.** The prune depends on one fact: the hold
  of a deeper state is the current hold or a piece from `queue[level..]`. That
  fact is read from `expand` and is not currently asserted anywhere. If a
  future change adds a transition that sets the hold to another value, the
  prune becomes unsound. The comment on the prune states this fact and names
  `expand` as its source.
- **A save pattern with many expansions.** The set has at most eight bits, so
  pattern size does not affect search cost. `Pattern::expand` itself can be
  large for a pattern with wildcards and permutations, but that cost already
  exists for the queue pattern.
- **`Percent` runs one search per queue.** The `suffix` set adds one
  allocation per search when a save is required. It is one small `Vec` against a
  tree search.

## Compatibility

- `reachable`, `reachable_2l`, `is_unfillable`, and `parse_fumen` keep their
  signatures and their answers.
- `Solve` and `Percent` gain a working `--save`. The flag already parses today,
  so no command line changes from invalid to valid.
- Without `--save`, both subcommands behave exactly as before, including the
  exit codes and the printed fraction.
- A bad `--save` pattern exits with code 2. The code is already used for a bad
  `--field` fumen, so a bad argument is distinguishable from a `no` answer,
  which exits with code 1.
- No data, configuration, or other crate changes. `jade_solve` does not gain a
  dependency; it keeps the same `Cargo.toml`.

## Testing

Behavior checks are run by the project owner. The cases:

1. Without `--save`, the answers of `solve` and `percent` on a set of queues
   are unchanged. Compare the exit codes and fractions against the same
   commands on the previous commit.
2. `solve IIIIIIIIII` is `true`. `solve IIIIIIIIII --save I` is `false`,
   because the queue has no `I` to hold.
3. A queue with a known solve, run with `--save` for a type in the queue and
   for a type not in the queue. The two answers may differ, and a difference
   must be explainable by whether the piece can be in hold when the field
   fills.
4. `solve <queue> --save '[TI]'` accepts both `T` and `I`. The answer must be
   the union of the answers for `--save T` and `--save I`.
5. `solve <queue> --2l --save <type>` composes. The save test applies to the
   two-line goal and to the full goal.
6. `solve <queue> --save IJ` exits with code 2 and a message.
7. `solve <queue> --save 'I{T>2}'`, an empty expansion, exits with code 2.
8. `percent --save <type>` prints a fraction that is not larger than the same
   query without `--save`.
9. A start field that is already the goal field. `solve <queue> --field <fumen>
   ` is `true`, and the same command with `--save <type>` is `false`, because
   the hold is empty at the start.

Build checks run here: `cargo build --workspace`, `cargo clippy --workspace
--all-targets`, and `cargo fmt --check`. The workspace sets
`missing_const_for_fn = deny` and `perf = deny`, so the new helpers are
`const fn` and the new bounds check on `suffix` is reviewed before it lands.

## Deviations

- The plan kept `cargo fmt --all` as a step. It reformatted files outside this
  change, in `jade_nav`, `jade_pattern`, `jade_perft`, and `jade_solve::parse`.
  Those reformats were reverted. The diff is now `jade_solve::solve`,
  `jade_solve` re-exports, and `jade_cli` only.

## Implementation Steps

1. Add the `PC_4` and `PC_2` field constants, `hold_code`, `hold_bit`, `holds`,
   `SAVE_ANY`, and `save_set` to `solve.rs`.
2. Change `pack` to use `hold_code`.
3. Replace `goals` and `n_goals` in `Search` with `two_l`, `saves`, and
   `suffix`. Rewrite the goal test in `visit` as the two cases above.
4. Add the save prune to `visit`, after the other cheap checks and before the
   table insert.
5. Build `suffix` in `solve_inner` and pass it to `Search`. Keep the start-state
   goal test in `solve_inner`, and add the `holds(saves, None)` term to it.
6. Add `reachable_save` and `reachable_save_2l`, and re-export them from
   `lib.rs`.
7. Add `parse_save` and the `save` parameter to `has_solve` in `main.rs`. Pass
   the result through in `Solve` and `Percent`.
8. Run `cargo fmt`, `cargo build --workspace`, and `cargo clippy --workspace
   --all-targets`.
9. Compare the implementation against this plan and record the deviations.
10. Hand the test list above to the project owner.
