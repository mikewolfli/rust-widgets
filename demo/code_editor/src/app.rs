//! Code Editor Demo — `CodeEditor` 挂载进窗口，并带完整菜单栏 + 工具栏 + 项目树。
//!
//! # 这个 demo 演示什么
//!
//! 1. **自绘型控件挂载**——`CodeEditor` 与项目树都自己负责绘制内容，所以没有对应的
//!    `new_*` 工厂方法，用 `WindowHandle::mount_surface` 交给窗口托管。
//!    窗口**用什么**承载它由 `src/platform/` 决定，本 demo 不需要知道，
//!    也不写任何 OS 分支。
//! 2. **菜单栏驱动控件**——菜单项激活后从菜单事件队列取出，转成 [`Command`]，
//!    通过 [`CustomWidgetHandle::update`] 打到编辑器上。菜单与编辑器之间只通过
//!    这条平台无关的通道通信。
//! 3. **工具栏 + 状态栏**——工具栏提供高频操作按钮，状态栏实时显示光标位置。
//! 4. **左右分栏宽度由布局决定**——项目树与编辑器的宽度比是**权重**，不是像素：
//!    见 [`SPLIT_WEIGHTS`] 与 [`build_window_layout`]。
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
use rust_widgets::core::{ObjectId, Orientation, Rect};
use rust_widgets::layout::{BoxLayout, Layout, LayoutConstraints, SizePolicy};
use rust_widgets::widget::special_widgets::code_editor::{
    CodeEditor, CodeEditorConfig, DiagnosticMarker, LanguageId, MarkerSeverity,
};
use rust_widgets::widget::view_widgets::tree_view::{TreeView, VecTreeModel};
use rust_widgets::widget::Widget;
use std::sync::atomic::{AtomicBool, Ordering};
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

const WINDOW_W: u32 = 1100;
const WINDOW_H: u32 = 720;
/// 菜单栏高度。
const MENU_H: u32 = 26;
/// 工具栏高度：必须 >= MIN_TOUCH_TARGET(28)，兼顾触屏。
const TOOLBAR_H: u32 = 36;
/// 状态栏高度。
const STATUS_H: u32 = 26;
/// 工具栏按钮条与下方内容之间的间距。
const STRIP_GAP: u32 = 2;
/// 项目树与编辑器之间的间距。
const SPLIT_GAP: u32 = 4;

/// 左右分栏的宽度**权重**（项目树 : 编辑器）。
///
/// # 为什么是权重而不是像素
///
/// 写死「项目树 240px」会有两个后果：窗口变宽时多出来的宽度全归编辑器，
/// 树永远 240px（比例越来越窄）；窗口变窄时树先把编辑器挤没。用权重表达后，
/// 两者按同一比例伸缩，且**窗口一产生就正确**——这正是本 demo 之前的问题：
/// 编辑器独占了整个宽度，根本没有分栏。
///
/// `1 : 4` 对应 1100px 窗口下的 220 : 880 —— 文档项目的常规比例。
const SPLIT_WEIGHTS: (u32, u32) = (1, 4);
/// 项目树的最小宽度：窄于它目录名就会被截断到不可读。
const TREE_MIN_W: u32 = 140;
/// 编辑器的最小宽度：窄于它一行代码都放不下。
const EDITOR_MIN_W: u32 = 320;

/// 分栏行在窗口纵向布局里的**占位 id**。
///
/// 窗口的纵向布局需要一个 id 来表示“中间那一行”，但那一行里并没有控件 —— 两栏是
/// 横向切开的。用一个不会与真实控件冲突的保留 id 占位即可：
/// 布局会对它调一次 `set_widget_geometry`，而对一个不存在的 id 那是无操作。
///
/// 之所以不另建一个 `Panel` 来占位：`PanelHandle::set_geometry` 会把几何记到它自己的
/// 表里，而窗口布局走的是 `set_widget_geometry`，两者不互通 —— 面板占位会馁化到
/// “首次手动设一次几何”，窗口后续缩放就不会再驱动分栏了。
const SPLIT_ROW_ID: ObjectId = u64::MAX;

/// 组装窗口的纵向布局：工具栏（固定）→ 分栏（吃掉剩余高度）→ 状态栏（固定）。
///
/// # 为什么用布局而不是 `editor_rect()` 手算
///
/// 手算 `WINDOW_H - MENU_H - TOOLBAR_H - STATUS_H` 的问题不在算术本身，而在**它会
/// 漂移**：窗口高度、工具栏高度、状态栏高度三者中任何一个改了，这行减法都可能
/// 忘记同步，而症状只是编辑器底部被状态栏盖住几像素。交给 `BoxLayout` 后，“谁在
/// 上、谁在下、谁吃掉剩余空间”由图表达一次，几何是算出来的而不是抄出来的。
fn build_window_layout(
    tool_bar_id: ObjectId,
    split_id: ObjectId,
    status_bar_id: ObjectId,
) -> BoxLayout {
    let mut layout = BoxLayout::new(Orientation::Vertical, STRIP_GAP, 0);

    // 工具栏与状态栏高度固定；中间那行吃掉剩下的一切。
    layout.add_widget(tool_bar_id, 1);
    layout.set_constraints(tool_bar_id, LayoutConstraints::new(TOOLBAR_H, Some(TOOLBAR_H)));
    layout.set_size_policy(tool_bar_id, SizePolicy::Fixed);

    layout.add_widget(split_id, 10);
    layout.set_constraints(split_id, LayoutConstraints::new(120, None));

    layout.add_widget(status_bar_id, 1);
    layout.set_constraints(status_bar_id, LayoutConstraints::new(STATUS_H, Some(STATUS_H)));
    layout.set_size_policy(status_bar_id, SizePolicy::Fixed);

    layout
}

/// 分栏内部的横向布局：项目树 | 编辑器，宽度按 [`SPLIT_WEIGHTS`] 分配。
///
/// 这是本 demo 的“宽度比例”定义处：比例只在这里出现一次，
/// 项目树与编辑器的初始矩形也从它推算（见 [`pane_rects`]），所以两者不可能不一致。
fn build_split_layout(tree_id: ObjectId, editor_id: ObjectId) -> BoxLayout {
    let mut split = BoxLayout::new(Orientation::Horizontal, SPLIT_GAP, 0);

    split.add_widget(tree_id, SPLIT_WEIGHTS.0);
    // 两栏都能被压缩，但不会窄到不可用：`min` 是各自的可用下限，
    // 空间不够时 `BoxLayout` 会按比例摊派而不是让某一栏溢出窗口。
    split.set_constraints(tree_id, LayoutConstraints::new(TREE_MIN_W, None));

    split.add_widget(editor_id, SPLIT_WEIGHTS.1);
    split.set_constraints(editor_id, LayoutConstraints::new(EDITOR_MIN_W, None));

    split
}

/// 把两栏放进分栏那一行的矩形里。
///
/// # 为什么不挂在某个容器上
///
/// 窗口的纵向布局已经把“分栏那一行”算出来了，但布局项只能是控件 id，
/// 它无法顺便把一个**子布局**也应用上。所以这里用同一份矩形把两栏切开 ——
/// 矩形的唯一来源是 [`split_row_rect_in`]，它从窗口布局输出里读回分栏行，
/// 所以两层不会不一致。
fn apply_split(
    split: &BoxLayout,
    rect: Rect,
    tree: &CustomWidgetHandle,
    editor: &CustomWidgetHandle,
) {
    let tree_id = tree.raw_id();
    let editor_id = editor.raw_id();
    split.update(rect, &mut |id, geometry| {
        if id == tree_id {
            tree.set_geometry(geometry);
        } else if id == editor_id {
            editor.set_geometry(geometry);
        }
    });
}

/// 分栏那一行的矩形，从窗口纵向布局的输出里读回。
///
/// 传入的是 `build_window_layout` 用的同三个 id，所以这里的答案就是窗口布局
/// 实际会给分栏行的那块地方 —— 不需要再抄一遍“工具栏 + 间距”的减法。
fn split_row_rect_in(tool_bar_id: ObjectId, split_id: ObjectId, status_bar_id: ObjectId) -> Rect {
    let mut placed: Vec<(ObjectId, Rect)> = Vec::new();
    build_window_layout(tool_bar_id, split_id, status_bar_id)
        .update(Rect::new(0, 0, WINDOW_W, WINDOW_H), &mut |id, rect| placed.push((id, rect)));
    placed.iter().find(|(id, _)| *id == split_id).map(|(_, rect)| *rect).unwrap_or(Rect::new(
        0,
        MENU_H as i32 + TOOLBAR_H as i32,
        WINDOW_W,
        split_row_height(),
    ))
}

/// 分栏那一行占据的高度：客户区去掉工具栏与状态栏，以及两条间距。
fn split_row_height() -> u32 {
    WINDOW_H.saturating_sub(TOOLBAR_H + STATUS_H + STRIP_GAP * 2)
}

/// 分栏内两栏的初始矩形，与 [`build_split_layout`] 的结果一致。
///
/// 挂载自绘型控件必须给一个矩形（`mount_surface` 的签名如此），所以这里按同一个
/// 横向布局算一份出来 —— 而不是自己再写一遍比例算术。布局随后会把几何修正到与
/// 真实分栏一致；两者同源，所以首帧不会跳动。
fn pane_rects() -> (Rect, Rect) {
    let row = split_row_rect_in(1, 2, 3);

    // 用一个临时布局跑一遍即可拿到两栏的宽度；id 只是占位，不指向真实控件。
    let mut placed: Vec<(ObjectId, Rect)> = Vec::new();
    build_split_layout(1, 2).update(row, &mut |id, rect| placed.push((id, rect)));

    let take = |id: ObjectId| -> Rect {
        placed
            .iter()
            .find(|(placed_id, _)| *placed_id == id)
            .map(|(_, rect)| *rect)
            .unwrap_or(Rect::new(row.x, row.y, TREE_MIN_W, row.height))
    };
    (take(1), take(2))
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
///
/// 矩形由调用方传入（即分栏算出的编辑器栏），不再自己算 —— 几何只有一个来源。
fn build_editor(rect: Rect) -> CodeEditor {
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
// 项目树
// ═══════════════════════════════════════════════════════════════════════════════

/// 项目树的示例目录结构。
///
/// 用缩进表达层级：树模型暴露的是**可见行序列**（`node_path`），所以层级要由文本
/// 自己携带。这里用两个空格一级，与编辑器里 `SAMPLE_RUST` 的结构对应。
const PROJECT_TREE: &[&str] = &[
    "fibonacci",
    "  src",
    "    main.rs",
    "    fib.rs",
    "    lib.rs",
    "  tests",
    "    fib_test.rs",
    "  examples",
    "    bench.rs",
    "  Cargo.toml",
    "  README.md",
];

/// 建立项目树，并用 `VecTreeModel` 绑定内容。
///
/// 树控件与编辑器一样是自绘型：它的内容来自模型（`TreeModel`），而不是靠
/// 调用方逐个摆节点。所以这个函数只做两件事：装数据、交给窗口托管。
fn build_project_tree(rect: Rect, log: &Arc<EventLog>) -> TreeView {
    let rows: Vec<String> = PROJECT_TREE.iter().map(|row| (*row).to_string()).collect();
    let mut tree = TreeView::new(rect);
    tree.set_model(Arc::new(VecTreeModel::new(rows)));
    log.append(format!("[TreeView] {} 行（含目录缩进）", tree.node_count()));
    tree
}

// ═══════════════════════════════════════════════════════════════════════════════
// 菜单栏
// ═══════════════════════════════════════════════════════════════════════════════

/// 单个菜单项：命令 + 触发后要投递到的槽位。
///
/// `Clone` 是因为轮询线程要持有自己的一份：这些绑定在**主线程**构造（绑定菜单项需要
/// 平台 API），而轮询发生在后台线程，两者不能共享 `&` 借用。`Command` 本身是 `Copy`，
/// 所以克隆是廉价的。
#[derive(Clone)]
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
///
/// 返回工具栏的 id，供窗口布局把它排在编辑器的上方。
fn build_tool_bar(
    win: &rust_widgets::app::WindowHandle,
    editor: &CustomWidgetHandle,
    log: &Arc<EventLog>,
) -> ObjectId {
    let bar = win.new_tool_bar(0, MENU_H as i32, WINDOW_W, TOOLBAR_H);
    log.append(format!("[ToolBar] created id={}", bar.raw_id()));

    // The button strip is laid out left-to-right here rather than by a layout manager:
    // each button's width is a designed constant (a 2 px separator is visible as a gap,
    // not a button), and `ToolBarHandle` has no `set_layout`. The strip itself is a
    // fixed-height row in the window layout, so its vertical position is not hand-placed.
    let mut x = 6i32;
    let top = MENU_H as i32;
    let mut button_count = 0usize;
    for (entry, width) in TOOLBAR_LAYOUT {
        match entry {
            None => {
                x += width;
            }
            Some(command) => {
                let button =
                    win.new_button(command.label(), x, top + 4, *width as u32, TOOLBAR_H - 8);
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
    bar.raw_id()
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
    if !rust_widgets::supports_surfaces() {
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

    // ── 分栏几何：项目树 | 编辑器（比例来自 `SPLIT_WEIGHTS`）──────────────
    let (tree_rect, rect) = pane_rects();

    // ── 挂载项目树 ──────────────────────────────────────────────────────
    let tree_box: Box<dyn Widget> = Box::new(build_project_tree(tree_rect, &log));
    let tree = match win.mount_surface(tree_box, tree_rect) {
        Ok(handle) => {
            log.append(format!(
                "[Mount] ProjectTree id={} rect=({},{}, {}, {})",
                handle.raw_id(),
                tree_rect.x,
                tree_rect.y,
                tree_rect.width,
                tree_rect.height
            ));
            handle
        }
        Err(error) => {
            eprintln!("错误：项目树挂载失败：{error}");
            std::process::exit(1);
        }
    };

    // ── 挂载编辑器 ──────────────────────────────────────────────────────
    let editor_box: Box<dyn Widget> = Box::new(build_editor(rect));
    let editor = match win.mount_surface(editor_box, rect) {
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
    let tool_bar_id = build_tool_bar(&win, &editor, &log);

    // 状态栏横跨整个窗口，而不是只对齐编辑器：它报告的是整个应用的状态（光标位置、
    // 已处理事件数），不是某一栏的。它是窗口布局的一行，因此天然占满宽度。
    let status = win.new_status_bar(
        &commands::status_line(&editor).unwrap_or_default(),
        0,
        (WINDOW_H - STATUS_H) as i32,
        WINDOW_W,
        STATUS_H,
    );
    log.append(format!("[StatusBar] created id={}", status.raw_id()));

    // ── 应用窗口布局 ────────────────────────────────────────────────────
    //
    // 两层：外层纵向分三行（工具栏 / 分栏 / 状态栏），分栏内部再横向分成
    // 项目树与编辑器。两者由 [`apply_layout`] 一次算完，避免“只应用了一层”。
    // **宽度比例只在 `build_split_layout` 里出现一次**，所以两栏不会不一致，
    // 也不会像本轮之前那样让编辑器独占整个宽度。
    apply_layout(&win, tool_bar_id, status.raw_id(), &tree, &editor, &log);

    win.show();
    log.append("[Window] shown");

    print_usage();
    run_loop(&app, &bindings, &editor, &log);
}

/// 一次性应用两层布局：窗口三行 + 分栏两栏。
///
/// # 为什么要封装
///
/// 两层布局必须**同时**成立：只应用外层会让两栏停在初始矩形，只应用内层会让分栏
/// 行的位置/高度是旧的。把它们封成一个函数，就不会出现“改了一层忘了另一层”。
/// 首次启动与显式改变窗口尺寸都调同一个函数。
fn apply_layout(
    win: &rust_widgets::app::WindowHandle,
    tool_bar_id: ObjectId,
    status_bar_id: ObjectId,
    tree: &CustomWidgetHandle,
    editor: &CustomWidgetHandle,
    log: &Arc<EventLog>,
) {
    // 外层：三行。
    let layout = build_window_layout(tool_bar_id, SPLIT_ROW_ID, status_bar_id);
    log.append(format!(
        "[Layout] window BoxLayout(Vertical): toolbar {TOOLBAR_H}px (fixed) + \
         split (expanding) + status bar {STATUS_H}px (fixed)"
    ));
    win.set_layout(layout);

    // 内层：用外层算出的分栏行矩形把两栏切开。
    let row = split_row_rect_in(tool_bar_id, SPLIT_ROW_ID, status_bar_id);
    let split = build_split_layout(tree.raw_id(), editor.raw_id());
    apply_split(&split, row, tree, editor);
    log.append(format!(
        "[Layout] split BoxLayout(Horizontal): tree:editor = {}:{} \
         (min {TREE_MIN_W}px / {EDITOR_MIN_W}px) in row ({},{}, {}, {})",
        SPLIT_WEIGHTS.0, SPLIT_WEIGHTS.1, row.x, row.y, row.width, row.height
    ));
    log.append("[Layout] applied");
}

/// 轮询菜单/控件事件并转成命令。
///
/// # 线程分工：平台循环在主线程，轮询在后台线程
///
/// 两个事实决定了这个分工，顺序不能颠倒：
///
/// 1. **平台事件循环必须在主线程跑。** GTK 3 把每个控件绑定到唯一的主线程，
///    `gtk::main()` 从别的线程调用会直接 panic（`GTK may only be used from the main
///    thread.`）。
/// 2. **宿主又必须能轮询。** `poll_menu_triggered()` / `poll_widget_trigger_event()`
///    是必须在循环之外调用的——循环一进去就回不到 Rust 侧，菜单就变成纯装饰。
///
/// 所以：平台循环占住主线程，**轮询放到后台线程**。这与本文件早先的写法正好相反
/// （那时尝试把平台循环放到后台）——那种写法在 GTK 下必然 panic，因为它在工作线程里
/// 初始化并驱动 GTK。
///
/// 只有轮询本身会动 Qt/GTK 对象时才会出问题；`poll_*` 只读队列并分派命令，
/// 不需要平台 API，所以可以安全地在后台线程运行。
fn run_loop(
    app: &App,
    bindings: &[(u64, MenuBinding)],
    editor: &CustomWidgetHandle,
    log: &Arc<EventLog>,
) {
    // 轮询线程需要在主线程进入平台循环之前启动，否则它一开始就看不到已经入队的
    // 触发；`Arc` 让两个线程各自持有需要的东西。
    let bindings: Arc<Vec<(u64, MenuBinding)>> = Arc::new(bindings.to_vec());
    let editor = editor.clone();
    let poll_log = Arc::clone(log);
    let done = Arc::new(AtomicBool::new(false));
    let done_flag = Arc::clone(&done);

    let poller = std::thread::spawn(move || {
        let mut handled = 0usize;
        let mut idle_ticks = 0u32;
        while !done_flag.load(Ordering::SeqCst) {
            let mut did_work = false;

            // 菜单激活：菜单不会直接回调 Rust，必须从队列取出。
            while let Some(item_id) = rust_widgets::poll_menu_triggered() {
                if dispatch_menu_event(item_id, &bindings, &editor, &poll_log) {
                    did_work = true;
                    handled += 1;
                }
            }

            // 控件触发（工具栏按钮已由 on_click 回调处理，这里只计数）。
            while let Some(event) = rust_widgets::poll_widget_trigger_event() {
                poll_log.append(format!("[Widget] id={} kind={:?}", event.widget_id, event.kind));
                did_work = true;
                handled += 1;
            }

            if did_work {
                idle_ticks = 0;
                refresh_status(&poll_log, &editor);
            } else {
                idle_ticks += 1;
                // 事件循环空转超过 30 秒无任何事件也退出，避免无法关闭的孤儿进程。
                if idle_ticks > 30 * 60 * 2 {
                    poll_log.append("[App] 长时间无事件，自动退出");
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        poll_log.append(format!("[App] 轮询结束（处理 {handled} 个事件）"));
    });

    // 主线程跑平台循环，直到窗口关闭（`gtk::main()` 返回）。
    log.append("[App] 平台事件循环在主线程启动");
    app.run();

    // 循环已返回，通知轮询线程收工并等它把剩余事件处理完，
    // 否则最后几个菜单触发会在退出时丢失。
    done.store(true, Ordering::SeqCst);
    let _ = poller.join();
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

// ═══════════════════════════════════════════════════════════════════════════════
// 测试 —— 验证窗口布局把三行与两栏都放对了
// ═══════════════════════════════════════════════════════════════════════════════
//
// `Layout::update` 是纯函数，所以三行的高度分配与两栏的宽度比例都可以在无头环境下
// 断言。这正是本轮修复“编辑器独占整个宽度”后必须加的回归面。
#[cfg(test)]
mod tests {
    use super::*;

    /// 收集一次布局的结果：`(控件 id, 几何)`。
    fn laid_out(layout: &dyn Layout, area: Rect) -> Vec<(ObjectId, Rect)> {
        let mut result = Vec::new();
        layout.update(area, &mut |id, geometry| result.push((id, geometry)));
        result
    }

    /// 假的三行 id（工具栏 / 分栏宿主 / 状态栏）。
    fn strip_ids() -> (ObjectId, ObjectId, ObjectId) {
        (1, 2, 3)
    }

    /// 假的两栏 id（项目树 / 编辑器）。
    fn pane_ids() -> (ObjectId, ObjectId) {
        (11, 12)
    }

    /// 从一次布局结果里取某个 id 的矩形。
    fn rect_of(placed: &[(ObjectId, Rect)], id: ObjectId) -> Rect {
        placed
            .iter()
            .find(|(placed_id, _)| *placed_id == id)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("id {id} was not placed"))
    }

    /// 工具栏在顶部、状态栏在底部、分栏居中且吃掉剩余高度。
    #[test]
    fn the_three_rows_stack_in_order() {
        let (toolbar, split, status) = strip_ids();
        let layout = build_window_layout(toolbar, split, status);
        let placed = laid_out(&layout, Rect::new(0, 0, WINDOW_W, WINDOW_H));

        let toolbar_rect = rect_of(&placed, toolbar);
        let split_rect = rect_of(&placed, split);
        let status_rect = rect_of(&placed, status);

        assert!(toolbar_rect.y < split_rect.y, "the toolbar must precede the split");
        assert!(split_rect.y < status_rect.y, "the split must precede the status bar");
        assert!(
            toolbar_rect.y + toolbar_rect.height as i32 <= split_rect.y,
            "the toolbar and split must not overlap"
        );
        assert!(
            split_rect.y + split_rect.height as i32 <= status_rect.y,
            "the split and status bar must not overlap"
        );
        assert!(
            status_rect.y + status_rect.height as i32 <= WINDOW_H as i32,
            "the status bar must stay in the window: {status_rect:?}"
        );
    }

    /// 工具栏与状态栏是固定高度，分栏拿走剩下的全部高度。
    #[test]
    fn fixed_rows_keep_their_height_and_the_split_absorbs_the_rest() {
        let (toolbar, split, status) = strip_ids();
        let layout = build_window_layout(toolbar, split, status);

        for height in [WINDOW_H, WINDOW_H + 200] {
            let placed = laid_out(&layout, Rect::new(0, 0, WINDOW_W, height));
            assert_eq!(rect_of(&placed, toolbar).height, TOOLBAR_H, "the toolbar is fixed");
            assert_eq!(rect_of(&placed, status).height, STATUS_H, "the status bar is fixed");

            let expected = height - TOOLBAR_H - STATUS_H - STRIP_GAP * 2;
            assert_eq!(
                rect_of(&placed, split).height,
                expected,
                "the split must absorb the leftover height at window height {height}"
            );
        }
    }

    /// 三行都占满客户区宽度：状态栏不能只对齐编辑器。
    #[test]
    fn every_row_spans_the_client_width() {
        let (toolbar, split, status) = strip_ids();
        let layout = build_window_layout(toolbar, split, status);
        let placed = laid_out(&layout, Rect::new(0, 0, WINDOW_W, WINDOW_H));
        for (id, rect) in &placed {
            assert_eq!(rect.x, 0, "id {id} must start at the left edge: {rect:?}");
            assert_eq!(rect.width, WINDOW_W, "id {id} must span the width: {rect:?}");
        }
    }

    // ── 分栏（本轮修复的核心）────────────────────────────────────────────

    /// 编辑器**不再**独占宽度：项目树与编辑器各占一份，且比例符合权重。
    ///
    /// 这是本轮修复的直接回归护栏。修复前窗口里根本没有项目树，
    /// 编辑器 `rect.width == WINDOW_W`，即“宽度比例”是 0 : 1。
    #[test]
    fn the_editor_no_longer_takes_the_whole_width() {
        let (tree, editor) = pane_ids();
        let split = build_split_layout(tree, editor);
        let placed = laid_out(&split, Rect::new(0, 0, WINDOW_W, split_row_height()));

        let tree_rect = rect_of(&placed, tree);
        let editor_rect = rect_of(&placed, editor);

        assert!(
            editor_rect.width < WINDOW_W,
            "the editor must share the width with the tree: {editor_rect:?}"
        );
        assert!(tree_rect.width > 0, "the tree must have width: {tree_rect:?}");
        assert!(
            tree_rect.x + tree_rect.width as i32 <= editor_rect.x,
            "the tree and editor must not overlap: {tree_rect:?} then {editor_rect:?}"
        );
        // 两栏加间距必须等于（不超过）可用宽度。
        let used = tree_rect.width + editor_rect.width + SPLIT_GAP;
        assert!(used <= WINDOW_W, "the split overflows the window: {used} > {WINDOW_W}");
    }

    /// 两栏的宽度比必须与 `SPLIT_WEIGHTS` 一致（允许 1px 的整数取整误差）。
    #[test]
    fn the_two_panes_follow_the_declared_weights() {
        let (tree, editor) = pane_ids();
        let split = build_split_layout(tree, editor);
        let placed = laid_out(&split, Rect::new(0, 0, WINDOW_W, split_row_height()));

        let tree_w = rect_of(&placed, tree).width as f64;
        let editor_w = rect_of(&placed, editor).width as f64;
        let expected = SPLIT_WEIGHTS.0 as f64 / (SPLIT_WEIGHTS.0 + SPLIT_WEIGHTS.1) as f64;
        let actual = tree_w / (tree_w + editor_w);
        assert!(
            (actual - expected).abs() < 0.02,
            "the tree must take ~{expected:.3} of the split, got {actual:.3} \
             (tree {tree_w}px, editor {editor_w}px)"
        );
    }

    /// 加宽窗口时两栏**同时**变宽（比例如故），而不是某一栏占掉全部增量。
    ///
    /// 这正是用权重、而不是写死“项目树 240px”的价值。
    #[test]
    fn growing_the_window_grows_both_panes_in_proportion() {
        let (tree, editor) = pane_ids();
        let split = build_split_layout(tree, editor);

        let narrow = laid_out(&split, Rect::new(0, 0, 800, split_row_height()));
        let wide = laid_out(&split, Rect::new(0, 0, 1600, split_row_height()));

        let tree_narrow = rect_of(&narrow, tree).width;
        let tree_wide = rect_of(&wide, tree).width;
        let editor_narrow = rect_of(&narrow, editor).width;
        let editor_wide = rect_of(&wide, editor).width;

        assert!(tree_wide > tree_narrow, "the tree must widen: {tree_narrow} -> {tree_wide}");
        assert!(
            editor_wide > editor_narrow,
            "the editor must widen too: {editor_narrow} -> {editor_wide}"
        );
        // 两边按**同一比例**变宽：增量之比应等于权重之比（1:4）。
        // 断言“同一个比例”而不是“两边增量相近” —— 后者会把正确的 1:4 当成错误。
        let tree_growth = tree_wide - tree_narrow;
        let editor_growth = editor_wide - editor_narrow;
        assert!(tree_growth > 0 && editor_growth > 0);
        let expected_ratio = SPLIT_WEIGHTS.1 as f64 / SPLIT_WEIGHTS.0 as f64;
        let actual_ratio = editor_growth as f64 / tree_growth as f64;
        assert!(
            (actual_ratio - expected_ratio).abs() < 0.1,
            "both panes must grow in the declared proportion ({expected_ratio:.2}:1), \
             got {actual_ratio:.2}:1 (tree +{tree_growth}, editor +{editor_growth})"
        );
    }

    /// 极窄窗口下两栏仍不重叠、不溢出，且各自不低于最小宽度（或按比例摊派）。
    #[test]
    fn a_narrow_window_still_places_both_panes_inside() {
        let (tree, editor) = pane_ids();
        let split = build_split_layout(tree, editor);
        for width in [200u32, TREE_MIN_W + EDITOR_MIN_W + SPLIT_GAP, 400] {
            let placed = laid_out(&split, Rect::new(0, 0, width, split_row_height()));
            let tree_rect = rect_of(&placed, tree);
            let editor_rect = rect_of(&placed, editor);
            assert!(
                tree_rect.x + tree_rect.width as i32 <= editor_rect.x,
                "panes overlap at width {width}: {tree_rect:?} / {editor_rect:?}"
            );
            assert!(
                editor_rect.x + editor_rect.width as i32 <= width as i32,
                "the editor escapes a {width}px window: {editor_rect:?}"
            );
        }
    }

    /// 初始的 `pane_rects()` 必须与两栏布局算出的结果一致。
    ///
    /// 挂载自绘型控件必须先给一个矩形，两者不一致会让首帧跳动。
    #[test]
    fn the_initial_pane_rects_match_the_laid_out_ones() {
        let (tree, editor) = pane_ids();
        let split = build_split_layout(tree, editor);
        let placed = laid_out(&split, Rect::new(0, 0, WINDOW_W, split_row_height()));
        let (initial_tree, initial_editor) = pane_rects();

        assert_eq!(initial_tree.width, rect_of(&placed, tree).width, "tree width mismatch");
        assert_eq!(initial_tree.x, rect_of(&placed, tree).x, "tree x mismatch");
        assert_eq!(initial_editor.width, rect_of(&placed, editor).width, "editor width mismatch");
        assert_eq!(initial_editor.x, rect_of(&placed, editor).x, "editor x mismatch");
        assert_eq!(initial_tree.height, split_row_height());
        assert_eq!(initial_editor.height, split_row_height());
    }

    /// `apply_layout` 两层用的必须是同一块分栏行区域。
    ///
    /// 这条测试盯的是最容易犯的分层错误：外层把分栏行算在 y=38、高 654，
    /// 而内层却按 y=0、全窗高去切两栏（或反之）。两者必须完全一致，
    /// 否则两栏会压在工具栏/状态栏上，或者在底部留出一条缝。
    #[test]
    fn both_layout_layers_agree_on_the_split_row() {
        let (toolbar, split_row, status) = strip_ids();
        let row = split_row_rect_in(toolbar, split_row, status);

        // 外层实际给分栏行的矩形，与 `split_row_rect_in` 的答案必须相同。
        let outer = laid_out(
            &build_window_layout(toolbar, split_row, status),
            Rect::new(0, 0, WINDOW_W, WINDOW_H),
        );
        assert_eq!(row, rect_of(&outer, split_row), "the two layers disagree on the row");

        // 内层在 `row` 里切出的两栏必须都落在 `row` 内。
        let (tree, editor) = pane_ids();
        let inner = laid_out(&build_split_layout(tree, editor), row);
        for (id, rect) in &inner {
            assert!(
                rect.x >= row.x && rect.y >= row.y,
                "id {id} starts outside the split row: {rect:?} vs {row:?}"
            );
            assert!(
                rect.x + rect.width as i32 <= row.x + row.width as i32,
                "id {id} escapes the split row horizontally: {rect:?}"
            );
            assert_eq!(rect.height, row.height, "id {id} must fill the row's height");
        }
    }
}
