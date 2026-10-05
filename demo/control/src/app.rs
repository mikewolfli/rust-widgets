//! Control Demo — 基础控件综合演示（基于 App 框架）
//!
//! 使用 App + WindowHandle + WidgetHandle 体系，创建窗口并放置各类控件。
//! 所有控件事件通过 handle.on_click / on_value_changed 实时记录。
//!
//! Demo 版式（均为普通控件，放置方式由库决定）：
//!   菜单栏 — File / View
//!   工具栏 + 状态栏 — ToolBar / StatusBar
//!   行 0 — Button:      Button（含一个切换外观的 Button 与一个 disabled Button）
//!   行 1 — Toggle:      CheckBox, tri-state CheckBox, RadioButton x3
//!   行 2 — Text input:  LineEdit（普通）, password LineEdit, read-only LineEdit, SpinBox
//!   行 3 — Selection:   ComboBox, ListBox
//!   行 4 — Range:       Slider, ProgressBar, indeterminate ProgressBar
//!   行 5 — Text area:   ScrollArea（内含多行 Label）
//!   行 6 — Panel/Frame: Panel + Frame
//!   行 7 — Dialog:      MessageBox（默认隐藏，由 “Click Me” 打开）
//!   底部 — 事件日志（Label 行）
//!
//! 本 demo 只演示**平台普通控件**；自绘型控件（如 `CodeEditor`、`TreeView`）
//! 需要挂载面，归 `demo/code_editor` 演示。
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

use rust_widgets::app::{App, AppConfig, WidgetHandle, WindowHandle};
use rust_widgets::core::{ObjectId, Orientation, Rect};
use rust_widgets::layout::Layout as LayoutTrait;
use rust_widgets::theme::{global_theme_manager, AppearanceMode};

// ═══════════════════════════════════════════════════════════════════════════════
// 外观（暗色）
// ═══════════════════════════════════════════════════════════════════════════════

/// 当前是否已经是暗色外观。
///
/// 问管理器而不是自己记一个 bool：主题是全局状态，别的代码也可能改它，
/// 自记的副本会与真相不一致，然后按钮就会往错的方向切。
fn dark_active() -> bool {
    global_theme_manager()
        .current_theme()
        .map(|theme| theme.appearance == AppearanceMode::Dark)
        .unwrap_or(false)
}

/// 切到指定外观，并让已存在的控件跟着换。
///
/// 两步缺一不可：`set_appearance` 改全局主题（之后新建的控件会用它），
/// `reapply_active_theme` 重刷**已经挂载**的控件（没有这一步，屏幕上的控件保持
/// 旧配色，切换看起来没发生）。返回真正生效的主题名，该外观未注册时为 `None`。
fn switch_appearance(dark: bool) -> Option<String> {
    let mode = if dark { AppearanceMode::Dark } else { AppearanceMode::Light };
    if !global_theme_manager().set_appearance(mode) {
        return None;
    }
    rust_widgets::reapply_active_theme();
    Some(global_theme_manager().current_theme_name().to_string())
}

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
// 响应式布局 —— 窗口缩放后重排所有控件
// ═══════════════════════════════════════════════════════════════════════════════

/// 一个控件在窗口里的行/列归属。
///
/// 行持有「纵坐标 + 高度」，列持有「横坐标 + 宽度」，两者按权重分配可用空间。
/// 这样窗口拉大时每一行/列等比变宽，而不是像固定坐标那样把右侧留白。
#[derive(Debug, Clone, Copy)]
struct Slot {
    /// 控件 id。
    id: ObjectId,
    /// 行索引，用于取纵坐标与高度。
    row: usize,
    /// 该行内的横向权重；`0` 表示「只占自身标称宽度，不拉伸」。
    weight: u32,
    /// 标称尺寸，在权重为 0 或窗口过小时使用。
    nominal: (i32, i32, u32, u32),
}

/// 窗口版式：把标称坐标按窗口实际尺寸重新分配。
///
/// # 为什么需要它
///
/// 本 demo 以往的所有控件都用**绝对坐标**创建，且只有启动时算一次。用户拖动窗口
/// 放大后，控件仍然停留在为 1120×620 算出的位置上——右下角空出一大片，而控件本身
/// 一个都没动。库侧的链路其实是完整的（OS 报 resize → 后端入队 `Resized` →
/// `app/handle.rs` 重跑窗口 layout，见 `apply_window_layout`），缺的只是
/// **一个交给 `WindowHandle::set_layout` 的 layout**：调用了它，resize 才会真的重排。
///
/// `demo/finance` 修过同一个缺陷（见其 `PanelLayout` 的说明）；本 demo 当时漏掉了。
///
/// # 为什么记住上次应用的尺寸
///
/// 一次拖拽会重复投递同一个客户区尺寸。对同一个矩形重跑整套放置是不可见的工作量，
/// 而逐帧累积的不可见工作量正是卡顿的来源。记住上次尺寸让重复投递变成空操作。
struct ControlGridLayout {
    /// 所有参与重排的控件。
    slots: Vec<Slot>,
    /// 日志，用于把重排结果写进事件面板。
    log: Arc<EventLog>,
    /// 上次应用过的尺寸；相同则跳过。
    ///
    /// `Cell` 是因为 `update` 取 `&self`：记不住「已经做过什么」的 layout 只能重复做。
    last_applied: std::cell::Cell<Option<(u32, u32)>>,
}

impl ControlGridLayout {
    /// 把窗口客户区切成各行的纵向范围。
    ///
    /// 行高按「标称高度」比例分配而不是平均分配：控件行本来就是高低不一的
    /// （工具栏 32、列表 90、标签行 18），平均分会把它们挤变形。多出来的空间
    /// 按比例分给每一行，等于整体等比放大，与固定坐标下的观感一致。
    fn row_bands(&self, total_height: u32) -> Vec<(i32, u32)> {
        // 每行的标称高度 = 该行所有控件里最大的高度。
        let mut nominal: Vec<u32> = Vec::new();
        for slot in &self.slots {
            if nominal.len() <= slot.row {
                nominal.resize(slot.row + 1, 0);
            }
            nominal[slot.row] = nominal[slot.row].max(slot.nominal.3);
        }
        if nominal.is_empty() {
            return Vec::new();
        }
        let nominal_total: u32 = nominal.iter().sum::<u32>().max(1);
        let mut bands = Vec::with_capacity(nominal.len());
        let mut y = 0i32;
        for (index, height) in nominal.iter().enumerate() {
            // 最后一行吃掉余数，避免累计取整让底部漏出一条缝。
            let band = if index + 1 == nominal.len() {
                (total_height as i32 - y).max(0) as u32
            } else {
                ((*height as u64 * total_height as u64) / nominal_total as u64) as u32
            };
            bands.push((y, band));
            y += band as i32;
        }
        bands
    }
}

impl LayoutTrait for ControlGridLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        // 槽位在构建时一次性登记；多出来的 id 没有行归属，收下只会让控件落在 (0,0)。
        let _ = widget_id;
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.slots.retain(|slot| slot.id != widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if self.last_applied.get() == Some((rect.width, rect.height)) {
            return;
        }
        self.last_applied.set(Some((rect.width, rect.height)));

        let bands = self.row_bands(rect.height);
        // 每行的可用宽度 = 窗口宽度减去左右留白。
        let usable = rect.width.saturating_sub(MARGIN * 2);
        let mut moved = 0usize;
        for slot in &self.slots {
            let Some(&(row_y, row_h)) = bands.get(slot.row) else {
                continue;
            };
            // 有横向权重的控件按下标参与拉伸；权重为 0 的保持标称宽度。
            let (x, w) = if slot.weight == 0 {
                (MARGIN as i32 + slot.nominal.0, slot.nominal.2.min(usable.max(1)))
            } else {
                let weight_total: u32 = self
                    .slots
                    .iter()
                    .filter(|other| other.row == slot.row)
                    .map(|other| other.weight)
                    .sum::<u32>()
                    .max(1);
                // 同行内按权重切分：先累加本控件之前的权重得到起点。
                let before: u32 = self
                    .slots
                    .iter()
                    .take_while(|other| other.id != slot.id)
                    .filter(|other| other.row == slot.row)
                    .map(|other| other.weight)
                    .sum();
                let start = MARGIN as u64 + (usable as u64 * before as u64) / weight_total as u64;
                let end = MARGIN as u64
                    + (usable as u64 * (before + slot.weight) as u64) / weight_total as u64;
                (start as i32, (end.saturating_sub(start)).max(1) as u32)
            };
            // 纵向只让控件在自己的行带内居中，不做拉伸：控件高度是设计值，拉伸会破坏比例。
            let height = slot.nominal.3.min(row_h.max(1));
            let y = row_y + ((row_h as i32 - height as i32) / 2).max(0);
            widgets(slot.id, Rect::new(x, y, w, height));
            moved += 1;
        }
        if moved > 0 {
            self.log.append(format!(
                "[Layout] 窗口 {}x{} -> {} 个控件重排",
                rect.width, rect.height, moved
            ));
        }
    }
}

/// 窗口四周留白，与控件创建时的 x=20 保持一致。
const MARGIN: u32 = 20;

#[cfg(test)]
mod layout_tests {
    use super::*;

    /// 构造一个两行三列的版式：第 0 行三个等权控件，第 1 行一个满宽控件。
    fn slots() -> Vec<Slot> {
        vec![
            Slot { id: 1, row: 0, weight: 1, nominal: (20, 20, 100, 30) },
            Slot { id: 2, row: 0, weight: 1, nominal: (140, 20, 100, 30) },
            Slot { id: 3, row: 0, weight: 1, nominal: (260, 20, 100, 30) },
            Slot { id: 4, row: 1, weight: 1, nominal: (20, 60, 340, 30) },
        ]
    }

    fn layout() -> ControlGridLayout {
        ControlGridLayout {
            slots: slots(),
            log: Arc::new(EventLog::new()),
            last_applied: std::cell::Cell::new(None),
        }
    }

    fn arrange(layout: &ControlGridLayout, width: u32, height: u32) -> Vec<(ObjectId, Rect)> {
        let mut out = Vec::new();
        LayoutTrait::update(layout, Rect::new(0, 0, width, height), &mut |id, rect| {
            out.push((id, rect));
        });
        out
    }

    /// 窗口变宽时，控件必须真的变宽 —— 这正是「固定坐标」做不到的事。
    #[test]
    fn a_wider_window_widens_the_controls() {
        let layout = layout();
        let narrow = arrange(&layout, 600, 400);
        layout.last_applied.set(None);
        let wide = arrange(&layout, 1200, 400);

        for ((id, narrow_rect), (wide_id, wide_rect)) in narrow.iter().zip(wide.iter()) {
            assert_eq!(id, wide_id, "两次重排必须覆盖同一批控件");
            assert!(
                wide_rect.width > narrow_rect.width,
                "控件 {id} 在更宽的窗口里没有变宽：{} -> {}",
                narrow_rect.width,
                wide_rect.width
            );
        }
    }

    /// 同行控件在重排后不能重叠，且必须落在客户区内 —— 权重切分最容易在这两点上出错。
    #[test]
    fn same_row_slots_do_not_overlap_and_stay_inside() {
        let layout = layout();
        for width in [400u32, 800, 1600] {
            layout.last_applied.set(None);
            let rects: Vec<Rect> = arrange(&layout, width, 400)
                .into_iter()
                .filter(|(id, _)| *id <= 3)
                .map(|(_, rect)| rect)
                .collect();
            let mut sorted = rects.clone();
            sorted.sort_by_key(|rect| rect.x);
            for pair in sorted.windows(2) {
                assert!(
                    pair[0].x + pair[0].width as i32 <= pair[1].x,
                    "窗口宽 {width} 时同行控件重叠：{:?} 与 {:?}",
                    pair[0],
                    pair[1]
                );
            }
            for rect in &sorted {
                assert!(
                    rect.x >= 0 && rect.x + rect.width as i32 <= width as i32,
                    "窗口宽 {width} 时控件越界：{rect:?}"
                );
            }
            assert_eq!(sorted.len(), 3, "三个同行控件都必须被放置");
        }
    }

    /// 同尺寸重复投递必须被跳过 —— 一次拖拽会重复送来同一个尺寸。
    #[test]
    fn a_repeated_size_is_not_reapplied() {
        let layout = layout();
        let first = arrange(&layout, 900, 500);
        assert_eq!(first.len(), 4, "第一次必须放置全部控件");
        let second = arrange(&layout, 900, 500);
        assert!(second.is_empty(), "同一尺寸重复投递不应再放一次");

        layout.last_applied.set(None);
        let third = arrange(&layout, 901, 500);
        assert_eq!(third.len(), 4, "尺寸变了就必须重排");
    }

    /// 窗口比控件还窄时不能产生负宽度或 panic。
    #[test]
    fn a_very_narrow_window_still_produces_usable_rects() {
        let layout = layout();
        let rects = arrange(&layout, 10, 10);
        assert_eq!(rects.len(), 4);
        for (id, rect) in rects {
            assert!(rect.width >= 1, "控件 {id} 宽度退化为 0：{rect:?}");
            assert!(rect.height >= 1, "控件 {id} 高度退化为 0：{rect:?}");
        }
    }

    /// 滚动区必须声明内容**范围**，且内容行必须真的是它的子控件。
    ///
    /// # 这条测什么
    ///
    /// 修复前滚动区只是一个 260×90 的视口，没有 `content_size`，六行“log line …”
    /// 也只是窗口的兄弟控件、恰好压在它上面。这里用与 demo 相同的后端调用重建那一块：
    /// 内容行用**滚动区**做 parent（而不是窗口），并声明内容范围。
    ///
    /// 断言分两半：
    /// * **内容行确实被滚动区拥有** —— 直接读控件自身的子列表，这是 demo 要的“内容”关系；
    /// * **内容范围已声明** —— 通过 `ScrollAreaHandle` 读取回它记录的范围（`set_content_size`
    ///   在句柄侧记下这份数据，`scroll_to_bottom`/`AsNeeded` 据此计算）。
    #[test]
    fn scroll_area_declares_its_extent_and_owns_its_content() {
        use rust_widgets::app::ScrollAreaHandle;

        rust_widgets::init();
        let window = rust_widgets::create_window("scroll-test", 0, 0, 320, 240);
        assert_ne!(window, 0, "窗口必须创建成功");

        let area = rust_widgets::create_scroll_area(window, 20, 286, 260, 90);
        assert_ne!(area, 0, "滚动区必须创建成功");

        const CONTENT_H: u32 = 6 * 20 + 4;
        let handle = ScrollAreaHandle::from_raw(area);
        handle.set_content_size(260, CONTENT_H);

        for i in 0..6u32 {
            let child = rust_widgets::create_label(
                area,
                &format!("log line {i} - scroll me"),
                4,
                4 + i as i32 * 20,
                244,
                18,
            );
            assert_ne!(child, 0, "内容行 {i} 必须挂载成功");
        }

        // 内容范围已声明且高于视口：`scroll_to_bottom` 会按这份高度算出底部偏移。
        handle.scroll_to_bottom();
        assert_eq!(
            handle.scroll_position().1,
            CONTENT_H as i32,
            "内容范围必须恰恰是声明的那个（且高于视口）"
        );
        assert!(CONTENT_H > 90, "内容必须高于视口，否则无滚可滚");

        // 六行都是滚动区的子控件，而不是窗口的兄弟。
        let child_count = rust_widgets::widget::runtime::with_widget(area, |widget| {
            widget.base().children().len()
        })
        .expect("滚动区必须已挂载");
        assert_eq!(child_count, 6, "六行内容都必须是滚动区的子控件");
    }
}

/// 注册窗口 layout，使 resize 后所有控件重新排布。
///
/// 必须在所有控件都建好之后调用：layout 需要它们的 id。
fn register_window_layout(win: &WindowHandle, slots: Vec<Slot>, log: &Arc<EventLog>) {
    win.set_layout(ControlGridLayout {
        slots,
        log: Arc::clone(log),
        last_applied: std::cell::Cell::new(None),
    });
}

// ═══════════════════════════════════════════════════════════════════════════════
// 构建所有控件 + 日志面板
// ═══════════════════════════════════════════════════════════════════════════════

fn build_all_controls(win: &WindowHandle, log: &Arc<EventLog>) -> Vec<Slot> {
    // 每个控件建好后把自己的 id 与行归属登记进来，供窗口 layout 在 resize 时重排。
    // 行号只描述「同一水平带」，列内按权重分宽度。
    let mut slots: Vec<Slot> = Vec::new();
    macro_rules! row {
        ($handle:expr, $row:expr, $weight:expr, $nominal:expr) => {{
            slots.push(Slot {
                id: $handle.raw_id(),
                row: $row,
                weight: $weight,
                nominal: $nominal,
            });
            $handle
        }};
    }
    // ── 对话框（平常不显示） ─────────────────────────────────────────
    //
    // 先建出来但不显示：`show_modal()` 才让它出现。创建对话框不等于显示对话框 ——
    // 见 `create_message_box` 的说明。
    //
    // 它在窗口底部有自己的坐标，但**不会被画出来**，因为控件默认不可见时
    // 窗口树的绘制会跳过它。
    let dialog = win.new_message_box(
        "Demo Info",
        "This is a demo message box.\nIt was opened by the Click Me button.",
        20,
        386,
        300,
        100,
    );
    dialog.set_title("Information");
    let l = Arc::clone(log);
    let dialog_close = dialog.clone();
    dialog.on_click(move || {
        l.append("[MessageBox] button clicked -> closing");
        dialog_close.close();
    });
    log.append("[MessageBox] created (hidden; opened by 'Click Me')");

    // ── Row 0: Button Controls ───────────────────────────────────────
    log.append("═══ Row: Button Controls ═══");

    let btn = row!(win.new_button("Click Me", 20, 20, 150, 32), 0, 1, (20, 20, 150, 32));
    let l = Arc::clone(log);
    let dialog_for_click = dialog.clone();
    btn.on_click(move || {
        l.append("[Button] clicked! -> showing dialog");
        dialog_for_click.show_modal();
    });
    log.append("[Button] at (20,20,150,32)");

    let tog = row!(win.new_button("Dark Mode", 180, 20, 150, 32), 0, 1, (180, 20, 150, 32));
    let l = Arc::clone(log);
    let tog_label = tog.clone();
    tog.on_click(move || match switch_appearance(!dark_active()) {
        Some(name) => {
            l.append(format!("[Theme] switched to '{name}'"));
            tog_label.set_text(if dark_active() { "Light Mode" } else { "Dark Mode" });
        }
        None => l.append("[Theme] 当前构建没有注册该外观的主题"),
    });
    // A `Button` that happens to toggle appearance, not a `ToggleButton`: this crate publishes
    // no `new_toggle_button`, and the log tag must name the control that is actually here so a
    // reader looking for it in the log is not sent after a family that does not exist.
    log.append("[Button] at (180,20,150,32) (toggles appearance)");

    // Disabled button: proves the enable/disable mirror reaches the native control.
    let disabled = row!(win.new_button("Disabled", 340, 20, 150, 32), 0, 1, (340, 20, 150, 32));
    disabled.disable();
    log.append(format!("[Button] disabled sample enabled={}", disabled.is_enabled()));

    // ── Row 1: Toggle Controls ───────────────────────────────────────
    log.append("═══ Row: Toggle Controls ═══");

    let cb =
        row!(win.new_checkbox("Enable notifications", 20, 64, 200, 24), 1, 1, (20, 64, 200, 24));
    let l = Arc::clone(log);
    let cb2 = cb.clone();
    cb2.on_value_changed(move |_val: String| {
        let checked = cb.is_checked();
        l.append(format!("[CheckBox] checked={}", checked));
    });
    log.append("[CheckBox] at (20,64,200,24)");

    // Tri-state check-box: all three states are reachable (Unchecked / Checked /
    // PartiallyChecked), which the mixed state requires tri-state mode to be on.
    let tri = row!(win.new_checkbox("Tri-state", 230, 64, 130, 24), 1, 1, (230, 64, 130, 24));
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

    let le = row!(win.new_line_edit("", 20, 104, 200, 26), 2, 1, (20, 104, 200, 26));
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
    let pw = row!(win.new_line_edit("secret", 230, 104, 150, 26), 2, 1, (230, 104, 150, 26));
    pw.set_echo_mode(rust_widgets::app::EchoMode::Password);
    log.append(format!("[LineEdit] echo_mode={:?}", pw.widget_echo_mode()));

    let ro =
        row!(win.new_line_edit("read-only value", 390, 104, 180, 26), 2, 1, (390, 104, 180, 26));
    ro.set_read_only(true);
    log.append(format!("[LineEdit] read_only={:?}", ro.clone().is_read_only()));

    let sb = row!(win.new_spin_box(580, 104, 120, 26), 2, 1, (580, 104, 120, 26));
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

    let cbx = row!(win.new_combo_box(20, 142, 180, 26), 3, 1, (20, 142, 180, 26));
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

    let lb = row!(win.new_list_box(210, 142, 200, 90), 3, 1, (210, 142, 200, 90));
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
    let rb1 = row!(win.new_radio_button("Option A", 420, 142, 100, 24), 3, 0, (420, 142, 100, 24));
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
    let sl = row!(
        win.new_slider_with_orientation(Orientation::Horizontal, 20, 242, 260, 32),
        4,
        1,
        (20, 242, 260, 32)
    );
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

    let pb = row!(win.new_progress_bar(290, 242, 200, 24), 4, 1, (290, 242, 200, 24));
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
    let busy = row!(win.new_progress_bar(500, 242, 200, 24), 4, 1, (500, 242, 200, 24));
    busy.set_indeterminate(true);
    log.append(format!("[ProgressBar] indeterminate={:?}", busy.is_indeterminate()));

    // ── Method-driven update: 一个按钮用**方法**改写其它控件 ────────
    //
    // 前面的行演示的是“控件 → 日志”方向（事件）。这里演示反向：宿主主动调用控件
    // 方法，而且是**先读后写**。两个方向都要有例子—— “能改”与“能读回”是两件事，
    // 只演示其中一侧会让人误以为另一侧不存在。
    //
    // 放在这里是因为它要引用 `tri`/`sl`/`busy`/`rb3`，四个控件都已创建。
    let sync = row!(win.new_button("Sync", 540, 20, 140, 32), 0, 1, (540, 20, 140, 32));
    let sync_log = Arc::clone(log);
    let sync_tri = tri.clone();
    let sync_sl = sl.clone();
    let sync_busy = busy.clone();
    let sync_rb = rb3.clone();
    sync.on_click(move || {
        // Read: 先读出当前状态（方法的读回侧）。
        let state = sync_tri.check_state();
        let slider = sync_sl.value();
        // Write: 把 Tri-state 推进到下一个状态、滑块推高 5、工忙条切换。
        let next = match state {
            rust_widgets::app::CheckState::Unchecked => rust_widgets::app::CheckState::Checked,
            rust_widgets::app::CheckState::Checked => {
                rust_widgets::app::CheckState::PartiallyChecked
            }
            rust_widgets::app::CheckState::PartiallyChecked => {
                rust_widgets::app::CheckState::Unchecked
            }
        };
        sync_tri.set_check_state(next);
        sync_sl.set_value((slider + 5).min(100));
        sync_busy.set_indeterminate(!sync_busy.is_indeterminate().unwrap_or(false));
        // 单选组：`select` 会把同组的其他按钮自动取消选中（组语义）。
        sync_rb.select();
        sync_log.append(format!(
            "[Sync] tri-state {:?} -> {:?}, slider {} -> {}, busy={:?}, radio C selected={}",
            state,
            sync_tri.check_state(),
            slider,
            sync_sl.value(),
            sync_busy.is_indeterminate(),
            sync_rb.is_selected()
        ));
    });
    log.append("[Button] 'Sync' reads then writes 4 other controls（方法方向）");

    // ── Row 5: Scrollable Text Area ──────────────────────────────────
    log.append("═══ Row: Scrollable Text Area ═══");

    let area = row!(win.new_scroll_area(20, 286, 260, 90), 5, 1, (20, 286, 260, 90));
    // # The content extent is what makes a scrollbar possible
    //
    // `new_scroll_area` creates the **viewport** only; without a declared content
    // size the area has nothing to scroll to, so the `AsNeeded` policy decides no bar
    // is needed and the content is simply clipped (see the note on
    // `ScrollAreaHandle::set_content_size`). Six 20px rows plus 4px of leading padding
    // is 124 content pixels against a 90px viewport, which is what gives the bar a
    // reason to exist.
    let content_h = 6 * 20 + 4;
    area.set_content_size(260, content_h);
    for i in 0..6 {
        // ASCII only: the default build's glyph face carries no em dash, so a `—` here
        // painted as a missing-glyph box. A separator that renders is worth more than a
        // typographically nicer one that does not.
        //
        // # Parented into the scroll area, not the window
        //
        // These used to be `win.new_label(..)` — window **siblings** at absolute
        // coordinates that merely happened to overlap the scroll area. The scroll area
        // knew nothing about them: they were not its content, so scrolling moved the
        // viewport while the labels stayed put, and the extent above had no rows to
        // cover. `create_label(parent, ..)` mounts each row under the scroll area, so
        // it is genuinely the area's content.
        let label = rust_widgets::create_label(
            area.raw_id(),
            &format!("log line {i} - scroll me"),
            4,
            4 + i * 20,
            244,
            18,
        );
        // The scroll area draws its own content frame; each row must still be
        // parented before it can be seen, and reporting a failure to mount keeps a
        // silently-empty area from looking like a layout accent.
        if label == 0 {
            log.append(format!("[ScrollArea] WARN: 内容行 {i} 挂载失败"));
        }
    }
    log.append(format!(
        "[ScrollArea] viewport (20,286,260,90) with content 260x{content_h} and 6 stacked labels"
    ));

    // ── Row 6: Panel / Frame ─────────────────────────────────────────
    log.append("═══ Row: Panel / Frame ═══");

    let panel = row!(win.new_panel(290, 286, 200, 90), 5, 1, (290, 286, 200, 90));
    panel.set_title("Panel");
    let frame = row!(win.new_frame(500, 286, 200, 90), 5, 1, (500, 286, 200, 90));
    frame.set_text("Frame");
    log.append(format!("[Panel] id={:?}  [Frame] id={:?}", panel.raw_id(), frame.raw_id()));

    // ── Log Panel (底部 4 行标签) ─────────────────────────────────────
    log.append("═══ Log Panel ═══");

    let _title = row!(win.new_label("-- Event Log --", 20, 496, 680, 18), 6, 1, (20, 496, 680, 18));
    for i in 0..4 {
        let _row =
            row!(win.new_label("", 20, 518 + i * 20, 680, 18), 6, 1, (20, 518 + i * 20, 680, 18));
    }
    log.append("[LogPanel] 4 label rows at bottom");

    slots
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
    println!("║     rust_widgets  —  Controls Demo v2.1.0             ║");
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

    // 默认暗色外观，并由 `--light` 切回浅色。
    //
    // 必须在创建控件**之前**切换：主题是在控件创建时应用的，先建后切虽然后面能用
    // `reapply_active_theme` 补上，但先切就不用补。
    let want_light = std::env::args().any(|arg| arg == "--light");
    match switch_appearance(!want_light) {
        Some(name) => log.append(format!("[Theme] appearance '{name}'")),
        None => log.append("[Theme] 该外观未注册，沿用默认"),
    }

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

    // 构建全部基础控件，并交给窗口一个 layout
    let slots = build_all_controls(&win, &log);
    // 必须在控件都建好之后：layout 需要它们的 id。
    //
    // 没有这一步，窗口 resize 后**什么都不会重排**：库侧链路是完整的
    // （OS 报 resize → 后端入队 `Resized` → `app/handle.rs` 重跑窗口 layout），
    // 但 `LAYOUTS` 里没有本窗口的条目，`apply_window_layout` 拿到空列表，控件
    // 停留在启动时为 1120×620 算出的坐标。
    register_window_layout(&win, slots, &log);
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
