use wh_core::Point;

use crate::editor::EditorState;
use crate::tools::enter_tool::EnterTool;
use crate::tools::instancer_tool::InstancerTool;
use crate::tools::measure_tool::MeasureTool;
use crate::tools::outline_tool::OutlineTool;
use crate::tools::select_tool::SelectTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Outline,
    Shelf,
    Entrance,
    Measure,
}

impl Tool {
    pub fn from_index(i: i32) -> Tool {
        match i {
            1 => Tool::Outline,
            2 => Tool::Shelf,
            3 => Tool::Entrance,
            4 => Tool::Measure,
            _ => Tool::Select,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            Tool::Select => 0,
            Tool::Outline => 1,
            Tool::Shelf => 2,
            Tool::Entrance => 3,
            Tool::Measure => 4,
        }
    }
}

pub trait ToolBehavior {
    fn hint(&self) -> &'static str;

    fn on_click(&mut self, editor: &mut EditorState, world: Point, tol: f64);

    fn on_drag(&mut self, _editor: &mut EditorState, _dx: f64, _dy: f64) -> bool {
        false
    }

    fn on_rotate(&mut self, _editor: &mut EditorState) {}
}

#[derive(Default)]
pub struct ToolContext {
    pub select: SelectTool,
    pub outline: OutlineTool,
    pub instancer: InstancerTool,
    pub enter: EnterTool,
    pub measure: MeasureTool,
}

impl ToolContext {
    pub fn get(&self, tool: Tool) -> &dyn ToolBehavior {
        match tool {
            Tool::Select => &self.select,
            Tool::Outline => &self.outline,
            Tool::Shelf => &self.instancer,
            Tool::Entrance => &self.enter,
            Tool::Measure => &self.measure,
        }
    }

    pub fn get_mut(&mut self, tool: Tool) -> &mut dyn ToolBehavior {
        match tool {
            Tool::Select => &mut self.select,
            Tool::Outline => &mut self.outline,
            Tool::Shelf => &mut self.instancer,
            Tool::Entrance => &mut self.enter,
            Tool::Measure => &mut self.measure,
        }
    }
}
