# OQB Decision Tree Viewer

## Objective

Create a standalone, isolated single-file web viewer at `tools/viewer/index.html` that loads an OQB indented decision tree and renders it as an interactive, collapsible tree.

## Current Behavior

Trees are produced by `Decision::to_indented_string` in `crates/jade_oqb/src/serde.rs`.

Format: tab-indented lines. Branch and Solve nodes begin with a fumen string, `leaves`, and `cost` separated by tabs. Branch nodes are followed by child nodes, each preceded by a single-letter piece label line. Solve nodes may include a comma-separated events string.

## Proposed Behavior

- User opens `tools/viewer/index.html` in a browser.
- User provides a tree file via drag-and-drop or a file picker.
- Viewer reads the file and parses tab-indented structure into a tree.
- Root node is collapsed initially.
- Clicking a node toggles expansion of its children.
- Each node renders:
  - Its board as an HTML table with exactly 10 columns and 4 rows.
  - Its average score as `cost / leaves`.
- Solve nodes render each placement in order as separate 10x4 boards, with the corresponding scoring event under each.

## Design

- **File**: `tools/viewer/index.html` only. All CSS and JS inline.
- **Dependencies**: `tetris-fumen` loaded from CDN.
- **Parser**:
  - Read input text.
  - Split into lines.
  - Determine depth by counting leading tabs.
  - Build tree recursively: a node line may be followed by child piece-label lines and child nodes.
  - Store fields: `fumen`, `leaves`, `cost`, `events`, `children`, `pieceLabel`.
- **Renderer**:
  - Recursive function renders node container.
  - Header shows average score.
  - Board table generated from fumen via tetris-fumen, dimensions 10x4.
  - Bottommost 4 rows are rendered (array indices 0-3).
  - For Solve nodes, iterate placements and render board + event.
  - Click handler toggles a `.collapsed` class on children container.
- **Styling**: black text on white background, minimal borders.
- **Colors**:
  - T: (1, 0.5, 1)
  - I: (0.5, 1, 1)
  - J: (0.5, 0.5, 1)
  - L: (1, 0.25, 0.5)
  - O: (1, 1, 0.5)
  - S: (0.5, 1, 0.5)
  - Z: (1, 0.5, 0.5)
  - Garbage/gray: (0.5, 0.5, 0.5)
  - Empty: transparent

## Alternatives Considered

- Rust/WASM viewer: rejected; user requested isolated HTML/JS.
- Canvas rendering: rejected; user requested HTML table.

## Risks

- `tetris-fumen` API details may require adjustment.
- Parsing must correctly associate piece labels with child nodes.

## Compatibility

- No changes to Rust code.
- No external build step.

## Testing

- Manual: open HTML, load sample `tree`, verify parse, collapse/expand, score display, Solve rendering.

## Implementation Steps

1. Create `tools/viewer/index.html`.
2. Add parser.
3. Add renderer using tetris-fumen.
4. Add minimal styling and interaction.
5. Test with sample tree.