//! Состояние и операции редактора. Никакого UI: только данные и действия над ними,
//! поэтому всё покрывается обычными unit-тестами.

use std::f64::consts::{FRAC_PI_2, TAU};

use wh_core::{
    OrientedRect, Placement, Point, Polygon, Product, ProductId, Shelf, ShelfAccess, ShelfId,
    Snapshot, ValidationIssue, Warehouse, WarehouseId,
};
use wh_nav::{analyze as analyze_access, NavConfig};

/// Шаг привязки к сетке, м.
pub const GRID_STEP: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Outline,
    Shelf,
    Entrance,
}

impl Tool {
    pub fn from_index(i: i32) -> Tool {
        match i {
            1 => Tool::Outline,
            2 => Tool::Shelf,
            3 => Tool::Entrance,
            _ => Tool::Select,
        }
    }
    pub fn index(self) -> i32 {
        match self {
            Tool::Select => 0,
            Tool::Outline => 1,
            Tool::Shelf => 2,
            Tool::Entrance => 3,
        }
    }
}

pub struct ShelfPreset {
    pub name: &'static str,
    pub width: f64,
    pub depth: f64,
    pub capacity: u32,
}

pub const PRESETS: [ShelfPreset; 4] = [
    ShelfPreset { name: "3×1 м (6 яч.)", width: 3.0, depth: 1.0, capacity: 6 },
    ShelfPreset { name: "2×1 м (4 яч.)", width: 2.0, depth: 1.0, capacity: 4 },
    ShelfPreset { name: "1.2×0.5 м (2 яч.)", width: 1.2, depth: 0.5, capacity: 2 },
    ShelfPreset { name: "4×1.5 м (8 яч.)", width: 4.0, depth: 1.5, capacity: 8 },
];

pub struct EditorState {
    pub warehouse: Warehouse,
    pub products: Vec<Product>,
    pub placements: Vec<Placement>,
    /// Результат последнего анализа; пусто — анализ устарел или не запускался.
    pub access: Vec<ShelfAccess>,

    pub tool: Tool,
    /// Привязка новых вершин контура к горизонтали/вертикали.
    pub ortho: bool,
    /// Идёт рисование контура: вершины лежат в `draft`.
    pub drawing: bool,
    pub draft: Vec<Point>,
    pub preset: usize,
    /// Поворот, с которым будет поставлен следующий стеллаж.
    pub pending_angle: f64,

    pub selected_shelf: Option<ShelfId>,
    pub selected_product: Option<ProductId>,
    pub status: String,
    /// Оболочка должна вписать содержимое в окно (после загрузки / замыкания контура).
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
            ortho: true,
            drawing: false,
            draft: vec![],
            preset: 0,
            pending_angle: 0.0,
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

    pub fn has_outline(&self) -> bool {
        !self.drawing && self.warehouse.outline.points.len() >= 3
    }

    /// Границы содержимого (для «вписать в окно»).
    pub fn bounds(&self) -> (Point, Point) {
        let pts: &[Point] = if self.drawing && !self.draft.is_empty() {
            &self.draft
        } else {
            &self.warehouse.outline.points
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

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.status = match tool {
            Tool::Select => "Выбор: щёлкните по стеллажу".into(),
            Tool::Outline => "Контур: щёлкайте вершины; клик у первой точки или «Замкнуть» завершает".into(),
            Tool::Shelf => "Стеллаж: щёлкните по месту установки; «Повернуть» меняет сторону выдачи".into(),
            Tool::Entrance => "Вход: щёлкните точку входа внутри помещения".into(),
        };
    }

    // ───────────── клики по карте ─────────────

    /// Клик по карте. `tol` — допуск «попал в точку» в метрах (зависит от масштаба).
    pub fn click(&mut self, world: Point, tol: f64) {
        match self.tool {
            Tool::Select => {
                self.selected_shelf = self.pick_shelf(world);
            }
            Tool::Outline => self.outline_click(world, tol),
            Tool::Shelf => self.place_shelf(snap_point(world)),
            Tool::Entrance => self.set_entrance(snap_point(world)),
        }
    }

    pub fn pick_shelf(&self, p: Point) -> Option<ShelfId> {
        self.warehouse.shelves.iter().rev().find_map(|s| {
            let l = s.footprint.to_local(p);
            (l.x.abs() <= s.footprint.half_w && l.y.abs() <= s.footprint.half_d).then_some(s.id)
        })
    }

    // ───────────── контур ─────────────

    fn outline_click(&mut self, world: Point, tol: f64) {
        let snapped = snap_point(world);
        if self.drawing && self.draft.len() >= 3 && snapped.distance(self.draft[0]) <= tol.max(GRID_STEP * 1.5) {
            self.close_outline();
            return;
        }
        let mut p = snapped;
        if self.drawing && self.ortho {
            if let Some(&last) = self.draft.last() {
                let d = snapped - last;
                if d.x.abs() >= d.y.abs() {
                    p.y = last.y;
                } else {
                    p.x = last.x;
                }
            }
        }
        if !self.drawing {
            self.begin_outline();
        }
        if self.draft.last().map_or(true, |l| l.distance(p) > 1e-9) {
            self.draft.push(p);
        }
        self.status = format!("Вершин: {}", self.draft.len());
    }

    fn begin_outline(&mut self) {
        if !self.warehouse.outline.points.is_empty() {
            // Новый контур делает старую расстановку бессмысленной.
            self.warehouse.outline = Polygon::default();
            self.warehouse.shelves.clear();
            self.placements.clear();
            self.access.clear();
            self.selected_shelf = None;
        }
        self.drawing = true;
        self.draft.clear();
    }

    pub fn undo_point(&mut self) {
        if self.drawing {
            self.draft.pop();
            if self.draft.is_empty() {
                self.drawing = false;
            }
            self.status = format!("Вершин: {}", self.draft.len());
        }
    }

    pub fn close_outline(&mut self) {
        if !self.drawing {
            return;
        }
        if self.draft.len() < 3 {
            self.status = "Нужно минимум 3 вершины".into();
            return;
        }
        let mut poly = Polygon::new(self.draft.clone());
        if poly.area() < 0.5 || !poly.is_simple() {
            self.status = "Контур самопересекается или слишком мал".into();
            return;
        }
        if !poly.is_ccw() {
            poly.points.reverse();
        }
        self.warehouse.entrance = default_entrance(&poly);
        self.warehouse.outline = poly;
        self.draft.clear();
        self.drawing = false;
        self.tool = Tool::Select;
        self.access.clear();
        self.fit_requested = true;
        self.status = format!(
            "Контур замкнут: {:.1} м², периметр {:.1} м. Теперь добавьте стеллажи",
            self.warehouse.outline.area(),
            self.warehouse.outline.perimeter()
        );
    }

    fn set_entrance(&mut self, p: Point) {
        if !self.has_outline() {
            self.status = "Сначала замкните контур".into();
        } else if !self.warehouse.outline.contains(p) {
            self.status = "Вход должен быть внутри помещения".into();
        } else {
            self.warehouse.entrance = p;
            self.access.clear();
            self.status = "Вход перемещён".into();
        }
    }

    // ───────────── стеллажи ─────────────

    fn fits(&self, rect: &OrientedRect, ignore: Option<ShelfId>) -> Result<(), String> {
        let c = rect.corners();
        let inside = (0..4).all(|i| self.warehouse.outline.contains_segment(c[i], c[(i + 1) % 4]));
        if !inside {
            return Err("Стеллаж выходит за контур".into());
        }
        let clash = self
            .warehouse
            .shelves
            .iter()
            .any(|s| Some(s.id) != ignore && s.footprint.overlaps(rect));
        if clash {
            return Err("Стеллаж пересекается с другим".into());
        }
        Ok(())
    }

    fn place_shelf(&mut self, center: Point) {
        if !self.has_outline() {
            self.status = "Сначала нарисуйте и замкните контур".into();
            return;
        }
        let preset = &PRESETS[self.preset.min(PRESETS.len() - 1)];
        let rect = OrientedRect::new(center, preset.width, preset.depth, self.pending_angle);
        if let Err(msg) = self.fits(&rect, None) {
            self.status = msg;
            return;
        }
        let id = ShelfId(self.warehouse.shelves.iter().map(|s| s.id.0).max().unwrap_or(0) + 1);
        self.warehouse
            .shelves
            .push(Shelf::new(id, format!("S{}", id.0), rect, preset.capacity));
        self.selected_shelf = Some(id);
        self.access.clear();
        self.status = format!("Стеллаж S{} добавлен", id.0);
    }

    /// Поворот на 90°: выбранного стеллажа (инструмент «Выбор») или следующего устанавливаемого.
    pub fn rotate(&mut self) {
        match (self.tool, self.selected_shelf) {
            (Tool::Select, Some(id)) => {
                let Some(shelf) = self.warehouse.shelf(id) else { return };
                let mut rect = shelf.footprint;
                rect.angle = normalize_angle(rect.angle + FRAC_PI_2);
                match self.fits(&rect, Some(id)) {
                    Ok(()) => {
                        if let Some(s) = self.warehouse.shelves.iter_mut().find(|s| s.id == id) {
                            s.footprint = rect;
                        }
                        self.access.clear();
                        self.status = "Стеллаж повёрнут".into();
                    }
                    Err(msg) => self.status = format!("Нельзя повернуть: {msg}"),
                }
            }
            _ => {
                self.pending_angle = normalize_angle(self.pending_angle + FRAC_PI_2);
                self.status = format!("Поворот нового стеллажа: {:.0}°", self.pending_angle.to_degrees());
            }
        }
    }

    pub fn delete_selected(&mut self) {
        let Some(id) = self.selected_shelf.take() else {
            self.status = "Выберите стеллаж".into();
            return;
        };
        self.warehouse.shelves.retain(|s| s.id != id);
        self.placements.retain(|p| p.shelf != id);
        self.access.clear();
        self.status = "Стеллаж удалён".into();
    }

    pub fn toggle_double_sided(&mut self) {
        let Some(id) = self.selected_shelf else {
            self.status = "Выберите стеллаж".into();
            return;
        };
        if let Some(s) = self.warehouse.shelves.iter_mut().find(|s| s.id == id) {
            s.double_sided = !s.double_sided;
            let text = if s.double_sided { "Доступ с двух сторон" } else { "Доступ только спереди" };
            self.status = text.to_string();
        }
        self.access.clear();
    }

    pub fn shelf_used(&self, shelf: ShelfId) -> u32 {
        self.placements
            .iter()
            .filter(|p| p.shelf == shelf)
            .filter_map(|p| self.products.iter().find(|x| x.id == p.product))
            .map(|x| x.size)
            .sum()
    }

    // ───────────── товары ─────────────

    pub fn add_product(&mut self, name: &str, size: &str, demand: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Введите название товара".into());
        }
        let size = if size.trim().is_empty() {
            1
        } else {
            size.trim().parse::<u32>().map_err(|_| "Размер — целое число ячеек".to_string())?
        };
        if size == 0 {
            return Err("Размер должен быть больше нуля".into());
        }
        let demand = if demand.trim().is_empty() {
            None
        } else {
            Some(
                demand
                    .trim()
                    .replace(',', ".")
                    .parse::<f32>()
                    .map_err(|_| "Спрос — число".to_string())?,
            )
        };
        let id = ProductId(self.products.iter().map(|p| p.id.0).max().unwrap_or(0) + 1);
        self.products.push(Product { id, sku: format!("SKU-{:04}", id.0), name: name.into(), size, demand });
        self.status = format!("Товар «{name}» добавлен");
        Ok(())
    }

    pub fn placement_of(&self, product: ProductId) -> Option<&Placement> {
        self.placements.iter().find(|p| p.product == product)
    }

    /// Строки списка товаров: (id, текст, выбран). Сначала самые востребованные.
    pub fn product_rows(&self, query: &str) -> Vec<(ProductId, String, bool)> {
        let q = query.trim().to_lowercase();
        let mut items: Vec<&Product> = self
            .products
            .iter()
            .filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q) || p.sku.to_lowercase().contains(&q))
            .collect();
        items.sort_by(|a, b| {
            let (da, db) = (a.demand.unwrap_or(f32::NEG_INFINITY), b.demand.unwrap_or(f32::NEG_INFINITY));
            db.total_cmp(&da).then(a.id.cmp(&b.id))
        });
        items
            .into_iter()
            .map(|p| {
                let place = match self.placement_of(p.id) {
                    Some(pl) => {
                        let label = self.warehouse.shelf(pl.shelf).map_or("?", |s| s.label.as_str());
                        format!("→ {label}{}", if pl.pinned { " [закреп.]" } else { "" })
                    }
                    None => "→ —".to_string(),
                };
                let demand = p.demand.map_or("—".to_string(), |d| format!("{d}"));
                (p.id, format!("{} · спрос {} · {} яч. {}", p.name, demand, p.size, place), self.selected_product == Some(p.id))
            })
            .collect()
    }

    pub fn pin_selected(&mut self) {
        let (Some(pid), Some(sid)) = (self.selected_product, self.selected_shelf) else {
            self.status = "Выберите товар в списке и стеллаж на карте".into();
            return;
        };
        let Some(size) = self.products.iter().find(|p| p.id == pid).map(|p| p.size) else { return };
        let Some(capacity) = self.warehouse.shelf(sid).map(|s| s.capacity) else { return };
        let used_by_others: u32 = self
            .placements
            .iter()
            .filter(|p| p.shelf == sid && p.product != pid)
            .filter_map(|p| self.products.iter().find(|x| x.id == p.product))
            .map(|x| x.size)
            .sum();
        if used_by_others + size > capacity {
            self.status = format!("Не хватает места: свободно {} яч., нужно {size}", capacity.saturating_sub(used_by_others));
            return;
        }
        self.placements.retain(|p| p.product != pid);
        self.placements.push(Placement { product: pid, shelf: sid, pinned: true });
        self.status = "Товар закреплён за стеллажом".into();
    }

    pub fn unpin_selected(&mut self) {
        let Some(pid) = self.selected_product else {
            self.status = "Выберите товар в списке".into();
            return;
        };
        match self.placements.iter_mut().find(|p| p.product == pid) {
            Some(p) => {
                p.pinned = false;
                self.status = "Закрепление снято: при следующей авторасстановке товар может переехать".into();
            }
            None => self.status = "Товар не размещён".into(),
        }
    }

    // ───────────── анализ и авторасстановка ─────────────

    /// Считает маршруты и доступность. `false` — планировка некорректна (причина в `status`).
    pub fn analyze(&mut self) -> bool {
        if !self.has_outline() {
            self.status = "Сначала нарисуйте контур склада".into();
            return false;
        }
        let issues = self.warehouse.validate();
        if !issues.is_empty() {
            let text: Vec<String> = issues.iter().map(describe_issue).collect();
            self.status = format!("Ошибки планировки: {}", text.join("; "));
            return false;
        }
        self.access = analyze_access(&self.warehouse, &NavConfig::default());
        let unreachable = self.access.iter().filter(|a| a.distance.is_none()).count();
        self.status = if unreachable == 0 {
            format!("Доступность рассчитана для {} стеллажей", self.access.len())
        } else {
            format!("Рассчитано; недостижимых стеллажей: {unreachable} (проверьте проходы и сторону выдачи)")
        };
        true
    }

    /// Расставляет товары: закреплённые остаются, остальные раскладываются по востребованности.
    pub fn auto_place(&mut self) {
        if self.access.len() != self.warehouse.shelves.len() && !self.analyze() {
            return;
        }
        if self.warehouse.shelves.is_empty() {
            self.status = "Нет стеллажей".into();
            return;
        }
        let plan = wh_layout::auto_place(&self.warehouse, &self.access, &self.products, &self.placements);
        self.placements = plan.placements;
        self.status = if plan.unplaced.is_empty() {
            format!("Размещено товаров: {}", self.placements.len())
        } else {
            format!("Размещено {}, не поместилось {}", self.placements.len(), plan.unplaced.len())
        };
    }

    // ───────────── информация ─────────────

    pub fn shelf_info(&self) -> String {
        let Some(id) = self.selected_shelf else { return String::new() };
        let Some(shelf) = self.warehouse.shelf(id) else { return String::new() };
        let fp = &shelf.footprint;
        let mut s = format!(
            "{} — {:.1}×{:.1} м{}\nЗанято {}/{} яч.",
            shelf.label,
            fp.half_w * 2.0,
            fp.half_d * 2.0,
            if shelf.double_sided { ", двусторонний" } else { "" },
            self.shelf_used(id),
            shelf.capacity
        );
        if let Some(a) = self.access.iter().find(|a| a.shelf == id) {
            match a.distance {
                Some(d) => s.push_str(&format!(" · маршрут {d:.1} м · доступность {:.0}%", a.score * 100.0)),
                None => s.push_str(" · недостижим"),
            }
        }
        let names: Vec<String> = self
            .placements
            .iter()
            .filter(|p| p.shelf == id)
            .filter_map(|p| self.products.iter().find(|x| x.id == p.product))
            .map(|x| format!("{} ({})", x.name, x.size))
            .collect();
        if !names.is_empty() {
            s.push_str("\nТовары: ");
            s.push_str(&names.join(", "));
        }
        s
    }
}

// ───────────── вспомогательные ─────────────

fn describe_issue(issue: &ValidationIssue) -> String {
    match issue {
        ValidationIssue::OutlineTooSmall => "контур слишком мал".into(),
        ValidationIssue::OutlineSelfIntersects => "контур самопересекается".into(),
        ValidationIssue::EntranceOutside => "вход вне помещения".into(),
        ValidationIssue::ShelfOutside(id) => format!("стеллаж S{id} вне контура"),
        ValidationIssue::ShelvesOverlap(a, b) => format!("стеллажи S{a} и S{b} пересекаются"),
        ValidationIssue::DuplicateShelfId(id) => format!("повторный id стеллажа {id}"),
    }
}

fn snap(v: f64) -> f64 {
    let r = (v / GRID_STEP).round() * GRID_STEP;
    // убираем шум float и «-0.0»
    let r = (r * 1e6).round() / 1e6;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn snap_point(p: Point) -> Point {
    Point::new(snap(p.x), snap(p.y))
}

fn normalize_angle(a: f64) -> f64 {
    let r = a.rem_euclid(TAU);
    if (r - TAU).abs() < 1e-9 {
        0.0
    } else {
        r
    }
}

/// Вход по умолчанию — у первой вершины, чуть сдвинутый внутрь.
fn default_entrance(poly: &Polygon) -> Point {
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

#[cfg(test)]
mod tests {
    use super::*;
    use wh_core::sample::demo_snapshot;

    fn room() -> EditorState {
        let mut s = EditorState::new_empty();
        s.set_tool(Tool::Outline);
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)] {
            s.click(Point::new(x, y), 0.3);
        }
        s.click(Point::new(0.05, 0.05), 0.3); // клик у первой точки замыкает контур
        s
    }

    #[test]
    fn outline_closes_by_clicking_first_point() {
        let s = room();
        assert!(s.has_outline());
        assert_eq!(s.warehouse.outline.points.len(), 4);
        assert!((s.warehouse.outline.area() - 80.0).abs() < 1e-9);
        assert!(s.warehouse.outline.is_ccw());
        assert!(s.warehouse.outline.contains(s.warehouse.entrance));
    }

    #[test]
    fn ortho_snaps_vertices_to_axes() {
        let mut s = EditorState::new_empty();
        s.set_tool(Tool::Outline);
        s.click(Point::new(0.0, 0.0), 0.3);
        s.click(Point::new(10.2, 0.7), 0.3); // почти горизонталь
        assert_eq!(s.draft[1].y, 0.0);
        s.ortho = false;
        s.click(Point::new(10.2, 5.0), 0.3);
        assert!((s.draft[2].x - 10.2).abs() < 1e-9);
    }

    #[test]
    fn degenerate_outline_is_rejected() {
        let mut s = EditorState::new_empty();
        s.set_tool(Tool::Outline);
        s.click(Point::new(0.0, 0.0), 0.3);
        s.click(Point::new(5.0, 0.0), 0.3);
        s.close_outline();
        assert!(s.drawing, "две вершины — контур не замкнут");
    }

    #[test]
    fn shelves_are_validated_on_placement() {
        let mut s = room();
        s.set_tool(Tool::Shelf);
        s.click(Point::new(5.0, 4.0), 0.3);
        assert_eq!(s.warehouse.shelves.len(), 1);
        s.click(Point::new(5.5, 4.0), 0.3); // пересечение
        assert_eq!(s.warehouse.shelves.len(), 1);
        s.click(Point::new(0.5, 0.5), 0.3); // выходит за стену
        assert_eq!(s.warehouse.shelves.len(), 1);
    }

    #[test]
    fn rotate_and_delete_selected() {
        let mut s = room();
        s.set_tool(Tool::Shelf);
        s.click(Point::new(5.0, 4.0), 0.3);
        s.set_tool(Tool::Select);
        s.rotate();
        let angle = s.warehouse.shelves[0].footprint.angle;
        assert!((angle - FRAC_PI_2).abs() < 1e-9);
        s.delete_selected();
        assert!(s.warehouse.shelves.is_empty());
    }

    #[test]
    fn select_picks_shelf_under_cursor() {
        let mut s = EditorState::from_snapshot(demo_snapshot());
        s.set_tool(Tool::Select);
        s.click(Point::new(12.0, 3.0), 0.3);
        assert_eq!(s.selected_shelf, Some(ShelfId(1)));
        s.click(Point::new(1.0, 1.0), 0.3);
        assert_eq!(s.selected_shelf, None);
    }

    #[test]
    fn auto_place_puts_top_demand_on_best_shelf() {
        let mut s = EditorState::from_snapshot(demo_snapshot());
        assert!(s.analyze());
        s.auto_place();
        let best = s
            .access
            .iter()
            .filter(|a| a.distance.is_some())
            .max_by(|a, b| a.score.total_cmp(&b.score))
            .unwrap()
            .shelf;
        let top = s.products.iter().max_by(|a, b| a.demand.partial_cmp(&b.demand).unwrap()).unwrap().id;
        assert_eq!(s.placement_of(top).map(|p| p.shelf), Some(best));
    }

    #[test]
    fn pin_respects_capacity() {
        let mut s = EditorState::from_snapshot(demo_snapshot());
        s.add_product("Гигант", "7", "").unwrap(); // 7 > 6
        s.selected_product = Some(s.products.last().unwrap().id);
        s.selected_shelf = Some(ShelfId(1));
        s.pin_selected();
        assert!(s.placements.is_empty());

        s.selected_product = Some(ProductId(1)); // размер 2
        s.pin_selected();
        assert_eq!(s.placements.len(), 1);
        assert!(s.placements[0].pinned);
    }

    #[test]
    fn product_form_validation() {
        let mut s = EditorState::new_empty();
        assert!(s.add_product("", "1", "").is_err());
        assert!(s.add_product("X", "abc", "").is_err());
        assert!(s.add_product("X", "0", "").is_err());
        assert!(s.add_product("X", "2", "1,5").is_ok());
        assert_eq!(s.products[0].demand, Some(1.5));
    }

    #[test]
    fn geometry_edit_invalidates_analysis() {
        let mut s = EditorState::from_snapshot(demo_snapshot());
        assert!(s.analyze());
        assert!(!s.access.is_empty());
        s.selected_shelf = Some(ShelfId(1));
        s.toggle_double_sided();
        assert!(s.access.is_empty());
    }
}
