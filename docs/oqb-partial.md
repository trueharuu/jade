# Plan: Partial Scoring for OQB Trees

## Objective

Add partial scoring support to OQB decision trees. When enabled, a suffix that cannot reach a perfect clear can still contribute a scored prefix. Make such suffixes explicit in the tree by adding a `Partial` decision variant.

## Current Behavior

- `jade_score::Options` supports `partial: bool`.
- `score_suffix` in `crates/jade_oqb/src/lib.rs` calls `opts.best(...)` but discards whether the result is a perfect clear.
- `DecisionKind::Solve` stores a scored suffix with placements and events.
- `DecisionKind::Capped` stores only cost, cap, and leaves.
- A non-perfect-clear suffix is currently indistinguishable from a perfect-clear suffix when partial scoring is enabled.

## Proposed Behavior

- Add `partial: bool` to `DecisionSearch` and `oqb`.
- Pass `partial` through `score_suffix` into `jade_score::Options`.
- Add `DecisionKind::Partial` for scored prefixes that do not reach a perfect clear.
- Keep `DecisionKind::Solve` for perfect-clear suffixes.
- Keep `DecisionKind::Capped` for cases with no usable suffix.
- Mark partial suffixes clearly in indented tree output.

## Design

### Components Affected

- `crates/jade_oqb/src/decision.rs`
- `crates/jade_oqb/src/lib.rs`
- `crates/jade_oqb/src/search.rs`
- `crates/jade_oqb/src/serde.rs`
- `crates/jade_cli/src/main.rs`

### Data Flow

1. The caller selects partial scoring with a flag.
2. `oqb` passes the flag to `DecisionSearch`.
3. `tree_imperfect` calls `score_suffix` at terminal nodes.
4. `score_suffix` calls `jade_score::Options::best` with `partial` enabled or disabled.
5. If `best` returns a result, `score_suffix` reports whether its final event is a perfect clear.
6. `tree_imperfect` creates `Solve` for a perfect clear and `Partial` otherwise.
7. If `best` returns `None`, `tree_imperfect` creates `Capped`.

### Interfaces

#### `DecisionKind`

```rust
pub enum DecisionKind {
    Branch {
        field: Board,
        piece: Piece,
        mv: Move,
        children: [Option<Box<Decision>>; Piece::NB],
    },
    Solve {
        field: Board,
        queue: Queue,
        hold: Option<Piece>,
        b2b: bool,
        combo: Option<u32>,
        steps: List<Move, CAP>,
        events: List<Event, CAP>,
    },
    Partial {
        field: Board,
        queue: Queue,
        hold: Option<Piece>,
        b2b: bool,
        combo: Option<u32>,
        steps: List<Move, CAP>,
        events: List<Event, CAP>,
    },
    Capped,
}
```

#### `Decision` constructors

Add:

```rust
pub fn partial(
    cost: u64,
    cap: u64,
    leaves: u64,
    field: Board,
    queue: Queue,
    hold: Option<Piece>,
    b2b: bool,
    combo: Option<u32>,
    steps: List<Move, CAP>,
    events: List<Event, CAP>,
) -> Self
```

#### `score_suffix`

Return whether the suffix is a perfect clear:

```rust
(u64, Option<List<Move, CAP>>, List<Event, CAP>, bool)
```

The boolean is meaningful only when `steps` is `Some`. It is `true` when the final event has `pc == true`.

#### `DecisionSearch`

Add:

```rust
partial: bool
```

to `SearchContext` and pass it to `DecisionSearch::new`.

#### `oqb`

Add:

```rust
pub fn oqb(
    field: Board,
    queue: Queue,
    bag: Bag,
    two_l: bool,
    initial_hold: Option<Piece>,
    partial: bool,
) -> Decision
```

#### CLI

Add a `--partial` flag to the `Tree` command and pass it to `jade_oqb::oqb`.

### Important Design Decisions

- Use a separate `Partial` variant instead of a boolean on `Solve`. This makes the distinction explicit in pattern matches and output.
- Do not store partial prefixes in `Capped`. `Capped` remains a no-usable-suffix sentinel.
- Derive PC status from the final event rather than adding a separate scoring result type.
- Keep partial scoring disabled by default to preserve existing tree output and scores.

## Alternatives

### Add `pc: bool` to `Solve`

This is smaller, but it hides the PC/partial distinction inside a field. The `Partial` variant makes the distinction visible in every match site.

### Infer PC status from events in consumers

This avoids a schema change, but it is less reliable for empty suffixes and pushes inference logic into every consumer.

### Store partial prefixes in `Capped`

This would conflate "no concrete suffix" with "scored prefix" and would require `Capped` to carry the same data as `Solve`.

## Risks

- Adding an enum variant requires updating all exhaustive matches on `DecisionKind`.
- Partial suffixes may have shorter placement lists than perfect-clear suffixes.
- Consumers that count perfect clears must check the variant or final event, not just the presence of a `Solve` node.
- Enabling partial scoring can change tree costs and which branches appear useful.

## Compatibility

- Default behavior remains PC-only.
- Existing `Solve` and `Capped` semantics are unchanged.
- `Partial` only appears when partial scoring is enabled or when a result is not a perfect clear.
- Indented output gains a partial marker, but placement encoding remains the same.

## Testing

1. Build with `cargo build -p jade_oqb -p jade_cli`.
2. Run the existing tree test case with partial scoring disabled and confirm no `Partial` nodes.
3. Run the same case with partial scoring enabled and confirm partial suffixes appear as `Partial` nodes with placements and events.
4. Confirm perfect-clear suffixes remain `Solve`.
5. Confirm empty or dead-end suffixes remain `Capped`.

## Viewer

The OQB tree viewer must distinguish `Partial` nodes from `Solve` nodes.

- In `serde.rs`, render `Partial` as a five-field line:
  ```
  fumen\tleaves\tcost\tevents\tpartial
  ```
- In `tools/viewer/index.html`, parse five fields as `Partial`.
- Render `Partial` like `Solve`, but with a visible partial marker or badge.
- Update `docs/viewer.md` to document the five-field format.

## Implementation Steps

1. Add `Partial` to `DecisionKind` and `Decision::partial`.
2. Update `score_suffix` to return PC status.
3. Update `tree_imperfect` to construct `Solve` or `Partial`.
4. Update `serde.rs` to render `Partial` with a marker.
5. Thread `partial` through `DecisionSearch`, `oqb`, and the CLI `Tree` command.
6. Update the tree viewer parser and renderer for `Partial`.
7. Build and test.
8. Update documentation for the new flag and output marker.