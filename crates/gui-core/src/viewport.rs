//! Преобразование между мировыми координатами (метры, Y вверх) и экраном (пиксели, Y вниз).

use wh_core::Point;

#[derive(Debug, Clone)]
pub struct Viewport {
    pub center: Point,
    pub scale: f64,
    pub size: (f64, f64),
}

pub const MIN_SCALE: f64 = 2.0;
pub const MAX_SCALE: f64 = 400.0;

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center: Point::new(10.0, 7.5),
            scale: 30.0,
            size: (900.0, 700.0),
        }
    }
}

impl Viewport {
    pub fn to_screen(&self, p: Point) -> (f64, f64) {
        (
            (p.x - self.center.x) * self.scale + self.size.0 * 0.5,
            self.size.1 * 0.5 - (p.y - self.center.y) * self.scale,
        )
    }

    pub fn to_world(&self, sx: f64, sy: f64) -> Point {
        Point::new(
            (sx - self.size.0 * 0.5) / self.scale + self.center.x,
            (self.size.1 * 0.5 - sy) / self.scale + self.center.y,
        )
    }

    pub fn fit(&mut self, min: Point, max: Point) {
        let margin = 40.0;
        let bw = (max.x - min.x).max(1.0);
        let bh = (max.y - min.y).max(1.0);
        let sx = (self.size.0 - 2.0 * margin) / bw;
        let sy = (self.size.1 - 2.0 * margin) / bh;
        self.scale = sx.min(sy).clamp(MIN_SCALE, MAX_SCALE);
        self.center = Point::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5);
    }

    /// Масштабирование относительно центра экрана.
    pub fn zoom(&mut self, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        self.scale = (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
    }

    /// Масштабирование относительно точки экрана `(sx, sy)`: мировая точка под
    /// курсором/пальцем остаётся на месте.
    pub fn zoom_at(&mut self, factor: f64, sx: f64, sy: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let anchor = self.to_world(sx, sy);
        self.scale = (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
        // Пересчитываем центр уже с фактическим (после clamp) масштабом.
        self.center = Point::new(
            anchor.x - (sx - self.size.0 * 0.5) / self.scale,
            anchor.y - (self.size.1 * 0.5 - sy) / self.scale,
        );
    }

    pub fn pan_pixels(&mut self, dx: f64, dy: f64) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        self.center.x -= dx / self.scale;
        self.center.y += dy / self.scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_world_roundtrip() {
        let vp = Viewport {
            center: Point::new(3.0, 4.0),
            scale: 25.0,
            size: (800.0, 600.0),
        };
        let p = Point::new(7.5, -2.0);
        let (sx, sy) = vp.to_screen(p);
        let back = vp.to_world(sx, sy);
        assert!(back.distance(p) < 1e-9);
    }

    #[test]
    fn y_axis_points_up_on_screen() {
        let vp = Viewport::default();
        let (_, y_low) = vp.to_screen(Point::new(10.0, 0.0));
        let (_, y_high) = vp.to_screen(Point::new(10.0, 5.0));
        assert!(y_high < y_low);
    }

    #[test]
    fn zoom_at_keeps_point_under_cursor() {
        let mut vp = Viewport {
            center: Point::new(3.0, 4.0),
            scale: 25.0,
            size: (800.0, 600.0),
        };
        let (sx, sy) = (123.0, 456.0);
        let before = vp.to_world(sx, sy);
        vp.zoom_at(1.7, sx, sy);
        let after = vp.to_world(sx, sy);
        assert!(before.distance(after) < 1e-9);
        assert!((vp.scale - 25.0 * 1.7).abs() < 1e-9);
    }

    #[test]
    fn zoom_at_keeps_point_even_when_clamped() {
        let mut vp = Viewport {
            scale: MAX_SCALE - 1.0,
            size: (800.0, 600.0),
            ..Viewport::default()
        };
        let (sx, sy) = (700.0, 100.0);
        let before = vp.to_world(sx, sy);
        vp.zoom_at(10.0, sx, sy);
        assert_eq!(vp.scale, MAX_SCALE);
        assert!(before.distance(vp.to_world(sx, sy)) < 1e-9);
    }

    #[test]
    fn pan_pixels_moves_map_with_pointer() {
        let mut vp = Viewport {
            center: Point::new(3.0, 4.0),
            scale: 25.0,
            size: (800.0, 600.0),
        };
        let p = Point::new(5.0, 2.0);
        let (x0, y0) = vp.to_screen(p);
        vp.pan_pixels(30.0, -20.0);
        let (x1, y1) = vp.to_screen(p);
        assert!((x1 - x0 - 30.0).abs() < 1e-9);
        assert!((y1 - y0 + 20.0).abs() < 1e-9);
    }

    #[test]
    fn fit_puts_bounds_inside_canvas() {
        let mut vp = Viewport {
            size: (800.0, 600.0),
            ..Viewport::default()
        };
        vp.fit(Point::new(0.0, 0.0), Point::new(20.0, 16.0));
        for p in [Point::new(0.0, 0.0), Point::new(20.0, 16.0)] {
            let (x, y) = vp.to_screen(p);
            assert!((0.0..=800.0).contains(&x) && (0.0..=600.0).contains(&y));
        }
    }
}
