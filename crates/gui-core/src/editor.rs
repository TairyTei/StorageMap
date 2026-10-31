mod analysis;
pub(crate) mod geometry;
mod presets;
mod products;
mod room;
mod shelves;
#[cfg(test)]
mod tests;

pub use geometry::GRID_STEP;
pub use presets::{ShelfPreset, PRESETS};

use wh_core::{
    Placement, Point, Polygon, Product, ProductId, ShelfAccess, ShelfId, Snapshot, Warehouse,
    WarehouseId,
};

use crate::tools::tool::{Tool, ToolContext};

pub struct EditorState {
    pub warehouse: Warehouse,
    pub products: Vec<Product>,
    pub placements: Vec<Placement>,
    pub access: Vec<ShelfAccess>,

    pub tool: Tool,
    pub tool_context: ToolContext,
    pub ortho: bool,

    pub selected_shelf: Option<ShelfId>,
    pub selected_product: Option<ProductId>,
    pub status: String,
    pub fit_requested: bool,
}

impl EditorState {
    pub fn new_empty() -> Self {
        Self::from_snapshot(Snapshot {
            warehouse: Warehouse {
                id: WarehouseId(1),
                name: "Новый склад".into(),
                outline: Polygon::default(),
                entrance: Point::ZERO,
                shelves: vec![],
            },
            products: vec![],
            placements: vec![],
        })
    }

    pub fn from_snapshot(s: Snapshot) -> Self {
        Self {
            warehouse: s.warehouse,
            products: s.products,
            placements: s.placements,
            access: vec![],
            tool: Tool::Select,
            tool_context: ToolContext::default(),
            ortho: true,
            selected_shelf: None,
            selected_product: None,
            status: String::new(),
            fit_requested: true,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            warehouse: self.warehouse.clone(),
            products: self.products.clone(),
            placements: self.placements.clone(),
        }
    }

    pub fn bounds(&self) -> (Point, Point) {
        let draft = self.draft();
        let pts: &[Point] = if draft.is_empty() {
            &self.warehouse.outline.points
        } else {
            draft
        };
        if pts.is_empty() {
            return (Point::new(0.0, 0.0), Point::new(20.0, 15.0));
        }
        let mut min = pts[0];
        let mut max = pts[0];
        for p in pts {
            min = Point::new(min.x.min(p.x), min.y.min(p.y));
            max = Point::new(max.x.max(p.x), max.y.max(p.y));
        }
        (min, max)
    }

    pub fn draft(&self) -> &[Point] {
        self.tool_context.outline.draft()
    }

    pub fn is_drawing(&self) -> bool {
        self.tool_context.outline.is_drawing()
    }

    pub fn preset(&self) -> usize {
        self.tool_context.instancer.preset()
    }

    pub fn pending_angle(&self) -> f64 {
        self.tool_context.instancer.angle()
    }

    pub fn set_preset(&mut self, index: i32) {
        self.tool_context.instancer.set_preset(index);
        self.status = format!("Типоразмер: {}", PRESETS[self.preset()].name);
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.status = self.tool_context.get(tool).hint().into();
    }

    pub fn click(&mut self, world: Point, tol: f64) {
        self.with_tools(|tools, ed| tools.get_mut(ed.tool).on_click(ed, world, tol));
    }

    pub fn drag(&mut self, dx: f64, dy: f64) -> bool {
        self.with_tools(|tools, ed| tools.get_mut(ed.tool).on_drag(ed, dx, dy))
    }

    pub fn rotate(&mut self) {
        self.with_tools(|tools, ed| tools.get_mut(ed.tool).on_rotate(ed));
    }

    pub fn undo_point(&mut self) {
        self.with_tools(|tools, ed| tools.outline.undo_point(ed));
    }

    pub fn close_outline(&mut self) {
        self.with_tools(|tools, ed| tools.outline.close(ed));
    }

    fn with_tools<R>(&mut self, f: impl FnOnce(&mut ToolContext, &mut EditorState) -> R) -> R {
        let mut tools = std::mem::take(&mut self.tool_context);
        let result = f(&mut tools, self);
        self.tool_context = tools;
        result
    }

    fn invalidate_analysis(&mut self) {
        self.access.clear();
    }
}
