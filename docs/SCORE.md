# Plan: Adapt `score` to Multi-Queue Patterns

## Objective

Make the `score` command accept a pattern that expands to any number of queues. Run `best` for each expanded queue, group the results, and print a table with columns `queues_covered scoring_events score`. Run the per-queue solves in parallel. Write whole-pattern statistics to stderr.

## Current Behavior

- `Command::Score` rejects patterns that expand to more than one queue.
- It calls `best` once, then prints the score, a fumen, and the comma-separated scoring events.
- A queue with no perfect clear causes the command to exit with status 1.

## Proposed Behavior

- Expand the pattern with `pattern.expand()`.
- Solve each queue in parallel with `rayon`.
- Group successful results by their event sequence and total score. `queues_covered` is the group size.
- Represent queues with no perfect clear as one additional row with an empty scoring-events field and score `0`.
- Print one stdout row per group in the order `queues_covered scoring_events score`.
- Keep one scoring event per placed piece. Do not collapse chained zero-line events. Do not coalesce sequences that differ only in where zero-line events occur.
- Print to stderr: total queues, solved queues, and average score over all expanded queues. Failed queues contribute `0` to the average.

## Current Relevant Code

- `crates/jade_cli/src/main.rs`, `Command::Score` arm.
- `crates/jade_solve/src/best.rs`, `best`.
- `crates/jade_core/src/score.rs`, `Event` and `Spin`.

## Design

### Components Affected

- `crates/jade_cli/src/main.rs`: `Command::Score` arm.
- `crates/jade_core/src/score.rs`: optional `Hash` derives on `Spin` and `Event` if typed grouping is used.

### Data Flow

1. Parse the pattern and expand it into a `BTreeSet<Queue>`.
2. Parse the starting board.
3. Run `best(board, queue.as_slice(), false, save, *hold)` for each queue in parallel.
4. Map a failed result to the key `(empty event list, 0)`.
5. Map a successful result to `(events, score)`.
6. Count queues per key.
7. Sort rows by `queues_covered` descending, then score descending, then event string ascending.
8. Print rows to stdout.
9. Compute and print statistics to stderr.

### Interfaces

- `best` keeps its current signature.
- Grouping key can be the formatted event string plus score, or `(Vec<Event>, u32)` if `Hash` is derived for `Event` and `Spin`.

### Parallelism

- Use `rayon` to map over expanded queues.
- Grouping remains sequential because it is cheap compared with the search.

### Statistics

- Total queues: number of expanded queues.
- Solved queues: number of queues for which `best` returns `Some`.
- Average score: sum of all queue scores divided by total queues. Failed queues contribute `0`.

## Alternatives

### Coalesce sequences with the same relevant events

- Not selected. Zero-line placements affect combo and B2B state, so sequences that differ in zero placement position are not equivalent.

### Collapse chained zero-line events for display

- Not selected. N pieces should produce N scoring events for correctness.

### Omit failed queues from the table

- Not selected. The table should sum to the total queue count, so failed queues are represented by an empty-events, zero-score row.

## Risks

- Runtime scales with the number of expanded queues. Parallelism reduces wall time but does not reduce total search work.
- `best` keeps only the first maximum-scoring path on ties, so one queue contributes one group.
- Empty scoring-events fields make rows visually ambiguous, but the three-column order remains fixed.

## Compatibility

- CLI stdout changes from single-queue score, fumen, and events to the grouped table.
- Statistics go to stderr and do not affect stdout parsing.
- No changes to `best` or `Event`.

## Testing

- Build with `cargo build`.
- Run `score` on a single-queue pattern and confirm one row with `queues_covered` equal to 1.
- Run `score` on a multi-queue pattern and confirm that `queues_covered` values sum to the number of expanded queues.
- Run `score` on a pattern containing unsolvable queues and confirm the empty-events, zero-score row appears.

## Implementation Steps

1. Remove the single-queue restriction in the `Command::Score` arm.
2. Add the `rayon` import.
3. Run `best` for each expanded queue in parallel.
4. Map failed results to the empty-events, zero-score key.
5. Group, sort, and print rows.
6. Compute and print statistics to stderr.
7. Build and run smoke tests.