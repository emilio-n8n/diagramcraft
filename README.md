# DiagramCraft

Diagram editor for shapes, connectors and SVG/PDF export

**DiagramCraft is an independent, community open-source project.** It is not an
official product of, endorsed by, or affiliated with any third-party vendor it
may interoperate with.

Built on [CraftKit](https://github.com/emilio-n8n/craftkit), a small native Rust
desktop foundation.

## Features

- Native Rust desktop UI (egui/eframe), no web runtime.
- Shape palette: Rectangle, Ellipse, Note and Diamond; click the canvas to
  place a shape, snapped to a 10-unit grid.
- Edit labels in place with a double-click (or F2); one undo step per edit.
- Select and move shapes with the mouse; drag the eight resize grips to resize.
  Every drag is a single undo step.
- Shift+click for multiple selection, grouped move/delete, copy/paste/duplicate,
  align (six modes) and distribute (two axes).
- Connect tool with straight or orthogonal routing; orthogonal connectors have
  draggable waypoints (double-click a link to add one). Links can be selected
  and deleted on their own.
- Canvas pan by dragging the background, zoom with the scroll wheel and
  zoom-to-fit (F). The grid and snapping can be toggled in the left panel.
- Readable, standalone SVG export and vector PDF export of the whole diagram.
- Versioned JSON project files with atomic saves (format `diagramcraft`,
  schema 2; schema-1 files are migrated on open).
- Bounded undo/redo through the shared document history.
- Background jobs with progress and cooperative cancellation.
- Recent files and persisted preferences.
- Offline-first: no account, no telemetry, no network required.

## Run

```bash
cargo run --release -- --sample
cargo run --release -- --open project.json
```

Without arguments the application opens an empty document.

- `--sample` opens the built-in demonstration document.
- `--open <file>` opens a project.
- `--version` prints the version.
- `--help` prints usage.

## Shortcuts

| Action | Shortcut |
| --- | --- |
| Save / Save As | Ctrl+S / Ctrl+Shift+S |
| Open / New | Ctrl+O / Ctrl+N |
| Undo / Redo | Ctrl+Z / Ctrl+Y |
| Copy / Paste / Duplicate | Ctrl+C / Ctrl+V / Ctrl+D |
| Select all | Ctrl+A |
| Edit selected label | F2 or Enter |
| Delete selection | Delete / Backspace |
| Zoom to fit | F |
| Export SVG / PDF | Ctrl+Shift+E / toolbar |

## Build and test

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
python3 tools/launcher_check.py .
```

Linux windowing development packages are installed by the CI workflow.

## Project layout

```text
src/main.rs       CLI and window startup
src/app.rs        UI, palette, canvas interactions and lifecycle
src/document.rs   application document model and domain operations
src/geometry.rs   pure resize/route/align/fit geometry
src/svg.rs        pure SVG renderer
src/pdf.rs        pure vector PDF renderer
```

The application depends on pinned CraftKit crates from Git. Keep the tag in
`Cargo.toml` and `Cargo.lock` consistent when upgrading the foundation.

## File format

Projects are JSON envelopes:

```json
{
  "format": "diagramcraft",
  "schema_version": 2,
  "app_version": "0.2.0",
  "document": { }
}
```

The application owns the `document` schema and its migrations. A newer
`schema_version` is rejected instead of being silently misread. Connectors in
schema 2 add optional `routing` and `waypoints` fields; older files default to
straight routing with no waypoints.

## Licence

Licensed under either of:

- Apache License, Version 2.0 (`LICENSE-APACHE`)
- MIT license (`LICENSE-MIT`)

at your option.

## Third-party notices

See [THIRD_PARTY.md](THIRD_PARTY.md).
