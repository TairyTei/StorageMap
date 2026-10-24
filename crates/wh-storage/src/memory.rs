use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use wh_core::model::{Placement, Product, ProductId, Warehouse, WarehouseId};

use crate::{CatalogRepository, PlacementRepository, Result, StorageError, WarehouseRepository};

#[derive(Default)]
struct State {
    warehouses: BTreeMap<WarehouseId, Warehouse>,
    products: BTreeMap<WarehouseId, Vec<Product>>,
    placements: BTreeMap<WarehouseId, Vec<Placement>>,
}

#[derive(Default)]
pub struct InMemoryStorage {
    state: Mutex<State>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| StorageError::Backend("mutex poisoned".into()))
    }
}

fn require_warehouse(state: &State, id: WarehouseId) -> Result<()> {
    if state.warehouses.contains_key(&id) {
        Ok(())
    } else {
        Err(StorageError::NotFound(format!("warehouse {id}")))
    }
}

impl WarehouseRepository for InMemoryStorage {
    fn list_warehouses(&self) -> Result<Vec<(WarehouseId, String)>> {
        Ok(self
            .lock()?
            .warehouses
            .values()
            .map(|w| (w.id, w.name.clone()))
            .collect())
    }

    fn get_warehouse(&self, id: WarehouseId) -> Result<Warehouse> {
        self.lock()?
            .warehouses
            .get(&id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(format!("warehouse {id}")))
    }

    fn save_warehouse(&self, warehouse: Warehouse) -> Result<()> {
        self.lock()?.warehouses.insert(warehouse.id, warehouse);
        Ok(())
    }

    fn delete_warehouse(&self, id: WarehouseId) -> Result<()> {
        let mut s = self.lock()?;
        s.products.remove(&id);
        s.placements.remove(&id);
        s.warehouses
            .remove(&id)
            .map(|_| ())
            .ok_or_else(|| StorageError::NotFound(format!("warehouse {id}")))
    }
}

impl CatalogRepository for InMemoryStorage {
    fn list_products(&self, warehouse: WarehouseId) -> Result<Vec<Product>> {
        let s = self.lock()?;
        require_warehouse(&s, warehouse)?;
        Ok(s.products.get(&warehouse).cloned().unwrap_or_default())
    }

    fn upsert_product(&self, warehouse: WarehouseId, product: Product) -> Result<()> {
        let mut s = self.lock()?;
        require_warehouse(&s, warehouse)?;
        let list = s.products.entry(warehouse).or_default();
        match list.iter_mut().find(|p| p.id == product.id) {
            Some(slot) => *slot = product,
            None => list.push(product),
        }
        Ok(())
    }

    fn delete_product(&self, warehouse: WarehouseId, product: ProductId) -> Result<()> {
        let mut s = self.lock()?;
        require_warehouse(&s, warehouse)?;
        let list = s.products.entry(warehouse).or_default();
        let before = list.len();
        list.retain(|p| p.id != product);
        if list.len() == before {
            return Err(StorageError::NotFound(format!("product {product}")));
        }
        if let Some(pl) = s.placements.get_mut(&warehouse) {
            pl.retain(|p| p.product != product);
        }
        Ok(())
    }
}

impl PlacementRepository for InMemoryStorage {
    fn get_placements(&self, warehouse: WarehouseId) -> Result<Vec<Placement>> {
        let s = self.lock()?;
        require_warehouse(&s, warehouse)?;
        Ok(s.placements.get(&warehouse).cloned().unwrap_or_default())
    }

    fn replace_placements(&self, warehouse: WarehouseId, placements: Vec<Placement>) -> Result<()> {
        let mut s = self.lock()?;
        require_warehouse(&s, warehouse)?;
        s.placements.insert(warehouse, placements);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Storage;
    use wh_core::geometry::{Point, Polygon};
    use wh_core::model::ShelfId;

    fn warehouse(id: u64) -> Warehouse {
        Warehouse {
            id: WarehouseId(id),
            name: format!("W{id}"),
            outline: Polygon::new(vec![
                Point::new(0.0, 0.0),
                Point::new(5.0, 0.0),
                Point::new(5.0, 5.0),
            ]),
            entrance: Point::new(1.0, 0.5),
            shelves: vec![],
        }
    }

    fn product(id: u64) -> Product {
        Product {
            id: ProductId(id),
            sku: format!("S{id}"),
            name: format!("P{id}"),
            size: 1,
            demand: None,
        }
    }

    #[test]
    fn crud_roundtrip_through_trait_object() {
        let storage: Box<dyn Storage> = Box::new(InMemoryStorage::new());
        let w = WarehouseId(1);

        assert!(matches!(
            storage.get_warehouse(w),
            Err(StorageError::NotFound(_))
        ));
        storage.save_warehouse(warehouse(1)).unwrap();
        assert_eq!(
            storage.list_warehouses().unwrap(),
            vec![(w, "W1".to_string())]
        );

        storage.upsert_product(w, product(1)).unwrap();
        let mut renamed = product(1);
        renamed.name = "renamed".into();
        storage.upsert_product(w, renamed).unwrap();
        let products = storage.list_products(w).unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0].name, "renamed");
    }

    #[test]
    fn deleting_product_drops_its_placements() {
        let storage = InMemoryStorage::new();
        let w = WarehouseId(1);
        storage.save_warehouse(warehouse(1)).unwrap();
        storage.upsert_product(w, product(1)).unwrap();
        storage
            .replace_placements(
                w,
                vec![Placement {
                    product: ProductId(1),
                    shelf: ShelfId(1),
                    pinned: false,
                }],
            )
            .unwrap();

        storage.delete_product(w, ProductId(1)).unwrap();
        assert!(storage.get_placements(w).unwrap().is_empty());
        assert!(storage.delete_product(w, ProductId(1)).is_err());
    }

    #[test]
    fn unknown_warehouse_is_an_error() {
        let storage = InMemoryStorage::new();
        assert!(storage.list_products(WarehouseId(9)).is_err());
        assert!(storage.replace_placements(WarehouseId(9), vec![]).is_err());
    }
}
