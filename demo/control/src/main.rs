//! Control Demo — 基础控件综合演示（基于 App 框架）
//!
//! 使用 App + WindowHandle + WidgetHandle 体系，
//! 创建窗口并在其中放置各类基础控件。
//! 所有控件事件通过 handle.on_click / on_value_changed 实时记录。
//!
//! 控件分类（覆盖窗口坐标区域）：
//!   行 0 — Button:   Button, ToggleButton
//!   行 1 — Toggle:   CheckBox, RadioButton x3, Switch
//!   行 2 — Input:    SpinBox, ComboBox, LineEdit
//!   行 3 — Range:    Slider, ProgressBar
//!   行 4 — Dialog:   MessageBox
//!   底部 — Log:      4 行标签显示最新事件

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
        l.append(&format!("[CheckBox] checked={}", checked));
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
        l.append(&format!("[SpinBox] value={}", sb.value()));
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
        l.append(&format!("[ComboBox] selected '{}' (idx={})", text, idx));
    });
    log.append("[ComboBox] items=[Red,Green,Blue,Yellow]");

    let le = win.new_line_edit("", 360, 170, 200, 28);
    le.set_placeholder("Type here...");
    let l = Arc::clone(log);
    let le2 = le.clone();
    le2.on_value_changed(move |_val: String| {
        l.append(&format!("[LineEdit] text='{}'", le.text()));
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
        l.append(&format!("[Slider] value={}", sl.value()));
    });
    log.append("[Slider] range=[0..100], value=50");

    let pb = win.new_progress_bar(340, 220, 220, 28);
    pb.set_min(0u32);
    pb.set_max(100u32);
    pb.set_value(75u32);
    let l = Arc::clone(log);
    let pb2 = pb.clone();
    pb2.on_value_changed(move |_val: String| {
        l.append(&format!("[ProgressBar] value={}", pb.value()));
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
// main — App 入口，使用 app.run() 显示原生窗口
// ═══════════════════════════════════════════════════════════════════════════════

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     rust_widgets  —  Controls Demo v0.9.9              ║");
    println!("║     App 框架 · 原生窗口 · 实时事件日志                  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let log = Arc::new(EventLog::new());

    // 创建 App
    let app = App::with_config(
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

    // init: 初始化平台 + i18n
    app.init();
    log.append("[App] init() done");

    // 创建窗口（必须在 init 之后，run 之前）
    let win = app.new_window("Controls Demo — rust_widgets", 100, 100, 600, 520);
    log.append(&format!("[Window] created: id={:?}", win.raw_id()));

    // 构建全部控件
    build_all_controls(&win, &log);
    log.append("[App] controls ready — starting event loop");

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
