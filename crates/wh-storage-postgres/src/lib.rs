use std::sync::{Mutex, MutexGuard};

use postgres::{Client, GenericClient, NoTls};
use wh_core::geometry::{OrientedRect, Point, Polygon};
use wh_core::model::{Placement, Product, ProductId, Shelf, ShelfId, Warehouse, WarehouseId};
use wh_core::Snapshot;
use wh_storage::{
    CatalogRepository, PlacementRepository, Result, StorageError, WarehouseRepository,
};

const SCHEMA_SQL: &str = include_str!("../migrations/001_init.sql");

pub struct PostgresStorage {
    client: Mutex<Client>,
}

impl PostgresStorage {
    pub fn connect(conn: &str) -> Result<Self> {
        let client = Client::connect(conn, NoTls).map_err(be)?;
        Ok(Self::from_client(client))
    }

    pub fn from_client(client: Client) -> Self {
        Self {
            client: Mutex::new(client),
        }
    }

    pub fn migrate(&self) -> Result<()> {
        self.lock()?.batch_execute(SCHEMA_SQL).map_err(be)
    }

    pub fn save_snapshot(&self, s: &Snapshot) -> Result<()> {
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        let wid = id_out(s.warehouse.id.0)?;

        store_warehouse(&mut tx, &s.warehouse)?;

        let keep: Vec<i64> = s
            .products
            .iter()
            .map(|p| id_out(p.id.0))
            .collect::<Result<_>>()?;
        tx.execute(
            "DELETE FROM wh.products WHERE warehouse_id = $1 AND id <> ALL($2)",
            &[&wid, &keep],
        )
        .map_err(be)?;
        for p in &s.products {
            store_product(&mut tx, wid, p)?;
        }
        store_placements(&mut tx, wid, &s.placements)?;

        tx.commit().map_err(be)
    }

    pub fn load_snapshot(&self, id: WarehouseId) -> Result<Snapshot> {
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        let warehouse = load_warehouse(&mut tx, id)?;
        let wid = id_out(id.0)?;
        let products = load_products(&mut tx, wid)?;
        let placements = load_placements(&mut tx, wid)?;
        tx.commit().map_err(be)?;
        Ok(Snapshot {
            warehouse,
            products,
            placements,
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Client>> {
        self.client
            .lock()
            .map_err(|_| StorageError::Backend("mutex poisoned".into()))
    }
}

fn be(e: postgres::Error) -> StorageError {
    match e.as_db_error() {
        Some(db) => {
            StorageError::Backend(format!("postgres {}: {}", db.code().code(), db.message()))
        }
        None => StorageError::Backend(e.to_string()),
    }
}

fn id_out(v: u64) -> Result<i64> {
    i64::try_from(v).map_err(|_| StorageError::Backend(format!("id {v} does not fit BIGINT")))
}

fn id_in(v: i64) -> Result<u64> {
    u64::try_from(v).map_err(|_| StorageError::Backend(format!("negative id {v} in database")))
}

fn u32_in(v: i64, what: &str) -> Result<u32> {
    u32::try_from(v).map_err(|_| StorageError::Backend(format!("{what} {v} out of u32 range")))
}

fn require_warehouse<C: GenericClient>(c: &mut C, wid: i64) -> Result<()> {
    let found = c
        .query_opt("SELECT 1 FROM wh.warehouses WHERE id = $1", &[&wid])
        .map_err(be)?;
    match found {
        Some(_) => Ok(()),
        None => Err(StorageError::NotFound(format!("warehouse {wid}"))),
    }
}

fn load_warehouse<C: GenericClient>(c: &mut C, id: WarehouseId) -> Result<Warehouse> {
    let wid = id_out(id.0)?;
    let row = c
        .query_opt(
            "SELECT name, entrance_x, entrance_y FROM wh.warehouses WHERE id = $1",
            &[&wid],
        )
        .map_err(be)?
        .ok_or_else(|| StorageError::NotFound(format!("warehouse {id}")))?;
    let name: String = row.get(0);
    let entrance = Point::new(row.get(1), row.get(2));

    let outline: Vec<Point> = c
        .query(
            "SELECT x, y FROM wh.outline_points WHERE warehouse_id = $1 ORDER BY idx",
            &[&wid],
        )
        .map_err(be)?
        .iter()
        .map(|r| Point::new(r.get(0), r.get(1)))
        .collect();

    let rows = c
        .query(
            "SELECT id, label, cx, cy, half_w, half_d, angle, double_sided, capacity \
             FROM wh.shelves WHERE warehouse_id = $1 ORDER BY pos",
            &[&wid],
        )
        .map_err(be)?;
    let mut shelves = Vec::with_capacity(rows.len());
    for r in &rows {
        shelves.push(Shelf {
            id: ShelfId(id_in(r.get(0))?),
            label: r.get(1),
            footprint: OrientedRect {
                center: Point::new(r.get(2), r.get(3)),
                half_w: r.get(4),
                half_d: r.get(5),
                angle: r.get(6),
            },
            double_sided: r.get(7),
            capacity: u32_in(r.get(8), "shelf capacity")?,
        });
    }

    Ok(Warehouse {
        id,
        name,
        outline: Polygon::new(outline),
        entrance,
        shelves,
    })
}

fn store_warehouse<C: GenericClient>(c: &mut C, w: &Warehouse) -> Result<()> {
    let wid = id_out(w.id.0)?;
    c.execute(
        "INSERT INTO wh.warehouses (id, name, entrance_x, entrance_y) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (id) DO UPDATE SET \
           name = EXCLUDED.name, entrance_x = EXCLUDED.entrance_x, entrance_y = EXCLUDED.entrance_y",
        &[&wid, &w.name, &w.entrance.x, &w.entrance.y],
    )
    .map_err(be)?;

    c.execute(
        "DELETE FROM wh.outline_points WHERE warehouse_id = $1",
        &[&wid],
    )
    .map_err(be)?;
    for (idx, p) in w.outline.points.iter().enumerate() {
        c.execute(
            "INSERT INTO wh.outline_points (warehouse_id, idx, x, y) VALUES ($1, $2, $3, $4)",
            &[&wid, &(idx as i32), &p.x, &p.y],
        )
        .map_err(be)?;
    }

    let keep: Vec<i64> = w
        .shelves
        .iter()
        .map(|s| id_out(s.id.0))
        .collect::<Result<_>>()?;
    c.execute(
        "DELETE FROM wh.shelves WHERE warehouse_id = $1 AND id <> ALL($2)",
        &[&wid, &keep],
    )
    .map_err(be)?;
    for (pos, s) in w.shelves.iter().enumerate() {
        let f = &s.footprint;
        c.execute(
            "INSERT INTO wh.shelves \
               (warehouse_id, id, pos, label, cx, cy, half_w, half_d, angle, double_sided, capacity) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
             ON CONFLICT (warehouse_id, id) DO UPDATE SET \
               pos = EXCLUDED.pos, label = EXCLUDED.label, cx = EXCLUDED.cx, cy = EXCLUDED.cy, \
               half_w = EXCLUDED.half_w, half_d = EXCLUDED.half_d, angle = EXCLUDED.angle, \
               double_sided = EXCLUDED.double_sided, capacity = EXCLUDED.capacity",
            &[
                &wid,
                &id_out(s.id.0)?,
                &(pos as i32),
                &s.label,
                &f.center.x,
                &f.center.y,
                &f.half_w,
                &f.half_d,
                &f.angle,
                &s.double_sided,
                &i64::from(s.capacity),
            ],
        )
        .map_err(be)?;
    }
    Ok(())
}

impl WarehouseRepository for PostgresStorage {
    fn list_warehouses(&self) -> Result<Vec<(WarehouseId, String)>> {
        let rows = self
            .lock()?
            .query("SELECT id, name FROM wh.warehouses ORDER BY id", &[])
            .map_err(be)?;
        rows.iter()
            .map(|r| Ok((WarehouseId(id_in(r.get(0))?), r.get::<_, String>(1))))
            .collect()
    }

    fn get_warehouse(&self, id: WarehouseId) -> Result<Warehouse> {
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        let w = load_warehouse(&mut tx, id)?;
        tx.commit().map_err(be)?;
        Ok(w)
    }

    fn save_warehouse(&self, warehouse: Warehouse) -> Result<()> {
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        store_warehouse(&mut tx, &warehouse)?;
        tx.commit().map_err(be)
    }

    fn delete_warehouse(&self, id: WarehouseId) -> Result<()> {
        let wid = id_out(id.0)?;
        // Дочерние таблицы чистятся через ON DELETE CASCADE.
        let n = self
            .lock()?
            .execute("DELETE FROM wh.warehouses WHERE id = $1", &[&wid])
            .map_err(be)?;
        if n == 0 {
            return Err(StorageError::NotFound(format!("warehouse {id}")));
        }
        Ok(())
    }
}

fn load_products<C: GenericClient>(c: &mut C, wid: i64) -> Result<Vec<Product>> {
    let rows = c
        .query(
            "SELECT id, sku, name, size, demand FROM wh.products \
             WHERE warehouse_id = $1 ORDER BY seq",
            &[&wid],
        )
        .map_err(be)?;
    rows.iter()
        .map(|r| {
            Ok(Product {
                id: ProductId(id_in(r.get(0))?),
                sku: r.get(1),
                name: r.get(2),
                size: u32_in(r.get(3), "product size")?,
                demand: r.get(4),
            })
        })
        .collect()
}

fn store_product<C: GenericClient>(c: &mut C, wid: i64, p: &Product) -> Result<()> {
    c.execute(
        "INSERT INTO wh.products (warehouse_id, id, sku, name, size, demand) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (warehouse_id, id) DO UPDATE SET \
           sku = EXCLUDED.sku, name = EXCLUDED.name, size = EXCLUDED.size, demand = EXCLUDED.demand",
        &[&wid, &id_out(p.id.0)?, &p.sku, &p.name, &i64::from(p.size), &p.demand],
    )
    .map_err(be)?;
    Ok(())
}

impl CatalogRepository for PostgresStorage {
    fn list_products(&self, warehouse: WarehouseId) -> Result<Vec<Product>> {
        let wid = id_out(warehouse.0)?;
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        require_warehouse(&mut tx, wid)?;
        let v = load_products(&mut tx, wid)?;
        tx.commit().map_err(be)?;
        Ok(v)
    }

    fn upsert_product(&self, warehouse: WarehouseId, product: Product) -> Result<()> {
        let wid = id_out(warehouse.0)?;
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        require_warehouse(&mut tx, wid)?;
        store_product(&mut tx, wid, &product)?;
        tx.commit().map_err(be)
    }

    fn delete_product(&self, warehouse: WarehouseId, product: ProductId) -> Result<()> {
        let wid = id_out(warehouse.0)?;
        let pid = id_out(product.0)?;
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        require_warehouse(&mut tx, wid)?;
        // Размещения товара удаляются каскадом.
        let n = tx
            .execute(
                "DELETE FROM wh.products WHERE warehouse_id = $1 AND id = $2",
                &[&wid, &pid],
            )
            .map_err(be)?;
        if n == 0 {
            return Err(StorageError::NotFound(format!("product {product}")));
        }
        tx.commit().map_err(be)
    }
}

fn load_placements<C: GenericClient>(c: &mut C, wid: i64) -> Result<Vec<Placement>> {
    let rows = c
        .query(
            "SELECT product_id, shelf_id, pinned FROM wh.placements \
             WHERE warehouse_id = $1 ORDER BY seq",
            &[&wid],
        )
        .map_err(be)?;
    rows.iter()
        .map(|r| {
            Ok(Placement {
                product: ProductId(id_in(r.get(0))?),
                shelf: ShelfId(id_in(r.get(1))?),
                pinned: r.get(2),
            })
        })
        .collect()
}

fn store_placements<C: GenericClient>(c: &mut C, wid: i64, list: &[Placement]) -> Result<()> {
    c.execute("DELETE FROM wh.placements WHERE warehouse_id = $1", &[&wid])
        .map_err(be)?;
    for p in list {
        c.execute(
            "INSERT INTO wh.placements (warehouse_id, product_id, shelf_id, pinned) \
             VALUES ($1, $2, $3, $4)",
            &[&wid, &id_out(p.product.0)?, &id_out(p.shelf.0)?, &p.pinned],
        )
        .map_err(be)?;
    }
    Ok(())
}

impl PlacementRepository for PostgresStorage {
    fn get_placements(&self, warehouse: WarehouseId) -> Result<Vec<Placement>> {
        let wid = id_out(warehouse.0)?;
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        require_warehouse(&mut tx, wid)?;
        let v = load_placements(&mut tx, wid)?;
        tx.commit().map_err(be)?;
        Ok(v)
    }

    fn replace_placements(&self, warehouse: WarehouseId, placements: Vec<Placement>) -> Result<()> {
        let wid = id_out(warehouse.0)?;
        let mut c = self.lock()?;
        let mut tx = c.transaction().map_err(be)?;
        require_warehouse(&mut tx, wid)?;
        store_placements(&mut tx, wid, &placements)?;
        tx.commit().map_err(be)
    }
}
