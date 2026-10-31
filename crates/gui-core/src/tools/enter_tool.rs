use wh_core::Point;

use crate::editor::geometry::snap_point;
use crate::editor::EditorState;
use crate::tools::tool::ToolBehavior;

#[derive(Default)]
pub struct EnterTool;

impl ToolBehavior for EnterTool {
    fn hint(&self) -> &'static str {
        "Вход: щёлкните точку входа внутри помещения"
    }

    fn on_click(&mut self, ed: &mut EditorState, world: Point, _tol: f64) {
        ed.set_entrance(snap_point(world));
    }
}
