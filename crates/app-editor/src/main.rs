use std::path::PathBuf;

use gui_core::{Mode, StartOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("warehouse.json"));
    gui_core::run(StartOptions {
        mode: Mode::Editor,
        file,
    })?;
    Ok(())
}
