# SIMD vector-of-boards (`Batch<const N>`)

## Objective

Add a batched board abstraction for the PC solver and setup finder.

The abstraction holds many 10x6 boards in parallel. Each board is one `u64`
lane (60 used bits). Operations run on all lanes at once.

The implementation must:

- Use `std::simd` (portable SIMD) on the existing nightly toolchain.
- Be portable across x86-64 and ARM64.
- Support two usage patterns:
  - Expand one base board into many candidates.
  - Hold many distinct boards and apply an operation to all of them.
- Keep the search structure undecided. The abstraction must serve both a
  beam search and a DFS with pruning.

## Current Behavior

- `Board(u64)` is the single 10x4 working-field board.
- All board operations are scalar `const fn`.
- `PMASK` is the single-band place-mask table.
  - Entry shape: `(u64 word, i8 x_bias)`.
  - Indexed by piece, canonical rotation, and anchor row `yr` in the band.
- `header.rs` constants:
  - `WIDTH = 10`.
  - `HEIGHT = 4` (working rows of the field).
  - `TLINES = 6` (rows per band).
  - `BOARD_BITS = WIDTH * TLINES = 60`.
- No search code exists yet.

## Proposed Behavior

- New type `Batch<const N>(Simd<u64, N>)` in a new module.
- One board per lane.
- Same primitive set as `Board`, operating on all lanes:
  - Collision check.
  - Place (XOR).
  - Line-clear detection.
  - Clear-shift (row compaction).
  - Empty check.
  - Highest-row extraction.
  - Survivor selection.
- Placement batching:
  - Splat a base board across lanes.
  - Build a column-`Simd` from one `PMASK` entry.
  - Shift per lane.
  - Mask off column overflow.
- Two construction paths:
  - `from_board`: multicast one base board.
  - `from_boards`: load distinct boards elementwise.
- Add `#![feature(portable_simd)]` to `main.rs`.

## Design

### Module layout

- New file `src/simd.rs`.
- `pub struct Batch<const N: usize>(Simd<u64, N>);`.
- `N` is a power of two: 4, 8, 16, or 32.
- LLVM lowers a wide `Simd` to multiple registers. No manual register
  splitting is needed.

### Lane primitives

Map each scalar `Board` kernel to an elementwise operator:

- Collision:
  - `(base & cand) == 0`.
  - Result type `Mask<i64, N>`.
- Place:
  - `base ^ cand`.
- Line-clear detection:
  - `(d & !ROW9_splat + ROW0_splat)`, then `d & summed & ROW9_splat`.
- Clear-shift:
  - Iterate `TLINES` times.
  - Use a per-lane variable shift by a masked lane count.
  - Lanes that already finished keep their value unchanged via `select`.
- Highest row:
  - `64 - leading_zeros` per lane.
- Empty check:
  - `reduce_all` against zero.

### Placement batching

- `PMASK[p][rc][yr].0` splatted across lanes.
- Column offset vector built from `Simd::from_array`.
- Shift per lane by the column vector.
- Mask overflow with a band-spanning column mask splat.

### Helper value handling

- Band constants are splatted per lane as needed.
- The `x_bias` value from `PMASK` is added into the per-lane shift.

## Scalar fixes required first

Two latent inconsistencies exist in the scalar kernels. They must be fixed
before batching, so the scalar path is a trustworthy reference.

1. `Board::clearshift` (src/board.rs).
   - Shifts by `HEIGHT` (4 bits).
   - Advances the clear-line mask by `HEIGHT`.
   - The bit-plane row stride is `WIDTH` (10 bits).
   - The shift distances must be `WIDTH`.
   - Current behavior miscompacts rows.

2. `row_span_mask`, `ROW0`, `ROW9` (src/header.rs).
   - Iterate `HEIGHT` rows (4).
   - A band spans `TLINES` rows (6).
   - Band-level operations need the 6-row span.
   - Otherwise column overflow masking does not cover the headroom.

## Alternatives

- `[u64; N]` arrays with LLVM auto-vectorization.
  - Not selected: `std::simd` gives explicit, portable control.
  - Retained as fallback if `portable_simd` fails on the nightly toolchain.
  - The public API stays identical; only the inner type changes.
- Raw `std::arch` intrinsics (`__m512i`).
  - Not selected: x86-64 only, most work to maintain.
- Full lane-lockstep search rewrite.
  - Not selected: the search structure is not decided yet.
  - The abstraction stays agnostic instead.

## Risks

- `portable_simd` is unstable.
  - Nightly can change the API.
  - Mitigation: the `[u64; N]` fallback with the same API.
- Per-lane variable shifts.
  - AVX2 (`vpsllvq`) and NEON (`vshlq_u64`) support them.
  - Pre-AVX2 x86 emulates them and is slower.
  - Correctness is unaffected.
- Lane divergence.
  - Batched phases must stay lane-uniform.
  - The recursion in a DFS stays scalar.
- Simultaneous clear counts differ per lane.
  - Handled by fixed iteration count and `select`.

## Compatibility

- Fully additive.
- `Board` and `PMASK` signatures do not change.
- `Batch` is a new public type.
- `main.rs` gains one feature attribute.
  - The crate is already nightly-only.
- The two scalar fixes change existing behavior.
  - No existing test depends on `clearshift`.
  - New tests lock the corrected behavior down.

## Testing

- Lane parity:
  - Random bases, masks, columns, and `PMASK` entries.
  - `Batch` result must equal the per-lane scalar `Board` result.
- Exhaustive small-board tests:
  - `line_clears` and `clearshift` against the `get`/`set` model.
- Placement batch:
  - Batch result must equal scalar `PMASK` application.
- Micro-benchmark:
  - Confirms batched execution is faster than a scalar loop.
  - Runs before any wider optimization.

## Implementation Steps

1. Enable `portable_simd` in `main.rs`.
   - Confirm the crate compiles on the current nightly.
2. Fix the scalar kernels.
   - `clearshift` shift distance.
   - Per-band spans for `row_span_mask`, `ROW0`, `ROW9`.
   - Add tests.
3. Add `src/simd.rs`.
   - `Batch<N>` type.
   - Splat and elementwise construction.
4. Add the lane primitives.
   - Collision.
   - Place.
   - Line-clear detection.
   - Clear-shift.
5. Add the placement batch builder.
   - Reads `PMASK`.
   - Shifts and masks per lane.
6. Add the height, empty, and survivor helpers.
7. Add lane-parity and exhaustive tests.
8. Run clippy and the test suite.
9. Run the micro-benchmark.
   - Record the result.