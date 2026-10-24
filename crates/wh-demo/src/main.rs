// Сквозной сценарий без GUI: склад L-образной формы → доступность → авторасстановка.

use wh_core::sample::{demo_products, demo_warehouse};
use wh_core::{Placement, ProductId, ShelfId};
use wh_layout::auto_place;
use wh_nav::{analyze, NavConfig};
use wh_storage::{CatalogRepository, InMemoryStorage, PlacementRepository, WarehouseRepository};

fn main() {
    let warehouse = demo_warehouse();
    let issues = warehouse.validate();
    assert!(issues.is_empty(), "demo warehouse is invalid: {issues:?}");

    let storage = InMemoryStorage::new();
    storage
        .save_warehouse(warehouse.clone())
        .expect("save warehouse");
    let id = warehouse.id;
    for p in demo_products() {
        storage.upsert_product(id, p).expect("save product");
    }

    let access = analyze(&warehouse, &NavConfig::default());
    println!("Доступность стеллажей:");
    for a in &access {
        let label = &warehouse.shelf(a.shelf).expect("shelf").label;
        match a.distance {
            Some(d) => println!(
                "  {label:<4} маршрут {d:>6.2} м, оценка {:.2}, поворотов: {}",
                a.score,
                a.route.len().saturating_sub(2)
            ),
            None => println!("  {label:<4} недостижим"),
        }
    }

    let products = storage.list_products(id).expect("products");
    let pinned = [Placement {
        product: ProductId(5),
        shelf: ShelfId(1),
        pinned: true,
    }];
    let plan = auto_place(&warehouse, &access, &products, &pinned);
    storage
        .replace_placements(id, plan.placements.clone())
        .expect("save placements");

    println!("\nРазмещение:");
    for pl in &plan.placements {
        let p = products
            .iter()
            .find(|p| p.id == pl.product)
            .expect("product");
        let s = warehouse.shelf(pl.shelf).expect("shelf");
        let pin = if pl.pinned {
            " (закреплён)"
        } else {
            ""
        };
        let demand = p.demand.map_or("—".to_string(), |d| d.to_string());
        println!("  {:<12} спрос {:>5} → {}{pin}", p.name, demand, s.label);
    }
    if !plan.unplaced.is_empty() {
        println!("\nНе поместились: {:?}", plan.unplaced);
    }
}
