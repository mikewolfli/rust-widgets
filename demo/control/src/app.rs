//! Control Demo — 基础控件综合演示（基于 App 框架）
//!
//! 使用 App + WindowHandle + WidgetHandle 体系，创建窗口并放置各类控件。
//! 所有控件事件通过 handle.on_click / on_value_changed 实时记录。
//!
//! 布局（左列与右列都是普通控件，放置方式由库决定）：
//!   菜单栏 — File / View
//!   工具栏 + 状态栏 — ToolBar / StatusBar
//!   行 0 — Button:      Button, ToggleButton, disabled Button
//!   行 1 — Toggle:      CheckBox, tri-state CheckBox, RadioButton x3
//!   行 2 — Text input:  LineEdit（普通）, password LineEdit, read-only LineEdit, SpinBox
//!   行 3 — Selection:   ComboBox, ListBox
//!   行 4 — Range:       Slider, ProgressBar, indeterminate ProgressBar
//!   行 5 — Text area:   ScrollArea（内含多行 Label）
//!   行 6 — Panel/Frame: GroupBox 面板 + Frame
//!   行 7 — Dialog:      MessageBox
//!   右列 — CodeEditor + Chip
//!   底部 — 事件日志
//!
//! # 跨平台（本 demo 的核心示范）
//!
//! 本 demo 没有任何 `cfg(target_os)`，也不出现任何 OS 控件名。**同一段代码**
//! 在所有平台跑：控件怎么落地（OS 控件还是库自己的绘制面）是 `src/platform/`
//! 的内部决定，调用方不需要知道，也无从知道。
//!
//! 唯一需要运行时询问的是**能力存在与否**，例如 `supports_custom_widgets()`：
//! 一个平台若暂时无法承载自绘型控件，接口会如实回答 `false`，而不是让调用方
//! 去按 OS 分支。

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

    let btn = win.new_button("Click Me", 20, 20, 150, 32);
    let l = Arc::clone(log);
    btn.on_click(move || l.append("[Button] clicked!"));
    log.append("[Button] at (20,20,150,32)");

    let tog = win.new_button("Dark Mode", 180, 20, 150, 32);
    let l = Arc::clone(log);
    tog.on_click(move || l.append("[ToggleButton] toggled!"));
    log.append("[ToggleButton] at (180,20,150,32)");

    // Disabled button: proves the enable/disable mirror reaches the native control.
    let disabled = win.new_button("Disabled", 340, 20, 150, 32);
    disabled.disable();
    log.append(format!("[Button] disabled sample enabled={}", disabled.is_enabled()));

    // ── Row 1: Toggle Controls ───────────────────────────────────────
    log.append("═══ Row: Toggle Controls ═══");

    let cb = win.new_checkbox("Enable notifications", 20, 64, 200, 24);
    let l = Arc::clone(log);
    let cb2 = cb.clone();
    cb2.on_value_changed(move |_val: String| {
        let checked = cb.is_checked();
        l.append(format!("[CheckBox] checked={}", checked));
    });
    log.append("[CheckBox] at (20,64,200,24)");

    // Tri-state check-box: all three states are reachable (Unchecked / Checked /
    // PartiallyChecked), which the mixed state requires tri-state mode to be on.
    let tri = win.new_checkbox("Tri-state", 230, 64, 130, 24);
    tri.set_tristate(true);
    tri.set_check_state(rust_widgets::app::CheckState::PartiallyChecked);
    log.append(format!(
        "[CheckBox] tri-state enabled={:?} state={:?} is_checked={}",
        tri.is_tristate(),
        tri.check_state(),
        tri.is_checked()
    ));

    // ── Row 2: Text Input Controls ───────────────────────────────────
    log.append("═══ Row: Text Input Controls ═══");

    let le = win.new_line_edit("", 20, 104, 200, 26);
    le.set_placeholder("Type here...");
    le.set_max_length(32);
    let l = Arc::clone(log);
    let le2 = le.clone();
    le2.on_value_changed(move |_val: String| {
        l.append(format!("[LineEdit] text='{}'", le.text()));
    });
    log.append("[LineEdit] placeholder + max_length=32");

    // Password field: echo mode is a real widget property; a platform whose text
    // control cannot express it reports so via `false`/`None` instead of lying.
    let pw = win.new_line_edit("secret", 230, 104, 150, 26);
    pw.set_echo_mode(rust_widgets::app::EchoMode::Password);
    log.append(format!("[LineEdit] echo_mode={:?}", pw.widget_echo_mode()));

    let ro = win.new_line_edit("read-only value", 390, 104, 180, 26);
    ro.set_read_only(true);
    log.append(format!("[LineEdit] read_only={:?}", ro.clone().is_read_only()));

    let sb = win.new_spin_box(580, 104, 120, 26);
    sb.set_range(0, 100);
    sb.set_value(50);
    sb.set_prefix("$");
    sb.set_suffix(".00");
    sb.set_step(5);
    let l = Arc::clone(log);
    let sb2 = sb.clone();
    sb2.on_value_changed(move |_val: String| {
        l.append(format!("[SpinBox] value={}", sb.value()));
    });
    log.append("[SpinBox] range=[0..100], value=50, step=5");

    // ── Row 3: Selection Controls ────────────────────────────────────
    log.append("═══ Row: Selection Controls ═══");

    let cbx = win.new_combo_box(20, 142, 180, 26);
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

    let lb = win.new_list_box(210, 142, 200, 90);
    for item in ["Alpha", "Bravo", "Charlie", "Delta", "Echo"] {
        lb.add_item(item);
    }
    lb.set_current_index(0);
    let l = Arc::clone(log);
    let lb2 = lb.clone();
    let lb3 = lb.clone();
    lb2.on_value_changed(move |_val: String| {
        let idx = lb3.current_index().unwrap_or(0);
        let text = lb3.item_text(idx).unwrap_or_else(|| String::from("(none)"));
        l.append(format!("[ListBox] selected '{}' (idx={})", text, idx));
    });
    log.append(format!("[ListBox] {} items", lb.item_count()));

    // Radio group lives beside the selection controls; it is a selection input.
    let rb1 = win.new_radio_button("Option A", 420, 142, 100, 24);
    rb1.set_group("opts");
    let rb2 = win.new_radio_button("Option B", 420, 168, 100, 24);
    rb2.set_group("opts");
    let rb3 = win.new_radio_button("Option C", 420, 194, 100, 24);
    rb3.set_group("opts");
    let l = Arc::clone(log);
    let r2 = rb2.clone();
    r2.on_value_changed(move |_val: String| {
        if rb2.is_selected() {
            l.append("[RadioButton] Option B selected");
        }
    });
    log.append("[RadioButton] 3 options in group 'opts'");

    // ── Row 4: Range Controls ────────────────────────────────────────
    log.append("═══ Row: Range Controls ═══");

    // Orientation is a creation-time property on every native toolkit (Win32 has
    // no runtime message for it), so it is passed at construction instead of being
    // set afterwards.
    let sl = win.new_slider_with_orientation(Orientation::Horizontal, 20, 242, 260, 32);
    sl.set_range(0, 100);
    sl.set_value(50);
    sl.set_step(5);
    let l = Arc::clone(log);
    let sl2 = sl.clone();
    let sl3 = sl.clone();
    sl2.on_value_changed(move |_val: String| {
        l.append(format!("[Slider] value={}", sl3.value()));
    });
    log.append(format!("[Slider] value=50 orientation={:?}", sl.orientation()));

    let pb = win.new_progress_bar(290, 242, 200, 24);
    pb.set_min(0u32);
    pb.set_max(100u32);
    pb.set_value(75u32);
    let l = Arc::clone(log);
    let pb2 = pb.clone();
    pb2.on_value_changed(move |_val: String| {
        l.append(format!("[ProgressBar] value={}", pb.value()));
    });
    log.append("[ProgressBar] range=[0..100], value=75");

    // Indeterminate (busy) bar: a distinct native code path from a fixed fraction.
    let busy = win.new_progress_bar(500, 242, 200, 24);
    busy.set_indeterminate(true);
    log.append(format!("[ProgressBar] indeterminate={:?}", busy.is_indeterminate()));

    // ── Row 5: Scrollable Text Area ──────────────────────────────────
    log.append("═══ Row: Scrollable Text Area ═══");

    let _area = win.new_scroll_area(20, 286, 260, 90);
    for i in 0..6 {
        let _line = win.new_label(&format!("log line {i} — scroll me"), 28, 294 + i * 20, 244, 18);
    }
    log.append("[ScrollArea] with 6 stacked labels at (20,286,260,90)");

    // ── Row 6: Panel / Frame ─────────────────────────────────────────
    log.append("═══ Row: Panel / Frame ═══");

    let panel = win.new_panel(290, 286, 200, 90);
    panel.set_title("Panel");
    let frame = win.new_frame(500, 286, 200, 90);
    frame.set_text("Frame");
    log.append(format!("[Panel] id={:?}  [Frame] id={:?}", panel.raw_id(), frame.raw_id()));

    // ── Row 7: Dialog ────────────────────────────────────────────────
    log.append("═══ Row: Dialog Controls ═══");

    let msg = win.new_message_box(
        "Demo Info",
        "This is a demo message box.\nAll controls are working!",
        20,
        386,
        300,
        100,
    );
    msg.set_title("Information");
    let l = Arc::clone(log);
    msg.on_click(move || l.append("[MessageBox] button clicked!"));
    log.append("[MessageBox] at (20,386,300,100)");

    // ── Log Panel (底部 4 行标签) ─────────────────────────────────────
    log.append("═══ Log Panel ═══");

    let _title = win.new_label("── Event Log ──", 20, 496, 680, 18);
    for i in 0..4 {
        let _row = win.new_label("", 20, 518 + i * 20, 680, 18);
    }
    log.append("[LogPanel] 4 label rows at bottom");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 工具栏 / 状态栏
// ═══════════════════════════════════════════════════════════════════════════════

/// 创建工具栏与状态栏，证明窗口级 chrome 控件也在统一 API 覆盖内。
fn build_window_chrome(win: &WindowHandle, log: &Arc<EventLog>) {
    log.append("═══ Window Chrome ═══");

    let bar = win.new_tool_bar(0, 0, 760, 32);
    log.append(format!("[ToolBar] id={:?}", bar.raw_id()));

    let status = win.new_status_bar("Ready", 0, 0, 760, 24);
    let l = Arc::clone(log);
    status.on_value_changed(move |val: String| l.append(format!("[StatusBar] '{val}'")));
    log.append("[StatusBar] text='Ready'");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ═══════════════════════════════════════════════════════
// 自绘型控件（控件自己绘制内容，放置方式由库决定）
// ═══════════════════════════════════════════════════════

/// 挂载自绘型控件，证明它们能和其它控件放在同一个窗口里。
///
/// `CodeEditor` 这类控件自己负责绘制内容，因此没有对应的 `new_*` 工厂方法；
/// 用 `mount_custom_widget` 交给窗口托管。至于窗口用**什么**来承载它（子窗口？
/// 绘制区？视图？）是 `src/platform/` 的内部细节，本 demo 不需要知道，
/// 跨平台也是同一段代码。
///
/// 唯一要问的是**能力是否存在**：`supports_custom_widgets()` 返回 `false` 时，
/// 说明当前平台暂时撑不住这类控件，如实跳过即可。
fn build_custom_painted_controls(win: &WindowHandle, log: &Arc<EventLog>) {
    use rust_widgets::core::Rect;
    use rust_widgets::widget::special_widgets::code_editor::{CodeEditorConfig, LanguageId};

    log.append("═══ Row: Custom-painted Widgets ═══");

    if !rust_widgets::supports_custom_widgets() {
        log.append(format!(
            "[Custom] 当前后端 '{}' 暂不能承载自绘型控件，跳过本区域",
            rust_widgets::backend_name()
        ));
        return;
    }

    // CodeEditor：右列 340x430，与左列控件并排。
    let rect = Rect::new(720, 48, 340, 430);
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
            match win.mount_custom_widget(widget, rect) {
                Ok(handle) => {
                    log.append(format!("[Custom] CodeEditor 挂载成功 id={}", handle.raw_id()))
                }
                Err(error) => log.append(format!("[Custom] CodeEditor 挂载失败：{error}")),
            }
        }
        Err(error) => log.append(format!("[Custom] CodeEditor 配置非法：{error}")),
    }

    // Chip：第二个自绘型控件，证明这条通道不是为单一控件开的。
    let chip_rect = Rect::new(720, 490, 160, 40);
    let factory = rust_widgets::widget::WidgetFactory::new_with_defaults();
    match factory.create("chip", chip_rect, "chip") {
        Some(chip) => match win.mount_custom_widget(chip, chip_rect) {
            Ok(handle) => log.append(format!("[Custom] Chip 挂载成功 id={}", handle.raw_id())),
            Err(error) => log.append(format!("[Custom] Chip 挂载失败：{error}")),
        },
        None => log.append("[Custom] Chip 未在控件工厂注册"),
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
    println!("║     rust_widgets  —  Controls Demo v1.1.3             ║");
    println!("║     App 框架 · 统一控件 API · 实时事件日志                ║");
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

    // 说明当前运行后端：无窗口的后端（如 Linux 未启用 gtk-native 时）
    // 会如实提示，避免"事件日志正常但看不到窗口"的困惑。
    let backend = rust_widgets::backend_name();
    let gui_mode = rust_widgets::runtime_gui_mode();
    println!("[run] backend={backend} gui_mode={gui_mode:?}");
    if gui_mode == rust_widgets::RuntimeGuiMode::PreviewOrStub {
        println!(
            "[run] 提示: 当前后端不创建窗口。Linux 桌面请以 `gtk-native` feature 构建: \
             cargo run --features ... (见 demo/control/Cargo.toml)"
        );
    }

    // init: 初始化平台 + i18n
    app.init();
    log.append("[App] init() done");

    // 创建窗口（必须在 init 之后，run 之前）
    let win = app.new_window("Controls Demo — rust_widgets", 100, 100, 1120, 620);
    log.append(format!("[Window] created: id={:?}", win.raw_id()));

    // 菜单栏：File / View
    let menu_bar = win.new_menu_bar(0, 0, 0, 0);
    let file_menu = win.new_menu(&menu_bar, "File", 0, 0, 0, 0);
    win.new_menu_item_with_shortcut(
        &file_menu,
        "Open",
        Some(rust_widgets::shortcut::Shortcut::primary(rust_widgets::shortcut::Key::O)),
    );
    win.new_menu(&menu_bar, "View", 0, 0, 0, 0);
    win.attach_menu_bar(&menu_bar);
    log.append("[MenuBar] File / View attached");

    // 窗口级 chrome：工具栏 + 状态栏
    build_window_chrome(&win, &log);

    // 构建全部基础控件
    build_all_controls(&win, &log);
    log.append("[App] native controls ready");

    // 构建自绘型控件：它们与其它控件同窗混排，放置方式由库决定。
    build_custom_painted_controls(&win, &log);
    log.append("[App] controls ready — starting event loop");

    // 显示窗口：WindowHandle::show() → platform show_widget → GTK show_all。
    // 不调用则事件循环正常但窗口永不可见（Linux/GTK 下尤为明显）。
    win.show();
    log.append(format!("[Window] shown: id={:?}", win.raw_id()));

    // run: 启动平台事件循环，显示窗口
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
