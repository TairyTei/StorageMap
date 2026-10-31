use std::fmt::Write;

use wh_core::{Point, ShelfId};

use crate::editor::EditorState;
use crate::tools::tool::Tool;
use crate::viewport::Viewport;

pub const SHELF_HEIGHT: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    fn scaled(self, k: f64) -> Rgb {
        let f = |c: u8| (c as f64 * k).round().clamp(0.0, 255.0) as u8;
        Rgb(f(self.0), f(self.1), f(self.2))
    }
}

#[derive(Debug, Clone)]
pub struct DrawShelf {
    pub commands: String,
    pub color: Rgb,
    pub label: String,
    pub lx: f32,
    pub ly: f32,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct DrawLabel {
    pub x: f32,
    pub y: f32,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub grid: String,
    pub floor: String,
    pub draft: String,
    pub markers: String,
    pub route: String,
    pub entrance: String,
    pub shelves: Vec<DrawShelf>,
    pub labels: Vec<DrawLabel>,
    pub preview_from: Option<(f32, f32)>,
    pub px_per_m: f32,
    pub hit: Vec<(ShelfId, Vec<(f64, f64)>)>,
}

pub fn build_2d(state: &EditorState, vp: &Viewport) -> Scene {
    let mut sc = Scene {
        px_per_m: vp.scale as f32,
        ..Scene::default()
    };
    let wh = &state.warehouse;

    sc.grid = grid_commands(vp);

    if state.has_outline() {
        let pts: Vec<_> = wh.outline.points.iter().map(|&p| vp.to_screen(p)).collect();
        sc.floor = poly(&pts, true);
        sc.labels.extend(edge_labels(&wh.outline.points, vp, true));
    } else if state.is_drawing() {
        let pts: Vec<_> = state.draft().iter().map(|&p| vp.to_screen(p)).collect();
        sc.draft = poly(&pts, false);
        for &(x, y) in &pts {
            sc.markers.push_str(&square(x, y, 4.0));
            sc.markers.push(' ');
        }
        sc.labels.extend(edge_labels(state.draft(), vp, false));
        if state.tool == Tool::Outline {
            let (x, y) = *pts.last().expect("draft is not empty");
            sc.preview_from = Some((x as f32, y as f32));
        }
    }

    if state.has_outline() {
        let (x, y) = vp.to_screen(wh.entrance);
        sc.entrance = diamond(x, y, 8.0);
    }

    if let Some(a) = state
        .selected_shelf
        .and_then(|id| state.access.iter().find(|a| a.shelf == id))
    {
        if a.route.len() >= 2 {
            let pts: Vec<_> = a.route.iter().map(|&p| vp.to_screen(p)).collect();
            sc.route = poly(&pts, false);
        }
    }

    for s in &wh.shelves {
        let corners: Vec<_> = s
            .footprint
            .corners()
            .iter()
            .map(|&p| vp.to_screen(p))
            .collect();
        let mut commands = poly(&corners, true);

        let n = s.footprint.front_dir();
        let edge_mid = s.footprint.center + n * s.footprint.half_d;
        let (x0, y0) = vp.to_screen(edge_mid);
        let (x1, y1) = vp.to_screen(edge_mid + n * 0.4);
        let _ = write!(commands, " M {x0:.1} {y0:.1} L {x1:.1} {y1:.1}");
        if s.double_sided {
            let back_mid = s.footprint.center - n * s.footprint.half_d;
            let (bx0, by0) = vp.to_screen(back_mid);
            let (bx1, by1) = vp.to_screen(back_mid - n * 0.4);
            let _ = write!(commands, " M {bx0:.1} {by0:.1} L {bx1:.1} {by1:.1}");
        }

        let (cx, cy) = vp.to_screen(s.footprint.center);
        sc.shelves.push(DrawShelf {
            commands,
            color: shelf_color(state, s.id),
            label: shelf_label(state, s.id),
            lx: cx as f32,
            ly: cy as f32,
            selected: state.selected_shelf == Some(s.id),
        });
    }
    if state.tool == Tool::Measure {
        draw_ruler(&mut sc, state, vp);
    }
    sc
}

const ISO_C: f64 = 0.866_025_403_784_438_6; // cos 30°
const ISO_S: f64 = 0.5; // sin 30°

fn iso_raw(p: Point, z: f64) -> (f64, f64) {
    ((p.x - p.y) * ISO_C, -(p.x + p.y) * ISO_S - z)
}

pub fn build_3d(state: &EditorState, size: (f64, f64)) -> Scene {
    let mut sc = Scene {
        px_per_m: 1.0,
        ..Scene::default()
    };
    let wh = &state.warehouse;

    let mut raw_pts: Vec<(f64, f64)> = wh.outline.points.iter().map(|&p| iso_raw(p, 0.0)).collect();
    for s in &wh.shelves {
        for c in s.footprint.corners() {
            raw_pts.push(iso_raw(c, 0.0));
            raw_pts.push(iso_raw(c, SHELF_HEIGHT));
        }
    }
    if raw_pts.is_empty() {
        raw_pts = [(0.0, 0.0), (20.0, 0.0), (20.0, 15.0), (0.0, 15.0)]
            .iter()
            .map(|&(x, y)| iso_raw(Point::new(x, y), 0.0))
            .collect();
    }
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in &raw_pts {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let margin = 40.0;
    let scale = ((size.0 - 2.0 * margin) / (max_x - min_x).max(1.0))
        .min((size.1 - 2.0 * margin) / (max_y - min_y).max(1.0))
        .max(1.0);
    let (cx, cy) = ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let proj = |p: Point, z: f64| -> (f64, f64) {
        let (rx, ry) = iso_raw(p, z);
        (
            (rx - cx) * scale + size.0 * 0.5,
            (ry - cy) * scale + size.1 * 0.5,
        )
    };

    if state.has_outline() {
        let pts: Vec<_> = wh.outline.points.iter().map(|&p| proj(p, 0.0)).collect();
        sc.floor = poly(&pts, true);
        let (x, y) = proj(wh.entrance, 0.0);
        sc.entrance = diamond(x, y, 8.0);
    }

    if let Some(a) = state
        .selected_shelf
        .and_then(|id| state.access.iter().find(|a| a.shelf == id))
    {
        if a.route.len() >= 2 {
            let pts: Vec<_> = a.route.iter().map(|&p| proj(p, 0.02)).collect();
            sc.route = poly(&pts, false);
        }
    }

    let mut order: Vec<_> = wh.shelves.iter().collect();
    order.sort_by(|a, b| {
        let ka = a.footprint.center.x + a.footprint.center.y;
        let kb = b.footprint.center.x + b.footprint.center.y;
        kb.total_cmp(&ka).then(a.id.cmp(&b.id))
    });

    let to_camera = Point::new(-1.0, -1.0);
    let light = Point::new(-0.6, -0.8);

    for s in order {
        let base = shelf_color(state, s.id);
        let selected = state.selected_shelf == Some(s.id);
        let c = s.footprint.corners();

        for i in 0..4 {
            let (a, b) = (c[i], c[(i + 1) % 4]);
            let d = b - a;
            let Some(n) = Point::new(d.y, -d.x).normalized() else {
                continue;
            };
            if n.dot(to_camera) <= 1e-6 {
                continue;
            }
            let face = [
                proj(a, 0.0),
                proj(b, 0.0),
                proj(b, SHELF_HEIGHT),
                proj(a, SHELF_HEIGHT),
            ];
            let shade = 0.5 + 0.5 * n.dot(light).max(0.0);
            sc.shelves.push(DrawShelf {
                commands: poly(&face, true),
                color: base.scaled(shade),
                label: String::new(),
                lx: 0.0,
                ly: 0.0,
                selected,
            });
            sc.hit.push((s.id, face.to_vec()));
        }

        let top: Vec<_> = c.iter().map(|&p| proj(p, SHELF_HEIGHT)).collect();
        let (lx, ly) = proj(s.footprint.center, SHELF_HEIGHT);
        sc.shelves.push(DrawShelf {
            commands: poly(&top, true),
            color: base.scaled(1.12),
            label: shelf_label(state, s.id),
            lx: lx as f32,
            ly: ly as f32,
            selected,
        });
        sc.hit.push((s.id, top));
    }
    sc
}

pub fn point_in_poly(px: f64, py: f64, poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % n];
        if (y1 > py) != (y2 > py) && px < x1 + (py - y1) * (x2 - x1) / (y2 - y1) {
            inside = !inside;
        }
    }
    inside
}

fn shelf_label(state: &EditorState, id: ShelfId) -> String {
    match state.warehouse.shelf(id) {
        Some(s) => format!("{} {}/{}", s.label, state.shelf_used(id), s.capacity),
        None => String::new(),
    }
}

fn shelf_color(state: &EditorState, id: ShelfId) -> Rgb {
    match state.access.iter().find(|a| a.shelf == id) {
        None => Rgb(88, 112, 150),
        Some(a) if a.distance.is_none() => Rgb(110, 60, 60),
        Some(a) => heat(a.score),
    }
}

fn heat(score: f64) -> Rgb {
    let (red, yellow, green) = (
        (205.0, 70.0, 60.0),
        (225.0, 190.0, 60.0),
        (70.0, 175.0, 95.0),
    );
    let t = score.clamp(0.0, 1.0);
    let (a, b, k) = if t < 0.5 {
        (red, yellow, t * 2.0)
    } else {
        (yellow, green, (t - 0.5) * 2.0)
    };
    let mix = |x: f64, y: f64| (x + (y - x) * k).round() as u8;
    Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

fn edge_labels(pts: &[Point], vp: &Viewport, closed: bool) -> Vec<DrawLabel> {
    let n = pts.len();
    let count = if closed { n } else { n.saturating_sub(1) };
    (0..count)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let (mx, my) = vp.to_screen(Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5));
            DrawLabel {
                x: mx as f32,
                y: my as f32,
                text: format!("{:.2} м", a.distance(b)),
            }
        })
        .collect()
}

fn grid_commands(vp: &Viewport) -> String {
    let step = [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]
        .into_iter()
        .find(|s| s * vp.scale >= 14.0)
        .unwrap_or(100.0);
    let (w, h) = vp.size;
    let top_left = vp.to_world(0.0, 0.0);
    let bottom_right = vp.to_world(w, h);
    let mut out = String::new();

    let mut x = (top_left.x / step).floor() * step;
    while x <= bottom_right.x {
        let (sx, _) = vp.to_screen(Point::new(x, 0.0));
        let _ = write!(out, "M {sx:.1} 0 L {sx:.1} {h:.1} ");
        x += step;
    }
    let mut y = (bottom_right.y / step).floor() * step;
    while y <= top_left.y {
        let (_, sy) = vp.to_screen(Point::new(0.0, y));
        let _ = write!(out, "M 0 {sy:.1} L {w:.1} {sy:.1} ");
        y += step;
    }
    out
}

fn poly(pts: &[(f64, f64)], close: bool) -> String {
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let _ = write!(s, "{}{x:.1} {y:.1}", if i == 0 { "M " } else { " L " });
    }
    if close && !pts.is_empty() {
        s.push_str(" Z");
    }
    s
}

fn square(x: f64, y: f64, r: f64) -> String {
    poly(
        &[
            (x - r, y - r),
            (x + r, y - r),
            (x + r, y + r),
            (x - r, y + r),
        ],
        true,
    )
}

fn diamond(x: f64, y: f64, r: f64) -> String {
    poly(&[(x, y - r), (x + r, y), (x, y + r), (x - r, y)], true)
}
fn draw_ruler(sc: &mut Scene, state: &EditorState, vp: &Viewport) {
    let ruler = &state.tool_context.measure;

    if let Some((a, b)) = ruler.segment() {
        let (pa, pb) = (vp.to_screen(a), vp.to_screen(b));
        sc.draft.push(' ');
        sc.draft.push_str(&poly(&[pa, pb], false));
        for (x, y) in [pa, pb] {
            sc.markers.push_str(&square(x, y, 4.0));
            sc.markers.push(' ');
        }
        sc.labels.extend(edge_labels(&[a, b], vp, false));
    } else if let Some(a) = ruler.anchor() {
        let (x, y) = vp.to_screen(a);
        sc.markers.push_str(&square(x, y, 4.0));
        sc.markers.push(' ');
        sc.preview_from.get_or_insert((x as f32, y as f32));
    }
}
#[cfg(test)]
mod tests {
    use crate::tools::tool::ToolContext;

    use super::*;
    use wh_core::sample::demo_snapshot;

    fn demo() -> EditorState {
        let mut s = EditorState::from_snapshot(demo_snapshot());
        assert!(s.analyze());
        s
    }

    #[test]
    fn plan_has_all_layers() {
        let state = demo();
        let mut vp = Viewport::default();
        let (min, max) = state.bounds();
        vp.fit(min, max);
        let sc = build_2d(&state, &vp);
        assert_eq!(sc.shelves.len(), 6);
        assert!(sc.floor.starts_with("M ") && sc.floor.ends_with('Z'));
        assert!(!sc.entrance.is_empty());
        assert_eq!(
            sc.labels.len(),
            6,
            "по подписи на каждое ребро L-образного контура"
        );
    }

    #[test]
    fn route_is_drawn_for_selected_shelf_only() {
        let mut state = demo();
        let vp = Viewport::default();
        assert!(build_2d(&state, &vp).route.is_empty());
        state.selected_shelf = Some(ShelfId(5));
        assert!(!build_2d(&state, &vp).route.is_empty());
    }

    #[test]
    fn draft_outline_shows_ruler() {
        let mut state = EditorState::new_empty();
        state.set_tool(Tool::from_index(0));
        state.click(Point::new(0.0, 0.0), 0.3);
        state.click(Point::new(5.0, 0.0), 0.3);
        let sc = build_2d(&state, &Viewport::default());
        assert!(sc.preview_from.is_some());
        assert_eq!(sc.labels.len(), 1);
        assert_eq!(sc.labels[0].text, "5.00 м");
    }

    #[test]
    fn isometric_boxes_show_two_sides_and_top() {
        let state = demo();
        let sc = build_3d(&state, (900.0, 700.0));
        assert_eq!(sc.shelves.len(), 6 * 3);
        assert_eq!(sc.hit.len(), 6 * 3);
    }

    #[test]
    fn hit_test_finds_top_face() {
        let state = demo();
        let sc = build_3d(&state, (900.0, 700.0));
        let top_of_s1 = sc
            .shelves
            .iter()
            .position(|s| s.label.starts_with("S1 "))
            .unwrap();
        let (lx, ly) = (
            sc.shelves[top_of_s1].lx as f64,
            sc.shelves[top_of_s1].ly as f64,
        );
        let (id, _) = sc
            .hit
            .iter()
            .rev()
            .find(|(_, p)| point_in_poly(lx, ly, p))
            .unwrap();
        assert_eq!(*id, ShelfId(1));
    }

    #[test]
    fn heat_gradient_endpoints() {
        assert_eq!(heat(0.0), Rgb(205, 70, 60));
        assert_eq!(heat(1.0), Rgb(70, 175, 95));
    }

    #[test]
    fn point_in_poly_basic() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(point_in_poly(5.0, 5.0, &sq));
        assert!(!point_in_poly(15.0, 5.0, &sq));
    }
}
