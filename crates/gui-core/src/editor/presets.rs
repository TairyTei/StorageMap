pub struct ShelfPreset {
    pub name: &'static str,
    pub width: f64,
    pub depth: f64,
    pub capacity: u32,
}

pub const PRESETS: [ShelfPreset; 4] = [
    ShelfPreset {
        name: "3×1 м (6 яч.)",
        width: 3.0,
        depth: 1.0,
        capacity: 6,
    },
    ShelfPreset {
        name: "2×1 м (4 яч.)",
        width: 2.0,
        depth: 1.0,
        capacity: 4,
    },
    ShelfPreset {
        name: "1.2×0.5 м (2 яч.)",
        width: 1.2,
        depth: 0.5,
        capacity: 2,
    },
    ShelfPreset {
        name: "4×1.5 м (8 яч.)",
        width: 4.0,
        depth: 1.5,
        capacity: 8,
    },
];
