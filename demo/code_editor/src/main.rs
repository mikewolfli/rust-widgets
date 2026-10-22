//! Code Editor demo 入口桩：`main` 只启动 `app::run()`，业务逻辑全部在 `app` 模块。

mod app;
mod commands;

fn main() {
    app::run();
}
