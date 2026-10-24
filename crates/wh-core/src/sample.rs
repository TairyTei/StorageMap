use std::f64::consts::{FRAC_PI_2, PI};

use crate::geometry::{OrientedRect, Point, Polygon};
use crate::model::{Product, ProductId, Shelf, ShelfId, Snapshot, Warehouse, WarehouseId};

pub fn demo_warehouse() -> Warehouse {
    let shelf = |id, label: &str, x, y, angle| {
        Shelf::new(
            ShelfId(id),
            label,
            OrientedRect::new(Point::new(x, y), 3.0, 1.0, angle),
            6,
        )
    };
    Warehouse {
        id: WarehouseId(1),
        name: "Demo (L-образный)".into(),
        outline: Polygon::new(vec![
            Point::new(0.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(20.0, 8.0),
            Point::new(8.0, 8.0),
            Point::new(8.0, 16.0),
            Point::new(0.0, 16.0),
        ]),
        entrance: Point::new(10.0, 1.0),
        shelves: vec![
            shelf(1, "S1", 12.0, 3.0, 0.0),
            shelf(2, "S2", 17.0, 3.0, 0.0),
            shelf(3, "S3", 12.0, 6.5, PI),
            shelf(4, "S4", 17.0, 6.5, PI),
            shelf(5, "S5", 3.0, 11.5, -FRAC_PI_2),
            shelf(6, "S6", 3.0, 14.5, -FRAC_PI_2),
        ],
    }
}

pub fn demo_products() -> Vec<Product> {
    let p = |id, name: &str, size, demand| Product {
        id: ProductId(id),
        sku: format!("SKU-{id:04}"),
        name: name.into(),
        size,
        demand,
    };
    vec![
        p(1, "Молоко", 2, Some(95.0)),
        p(2, "Хлеб", 2, Some(90.0)),
        p(3, "Макароны", 3, Some(60.0)),
        p(4, "Консервы", 4, Some(30.0)),
        p(5, "Свечи", 1, Some(2.0)),
        p(6, "Сувениры", 4, Some(5.0)),
        p(7, "Новинка", 3, None),
        p(8, "Крупа", 4, Some(55.0)),
    ]
}

pub fn demo_snapshot() -> Snapshot {
    Snapshot {
        warehouse: demo_warehouse(),
        products: demo_products(),
        placements: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_is_valid() {
        assert_eq!(demo_warehouse().validate(), vec![]);
    }
}
