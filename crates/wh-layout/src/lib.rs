use std::collections::{HashMap, HashSet};

use wh_core::model::{Placement, Product, ProductId, ShelfAccess, ShelfId, Warehouse};

#[derive(Debug, Clone, PartialEq)]
pub struct PlacementPlan {
    pub placements: Vec<Placement>,
    pub unplaced: Vec<ProductId>,
}

pub fn auto_place(
    warehouse: &Warehouse,
    access: &[ShelfAccess],
    products: &[Product],
    existing: &[Placement],
) -> PlacementPlan {
    let by_id: HashMap<ProductId, &Product> = products.iter().map(|p| (p.id, p)).collect();
    let mut free: HashMap<ShelfId, u32> = warehouse
        .shelves
        .iter()
        .map(|s| (s.id, s.capacity))
        .collect();

    let mut placements = Vec::new();
    let mut placed: HashSet<ProductId> = HashSet::new();

    for pl in existing.iter().filter(|p| p.pinned) {
        let (Some(product), Some(room)) = (by_id.get(&pl.product), free.get_mut(&pl.shelf)) else {
            continue;
        };
        *room = room.saturating_sub(product.size);
        placed.insert(pl.product);
        placements.push(pl.clone());
    }

    let mut ranked: Vec<&ShelfAccess> = access.iter().filter(|a| a.distance.is_some()).collect();
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(
                a.distance
                    .unwrap_or(f64::MAX)
                    .total_cmp(&b.distance.unwrap_or(f64::MAX)),
            )
            .then(a.shelf.cmp(&b.shelf))
    });

    let mut queue: Vec<&Product> = products
        .iter()
        .filter(|p| !placed.contains(&p.id))
        .collect();
    queue.sort_by(|a, b| {
        let da = a.demand.unwrap_or(f32::NEG_INFINITY);
        let db = b.demand.unwrap_or(f32::NEG_INFINITY);
        db.total_cmp(&da).then(a.id.cmp(&b.id))
    });

    let mut unplaced = Vec::new();
    for product in queue {
        let target = ranked
            .iter()
            .map(|a| a.shelf)
            .find(|id| free.get(id).is_some_and(|&room| room >= product.size));
        match target {
            Some(shelf) => {
                if let Some(room) = free.get_mut(&shelf) {
                    *room -= product.size;
                }
                placements.push(Placement {
                    product: product.id,
                    shelf,
                    pinned: false,
                });
            }
            None => unplaced.push(product.id),
        }
    }

    PlacementPlan {
        placements,
        unplaced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wh_core::geometry::{OrientedRect, Point, Polygon};
    use wh_core::model::{Shelf, WarehouseId};

    fn warehouse(shelf_count: u64, capacity: u32) -> Warehouse {
        Warehouse {
            id: WarehouseId(1),
            name: "t".into(),
            outline: Polygon::new(vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
                Point::new(0.0, 10.0),
            ]),
            entrance: Point::new(0.5, 0.5),
            shelves: (1..=shelf_count)
                .map(|i| {
                    let fp = OrientedRect::new(Point::new(i as f64 * 2.0, 5.0), 1.0, 1.0, 0.0);
                    Shelf::new(ShelfId(i), format!("S{i}"), fp, capacity)
                })
                .collect(),
        }
    }

    fn access(shelf: u64, score: f64) -> ShelfAccess {
        ShelfAccess {
            shelf: ShelfId(shelf),
            distance: Some(10.0 * (1.0 - score) + 1.0),
            score,
            route: vec![],
        }
    }

    fn product(id: u64, demand: Option<f32>) -> Product {
        Product {
            id: ProductId(id),
            sku: format!("SKU{id}"),
            name: format!("P{id}"),
            size: 2,
            demand,
        }
    }

    fn shelf_of(plan: &PlacementPlan, product: u64) -> Option<ShelfId> {
        plan.placements
            .iter()
            .find(|p| p.product == ProductId(product))
            .map(|p| p.shelf)
    }

    #[test]
    fn demand_goes_to_best_shelves() {
        let w = warehouse(3, 2);
        let acc = [access(1, 0.0), access(2, 1.0), access(3, 0.5)];
        let products = [
            product(10, Some(1.0)),
            product(11, Some(9.0)),
            product(12, Some(5.0)),
        ];
        let plan = auto_place(&w, &acc, &products, &[]);

        assert_eq!(shelf_of(&plan, 11), Some(ShelfId(2)));
        assert_eq!(shelf_of(&plan, 12), Some(ShelfId(3)));
        assert_eq!(shelf_of(&plan, 10), Some(ShelfId(1)));
        assert!(plan.unplaced.is_empty());
    }

    #[test]
    fn products_without_demand_come_last_and_overflow_is_reported() {
        let w = warehouse(2, 2);
        let acc = [access(1, 1.0), access(2, 0.0)];
        let products = [
            product(1, None),
            product(2, Some(3.0)),
            product(3, Some(1.0)),
        ];
        let plan = auto_place(&w, &acc, &products, &[]);

        assert_eq!(plan.unplaced, vec![ProductId(1)]);
        assert_eq!(shelf_of(&plan, 2), Some(ShelfId(1)));
    }

    #[test]
    fn pinned_placements_are_respected() {
        let w = warehouse(3, 2);
        let acc = [access(1, 1.0), access(2, 0.5), access(3, 0.0)];
        let products = [
            product(1, Some(9.0)),
            product(2, Some(5.0)),
            product(3, Some(1.0)),
        ];

        let pinned = Placement {
            product: ProductId(3),
            shelf: ShelfId(1),
            pinned: true,
        };
        let stale = Placement {
            product: ProductId(1),
            shelf: ShelfId(3),
            pinned: false,
        };
        let plan = auto_place(&w, &acc, &products, &[pinned, stale]);

        assert_eq!(shelf_of(&plan, 3), Some(ShelfId(1)));
        assert_eq!(shelf_of(&plan, 1), Some(ShelfId(2)));
        assert_eq!(shelf_of(&plan, 2), Some(ShelfId(3)));
    }

    #[test]
    fn unreachable_shelves_are_skipped() {
        let w = warehouse(2, 2);
        let acc = [
            ShelfAccess {
                shelf: ShelfId(1),
                distance: None,
                score: 0.0,
                route: vec![],
            },
            access(2, 0.3),
        ];
        let plan = auto_place(&w, &acc, &[product(1, Some(1.0))], &[]);
        assert_eq!(shelf_of(&plan, 1), Some(ShelfId(2)));
    }
}
