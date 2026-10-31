use wh_core::ValidationIssue;
use wh_nav::{analyze as analyze_access, NavConfig};

use super::EditorState;

impl EditorState {
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

    pub fn auto_place(&mut self) {
        if self.access.len() != self.warehouse.shelves.len() && !self.analyze() {
            return;
        }
        if self.warehouse.shelves.is_empty() {
            self.status = "Нет стеллажей".into();
            return;
        }
        let plan = wh_layout::auto_place(
            &self.warehouse,
            &self.access,
            &self.products,
            &self.placements,
        );
        self.placements = plan.placements;
        self.status = if plan.unplaced.is_empty() {
            format!("Размещено товаров: {}", self.placements.len())
        } else {
            format!(
                "Размещено {}, не поместилось {}",
                self.placements.len(),
                plan.unplaced.len()
            )
        };
    }
}

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
