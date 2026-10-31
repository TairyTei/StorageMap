use wh_core::{Placement, Product, ProductId, ShelfId};

use super::EditorState;

impl EditorState {
    pub fn add_product(&mut self, name: &str, size: &str, demand: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Введите название товара".into());
        }
        let size = if size.trim().is_empty() {
            1
        } else {
            size.trim()
                .parse::<u32>()
                .map_err(|_| "Размер — целое число ячеек".to_string())?
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
        self.products.push(Product {
            id,
            sku: format!("SKU-{:04}", id.0),
            name: name.into(),
            size,
            demand,
        });
        self.status = format!("Товар «{name}» добавлен");
        Ok(())
    }

    pub fn placement_of(&self, product: ProductId) -> Option<&Placement> {
        self.placements.iter().find(|p| p.product == product)
    }

    pub(super) fn products_on(&self, shelf: ShelfId) -> impl Iterator<Item = &Product> + '_ {
        self.placements
            .iter()
            .filter(move |p| p.shelf == shelf)
            .filter_map(move |p| self.products.iter().find(|x| x.id == p.product))
    }

    pub fn shelf_used(&self, shelf: ShelfId) -> u32 {
        self.products_on(shelf).map(|p| p.size).sum()
    }

    pub fn product_rows(&self, query: &str) -> Vec<(ProductId, String, bool)> {
        let q = query.trim().to_lowercase();
        let mut items: Vec<&Product> = self
            .products
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.name.to_lowercase().contains(&q)
                    || p.sku.to_lowercase().contains(&q)
            })
            .collect();
        items.sort_by(|a, b| {
            let (da, db) = (
                a.demand.unwrap_or(f32::NEG_INFINITY),
                b.demand.unwrap_or(f32::NEG_INFINITY),
            );
            db.total_cmp(&da).then(a.id.cmp(&b.id))
        });
        items
            .into_iter()
            .map(|p| {
                let place = match self.placement_of(p.id) {
                    Some(pl) => {
                        let label = self
                            .warehouse
                            .shelf(pl.shelf)
                            .map_or("?", |s| s.label.as_str());
                        format!("→ {label}{}", if pl.pinned { " [закреп.]" } else { "" })
                    }
                    None => "→ —".to_string(),
                };
                let demand = p.demand.map_or("—".to_string(), |d| format!("{d}"));
                (
                    p.id,
                    format!("{} · спрос {} · {} яч. {}", p.name, demand, p.size, place),
                    self.selected_product == Some(p.id),
                )
            })
            .collect()
    }

    pub fn pin_selected(&mut self) {
        let (Some(pid), Some(sid)) = (self.selected_product, self.selected_shelf) else {
            self.status = "Выберите товар в списке и стеллаж на карте".into();
            return;
        };
        let Some(size) = self.products.iter().find(|p| p.id == pid).map(|p| p.size) else {
            return;
        };
        let Some(capacity) = self.warehouse.shelf(sid).map(|s| s.capacity) else {
            return;
        };
        let used_by_others: u32 = self
            .products_on(sid)
            .filter(|p| p.id != pid)
            .map(|p| p.size)
            .sum();
        if used_by_others + size > capacity {
            self.status = format!(
                "Не хватает места: свободно {} яч., нужно {size}",
                capacity.saturating_sub(used_by_others)
            );
            return;
        }
        self.placements.retain(|p| p.product != pid);
        self.placements.push(Placement {
            product: pid,
            shelf: sid,
            pinned: true,
        });
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
                self.status =
                    "Закрепление снято: при следующей авторасстановке товар может переехать".into();
            }
            None => self.status = "Товар не размещён".into(),
        }
    }
}
