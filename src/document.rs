//! DiagramCraft document model.
//!
//! A document is a self-sufficient description of a diagram: shapes with
//! normalized geometry, connectors between shape ids and the id counters needed
//! to allocate new objects. Everything is plain data so a saved project can be
//! reopened without any external asset. Geometry uses abstract canvas units;
//! the UI applies pan and zoom when drawing.

use serde::{Deserialize, Serialize};

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

/// A straight link between two shapes, anchored to their edges at render time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connector {
    /// Stable identifier, allocated from [`Document::next_connector`].
    pub id: u64,
    /// Id of the shape the link starts from.
    pub from_shape: u64,
    /// Id of the shape the link ends at.
    pub to_shape: u64,
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
                },
                Connector {
                    id: 2,
                    from_shape: 2,
                    to_shape: 3,
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

    /// Delete a shape and every connector attached to it.
    ///
    /// Returns `true` when a shape was removed.
    pub fn remove_shape(&mut self, id: u64) -> bool {
        let before = self.shapes.len();
        self.shapes.retain(|shape| shape.id != id);
        if self.shapes.len() == before {
            return false;
        }
        self.connectors
            .retain(|link| link.from_shape != id && link.to_shape != id);
        true
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

    /// Link two existing shapes.
    ///
    /// Rejects self-links, unknown ids and duplicates (in either direction).
    /// Returns the new connector id on success.
    pub fn add_connector(&mut self, from: u64, to: u64) -> Option<u64> {
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
        });
        Some(id)
    }

    /// Bounding box `(min_x, min_y, max_x, max_y)` of all shapes, if any.
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
        Some(bounds)
    }
}

/// Centre of a shape.
#[must_use]
pub fn center(shape: &Shape) -> (f64, f64) {
    (shape.x + shape.w / 2.0, shape.y + shape.h / 2.0)
}

/// Point where a link aimed at `(toward_x, toward_y)` meets the shape's bounds.
///
/// Uses the shape's bounding rectangle, which gives a clean anchor for all four
/// kinds and makes both the canvas and the SVG renderer agree on endpoints.
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
    use craftkit_io::DocumentHost;

    const FORMAT: &str = "diagramcraft";
    const SCHEMA: u32 = 1;
    const VERSION: &str = "0.1.0";

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
    fn removing_a_shape_cleans_its_connectors() {
        let mut document = Document::sample();
        assert!(document.remove_shape(2));
        assert_eq!(document.shapes.len(), 2);
        assert!(document.connectors.is_empty());
        assert!(!document.remove_shape(2));
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
    fn round_trip_json_via_document_host() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("diagram.json");

        let mut original = host(Document::sample());
        original.apply(|document| document.title = "Round trip".into());
        original.save_as(&path).expect("save versioned envelope");

        let loaded = DocumentHost::<Document>::from_file(&path, FORMAT, SCHEMA, VERSION)
            .expect("load versioned envelope");
        assert_eq!(loaded.document(), original.document());
        assert_eq!(loaded.format(), FORMAT);
        assert_eq!(loaded.schema_version(), SCHEMA);
        assert!(!loaded.is_dirty());
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
    fn snapping_rounds_to_the_grid() {
        assert_eq!(snap(0.0), 0.0);
        assert_eq!(snap(4.0), 0.0);
        assert_eq!(snap(6.0), SNAP_GRID);
        assert_eq!(snap(-14.0), -10.0);
        assert_eq!(snap(123.0), 120.0);
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
    fn bounds_cover_every_shape() {
        let bounds = Document::sample().bounds().expect("bounds");
        assert_eq!(bounds, (40.0, 70.0, 620.0, 160.0));
        assert!(Document::default().bounds().is_none());
    }
}
