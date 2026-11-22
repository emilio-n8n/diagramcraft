# DiagramCraft

Diagram editor for shapes, connectors and SVG export

**DiagramCraft is an independent, community open-source project.** It is not an
official product of, endorsed by, or affiliated with any third-party vendor it
may interoperate with.

Built on [CraftKit](https://github.com/emilio-n8n/craftkit), a small native Rust
desktop foundation.

## Features

- Native Rust desktop UI (egui/eframe), no web runtime.
- Shape palette: Rectangle, Ellipse, Note and Diamond; click the canvas to
  place a shape, snapped to a 10-unit grid.
- Select and move shapes with the mouse; Delete removes a shape and every
  connector attached to it.
- Connect tool: click two shapes to link them with a straight line anchored on
  their edges.
- Canvas pan by dragging the background and zoom with the scroll wheel.
- Readable, standalone SVG export of the whole diagram.
- Versioned JSON project files with atomic saves (format `diagramcraft`,
  schema 1).
- Bounded undo/redo through the shared document history; drags are a single
  undo step.
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

## Build and test

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Linux windowing development packages are installed by the CI workflow.

## Project layout

```text
src/main.rs       CLI and window startup
src/app.rs        UI, palette, canvas interactions and lifecycle
src/document.rs   application document model and geometry helpers
src/svg.rs        pure SVG renderer
```

The application depends on pinned CraftKit crates from Git. Keep the tag in
`Cargo.toml` and `Cargo.lock` consistent when upgrading the foundation.

## File format

Projects are JSON envelopes:

```json
{
  "format": "diagramcraft",
  "schema_version": 1,
  "app_version": "0.1.0",
  "document": { }
}
```

The application owns the `document` schema and its migrations. A newer
`schema_version` is rejected instead of being silently misread.

## Licence

Licensed under either of:

- Apache License, Version 2.0 (`LICENSE-APACHE`)
- MIT license (`LICENSE-MIT`)

at your option.

## Third-party notices

See [THIRD_PARTY.md](THIRD_PARTY.md).
