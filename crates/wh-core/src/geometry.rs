use std::ops::{Add, Mul, Neg, Sub};

pub const EPS: f64 = 1e-9;
const ON_EDGE: f64 = 1e-7;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn dot(self, o: Point) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: Point) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn distance(self, o: Point) -> f64 {
        (self - o).length()
    }
    pub fn normalized(self) -> Option<Point> {
        let l = self.length();
        if l < EPS {
            None
        } else {
            Some(self * (1.0 / l))
        }
    }
    pub fn rotated(self, angle: f64) -> Point {
        let (s, c) = angle.sin_cos();
        Point::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }
}

impl Add for Point {
    type Output = Point;
    fn add(self, o: Point) -> Point {
        Point::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        Point::new(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for Point {
    type Output = Point;
    fn mul(self, k: f64) -> Point {
        Point::new(self.x * k, self.y * k)
    }
}
impl Neg for Point {
    type Output = Point;
    fn neg(self) -> Point {
        Point::new(-self.x, -self.y)
    }
}

pub fn point_segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 < EPS {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

pub fn intersection_params(a: Point, b: Point, p: Point, q: Point, out: &mut Vec<f64>) {
    let d = b - a;
    let e = q - p;
    let ap = p - a;
    let denom = d.cross(e);
    if denom.abs() < EPS {
        if ap.cross(d).abs() > 1e-9 * (1.0 + ap.length() * d.length()) {
            return;
        }
        let len2 = d.dot(d);
        if len2 < EPS {
            return;
        }
        for pt in [p, q] {
            let t = (pt - a).dot(d) / len2;
            if (0.0..=1.0).contains(&t) {
                out.push(t);
            }
        }
        return;
    }
    let t = ap.cross(e) / denom;
    let s = ap.cross(d) / denom;
    if (-EPS..=1.0 + EPS).contains(&t) && (-EPS..=1.0 + EPS).contains(&s) {
        out.push(t.clamp(0.0, 1.0));
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Polygon {
    pub points: Vec<Point>,
}

impl Polygon {
    pub fn new(points: Vec<Point>) -> Self {
        Self { points }
    }

    pub fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        let n = self.points.len();
        (0..n).map(move |i| (self.points[i], self.points[(i + 1) % n]))
    }

    pub fn signed_area(&self) -> f64 {
        self.edges().map(|(a, b)| a.cross(b)).sum::<f64>() * 0.5
    }
    pub fn area(&self) -> f64 {
        self.signed_area().abs()
    }
    pub fn is_ccw(&self) -> bool {
        self.signed_area() > 0.0
    }
    pub fn perimeter(&self) -> f64 {
        self.edges().map(|(a, b)| a.distance(b)).sum()
    }

    pub fn is_simple(&self) -> bool {
        let n = self.points.len();
        if n < 3 {
            return false;
        }
        let mut buf = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                let adjacent = j == i + 1 || (i == 0 && j == n - 1);
                if adjacent {
                    continue;
                }
                let (a, b) = (self.points[i], self.points[(i + 1) % n]);
                let (c, d) = (self.points[j], self.points[(j + 1) % n]);
                buf.clear();
                intersection_params(a, b, c, d, &mut buf);
                if !buf.is_empty() {
                    return false;
                }
            }
        }
        true
    }

    pub fn contains(&self, p: Point) -> bool {
        if self
            .edges()
            .any(|(a, b)| point_segment_distance(p, a, b) < ON_EDGE)
        {
            return true;
        }
        let mut inside = false;
        for (a, b) in self.edges() {
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
                if p.x < x {
                    inside = !inside;
                }
            }
        }
        inside
    }

    pub fn contains_segment(&self, a: Point, b: Point) -> bool {
        if !self.contains(a) || !self.contains(b) {
            return false;
        }
        let mut ts = vec![0.0, 1.0];
        for (p, q) in self.edges() {
            intersection_params(a, b, p, q, &mut ts);
        }
        ts.sort_by(|x, y| x.total_cmp(y));
        ts.windows(2).all(|w| {
            if w[1] - w[0] < 1e-9 {
                return true;
            }
            let mid = a + (b - a) * ((w[0] + w[1]) * 0.5);
            self.contains(mid)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrientedRect {
    pub center: Point,
    pub half_w: f64,
    pub half_d: f64,
    pub angle: f64,
}

impl OrientedRect {
    pub fn new(center: Point, width: f64, depth: f64, angle: f64) -> Self {
        Self {
            center,
            half_w: width * 0.5,
            half_d: depth * 0.5,
            angle,
        }
    }

    pub fn axes(&self) -> (Point, Point) {
        let (s, c) = self.angle.sin_cos();
        (Point::new(c, s), Point::new(-s, c))
    }
    pub fn front_dir(&self) -> Point {
        self.axes().1
    }

    pub fn to_local(&self, p: Point) -> Point {
        let (ax, ay) = self.axes();
        let d = p - self.center;
        Point::new(d.dot(ax), d.dot(ay))
    }

    pub fn corners(&self) -> [Point; 4] {
        let (ax, ay) = self.axes();
        let (w, d) = (self.half_w, self.half_d);
        [(-w, -d), (w, -d), (w, d), (-w, d)].map(|(lx, ly)| self.center + ax * lx + ay * ly)
    }

    pub fn polygon(&self) -> Polygon {
        Polygon::new(self.corners().to_vec())
    }

    pub fn inflated(&self, margin: f64) -> Self {
        Self {
            half_w: self.half_w + margin,
            half_d: self.half_d + margin,
            ..*self
        }
    }

    pub fn contains_strict(&self, p: Point) -> bool {
        let l = self.to_local(p);
        l.x.abs() < self.half_w - 1e-6 && l.y.abs() < self.half_d - 1e-6
    }

    pub fn segment_crosses_interior(&self, a: Point, b: Point) -> bool {
        let la = self.to_local(a);
        let lb = self.to_local(b);
        let d = lb - la;
        let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
        for (p0, dd, h) in [(la.x, d.x, self.half_w), (la.y, d.y, self.half_d)] {
            let h = h - 1e-6;
            if dd.abs() < 1e-12 {
                if p0.abs() >= h {
                    return false;
                }
            } else {
                let mut ta = (-h - p0) / dd;
                let mut tb = (h - p0) / dd;
                if ta > tb {
                    std::mem::swap(&mut ta, &mut tb);
                }
                t0 = t0.max(ta);
                t1 = t1.min(tb);
                if t0 >= t1 {
                    return false;
                }
            }
        }
        t0 < t1
    }

    pub fn overlaps(&self, other: &OrientedRect) -> bool {
        let (ax, ay) = self.axes();
        let (bx, by) = other.axes();
        let ca = self.corners();
        let cb = other.corners();
        for axis in [ax, ay, bx, by] {
            let (min_a, max_a) = project(&ca, axis);
            let (min_b, max_b) = project(&cb, axis);
            if max_a <= min_b + EPS || max_b <= min_a + EPS {
                return false;
            }
        }
        true
    }
}

fn project(pts: &[Point; 4], axis: Point) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for p in pts {
        let v = p.dot(axis);
        lo = lo.min(v);
        hi = hi.max(v);
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l_shape() -> Polygon {
        Polygon::new(vec![
            Point::new(0.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(20.0, 8.0),
            Point::new(8.0, 8.0),
            Point::new(8.0, 16.0),
            Point::new(0.0, 16.0),
        ])
    }

    #[test]
    fn area_and_orientation() {
        let p = l_shape();
        assert!((p.area() - (20.0 * 8.0 + 8.0 * 8.0)).abs() < 1e-9);
        assert!(p.is_ccw());
        assert!(p.is_simple());
    }

    #[test]
    fn bowtie_is_not_simple() {
        let p = Polygon::new(vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(4.0, 0.0),
            Point::new(0.0, 4.0),
        ]);
        assert!(!p.is_simple());
    }

    #[test]
    fn contains_point() {
        let p = l_shape();
        assert!(p.contains(Point::new(10.0, 4.0)));
        assert!(p.contains(Point::new(4.0, 12.0)));
        assert!(p.contains(Point::new(0.0, 5.0))); // граница
        assert!(!p.contains(Point::new(12.0, 12.0))); // «вырез» буквы L
    }

    #[test]
    fn contains_segment_handles_concavity() {
        let p = l_shape();
        assert!(p.contains_segment(Point::new(19.0, 1.0), Point::new(1.0, 1.0)));
        // Оба конца внутри, но прямая уходит в вырез:
        assert!(!p.contains_segment(Point::new(19.0, 1.0), Point::new(1.0, 15.0)));
    }

    #[test]
    fn rect_overlap_rules() {
        let a = OrientedRect::new(Point::new(0.0, 0.0), 2.0, 2.0, 0.0);
        let touching = OrientedRect::new(Point::new(2.0, 0.0), 2.0, 2.0, 0.0);
        let overlapping = OrientedRect::new(Point::new(1.5, 0.0), 2.0, 2.0, 0.0);
        let rotated_far = OrientedRect::new(Point::new(5.0, 5.0), 2.0, 2.0, 0.7);
        assert!(!a.overlaps(&touching));
        assert!(a.overlaps(&overlapping));
        assert!(!a.overlaps(&rotated_far));
    }

    #[test]
    fn segment_vs_rect_interior() {
        let r = OrientedRect::new(Point::new(0.0, 0.0), 2.0, 2.0, 0.0);
        assert!(r.segment_crosses_interior(Point::new(-3.0, 0.0), Point::new(3.0, 0.0)));
        // по верхней грани и в угол — не пересечение
        assert!(!r.segment_crosses_interior(Point::new(-1.0, 1.0), Point::new(1.0, 1.0)));
        assert!(!r.segment_crosses_interior(Point::new(-3.0, 3.0), Point::new(-1.0, 1.0)));
    }
}
