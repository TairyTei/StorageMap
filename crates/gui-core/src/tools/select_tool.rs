use wh_core::Point;

use crate::editor::EditorState;
use crate::tools::tool::ToolBehavior;

#[derive(Default)]
pub struct SelectTool;

impl ToolBehavior for SelectTool {
    fn hint(&self) -> &'static str {
        "Выбор: щёлкните по стеллажу"
    }

    fn on_click(&mut self, ed: &mut EditorState, world: Point, _tol: f64) {
        ed.selected_shelf = ed.pick_shelf(world);
    }

    fn on_drag(&mut self, ed: &mut EditorState, dx: f64, dy: f64) -> bool {
        ed.move_selected(dx, dy)
    }

    fn on_rotate(&mut self, ed: &mut EditorState) {
        ed.rotate_selected();
    }
}
