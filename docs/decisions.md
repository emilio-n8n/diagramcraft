# Decisions — DiagramCraft

Short architecture decision records. Newest first.

## ADR-001 — Built on CraftKit

Date: 2026-11-22

Decision: use the pinned CraftKit crates for the native shell, project
lifecycle, history and background jobs.

Alternatives: build the desktop foundation from scratch; use another UI
framework.

Reasons: consistent behaviour across the portfolio, fewer duplicated bugs,
shared release and testing conventions.

Consequences: CraftKit upgrades are explicit tag changes; domain code stays in
this repository.

## ADR-002 — Initial document model

Date: 2026-11-22

Decision: store a diagram as `Document { title, shapes, connectors, next_shape,
next_connector }` in schema version 1. A `Shape` owns its id, kind, rectangular
geometry, label and colour; a `Connector` only stores the ids of the two shapes
it links. Geometry uses abstract canvas units and the UI owns pan/zoom.

Alternatives: embed paths in each connector; store screen coordinates; reference
external assets by path.

Reasons: connectors survive moves because endpoints are resolved dynamically,
and a saved project stays self-sufficient. Plain data keeps the model testable
and serializable through the CraftKit envelope.

Consequences: deleting a shape must also drop its connectors; new shapes and
links are allocated from document-owned counters. The SVG renderer reuses the
same edge-point geometry as the canvas so exports match the editor.

## ADR-003 — Productisation schema and routing

Date: 2026-11-22

Decision: for the v0.2 editor, each `Connector` carries a `routing` mode
(straight or orthogonal) and optional `waypoints`. The schema is bumped to 2.
Schema-1 files keep loading because the new fields use `#[serde(default)]`;
`Document::normalize` then repairs counters and drops dangling links after a
file is opened.

Alternatives: keep a single routing style; store screen coordinates for
waypoints; reject older files and require manual conversion.

Reasons: orthogonal routing with waypoints is what makes connectors useful in a
technical diagram, and defaults keep the published v0.1.0 release readable as a
snapshot. Pure geometry functions (`geometry.rs`) are shared by canvas, SVG and
PDF so all three agree on connector endpoints.

Consequences: the SVG and PDF renderers emit polylines for orthogonal links;
drags use one history gesture per interaction; label edits go through a single
`DocumentHost::apply` so they are one undo step.

## ADR-004 — Dependency-free vector PDF export

Date: 2026-11-22

Decision: generate PDF 1.4 bytes directly in `src/pdf.rs` using native path and
text operators (rectangles, cubic Bézier ellipses, polygons, polylines and
Helvetica text), with no new crate dependency.

Alternatives: rasterise the canvas to an image and embed it (not vector);
introduce a heavyweight PDF crate.

Reasons: the document is plain data, so a small deterministic writer produces a
valid single-page file that can be tested byte-for-byte without a GUI or a
font file. Only standard Helvetica is referenced, and text is WinAnsi with
non-Latin-1 characters replaced by `?`.

Consequences: exports are crisp at any zoom; non-Latin labels degrade to `?`;
PDF validity is covered by unit tests on the header, xref offsets, stream length
and page content.
