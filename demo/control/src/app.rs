//! Control Demo — 基础控件综合演示（基于 App 框架）
//!
//! 使用 App + WindowHandle + WidgetHandle 体系，创建窗口并放置各类控件。
//! 所有控件事件通过 handle.on_click / on_value_changed 实时记录。
//!
//! 布局：
//!   菜单栏 — File / View（原生菜单，演示菜单事件轮询）
//!   行 0 — Button:   Button, ToggleButton
//!   行 1 — Toggle:   CheckBox, RadioButton x3
//!   行 2 — Input:    SpinBox, ComboBox, LineEdit
//!   行 3 — Range:    Slider, ProgressBar
//!   行 4 — Dialog:   MessageBox（不崩溃：对话框不是 view）
//!   右列 — 自绘控件: CodeEditor + Chip（原生画布渲染）
//!   底部 — StatusBar + 事件日志
//!
//! # 跨平台
//!
//! 本 demo 不含任何 `cfg(target_os)`。平台差异全部由库在运行时暴露：
//! `supports_self_drawn()` 决定是否挂载自绘控件，菜单/工具栏在所有桌面后端都有
//! 统一 API（详见 `docs/plans/platform_differences.md`）。

use std::sync::{Arc, Mutex};

use rust_widgets::app::{App, AppConfig, WidgetHandle, WindowHandle};
use rust_widgets::core::Orientation;

// ═══════════════════════════════════════════════════════════════════════════════
// Log System — 线程安全的事件日志
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

    fn snapshot(&self) -> Vec<String> {
        self.entries.lock().unwrap().clone()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 构建所有控件 + 日志面板
// ═══════════════════════════════════════════════════════════════════════════════

fn build_all_controls(win: &WindowHandle, log: &Arc<EventLog>) {
    // ── Row 0: Button Controls ───────────────────────────────────────
    log.append("═══ Row: Button Controls ═══");

    let btn = win.new_button("Click Me", 20, 20, 160, 36);
    let l = Arc::clone(log);
    btn.on_click(move || l.append("[Button] clicked!"));
    log.append("[Button] at (20,20,160,36)");

    let tog = win.new_button("Dark Mode", 20, 70, 160, 36);
    let l = Arc::clone(log);
    tog.on_click(move || l.append("[ToggleButton] toggled!"));
    log.append("[ToggleButton] at (20,70,160,36)");

    // ── Row 1: Toggle Controls ───────────────────────────────────────
    log.append("═══ Row: Toggle Controls ═══");

    let cb = win.new_checkbox("Enable notifications", 20, 120, 200, 28);
    let l = Arc::clone(log);
    let cb2 = cb.clone();
    cb2.on_value_changed(move |_val: String| {
        let checked = cb.is_checked();
        l.append(format!("[CheckBox] checked={}", checked));
    });
    log.append("[CheckBox] at (20,120,200,28)");

    let rb1 = win.new_radio_button("Option A", 230, 120, 100, 28);
    rb1.set_group("opts");
    let rb2 = win.new_radio_button("Option B", 340, 120, 100, 28);
    rb2.set_group("opts");
    let rb3 = win.new_radio_button("Option C", 450, 120, 100, 28);
    rb3.set_group("opts");
    let l = Arc::clone(log);
    let r2 = rb2.clone();
    r2.on_value_changed(move |_val: String| {
        if rb2.is_selected() {
            l.append("[RadioButton] Option B selected");
        }
    });
    log.append("[RadioButton] 3 options in group 'opts'");

    // ── Row 2: Input Controls ────────────────────────────────────────
    log.append("═══ Row: Input Controls ═══");

    let sb = win.new_spin_box(20, 170, 120, 28);
    sb.set_range(0, 100);
    sb.set_value(50);
    sb.set_prefix("$");
    sb.set_suffix(".00");
    let l = Arc::clone(log);
    let sb2 = sb.clone();
    sb2.on_value_changed(move |_val: String| {
        l.append(format!("[SpinBox] value={}", sb.value()));
    });
    log.append("[SpinBox] range=[0..100], value=50");

    let cbx = win.new_combo_box(160, 170, 180, 28);
    cbx.add_item("Red");
    cbx.add_item("Green");
    cbx.add_item("Blue");
    cbx.add_item("Yellow");
    cbx.set_current_index(0);
    let l = Arc::clone(log);
    let cbx2 = cbx.clone();
    cbx2.on_value_changed(move |_val: String| {
        let idx = cbx.current_index().unwrap_or(0);
        let text = cbx.item_text(idx).unwrap_or_else(|| String::from("(none)"));
        l.append(format!("[ComboBox] selected '{}' (idx={})", text, idx));
    });
    log.append("[ComboBox] items=[Red,Green,Blue,Yellow]");

    let le = win.new_line_edit("", 360, 170, 200, 28);
    le.set_placeholder("Type here...");
    let l = Arc::clone(log);
    let le2 = le.clone();
    le2.on_value_changed(move |_val: String| {
        l.append(format!("[LineEdit] text='{}'", le.text()));
    });
    log.append("[LineEdit] with placeholder");

    // ── Row 3: Range Controls ────────────────────────────────────────
    log.append("═══ Row: Range Controls ═══");

    let sl = win.new_slider(20, 220, 300, 40);
    sl.set_range(0, 100);
    sl.set_value(50);
    sl.set_step(5);
    sl.set_orientation(Orientation::Horizontal);
    let l = Arc::clone(log);
    let sl2 = sl.clone();
    sl2.on_value_changed(move |_val: String| {
        l.append(format!("[Slider] value={}", sl.value()));
    });
    log.append("[Slider] range=[0..100], value=50");

    let pb = win.new_progress_bar(340, 220, 220, 28);
    pb.set_min(0u32);
    pb.set_max(100u32);
    pb.set_value(75u32);
    let l = Arc::clone(log);
    let pb2 = pb.clone();
    pb2.on_value_changed(move |_val: String| {
        l.append(format!("[ProgressBar] value={}", pb.value()));
    });
    log.append("[ProgressBar] range=[0..100], value=75");

    // ── Row 4: Dialog / MessageBox ───────────────────────────────────
    log.append("═══ Row: Dialog Controls ═══");

    let msg = win.new_message_box(
        "Demo Info",
        "This is a demo message box.\nAll controls are working!",
        20,
        280,
        300,
        120,
    );
    msg.set_title("Information");
    let l = Arc::clone(log);
    msg.on_click(move || l.append("[MessageBox] button clicked!"));
    log.append("[MessageBox] at (20,280,300,120)");

    // ── Log Panel (底部 4 行标签) ─────────────────────────────────────
    log.append("═══ Log Panel ═══");

    let _title = win.new_label("── Event Log ──", 10, 410, 580, 20);
    for i in 0..4 {
        let _row = win.new_label("", 10, 435 + i * 22, 580, 20);
    }
    log.append("[LogPanel] 4 label rows at bottom");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 自绘控件（native canvas）
// ═══════════════════════════════════════════════════════════════════════════════

/// 挂载自绘控件，证明它们能和原生控件放在同一个窗口里。
///
/// `CodeEditor` 这类控件没有对应的 OS 控件，通过 `mount_self_drawn` 交给原生画布
/// 渲染；能力不足的后端会如实返回错误，而不是留下一个空白区域。
fn build_self_drawn_controls(win: &WindowHandle, log: &Arc<EventLog>) {
    use rust_widgets::core::Rect;
    use rust_widgets::widget::special_widgets::code_editor::{CodeEditorConfig, LanguageId};

    log.append("═══ Row: Self-drawn Widgets ═══");

    if !rust_widgets::supports_self_drawn() {
        log.append(format!(
            "[SelfDrawn] 后端 '{}' 不支持自绘控件，跳过本区域",
            rust_widgets::backend_name()
        ));
        return;
    }

    // CodeEditor：右侧 360x520，与左侧原生控件并排。
    let rect = Rect::new(610, 20, 350, 480);
    let config = CodeEditorConfig::new()
        .language(LanguageId::Rust)
        .tab_width(4)
        .show_line_numbers(true)
        .show_minimap(false);
    match rust_widgets::widget::special_widgets::code_editor::CodeEditor::with_config(rect, config)
    {
        Ok(mut editor) => {
            editor.set_text("fn main() {\n    let x = 1;\n}\n");
            let widget: Box<dyn rust_widgets::widget::Widget> = Box::new(editor);
            match win.mount_self_drawn(widget, rect) {
                Ok(handle) => {
                    log.append(format!("[SelfDrawn] CodeEditor 挂载成功 id={}", handle.raw_id()))
                }
                Err(error) => log.append(format!("[SelfDrawn] CodeEditor 挂载失败：{error}")),
            }
        }
        Err(error) => log.append(format!("[SelfDrawn] CodeEditor 配置非法：{error}")),
    }

    // Chip：第二个自绘控件，证明通道不是为单一控件开的。
    let chip_rect = Rect::new(610, 520, 160, 40);
    let factory = rust_widgets::widget::WidgetFactory::new_with_defaults();
    match factory.create("chip", chip_rect, "chip") {
        Some(chip) => match win.mount_self_drawn(chip, chip_rect) {
            Ok(handle) => log.append(format!("[SelfDrawn] Chip 挂载成功 id={}", handle.raw_id())),
            Err(error) => log.append(format!("[SelfDrawn] Chip 挂载失败：{error}")),
        },
        None => log.append("[SelfDrawn] Chip 未在控件工厂注册"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// run — App 入口，由 main() 调用
// ═══════════════════════════════════════════════════════════════════════════════

/// 启动 demo 应用。
///
/// 整个应用的入口逻辑集中于此函数中，`main()` 仅调用 `app::run()`。
/// 这种设计确保 `main` 只作为启动桩（stub），所有业务逻辑都在 app 层管理。
pub fn run() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     rust_widgets  —  Controls Demo v1.1.2             ║");
    println!("║     App 框架 · 原生窗口 · 实时事件日志                  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let log = Arc::new(EventLog::new());

    // 创建 App
    let mut app = App::with_config(
        AppConfig::default().with_app_name("Controls Demo").with_organization("rust_widgets"),
    )
    .on_startup({
        let log = Arc::clone(&log);
        move || {
            log.append("[App] startup: platform initialized");
        }
    })
    .on_shutdown({
        let log = Arc::clone(&log);
        move || {
            log.append("[App] shutdown");
        }
    });

    log.append("[App] created");

    // 说明当前运行后端：无原生窗口的后端（如 Linux 未启用 gtk-native 时）
    // 会如实提示，避免"事件日志正常但看不到窗口"的困惑。
    let backend = rust_widgets::backend_name();
    let gui_mode = rust_widgets::runtime_gui_mode();
    println!("[run] backend={backend} gui_mode={gui_mode:?}");
    if gui_mode == rust_widgets::RuntimeGuiMode::PreviewOrStub {
        println!(
            "[run] 提示: 当前后端不创建原生窗口。Linux 桌面请以 `gtk-native` feature 构建: \
             cargo run --features ... (见 demo/control/Cargo.toml)"
        );
    }

    // init: 初始化平台 + i18n
    app.init();
    log.append("[App] init() done");

    // 创建窗口（必须在 init 之后，run 之前）
    let win = app.new_window("Controls Demo — rust_widgets", 100, 100, 980, 620);
    log.append(format!("[Window] created: id={:?}", win.raw_id()));

    // 构建全部原生控件
    build_all_controls(&win, &log);
    log.append("[App] native controls ready");

    // 构建自绘控件：证明原生控件与自绘控件可以同窗混排。
    build_self_drawn_controls(&win, &log);
    log.append("[App] controls ready — starting event loop");

    // 显示窗口：WindowHandle::show() → platform show_widget → GTK show_all。
    // 不调用则事件循环正常但窗口永不可见（Linux/GTK 下尤为明显）。
    win.show();
    log.append(format!("[Window] shown: id={:?}", win.raw_id()));

    // run: 启动平台事件循环，显示原生窗口
    app.run();

    log.append("[App] event loop exited");

    // 打印最终日志
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                     EVENT LOG                           ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    let entries = log.snapshot();
    for (i, entry) in entries.iter().enumerate() {
        println!("  [{:>3}] {}", i + 1, entry);
    }
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  Total events: {}                                      ║", entries.len());
    println!("╚══════════════════════════════════════════════════════════╝");
}
