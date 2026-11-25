# Plan: Partial Scoring for Non-PC Queues

## Objective

Add partial scoring to `jade_score`. When a queue cannot reach a perfect clear, return the highest-scoring legal prefix that the search found. This improves scores for non-100% fields.

## Current Behavior

- `search` records a result only on a goal board with a valid leftover.
- A branch that cannot reach a goal is discarded.
- `place` skips moves that cannot still reach a goal.
- A queue with no perfect clear returns `None`.

## Proposed Behavior

- Record a result for a dead-end node when at least one piece has been placed.
- Use the accumulated score of that prefix as a candidate solution.
- Keep perfect-clear paths preferred when they exist.
- Return `None` only when no piece can be placed.

## Design

### Components Affected

- `crates/jade_score/src/search.rs`: `Searcher::search`, `Searcher::place`.
- `crates/jade_score/src/score.rs`: no structural change.

### Data Flow

1. `search` checks for a goal board first. On success it records `node.acc`.
2. If the board is full but not a goal, or is pruned, and `node.ply > 0`, record `node.acc`.
3. `place` generates PC-capable candidates.
4. If no candidate is produced for any take, and `node.ply > 0`, record `node.acc`.
5. Otherwise recurse into candidates.

### Interfaces

- `place` returns `bool`: `true` when it pushed at least one candidate.
- `BestResult`, `Options`, and `Options::best` keep their current signatures.

### Important Design Decisions

- Keep PC-only pruning. This avoids a large increase in search space.
- Compute `pc` as `is_goal_board(next, two_l) && save_ok(...)`. This prevents a save-incompatible full board from receiving the PC bonus in a partial path.

### Risks

- More `record` calls occur at dead ends. The search space is unchanged.
- Equal-score partial paths keep the first one found, as before.

### Compatibility

- `jade_cli::Command::Score` needs no change.
- Queues that previously returned `None` may now return `Some`.
- OQB behavior is unchanged unless it relies on `None` for non-PC suffixes.

### Testing

1. Build with `cargo build -p jade_score`.
2. Run `cargo test -p jade_score`.
3. Run `score` on a non-PC queue and confirm a partial row appears.
4. Run `score` on a PC queue and confirm the full PC score remains.