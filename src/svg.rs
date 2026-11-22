//! Readable SVG export for diagram documents.
//!
//! The renderer is a pure function of the document so it can be unit-tested
//! without a window. Connector endpoints use the same geometry as the canvas
//! and the PDF exporter, keeping exported diagrams consistent with the editor.

use crate::document::{self, Document, Point, Shape, ShapeKind};

const PADDING: f64 = 40.0;
const BACKGROUND: &str = "#121419";
const TEXT_COLOR: &str = "#eaeef6";
const LINK_COLOR: &str = "#8c95a5";

/// Replace characters that are special in XML text content or attributes.
fn escape_xml(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Format a `0xRRGGBB` colour as a CSS hex string.
fn hex(color: u32) -> String {
    format!("#{color:06X}")
}

/// Compact number formatting: integers stay integers, fractions get 2 decimals.
fn number(value: f64) -> String {
    if (value - value.round()).abs() < 1e-6 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.2}")
    }
}

/// Format a point as an SVG `x,y` pair.
fn point(value: Point) -> String {
    format!("{},{}", number(value.x), number(value.y))
}

/// Arrow head at the last point of a connector path.
fn arrow(from: Point, to: Point) -> String {
    let angle = (to.y - from.y).atan2(to.x - from.x);
    const LENGTH: f64 = 12.0;
    let left = Point::new(
        to.x + LENGTH * (angle + std::f64::consts::PI - 0.45).cos(),
        to.y + LENGTH * (angle + std::f64::consts::PI - 0.45).sin(),
    );
    let right = Point::new(
        to.x + LENGTH * (angle + std::f64::consts::PI + 0.45).cos(),
        to.y + LENGTH * (angle + std::f64::consts::PI + 0.45).sin(),
    );
    format!(
        "  <polygon points=\"{} {} {}\" fill=\"{}\"/>\n",
        point(to),
        point(left),
        point(right),
        LINK_COLOR
    )
}

/// Render a whole document as a standalone SVG string.
#[must_use]
pub fn render(document: &Document) -> String {
    let (x, y, width, height) = match document.bounds() {
        Some((min_x, min_y, max_x, max_y)) => (
            min_x - PADDING,
            min_y - PADDING,
            (max_x - min_x + 2.0 * PADDING).max(1.0),
            (max_y - min_y + 2.0 * PADDING).max(1.0),
        ),
        None => (0.0, 0.0, 800.0, 600.0),
    };

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">\n",
        number(width),
        number(height),
        number(x),
        number(y),
        number(width),
        number(height)
    ));
    out.push_str(&format!(
        "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
        number(x),
        number(y),
        number(width),
        number(height),
        BACKGROUND
    ));

    for connector in &document.connectors {
        let path = document.connector_points(connector);
        if path.len() < 2 {
            continue;
        }
        let line = if path.len() == 2 {
            format!(
                "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                number(path[0].x),
                number(path[0].y),
                number(path[1].x),
                number(path[1].y),
                LINK_COLOR
            )
        } else {
            let points: Vec<String> = path.iter().map(|point| crate::svg::point(*point)).collect();
            format!(
                "  <polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                points.join(" "),
                LINK_COLOR
            )
        };
        out.push_str(&line);

        let split = path.len().saturating_sub(1);
        let (before, last) = path.split_at(split);
        if let (Some(previous), Some(end)) = (before.last(), last.first()) {
            out.push_str(&arrow(*previous, *end));
        }
    }

    for shape in &document.shapes {
        out.push_str(&render_shape(shape));
    }

    out.push_str("</svg>\n");
    out
}

/// Render one shape and its centred label.
fn render_shape(shape: &Shape) -> String {
    let color = hex(shape.color);
    let mut attributes =
        format!("fill=\"{color}\" fill-opacity=\"0.18\" stroke=\"{color}\" stroke-width=\"2\"");
    if shape.kind == ShapeKind::Note {
        attributes.push_str(" stroke-dasharray=\"6 3\"");
    }

    let geometry = match shape.kind {
        ShapeKind::Rectangle | ShapeKind::Note => format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"6\" {attributes}/>",
            number(shape.x),
            number(shape.y),
            number(shape.w),
            number(shape.h)
        ),
        ShapeKind::Ellipse => {
            let (cx, cy) = document::center(shape);
            format!(
                "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {attributes}/>",
                number(cx),
                number(cy),
                number(shape.w / 2.0),
                number(shape.h / 2.0)
            )
        }
        ShapeKind::Diamond => {
            let (cx, cy) = document::center(shape);
            format!(
                "<polygon points=\"{},{} {},{} {},{} {},{}\" {attributes}/>",
                number(cx),
                number(shape.y),
                number(shape.x + shape.w),
                number(cy),
                number(cx),
                number(shape.y + shape.h),
                number(shape.x),
                number(cy)
            )
        }
    };

    let mut out = format!("  {geometry}\n");
    if shape.kind == ShapeKind::Note {
        let fold = 18.0;
        out.push_str(&format!(
            "  <polyline points=\"{},{} {},{} {},{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\"/>\n",
            number(shape.x + shape.w - fold),
            number(shape.y),
            number(shape.x + shape.w - fold),
            number(shape.y + fold),
            number(shape.x + shape.w),
            number(shape.y + fold)
        ));
    }

    let (cx, cy) = document::center(shape);
    out.push_str(&format!(
        "  <text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-family=\"sans-serif\" font-size=\"14\" fill=\"{}\">{}</text>\n",
        number(cx),
        number(cy),
        TEXT_COLOR,
        escape_xml(&shape.label)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Routing, ShapeKind};

    #[test]
    fn sample_renders_all_shapes_and_connectors() {
        let svg = render(&Document::sample());
        assert!(svg.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(svg.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert_eq!(svg.matches("<line ").count(), 1);
        assert!(svg.contains("<polyline"));
        assert_eq!(svg.matches("<polygon points=").count(), 3); // 2 arrow heads + 1 diamond
        assert!(svg.contains(">Start</text>"));
        assert!(svg.contains(">Decision?</text>"));
        assert!(svg.contains(">End</text>"));
        assert!(svg.contains("#E8573F"));
        assert!(svg.contains("viewBox=\"0 30 660 210\""));
    }

    #[test]
    fn orthogonal_connectors_render_as_polylines() {
        let mut document = Document::default();
        let a = document.add_shape(ShapeKind::Rectangle, 0.0, 0.0);
        let b = document.add_shape(ShapeKind::Rectangle, 300.0, 200.0);
        document
            .add_connector_with_routing(a, b, Routing::Orthogonal)
            .expect("connector");
        let svg = render(&document);
        assert!(svg.contains("<polyline"));
        assert!(!svg.contains("<line "));
        assert_eq!(svg.matches("<polygon points=").count(), 1); // arrow head
    }

    #[test]
    fn waypoints_appear_in_the_polyline() {
        let mut document = Document::default();
        let a = document.add_shape(ShapeKind::Rectangle, 0.0, 0.0);
        let b = document.add_shape(ShapeKind::Rectangle, 300.0, 200.0);
        let id = document
            .add_connector_with_routing(a, b, Routing::Orthogonal)
            .expect("connector");
        document.add_waypoint(id, Point::new(150.0, 250.0));
        let svg = render(&document);
        assert!(svg.contains("150,250"));
    }

    #[test]
    fn escapes_label_special_characters() {
        let mut document = Document::default();
        document.add_shape(ShapeKind::Rectangle, 0.0, 0.0);
        document.shapes[0].label = "A & B <C> \"D\" 'E'".into();
        let svg = render(&document);
        assert!(svg.contains("A &amp; B &lt;C&gt; &quot;D&quot; &apos;E&apos;"));
        assert!(!svg.contains("& B"));
    }

    #[test]
    fn empty_document_uses_a_default_viewbox() {
        let svg = render(&Document::default());
        assert!(svg.contains("viewBox=\"0 0 800 600\""));
        assert!(!svg.contains("<line "));
        assert!(svg.trim_end().ends_with("</svg>"));
    }
}
