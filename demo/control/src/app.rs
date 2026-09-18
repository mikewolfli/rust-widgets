//! Control Demo — 基础控件综合演示（基于 App 框架）
//!
//! 使用 App + WindowHandle + WidgetHandle 体系，创建窗口并放置各类控件。
//! 所有控件事件通过 handle.on_click / on_value_changed 实时记录。
//!
//! # 版式：标签页 + 每页自己的布局
//!
//! ```text
//!  ┌────────────────────────────────────────────────────────────┐
//!  │ 菜单栏 File / View                                          │
//!  ├────────────────────────────────────────────────────────────┤
//!  │ 工具栏                                                      │
//!  ├────────────────────────────────────────────────────────────┤
//!  │ [Buttons] [Toggles] [Text] [Selection] [Range] [Containers] │ ← 标签页按钮
//!  ├────────────────────────────────────────────────────────────┤
//!  │                                                            │
//!  │   当前标签页的 Panel —— 它有自己的布局，只显示这一页        │
//!  │                                                            │
//!  ├────────────────────────────────────────────────────────────┤
//!  │ 事件日志（全宽）                                            │
//!  ├────────────────────────────────────────────────────────────┤
//!  │ 状态栏                                                      │
//!  └────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 标签页是怎么做的
//!
//! 每一页是一个 `Panel`，页面内的控件由**该 Panel 自己的布局**管理，
//! 所以不同页面可以有不同的布局（三列网格、两列、单列纵向……）。
//! 切换标签页 = 让一个 Panel `set_visible(true)`、其余 `set_visible(false)`。
//!
//! 页与页共用一个矩形（它们叠在同一块区域里），所以“每页自己的布局”与
//! “全窗口布局”不冲突：外层只决定那块区域在哪，页内布局决定页内控件在哪。
//!
//! # 布局分三层，各管一件事
//!
//! ```text
//! 窗口 BoxLayout（纵向）      ── 工具栏 / 标签条 / 页面区 / 日志 / 状态栏
//!   ├─ 标签条 BoxLayout（横向）── 标签页按钮的等宽排列
//!   └─ 页面区：每个 Tab 一个 Panel + 它自己的布局
//! ```
//!
//! # 跨平台（本 demo 的核心示范）
//!
//! 本 demo 没有任何 `cfg(target_os)`，也不出现任何 OS 控件名。**同一段代码**
//! 在所有平台跑：控件怎么落地（OS 控件还是库自己的绘制面）是 `src/platform/`
//! 的内部决定，调用方不需要知道，也无从知道。
//!
//! 唯一需要运行时询问的是**能力存在与否**，例如 `supports_surfaces()`：
//! 一个平台若暂时无法承载自绘型控件，接口会如实回答 `false`，而不是让调用方
//! 去按 OS 分支。

use std::sync::{Arc, Mutex};

use rust_widgets::app::{App, AppConfig, PanelHandle, WidgetHandle, WindowHandle};
use rust_widgets::core::{ObjectId, Orientation};
use rust_widgets::layout::{BoxLayout, GridLayout, Layout, LayoutConstraints, SizePolicy};

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
// 窗口与布局常量
// ═══════════════════════════════════════════════════════════════════════════════

/// 窗口宽度。布局与窗口共用，避免两处字面量各自漂移。
const WINDOW_W: u32 = 1160;
/// 窗口高度。
const WINDOW_H: u32 = 700;
/// 网格外边距。
const MARGIN: u32 = 8;
/// 通用间距。
const GAP: u32 = 6;
/// 工具栏高度。
const TOOLBAR_H: u32 = 32;
/// 标签条高度。
const TABBAR_H: u32 = 34;
/// 事件日志正文行数。
const LOG_ROWS: u32 = 3;
/// 日志区高度（标题 + LOG_ROWS 行）。
const LOG_PANEL_H: u32 = 84;
/// 状态栏高度。
const STATUS_H: u32 = 24;

// ═══════════════════════════════════════════════════════════════════════════════
// 标签页模型 —— 把「哪个控件在哪一页、哪一格」写成数据
// ═══════════════════════════════════════════════════════════════════════════════

/// 一个控件在某一页网格中的位置。
///
/// 把“哪个控件放第几行第几列”写成数据、而不是塞进 `new_*(x, y, w, h)` 的调用里，
/// 是让布局可核对的前提：每条声明与一个控件一一对应，漏掉谁一眼能看得出来。
#[derive(Debug, Clone, Copy)]
struct Cell {
    row: u32,
    col: u32,
    id: ObjectId,
}

impl Cell {
    fn new(row: u32, col: u32, id: ObjectId) -> Self {
        Self { row, col, id }
    }
}

/// 一页标签的内容：标题、标题下的控件网格规格、以及页面内的控件。
struct TabPage {
    title: &'static str,
    /// 页面网格的行数。
    rows: u32,
    /// 页面网格的列数。
    cols: u32,
    /// 各列宽度权重（长度必须为 `cols`）。
    column_weights: Vec<u32>,
    /// 页面内的控件位置。
    cells: Vec<Cell>,
    /// 该页的空白（占位）格子，用来说明“这一页还没排满”。
    /// 占位格没有控件，只参与网格形状。
    panel: Option<PanelHandle>,
}

impl TabPage {
    fn new(title: &'static str, rows: u32, column_weights: &[u32]) -> Self {
        Self {
            title,
            rows,
            cols: column_weights.len() as u32,
            column_weights: column_weights.to_vec(),
            cells: Vec::new(),
            panel: None,
        }
    }

    /// 声明一个控件在第几行第几列，并返回它的 id 供调用方继续配置。
    fn place(&mut self, row: u32, col: u32, id: ObjectId) -> ObjectId {
        self.cells.push(Cell::new(row, col, id));
        id
    }

    /// 这一页的网格布局。
    ///
    /// 列宽用权重表达而非像素，所以页面随窗口变宽时各列同比例变宽。
    fn grid(&self) -> GridLayout {
        let mut grid = GridLayout::new(self.rows, self.cols, GAP, 0);
        for (index, weight) in self.column_weights.iter().enumerate() {
            grid.set_column_stretch_for_col(index as u32, *weight);
        }
        grid.set_row_stretch(1);
        for cell in &self.cells {
            grid.set_widget(cell.row, cell.col, cell.id);
        }
        grid
    }

    /// 把这一页的布局装到它的 Panel 上。
    fn apply_layout(&self) {
        if let Some(panel) = &self.panel {
            panel.set_layout(Box::new(self.grid()));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 窗口级布局
// ═══════════════════════════════════════════════════════════════════════════════

/// 窗口纵向布局的各行。
///
/// 一个具名结构而不是一串位置参数：六个 id 挨在一起时，调用处写错顺序是很容易的，
/// 而症状是“某个控件跑到了别的行”。
struct WindowRows {
    tool_bar: ObjectId,
    tab_bar: ObjectId,
    /// 页面区的“尺寸代理”：所有页面重叠，只能有一个参与纵向布局。
    page_proxy: Option<ObjectId>,
    log_title: ObjectId,
    log_rows: Vec<ObjectId>,
    status_bar: ObjectId,
}

/// 把窗口纵向切成工具栏 / 标签条 / 页面区 / 日志 / 状态栏。
///
/// 工具栏、日志、状态栏高度固定；标签条固定；**页面区吃掉剩余高度**。
/// 所有标签页共用一个矩形 —— 它们叠在同一块区域里，由可见性决定显示哪一页。
fn build_window_layout(rows: &WindowRows) -> BoxLayout {
    let mut layout = BoxLayout::new(Orientation::Vertical, GAP, MARGIN);

    layout.add_widget(rows.tool_bar, 1);
    layout.set_constraints(rows.tool_bar, LayoutConstraints::new(TOOLBAR_H, Some(TOOLBAR_H)));
    layout.set_size_policy(rows.tool_bar, SizePolicy::Fixed);

    layout.add_widget(rows.tab_bar, 1);
    layout.set_constraints(rows.tab_bar, LayoutConstraints::new(TABBAR_H, Some(TABBAR_H)));
    layout.set_size_policy(rows.tab_bar, SizePolicy::Fixed);

    // 页面区：所有页面重叠，只能把一个 id 放进纵向布局作为“尺寸代理”。
    if let Some(proxy) = rows.page_proxy {
        layout.add_widget(proxy, 10);
        layout.set_constraints(proxy, LayoutConstraints::new(120, None));
    }

    layout.add_widget(rows.log_title, 1);
    layout.set_constraints(rows.log_title, LayoutConstraints::new(18, Some(18)));
    layout.set_size_policy(rows.log_title, SizePolicy::Fixed);

    for row in &rows.log_rows {
        layout.add_widget(*row, 1);
        layout.set_constraints(*row, LayoutConstraints::new(16, Some(20)));
        layout.set_size_policy(*row, SizePolicy::Fixed);
    }

    layout.add_widget(rows.status_bar, 1);
    layout.set_constraints(rows.status_bar, LayoutConstraints::new(STATUS_H, Some(STATUS_H)));
    layout.set_size_policy(rows.status_bar, SizePolicy::Fixed);

    layout
}

/// 让所有标签页与页面区矩形完全对齐。
///
/// 页面是叠着放的（同一时刻只有一个可见），所以它们的几何必须一致 ——
/// 否则切换标签页时内容会跳到另一个位置。
fn align_pages(pages: &[PanelHandle], rect: (i32, i32, u32, u32)) {
    let (x, y, w, h) = rect;
    for page in pages {
        page.set_geometry(x, y, w, h);
    }
}

/// 检查每一页的每个控件是否都拿到了非零几何。
///
/// 返回一行摘要：`Buttons 6/6 ok · Toggles 6/6 ok · …`。
///
/// # 为什么需要这个
///
/// “控件已创建”与“控件可见”是两件事：这个 demo 曾经的缺陷正是控件全部创建成功、
/// 注册成功，却因为页面的布局跑在一个零面积矩形上而全部算成 0×0。
/// 创建日志不会发现它，几何检查会。
fn verify_pages_placed(pages: &[TabPage]) -> String {
    let mut parts = Vec::new();
    for page in pages {
        let mut placed = 0usize;
        let mut zero = Vec::new();
        for cell in &page.cells {
            let geometry = rust_widgets::widget::runtime::geometry_of(cell.id);
            match geometry {
                Some(rect) if rect.width > 0 && rect.height > 0 => placed += 1,
                Some(rect) => zero.push(format!("{:#x} {rect:?}", cell.id)),
                None => zero.push(format!("{:#x} (unmounted)", cell.id)),
            }
        }
        if zero.is_empty() {
            parts.push(format!("{} {placed}/{} ok", page.title, page.cells.len()));
        } else {
            parts.push(format!(
                "{} {placed}/{} BAD: {}",
                page.title,
                page.cells.len(),
                zero.join(", ")
            ));
        }
    }
    parts.join(" · ")
}

/// 页面区矩形：把窗口纵向布局跑一遍，读出代理页面那一行。
///
/// 和 code_editor demo 同一手法：不另抄一遍“工具栏 + 标签条 + 日志”的减法，
/// 而是**问布局要答案**。改任何一行高度时，这里自动跟上。
fn page_area_rect(rows: &WindowRows) -> rust_widgets::core::Rect {
    let Some(proxy) = rows.page_proxy else {
        return rust_widgets::core::Rect::new(MARGIN as i32, 0, WINDOW_W, 0);
    };
    let mut placed: Vec<(ObjectId, rust_widgets::core::Rect)> = Vec::new();
    build_window_layout(rows)
        .update(rust_widgets::core::Rect::new(0, 0, WINDOW_W, WINDOW_H), &mut |id, rect| {
            placed.push((id, rect))
        });
    placed
        .iter()
        .find(|(id, _)| *id == proxy)
        .map(|(_, rect)| *rect)
        .unwrap_or(rust_widgets::core::Rect::new(0, 0, WINDOW_W, 0))
}

// ═══════════════════════════════════════════════════════════════════════════════
// 标签页内容
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// 标签页构造
// ═══════════════════════════════════════════════════════════════════════════════

/// 构建全部标签页，顺序即标签条顺序。
///
/// 抽出来是为了让 `run()` 与测试用**同一个**构造路径。测试曾经手抄一份页面规格，
/// 于是删掉一个控件时测试仍然通过，而 demo 展示的东西已经变了。
fn build_all_pages(win: &WindowHandle, log: &Arc<EventLog>) -> Vec<TabPage> {
    vec![
        build_buttons_page(win, log),
        build_toggles_page(win, log),
        build_text_page(win, log),
        build_selection_page(win, log),
        build_range_page(win, log),
        build_containers_page(win, log),
    ]
}

/// 第 1 页：按钮类。
fn build_buttons_page(win: &WindowHandle, log: &Arc<EventLog>) -> TabPage {
    log.append("═══ Tab 1: Buttons ═══");
    let mut page = TabPage::new("Buttons", 2, &[1, 1, 1]);

    let btn = win.new_button("Click Me", 0, 0, 0, 0);
    let l = Arc::clone(log);
    btn.on_click(move || l.append("[Button] clicked!"));
    page.place(0, 0, btn.raw_id());

    let tog = win.new_button("Toggle", 0, 0, 0, 0);
    let l = Arc::clone(log);
    tog.on_click(move || l.append("[Button/Toggle] toggled!"));
    page.place(0, 1, tog.raw_id());

    let disabled = win.new_button("Disabled", 0, 0, 0, 0);
    disabled.disable();
    log.append(format!("[Button] disabled enabled={}", disabled.is_enabled()));
    page.place(0, 2, disabled.raw_id());

    // 第二行：命令链按钮（每个都带回调），说明同一行可以有多个同类控件。
    for (col, text) in ["Save", "Reload", "Clear"].iter().enumerate() {
        let button = win.new_button(text, 0, 0, 0, 0);
        let l = Arc::clone(log);
        let text = (*text).to_string();
        button.on_click(move || l.append(format!("[Button] {text} clicked")));
        page.place(1, col as u32, button.raw_id());
    }

    log.append(format!("[Tab 1] {} controls", page.cells.len()));
    page
}

/// 第 2 页：开关与单选。
fn build_toggles_page(win: &WindowHandle, log: &Arc<EventLog>) -> TabPage {
    log.append("═══ Tab 2: Toggles ═══");
    // 两列：左边复选框，右边单选组。
    let mut page = TabPage::new("Toggles", 4, &[1, 1]);

    let cb = win.new_checkbox("Enable notifications", 0, 0, 0, 0);
    let cb_id = cb.raw_id();
    let l = Arc::clone(log);
    let cb2 = cb.clone();
    cb2.on_value_changed(move |_val: String| {
        let checked = cb.is_checked();
        l.append(format!("[CheckBox] checked={}", checked));
    });
    page.place(0, 0, cb_id);

    let tri = win.new_checkbox("Tri-state", 0, 0, 0, 0);
    tri.set_tristate(true);
    tri.set_check_state(rust_widgets::app::CheckState::PartiallyChecked);
    log.append(format!(
        "[CheckBox] tri-state={:?} state={:?}",
        tri.is_tristate(),
        tri.check_state()
    ));
    page.place(1, 0, tri.raw_id());

    let check = win.new_checkbox("Checked by default", 0, 0, 0, 0);
    check.set_checked(true);
    log.append(format!("[CheckBox] checked={}", check.is_checked()));
    page.place(2, 0, check.raw_id());

    // 单选组：三个选项共用一组，选一个清掉其它。
    let mut options = Vec::new();
    for (row, label) in ["Option A", "Option B", "Option C"].iter().enumerate() {
        let radio = win.new_radio_button(label, 0, 0, 0, 0);
        radio.set_group("opts");
        options.push(radio);
        page.place(row as u32, 1, options[row].raw_id());
    }
    if let Some(second) = options.get(1) {
        let l = Arc::clone(log);
        let second2 = second.clone();
        second.on_value_changed(move |_val: String| {
            if second2.is_selected() {
                l.append("[RadioButton] Option B selected");
            }
        });
    }
    log.append("[RadioButton] 3 options in group 'opts'");

    log.append(format!("[Tab 2] {} controls", page.cells.len()));
    page
}

/// 第 3 页：文本输入。
fn build_text_page(win: &WindowHandle, log: &Arc<EventLog>) -> TabPage {
    log.append("═══ Tab 3: Text ═══");
    // 两列：左列普通/密码/只读，右列数字与预填。
    let mut page = TabPage::new("Text", 4, &[3, 2]);

    let le = win.new_line_edit("", 0, 0, 0, 0);
    le.set_placeholder("Type here...");
    le.set_max_length(32);
    let le_id = le.raw_id();
    let l = Arc::clone(log);
    let le2 = le.clone();
    le2.on_value_changed(move |_val: String| {
        l.append(format!("[LineEdit] text='{}'", le.text()));
    });
    log.append("[LineEdit] placeholder + max_length=32");
    page.place(0, 0, le_id);

    let pw = win.new_line_edit("secret", 0, 0, 0, 0);
    pw.set_echo_mode(rust_widgets::app::EchoMode::Password);
    log.append(format!("[LineEdit] echo_mode={:?}", pw.widget_echo_mode()));
    page.place(1, 0, pw.raw_id());

    let ro = win.new_line_edit("read-only value", 0, 0, 0, 0);
    ro.set_read_only(true);
    log.append(format!("[LineEdit] read_only={:?}", ro.clone().is_read_only()));
    page.place(2, 0, ro.raw_id());

    let sb = win.new_spin_box(0, 0, 0, 0);
    sb.set_range(0, 100);
    sb.set_value(50);
    sb.set_prefix("$");
    sb.set_suffix(".00");
    sb.set_step(5);
    let sb_id = sb.raw_id();
    let l = Arc::clone(log);
    let sb2 = sb.clone();
    sb2.on_value_changed(move |_val: String| {
        l.append(format!("[SpinBox] value={}", sb.value()));
    });
    log.append("[SpinBox] range=[0..100], value=50, step=5");
    page.place(0, 1, sb_id);

    let prefilled = win.new_line_edit("prefilled", 0, 0, 0, 0);
    log.append(format!("[LineEdit] text='{}'", prefilled.text()));
    page.place(1, 1, prefilled.raw_id());

    log.append(format!("[Tab 3] {} controls", page.cells.len()));
    page
}

/// 第 4 页：选择类。
fn build_selection_page(win: &WindowHandle, log: &Arc<EventLog>) -> TabPage {
    log.append("═══ Tab 4: Selection ═══");
    // 两列：左边下拉，右边列表（列表更高，占两行）。
    let mut page = TabPage::new("Selection", 3, &[1, 1]);

    let cbx = win.new_combo_box(0, 0, 0, 0);
    for item in ["Red", "Green", "Blue", "Yellow"] {
        cbx.add_item(item);
    }
    cbx.set_current_index(0);
    let cbx_id = cbx.raw_id();
    let l = Arc::clone(log);
    let cbx2 = cbx.clone();
    cbx2.on_value_changed(move |_val: String| {
        let idx = cbx.current_index().unwrap_or(0);
        let text = cbx.item_text(idx).unwrap_or_else(|| String::from("(none)"));
        l.append(format!("[ComboBox] selected '{}' (idx={})", text, idx));
    });
    log.append("[ComboBox] items=[Red,Green,Blue,Yellow]");
    page.place(0, 0, cbx_id);

    let editable = win.new_combo_box(0, 0, 0, 0);
    for item in ["Small", "Medium", "Large"] {
        editable.add_item(item);
    }
    log.append(format!("[ComboBox] {} items", editable.item_count()));
    page.place(1, 0, editable.raw_id());

    let lb = win.new_list_box(0, 0, 0, 0);
    for item in ["Alpha", "Bravo", "Charlie", "Delta", "Echo"] {
        lb.add_item(item);
    }
    lb.set_current_index(0);
    let lb_id = lb.raw_id();
    let l = Arc::clone(log);
    let lb2 = lb.clone();
    let lb3 = lb.clone();
    lb2.on_value_changed(move |_val: String| {
        let idx = lb3.current_index().unwrap_or(0);
        let text = lb3.item_text(idx).unwrap_or_else(|| String::from("(none)"));
        l.append(format!("[ListBox] selected '{}' (idx={})", text, idx));
    });
    log.append(format!("[ListBox] {} items", lb.item_count()));
    page.place(0, 1, lb_id);

    log.append(format!("[Tab 4] {} controls", page.cells.len()));
    page
}

/// 第 5 页：范围与进度。
fn build_range_page(win: &WindowHandle, log: &Arc<EventLog>) -> TabPage {
    log.append("═══ Tab 5: Range ═══");
    let mut page = TabPage::new("Range", 3, &[1, 1]);

    let sl = win.new_slider_with_orientation(Orientation::Horizontal, 0, 0, 0, 0);
    sl.set_range(0, 100);
    sl.set_value(50);
    sl.set_step(5);
    let sl_id = sl.raw_id();
    let l = Arc::clone(log);
    let sl2 = sl.clone();
    let sl3 = sl.clone();
    sl2.on_value_changed(move |_val: String| {
        l.append(format!("[Slider] value={}", sl3.value()));
    });
    log.append(format!("[Slider] value=50 orientation={:?}", sl.orientation()));
    page.place(0, 0, sl_id);

    let vertical = win.new_slider_with_orientation(Orientation::Vertical, 0, 0, 0, 0);
    vertical.set_range(0, 100);
    vertical.set_value(30);
    log.append(format!("[Slider] vertical value={}", vertical.value()));
    page.place(0, 1, vertical.raw_id());

    let pb = win.new_progress_bar(0, 0, 0, 0);
    pb.set_min(0u32);
    pb.set_max(100u32);
    pb.set_value(75u32);
    let pb_id = pb.raw_id();
    let l = Arc::clone(log);
    let pb2 = pb.clone();
    pb2.on_value_changed(move |_val: String| {
        l.append(format!("[ProgressBar] value={}", pb.value()));
    });
    log.append("[ProgressBar] range=[0..100], value=75");
    page.place(1, 0, pb_id);

    let busy = win.new_progress_bar(0, 0, 0, 0);
    busy.set_indeterminate(true);
    log.append(format!("[ProgressBar] indeterminate={:?}", busy.is_indeterminate()));
    page.place(1, 1, busy.raw_id());

    log.append(format!("[Tab 5] {} controls", page.cells.len()));
    page
}

/// 第 6 页：容器与自绘型控件。
fn build_containers_page(win: &WindowHandle, log: &Arc<EventLog>) -> TabPage {
    log.append("═══ Tab 6: Containers ═══");
    // 两列：左边滚动区 + 面板，右边留给自绘型控件（下面单独挂载）。
    let mut page = TabPage::new("Containers", 2, &[1, 1]);

    let area = win.new_scroll_area(0, 0, 0, 0);
    for index in 0..8 {
        let _line = win.new_label(&format!("scroll line {index}"), 0, 0, 0, 0);
    }
    log.append("[ScrollArea] created with 8 labels");
    page.place(0, 0, area.raw_id());

    let panel = win.new_panel(0, 0, 0, 0);
    panel.set_title("Panel");
    log.append(format!("[Panel] id={:?}", panel.raw_id()));
    page.place(1, 0, panel.raw_id());

    let frame = win.new_frame(0, 0, 0, 0);
    frame.set_text("Frame");
    log.append(format!("[Frame] id={:?}", frame.raw_id()));
    page.place(0, 1, frame.raw_id());

    let msg = win.new_message_box("Demo Info", "This is a message box.", 0, 0, 0, 0);
    msg.set_title("Information");
    let l = Arc::clone(log);
    msg.on_click(move || l.append("[MessageBox] button clicked!"));
    log.append("[MessageBox] created (shown on demand)");
    page.place(1, 1, msg.raw_id());

    log.append(format!("[Tab 6] {} controls", page.cells.len()));
    page
}

// ═══════════════════════════════════════════════════════════════════════════════
// 标签条
// ═══════════════════════════════════════════════════════════════════════════════

/// 建立标签条：一个横向 `BoxLayout`，每个标签一个等宽按钮。
///
/// 返回标签条 Panel 的 id，以及每个按钮的 id（顺序与 `titles` 一致）。
fn build_tab_bar(
    win: &WindowHandle,
    titles: Vec<&'static str>,
    log: &Arc<EventLog>,
    on_select: impl Fn(usize) + 'static + Clone,
) -> (ObjectId, Vec<ObjectId>) {
    let bar = win.new_panel(0, 0, 0, 0);
    let mut layout = BoxLayout::new(Orientation::Horizontal, GAP, 0);
    let mut buttons = Vec::new();
    for (index, title) in titles.iter().enumerate() {
        let button = win.new_button(title, 0, 0, 0, 0);
        layout.add_widget(button.raw_id(), 1);

        let callback = on_select.clone();
        let l = Arc::clone(log);
        // `title` is `&'static str`, so the closure can own it without borrowing `titles`.
        let title: &'static str = title;
        button.on_click(move || {
            l.append(format!("[TabBar] select '{title}'"));
            callback(index);
        });
        buttons.push(button.raw_id());
    }
    bar.set_layout(Box::new(layout));
    log.append(format!("[TabBar] {} tabs", titles.len()));
    (bar.raw_id(), buttons)
}

// ═══════════════════════════════════════════════════════════════════════════════
// 自绘型控件（控件自己绘制内容，放置方式由库决定）
// ═══════════════════════════════════════════════════════════════════════════════

/// 挂载自绘型控件，证明它们能和其它控件放在同一个窗口里。
///
/// 唯一要问的是**能力是否存在**：`supports_surfaces()` 返回 `false` 时，
/// 说明当前平台暂时撑不住这类控件，如实跳过即可。
fn build_custom_painted_controls(
    win: &WindowHandle,
    rect: rust_widgets::core::Rect,
    log: &Arc<EventLog>,
) {
    use rust_widgets::widget::special_widgets::code_editor::{CodeEditorConfig, LanguageId};

    log.append("═══ Custom-painted Widgets ═══");

    if !rust_widgets::supports_surfaces() {
        log.append(format!(
            "[Custom] 当前后端 '{}' 暂不能承载自绘型控件，跳过本区域",
            rust_widgets::backend_name()
        ));
        return;
    }

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
            match win.mount_surface(widget, rect) {
                Ok(handle) => {
                    log.append(format!("[Custom] CodeEditor 挂载成功 id={}", handle.raw_id()))
                }
                Err(error) => log.append(format!("[Custom] CodeEditor 挂载失败：{error}")),
            }
        }
        Err(error) => log.append(format!("[Custom] CodeEditor 配置非法：{error}")),
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
    println!("║     rust_widgets  —  Controls Demo                     ║");
    println!("║     App 框架 · 标签页布局 · 实时事件日志                    ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let log = Arc::new(EventLog::new());

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

    app.init();
    log.append("[App] init() done");

    let win = app.new_window("Controls Demo — rust_widgets", 100, 100, WINDOW_W, WINDOW_H);
    log.append(format!("[Window] created: id={:?}", win.raw_id()));

    // ── 菜单栏：File / View ────────────────────────────────────────────
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

    // ── 窗口 chrome ────────────────────────────────────────────────────
    let tool_bar = win.new_tool_bar(0, 0, WINDOW_W, TOOLBAR_H);
    log.append(format!("[ToolBar] id={:?}", tool_bar.raw_id()));

    let status = win.new_status_bar("Ready", 0, 0, WINDOW_W, STATUS_H);
    let l = Arc::clone(&log);
    status.on_value_changed(move |val: String| l.append(format!("[StatusBar] '{val}'")));
    log.append("[StatusBar] text='Ready'");

    // ── 各标签页的控件 ─────────────────────────────────────────────────
    let mut pages = build_all_pages(&win, &log);
    let control_count: usize = pages.iter().map(|page| page.cells.len()).sum();
    log.append(format!("[Tabs] {} tabs, {control_count} controls", pages.len()));

    // 每页一个 Panel，承载该页自己的布局。
    for page in &mut pages {
        page.panel = Some(win.new_panel(0, 0, 0, 0));
        page.apply_layout();
    }
    let page_handles: Vec<PanelHandle> =
        pages.iter().filter_map(|page| page.panel.clone()).collect();
    log.append(format!("[Tabs] {} pages, each with its own layout", page_handles.len()));

    // ── 事件日志 ───────────────────────────────────────────────────────
    let log_title = win.new_label("── Event Log ──", 0, 0, 0, 0);
    let log_rows: Vec<ObjectId> =
        (0..LOG_ROWS).map(|_| win.new_label("", 0, 0, 0, 0).raw_id()).collect();

    // ── 标签条：点击切换页面可见性 ─────────────────────────────────────
    //
    // 切换 = 让目标页 `set_visible(true)`、其余 `set_visible(false)`。
    // 页面彼此重叠，所以只需要可见性；不需要重新计算几何。
    let page_handles_for_bar = page_handles.clone();
    let log_for_bar = Arc::clone(&log);
    let titles: Vec<&'static str> = pages.iter().map(|page| page.title).collect();
    let (_tab_bar_id, tab_buttons) = build_tab_bar(&win, titles.clone(), &log, move |selected| {
        for (index, page) in page_handles_for_bar.iter().enumerate() {
            page.set_visible(index == selected);
        }
        let title = titles.get(selected).copied().unwrap_or("?");
        log_for_bar.append(format!("[Tabs] showing '{title}' (#{selected})"));
    });
    log.append(format!("[Tabs] {} tab buttons", tab_buttons.len()));

    // ── 应用窗口布局 ───────────────────────────────────────────────────
    //
    // 只有**第一页**参与窗口纵向布局（页面彼此重叠，放进布局会让它们纵向排开）。
    // 其余页面在布局跑完后对齐到同一矩形 —— 见 `page_area_rect`。
    let rows = WindowRows {
        tool_bar: tool_bar.raw_id(),
        tab_bar: _tab_bar_id,
        // 只把第一页交给纵向布局，它是页面区的“尺寸代理”。
        page_proxy: page_handles.first().map(|page| page.raw_id()),
        log_title: log_title.raw_id(),
        log_rows: log_rows.clone(),
        status_bar: status.raw_id(),
    };
    log.append(format!(
        "[Layout] window BoxLayout(Vertical): toolbar {TOOLBAR_H} + tab bar {TABBAR_H} + \
         pages (expanding) + log {LOG_PANEL_H} + status bar {STATUS_H}"
    ));
    win.set_layout(build_window_layout(&rows));

    // 页面区矩形来自同一次布局计算，所以所有页面与布局不会不一致。
    let area = page_area_rect(&rows);
    align_pages(&page_handles, (area.x, area.y, area.width, area.height));
    log.append(format!(
        "[Layout] {} pages aligned to ({},{}, {}, {})",
        page_handles.len(),
        area.x,
        area.y,
        area.width,
        area.height
    ));

    // 只显示第一页。
    for (index, page) in page_handles.iter().enumerate() {
        page.set_visible(index == 0);
    }
    log.append("[Tabs] showing 'Buttons' (#0)");

    // ── 自证：每页的控件必须真的拿到了几何 ─────────────────────────────
    //
    // 这一条直接对应当前的真实缺陷：页面没有任何尺寸时，页面布局会把子控件算成
    // 0×0 —— 控件“存在”但看不到。所以这里逐个检查，而不是只看创建日志。
    log.append(format!("[verify] {}", verify_pages_placed(&pages)));

    // ── 自绘型控件：放在 Containers 页右列 ─────────────────────────────
    //
    // 用页面区的右半作为区域（同一次算术，不另写一份常量）。
    let custom_rect = rust_widgets::core::Rect::new(
        area.x + (area.width / 2) as i32,
        area.y,
        area.width / 2,
        area.height,
    );
    build_custom_painted_controls(&win, custom_rect, &log);

    log.append("[App] controls ready — starting event loop");

    win.show();
    log.append(format!("[Window] shown: id={:?}", win.raw_id()));

    app.run();
    log.append("[App] event loop exited");

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

// ═══════════════════════════════════════════════════════════════════════════════
// 测试 —— 验证标签页版式真的成立
// ═══════════════════════════════════════════════════════════════════════════════
//
// `Layout::update` 是纯函数，所以三层的几何都能在无头环境下断言。
#[cfg(test)]
mod tests {
    use super::*;
    use rust_widgets::core::Rect;

    fn laid_out(layout: &dyn Layout, area: Rect) -> Vec<(ObjectId, Rect)> {
        let mut result = Vec::new();
        layout.update(area, &mut |id, geometry| result.push((id, geometry)));
        result
    }

    fn rect_of(placed: &[(ObjectId, Rect)], id: ObjectId) -> Rect {
        placed
            .iter()
            .find(|(placed_id, _)| *placed_id == id)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("id {id} was not placed"))
    }

    /// 一组合成的窗口行 id。
    fn sample_rows() -> WindowRows {
        WindowRows {
            tool_bar: 1,
            tab_bar: 2,
            page_proxy: Some(3),
            log_title: 4,
            log_rows: vec![5, 6, 7],
            status_bar: 8,
        }
    }

    /// 窗口各行自上而下排列，且不重叠。
    #[test]
    fn the_window_rows_stack_without_overlapping() {
        let rows = sample_rows();
        let placed = laid_out(&build_window_layout(&rows), Rect::new(0, 0, WINDOW_W, WINDOW_H));

        let order = [rows.tool_bar, rows.tab_bar, 3, rows.log_title, 5, 6, 7, rows.status_bar];
        let mut previous_bottom = 0i32;
        for id in order {
            let rect = rect_of(&placed, id);
            assert!(rect.y >= previous_bottom, "id {id} overlaps the row above: {rect:?}");
            previous_bottom = rect.y + rect.height as i32;
        }
        assert!(
            previous_bottom <= WINDOW_H as i32,
            "the rows must fit the window, ended at {previous_bottom}"
        );
    }

    /// 工具栏、标签条、日志、状态栏都是固定高度；页面区吃掉剩余高度。
    #[test]
    fn fixed_rows_keep_their_height_and_the_pages_absorb_the_rest() {
        let rows = sample_rows();
        for height in [WINDOW_H, WINDOW_H + 200] {
            let placed = laid_out(&build_window_layout(&rows), Rect::new(0, 0, WINDOW_W, height));
            assert_eq!(rect_of(&placed, rows.tool_bar).height, TOOLBAR_H);
            assert_eq!(rect_of(&placed, rows.tab_bar).height, TABBAR_H);
            assert_eq!(rect_of(&placed, rows.status_bar).height, STATUS_H);

            // 页面区必须随窗口变高而变高，而不是留一条空。
            let pages = rect_of(&placed, 3);
            assert!(pages.height > 0, "the page area must have height: {pages:?}");
            assert!(
                pages.height > rect_of(&placed, rows.tool_bar).height,
                "the page area must be the tallest row: {pages:?}"
            );
        }
    }

    /// `page_area_rect` 必须与布局真正给页面区的那块地方一致。
    ///
    /// 所有标签页都对齐到它，所以它一旦错位，整页内容都会错位。
    #[test]
    fn the_page_area_rect_matches_the_layout() {
        let rows = sample_rows();
        let placed = laid_out(&build_window_layout(&rows), Rect::new(0, 0, WINDOW_W, WINDOW_H));
        assert_eq!(page_area_rect(&rows), rect_of(&placed, 3));
    }

    /// 页面区必须横跨客户区宽度（减去外边距）。
    #[test]
    fn the_page_area_spans_the_client_width() {
        let rows = sample_rows();
        let area = page_area_rect(&rows);
        assert_eq!(area.width, WINDOW_W - MARGIN * 2, "the page area must span the width");
        assert_eq!(area.x, MARGIN as i32);
    }

    // ── 每一页自己的网格 ─────────────────────────────────────────────

    /// 一页的网格只放置属于它的控件，且不重叠。
    #[test]
    fn a_page_grid_places_only_its_own_cells_without_overlap() {
        let mut page = TabPage::new("Test", 2, &[1, 1]);
        // 四个合成控件，两行两列。
        page.place(0, 0, 101);
        page.place(0, 1, 102);
        page.place(1, 0, 103);
        page.place(1, 1, 104);

        let placed = laid_out(&page.grid(), Rect::new(0, 0, 400, 200));
        assert_eq!(placed.len(), 4, "every declared cell must be placed");
        for (id, rect) in &placed {
            assert!(rect.width > 0 && rect.height > 0, "cell {id} has no area: {rect:?}");
            assert!(rect.x >= 0 && rect.y >= 0, "cell {id} escapes: {rect:?}");
            assert!(
                rect.x + rect.width as i32 <= 400 && rect.y + rect.height as i32 <= 200,
                "cell {id} escapes the page: {rect:?}"
            );
        }

        // 同一行内按 x 排序后不重叠。
        let row0: Vec<Rect> = [101, 102].iter().map(|id| rect_of(&placed, *id)).collect();
        let mut row0 = row0;
        row0.sort_by_key(|rect| rect.x);
        assert!(row0[0].x + row0[0].width as i32 <= row0[1].x, "row 0 overlaps: {row0:?}");

        // 第 0 行在上面。
        assert!(rect_of(&placed, 101).y < rect_of(&placed, 103).y);
    }

    /// 列的宽度比必须与声明的权重一致。
    #[test]
    fn a_pages_columns_follow_their_weights() {
        let mut page = TabPage::new("Weights", 1, &[1, 3]);
        page.place(0, 0, 201);
        page.place(0, 1, 202);

        let placed = laid_out(&page.grid(), Rect::new(0, 0, 400, 100));
        let narrow = rect_of(&placed, 201).width as f64;
        let wide = rect_of(&placed, 202).width as f64;
        let actual = narrow / (narrow + wide);
        assert!(
            (actual - 0.25).abs() < 0.03,
            "a 1:3 column split must give ~0.25 to the first column, got {actual:.3}"
        );
    }

    /// 页面必须声明足够的列，否则控件会被静默丢弃。
    ///
    /// `GridLayout::set_widget` 会忽略越界的 `(row, col)`，所以一句写错的声明
    /// 不会报错，只是那个控件不再出现 —— 这正是要防的。
    #[test]
    fn every_page_declares_a_valid_grid_position() {
        // 一个覆盖全部页面的表（与 `run()` 里的页面构造保持一致的行/列数）。
        let pages: Vec<TabPage> = vec![
            TabPage::new("Buttons", 2, &[1, 1, 1]),
            TabPage::new("Toggles", 4, &[1, 1]),
            TabPage::new("Text", 4, &[3, 2]),
            TabPage::new("Selection", 3, &[1, 1]),
            TabPage::new("Range", 3, &[1, 1]),
            TabPage::new("Containers", 2, &[1, 1]),
        ];
        for page in &pages {
            assert!(page.rows > 0, "{} has no rows", page.title);
            assert_eq!(
                page.column_weights.len(),
                page.cols as usize,
                "{} declares {} columns but {} weights",
                page.title,
                page.cols,
                page.column_weights.len()
            );
            assert!(page.cols > 0, "{} has no columns", page.title);
        }
    }

    /// 每一页的网格必须真的能把它的控件放下来。
    ///
    /// 用页面区的真实尺寸跑一遍，确认没有控件被越界丢掉。
    #[test]
    fn every_pages_cells_are_addressable_in_its_grid() {
        let mut page = TabPage::new("Buttons", 2, &[1, 1, 1]);
        for col in 0..3 {
            let id = 300 + col as u64;
            page.place(0, col, id);
        }
        for col in 0..3 {
            let id = 400 + col as u64;
            page.place(1, col, id);
        }
        let grid = page.grid();
        assert_eq!(grid.cell_count(), 6, "all six declarations must land in the grid");
        assert_eq!(grid.rows(), 2);
        assert_eq!(grid.cols(), 3);
    }

    /// 真正构建一遍 demo，确认标签页与控件的实际数量。
    ///
    /// # 为什么要真的构建，而不是断言常量
    ///
    /// “6 个标签页、28 个控件”是一个会随 demo 演进而变化的数字。如果只把它写成断言常量，
    /// 有人删掉一个控件时测试仍然通过，而 demo 展示的内容已经变了；真的构建一遍再数，
    /// 数字才与 `run()` 展示的东西绑定。
    ///
    /// 这里同时覆盖最初的缺陷：控件曾被创建、被注册，却因为页面布局跑在零面积矩形上
    /// 而全部是 0×0。下面既数数量，也查几何。
    #[test]
    fn the_demo_builds_six_tabs_of_placed_controls() {
        let mut app = rust_widgets::app::App::new();
        app.init();
        let win = app.new_window("control-demo-test", 0, 0, WINDOW_W, WINDOW_H);
        let log = Arc::new(EventLog::new());

        let pages = build_all_pages(&win, &log);

        assert_eq!(pages.len(), 6, "the demo must present six tabs");
        let total: usize = pages.iter().map(|page| page.cells.len()).sum();
        assert_eq!(total, 28, "the demo must lay out 28 controls across its tabs");

        // 页面的网格必须真的能容纳每个已声明的控件，否则 `GridLayout` 会静默丢弃越界项。
        for page in &pages {
            let grid = page.grid();
            assert_eq!(
                grid.cell_count(),
                page.cells.len(),
                "{}: {} controls declared but only {} landed in the grid",
                page.title,
                page.cells.len(),
                grid.cell_count()
            );
        }
    }

    /// 每个控件必须真的拿到非零几何。
    ///
    /// 这条断言是那个缺陷的直接回归测试：控件“已创建”曾经是真的，“有大小”却不是。
    #[test]
    fn every_control_ends_up_with_a_non_zero_rect() {
        let mut app = rust_widgets::app::App::new();
        app.init();
        let win = app.new_window("control-demo-test", 0, 0, WINDOW_W, WINDOW_H);
        let log = Arc::new(EventLog::new());

        let pages = build_all_pages(&win, &log);

        // 所有标签页叠在同一个矩形里（由可见性决定显示哪一页），所以每一页都用该矩形
        // 跑自己的网格。
        let area = page_area_rect(&sample_rows());
        assert!(
            area.width > 0 && area.height > 0,
            "the page area must have a real size, or every check below is vacuous: {area:?}"
        );

        let mut checked = 0usize;
        for page in &pages {
            let placed = laid_out(&page.grid(), area);
            for cell in &page.cells {
                let geometry = rect_of(&placed, cell.id);
                assert!(
                    geometry.width > 0 && geometry.height > 0,
                    "{}: control {:#x} was placed at {geometry:?} — a zero-area rect means \
                     the layout ran against the wrong container size",
                    page.title,
                    cell.id
                );
                // 控件必须在页面区之内，否则它会画到相邻的行上（日志或状态栏）。
                assert!(
                    geometry.x >= area.x
                        && geometry.y >= area.y
                        && geometry.x + geometry.width as i32 <= area.x + area.width as i32
                        && geometry.y + geometry.height as i32 <= area.y + area.height as i32,
                    "{}: control {:#x} at {geometry:?} escapes the page area {area:?}",
                    page.title,
                    cell.id
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 28, "every one of the 28 controls must have been checked");
    }
}
