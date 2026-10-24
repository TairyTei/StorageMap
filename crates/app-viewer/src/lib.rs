//! Просмотрщик склада. Тот же `gui-core`, режим `Viewer`: без инструментов редактирования.
//!
//! * desktop: `cargo run -p app-viewer [проект.json]` — удобно отлаживать без телефона;
//! * Android: точка входа `android_main` (см. README, раздел «Android»).

use std::path::PathBuf;

use gui_core::{Mode, StartOptions};

pub fn run_viewer(file: PathBuf) -> Result<(), slint::PlatformError> {
    gui_core::run(StartOptions {
        mode: Mode::Viewer,
        file,
    })
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: slint::android::AndroidApp) {
    let file = app
        .internal_data_path()
        .map(|dir| dir.join("warehouse.json"))
        .unwrap_or_else(|| PathBuf::from("warehouse.json"));
    slint::android::init(app).expect("android backend init failed");
    run_viewer(file).expect("viewer failed");
}
