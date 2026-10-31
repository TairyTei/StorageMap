use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::{Color, ModelRc, SharedString, Timer, TimerMode, VecModel};
use wh_core::{sample::demo_snapshot, ProductId, ShelfId, Snapshot};
use wh_storage::json;

use crate::editor::{EditorState, PRESETS};
use crate::scene::{self, point_in_poly, Scene};
use crate::tools::tool::{Tool, ToolBehavior};
use crate::viewport::Viewport;

slint::include_modules!();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Editor,
    Viewer,
}

pub struct StartOptions {
    pub mode: Mode,
    pub file: PathBuf,
}

struct App {
    state: EditorState,
    vp: Viewport,
    mode: Mode,
    view3d: bool,
    auto_fit: bool,
    search: String,
    visible_products: Vec<ProductId>,
    hit: Vec<(ShelfId, Vec<(f64, f64)>)>,
    file: PathBuf,
}

impl App {
    fn new(opts: &StartOptions) -> Self {
        let (snapshot, status) = match json::load_file(&opts.file) {
            Ok(s) => (s, format!("Загружено: {}", opts.file.display())),
            Err(_) => (
                demo_snapshot(),
                "Демо-склад (файл проекта не найден)".to_string(),
            ),
        };
        let mut state = EditorState::from_snapshot(snapshot);
        state.analyze();
        if state.status.starts_with("Доступность") || state.status.starts_with("Рассчитано")
        {
            state.status = status;
        }
        Self {
            state,
            vp: Viewport::default(),
            mode: opts.mode,
            view3d: false,
            auto_fit: true,
            search: String::new(),
            visible_products: vec![],
            hit: vec![],
            file: opts.file.clone(),
        }
    }

    fn set_size(&mut self, w: f64, h: f64) -> bool {
        if w < 50.0 || h < 50.0 || ((w, h) == self.vp.size) {
            return false;
        }
        self.vp.size = (w, h);
        if self.auto_fit {
            self.fit();
        }
        true
    }

    fn fit(&mut self) {
        let (min, max) = self.state.bounds();
        self.vp.fit(min, max);
    }

    fn click(&mut self, sx: f64, sy: f64) {
        if self.view3d {
            self.state.selected_shelf = self
                .hit
                .iter()
                .rev()
                .find(|(_, poly)| point_in_poly(sx, sy, poly))
                .map(|(id, _)| *id);
            return;
        }
        if self.mode == Mode::Viewer {
            self.state.tool = Tool::Select;
        }
        let world = self.vp.to_world(sx, sy);
        let tol = 12.0 / self.vp.scale;
        self.state.click(world, tol);
    }
    fn pan(&mut self, dx: f32, dy: f32) {
        let (dx64, dy64) = (dx as f64, dy as f64);

        let (wdx64, wdy64) = self.vp.delta_to_world(dx64, dy64);

        if (!self.view3d && !self.state.drag(wdx64, wdy64)) {
            self.auto_fit = false;
            self.vp.pan_pixels(dx64, dy64);
        }
    }
    fn select_product(&mut self, row: usize) {
        let Some(&id) = self.visible_products.get(row) else {
            return;
        };
        self.state.selected_product = Some(id);
        if let Some(pl) = self.state.placement_of(id) {
            self.state.selected_shelf = Some(pl.shelf);
        }
    }

    fn save(&mut self) {
        self.state.status = match json::save_file(&self.file, &self.state.snapshot()) {
            Ok(()) => format!("Сохранено: {}", self.file.display()),
            Err(e) => format!("Ошибка сохранения: {e}"),
        };
    }

    fn load(&mut self) {
        match json::load_file(&self.file) {
            Ok(snapshot) => {
                self.replace_state(snapshot);
                self.state.status = format!("Загружено: {}", self.file.display());
            }
            Err(e) => self.state.status = format!("Не удалось открыть: {e}"),
        }
    }

    fn replace_state(&mut self, snapshot: Snapshot) {
        let ortho = self.state.ortho;
        self.state = EditorState::from_snapshot(snapshot);
        self.state.ortho = ortho;
        self.state.analyze();
    }

    fn push(&mut self, ui: &MainWindow) {
        if self.state.fit_requested {
            self.state.fit_requested = false;
            self.auto_fit = true;
            self.fit();
        }

        let scene: Scene = if self.view3d {
            scene::build_3d(&self.state, self.vp.size)
        } else {
            scene::build_2d(&self.state, &self.vp)
        };

        ui.set_grid_path(path(&scene.grid));
        ui.set_floor_path(path(&scene.floor));
        ui.set_draft_path(path(&scene.draft));
        ui.set_marker_path(path(&scene.markers));
        ui.set_route_path(path(&scene.route));
        ui.set_entrance_path(path(&scene.entrance));

        let shelves: Vec<ShelfView> = scene
            .shelves
            .iter()
            .map(|s| ShelfView {
                commands: s.commands.as_str().into(),
                color: Color::from_rgb_u8(s.color.0, s.color.1, s.color.2),
                label: s.label.as_str().into(),
                lx: s.lx,
                ly: s.ly,
                selected: s.selected,
            })
            .collect();
        ui.set_shelves(ModelRc::new(VecModel::from(shelves)));

        let labels: Vec<LabelView> = scene
            .labels
            .iter()
            .map(|l| LabelView {
                x: l.x,
                y: l.y,
                text: l.text.as_str().into(),
            })
            .collect();
        ui.set_labels(ModelRc::new(VecModel::from(labels)));

        match scene.preview_from {
            Some((x, y)) if !self.view3d => {
                ui.set_preview_from_x(x);
                ui.set_preview_from_y(y);
                ui.set_preview_active(true);
            }
            _ => ui.set_preview_active(false),
        }
        ui.set_px_per_m(scene.px_per_m);

        let rows = self.state.product_rows(&self.search);
        self.visible_products = rows.iter().map(|(id, _, _)| *id).collect();
        let products: Vec<ProductView> = rows
            .into_iter()
            .map(|(_, text, selected)| ProductView {
                text: text.into(),
                selected,
            })
            .collect();
        ui.set_products(ModelRc::new(VecModel::from(products)));

        ui.set_tool(self.state.tool.index());
        ui.set_view_mode(if self.view3d { 1 } else { 0 });
        ui.set_status(self.state.status.as_str().into());
        ui.set_shelf_info(self.state.shelf_info().into());

        self.hit = scene.hit;
    }
}

fn path(commands: &str) -> SharedString {
    if commands.trim().is_empty() {
        "M 0 0".into()
    } else {
        commands.into()
    }
}

fn sync_from_ui(ui: &MainWindow, app: &mut App) {
    app.set_size(ui.get_canvas_width() as f64, ui.get_canvas_height() as f64);
    app.state.ortho = ui.get_ortho();
}

macro_rules! wire {
    ($ui:expr, $app:expr, $on:ident, |$a:ident, $w:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
        let app = Rc::clone(&$app);
        let weak = $ui.as_weak();
        $ui.$on(move |$($arg: $ty),*| {
            if let Some(ui) = weak.upgrade() {
                {
                    let mut guard = app.borrow_mut();
                    let $a: &mut App = &mut guard;
                    sync_from_ui(&ui, $a);
                    let $w: &MainWindow = &ui;
                    $body;
                }
                app.borrow_mut().push(&ui);
            }
        });
    }};
}

pub fn run(opts: StartOptions) -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let app = Rc::new(RefCell::new(App::new(&opts)));

    ui.set_editor_mode(opts.mode == Mode::Editor);
    ui.set_preset_names(ModelRc::new(VecModel::from(
        PRESETS
            .iter()
            .map(|p| SharedString::from(p.name))
            .collect::<Vec<_>>(),
    )));

    wire!(ui, app, on_canvas_clicked, |a, _w, x: f32, y: f32| a
        .click(x as f64, y as f64));
    wire!(ui, app, on_zoom_in, |a, _w| {
        a.auto_fit = false;
        a.vp.zoom(1.25);
    });
    wire!(ui, app, on_zoom_out, |a, _w| {
        a.auto_fit = false;
        a.vp.zoom(0.8);
    });
    wire!(ui, app, on_zoom_at, |a, _w, fac: f32, x: f32, y: f32| {
        a.auto_fit = false;
        a.vp.zoom_at(fac.clamp(0.5, 2.0) as f64, x as f64, y as f64);
    });
    wire!(ui, app, on_fit_view, |a, _w| {
        a.auto_fit = true;
        a.fit();
    });
    wire!(ui, app, on_pan_pixels, |a, _w, dx: f32, dy: f32| {
        a.pan(dx, dy);
    });
    wire!(ui, app, on_view_mode_selected, |a, _w, mode: i32| a
        .view3d =
        mode == 1);

    wire!(ui, app, on_tool_selected, |a, _w, i: i32| {
        if a.mode == Mode::Editor {
            a.state.set_tool(Tool::from_index(i));
        }
    });
    wire!(ui, app, on_preset_selected, |a, _w, i: i32| a
        .state
        .set_preset(i));

    wire!(ui, app, on_close_outline, |a, _w| a.state.close_outline());
    wire!(ui, app, on_undo_point, |a, _w| a.state.undo_point());
    wire!(ui, app, on_rotate_shelf, |a, _w| a.state.rotate());
    wire!(ui, app, on_delete_shelf, |a, _w| a.state.delete_selected());
    wire!(ui, app, on_toggle_double_sided, |a, _w| a
        .state
        .toggle_double_sided());

    wire!(ui, app, on_analyze_clicked, |a, _w| {
        a.state.analyze();
    });
    wire!(ui, app, on_place_clicked, |a, _w| a.state.auto_place());

    wire!(ui, app, on_product_clicked, |a, _w, row: i32| a
        .select_product(row.max(0) as usize));
    wire!(ui, app, on_search_edited, |a, _w, text: SharedString| {
        a.search = text.to_string();
    });
    wire!(ui, app, on_add_product, |a, w| {
        let (name, size, demand) = (
            w.get_new_name().to_string(),
            w.get_new_size().to_string(),
            w.get_new_demand().to_string(),
        );
        match a.state.add_product(&name, &size, &demand) {
            Ok(()) => {
                w.set_new_name("".into());
                w.set_new_size("".into());
                w.set_new_demand("".into());
            }
            Err(e) => a.state.status = e,
        }
    });
    wire!(ui, app, on_pin_product, |a, _w| a.state.pin_selected());
    wire!(ui, app, on_unpin_product, |a, _w| a.state.unpin_selected());

    wire!(ui, app, on_new_project, |a, _w| {
        a.state = EditorState::new_empty();
        a.state.status = "Новый проект: выберите «Контур склада» и нарисуйте помещение".into();
    });
    wire!(ui, app, on_save_project, |a, _w| a.save());
    wire!(ui, app, on_load_project, |a, _w| a.load());

    let timer = Timer::default();
    {
        let app = Rc::clone(&app);
        let weak = ui.as_weak();
        timer.start(TimerMode::Repeated, Duration::from_millis(120), move || {
            if let Some(ui) = weak.upgrade() {
                let changed = app
                    .borrow_mut()
                    .set_size(ui.get_canvas_width() as f64, ui.get_canvas_height() as f64);
                if changed {
                    app.borrow_mut().push(&ui);
                }
            }
        });
    }

    app.borrow_mut().push(&ui);
    ui.run()?;
    drop(timer);
    Ok(())
}
