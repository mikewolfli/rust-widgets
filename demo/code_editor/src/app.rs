//! Code Editor Demo — `CodeEditor` 挂载进窗口，并带完整菜单栏 + 工具栏。
//!
//! # 这个 demo 演示什么
//!
//! 1. **自绘型控件挂载**——`CodeEditor` 自己负责绘制内容，所以没有对应的
//!    `new_*` 工厂方法，用 `WindowHandle::mount_custom_widget` 交给窗口托管。
//!    窗口**用什么**承载它由 `src/platform/` 决定，本 demo 不需要知道，
//!    也不写任何 OS 分支。
//! 2. **菜单栏驱动控件**——菜单项激活后从菜单事件队列取出，转成 [`Command`]，
//!    通过 [`CustomWidgetHandle::update`] 打到编辑器上。菜单与编辑器之间只通过
//!    这条平台无关的通道通信。
//! 3. **工具栏 + 状态栏**——工具栏提供高频操作按钮，状态栏实时显示光标位置。
//!
//! # 运行
//!
//! ```text
//! cd demo/code_editor
//! cargo run
//! ```
//!
//! 当前平台若无法承载自绘型控件，demo **如实报错并停止**，不会留下空窗口。

use rust_widgets::app::{App, AppConfig, CustomWidgetHandle, WidgetHandle};
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::code_editor::{
    CodeEditor, CodeEditorConfig, DiagnosticMarker, LanguageId, MarkerSeverity,
};
use rust_widgets::widget::Widget;
use std::sync::{Arc, Mutex};

use crate::commands::{self, Command};

// ═══════════════════════════════════════════════════════════════════════════════
// 日志
// ═══════════════════════════════════════════════════════════════════════════════

struct EventLog {
    entries: Mutex<Vec<String>>,
}

impl EventLog {
    fn new() -> Self {
        Self { entries: Mutex::new(Vec::new()) }
    }

    fn append(&self, msg: impl Into<String>) {
        let msg = msg.into();
        println!("[EVENT] {}", msg);
        self.entries.lock().unwrap().push(msg);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 布局常量
// ═══════════════════════════════════════════════════════════════════════════════

const WINDOW_W: u32 = 980;
const WINDOW_H: u32 = 680;
/// 菜单栏高度。
const MENU_H: u32 = 26;
/// 工具栏高度：必须 >= MIN_TOUCH_TARGET(28)，兼顾触屏。
const TOOLBAR_H: i32 = 36;
/// 状态栏高度。
const STATUS_H: i32 = 26;

/// 编辑器占用的客户区（工具栏之下、状态栏之上）。
fn editor_rect() -> Rect {
    Rect::new(
        0,
        MENU_H as i32 + TOOLBAR_H,
        WINDOW_W,
        WINDOW_H - MENU_H - TOOLBAR_H as u32 - STATUS_H as u32,
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// 示例源码
// ═══════════════════════════════════════════════════════════════════════════════

const SAMPLE_RUST: &str = "\
// rust_widgets — CodeEditor 演示源码
// 试试菜单栏 Edit / View，或点工具栏按钮
fn fib(n: u64) -> u64 {
    if n < 2 { return n; }
    fib(n - 1) + fib(n - 2)
}

fn main() {
    let limit = 10;
    for i in 0..limit {
        println!(\"fib({i}) = {}\", fib(i));
    }
}
";

/// 构建编辑器：配置、示例文本、诊断标记全部就位。
fn build_editor() -> CodeEditor {
    let rect = editor_rect();
    let config = CodeEditorConfig::new()
        .language(LanguageId::Rust)
        .tab_width(4)
        .insert_spaces(true)
        .show_line_numbers(true)
        .show_minimap(true)
        .show_indent_guides(true)
        .highlight_brackets(true)
        .highlight_occurrences(true)
        .highlight_active_line(true)
        .auto_close_brackets(true)
        .column_ruler(100);

    let mut editor =
        CodeEditor::with_config(rect, config).expect("demo 配置必须合法（由 validate 保证）");
    editor.set_title("fibonacci.rs");
    editor.set_text(SAMPLE_RUST);
    editor.set_markers(vec![
        DiagnosticMarker::new(3, "deep recursion", MarkerSeverity::Warning),
        DiagnosticMarker::new(8, "`limit` could be inlined", MarkerSeverity::Info),
    ]);
    editor
}

// ═══════════════════════════════════════════════════════════════════════════════
// 菜单栏
// ═══════════════════════════════════════════════════════════════════════════════

/// 单个菜单项：命令 + 触发后要投递到的槽位。
struct MenuBinding {
    command: Command,
}

/// 菜单栏结构：`(菜单标题, 该菜单下的命令)`。
///
/// 与 `Edit` / `View` 两个顶层菜单对应，顺序即显示顺序。
const MENU_STRUCTURE: &[(&str, &[Command])] = &[
    (
        "Edit",
        &[
            Command::Undo,
            Command::Redo,
            Command::Copy,
            Command::Cut,
            Command::Paste,
            Command::SelectAll,
            Command::ToggleComment,
            Command::DuplicateLine,
            Command::DeleteLine,
            Command::JoinLines,
            Command::Indent,
            Command::Outdent,
            Command::MoveLineUp,
            Command::MoveLineDown,
            Command::TrimTrailingWhitespace,
            Command::TriggerCompletion,
        ],
    ),
    (
        "View",
        &[
            Command::OpenFind,
            Command::OpenReplace,
            Command::CloseFind,
            Command::FindNext,
            Command::ReplaceAll,
            Command::AddCursorAbove,
            Command::AddCursorBelow,
            Command::SelectNextOccurrence,
            Command::SelectAllOccurrences,
            Command::CollapseCursors,
            Command::FoldBlock,
            Command::UnfoldAll,
            Command::PageUp,
            Command::PageDown,
        ],
    ),
];

/// 建立菜单栏，返回 `菜单项 id -> 命令` 的映射。
///
/// 宿主必须在事件循环里轮询 `poll_menu_triggered()`，把返回的 id 喂给
/// [`dispatch_menu_event`]，菜单才真正有作用——菜单不会自己调用 Rust 回调。
fn build_menu_bar(
    win: &rust_widgets::app::WindowHandle,
    log: &Arc<EventLog>,
) -> Vec<(u64, MenuBinding)> {
    let bar = win.new_menu_bar(0, 0, WINDOW_W, MENU_H);
    log.append(format!("[MenuBar] created id={}", bar.raw_id()));

    if !win.attach_menu_bar(&bar) {
        log.append("[MenuBar] WARN: 后端拒绝挂载菜单栏，菜单将不可用");
    }

    let mut bindings = Vec::new();
    for (index, (title, items)) in MENU_STRUCTURE.iter().enumerate() {
        let menu = win.new_menu(&bar, title, 0, index as i32 * MENU_H as i32, 120, MENU_H);
        if menu.raw_id() == 0 {
            log.append(format!(
                "[MenuBar] ERROR: menu '{title}' 创建失败（parent 必须是 MenuBar）"
            ));
            continue;
        }
        for command in *items {
            // Typed shortcut: the platform renders the host's own notation, so
            // the menu reads `⌘Z` on macOS and `Ctrl+Z` on Windows/Linux from
            // this one call site.
            let item = win.new_menu_item_with_shortcut(&menu, command.label(), command.shortcut());
            if item.raw_id() == 0 {
                log.append(format!("[MenuBar] ERROR: 菜单项 '{}' 创建失败", command.label()));
                continue;
            }
            bindings.push((item.raw_id(), MenuBinding { command: *command }));
        }
        log.append(format!("[MenuBar] menu '{title}' with {} items", items.len()));
    }
    bindings
}

/// 把一次菜单激活事件分派给编辑器。
///
/// 返回是否命中某个绑定。未命中的 id 属于别的控件（窗口自带菜单等），忽略即可。
fn dispatch_menu_event(
    item_id: u64,
    bindings: &[(u64, MenuBinding)],
    editor: &CustomWidgetHandle,
    log: &Arc<EventLog>,
) -> bool {
    let Some((_, binding)) = bindings.iter().find(|(id, _)| *id == item_id) else {
        return false;
    };
    let changed = binding.command.dispatch(editor);
    log.append(format!(
        "[Menu] {} -> {}",
        binding.command.label(),
        if changed { "applied" } else { "no-op" }
    ));
    true
}

// ═══════════════════════════════════════════════════════════════════════════════
// 工具栏
// ═══════════════════════════════════════════════════════════════════════════════

/// 工具栏按钮布局：`(命令, 按钮宽度)`。命令为 `None` 时是分隔符。
const TOOLBAR_LAYOUT: &[(Option<Command>, i32)] = &[
    (Some(Command::Undo), 64),
    (Some(Command::Redo), 64),
    (None, 2),
    (Some(Command::Cut), 56),
    (Some(Command::Copy), 56),
    (Some(Command::Paste), 56),
    (None, 2),
    (Some(Command::ToggleComment), 72),
    (Some(Command::Indent), 64),
    (Some(Command::Outdent), 72),
    (None, 2),
    (Some(Command::OpenFind), 56),
    (Some(Command::AddCursorBelow), 72),
    (Some(Command::SortLines), 72),
];

/// 建立工具栏。按钮的 `on_click` 直接打到编辑器。
///
/// 工具栏按钮走的是标准回调路径，
/// 不需要轮询——这与菜单不同。
fn build_tool_bar(
    win: &rust_widgets::app::WindowHandle,
    editor: &CustomWidgetHandle,
    log: &Arc<EventLog>,
) {
    let bar = win.new_tool_bar(0, MENU_H as i32, WINDOW_W, TOOLBAR_H as u32);
    log.append(format!("[ToolBar] created id={}", bar.raw_id()));

    let mut x = 6i32;
    let top = MENU_H as i32;
    let mut button_count = 0usize;
    for (entry, width) in TOOLBAR_LAYOUT {
        match entry {
            None => {
                x += width;
            }
            Some(command) => {
                let button = win.new_button(
                    command.label(),
                    x,
                    top + 4,
                    *width as u32,
                    (TOOLBAR_H - 8) as u32,
                );
                // Record the shortcut next to the button in the event log. Native
                // buttons expose no tooltip through the handle API yet, so the
                // chord is surfaced here instead. It is rendered by the host OS,
                // so it reads `⌘Z` or `Ctrl+Z` as appropriate.
                match command.shortcut_label() {
                    Some(chord) => log
                        .append(format!("[ToolBar] {} button (shortcut {chord})", command.label())),
                    None => log.append(format!("[ToolBar] {} button", command.label())),
                }
                let editor = editor.clone();
                let log = Arc::clone(log);
                let command = *command;
                button.on_click(move || {
                    let changed = command.dispatch(&editor);
                    log.append(format!(
                        "[ToolBar] {} -> {}",
                        command.label(),
                        if changed { "applied" } else { "no-op" }
                    ));
                });
                x += width + 4;
                button_count += 1;
            }
        }
    }
    log.append(format!("[ToolBar] {button_count} buttons"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// run — App 入口，由 main() 调用
// ═══════════════════════════════════════════════════════════════════════════════

/// 启动 demo 应用。
///
/// `main()` 仅调用 `app::run()`；所有窗口创建、菜单/工具栏构建、事件循环都在这里。
pub fn run() {
    banner();

    let log = Arc::new(EventLog::new());
    let backend = rust_widgets::backend_name();
    println!("[run] backend={backend} gui_mode={:?}", rust_widgets::runtime_gui_mode());

    // ── 前置检查：当前后端必须真的能承载自绘型控件 ────────────────────
    //
    // 这一步不能省：把"工厂能造出对象"当成"窗口里能看到"会导致窗口空白却毫无提示。
    if !rust_widgets::supports_custom_widgets() {
        eprintln!(
            "错误：后端 '{backend}' 暂不能承载自绘型控件，无法显示 CodeEditor。\n\
             Linux 桌面请启用 gtk-native（见 demo/code_editor/Cargo.toml），\n\
             或在支持的桌面平台上运行。"
        );
        std::process::exit(1);
    }
    log.append(format!("[check] backend '{backend}' 支持自绘型控件"));

    let mut app = App::with_config(
        AppConfig::default().with_app_name("Code Editor Demo").with_organization("rust_widgets"),
    );
    app.init();
    log.append("[App] init 完成");

    let win = app.new_window("rust_widgets — CodeEditor", 80, 80, WINDOW_W, WINDOW_H);
    log.append(format!("[Window] 创建 id={:?}", win.raw_id()));

    // ── 挂载编辑器 ──────────────────────────────────────────────────────
    let rect = editor_rect();
    let editor_box: Box<dyn Widget> = Box::new(build_editor());
    let editor = match win.mount_custom_widget(editor_box, rect) {
        Ok(handle) => {
            log.append(format!(
                "[Mount] CodeEditor id={} rect=({},{}, {}, {})",
                handle.raw_id(),
                rect.x,
                rect.y,
                rect.width,
                rect.height
            ));
            handle
        }
        Err(error) => {
            eprintln!("错误：CodeEditor 挂载失败：{error}");
            std::process::exit(1);
        }
    };

    if !commands::is_editor(&editor) {
        eprintln!("错误：挂载的控件不是 CodeEditor，命令将无法生效");
        std::process::exit(1);
    }

    // ── 菜单栏 + 工具栏 + 状态栏 ────────────────────────────────────────
    let bindings = build_menu_bar(&win, &log);
    build_tool_bar(&win, &editor, &log);

    let status = win.new_status_bar(
        &commands::status_line(&editor).unwrap_or_default(),
        rect.x,
        rect.y + rect.height as i32,
        rect.width,
        STATUS_H as u32,
    );
    log.append(format!("[StatusBar] created id={}", status.raw_id()));

    win.show();
    log.append("[Window] shown");

    print_usage();
    run_loop(&app, &bindings, &editor, &log);
}

/// 轮询菜单/控件事件并转成命令。
///
/// # 为什么不能直接用 `app.run()`
///
/// `App::run()` 是阻塞的平台事件循环，且必须在主线程上跑。一旦进去就再也回不到
/// Rust 侧，`poll_menu_triggered()` 永远没机会被调用，菜单就变成纯装饰。
///
/// 正确做法与 C ABI 示例（`examples/c_abi_poll_demo.c`）一致：**在后台线程跑平台循环，
/// 主线程轮询队列**。菜单激活先进入平台队列，由宿主取出后分派给编辑器。
fn run_loop(
    app: &App,
    bindings: &[(u64, MenuBinding)],
    editor: &CustomWidgetHandle,
    log: &Arc<EventLog>,
) {
    // 平台事件循环跑在后台，主线程保持可轮询。
    let platform_loop = app.run_async();
    log.append("[App] 平台事件循环在后台线程启动");

    let mut handled = 0usize;
    let mut idle_ticks = 0u32;
    loop {
        let mut did_work = false;

        // 菜单激活：菜单不会直接回调 Rust，必须从队列取出。
        while let Some(item_id) = rust_widgets::poll_menu_triggered() {
            if dispatch_menu_event(item_id, bindings, editor, log) {
                did_work = true;
                handled += 1;
            }
        }

        // 控件触发（工具栏按钮已由 on_click 回调处理，这里只计数）。
        while let Some(event) = rust_widgets::poll_widget_trigger_event() {
            log.append(format!("[Widget] id={} kind={:?}", event.widget_id, event.kind));
            did_work = true;
            handled += 1;
        }

        if did_work {
            idle_ticks = 0;
            refresh_status(log, editor);
        } else {
            idle_ticks += 1;
            // 平台循环已退出（窗口关闭）时收工。
            if platform_loop.is_finished() {
                break;
            }
            // 事件循环空转超过 30 秒无任何事件也退出，避免无法关闭的孤儿进程。
            if idle_ticks > 30 * 60 * 2 {
                log.append("[App] 长时间无事件，自动退出");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }
    log.append(format!("[App] 轮询结束（处理 {handled} 个事件）"));
}

/// 把光标位置刷新到状态栏。
///
/// 状态栏直接改文本即可；`None` 表示控件已卸载（窗口正在关闭），
/// 此时不再访问平台，避免关窗期间的无效调用。
fn refresh_status(log: &Arc<EventLog>, editor: &CustomWidgetHandle) {
    let Some(line) = commands::status_line(editor) else {
        return;
    };
    log.append(format!("[Status] {line}"));
}

fn banner() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     rust_widgets  —  Code Editor Demo                  ║");
    println!("║     代码编辑器 · 菜单栏 + 工具栏 · 可交互                  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
}

fn print_usage() {
    println!();
    println!("窗口已打开：");
    println!("  · 菜单栏 Edit / View  → 全部编辑命令（带快捷键提示）");
    println!("  · 工具栏按钮          → 高频操作");
    println!("  · 鼠标点击 / 拖动     → 放置光标、选择文本");
    println!("  · 直接键入            → 插入文本（自动配对括号）");
    println!("  · Alt + ↑ / ↓         → 增加光标（多光标编辑）");
    println!();
    println!("关闭窗口即可退出。");
    println!();
}
