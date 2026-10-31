use std::f64::consts::FRAC_PI_2;

use wh_core::sample::demo_snapshot;
use wh_core::{Point, ProductId, ShelfId};

use super::EditorState;
use crate::tools::tool::Tool;

fn room() -> EditorState {
    let mut s = EditorState::new_empty();
    s.set_tool(Tool::Outline);
    for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)] {
        s.click(Point::new(x, y), 0.3);
    }
    s.click(Point::new(0.05, 0.05), 0.3);
    s
}

#[test]
fn outline_closes_by_clicking_first_point() {
    let s = room();
    assert!(s.has_outline());
    assert_eq!(s.warehouse.outline.points.len(), 4);
    assert!((s.warehouse.outline.area() - 80.0).abs() < 1e-9);
    assert!(s.warehouse.outline.is_ccw());
    assert!(s.warehouse.outline.contains(s.warehouse.entrance));
    assert_eq!(s.tool, Tool::Select);
    assert!(!s.is_drawing());
}

#[test]
fn ortho_snaps_vertices_to_axes() {
    let mut s = EditorState::new_empty();
    s.set_tool(Tool::Outline);
    s.click(Point::new(0.0, 0.0), 0.3);
    s.click(Point::new(10.2, 0.7), 0.3);
    assert_eq!(s.draft()[1].y, 0.0);
    s.ortho = false;
    s.click(Point::new(10.2, 5.0), 0.3);
    assert!((s.draft()[2].x - 10.2).abs() < 1e-9);
}

#[test]
fn degenerate_outline_is_rejected() {
    let mut s = EditorState::new_empty();
    s.set_tool(Tool::Outline);
    s.click(Point::new(0.0, 0.0), 0.3);
    s.click(Point::new(5.0, 0.0), 0.3);
    s.close_outline();
    assert!(s.is_drawing(), "две вершины — контур не замкнут");
}

#[test]
fn undo_point_removes_last_vertex_and_stops_drawing() {
    let mut s = EditorState::new_empty();
    s.set_tool(Tool::Outline);
    s.click(Point::new(0.0, 0.0), 0.3);
    s.click(Point::new(5.0, 0.0), 0.3);
    s.undo_point();
    assert_eq!(s.draft().len(), 1);
    s.undo_point();
    assert!(!s.is_drawing());
}

#[test]
fn new_outline_resets_layout() {
    let mut s = room();
    s.set_tool(Tool::Shelf);
    s.click(Point::new(5.0, 4.0), 0.3);
    assert_eq!(s.warehouse.shelves.len(), 1);

    s.set_tool(Tool::Outline);
    s.click(Point::new(0.0, 0.0), 0.3);
    assert!(s.is_drawing());
    assert!(s.warehouse.shelves.is_empty());
    assert!(!s.has_outline());
}

#[test]
fn shelves_are_validated_on_placement() {
    let mut s = room();
    s.set_tool(Tool::Shelf);
    s.click(Point::new(5.0, 4.0), 0.3);
    assert_eq!(s.warehouse.shelves.len(), 1);
    s.click(Point::new(5.5, 4.0), 0.3);
    assert_eq!(s.warehouse.shelves.len(), 1);
    s.click(Point::new(0.5, 0.5), 0.3);
    assert_eq!(s.warehouse.shelves.len(), 1);
}

#[test]
fn rotate_and_delete_selected() {
    let mut s = room();
    s.set_tool(Tool::Shelf);
    s.click(Point::new(5.0, 4.0), 0.3);
    s.set_tool(Tool::Select);
    s.rotate();
    let angle = s.warehouse.shelves[0].footprint.angle;
    assert!((angle - FRAC_PI_2).abs() < 1e-9);
    s.delete_selected();
    assert!(s.warehouse.shelves.is_empty());
}

#[test]
fn rotate_in_shelf_tool_changes_pending_angle_only() {
    let mut s = room();
    s.set_tool(Tool::Shelf);
    s.rotate();
    assert!((s.pending_angle() - FRAC_PI_2).abs() < 1e-9);
    s.click(Point::new(5.0, 4.0), 0.3);
    let angle = s.warehouse.shelves[0].footprint.angle;
    assert!((angle - FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn drag_is_taken_by_select_tool_only() {
    let mut s = room();
    s.set_tool(Tool::Shelf);
    s.click(Point::new(5.0, 4.0), 0.3);
    assert!(!s.drag(0.1, 0.0), "в режиме стеллажа жест — панорама");
    s.set_tool(Tool::Select);
    assert!(s.drag(0.1, 0.0));
}

#[test]
fn select_picks_shelf_under_cursor() {
    let mut s = EditorState::from_snapshot(demo_snapshot());
    s.set_tool(Tool::Select);
    s.click(Point::new(12.0, 3.0), 0.3);
    assert_eq!(s.selected_shelf, Some(ShelfId(1)));
    s.click(Point::new(1.0, 1.0), 0.3);
    assert_eq!(s.selected_shelf, None);
}

#[test]
fn auto_place_puts_top_demand_on_best_shelf() {
    let mut s = EditorState::from_snapshot(demo_snapshot());
    assert!(s.analyze());
    s.auto_place();
    let best = s
        .access
        .iter()
        .filter(|a| a.distance.is_some())
        .max_by(|a, b| a.score.total_cmp(&b.score))
        .unwrap()
        .shelf;
    let top = s
        .products
        .iter()
        .max_by(|a, b| a.demand.partial_cmp(&b.demand).unwrap())
        .unwrap()
        .id;
    assert_eq!(s.placement_of(top).map(|p| p.shelf), Some(best));
}

#[test]
fn pin_respects_capacity() {
    let mut s = EditorState::from_snapshot(demo_snapshot());
    s.add_product("Гигант", "7", "").unwrap();
    s.selected_product = Some(s.products.last().unwrap().id);
    s.selected_shelf = Some(ShelfId(1));
    s.pin_selected();
    assert!(s.placements.is_empty());

    s.selected_product = Some(ProductId(1));
    s.pin_selected();
    assert_eq!(s.placements.len(), 1);
    assert!(s.placements[0].pinned);
}

#[test]
fn product_form_validation() {
    let mut s = EditorState::new_empty();
    assert!(s.add_product("", "1", "").is_err());
    assert!(s.add_product("X", "abc", "").is_err());
    assert!(s.add_product("X", "0", "").is_err());
    assert!(s.add_product("X", "2", "1,5").is_ok());
    assert_eq!(s.products[0].demand, Some(1.5));
}

#[test]
fn geometry_edit_invalidates_analysis() {
    let mut s = EditorState::from_snapshot(demo_snapshot());
    assert!(s.analyze());
    assert!(!s.access.is_empty());
    s.selected_shelf = Some(ShelfId(1));
    s.toggle_double_sided();
    assert!(s.access.is_empty());
}
