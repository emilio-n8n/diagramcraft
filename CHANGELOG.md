# Changelog — DiagramCraft

All notable changes to DiagramCraft are documented here.

## [0.2.1] - 2026-11-22

### Fixed

- The Rectangle, Ellipse, Note and Diamond tools now place a new shape when
  clicking anywhere on empty canvas, instead of only on an existing shape.
- Removed duplicated Settings and Job buttons from the toolbar.

## [0.2.0] - 2026-11-22

### Added

- In-place shape label editing with a double-click or Enter (F2); commits as
  one undo step.
- Eight resize grips for single-shape selection, with a minimum size and one
  undo step per drag.
- Connector routing choice: straight (unchanged) or orthogonal with editable
  waypoints. Waypoints can be dragged, added with a double-click and removed.
- Connectors can be selected and deleted independently of shapes.
- Multi-selection with Shift+click, grouped drag and grouped deletion.
- Copy, paste and duplicate of a selection, including links inside the
  selection, with fresh ids.
- Align (six modes) and distribute (two axes) commands for multiple shapes.
- Vector PDF export alongside the existing standalone SVG export.
- Zoom-to-fit (F or Fit button) and an optional grid and snapping toggle.
- Empty-canvas hint and a status bar showing tool, zoom and selection state.
- Schema 2: connectors store their routing and waypoints. Schema-1 files stay
  readable and are migrated with serde defaults plus counter normalisation.

### Changed

- Selection is now a list of shape ids rather than a single id.
- Drags (move, resize, waypoint) each record exactly one undo step through a
  document gesture.
- Sample document demonstrates one straight and one orthogonal connector.

## [0.1.0] - 2026-11-22

### Added

- Initial CraftKit-based application shell.
- Versioned project files with atomic save.
- Undo/redo through the shared document history.
- Background jobs with progress and cancellation.
- Diagram document model with shapes, connectors and id allocation.
- Shape palette, canvas with pan/zoom, selection, move, connect and delete.
- Grid snapping and edge-anchored straight connectors.
- Standalone SVG export.
