//! Абстракция хранилища. Сервер зависит только от этих трейтов; реальная БД
//! (SQLite/Postgres) появится отдельным крейтом `wh-storage-<backend>`.
//!
//! Методы синхронные — пока работаем локально. Когда появится сетевая БД, трейты
//! перейдут на `async fn` (стабильно с Rust 1.75) без изменения доменной модели.

pub mod json;
mod memory;

pub use memory::InMemoryStorage;

use std::fmt;

use wh_core::model::{Placement, Product, ProductId, Warehouse, WarehouseId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    NotFound(String),
    Backend(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::NotFound(what) => write!(f, "not found: {what}"),
            StorageError::Backend(msg) => write!(f, "storage backend error: {msg}"),
        }
    }
}

impl std::error::Error for StorageError {}

pub type Result<T> = std::result::Result<T, StorageError>;

pub trait WarehouseRepository: Send + Sync {
    fn list_warehouses(&self) -> Result<Vec<(WarehouseId, String)>>;
    fn get_warehouse(&self, id: WarehouseId) -> Result<Warehouse>;
    fn save_warehouse(&self, warehouse: Warehouse) -> Result<()>;
    fn delete_warehouse(&self, id: WarehouseId) -> Result<()>;
}

pub trait CatalogRepository: Send + Sync {
    fn list_products(&self, warehouse: WarehouseId) -> Result<Vec<Product>>;
    fn upsert_product(&self, warehouse: WarehouseId, product: Product) -> Result<()>;
    fn delete_product(&self, warehouse: WarehouseId, product: ProductId) -> Result<()>;
}

pub trait PlacementRepository: Send + Sync {
    fn get_placements(&self, warehouse: WarehouseId) -> Result<Vec<Placement>>;
    fn replace_placements(&self, warehouse: WarehouseId, placements: Vec<Placement>) -> Result<()>;
}

pub trait Storage: WarehouseRepository + CatalogRepository + PlacementRepository {}
impl<T: WarehouseRepository + CatalogRepository + PlacementRepository> Storage for T {}
