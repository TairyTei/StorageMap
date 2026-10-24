//! Кратчайший путь внутри многоугольника с прямоугольными препятствиями.
//!
//! Метод: граф видимости. Вершины графа — углы (раздутых на `clearance`) препятствий
//! и «вогнутые» вершины контура, сдвинутые внутрь помещения. Рёбра — пары вершин,
//! видимые друг другу. Кратчайший ломаный путь всегда проходит через такие вершины.
//!
//! Стоимость построения ≈ O(N² · (E + S)), где N — число вершин, E — рёбра контура,
//! S — число препятствий. Для сотен стеллажей это доли секунды; граф строится один раз
//! на планировку и переиспользуется для любого числа целей.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use wh_core::geometry::{OrientedRect, Point, Polygon, EPS};

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub points: Vec<Point>,
    pub length: f64,
}

pub struct Navigator {
    outline: Polygon,
    obstacles: Vec<OrientedRect>,
    nodes: Vec<Point>,
    adj: Vec<Vec<(usize, f64)>>,
}

impl Navigator {
    /// `obstacles` — уже раздутые на ширину прохода. `clearance` нужен только для
    /// сдвига вогнутых углов контура.
    pub fn new(outline: Polygon, obstacles: Vec<OrientedRect>, clearance: f64) -> Self {
        let nodes = build_nodes(&outline, &obstacles, clearance);
        let mut nav = Navigator { outline, obstacles, nodes, adj: Vec::new() };
        nav.adj = nav.build_adjacency();
        nav
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Свободен ли прямой отрезок `a→b`.
    pub fn visible(&self, a: Point, b: Point) -> bool {
        if a.distance(b) < EPS {
            return true;
        }
        if self.obstacles.iter().any(|o| o.segment_crosses_interior(a, b)) {
            return false;
        }
        self.outline.contains_segment(a, b)
    }

    fn build_adjacency(&self) -> Vec<Vec<(usize, f64)>> {
        let n = self.nodes.len();
        let mut adj = vec![Vec::new(); n];
        for i in 0..n {
            for j in (i + 1)..n {
                if self.visible(self.nodes[i], self.nodes[j]) {
                    let w = self.nodes[i].distance(self.nodes[j]);
                    adj[i].push((j, w));
                    adj[j].push((i, w));
                }
            }
        }
        adj
    }

    /// Дейкстра от `start` до всех вершин графа. Результат переиспользуется для
    /// любого количества целей.
    pub fn routes_from(&self, start: Point) -> RouteField<'_> {
        let n = self.nodes.len();
        let mut dist = vec![f64::INFINITY; n];
        let mut prev: Vec<Option<usize>> = vec![None; n];
        let mut heap = BinaryHeap::new();

        for (i, &p) in self.nodes.iter().enumerate() {
            if self.visible(start, p) {
                let d = start.distance(p);
                dist[i] = d;
                heap.push(State { cost: d, node: i });
            }
        }
        while let Some(State { cost, node }) = heap.pop() {
            if cost > dist[node] {
                continue;
            }
            for &(nb, w) in &self.adj[node] {
                let nd = cost + w;
                if nd < dist[nb] {
                    dist[nb] = nd;
                    prev[nb] = Some(node);
                    heap.push(State { cost: nd, node: nb });
                }
            }
        }
        RouteField { nav: self, start, dist, prev }
    }
}

pub struct RouteField<'a> {
    nav: &'a Navigator,
    start: Point,
    dist: Vec<f64>,
    prev: Vec<Option<usize>>,
}

impl RouteField<'_> {
    /// Кратчайший маршрут до произвольной точки. `None` — точка недостижима.
    pub fn route_to(&self, target: Point) -> Option<Route> {
        if self.nav.visible(self.start, target) {
            return Some(Route {
                points: vec![self.start, target],
                length: self.start.distance(target),
            });
        }
        let mut best: Option<(usize, f64)> = None;
        for (i, &p) in self.nav.nodes.iter().enumerate() {
            if !self.dist[i].is_finite() {
                continue;
            }
            let total = self.dist[i] + p.distance(target);
            if best.map_or(true, |(_, b)| total < b) && self.nav.visible(p, target) {
                best = Some((i, total));
            }
        }
        let (last, length) = best?;

        let mut pts = vec![target, self.nav.nodes[last]];
        let mut cur = last;
        while let Some(p) = self.prev[cur] {
            pts.push(self.nav.nodes[p]);
            cur = p;
        }
        pts.push(self.start);
        pts.reverse();
        Some(Route { points: pts, length })
    }
}

// ───────────────────── internals ─────────────────────

fn build_nodes(outline: &Polygon, obstacles: &[OrientedRect], clearance: f64) -> Vec<Point> {
    let mut raw: Vec<Point> = obstacles.iter().flat_map(|o| o.corners()).collect();

    // Вогнутые вершины контура: путь огибает их с отступом внутрь помещения.
    let pts = &outline.points;
    let n = pts.len();
    let ccw = outline.is_ccw();
    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let v = pts[i];
        let next = pts[(i + 1) % n];
        let cross = (v - prev).cross(next - v);
        let reflex = if ccw { cross < -EPS } else { cross > EPS };
        if !reflex {
            continue;
        }
        let (Some(u1), Some(u2)) = ((prev - v).normalized(), (next - v).normalized()) else {
            continue;
        };
        let Some(bisector) = (u1 + u2).normalized() else {
            continue;
        };
        // Угол между рёбрами < 180° лежит вне помещения; внутрь — против биссектрисы.
        let sin_half = ((1.0 - u1.dot(u2)) * 0.5).max(0.0).sqrt();
        let shift = (clearance / sin_half.max(0.25)).min(clearance * 4.0);
        raw.push(v - bisector * shift);
    }

    let mut nodes: Vec<Point> = Vec::new();
    for p in raw {
        let usable = outline.contains(p)
            && !obstacles.iter().any(|o| o.contains_strict(p))
            && !nodes.iter().any(|q| q.distance(p) < 1e-6);
        if usable {
            nodes.push(p);
        }
    }
    nodes
}

#[derive(PartialEq)]
struct State {
    cost: f64,
    node: usize,
}
impl Eq for State {}
impl Ord for State {
    // Обратный порядок: BinaryHeap — max-heap, нужен min по стоимости.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| self.node.cmp(&other.node))
    }
}
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: f64) -> Polygon {
        Polygon::new(vec![
            Point::new(0.0, 0.0),
            Point::new(size, 0.0),
            Point::new(size, size),
            Point::new(0.0, size),
        ])
    }

    fn polyline_len(pts: &[Point]) -> f64 {
        pts.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    #[test]
    fn open_room_is_straight_line() {
        let nav = Navigator::new(square(10.0), vec![], 0.0);
        let r = nav.routes_from(Point::new(1.0, 1.0)).route_to(Point::new(9.0, 7.0)).unwrap();
        assert_eq!(r.points.len(), 2);
        assert!((r.length - 10.0).abs() < 1e-9);
    }

    #[test]
    fn wall_forces_detour() {
        // Вертикальная перегородка x∈[4.5,5.5], y∈[1,9] в комнате 10×10.
        let wall = OrientedRect::new(Point::new(5.0, 5.0), 1.0, 8.0, 0.0);
        let nav = Navigator::new(square(10.0), vec![wall], 0.0);
        let r = nav.routes_from(Point::new(1.0, 5.0)).route_to(Point::new(9.0, 5.0)).unwrap();

        let expected = 2.0 * (3.5_f64.powi(2) + 4.0_f64.powi(2)).sqrt() + 1.0;
        assert!((r.length - expected).abs() < 1e-6, "got {}", r.length);
        assert!((polyline_len(&r.points) - r.length).abs() < 1e-6);
        assert_eq!(r.points.len(), 4);
    }

    #[test]
    fn l_shaped_room_goes_around_reflex_corner() {
        let l = Polygon::new(vec![
            Point::new(0.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(20.0, 8.0),
            Point::new(8.0, 8.0),
            Point::new(8.0, 16.0),
            Point::new(0.0, 16.0),
        ]);
        let nav = Navigator::new(l, vec![], 0.4);
        let (a, b) = (Point::new(19.0, 1.0), Point::new(1.0, 15.0));
        let r = nav.routes_from(a).route_to(b).unwrap();

        assert!(r.points.len() >= 3);
        assert!(r.length > a.distance(b) + 0.1);
        assert!((polyline_len(&r.points) - r.length).abs() < 1e-6);
    }

    #[test]
    fn enclosed_target_is_unreachable() {
        // Цель лежит внутри препятствия: ни одна вершина графа её «не видит».
        let top = OrientedRect::new(Point::new(5.0, 6.0), 4.0, 1.0, 0.0);
        let bottom = OrientedRect::new(Point::new(5.0, 4.0), 4.0, 1.0, 0.0);
        let nav = Navigator::new(square(10.0), vec![top, bottom], 0.0);
        
        assert!(nav.routes_from(Point::new(1.0, 1.0)).route_to(Point::new(5.0, 6.0)).is_none());
    }
}
