//! Доменная модель. Только данные и проверки инвариантов — никакой логики
//! маршрутизации или расстановки (она живёт в wh-nav / wh-layout).

use crate::geometry::{OrientedRect, Point, Polygon};

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name(pub u64);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_type!(/// Идентификатор склада / торгового зала.
    WarehouseId);
id_type!(/// Идентификатор стеллажа.
    ShelfId);
id_type!(/// Идентификатор товара.
    ProductId);

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Warehouse {
    pub id: WarehouseId,
    pub name: String,
    pub outline: Polygon,
    pub entrance: Point,
    pub shelves: Vec<Shelf>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shelf {
    pub id: ShelfId,
    pub label: String,
    pub footprint: OrientedRect,
    pub double_sided: bool,
    pub capacity: u32,
}

impl Shelf {
    pub fn new(
        id: ShelfId,
        label: impl Into<String>,
        footprint: OrientedRect,
        capacity: u32,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            footprint,
            double_sided: false,
            capacity,
        }
    }

    pub fn access_points(&self, offset: f64) -> Vec<Point> {
        let n = self.footprint.front_dir();
        let dist = self.footprint.half_d + offset;
        let mut pts = vec![self.footprint.center + n * dist];
        if self.double_sided {
            pts.push(self.footprint.center - n * dist);
        }
        pts
    }
}

impl Warehouse {
    pub fn shelf(&self, id: ShelfId) -> Option<&Shelf> {
        self.shelves.iter().find(|s| s.id == id)
    }

    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        if self.outline.points.len() < 3 || self.outline.area() < 1e-6 {
            issues.push(ValidationIssue::OutlineTooSmall);
        } else if !self.outline.is_simple() {
            issues.push(ValidationIssue::OutlineSelfIntersects);
        }

        if !self.outline.contains(self.entrance) {
            issues.push(ValidationIssue::EntranceOutside);
        }

        for s in &self.shelves {
            let c = s.footprint.corners();
            let inside = (0..4).all(|i| self.outline.contains_segment(c[i], c[(i + 1) % 4]));
            if !inside {
                issues.push(ValidationIssue::ShelfOutside(s.id));
            }
        }

        for (i, a) in self.shelves.iter().enumerate() {
            for b in &self.shelves[i + 1..] {
                if a.id == b.id {
                    issues.push(ValidationIssue::DuplicateShelfId(a.id));
                }
                if a.footprint.overlaps(&b.footprint) {
                    issues.push(ValidationIssue::ShelvesOverlap(a.id, b.id));
                }
            }
        }
        issues
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    OutlineTooSmall,
    OutlineSelfIntersects,
    EntranceOutside,
    ShelfOutside(ShelfId),
    ShelvesOverlap(ShelfId, ShelfId),
    DuplicateShelfId(ShelfId),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Product {
    pub id: ProductId,
    pub sku: String,
    pub name: String,
    pub size: u32,
    pub demand: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Placement {
    pub product: ProductId,
    pub shelf: ShelfId,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Snapshot {
    pub warehouse: Warehouse,
    pub products: Vec<Product>,
    pub placements: Vec<Placement>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShelfAccess {
    pub shelf: ShelfId,
    pub distance: Option<f64>,
    pub score: f64,
    pub route: Vec<Point>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room() -> Polygon {
        Polygon::new(vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ])
    }

    fn shelf(id: u64, x: f64, y: f64) -> Shelf {
        Shelf::new(
            ShelfId(id),
            format!("S{id}"),
            OrientedRect::new(Point::new(x, y), 3.0, 1.0, 0.0),
            4,
        )
    }

    fn warehouse(shelves: Vec<Shelf>) -> Warehouse {
        Warehouse {
            id: WarehouseId(1),
            name: "test".into(),
            outline: room(),
            entrance: Point::new(1.0, 1.0),
            shelves,
        }
    }

    #[test]
    fn valid_warehouse_has_no_issues() {
        assert!(warehouse(vec![shelf(1, 3.0, 5.0), shelf(2, 7.0, 5.0)])
            .validate()
            .is_empty());
    }

    #[test]
    fn detects_overlap_and_outside() {
        let w = warehouse(vec![
            shelf(1, 3.0, 5.0),
            shelf(2, 4.0, 5.0),
            shelf(3, 9.5, 5.0),
        ]);
        let issues = w.validate();
        assert!(issues.contains(&ValidationIssue::ShelvesOverlap(ShelfId(1), ShelfId(2))));
        assert!(issues.contains(&ValidationIssue::ShelfOutside(ShelfId(3))));
    }

    #[test]
    fn entrance_must_be_inside() {
        let mut w = warehouse(vec![]);
        w.entrance = Point::new(-1.0, 0.0);
        assert_eq!(w.validate(), vec![ValidationIssue::EntranceOutside]);
    }

    #[test]
    fn double_sided_has_two_access_points() {
        let mut s = shelf(1, 5.0, 5.0);
        assert_eq!(s.access_points(0.5).len(), 1);
        s.double_sided = true;
        let pts = s.access_points(0.5);
        assert_eq!(pts.len(), 2);
        assert!((pts[0].y - 6.0).abs() < 1e-9 && (pts[1].y - 4.0).abs() < 1e-9);
    }
}
