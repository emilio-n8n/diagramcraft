//! Pure geometry helpers shared by the canvas and the exporters.
//!
//! Every function in this module works on plain canvas coordinates (f64,
//! Y grows downwards) and has no UI dependency. Keeping the maths here makes
//! resize, routing, alignment and zoom-to-fit directly unit-testable.

/// Smallest accepted width or height of a shape, in canvas units.
pub const MIN_SHAPE_SIZE: f64 = 20.0;

/// One of the eight resize grips shown around a selected shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// Top edge, centre.
    N,
    /// Top-right corner.
    NE,
    /// Right edge, centre.
    E,
    /// Bottom-right corner.
    SE,
    /// Bottom edge, centre.
    S,
    /// Bottom-left corner.
    SW,
    /// Left edge, centre.
    W,
    /// Top-left corner.
    NW,
}

impl Handle {
    /// All handles, clockwise from north.
    pub const ALL: [Self; 8] = [
        Self::N,
        Self::NE,
        Self::E,
        Self::SE,
        Self::S,
        Self::SW,
        Self::W,
        Self::NW,
    ];

    /// Whether dragging this handle moves the left edge.
    #[must_use]
    pub const fn moves_left(self) -> bool {
        matches!(self, Self::W | Self::NW | Self::SW)
    }

    /// Whether dragging this handle moves the right edge.
    #[must_use]
    pub const fn moves_right(self) -> bool {
        matches!(self, Self::E | Self::NE | Self::SE)
    }

    /// Whether dragging this handle moves the top edge.
    #[must_use]
    pub const fn moves_top(self) -> bool {
        matches!(self, Self::N | Self::NE | Self::NW)
    }

    /// Whether dragging this handle moves the bottom edge.
    #[must_use]
    pub const fn moves_bottom(self) -> bool {
        matches!(self, Self::S | Self::SE | Self::SW)
    }

    /// Position of the grip on a rectangle `(x, y, w, h)`.
    #[must_use]
    #[allow(dead_code)]
    pub const fn point(self, rect: (f64, f64, f64, f64)) -> (f64, f64) {
        let (x, y, w, h) = rect;
        match self {
            Self::N => (x + w / 2.0, y),
            Self::NE => (x + w, y),
            Self::E => (x + w, y + h / 2.0),
            Self::SE => (x + w, y + h),
            Self::S => (x + w / 2.0, y + h),
            Self::SW => (x, y + h),
            Self::W => (x, y + h / 2.0),
            Self::NW => (x, y),
        }
    }
}

/// Resize a rectangle by dragging `handle` by `(dx, dy)` canvas units.
///
/// The opposite edges stay pinned and the result never gets smaller than
/// [`MIN_SHAPE_SIZE`] on either axis.
#[must_use]
pub fn resize_rect(
    rect: (f64, f64, f64, f64),
    handle: Handle,
    dx: f64,
    dy: f64,
) -> (f64, f64, f64, f64) {
    let (x, y, w, h) = rect;
    let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
    if handle.moves_left() {
        left += dx;
    }
    if handle.moves_right() {
        right += dx;
    }
    if handle.moves_top() {
        top += dy;
    }
    if handle.moves_bottom() {
        bottom += dy;
    }

    if right - left < MIN_SHAPE_SIZE {
        if handle.moves_left() {
            left = right - MIN_SHAPE_SIZE;
        } else {
            right = left + MIN_SHAPE_SIZE;
        }
    }
    if bottom - top < MIN_SHAPE_SIZE {
        if handle.moves_top() {
            top = bottom - MIN_SHAPE_SIZE;
        } else {
            bottom = top + MIN_SHAPE_SIZE;
        }
    }
    (left, top, right - left, bottom - top)
}

/// Distance from a point to a line segment, in canvas units.
#[must_use]
pub fn point_segment_distance(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let vx = bx - ax;
    let vy = by - ay;
    let length_squared = vx * vx + vy * vy;
    let t = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (((px - ax) * vx + (py - ay) * vy) / length_squared).clamp(0.0, 1.0)
    };
    let closest_x = ax + t * vx;
    let closest_y = ay + t * vy;
    ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
}

/// Build an axis-aligned route from `start` to `end`.
///
/// Without waypoints a single elbow is inserted, choosing the dominant axis
/// of the two endpoints. With waypoints, a horizontal corner is inserted
/// before each target so every segment stays orthogonal.
#[must_use]
pub fn orthogonal_route(
    start: (f64, f64),
    end: (f64, f64),
    waypoints: &[(f64, f64)],
) -> Vec<(f64, f64)> {
    if waypoints.is_empty() {
        let middle_x = (start.0 + end.0) / 2.0;
        let middle_y = (start.1 + end.1) / 2.0;
        if (end.0 - start.0).abs() >= (end.1 - start.1).abs() {
            return vec![start, (middle_x, start.1), (middle_x, end.1), end];
        }
        return vec![start, (start.0, middle_y), (end.0, middle_y), end];
    }

    let mut points = vec![start];
    for target in waypoints.iter().copied().chain(std::iter::once(end)) {
        let previous = points.last().copied().unwrap_or(start);
        let corner = (target.0, previous.1);
        if corner != previous {
            points.push(corner);
        }
        if target != previous {
            points.push(target);
        }
    }
    points
}

/// Horizontal or vertical axis used by distribution commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Distribute along X.
    Horizontal,
    /// Distribute along Y.
    Vertical,
}

impl Axis {
    /// Short label used on toolbar buttons.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "Space H",
            Self::Vertical => "Space V",
        }
    }
}

/// Align rectangles to the extremes or centre of their collective bounds.
pub fn align_rects(rects: &mut [(f64, f64, f64, f64)], mode: AlignMode) {
    if rects.len() < 2 {
        return;
    }
    let min_x = rects
        .iter()
        .map(|rect| rect.0)
        .fold(f64::INFINITY, f64::min);
    let max_x = rects
        .iter()
        .map(|rect| rect.0 + rect.2)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = rects
        .iter()
        .map(|rect| rect.1)
        .fold(f64::INFINITY, f64::min);
    let max_y = rects
        .iter()
        .map(|rect| rect.1 + rect.3)
        .fold(f64::NEG_INFINITY, f64::max);
    let centre_x = (min_x + max_x) / 2.0;
    let centre_y = (min_y + max_y) / 2.0;

    for rect in rects {
        match mode {
            AlignMode::Left => rect.0 = min_x,
            AlignMode::CenterH => rect.0 = centre_x - rect.2 / 2.0,
            AlignMode::Right => rect.0 = max_x - rect.2,
            AlignMode::Top => rect.1 = min_y,
            AlignMode::CenterV => rect.1 = centre_y - rect.3 / 2.0,
            AlignMode::Bottom => rect.1 = max_y - rect.3,
        }
    }
}

/// Alignment target for a group of shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    /// Align left edges.
    Left,
    /// Centre horizontally.
    CenterH,
    /// Align right edges.
    Right,
    /// Align top edges.
    Top,
    /// Centre vertically.
    CenterV,
    /// Align bottom edges.
    Bottom,
}

impl AlignMode {
    /// All alignment modes, in the order shown in the UI.
    pub const ALL: [Self; 6] = [
        Self::Left,
        Self::CenterH,
        Self::Right,
        Self::Top,
        Self::CenterV,
        Self::Bottom,
    ];

    /// Short label used on toolbar buttons.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::CenterH => "Center H",
            Self::Right => "Right",
            Self::Top => "Top",
            Self::CenterV => "Center V",
            Self::Bottom => "Bottom",
        }
    }
}

/// Distribute rectangles so the gaps between them become equal.
///
/// The first and last rectangle keep their positions; only the ones in
/// between move.
pub fn distribute_rects(rects: &mut [(f64, f64, f64, f64)], axis: Axis) {
    if rects.len() < 3 {
        return;
    }
    match axis {
        Axis::Horizontal => {
            rects.sort_by(|a, b| a.0.total_cmp(&b.0));
            let first_edge = rects.first().map_or(0.0, |rect| rect.0);
            let last_edge = rects.last().map_or(first_edge, |rect| rect.0 + rect.2);
            let used: f64 = rects.iter().map(|rect| rect.2).sum();
            let gap = (last_edge - first_edge - used) / (rects.len() as f64 - 1.0);
            let mut cursor = first_edge;
            for rect in rects.iter_mut() {
                rect.0 = cursor;
                cursor += rect.2 + gap;
            }
        }
        Axis::Vertical => {
            rects.sort_by(|a, b| a.1.total_cmp(&b.1));
            let first_edge = rects.first().map_or(0.0, |rect| rect.1);
            let last_edge = rects.last().map_or(first_edge, |rect| rect.1 + rect.3);
            let used: f64 = rects.iter().map(|rect| rect.3).sum();
            let gap = (last_edge - first_edge - used) / (rects.len() as f64 - 1.0);
            let mut cursor = first_edge;
            for rect in rects.iter_mut() {
                rect.1 = cursor;
                cursor += rect.3 + gap;
            }
        }
    }
}

/// Compute the pan (relative to the viewport's top-left) and zoom that fit a
/// world-space bounding box inside a viewport with the given padding.
#[must_use]
pub fn fit_view(
    bounds: (f64, f64, f64, f64),
    viewport: (f32, f32),
    padding: f32,
    min_zoom: f32,
    max_zoom: f32,
) -> (f32, f32, f32) {
    let (min_x, min_y, max_x, max_y) = bounds;
    let width = (max_x - min_x).max(1.0) as f32;
    let height = (max_y - min_y).max(1.0) as f32;
    let available_x = (viewport.0 - 2.0 * padding).max(1.0);
    let available_y = (viewport.1 - 2.0 * padding).max(1.0);
    let zoom = (available_x / width)
        .min(available_y / height)
        .clamp(min_zoom, max_zoom);
    let centre_x = ((min_x + max_x) / 2.0) as f32;
    let centre_y = ((min_y + max_y) / 2.0) as f32;
    let pan_x = viewport.0 / 2.0 - centre_x * zoom;
    let pan_y = viewport.1 / 2.0 - centre_y * zoom;
    (pan_x, pan_y, zoom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_handle_moves_the_expected_edges() {
        let rect = (100.0, 100.0, 200.0, 100.0);
        assert_eq!(
            resize_rect(rect, Handle::E, 20.0, 0.0),
            (100.0, 100.0, 220.0, 100.0)
        );
        assert_eq!(
            resize_rect(rect, Handle::W, 20.0, 0.0),
            (120.0, 100.0, 180.0, 100.0)
        );
        assert_eq!(
            resize_rect(rect, Handle::N, 0.0, -10.0),
            (100.0, 90.0, 200.0, 110.0)
        );
        assert_eq!(
            resize_rect(rect, Handle::S, 0.0, -10.0),
            (100.0, 100.0, 200.0, 90.0)
        );
        assert_eq!(
            resize_rect(rect, Handle::NW, 10.0, 10.0),
            (110.0, 110.0, 190.0, 90.0)
        );
        assert_eq!(
            resize_rect(rect, Handle::SE, 10.0, 10.0),
            (100.0, 100.0, 210.0, 110.0)
        );
    }

    #[test]
    fn resize_enforces_a_minimum_size() {
        let rect = (0.0, 0.0, 40.0, 40.0);
        let (_, _, w, h) = resize_rect(rect, Handle::SE, -100.0, -100.0);
        assert_eq!((w, h), (MIN_SHAPE_SIZE, MIN_SHAPE_SIZE));
        let (x, y, w, h) = resize_rect(rect, Handle::NW, 100.0, 100.0);
        assert_eq!((x + w, y + h), (40.0, 40.0));
        assert_eq!((w, h), (MIN_SHAPE_SIZE, MIN_SHAPE_SIZE));
    }

    #[test]
    fn handle_points_cover_all_corners_and_edges() {
        let rect = (10.0, 20.0, 100.0, 50.0);
        assert_eq!(Handle::N.point(rect), (60.0, 20.0));
        assert_eq!(Handle::NE.point(rect), (110.0, 20.0));
        assert_eq!(Handle::E.point(rect), (110.0, 45.0));
        assert_eq!(Handle::SE.point(rect), (110.0, 70.0));
        assert_eq!(Handle::S.point(rect), (60.0, 70.0));
        assert_eq!(Handle::SW.point(rect), (10.0, 70.0));
        assert_eq!(Handle::W.point(rect), (10.0, 45.0));
        assert_eq!(Handle::NW.point(rect), (10.0, 20.0));
    }

    #[test]
    fn point_segment_distance_handles_interior_and_endpoints() {
        assert_eq!(point_segment_distance(5.0, 5.0, 0.0, 0.0, 10.0, 0.0), 5.0);
        assert_eq!(point_segment_distance(-5.0, 0.0, 0.0, 0.0, 10.0, 0.0), 5.0);
        assert_eq!(point_segment_distance(15.0, 0.0, 0.0, 0.0, 10.0, 0.0), 5.0);
        assert_eq!(point_segment_distance(3.0, 4.0, 3.0, 4.0, 3.0, 4.0), 0.0);
    }

    #[test]
    fn orthogonal_route_without_waypoints_is_axis_aligned() {
        let route = orthogonal_route((0.0, 0.0), (100.0, 40.0), &[]);
        assert_eq!(route.len(), 4);
        for pair in route.windows(2) {
            assert!(pair[0].0 == pair[1].0 || pair[0].1 == pair[1].1);
        }
        assert_eq!(route[1].0, 50.0);
        let vertical = orthogonal_route((0.0, 0.0), (10.0, 100.0), &[]);
        assert!(
            vertical
                .windows(2)
                .all(|pair| pair[0].0 == pair[1].0 || pair[0].1 == pair[1].1)
        );
    }

    #[test]
    fn waypoints_are_visited_and_keep_the_route_orthogonal() {
        let waypoints = [(30.0, 80.0), (90.0, 20.0)];
        let route = orthogonal_route((0.0, 0.0), (100.0, 60.0), &waypoints);
        assert!(route.contains(&waypoints[0]));
        assert!(route.contains(&waypoints[1]));
        assert_eq!(route.first(), Some(&(0.0, 0.0)));
        assert_eq!(route.last(), Some(&(100.0, 60.0)));
        assert!(
            route
                .windows(2)
                .all(|pair| pair[0].0 == pair[1].0 || pair[0].1 == pair[1].1)
        );
    }

    #[test]
    fn align_and_distribute_act_on_the_collective_bounds() {
        let mut rects = [
            (0.0, 0.0, 20.0, 10.0),
            (50.0, 30.0, 20.0, 10.0),
            (80.0, 10.0, 20.0, 10.0),
        ];
        align_rects(&mut rects, AlignMode::Left);
        assert!(rects.iter().all(|rect| rect.0 == 0.0));
        align_rects(&mut rects, AlignMode::Bottom);
        assert!(rects.iter().all(|rect| rect.1 + rect.3 == 40.0));

        let mut rects = [
            (0.0, 0.0, 10.0, 10.0),
            (10.0, 0.0, 10.0, 10.0),
            (100.0, 0.0, 10.0, 10.0),
        ];
        distribute_rects(&mut rects, Axis::Horizontal);
        assert_eq!(rects[0].0, 0.0);
        assert_eq!(rects[2].0, 100.0);
        assert_eq!(rects[1].0, 50.0);
    }

    #[test]
    fn fit_view_centres_and_clamps_zoom() {
        let (pan_x, pan_y, zoom) =
            fit_view((0.0, 0.0, 200.0, 100.0), (800.0, 600.0), 40.0, 0.25, 4.0);
        assert_eq!(zoom, 3.6);
        assert_eq!(pan_x, 400.0 - 100.0 * zoom);
        assert_eq!(pan_y, 300.0 - 50.0 * zoom);
        let (_, _, clamped) = fit_view((0.0, 0.0, 1.0, 1.0), (1000.0, 1000.0), 40.0, 0.25, 4.0);
        assert_eq!(clamped, 4.0);
    }
}
