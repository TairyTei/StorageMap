//! Общая библиотека: геометрия и доменная модель склада.
//! Без I/O и без платформенных зависимостей — собирается и под desktop, и под Android.

pub mod geometry;
pub mod model;
pub mod sample;

pub use geometry::{OrientedRect, Point, Polygon};
pub use model::*;
