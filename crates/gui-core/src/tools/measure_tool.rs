use wh_core::Point;

use crate::editor::geometry::snap_point;
use crate::editor::EditorState;
use crate::tools::tool::ToolBehavior;

#[derive(Default)]
pub struct MeasureTool {
    phase: Phase,
}

#[derive(Default, Clone, Copy)]
enum Phase {
    #[default]
    Idle,
    Started(Point),
    Done(Point, Point),
}

impl MeasureTool {
    pub fn anchor(&self) -> Option<Point> {
        match self.phase {
            Phase::Started(a) => Some(a),
            _ => None,
        }
    }

    pub fn segment(&self) -> Option<(Point, Point)> {
        match self.phase {
            Phase::Done(a, b) => Some((a, b)),
            _ => None,
        }
    }

    pub fn length(&self) -> Option<f64> {
        self.segment().map(|(a, b)| a.distance(b))
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
    }
}

impl ToolBehavior for MeasureTool {
    fn hint(&self) -> &'static str {
        "Линейка: щёлкните начало и конец отрезка"
    }

    fn on_click(&mut self, ed: &mut EditorState, world: Point, _tol: f64) {
        let p = snap_point(world);
        self.phase = match self.phase {
            Phase::Started(a) => {
                ed.status = format!(
                    "Расстояние: {:.2} м (Δx {:.2}, Δy {:.2})",
                    a.distance(p),
                    (p.x - a.x).abs(),
                    (p.y - a.y).abs()
                );
                Phase::Done(a, p)
            }
            Phase::Idle | Phase::Done(..) => {
                ed.status = "Линейка: щёлкните вторую точку".into();
                Phase::Started(p)
            }
        };
    }

    fn on_rotate(&mut self, ed: &mut EditorState) {
        self.reset();
        ed.status = "Измерение сброшено".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(t: &mut MeasureTool, ed: &mut EditorState, x: f64, y: f64) {
        t.on_click(ed, Point::new(x, y), 0.3);
    }

    #[test]
    fn two_clicks_measure_distance() {
        let (mut ed, mut t) = (EditorState::new_empty(), MeasureTool::default());
        click(&mut t, &mut ed, 0.0, 0.0);
        assert!(t.anchor().is_some() && t.segment().is_none());
        click(&mut t, &mut ed, 3.0, 4.0);
        assert!((t.length().unwrap() - 5.0).abs() < 1e-9);
        assert!(ed.status.contains("5.00"));
    }

    #[test]
    fn third_click_starts_new_measurement() {
        let (mut ed, mut t) = (EditorState::new_empty(), MeasureTool::default());
        click(&mut t, &mut ed, 0.0, 0.0);
        click(&mut t, &mut ed, 1.0, 0.0);
        click(&mut t, &mut ed, 5.0, 5.0);
        assert!(t.anchor().is_some() && t.segment().is_none());
    }

    #[test]
    fn rotate_resets() {
        let (mut ed, mut t) = (EditorState::new_empty(), MeasureTool::default());
        click(&mut t, &mut ed, 0.0, 0.0);
        t.on_rotate(&mut ed);
        assert!(t.anchor().is_none() && t.segment().is_none());
    }
}
