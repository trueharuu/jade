# Change Plan: Exact Fast/Parity Move Generation

## Objective

Make the fast move generator in `crates/jade_nav/src/fast.rs`
produce exactly the locked placements that the oracle
`crates/jade_nav/src/oracle.rs` produces, on every board.
The design contract from `docs/phase-movegen.md` states that the
oracle must check the fast path and the two must agree.

The current fast generator diverges on random boards.
The divergence has two directions:

- Undercounts: rest positions that the oracle reaches but fast misses.
- Overcounts: rest positions that fast seeds but the oracle cannot reach.

## Current Behavior

The fast generator seeds each rotation plane with "sky drops":

`search[r] = (!surface & usable[r]) | tuck(...)`

A sky drop assumes a straight vertical fall at any column whose
origins are free. This over-approximates reachability: a column can be
empty and still be unreachable from the spawn column. Example: board
`0xe020c1018c90148`, Z piece. Columns 0 to 2 are empty in several rows,
but a wall at row 4 column 3 blocks the path from spawn column 4.
The oracle never visits columns 0 to 2. Fast emits `(1, 0)` North for
Z on this board; the oracle cannot reach it.

The fixed point also uses a lateral rule that is stricter than the
oracle:

`temp_all = shifted(-1,0) & shifted(1,0) | shifted(0,-1)`

This requires both horizontal neighbors or a reachable cell above the
target. The oracle moves laterally when the target origin fits,
whatever the other neighbor. The fast rule omits some rest positions.

The loop terminates on `done`/`remaining` bookkeeping that assumes
`search` must cover every candidate (`missing` becomes empty).
The oracle can legitimately leave candidates unmatched. With stricter
seeds the loop can exit while `missing` is still full, and the result
loses those placements silently.

Known fixes already applied:

- `LANDED_OK` in `jade_core/src/header.rs` now uses rows
  `-min_y ..= 3 - max_y`.
- `horizontal_tuck` in `crates/jade_nav/src/op.rs` masks before shifts.
- Spawn failure returns an empty move set.
- Kicks target the canonical rotation's `usable` plane.

## Proposed Behavior

The fast generator mirrors the oracle operations exactly, in plane
form, from a single spawn origin. Reachability is the monotone fixed
point of these operations:

- Fall: a reachable origin at `(x, y)` marks `(x, y-1)` when it fits.
  This equals the oracle soft drop.
- Lateral: a reachable origin at `(x, y)` marks `(x+1, y)` and
  `(x-1, y)` when the target fits. One side suffices, as in the oracle.
- Kick: a reachable origin at rotation `R` marks kick targets of
  cw, ccw, and 180 degree rotation when the target fits.

The seed is one cell: the spawn origin `(4, sy)` in the spawn
rotation. Everything else follows from the fixed point.

Termination: the outer loop repeats the full pass over all four
rotation slots. A pass runs every closure and every kick once. The
loop stops when a pass adds no new cell to any search plane. The
universe is finite (60 bits), the operations are monotone, so this
terminates.

Emission: after the fixed point, `missing[r] = cands[r] & !search[rc]`
where `rc` is the canonical rotation of slot `r`. A candidate that the
fixed point never reaches stays missing. Emission is
`cands & !missing`, so unreachable candidates are correctly excluded.

## Design

### Components affected

- `crates/jade_nav/src/fast.rs`: rewrite `generate`, `process_rot`,
  and the kick call sites. Remove the `done`/`remaining` machinery.
- `crates/jade_nav/src/oracle.rs`: remove the temporary trace print.
- `crates/jade_nav/examples/parity.rs`: keep as regression harness.
- `crates/jade_nav/examples/ztrace.rs`: keep for spot checks, delete
  at the end.
- `crates/jade_nav/src/fast.rs`: remove temporary `JADE_TRACE` prints.

### Data flow

1. `usable = usable_map(board)` per rotation slot.
   Rows 0 to 5. Row 0 is the bottom.
2. `cands[r] = landable_map & LANDED_OK`.
   Rest origins in the legal lock rows.
3. `search[canonical(North)] = {x:4, y:sy}`.
   `sy = 5 - P.h_spawn()`.
4. Fixed point:
   - For each slot `R` in 0..4, close `search[R]` under
     fall and lateral within `usable[rc]`.
   - For each slot `R` in 0..4 and each direction `D` in 0..3,
     add kick targets into `search[r1]`.
   - Compare the combined planes before and after a full pass.
   - Stop when unchanged.
5. `missing[r] = cands[r] & !search[rc]`.
6. Emit `mask[r] = cands[r] & !missing[r]`.

### Control flow

```
generate(P, board):
    if spawn origin not usable: return empty
    search = empty planes
    search[canon(N)] = spawn cell
    loop:
        snapshot = search[0]|search[1]|search[2]|search[3]
        for R in 0..4: close(R)        # fall + lateral
        for R in 0..4: for D in 0..3: kick(R, D)
        if combined == snapshot: break
    missing[r] = cands[r] & !search[rc(r)]
    emit per-rotation mask
```

`close(R)`:

```
loop:
    g = search[R]
    new = (g.shifted(1,0) | g.shifted(-1,0) | g.shifted(0,-1))
          & !search[R] & usable[rc]
    if !new.any(): break
    search[R] |= new
```

`kick(R, D)`:

```
r1 = (R + D-table) & 3          # D 0=cw, 1=ccw, 2=flip
r1c = canonical(r1)
off = canonical_offset(R) - canonical_offset(r1)
for each kick (kx, ky) in kt[R][r1]:
    cand = search[R].shifted(kx+off.x, ky+off.y) & usable[r1c]
    ...
    search[r1] |= cand
    (optionally drop already-collected cells)
```

### Interfaces

`generate::<P>(board) -> Moves` keeps its signature.
`Moves` and emission stay unchanged. The oracle is untouched.

### Dependencies

`op::usable_map`, `op::landable_map`, `header::LANDED_OK`,
`data::KICKS_TJLSZ`, `data::KICKS_I`, `Piece::groups/search_size/
canonical_rotation/canonical_offset/h_spawn`.

### Important design decisions

- The seed is the spawn cell only. The sky-drop seed is removed
  because it is the source of the overcounts.
- The lateral closure uses single-side expansion. A cell whose only
  reachable neighbor is on one side is reachable, matching the oracle.
- The two empty mirrors for group-2 pieces (I, S, Z), rotations 2 and
  3, are kick targets with canonical-plane indexing. This replaces the
  old group-2 mirroring.
- Termination is "no growth in one full pass". Simple, correct, and
  independent of `missing`. The old `done`/`remaining` invariant that
  `search` covers all candidates is dropped.
- Kicks take every valid target. The oracle stops at the first valid
  kick; the plane form cannot stop per cell. Taking all valid targets
  adds only reachable cells, so completeness and soundness hold.

## Alternatives

1. Restrict the sky-drop seed to columns reachable at the spawn row.
   Rejected. A reachable row-`sy` cell requires the spawn-row orbit,
   and columns reached only by lateral move at a lower row would be
   missed. The fix has the same cost as the fixed point and is still
   approximate.
2. Keep the sky seed and the both-neighbors rule, and accept
   divergence. Rejected. The design contract requires parity.
3. Make `fast` call the oracle. Rejected. `fast` exists for the
   lane-batched search; the oracle is scalar BFS.

## Risks

- Fixed-point iteration count grows on boards with large open areas.
  The plane is 60 bits, so the bound is small and constant per board.
- Emission may lose placements if the fixed point stops early.
  Mitigation: parity fuzz with 20000 random boards must report zero
  mismatches.
- Kick indexing bugs for group-2 pieces. Mitigation: the parity fuzz
  covers I, S, Z with random boards.

## Compatibility

Behavior contracts that are preserved:

- `Moves` layout and emission.
- Lock rule via `LANDED_OK`.
- The spawn-failure empty set.
- The lock filter rows 0 to 3.
- No spins. 180 rotations stay on.
- No rule configuration.

Anything that must not change for callers stays unchanged. The change
is internal to `generate`.

## Testing

1. Structured suite in `examples/parity.rs`:
   empty, row0, wall, row2, well, bump boards, all 7 pieces.
   Must match.
2. Board `0xe020c1018c90148`, Z piece: `fast == oracle`.
3. Fuzz: 20000 random 6-row boards, all pieces, set equality and
   counts. Expect zero mismatches.
4. Spot checks with `examples/ztrace.rs` and `JADE_TRACE` during the
   work, removed before the final report.
5. Release build runs the examples for panic-free compile.

## Implementation Steps

1. Rewrite `generate` in `fast.rs`: spawn-only seed, single-side
   lateral closure, kick passes, growth-based termination.
2. Remove the `done`/`remaining` machinery and the old sky seeding.
3. Keep `finish`; change its `missing` accounting to the fixed point.
4. Run the structured boards and the Z trace. Fix discrepancies.
5. Run the 20000-board fuzz. Drive mismatches to zero.
6. Remove `JADE_TRACE` prints and the oracle probe.
7. Delete `examples/ztrace.rs`.
8. Run `cargo clippy` and the workspace tests.