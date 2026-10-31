use wh_core::geometry::{OrientedRect, Point, Polygon};
use wh_core::model::{Placement, Product, ProductId, Shelf, ShelfId, Warehouse, WarehouseId};
use wh_core::Snapshot;
use wh_storage::{
    CatalogRepository, PlacementRepository, Storage, StorageError, WarehouseRepository,
};
use wh_storage_postgres::PostgresStorage;

fn storage() -> Option<PostgresStorage> {
    let url = std::env::var("WH_TEST_DATABASE_URL").ok()?;
    let s = PostgresStorage::connect(&url).expect("connect");
    s.migrate().expect("migrate");
    Some(s)
}

fn warehouse(id: u64) -> Warehouse {
    Warehouse {
        id: WarehouseId(id),
        name: format!("W{id}"),
        outline: Polygon::new(vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ]),
        entrance: Point::new(1.0, 0.5),
        shelves: vec![
            Shelf::new(
                ShelfId(1),
                "A",
                OrientedRect::new(Point::new(3.0, 5.0), 3.0, 1.0, 0.0),
                4,
            ),
            Shelf::new(
                ShelfId(2),
                "B",
                OrientedRect::new(Point::new(7.0, 5.0), 3.0, 1.0, 0.3),
                6,
            ),
        ],
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

fn placement(product: u64, shelf: u64) -> Placement {
    Placement {
        product: ProductId(product),
        shelf: ShelfId(shelf),
        pinned: false,
    }
}

fn reset(s: &PostgresStorage, id: u64) {
    let _ = s.delete_warehouse(WarehouseId(id));
}

#[test]
fn crud_roundtrip_through_trait_object() {
    let Some(pg) = storage() else { return };
    reset(&pg, 9001);
    let storage: Box<dyn Storage> = Box::new(pg);
    let w = WarehouseId(9001);

    assert!(matches!(
        storage.get_warehouse(w),
        Err(StorageError::NotFound(_))
    ));
    storage.save_warehouse(warehouse(9001)).unwrap();
    assert_eq!(storage.get_warehouse(w).unwrap(), warehouse(9001));
    assert!(storage
        .list_warehouses()
        .unwrap()
        .contains(&(w, "W9001".to_string())));

    storage.upsert_product(w, product(1)).unwrap();
    let mut renamed = product(1);
    renamed.name = "renamed".into();
    renamed.demand = Some(0.5);
    storage.upsert_product(w, renamed).unwrap();
    let products = storage.list_products(w).unwrap();
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].name, "renamed");
    assert_eq!(products[0].demand, Some(0.5));

    storage.delete_warehouse(w).unwrap();
    assert!(matches!(
        storage.delete_warehouse(w),
        Err(StorageError::NotFound(_))
    ));
}

#[test]
fn deleting_product_drops_its_placements() {
    let Some(s) = storage() else { return };
    reset(&s, 9002);
    let w = WarehouseId(9002);
    s.save_warehouse(warehouse(9002)).unwrap();
    s.upsert_product(w, product(1)).unwrap();
    s.replace_placements(w, vec![placement(1, 1)]).unwrap();

    s.delete_product(w, ProductId(1)).unwrap();
    assert!(s.get_placements(w).unwrap().is_empty());
    assert!(s.delete_product(w, ProductId(1)).is_err());
    reset(&s, 9002);
}

#[test]
fn unknown_warehouse_is_an_error() {
    let Some(s) = storage() else { return };
    reset(&s, 9003);
    let w = WarehouseId(9003);
    assert!(matches!(s.list_products(w), Err(StorageError::NotFound(_))));
    assert!(matches!(
        s.replace_placements(w, vec![]),
        Err(StorageError::NotFound(_))
    ));
}

#[test]
fn saving_warehouse_keeps_placements_of_remaining_shelves() {
    let Some(s) = storage() else { return };
    reset(&s, 9004);
    let w = WarehouseId(9004);
    s.save_warehouse(warehouse(9004)).unwrap();
    s.upsert_product(w, product(1)).unwrap();
    s.upsert_product(w, product(2)).unwrap();
    s.replace_placements(w, vec![placement(1, 1), placement(2, 2)])
        .unwrap();

    let mut edited = warehouse(9004);
    edited.move_shelf(ShelfId(1), 1.0, 0.0);
    edited.shelves.retain(|sh| sh.id == ShelfId(1));
    s.save_warehouse(edited.clone()).unwrap();

    assert_eq!(s.get_warehouse(w).unwrap(), edited);
    assert_eq!(s.get_placements(w).unwrap(), vec![placement(1, 1)]);
    reset(&s, 9004);
}

#[test]
fn snapshot_roundtrip_is_atomic_and_ordered() {
    let Some(s) = storage() else { return };
    reset(&s, 9005);
    let w = WarehouseId(9005);
    let snap = Snapshot {
        warehouse: warehouse(9005),
        products: vec![product(30), product(10), product(20)],
        placements: vec![placement(20, 2), placement(10, 1)],
    };
    s.save_snapshot(&snap).unwrap();
    assert_eq!(s.load_snapshot(w).unwrap(), snap);

    let smaller = Snapshot {
        warehouse: snap.warehouse.clone(),
        products: vec![product(10)],
        placements: vec![placement(10, 1)],
    };
    s.save_snapshot(&smaller).unwrap();
    assert_eq!(s.load_snapshot(w).unwrap(), smaller);

    let broken = Snapshot {
        warehouse: snap.warehouse.clone(),
        products: vec![product(10), product(99)],
        placements: vec![placement(10, 777)],
    };
    assert!(s.save_snapshot(&broken).is_err());
    assert_eq!(s.load_snapshot(w).unwrap(), smaller);
    reset(&s, 9005);
}
