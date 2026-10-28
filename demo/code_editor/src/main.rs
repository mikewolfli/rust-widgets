//! Code Editor demo 入口桩：`main` 只启动 `app::run()`，业务逻辑全部在库的 `app` 模块。
//!
//! 退出码由 `app::run()` 决定并返回，本桩不做任何分支判断。

use code_editor_demo::app;

fn main() -> std::process::ExitCode {
    app::run()
}
