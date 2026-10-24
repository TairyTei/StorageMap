fn main() {
    // Стиль задан явно: «fluent» не требует Qt и одинаково выглядит на desktop и Android.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/main.slint", config).expect("slint compile failed");
}
