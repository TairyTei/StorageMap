use std::f64::consts::TAU;

use wh_core::{Point, Polygon};

pub const GRID_STEP: f64 = 0.1;

fn snap(v: f64) -> f64 {
    let r = (v / GRID_STEP).round() * GRID_STEP;
    let r = (r * 1e6).round() / 1e6;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

pub(crate) fn snap_point(p: Point) -> Point {
    Point::new(snap(p.x), snap(p.y))
}

pub(crate) fn normalize_angle(a: f64) -> f64 {
    let r = a.rem_euclid(TAU);
    if (r - TAU).abs() < 1e-9 {
        0.0
    } else {
        r
    }
}

pub(crate) fn default_entrance(poly: &Polygon) -> Point {
    let v0 = poly.points[0];
    let n = poly.points.len() as f64;
    let avg = Point::new(
        poly.points.iter().map(|p| p.x).sum::<f64>() / n,
        poly.points.iter().map(|p| p.y).sum::<f64>() / n,
    );
    if let Some(dir) = (avg - v0).normalized() {
        for step in [0.5, 0.2] {
            let cand = v0 + dir * step;
            if poly.contains(cand) {
                return cand;
            }
        }
    }
    v0
}
