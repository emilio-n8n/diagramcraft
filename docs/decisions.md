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
