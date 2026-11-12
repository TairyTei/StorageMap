use wh_core::geometry::OrientedRect;
use wh_core::model::{ShelfAccess, ShelfId, Warehouse};

use crate::navigator::{Navigator, Route};

const ACCESS_GAP: f64 = 0.01;

#[derive(Debug, Clone)]
pub struct NavConfig {
    pub clearance: f64,
}

impl Default for NavConfig {
    fn default() -> Self {
        Self { clearance: 0.4 }
    }
}

pub fn analyze(warehouse: &Warehouse, cfg: &NavConfig) -> Vec<ShelfAccess> {
    let obstacles: Vec<OrientedRect> = warehouse
        .shelves
        .iter()
        .map(|s| s.footprint.inflated(cfg.clearance))
        .collect();
    let nav = Navigator::new(warehouse.outline.clone(), obstacles, cfg.clearance);
    let field = nav.routes_from(warehouse.entrance);

    let offset = cfg.clearance + ACCESS_GAP;
    let raw: Vec<(ShelfId, Option<Route>)> = warehouse
        .shelves
        .iter()
        .map(|s| {
            let best = s
                .access_points(offset)
                .into_iter()
                .filter_map(|p| field.route_to(p))
                .min_by(|a, b| a.length.total_cmp(&b.length));
            (s.id, best)
        })
        .collect();

    let lengths = raw.iter().filter_map(|(_, r)| r.as_ref().map(|r| r.length));
    let d_min = lengths.clone().fold(f64::INFINITY, f64::min);
    let d_max = lengths.fold(f64::NEG_INFINITY, f64::max);
    let span = d_max - d_min;

    raw.into_iter()
        .map(|(shelf, route)| match route {
            Some(r) => ShelfAccess {
                shelf,
                distance: Some(r.length),
                score: if span < 1e-9 {
                    1.0
                } else {
                    1.0 - (r.length - d_min) / span
                },
                route: r.points,
            },
            None => ShelfAccess {
                shelf,
                distance: None,
                score: 0.0,
                route: Vec::new(),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wh_core::geometry::{OrientedRect, Point, Polygon};
    use wh_core::model::{Shelf, WarehouseId};

    fn shelf(id: u64, x: f64, y: f64, angle: f64) -> Shelf {
        Shelf::new(
            ShelfId(id),
            format!("S{id}"),
            OrientedRect::new(Point::new(x, y), 3.0, 1.0, angle),
            4,
        )
    }

    fn warehouse(shelves: Vec<Shelf>) -> Warehouse {
        Warehouse {
            id: WarehouseId(1),
            name: "t".into(),
            outline: Polygon::new(vec![
                Point::new(0.0, 0.0),
                Point::new(20.0, 0.0),
                Point::new(20.0, 12.0),
                Point::new(0.0, 12.0),
            ]),
            entrance: Point::new(1.0, 1.0),
            shelves,
        }
    }

    #[test]
    fn nearer_shelf_scores_higher() {
        let w = warehouse(vec![shelf(1, 4.0, 5.0, 0.0), shelf(2, 16.0, 9.0, 0.0)]);
        let a = analyze(&w, &NavConfig::default());
        let (s1, s2) = (&a[0], &a[1]);
        assert!(s1.distance.unwrap() < s2.distance.unwrap());
        assert!(s1.score > s2.score);
        assert!((s1.score - 1.0).abs() < 1e-9 && s2.score.abs() < 1e-9);
        assert_eq!(s1.route.first(), Some(&w.entrance));
    }

    #[test]
    fn shelf_facing_wall_is_unreachable() {
        // Лицом (+Y, angle=0) в стену: точка доступа за пределами помещения.
        let w = warehouse(vec![shelf(1, 10.0, 11.5, 0.0)]);
        let a = analyze(&w, &NavConfig::default());
        assert_eq!(a[0].distance, None);
        assert_eq!(a[0].score, 0.0);
    }
}
