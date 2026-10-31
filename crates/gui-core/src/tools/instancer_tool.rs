use std::f64::consts::FRAC_PI_2;

use wh_core::Point;

use crate::editor::geometry::{normalize_angle, snap_point};
use crate::editor::{EditorState, PRESETS};
use crate::tools::tool::ToolBehavior;

#[derive(Default)]
pub struct InstancerTool {
    preset: usize,
    angle: f64,
}

impl InstancerTool {
    pub fn preset(&self) -> usize {
        self.preset
    }

    pub fn angle(&self) -> f64 {
        self.angle
    }

    pub fn set_preset(&mut self, index: i32) {
        self.preset = (index.max(0) as usize).min(PRESETS.len() - 1);
    }
}

impl ToolBehavior for InstancerTool {
    fn hint(&self) -> &'static str {
        "Стеллаж: щёлкните по месту установки; «Повернуть» меняет сторону выдачи"
    }

    fn on_click(&mut self, ed: &mut EditorState, world: Point, _tol: f64) {
        ed.place_shelf(snap_point(world), &PRESETS[self.preset], self.angle);
    }

    fn on_rotate(&mut self, ed: &mut EditorState) {
        self.angle = normalize_angle(self.angle + FRAC_PI_2);
        ed.status = format!("Поворот нового стеллажа: {:.0}°", self.angle.to_degrees());
    }
}
