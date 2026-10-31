use std::f64::consts::FRAC_PI_2;

use wh_core::{OrientedRect, Point, Shelf, ShelfId};

use super::geometry::normalize_angle;
use super::presets::ShelfPreset;
use super::EditorState;

impl EditorState {
    pub fn pick_shelf(&self, p: Point) -> Option<ShelfId> {
        self.warehouse.shelves.iter().rev().find_map(|s| {
            let l = s.footprint.to_local(p);
            (l.x.abs() <= s.footprint.half_w && l.y.abs() <= s.footprint.half_d).then_some(s.id)
        })
    }

    fn fits(&self, rect: &OrientedRect, ignore: Option<ShelfId>) -> Result<(), String> {
        let c = rect.corners();
        let inside = (0..4).all(|i| {
            self.warehouse
                .outline
                .contains_segment(c[i], c[(i + 1) % 4])
        });
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

    pub(crate) fn place_shelf(&mut self, center: Point, preset: &ShelfPreset, angle: f64) {
        if !self.has_outline() {
            self.status = "Сначала нарисуйте и замкните контур".into();
            return;
        }
        let rect = OrientedRect::new(center, preset.width, preset.depth, angle);
        if let Err(msg) = self.fits(&rect, None) {
            self.status = msg;
            return;
        }
        let id = ShelfId(
            self.warehouse
                .shelves
                .iter()
                .map(|s| s.id.0)
                .max()
                .unwrap_or(0)
                + 1,
        );
        self.warehouse
            .shelves
            .push(Shelf::new(id, format!("S{}", id.0), rect, preset.capacity));
        self.selected_shelf = Some(id);
        self.invalidate_analysis();
        self.status = format!("Стеллаж S{} добавлен", id.0);
    }

    pub(crate) fn move_selected(&mut self, dx: f64, dy: f64) -> bool {
        let Some(id) = self.selected_shelf else {
            return false;
        };
        self.warehouse.move_shelf(id, dx, dy);
        self.invalidate_analysis();
        true
    }

    pub(crate) fn rotate_selected(&mut self) {
        let Some(id) = self.selected_shelf else {
            self.status = "Выберите стеллаж".into();
            return;
        };
        let Some(shelf) = self.warehouse.shelf(id) else {
            return;
        };
        let mut rect = shelf.footprint;
        rect.angle = normalize_angle(rect.angle + FRAC_PI_2);
        match self.fits(&rect, Some(id)) {
            Ok(()) => {
                if let Some(s) = self.warehouse.shelves.iter_mut().find(|s| s.id == id) {
                    s.footprint = rect;
                }
                self.invalidate_analysis();
                self.status = "Стеллаж повёрнут".into();
            }
            Err(msg) => self.status = format!("Нельзя повернуть: {msg}"),
        }
    }

    pub fn delete_selected(&mut self) {
        let Some(id) = self.selected_shelf.take() else {
            self.status = "Выберите стеллаж".into();
            return;
        };
        self.warehouse.shelves.retain(|s| s.id != id);
        self.placements.retain(|p| p.shelf != id);
        self.invalidate_analysis();
        self.status = "Стеллаж удалён".into();
    }

    pub fn toggle_double_sided(&mut self) {
        let Some(id) = self.selected_shelf else {
            self.status = "Выберите стеллаж".into();
            return;
        };
        if let Some(s) = self.warehouse.shelves.iter_mut().find(|s| s.id == id) {
            s.double_sided = !s.double_sided;
            let text = if s.double_sided {
                "Доступ с двух сторон"
            } else {
                "Доступ только спереди"
            };
            self.status = text.to_string();
        }
        self.invalidate_analysis();
    }

    pub fn shelf_info(&self) -> String {
        let Some(id) = self.selected_shelf else {
            return String::new();
        };
        let Some(shelf) = self.warehouse.shelf(id) else {
            return String::new();
        };
        let fp = &shelf.footprint;
        let mut s = format!(
            "{} — {:.1}×{:.1} м{}\nЗанято {}/{} яч.",
            shelf.label,
            fp.half_w * 2.0,
            fp.half_d * 2.0,
            if shelf.double_sided {
                ", двусторонний"
            } else {
                ""
            },
            self.shelf_used(id),
            shelf.capacity
        );
        if let Some(a) = self.access.iter().find(|a| a.shelf == id) {
            match a.distance {
                Some(d) => s.push_str(&format!(
                    " · маршрут {d:.1} м · доступность {:.0}%",
                    a.score * 100.0
                )),
                None => s.push_str(" · недостижим"),
            }
        }
        let names: Vec<String> = self
            .products_on(id)
            .map(|x| format!("{} ({})", x.name, x.size))
            .collect();
        if !names.is_empty() {
            s.push_str("\nТовары: ");
            s.push_str(&names.join(", "));
        }
        s
    }
}
