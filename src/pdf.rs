//! Minimal vector PDF exporter.
//!
//! The writer emits a single-page PDF 1.4 file whose page content is made of
//! native path and text operators (no raster image, no external dependency).
//! It shares document geometry with the SVG exporter so both outputs look the
//! same. Only the standard Helvetica font is referenced; text is encoded as
//! WinAnsi with non-Latin-1 code points replaced by a question mark.

use crate::document::{self, Document, Point, Shape, ShapeKind};

const PADDING: f64 = 40.0;
const BACKGROUND: (f64, f64, f64) = (0.071, 0.078, 0.098);
const LINK_COLOR: (f64, f64, f64) = (0.549, 0.584, 0.647);
const TEXT_COLOR: (f64, f64, f64) = (0.918, 0.933, 0.965);

/// Print a finite coordinate with two decimals.
fn num(value: f64) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    format!("{value:.2}")
}

/// Print a colour as three PDF components in the 0..1 range.
fn color((r, g, b): (f64, f64, f64)) -> String {
    format!("{} {} {}", num(r), num(g), num(b))
}

/// Blend a shape colour with the background to emulate the SVG fill opacity.
fn fill_color(shape: &Shape) -> (f64, f64, f64) {
    let (r, g, b) = document::rgb(shape.color);
    let (r, g, b) = (
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    );
    (
        r * 0.18 + BACKGROUND.0 * 0.82,
        g * 0.18 + BACKGROUND.1 * 0.82,
        b * 0.18 + BACKGROUND.2 * 0.82,
    )
}

/// Escape a string for a PDF literal and map it to WinAnsi.
fn escape_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\n' | '\r' => out.push(' '),
            c if (c as u32) < 256 => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

/// Coordinate mapper from canvas units to PDF points (top-left origin).
struct Map {
    x0: f64,
    y0: f64,
    width: f64,
    height: f64,
}

impl Map {
    fn x(&self, value: f64) -> f64 {
        value - self.x0 + PADDING
    }

    fn y(&self, value: f64) -> f64 {
        self.height - (value - self.y0 + PADDING)
    }

    fn point(&self, value: Point) -> (f64, f64) {
        (self.x(value.x), self.y(value.y))
    }
}

/// Draw the arrow head of a connector at its final point.
fn draw_arrow(out: &mut String, map: &Map, previous: Point, last: Point) {
    let angle = (last.y - previous.y).atan2(last.x - previous.x);
    const LENGTH: f64 = 12.0;
    let left = Point::new(
        last.x + LENGTH * (angle + std::f64::consts::PI - 0.45).cos(),
        last.y + LENGTH * (angle + std::f64::consts::PI - 0.45).sin(),
    );
    let right = Point::new(
        last.x + LENGTH * (angle + std::f64::consts::PI + 0.45).cos(),
        last.y + LENGTH * (angle + std::f64::consts::PI + 0.45).sin(),
    );
    let tip = map.point(last);
    let left = map.point(left);
    let right = map.point(right);
    out.push_str(&format!(
        "q {} rg {} {} m {} {} l {} {} l h f Q\n",
        color(LINK_COLOR),
        num(tip.0),
        num(tip.1),
        num(left.0),
        num(left.1),
        num(right.0),
        num(right.1)
    ));
}

/// Draw one connector path with a stroke and an arrow head.
fn draw_connector(out: &mut String, map: &Map, points: &[Point]) {
    if points.len() < 2 {
        return;
    }
    out.push_str(&format!("q {} RG 2 w\n", color(LINK_COLOR)));
    let first = map.point(points[0]);
    out.push_str(&format!("{} {} m\n", num(first.0), num(first.1)));
    for point in &points[1..] {
        let (x, y) = map.point(*point);
        out.push_str(&format!("{x} {y} l\n"));
    }
    out.push_str("S\n");
    let last = points.len() - 1;
    draw_arrow(out, map, points[last - 1], points[last]);
    out.push_str("Q\n");
}

/// Draw an ellipse as four cubic Bézier segments.
fn draw_ellipse(out: &mut String, map: &Map, shape: &Shape, stroke: &str, fill: &str) {
    const K: f64 = 0.552_284_749_8;
    let cx = shape.x + shape.w / 2.0;
    let cy = shape.y + shape.h / 2.0;
    let rx = shape.w / 2.0;
    let ry = shape.h / 2.0;
    let (x, y) = map.point(Point::new(cx + rx, cy));
    out.push_str(&format!("{fill} rg {stroke} RG {} {} m\n", num(x), num(y)));
    let curves = [
        (
            (cx + rx, cy + K * ry),
            (cx + K * rx, cy + ry),
            (cx, cy + ry),
        ),
        (
            (cx - K * rx, cy + ry),
            (cx - rx, cy + K * ry),
            (cx - rx, cy),
        ),
        (
            (cx - rx, cy - K * ry),
            (cx - K * rx, cy - ry),
            (cx, cy - ry),
        ),
        (
            (cx + K * rx, cy - ry),
            (cx + rx, cy - K * ry),
            (cx + rx, cy),
        ),
    ];
    for (a, b, c) in curves {
        let a = map.point(Point::new(a.0, a.1));
        let b = map.point(Point::new(b.0, b.1));
        let c = map.point(Point::new(c.0, c.1));
        out.push_str(&format!(
            "{} {} {} {} {} {} c\n",
            num(a.0),
            num(a.1),
            num(b.0),
            num(b.1),
            num(c.0),
            num(c.1)
        ));
    }
    out.push_str("h B\n");
}

/// Draw one shape and its centred label.
fn draw_shape(out: &mut String, map: &Map, shape: &Shape) {
    let (r, g, b) = document::rgb(shape.color);
    let stroke = format!(
        "{} {} {} RG 1.5 w",
        num(f64::from(r) / 255.0),
        num(f64::from(g) / 255.0),
        num(f64::from(b) / 255.0)
    );
    let fill = color(fill_color(shape));
    match shape.kind {
        ShapeKind::Rectangle | ShapeKind::Note => {
            let (x, y) = map.point(Point::new(shape.x, shape.y));
            out.push_str(&format!(
                "{fill} rg {stroke} {x} {} {} {} re B\n",
                num(y),
                num(shape.w),
                num(shape.h)
            ));
            if shape.kind == ShapeKind::Note {
                let fold = 18.0;
                let a = map.point(Point::new(shape.x + shape.w - fold, shape.y));
                let b = map.point(Point::new(shape.x + shape.w - fold, shape.y + fold));
                let c = map.point(Point::new(shape.x + shape.w, shape.y + fold));
                out.push_str(&format!(
                    "{stroke} {} {} m {} {} l {} {} l S\n",
                    num(a.0),
                    num(a.1),
                    num(b.0),
                    num(b.1),
                    num(c.0),
                    num(c.1)
                ));
            }
        }
        ShapeKind::Ellipse => draw_ellipse(out, map, shape, &stroke, &fill),
        ShapeKind::Diamond => {
            let (cx, cy) = document::center(shape);
            let corners = [
                Point::new(cx, shape.y),
                Point::new(shape.x + shape.w, cy),
                Point::new(cx, shape.y + shape.h),
                Point::new(shape.x, cy),
            ];
            out.push_str(&format!("{fill} rg {stroke}\n"));
            for (index, corner) in corners.iter().enumerate() {
                let (x, y) = map.point(*corner);
                out.push_str(&format!(
                    "{} {} {}\n",
                    num(x),
                    num(y),
                    if index == 0 { 'm' } else { 'l' }
                ));
            }
            out.push_str("h B\n");
        }
    }

    let (cx, cy) = document::center(shape);
    let font_size = 14.0;
    let estimated_width = font_size * 0.52 * shape.label.chars().count() as f64;
    let text_x = cx - estimated_width / 2.0;
    let text_y = cy + font_size * 0.35;
    let (x, y) = map.point(Point::new(text_x, text_y));
    out.push_str(&format!(
        "BT /F1 {} Tf {} rg {} {} Td ({}) Tj ET\n",
        num(font_size),
        color(TEXT_COLOR),
        num(x),
        num(y),
        escape_text(&shape.label)
    ));
}

/// Build the content stream of the single page.
fn content(document: &Document, map: &Map) -> String {
    let mut out = format!(
        "q {} rg 0 0 {} {} re f Q\n",
        color(BACKGROUND),
        num(map.width),
        num(map.height)
    );

    for connector in &document.connectors {
        let points = document.connector_points(connector);
        draw_connector(&mut out, map, &points);
    }
    for shape in &document.shapes {
        draw_shape(&mut out, map, shape);
    }
    out
}

/// Render a document as a standalone vector PDF file.
#[must_use]
pub fn render(document: &Document) -> Vec<u8> {
    let (x0, y0, width, height) = match document.bounds() {
        Some((min_x, min_y, max_x, max_y)) => (
            min_x - PADDING,
            min_y - PADDING,
            (max_x - min_x + 2.0 * PADDING).max(1.0),
            (max_y - min_y + 2.0 * PADDING).max(1.0),
        ),
        None => (0.0, 0.0, 800.0, 600.0),
    };
    let map = Map {
        x0,
        y0,
        width: width + 2.0 * PADDING,
        height: height + 2.0 * PADDING,
    };
    let stream = content(document, &map);
    let page_width = num(map.width);
    let page_height = num(map.height);

    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_width} {page_height}] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
        ),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_owned(),
    ];

    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Routing, ShapeKind};

    #[test]
    fn every_xref_entry_points_at_its_object_header() {
        let pdf = render(&Document::sample());
        let text = String::from_utf8_lossy(&pdf);
        let xref = text.find("\nxref\n").expect("xref table") + 1;
        let section = &text[xref..];
        let mut lines = section.lines();
        assert_eq!(lines.next(), Some("xref"));
        assert_eq!(lines.next(), Some("0 6"));
        assert_eq!(lines.next(), Some("0000000000 65535 f "));
        for index in 1..=5 {
            let entry = lines.next().expect("xref entry");
            let offset: usize = entry[..10].parse().expect("offset digits");
            let header = format!("{index} 0 obj\n");
            assert!(
                pdf[offset..].starts_with(header.as_bytes()),
                "xref entry {index} points at {:?}",
                &pdf[offset..offset + 16]
            );
        }
    }

    #[test]
    fn content_stream_length_matches_its_body() {
        let pdf = render(&Document::sample());
        let text = String::from_utf8_lossy(&pdf);
        let marker = text.find("<< /Length ").expect("content length");
        let length_start = marker + "<< /Length ".len();
        let length_end = text[length_start..].find(" >>").expect("length end") + length_start;
        let declared: usize = text[length_start..length_end]
            .parse()
            .expect("length value");
        let stream_start = text[length_end..].find("stream\n").expect("stream") + length_end + 7;
        let stream_end = text[stream_start..].find("endstream").expect("endstream") + stream_start;
        assert_eq!(declared, stream_end - stream_start);
    }

    #[test]
    fn xref_table_offsets_are_byte_accurate() {
        let pdf = render(&Document::sample());
        let text = String::from_utf8_lossy(&pdf);
        let marker = text.rfind("startxref\n").expect("startxref");
        let xref_offset: usize = text[marker + "startxref\n".len()..]
            .lines()
            .next()
            .expect("offset")
            .trim()
            .parse()
            .expect("number");
        assert!(pdf[xref_offset..].starts_with(b"xref\n0 6\n"));

        let entries: Vec<&str> = text[xref_offset..]
            .lines()
            .filter(|line| line.ends_with("n "))
            .collect();
        assert_eq!(entries.len(), 5);
        for (index, entry) in entries.iter().enumerate() {
            let offset: usize = entry[..10].parse().expect("offset");
            let header = format!("{} 0 obj\n", index + 1);
            assert!(
                pdf[offset..].starts_with(header.as_bytes()),
                "xref entry {} does not point at its object",
                index + 1
            );
        }
    }

    #[test]
    fn pdf_has_a_valid_header_and_trailer() {
        let pdf = render(&Document::sample());
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Type /Page"));
        assert!(text.contains("/Helvetica"));
        assert!(text.contains("trailer"));
    }

    #[test]
    fn xref_offsets_point_at_object_headers() {
        let pdf = render(&Document::sample());
        let text = String::from_utf8_lossy(&pdf);
        let marker = text.rfind("startxref").expect("startxref");
        let digits: String = text[marker + 9..]
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let xref_offset: usize = digits.parse().expect("xref offset");
        assert!(pdf[xref_offset..].starts_with(b"xref\n"));

        for index in 1..=5 {
            let needle = format!("\n{index} 0 obj\n");
            let position = text.find(&needle).expect("object header");
            assert!(position + 1 < pdf.len());
        }
        assert!(pdf.iter().filter(|byte| **byte == b'\n').count() > 10);
    }

    #[test]
    fn sample_contains_vector_paths_and_labels() {
        let pdf = render(&Document::sample());
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("(Start) Tj"));
        assert!(text.contains("(Decision?) Tj"));
        assert!(text.contains("(End) Tj"));
        assert!(text.contains(" re B"));
        assert!(text.contains(" c\n")); // ellipse beziers
        assert!(text.contains(" m\n"));
        assert!(text.contains(" l\n"));
        assert!(text.contains("Tj ET"));
    }

    #[test]
    fn orthogonal_routes_use_multiple_line_segments() {
        let mut document = Document::default();
        let a = document.add_shape(ShapeKind::Rectangle, 0.0, 0.0);
        let b = document.add_shape(ShapeKind::Rectangle, 300.0, 200.0);
        document
            .add_connector_with_routing(a, b, Routing::Orthogonal)
            .expect("connector");
        let pdf = render(&document);
        let text = String::from_utf8_lossy(&pdf);
        let connector_part = text.split("RG 2 w").nth(1).expect("connector stroke");
        let line_ops = connector_part.matches(" l\n").count();
        assert!(
            line_ops >= 3,
            "expected an orthogonal polyline, got {line_ops} line ops"
        );
    }

    #[test]
    fn text_is_escaped_and_empty_documents_are_valid() {
        let mut document = Document::default();
        document.add_shape(ShapeKind::Rectangle, 0.0, 0.0);
        document.shapes[0].label = "A (B) \\ C".into();
        let pdf = render(&document);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("(A \\(B\\) \\\\ C) Tj"));
        let empty = render(&Document::default());
        assert!(empty.ends_with(b"%%EOF\n"));
    }
}
