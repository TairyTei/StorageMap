use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Трейт `Model` нужно импортировать, чтобы были доступны его методы (row_data, set_row_data).
// В C# методы интерфейса доступны всегда, а в Rust трейт должен быть "в области видимости".
use slint::{Model, ModelRc, Timer, TimerMode, VecModel};

// Подключаем код, который build.rs сгенерировал из app.slint.
// После этого нам доступен тип `MainWindow`.
slint::include_modules!();

const COLS: usize = 24;
const ROWS: usize = 24;

// ───────────────────────── Логика игры (чистый Rust, без UI) ─────────────────────────

struct Life {
    cols: usize,
    rows: usize,
    cells: Vec<bool>, // плоский массив: индекс = y * cols + x
    generation: u32,
    rng_state: u64, // состояние простейшего генератора случайных чисел
}

// `impl` - блок методов типа. Первый параметр `&self` / `&mut self` - это `this`:
//   &self     - только читаем (ссылка "только для чтения")
//   &mut self - можем менять (эксклюзивная ссылка, одновременно только одна)
impl Life {
    // Ассоциированная функция без `self` - аналог статического метода/конструктора.
    fn new(cols: usize, rows: usize) -> Self {
        // duration_since возвращает Result (время могло пойти назад).
        // Вместо исключений ошибки в Rust - это значения; здесь просто берём запасной вариант.
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
            | 1; // xorshift ломается на нуле, поэтому гарантируем ненулевое зерно

        // Последнее выражение без `;` - это возвращаемое значение (return не нужен).
        Self {
            cols,
            rows,
            cells: vec![false; cols * rows],
            generation: 0,
            rng_state: seed,
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.cols + x
    }

    // Маленький xorshift64, чтобы не тянуть крейт `rand`. Нужен `&mut self`, т.к. меняем состояние.
    fn next_random(&mut self) -> u64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        x
    }

    fn alive_neighbors(&self, x: usize, y: usize) -> u8 {
        let mut count = 0;
        for dy in [-1i32, 0, 1] {
            for dx in [-1i32, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue; // саму клетку не считаем
                }
                // Поле замкнуто в тор: уходим за правый край - появляемся слева.
                // rem_euclid, в отличие от `%`, не возвращает отрицательных чисел.
                // usize беззнаковый, поэтому на время арифметики приводим к i32 через `as`.
                let nx = (x as i32 + dx).rem_euclid(self.cols as i32) as usize;
                let ny = (y as i32 + dy).rem_euclid(self.rows as i32) as usize;
                if self.cells[self.idx(nx, ny)] {
                    count += 1;
                }
            }
        }
        count
    }

    fn step(&mut self) {
        // Новое поколение считаем в отдельном векторе, т.к. все клетки обновляются одновременно.
        let mut next = vec![false; self.cells.len()];

        for y in 0..self.rows {
            for x in 0..self.cols {
                let i = self.idx(x, y);
                let n = self.alive_neighbors(x, y);

                // `matches!` + pattern matching: аналог switch expression из C# с паттернами.
                // Живая клетка выживает при 2 или 3 соседях, мёртвая рождается при ровно 3.
                next[i] = matches!((self.cells[i], n), (true, 2 | 3) | (false, 3));
            }
        }

        self.cells = next; // старый вектор тут же освобождается (деструктор, GC не нужен)
        self.generation += 1;
    }

    fn toggle(&mut self, i: usize) {
        self.cells[i] = !self.cells[i]; // выход за границы = panic, а не тихая порча памяти
    }

    fn randomize(&mut self) {
        for i in 0..self.cells.len() {
            // Сначала считаем значение, потом пишем: одновременно брать &mut self
            // для генератора и для индексации cells нельзя (привет, borrow checker).
            let alive = self.next_random() % 4 == 0; // примерно 25% живых
            self.cells[i] = alive;
        }
        self.generation = 0;
    }
    fn toggle_all(&mut self) {
        for i in 0..self.cells.len() {
            self.cells[i] = !self.cells[i];
        }
    }

    fn clear(&mut self) {
        self.cells.fill(false);
        self.generation = 0;
    }

    // Классический "глайдер" - фигурка, которая ползёт по диагонали.
    fn spawn_glider(&mut self) {
        for (x, y) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
            let i = self.idx(x, y);
            self.cells[i] = true;
        }
    }
}

// ───────────────────────── Синхронизация логики с UI ─────────────────────────

// Переносим состояние игры в модель, которую видит UI.
// Меняем только отличающиеся клетки, чтобы UI не перерисовывал лишнего.
fn sync_ui(ui: &MainWindow, model: &VecModel<bool>, life: &Life) {
    for (i, &alive) in life.cells.iter().enumerate() {
        if model.row_data(i) != Some(alive) {
            // row_data возвращает Option<bool>: Some(значение) или None, если индекса нет.
            // null в Rust нет - вместо него Option, и компилятор заставляет его обработать.
            model.set_row_data(i, alive);
        }
    }
    ui.set_generation(life.generation as i32);
}

// ───────────────────────── Точка входа ─────────────────────────

pub fn run() -> Result<(), slint::PlatformError> {
    // `?` - если вернулась ошибка, сразу выходим из функции с этой ошибкой (как проброс исключения).
    let ui = MainWindow::new()?;

    // Состояние игры нужно нескольким колбэкам сразу, а у значения в Rust ровно один владелец.
    // Поэтому:
    //   Rc       - счётчик ссылок (shared_ptr для однопоточного кода), владельцев может быть много
    //   RefCell  - проверка правил заимствования не на этапе компиляции, а в рантайме:
    //              borrow() / borrow_mut(), а при нарушении правил будет panic
    // Для многопоточности были бы Arc<Mutex<…>>, но Slint работает в одном UI-потоке.
    let life = Rc::new(RefCell::new(Life::new(COLS, ROWS)));
    life.borrow_mut().spawn_glider();

    // Модель, которую UI «наблюдает» (как ObservableCollection в C#).
    let model = Rc::new(VecModel::from(life.borrow().cells.clone()));
    ui.set_cells(ModelRc::from(model.clone())); // clone() у Rc копирует лишь указатель
    ui.set_cols(COLS as i32);
    ui.set_rows(ROWS as i32);

    // Общая функция «обнови экран». Хранится как Rc<dyn Fn()>, т.е. как Func/Action в C#.
    // Окно держим через Weak (слабая ссылку): колбэки живут внутри окна, и сильная ссылка
    // создала бы цикл «окно -> колбэк -> окно», который счётчик ссылок никогда не освободит.
    let refresh: Rc<dyn Fn()> = {
        let ui_weak = ui.as_weak();
        let model = model.clone();
        let life = life.clone();
        Rc::new(move || {
            // upgrade() даёт Some(окно), если оно ещё живо, иначе None
            if let Some(ui) = ui_weak.upgrade() {
                sync_ui(&ui, &model, &life.borrow());
            }
        })
    };

    // `move` переносит захваченные переменные внутрь замыкания (оно становится их владельцем).
    // Поэтому перед каждым замыканием делаем свои клоны Rc - так с ними проще и привычнее.
    ui.on_toggle_cell({
        let life = life.clone();
        let refresh = refresh.clone();
        move |i| {
            life.borrow_mut().toggle(i as usize);
            refresh();
        }
    });
    ui.on_toggle_all({
        let life = life.clone();
        let refresh = refresh.clone();
        move || {
            life.borrow_mut().toggle_all();
            refresh();
        }
    });

    ui.on_step({
        let life = life.clone();
        let refresh = refresh.clone();
        move || {
            life.borrow_mut().step();
            refresh();
        }
    });

    ui.on_randomize({
        let life = life.clone();
        let refresh = refresh.clone();
        move || {
            life.borrow_mut().randomize();
            refresh();
        }
    });

    ui.on_clear({
        let life = life.clone();
        let refresh = refresh.clone();
        move || {
            life.borrow_mut().clear();
            refresh();
        }
    });

    // Таймер тикает всегда, но шаг делает, только если в UI включён "Старт".
    // ВАЖНО: Timer остановится, как только переменная `timer` будет уничтожена (RAII, как Dispose()),
    // поэтому она должна жить до конца main.
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(120), {
        let ui_weak = ui.as_weak();
        let life = life.clone();
        let refresh = refresh.clone();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                if ui.get_running() {
                    life.borrow_mut().step();
                    refresh();
                }
            }
        }
    });

    // Запускаем цикл сообщений. Блокирует поток до закрытия окна.
    ui.run()
}
