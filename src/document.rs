//! DiagramCraft document model.
//!
//! A document is a self-sufficient description of a diagram: shapes with
//! rectangular geometry, connectors between shape ids, and the counters needed
//! to allocate new objects. Everything is plain data so a saved project can be
//! reopened without any external asset. Geometry uses abstract canvas units;
//! the UI applies pan and zoom when drawing.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::geometry::{self, AlignMode, Axis, Handle};

/// Spacing of the placement/move snapping grid, in canvas units.
pub const SNAP_GRID: f64 = 10.0;

/// The visual kind of a placed shape.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    /// A plain rectangle.
    #[default]
    Rectangle,
    /// An ellipse fitted to the shape bounds.
    Ellipse,
    /// A sticky-note style rectangle with a folded corner.
    Note,
    /// A diamond (rhombus) fitted to the shape bounds.
    Diamond,
}

impl ShapeKind {
    /// All kinds, in the order shown in the palette.
    pub const ALL: [Self; 4] = [Self::Rectangle, Self::Ellipse, Self::Note, Self::Diamond];

    /// Human-readable name used in the UI.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Note => "Note",
            Self::Diamond => "Diamond",
        }
    }

    /// Default fill/stroke colour as packed `0xRRGGBB`.
    #[must_use]
    pub const fn default_color(self) -> u32 {
        match self {
            Self::Rectangle => 0xE8_57_3F,
            Self::Ellipse => 0x6A_A8_F5,
            Self::Note => 0xEE_B4_56,
            Self::Diamond => 0x54_D1_84,
        }
    }

    /// Default width and height of a newly placed shape.
    #[must_use]
    pub const fn default_size(self) -> (f64, f64) {
        match self {
            Self::Rectangle => (140.0, 70.0),
            Self::Ellipse => (130.0, 80.0),
            Self::Note => (140.0, 90.0),
            Self::Diamond => (140.0, 90.0),
        }
    }
}

/// A shape placed on the canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    /// Stable identifier, allocated from [`Document::next_shape`].
    pub id: u64,
    /// Visual kind.
    pub kind: ShapeKind,
    /// Left edge in canvas units.
    pub x: f64,
    /// Top edge in canvas units.
    pub y: f64,
    /// Width in canvas units.
    pub w: f64,
    /// Height in canvas units.
    pub h: f64,
    /// Text drawn in the centre of the shape.
    pub label: String,
    /// Fill and stroke colour as packed `0xRRGGBB`.
    pub color: u32,
}

impl Shape {
    /// Rectangle `(x, y, w, h)` of this shape.
    #[must_use]
    pub const fn rect(&self) -> (f64, f64, f64, f64) {
        (self.x, self.y, self.w, self.h)
    }

    /// Copy of this shape with a new id and a position offset.
    #[must_use]
    pub fn translated(&self, id: u64, dx: f64, dy: f64) -> Self {
        Self {
            id,
            x: self.x + dx,
            y: self.y + dy,
            ..self.clone()
        }
    }
}

/// A point in canvas units, used for connector waypoints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl Point {
    /// Build a point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Shift the point by `(dx, dy)`.
    #[must_use]
    pub const fn translated(self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// How a connector is drawn between two shapes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Routing {
    /// One direct line between the two edge anchors.
    #[default]
    Straight,
    /// Axis-aligned segments, optionally through waypoints.
    Orthogonal,
}

impl Routing {
    /// Human-readable name used in the UI.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Straight => "Straight",
            Self::Orthogonal => "Orthogonal",
        }
    }
}

/// A link between two shapes, anchored to their edges at render time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    /// Stable identifier, allocated from [`Document::next_connector`].
    pub id: u64,
    /// Id of the shape the link starts from.
    pub from_shape: u64,
    /// Id of the shape the link ends at.
    pub to_shape: u64,
    /// Routing style. Defaults to straight for schema-1 files.
    #[serde(default)]
    pub routing: Routing,
    /// Optional editable bend points, used by orthogonal routing.
    #[serde(default)]
    pub waypoints: Vec<Point>,
}

impl Connector {
    /// Copy of this connector with new shape ids and shifted waypoints.
    #[must_use]
    fn remapped(&self, id: u64, from: u64, to: u64, dx: f64, dy: f64) -> Self {
        Self {
            id,
            from_shape: from,
            to_shape: to,
            routing: self.routing,
            waypoints: self
                .waypoints
                .iter()
                .map(|point| point.translated(dx, dy))
                .collect(),
        }
    }
}

/// A copied set of shapes and the links entirely inside that set.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Clipboard {
    /// Copied shapes, with their original ids.
    pub shapes: Vec<Shape>,
    /// Copied links, with their original ids.
    pub links: Vec<Connector>,
}

/// The versioned application document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Editable document title.
    pub title: String,
    /// Placed shapes, in drawing order.
    pub shapes: Vec<Shape>,
    /// Links between shapes.
    pub connectors: Vec<Connector>,
    /// Next shape id to allocate.
    pub next_shape: u64,
    /// Next connector id to allocate.
    pub next_connector: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            title: "Untitled".into(),
            shapes: Vec::new(),
            connectors: Vec::new(),
            next_shape: 1,
            next_connector: 1,
        }
    }
}

impl Document {
    /// Built-in demonstration content: three shapes linked in a chain.
    #[must_use]
    pub fn sample() -> Self {
        let make = |id: u64, kind: ShapeKind, x: f64, y: f64, label: &str| Shape {
            id,
            kind,
            x,
            y,
            w: kind.default_size().0,
            h: kind.default_size().1,
            label: label.into(),
            color: kind.default_color(),
        };
        Self {
            title: "DiagramCraft sample".into(),
            shapes: vec![
                make(1, ShapeKind::Rectangle, 40.0, 80.0, "Start"),
                make(2, ShapeKind::Diamond, 260.0, 70.0, "Decision?"),
                make(3, ShapeKind::Ellipse, 490.0, 80.0, "End"),
            ],
            connectors: vec![
                Connector {
                    id: 1,
                    from_shape: 1,
                    to_shape: 2,
                    routing: Routing::Straight,
                    waypoints: Vec::new(),
                },
                Connector {
                    id: 2,
                    from_shape: 2,
                    to_shape: 3,
                    routing: Routing::Orthogonal,
                    waypoints: vec![Point::new(445.0, 200.0)],
                },
            ],
            next_shape: 4,
            next_connector: 3,
        }
    }

    /// Look up a shape by id.
    #[must_use]
    pub fn find_shape(&self, id: u64) -> Option<&Shape> {
        self.shapes.iter().find(|shape| shape.id == id)
    }

    /// Mutable lookup of a shape by id.
    pub fn find_shape_mut(&mut self, id: u64) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|shape| shape.id == id)
    }

    /// Look up a connector by id.
    #[must_use]
    pub fn find_connector(&self, id: u64) -> Option<&Connector> {
        self.connectors.iter().find(|link| link.id == id)
    }

    /// Mutable lookup of a connector by id.
    pub fn find_connector_mut(&mut self, id: u64) -> Option<&mut Connector> {
        self.connectors.iter_mut().find(|link| link.id == id)
    }

    /// Place a new shape centred on `(center_x, center_y)` and return its id.
    pub fn add_shape(&mut self, kind: ShapeKind, center_x: f64, center_y: f64) -> u64 {
        let id = self.next_shape;
        self.next_shape += 1;
        let (w, h) = kind.default_size();
        self.shapes.push(Shape {
            id,
            kind,
            x: center_x - w / 2.0,
            y: center_y - h / 2.0,
            w,
            h,
            label: kind.label().into(),
            color: kind.default_color(),
        });
        id
    }

    /// Change a shape's label. Returns `true` when it changed.
    pub fn set_label(&mut self, id: u64, label: &str) -> bool {
        let Some(shape) = self.find_shape_mut(id) else {
            return false;
        };
        if shape.label == label {
            return false;
        }
        shape.label = label.to_owned();
        true
    }

    /// Replace a shape's rectangle. Returns `true` when it changed.
    pub fn set_shape_rect(&mut self, id: u64, rect: (f64, f64, f64, f64)) -> bool {
        let Some(shape) = self.find_shape_mut(id) else {
            return false;
        };
        if shape.rect() == rect {
            return false;
        }
        (shape.x, shape.y, shape.w, shape.h) = rect;
        true
    }

    /// Resize a shape by dragging `handle` by `(dx, dy)` from its current rect.
    #[allow(dead_code)]
    pub fn resize_shape(&mut self, id: u64, handle: Handle, dx: f64, dy: f64) -> bool {
        let Some(shape) = self.find_shape(id) else {
            return false;
        };
        let resized = geometry::resize_rect(shape.rect(), handle, dx, dy);
        self.set_shape_rect(id, resized)
    }

    /// Move a shape to a new top-left corner. Returns `true` when it changed.
    pub fn move_shape(&mut self, id: u64, x: f64, y: f64) -> bool {
        let Some(shape) = self.find_shape_mut(id) else {
            return false;
        };
        if shape.x == x && shape.y == y {
            return false;
        }
        shape.x = x;
        shape.y = y;
        true
    }

    /// Apply absolute positions `(id, x, y)` to several shapes at once.
    pub fn move_shapes_to(&mut self, positions: &[(u64, f64, f64)]) -> bool {
        let mut changed = false;
        for (id, x, y) in positions {
            changed |= self.move_shape(*id, *x, *y);
        }
        changed
    }

    /// Delete a shape and every connector attached to it.
    ///
    /// Returns `true` when a shape was removed.
    #[allow(dead_code)]
    pub fn remove_shape(&mut self, id: u64) -> bool {
        self.remove_shapes(&[id]) > 0
    }

    /// Delete several shapes and every connector attached to any of them.
    ///
    /// Returns the number of removed shapes.
    pub fn remove_shapes(&mut self, ids: &[u64]) -> usize {
        let wanted: HashSet<u64> = ids.iter().copied().collect();
        let before = self.shapes.len();
        self.shapes.retain(|shape| !wanted.contains(&shape.id));
        let removed = before - self.shapes.len();
        if removed > 0 {
            self.connectors.retain(|link| {
                !wanted.contains(&link.from_shape) && !wanted.contains(&link.to_shape)
            });
        }
        removed
    }

    /// Delete connectors by id. Returns the number of removed links.
    pub fn remove_connectors(&mut self, ids: &[u64]) -> usize {
        let wanted: HashSet<u64> = ids.iter().copied().collect();
        let before = self.connectors.len();
        self.connectors.retain(|link| !wanted.contains(&link.id));
        before - self.connectors.len()
    }

    /// Link two existing shapes with the given routing style.
    ///
    /// Rejects self-links, unknown ids and duplicates (in either direction).
    /// Returns the new connector id on success.
    pub fn add_connector_with_routing(
        &mut self,
        from: u64,
        to: u64,
        routing: Routing,
    ) -> Option<u64> {
        if from == to
            || self.find_shape(from).is_none()
            || self.find_shape(to).is_none()
            || self.connectors.iter().any(|link| {
                (link.from_shape == from && link.to_shape == to)
                    || (link.from_shape == to && link.to_shape == from)
            })
        {
            return None;
        }
        let id = self.next_connector;
        self.next_connector += 1;
        self.connectors.push(Connector {
            id,
            from_shape: from,
            to_shape: to,
            routing,
            waypoints: Vec::new(),
        });
        Some(id)
    }

    /// Link two existing shapes with straight routing.
    #[allow(dead_code)]
    pub fn add_connector(&mut self, from: u64, to: u64) -> Option<u64> {
        self.add_connector_with_routing(from, to, Routing::Straight)
    }

    /// Change the routing style of a connector.
    pub fn set_routing(&mut self, id: u64, routing: Routing) -> bool {
        let Some(link) = self.find_connector_mut(id) else {
            return false;
        };
        if link.routing == routing {
            return false;
        }
        link.routing = routing;
        true
    }

    /// Append a waypoint to a connector.
    pub fn add_waypoint(&mut self, id: u64, point: Point) -> bool {
        let Some(link) = self.find_connector_mut(id) else {
            return false;
        };
        link.waypoints.push(point);
        true
    }

    /// Move a waypoint of a connector.
    pub fn move_waypoint(&mut self, id: u64, index: usize, point: Point) -> bool {
        let Some(link) = self.find_connector_mut(id) else {
            return false;
        };
        let Some(waypoint) = link.waypoints.get_mut(index) else {
            return false;
        };
        if *waypoint == point {
            return false;
        }
        *waypoint = point;
        true
    }

    /// Remove a waypoint of a connector.
    #[allow(dead_code)]
    pub fn remove_waypoint(&mut self, id: u64, index: usize) -> bool {
        let Some(link) = self.find_connector_mut(id) else {
            return false;
        };
        if index >= link.waypoints.len() {
            return false;
        }
        link.waypoints.remove(index);
        true
    }

    /// Copy the given shapes and the links entirely inside the selection.
    #[must_use]
    pub fn copy(&self, ids: &[u64]) -> Clipboard {
        let selected: HashSet<u64> = ids.iter().copied().collect();
        Clipboard {
            shapes: self
                .shapes
                .iter()
                .filter(|shape| selected.contains(&shape.id))
                .cloned()
                .collect(),
            links: self
                .connectors
                .iter()
                .filter(|link| {
                    selected.contains(&link.from_shape) && selected.contains(&link.to_shape)
                })
                .cloned()
                .collect(),
        }
    }

    /// Paste a clipboard offset by `(dx, dy)`, allocating fresh ids.
    ///
    /// Returns the ids of the pasted shapes in clipboard order.
    pub fn paste(&mut self, clipboard: &Clipboard, dx: f64, dy: f64) -> Vec<u64> {
        let mut id_map = HashMap::new();
        let mut new_ids = Vec::new();
        for shape in &clipboard.shapes {
            let id = self.next_shape;
            self.next_shape += 1;
            id_map.insert(shape.id, id);
            self.shapes.push(shape.translated(id, dx, dy));
            new_ids.push(id);
        }
        for link in &clipboard.links {
            let (Some(from), Some(to)) = (id_map.get(&link.from_shape), id_map.get(&link.to_shape))
            else {
                continue;
            };
            let id = self.next_connector;
            self.next_connector += 1;
            self.connectors.push(link.remapped(id, *from, *to, dx, dy));
        }
        new_ids
    }

    /// Duplicate the given shapes, including links entirely inside the
    /// selection. Returns the ids of the new shapes in the same order.
    pub fn duplicate_shapes(&mut self, ids: &[u64], dx: f64, dy: f64) -> Vec<u64> {
        let clipboard = self.copy(ids);
        self.paste(&clipboard, dx, dy)
    }

    /// Align the given shapes. Returns `true` when anything moved.
    pub fn align_shapes(&mut self, ids: &[u64], mode: AlignMode) -> bool {
        let mut rects: Vec<(u64, (f64, f64, f64, f64))> = self
            .shapes
            .iter()
            .filter(|shape| ids.contains(&shape.id))
            .map(|shape| (shape.id, shape.rect()))
            .collect();
        if rects.len() < 2 {
            return false;
        }
        let mut values: Vec<_> = rects.iter().map(|entry| entry.1).collect();
        geometry::align_rects(&mut values, mode);
        let mut changed = false;
        for ((id, original), value) in rects.iter_mut().zip(values) {
            if *original != value {
                changed |= self.set_shape_rect(*id, value);
            }
        }
        changed
    }

    /// Distribute the given shapes. Returns `true` when anything moved.
    pub fn distribute_shapes(&mut self, ids: &[u64], axis: Axis) -> bool {
        let mut entries: Vec<(u64, (f64, f64, f64, f64))> = self
            .shapes
            .iter()
            .filter(|shape| ids.contains(&shape.id))
            .map(|shape| (shape.id, shape.rect()))
            .collect();
        if entries.len() < 3 {
            return false;
        }
        let mut values: Vec<_> = entries.iter().map(|entry| entry.1).collect();
        geometry::distribute_rects(&mut values, axis);
        let mut changed = false;
        for ((id, original), value) in entries.iter_mut().zip(values) {
            if *original != value {
                changed |= self.set_shape_rect(*id, value);
            }
        }
        changed
    }

    /// World-space polyline points of a connector, including edge anchors.
    #[must_use]
    pub fn connector_points(&self, connector: &Connector) -> Vec<Point> {
        let (Some(from), Some(to)) = (
            self.find_shape(connector.from_shape),
            self.find_shape(connector.to_shape),
        ) else {
            return Vec::new();
        };
        let from_centre = center(from);
        let to_centre = center(to);
        let start = edge_point(from, to_centre.0, to_centre.1);
        let end = edge_point(to, from_centre.0, from_centre.1);
        match connector.routing {
            Routing::Straight => vec![Point::new(start.0, start.1), Point::new(end.0, end.1)],
            Routing::Orthogonal => {
                let bends: Vec<(f64, f64)> = connector
                    .waypoints
                    .iter()
                    .map(|point| (point.x, point.y))
                    .collect();
                geometry::orthogonal_route(start, end, &bends)
                    .into_iter()
                    .map(|(x, y)| Point::new(x, y))
                    .collect()
            }
        }
    }

    /// Id of the connector whose polyline passes within `tolerance` units of
    /// the point, or `None`.
    #[must_use]
    pub fn hit_connector(&self, x: f64, y: f64, tolerance: f64) -> Option<u64> {
        let mut closest = None;
        let mut best = tolerance;
        for connector in &self.connectors {
            let points = self.connector_points(connector);
            for pair in points.windows(2) {
                let distance = geometry::point_segment_distance(
                    x, y, pair[0].x, pair[0].y, pair[1].x, pair[1].y,
                );
                if distance < best {
                    best = distance;
                    closest = Some(connector.id);
                }
            }
        }
        closest
    }

    /// Bounding box `(min_x, min_y, max_x, max_y)` of all shapes and waypoints.
    #[must_use]
    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let first = self.shapes.first()?;
        let mut bounds = (first.x, first.y, first.x + first.w, first.y + first.h);
        for shape in &self.shapes[1..] {
            bounds.0 = bounds.0.min(shape.x);
            bounds.1 = bounds.1.min(shape.y);
            bounds.2 = bounds.2.max(shape.x + shape.w);
            bounds.3 = bounds.3.max(shape.y + shape.h);
        }
        for connector in &self.connectors {
            for point in &connector.waypoints {
                bounds.0 = bounds.0.min(point.x);
                bounds.1 = bounds.1.min(point.y);
                bounds.2 = bounds.2.max(point.x);
                bounds.3 = bounds.3.max(point.y);
            }
        }
        Some(bounds)
    }

    /// Repair counters and drop dangling or self-referencing connectors.
    ///
    /// Called after opening older files so their id allocation cannot collide
    /// with existing objects.
    pub fn normalize(&mut self) {
        let max_shape = self.shapes.iter().map(|shape| shape.id).max().unwrap_or(0);
        self.next_shape = self.next_shape.max(max_shape.saturating_add(1));
        let max_connector = self
            .connectors
            .iter()
            .map(|link| link.id)
            .max()
            .unwrap_or(0);
        self.next_connector = self.next_connector.max(max_connector.saturating_add(1));
        let ids: HashSet<u64> = self.shapes.iter().map(|shape| shape.id).collect();
        self.connectors.retain(|link| {
            link.from_shape != link.to_shape
                && ids.contains(&link.from_shape)
                && ids.contains(&link.to_shape)
        });
    }
}

/// Position of a resize grip on a shape.
#[must_use]
pub fn handle_point(shape: &Shape, handle: Handle) -> Point {
    let (cx, cy) = center(shape);
    let x = if handle.moves_left() {
        shape.x
    } else if handle.moves_right() {
        shape.x + shape.w
    } else {
        cx
    };
    let y = if handle.moves_top() {
        shape.y
    } else if handle.moves_bottom() {
        shape.y + shape.h
    } else {
        cy
    };
    Point::new(x, y)
}

/// Find a resize grip near a canvas point, with a tolerance in canvas units.
#[must_use]
pub fn handle_at(shape: &Shape, x: f64, y: f64, tolerance: f64) -> Option<Handle> {
    Handle::ALL.iter().copied().find(|handle| {
        let point = handle_point(shape, *handle);
        (point.x - x).abs() <= tolerance && (point.y - y).abs() <= tolerance
    })
}

/// Centre of a shape.
#[must_use]
pub fn center(shape: &Shape) -> (f64, f64) {
    (shape.x + shape.w / 2.0, shape.y + shape.h / 2.0)
}

/// Point where a link aimed at `(toward_x, toward_y)` meets the shape's bounds.
///
/// Uses the shape's bounding rectangle, which gives a clean anchor for all four
/// kinds and makes canvas, SVG and PDF rendering agree on endpoints.
#[must_use]
pub fn edge_point(shape: &Shape, toward_x: f64, toward_y: f64) -> (f64, f64) {
    let (cx, cy) = center(shape);
    let dx = toward_x - cx;
    let dy = toward_y - cy;
    if dx == 0.0 && dy == 0.0 {
        return (cx, cy);
    }
    let half_w = (shape.w / 2.0).max(f64::EPSILON);
    let half_h = (shape.h / 2.0).max(f64::EPSILON);
    let tx = if dx == 0.0 {
        f64::INFINITY
    } else {
        half_w / dx.abs()
    };
    let ty = if dy == 0.0 {
        f64::INFINITY
    } else {
        half_h / dy.abs()
    };
    let t = tx.min(ty);
    (cx + dx * t, cy + dy * t)
}

/// Whether a canvas point lies inside a shape's bounding box.
#[must_use]
pub fn contains_point(shape: &Shape, x: f64, y: f64) -> bool {
    x >= shape.x && x <= shape.x + shape.w && y >= shape.y && y <= shape.y + shape.h
}

/// Snap a canvas coordinate to the nearest grid point.
#[must_use]
pub fn snap(value: f64) -> f64 {
    (value / SNAP_GRID).round() * SNAP_GRID
}

/// Snap only when grid snapping is enabled.
#[must_use]
pub fn snap_if(enabled: bool, value: f64) -> f64 {
    if enabled { snap(value) } else { value }
}

/// Unpack a `0xRRGGBB` colour into its components.
#[must_use]
pub fn rgb(color: u32) -> (u8, u8, u8) {
    (
        ((color >> 16) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        (color & 0xFF) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use craftkit_core::ProjectEnvelope;
    use craftkit_io::DocumentHost;

    const FORMAT: &str = "diagramcraft";
    const SCHEMA: u32 = 2;
    const VERSION: &str = "0.2.0";

    fn host(document: Document) -> DocumentHost<Document> {
        DocumentHost::new(document, FORMAT, SCHEMA, VERSION)
    }

    #[test]
    fn sample_has_three_shapes_and_two_connectors() {
        let document = Document::sample();
        assert_eq!(document.shapes.len(), 3);
        assert_eq!(document.connectors.len(), 2);
        assert_eq!(document.next_shape, 4);
        assert_eq!(document.next_connector, 3);
        assert_eq!(document.connectors[0].routing, Routing::Straight);
        assert_eq!(document.connectors[1].routing, Routing::Orthogonal);
        assert_eq!(document.connectors[1].waypoints.len(), 1);
    }

    #[test]
    fn adding_a_shape_assigns_a_unique_id() {
        let mut document = Document::default();
        let first = document.add_shape(ShapeKind::Rectangle, 100.0, 100.0);
        let second = document.add_shape(ShapeKind::Note, 100.0, 100.0);
        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert_eq!(document.next_shape, 3);
        let shape = document.find_shape(first).expect("first shape");
        assert_eq!(shape.x, 100.0 - 140.0 / 2.0);
        assert_eq!(shape.label, "Rectangle");
    }

    #[test]
    fn find_shape_by_id() {
        let document = Document::sample();
        assert_eq!(
            document.find_shape(2).map(|shape| shape.label.as_str()),
            Some("Decision?")
        );
        assert!(document.find_shape(999).is_none());
        let mut document = document;
        document.find_shape_mut(2).expect("shape 2").label = "Revised".into();
        assert_eq!(document.find_shape(2).expect("shape 2").label, "Revised");
    }

    #[test]
    fn set_label_is_one_action_and_ignores_unchanged_text() {
        let mut document = Document::sample();
        assert!(!document.set_label(1, "Start"));
        assert!(document.set_label(1, "Begin"));
        assert_eq!(document.find_shape(1).expect("shape").label, "Begin");
        assert!(!document.set_label(999, "Missing"));
    }

    #[test]
    fn resize_and_set_rect_enforce_the_shape_geometry() {
        let mut document = Document::sample();
        assert!(document.resize_shape(1, Handle::E, 40.0, 0.0));
        assert_eq!(document.find_shape(1).expect("shape").w, 180.0);
        assert!(document.set_shape_rect(1, (0.0, 0.0, 30.0, 30.0)));
        assert_eq!(
            document.find_shape(1).expect("shape").rect(),
            (0.0, 0.0, 30.0, 30.0)
        );
        assert!(!document.resize_shape(999, Handle::E, 1.0, 0.0));
    }

    #[test]
    fn removing_a_shape_cleans_its_connectors() {
        let mut document = Document::sample();
        assert!(document.remove_shape(2));
        assert_eq!(document.shapes.len(), 2);
        assert!(document.connectors.is_empty());
        assert!(!document.remove_shape(2));
    }

    #[test]
    fn removing_a_selection_cleans_all_attached_connectors() {
        let mut document = Document::sample();
        document.add_shape(ShapeKind::Rectangle, 700.0, 80.0);
        let fourth = document.next_shape - 1;
        assert_eq!(document.add_connector(3, fourth), Some(3));
        assert_eq!(document.remove_shapes(&[1, 3]), 2);
        assert_eq!(document.shapes.len(), 2);
        assert!(document.connectors.is_empty());
    }

    #[test]
    fn connectors_reject_self_links_unknown_ids_and_duplicates() {
        let mut document = Document::sample();
        assert!(document.add_connector(1, 1).is_none());
        assert!(document.add_connector(1, 999).is_none());
        assert!(document.add_connector(1, 2).is_none()); // already linked
        assert!(document.add_connector(2, 1).is_none()); // duplicate, reversed
        assert_eq!(document.add_connector(1, 3), Some(3));
        assert_eq!(document.next_connector, 4);
    }

    #[test]
    fn routing_and_waypoints_can_be_edited_and_deleted() {
        let mut document = Document::sample();
        let id = 1;
        assert!(document.set_routing(id, Routing::Orthogonal));
        assert!(!document.set_routing(id, Routing::Orthogonal));
        assert!(document.add_waypoint(id, Point::new(200.0, 300.0)));
        assert!(document.move_waypoint(id, 0, Point::new(210.0, 310.0)));
        assert_eq!(
            document.find_connector(id).expect("link").waypoints,
            vec![Point::new(210.0, 310.0)]
        );
        let points = document.connector_points(document.find_connector(id).expect("link"));
        assert!(points.contains(&Point::new(210.0, 310.0)));
        assert!(
            points
                .windows(2)
                .all(|pair| pair[0].x == pair[1].x || pair[0].y == pair[1].y)
        );
        assert!(document.remove_waypoint(id, 0));
        assert!(!document.remove_waypoint(id, 0));
        assert_eq!(document.remove_connectors(&[id]), 1);
    }

    #[test]
    fn connector_hit_test_finds_the_nearest_segment() {
        let document = Document::sample();
        let link = document.find_connector(1).expect("link");
        let points = document.connector_points(link);
        let middle = (
            (points[0].x + points[1].x) / 2.0,
            (points[0].y + points[1].y) / 2.0,
        );
        assert_eq!(document.hit_connector(middle.0, middle.1, 10.0), Some(1));
        assert_eq!(document.hit_connector(5000.0, 5000.0, 10.0), None);
    }

    #[test]
    fn copy_paste_keeps_links_and_offsets_waypoints() {
        let mut document = Document::sample();
        let clipboard = document.copy(&[2, 3]);
        assert_eq!(clipboard.shapes.len(), 2);
        assert_eq!(clipboard.links.len(), 1);
        let ids = document.paste(&clipboard, 25.0, 35.0);
        assert_eq!(ids, vec![4, 5]);
        let pasted = document.find_shape(4).expect("pasted shape");
        assert_eq!((pasted.x, pasted.y), (285.0, 105.0));
        let link = document.find_connector(3).expect("pasted link");
        assert_eq!((link.from_shape, link.to_shape), (4, 5));
        assert_eq!(link.waypoints, vec![Point::new(470.0, 235.0)]);
    }

    #[test]
    fn copy_and_paste_keep_the_clipboard_unchanged() {
        let mut document = Document::sample();
        let clipboard = document.copy(&[1, 2]);
        assert_eq!(clipboard.shapes.len(), 2);
        assert_eq!(clipboard.links.len(), 1);
        let first = document.paste(&clipboard, 20.0, 30.0);
        let second = document.paste(&clipboard, 40.0, 50.0);
        assert_eq!(first, vec![4, 5]);
        assert_eq!(second, vec![6, 7]);
        assert_eq!(document.shapes.len(), 7);
        assert_eq!(document.find_shape(4).expect("paste").x, 60.0);
        assert_eq!(document.find_shape(6).expect("paste").x, 80.0);
        assert_eq!(clipboard.shapes[0].id, 1);
    }

    #[test]
    fn duplicate_shapes_copies_internal_links_and_offsets_waypoints() {
        let mut document = Document::sample();
        let new_ids = document.duplicate_shapes(&[2, 3], 20.0, 40.0);
        assert_eq!(new_ids, vec![4, 5]);
        assert_eq!(document.shapes.len(), 5);
        let copy = document.find_shape(4).expect("first copy");
        assert_eq!((copy.x, copy.y), (280.0, 110.0));
        assert_eq!(document.connectors.len(), 3);
        let copied_link = document.find_connector(3).expect("copied link");
        assert_eq!((copied_link.from_shape, copied_link.to_shape), (4, 5));
        assert_eq!(copied_link.waypoints, vec![Point::new(465.0, 240.0)]);
    }

    #[test]
    fn align_and_distribute_move_only_the_selected_shapes() {
        let mut document = Document::sample();
        assert!(document.align_shapes(&[1, 2], AlignMode::Left));
        let first = document.find_shape(1).expect("shape 1");
        let second = document.find_shape(2).expect("shape 2");
        assert_eq!(first.x, second.x);
        assert_eq!(document.find_shape(3).expect("shape 3").x, 490.0);
        assert!(!document.align_shapes(&[1], AlignMode::Left));

        assert!(document.distribute_shapes(&[1, 2, 3], Axis::Horizontal));
        assert_eq!(document.find_shape(2).expect("shape 2").x, 265.0);
        assert!(!document.distribute_shapes(&[1, 2], Axis::Horizontal));
    }

    #[test]
    fn round_trip_json_via_document_host() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("diagram.json");

        let mut original = host(Document::sample());
        original.apply(|document| {
            document.title = "Round trip".into();
            document
                .find_connector_mut(2)
                .expect("link")
                .waypoints
                .push(Point::new(500.0, 250.0));
        });
        original.save_as(&path).expect("save versioned envelope");

        let loaded = DocumentHost::<Document>::from_file(&path, FORMAT, SCHEMA, VERSION)
            .expect("load versioned envelope");
        assert_eq!(loaded.document(), original.document());
        assert_eq!(loaded.format(), FORMAT);
        assert_eq!(loaded.schema_version(), SCHEMA);
        assert!(!loaded.is_dirty());
    }

    #[test]
    fn schema_one_files_migrate_to_the_current_document() {
        let legacy = r#"{
            "format": "diagramcraft",
            "schema_version": 1,
            "app_version": "0.1.0",
            "document": {
                "title": "Legacy",
                "shapes": [
                    {"id":1,"kind":"Rectangle","x":0.0,"y":0.0,"w":100.0,"h":50.0,"label":"A","color":123456},
                    {"id":2,"kind":"Ellipse","x":200.0,"y":0.0,"w":100.0,"h":50.0,"label":"B","color":654321}
                ],
                "connectors": [{"id":1,"from_shape":1,"to_shape":2}],
                "next_shape": 2,
                "next_connector": 1
            }
        }"#;
        let envelope: ProjectEnvelope<Document> =
            serde_json::from_str(legacy).expect("parse schema-1 project");
        envelope
            .validate(FORMAT, SCHEMA)
            .expect("older schema accepted");
        let mut document = envelope.document;
        assert_eq!(document.connectors[0].routing, Routing::Straight);
        assert!(document.connectors[0].waypoints.is_empty());
        document.normalize();
        assert_eq!(document.next_shape, 3);
        assert_eq!(document.next_connector, 2);
    }

    #[test]
    fn undo_redo_of_a_move_gesture_is_one_step() {
        let mut host = host(Document::sample());
        let id = 1;
        let (x, y) = {
            let shape = host.document().find_shape(id).expect("shape 1");
            (shape.x, shape.y)
        };

        host.history_mut().begin_gesture();
        for offset in 1..=3 {
            host.history_mut().current_mut().move_shape(
                id,
                x + 10.0 * f64::from(offset),
                y + 5.0 * f64::from(offset),
            );
        }
        host.history_mut().end_gesture();

        assert_eq!(host.history().undo_depth(), 1);
        assert_eq!(host.document().find_shape(id).expect("shape 1").x, x + 30.0);
        assert!(host.undo());
        assert_eq!(host.document().find_shape(id).expect("shape 1").x, x);
        assert_eq!(host.document().find_shape(id).expect("shape 1").y, y);
        assert!(host.redo());
        assert_eq!(host.document().find_shape(id).expect("shape 1").x, x + 30.0);
    }

    #[test]
    fn label_edit_in_a_host_is_one_undo_step() {
        let mut host = host(Document::sample());
        assert!(host.apply(|document| {
            document.set_label(2, "Approved?");
        }));
        assert_eq!(host.history().undo_depth(), 1);
        assert!(host.undo());
        assert_eq!(
            host.document().find_shape(2).expect("shape").label,
            "Decision?"
        );
        assert!(host.redo());
        assert_eq!(
            host.document().find_shape(2).expect("shape").label,
            "Approved?"
        );
    }

    #[test]
    fn snapping_rounds_to_the_grid_only_when_enabled() {
        assert_eq!(snap(0.0), 0.0);
        assert_eq!(snap(4.0), 0.0);
        assert_eq!(snap(6.0), SNAP_GRID);
        assert_eq!(snap(-14.0), -10.0);
        assert_eq!(snap(123.0), 120.0);
        assert_eq!(snap_if(false, 123.0), 123.0);
        assert_eq!(snap_if(true, 123.0), 120.0);
    }

    #[test]
    fn edge_point_lies_on_the_rectangle_boundary() {
        let shape = Shape {
            id: 1,
            kind: ShapeKind::Rectangle,
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
            label: String::new(),
            color: 0,
        };
        assert_eq!(edge_point(&shape, 200.0, 25.0), (100.0, 25.0));
        assert_eq!(edge_point(&shape, 50.0, -100.0), (50.0, 0.0));
        assert_eq!(edge_point(&shape, 50.0, 1000.0), (50.0, 50.0));
        assert_eq!(edge_point(&shape, 50.0, 25.0), (50.0, 25.0));
    }

    #[test]
    fn contains_point_matches_the_bounding_box() {
        let shape = Document::sample().shapes[0].clone();
        assert!(contains_point(&shape, shape.x + 1.0, shape.y + 1.0));
        assert!(!contains_point(&shape, shape.x - 1.0, shape.y + 1.0));
    }

    #[test]
    fn bounds_cover_shapes_and_waypoints() {
        let bounds = Document::sample().bounds().expect("bounds");
        assert_eq!(bounds, (40.0, 70.0, 620.0, 200.0));
        assert!(Document::default().bounds().is_none());
    }
}
