//! Code Editor demo 入口桩：`main` 只启动 `app::run()`，业务逻辑全部在 `app` 模块。
//!
//! 退出码是这里唯一的决定：`app::run()` 记录失败原因并正常返回，由本桩把它转成进程
//! 退出码，因此 demo 内部没有 `process::exit`，App 与窗口的清理路径也不会被绕过。

mod app;
mod commands;

fn main() -> std::process::ExitCode {
    match app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("错误：{message}");
            std::process::ExitCode::FAILURE
        }
    }
}
