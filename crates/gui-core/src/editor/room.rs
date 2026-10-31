use wh_core::{Point, Polygon};

use super::geometry::default_entrance;
use super::EditorState;

impl EditorState {
    pub fn has_outline(&self) -> bool {
        self.warehouse.outline.points.len() >= 3
    }

    pub(crate) fn clear_layout(&mut self) {
        self.warehouse.outline = Polygon::default();
        self.warehouse.shelves.clear();
        self.placements.clear();
        self.selected_shelf = None;
        self.invalidate_analysis();
    }

    pub(crate) fn set_outline(&mut self, poly: Polygon) {
        self.warehouse.entrance = default_entrance(&poly);
        self.warehouse.outline = poly;
        self.invalidate_analysis();
        self.fit_requested = true;
    }

    pub(crate) fn set_entrance(&mut self, p: Point) {
        if !self.has_outline() {
            self.status = "Сначала замкните контур".into();
        } else if !self.warehouse.outline.contains(p) {
            self.status = "Вход должен быть внутри помещения".into();
        } else {
            self.warehouse.entrance = p;
            self.invalidate_analysis();
            self.status = "Вход перемещён".into();
        }
    }
}
