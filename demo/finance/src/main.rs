//! Finance demo 入口桩：`main` 只启动 `app::run()`，业务逻辑全部在库的 `app` 模块。

use finance_demo::app;

fn main() {
    app::run();
}
