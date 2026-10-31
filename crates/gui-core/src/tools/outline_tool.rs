use wh_core::{Point, Polygon};

use crate::editor::geometry::snap_point;
use crate::editor::{EditorState, GRID_STEP};
use crate::tools::tool::{Tool, ToolBehavior};

#[derive(Default)]
pub struct OutlineTool {
    draft: Vec<Point>,
}

impl OutlineTool {
    pub fn draft(&self) -> &[Point] {
        &self.draft
    }

    pub fn is_drawing(&self) -> bool {
        !self.draft.is_empty()
    }

    pub fn undo_point(&mut self, ed: &mut EditorState) {
        if self.draft.pop().is_some() {
            ed.status = format!("Вершин: {}", self.draft.len());
        }
    }

    pub fn close(&mut self, ed: &mut EditorState) {
        if !self.is_drawing() {
            return;
        }
        if self.draft.len() < 3 {
            ed.status = "Нужно минимум 3 вершины".into();
            return;
        }
        let mut poly = Polygon::new(self.draft.clone());
        if poly.area() < 0.5 || !poly.is_simple() {
            ed.status = "Контур самопересекается или слишком мал".into();
            return;
        }
        if !poly.is_ccw() {
            poly.points.reverse();
        }
        let (area, perimeter) = (poly.area(), poly.perimeter());
        ed.set_outline(poly);
        self.draft.clear();
        ed.tool = Tool::Select;
        ed.status = format!(
            "Контур замкнут: {area:.1} м², периметр {perimeter:.1} м. Теперь добавьте стеллажи"
        );
    }

    fn apply_ortho(&self, p: Point, ortho: bool) -> Point {
        if !ortho {
            return p;
        }
        let Some(&last) = self.draft.last() else {
            return p;
        };
        let d = p - last;
        let mut out = p;
        if d.x.abs() >= d.y.abs() {
            out.y = last.y;
        } else {
            out.x = last.x;
        }
        out
    }
}

impl ToolBehavior for OutlineTool {
    fn hint(&self) -> &'static str {
        "Контур: щёлкайте вершины; клик у первой точки или «Замкнуть» завершает"
    }

    fn on_click(&mut self, ed: &mut EditorState, world: Point, tol: f64) {
        let snapped = snap_point(world);

        if self.draft.len() >= 3 && snapped.distance(self.draft[0]) <= tol.max(GRID_STEP * 1.5) {
            self.close(ed);
            return;
        }

        let p = self.apply_ortho(snapped, ed.ortho);

        if !self.is_drawing() && !ed.warehouse.outline.points.is_empty() {
            ed.clear_layout();
        }

        if self.draft.last().map_or(true, |l| l.distance(p) > 1e-9) {
            self.draft.push(p);
        }
        ed.status = format!("Вершин: {}", self.draft.len());
    }
}
