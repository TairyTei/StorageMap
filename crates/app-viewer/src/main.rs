use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("warehouse.json"));
    app_viewer::run_viewer(file)?;
    Ok(())
}
