# Boolean solver `reachable` plan

This document describes the design and implementation of `reachable` in `crates/jade_solve/src/solve.rs`.

## Overview

`reachable(board: Board, queue: &[Piece], two_l: bool, saves: u8, hold: bool)` returns `true` if the goal can be reached from `board` using pieces from `queue` in order, optionally using hold. Goal can be reached at any prefix of the queue. The search terminates immediately on success.

## Goal

- Goal boards: exactly `PC_4`, or `{PC_2, PC_4}` if `two_l` is true.
- Checked on the child board after a placement (`clearshift` applied).
- Reached at **any point** → return true immediately if saves constraint is satisfied.
- If initial board is already a goal: return true only if saves constraint is satisfied for initial state.

```rust
fn is_goal(b: Board, two_l: bool) -> bool {
    b == PC_4 || (two_l && b == PC_2)
}
```

## Placements

A placement maps `field` to `(field | mv.mask()).clearshift()`.

`Board::clearshift()` does **not** delete cells. It moves completed rows to
the bottom of the board and keeps them as markers; the residue sits above
them. Cells only accumulate: every placement adds exactly four cells on
empty cells. The goal `PC_4` is therefore the state where all 40 play cells
are filled, and `PC_2` is the state where exactly rows 0-1 are filled.

The initial board is masked to the four play rows and `clearshift()`ed at
entry, so full rows in the input settle as markers and no cell outside the
play field can enter the search (this also keeps the visited key in range).

## Pruning

Only `pruned(board)` is used to decide whether to expand a node. Do not expand if `pruned(board)` is true. The check runs when a node is entered (the root included), after the visited check.

Definition (from `solve.rs`):
```rust
pub fn pruned(board: Board) -> bool {
    board.has_imbalanced_split() || board.has_isolated_cell()
}
```

Both predicates reject boards that can never be fully filled. Because cells only accumulate and `PC_2` needs rows 2-3 empty, a board that fails either check can reach neither goal: a sealed seam or a bounded column always involves cells in rows 2-3.

The goal test runs on the child before the node is entered. This matters for `PC_4`, which fails `pruned` (its seams are sealed) and is terminal: no piece fits on it. `PC_2` does not fail `pruned`, so a `PC_2` whose saves check fails is expanded like any other node and may continue toward `PC_4`.

No move ordering and no domination pruning are used.

## State

State is `(field: Board, idx: usize, hold: Option<Piece>)` where:
- `field`: board state after `clearshift` (`Board.0` as u64, bits below 2^40)
- `idx`: index into `queue` (number of queue elements consumed so far; next is `queue[idx]` if `< queue.len()`)
- `hold`: `Some(p)` if holding piece `p`, else `None`

**Visited key** must include all three. Pack into one `u64`: the 40 board
bits at the bottom, `hold_id` (`0-6` for `Some(p)`, `7` for `None`) at bits
40-42, and `idx` from bit 43. This requires the field to hold no cell above
row 3 (guaranteed by the entry mask) and `queue.len() < 2^21`. Use a fast
hash set (`rustc-hash`) of the key.

## Saves constraint

At goal time (after placement+clear produces a goal), let `h_goal = next_hold` from the action that produced this placement.

- If `saves_mask == 0`: ok.
- If `h_goal.is_some(p)`: ok if `(saves_mask & (1 << (p as u8))) != 0`.
- If `h_goal.is_none()`:
  - if `idx_state < queue.len()`: ok if `(saves_mask & (1 << (queue[idx_state] as u8))) != 0` (use first piece in queue if any)
  - else: false (queue empty, no hold)

Where `idx_state` is `act.next_idx`.

## Actions (hold semantics)

From state `(field, idx, hold)` with `hold_param = hold`:
- If `idx >= queue.len()`:
  - No queue pieces left. Only action: `active = h`, `next_idx = idx`, `next_hold = None` if `hold.is_some(h)`.
- Else:
  - Let `c = queue[idx]`.
  - (a) **No swap**: place `c`. Action `(active = c, next_idx = idx+1, next_hold = hold)`.
  - (b) **Swap** (if `hold_param` true): swap the current piece with the hold
      box, then place the swapped-in piece. The current piece moves into the
      hold box, so it is consumed from the queue and `next_idx` advances.
      - If `hold.is_some(hp)`: Action `(active = hp, next_idx = idx+1, next_hold = Some(c))`.
      - If `hold.is_none()`: hold first piece `c` (consumed into hold), place second piece `queue[idx+1]` (consumed into playfield) if it exists.  
          - If `idx+1 < queue.len()`: Action `(active = queue[idx+1], next_idx = idx+2, next_hold = Some(c))`.  
          - If `queue.len() == 1`: cannot perform swap action (no second piece) — skip.

`next_idx = idx + 1` (not `idx`) for a non-empty-hold swap is required for
soundness: the swapped-out piece `c` now lives in the hold box, so it must
also leave the queue. Keeping `idx` would let the search place `c` twice.
When `hp` and `c` are the same piece type, the swap result equals the direct
placement (a), so it is skipped to avoid a duplicate move generation.

Swap can be performed repeatedly later: each swap is one transition and is
always followed by exactly one placement, which matches modern Tetris hold
rules.

## Search (DFS)

```rust
fn dfs(&mut self, field: Board, idx: usize, hold: Option<Piece>) -> bool {
    // visited: contains -> false, then insert
    if !self.visited.insert(pack(field, idx, hold)) { return false; }
    if pruned(field) { return false; }

    let Some(c) = queue.get(idx) else {
        // queue exhausted: only the held piece remains (hold_param implies
        // hold can only be Some when hold_param was true)
        return hold.map_or(false, |h| self.place_all(field, h, idx, None));
    };

    // (a) place the current queue piece
    if self.place_all(field, c, idx + 1, hold) { return true; }
    if !hold_param { return false; }

    match hold {
        // (b) swap: place the held piece, the current piece moves into hold
        Some(hp) => {
            if hp == c { return false; } // identical to (a)
            self.place_all(field, hp, idx + 1, Some(c))
        }
        // (b) empty hold: hold the current piece, place the next one
        None => match queue.get(idx + 1) {
            Some(&next) => self.place_all(field, next, idx + 2, Some(c)),
            None => false,
        },
    }
}

fn place_all(&mut self, field: Board, active: Piece, next_idx: usize, next_hold: Option<Piece>) -> bool {
    for mv in moves(field, active).iter() {
        let child = (field | mv.mask()).clearshift();
        if is_goal(child, self.two_l)
            && saves_ok(next_hold, self.saves, self.queue, next_idx)
        {
            return true;
        }
        if self.dfs(child, next_idx, next_hold) { return true; }
    }
    false
}
```

Key properties: a node is expanded at most once (`visited`). The goal test runs on every child before it is entered, so a goal is detected even when `pruned` would reject the goal board. Return true immediately on goal with valid saves. No move ordering. No domination pruning.

## Entry point

```rust
#[must_use]
pub fn reachable(board: Board, queue: &[Piece], two_l: bool, saves: u8, hold: bool) -> bool {
    debug_assert!(queue.len() < 1 << 21, "level does not fit in the key");
    // keep only the play rows; full rows settle as markers at the bottom
    let board = (board & PC_4).clearshift();
    if is_goal(board, two_l) && saves_ok(None, saves, queue, 0) {
        return true;
    }
    if queue.is_empty() {
        return false;
    }
    let mut search = Search { queue, two_l, saves, hold_param: hold, visited: FxHashSet::default() };
    search.dfs(board, 0, None)
}
```

Notes:
- Initial hold is treated as `None` (per API). Answer 7 states equivalence to no initial hold + held piece first; search will find paths accordingly.
- An initial goal board with a failed saves check falls into the DFS. An initial `PC_4` is rejected by `pruned` (terminal: no piece fits), while an initial `PC_2` in two-line mode continues the search toward `PC_4`.
- `moves(field, piece)` uses `solve::moves(field, piece)` (which delegates to `jade_nav::fast::generate`).
- Residue never exceeds 10x4 playfield (ensured by `moves()`).
- Queue length up to 11 remains fast (target ~0.001s for length 7).