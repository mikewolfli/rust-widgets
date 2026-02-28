# rust_widgets Справка (Русский)

## Связанные документы

- Архитектура: [ARCHITECTURE.md](ARCHITECTURE.md)
- Каталог демо: [../demos/README.md](../demos/README.md)
- Справка на английском: [HELP.en.md](HELP.en.md)
- Справка на китайском (упрощённый): [HELP.zh-CN.md](HELP.zh-CN.md)
- Справка на китайском (традиционный): [HELP.zh-TW.md](HELP.zh-TW.md)
- Справка на французском: [HELP.fr.md](HELP.fr.md)
- Быстрый старт C ABI: [C_ABI_QUICKSTART.md](C_ABI_QUICKSTART.md)

## Кратко

- Нативная кроссплатформенная GUI-архитектура на чистом Rust.
- Настольные платформы: Windows, macOS, Linux, Harmony Desktop.
- Облегчённый профиль для встраиваемых систем.
- Зарезервированный единый API для мобильных платформ (Android / iOS / Harmony mobile).
- Модули: очередь событий, сигналы-слоты, темы/стили, layout, XML, i18n, печать, PDF, графики.

## Профили

- Полный: `default` + `full`.
- Облегчённый: `embedded`.
- Резерв для мобильных платформ: `mobile-api` для единых точек расширения.

## Команды

```bash
cargo check
cargo check --examples
cargo run --example demo_main
```

## Примеры переключения feature-флагов

```bash
# Полный профиль (по умолчанию)
cargo check

# Облегчённый встроенный профиль
cargo check --no-default-features --features embedded

# Полный профиль + резерв мобильного API
cargo check --features "full,mobile-api"

# Встроенный профиль + резерв мобильного API
cargo check --no-default-features --features "embedded,mobile-api"
```

## Демонстрации

- Полный каталог по категориям: `demos/README.md`.
- Архитектурные демо: `demo_main`, `demo_layout`, `demo_xml`, `demo_i18n`.
- Демо нативного polling: `demo_native_events` (триггеры меню + типизированные триггеры виджетов).
- Демо контролов покрывают окно/диалог/popup, базовый ввод, просмотр данных,
  контейнеры, меню/панель инструментов/статус, а также контролы table/grid/chart/canvas.

## Мульти-языковые биндинги

C ABI находится в `src/bindings/mod.rs`, зарезервированы точки расширения для Python/C++/Java.
Также доступны API опроса нативных триггеров: `rust_widgets_poll_menu_triggered` и `rust_widgets_poll_widget_triggered`.
Для типизированного события виджета используйте `rust_widgets_poll_widget_trigger_event(widget_id_out)`, коды: `0` нет, `1` клик, `2` изменение значения.
Полные команды сборки/запуска C ABI смотрите в `docs/C_ABI_QUICKSTART.md`.

Быстрая сборка/запуск (из корня проекта):

```bash
# Сборка динамической библиотеки
cargo build

# Компиляция C-примера на macOS
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/debug -lrust_widgets -o target/debug/c_abi_poll_demo

# Запуск на macOS
DYLD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

Пример для Linux (runtime loader):

```bash
LD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```
