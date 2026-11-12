# warehouse-map
(Курсовой проект)
Учёт товаров на визуальной карте склада / торгового зала.
Единицы везде — метры, ось Y вверх, углы в радианах против часовой.

## Запуск

```
cargo test --workspace          # тесты всех крейтов
cargo run -p wh-demo            # консольный сценарий
cargo run -p app-editor         # редактор (читает/пишет ./warehouse.json)
cargo run -p app-viewer         # просмотрщик на desktop
```
