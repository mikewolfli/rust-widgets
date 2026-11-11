//! Control Demo — 基础控件综合演示（基于 App 框架）
//!
//! 使用 App + WindowHandle + WidgetHandle 体系，创建窗口并放置各类控件。
//! 所有控件事件通过 handle.on_click / on_value_changed 实时记录。
//!
//! Demo 版式（均为普通控件，放置方式由库决定）：
//!   状态栏 — StatusBar（贴在窗口底部）
//!   行 0 — Button:      Button（含一个切换外观的 Button 与一个 disabled Button）
//!   行 1 — Toggle:      CheckBox, tri-state CheckBox, RadioButton x3
//!   行 2 — Text input:  LineEdit（普通）, password LineEdit, read-only LineEdit, SpinBox
//!   行 3 — Selection:   ComboBox, ListBox
//!   行 4 — Range:       Slider, ProgressBar, indeterminate ProgressBar
//!   行 5 — Text area:   ScrollArea（内含多行 Label）+ Panel + Frame
//!   行 6 — Paging:      Pagination (jump cell + page-size selector) + TabWidget
//!   行 7 — Chart:       BarChart (eased value changes)
//!   行 8 — State:       Shuffle data / Save state / Restore state（Shuffle 同时切换 MDI 层叠/平铺）
//!   行 9 — 开发范式:    AdaptiveScaffold（移动脚手架）+ MdiArea（桌面 MDI，两个子窗）
//!   行 10-14 — Log:     事件日志标题与最近四条事件（随控件事件实时更新）
//!   隐藏 — MessageBox（由 “Click Me” 打开）
//!
//! 菜单栏（File / Edit / View）由 `new_menu_bar` + `new_menu` 创建：每个 `new_menu`
//! 会把标题加进菜单栏，点击标题时宿主用 `MenuHandle::open_at` 在对应条目下方展开
//! 弹出菜单；菜单项的选择通过 `MenuHandle::on_triggered` 进入日志，View 的外观切换
//! 则按行的 action id（`MenuHandle::on_item_action_triggered`）分发，标签只负责显示。
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

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use rust_widgets::app::{App, AppConfig, LabelHandle, WidgetHandle, WindowHandle};
use rust_widgets::core::{ObjectId, Orientation, Rect};
use rust_widgets::layout::Layout as LayoutTrait;
use rust_widgets::theme::global_theme_manager;
use rust_widgets::widget::chart_widgets::bar_chart::{BarChart, BarEntry};
use rust_widgets::widget::container_widgets::mdiarea::MdiArea;
use rust_widgets::widget::container_widgets::tabwidget::TabWidget;
use rust_widgets::widget::nav_widgets::adaptive_scaffold::AdaptiveScaffold;
use rust_widgets::widget::nav_widgets::pagination::Pagination;
use rust_widgets::widget::runtime::{with_widget, with_widget_mut};
use rust_widgets::{collect_save_state, restore_save_state, SavedState};

// ═══════════════════════════════════════════════════════════════════════════════
// 外观（暗色）
// ═══════════════════════════════════════════════════════════════════════════════
//
// The demo registers and selects the library's macOS presets through
// `preview_common::apply_appearance_mode` (DM3-1). The palette table, the registration order
// and the `reapply_active_theme` half all live in that one function now: a private copy here
// had already dropped the repaint, which made a theme switch after the first paint look inert.

// ═══════════════════════════════════════════════════════════════════════════════
// Log System — 线程安全的事件日志
// ═══════════════════════════════════════════════════════════════════════════════

/// How many recent entries the log keeps in memory.
///
/// This demo appends on every control event and runs for as long as its window is open, so an
/// unbounded buffer grows with the event count. Past this limit the oldest entry is dropped:
/// memory stays bounded and the entries a reader looks at — the most recent ones, the four the
/// on-screen panel shows — are exactly the ones that survive.
const EVENT_LOG_CAPACITY: usize = 200;

/// Set to any value to make the chart row export its SVG at start-up (DM4-6).
///
/// The export used to run unconditionally on the construction path, which the offscreen preview
/// also uses, so every preview paid a full serialise and a temp-file write for an artifact nothing
/// reads. Same opt-in shape as finance's paint check.
const EXPORT_SVG_ENV: &str = "RW_DEMO_EXPORT_SVG";

/// One on-screen log row together with the text it currently shows.
struct LogRow {
    /// The label that renders this row.
    handle: LabelHandle,
    /// The text last written to `handle`.
    ///
    /// Kept so a write that would not change anything can be skipped: a burst of events that
    /// leaves a row identical must not cost a native `set_text` per event.
    shown: String,
}

struct EventLog {
    entries: Mutex<VecDeque<String>>,
    /// On-screen rows that mirror the most recent entries.
    ///
    /// Held by handle rather than by reference: the rows are created after most of the startup
    /// events, and a `Mutex<Vec<LogRow>>` is the smallest way to let `append` keep them
    /// current without a second callback path. Every append in this demo runs on the loop
    /// thread (startup hooks and control callbacks), which is the thread the widget registry
    /// belongs to.
    rows: Mutex<Vec<LogRow>>,
}

impl EventLog {
    fn new() -> Self {
        Self { entries: Mutex::new(VecDeque::new()), rows: Mutex::new(Vec::new()) }
    }

    fn append(&self, msg: impl Into<String>) {
        let msg = msg.into();

        // One writer mutex order everywhere in this type: rows first, then entries.
        let mut rows = self.rows.lock().unwrap();
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= EVENT_LOG_CAPACITY {
            entries.pop_front();
        }
        entries.push_back(msg);
        // Show the tail of the log in the panel: the last N messages, oldest first, so the
        // newest line is always the bottom row — the direction a terminal scrolls.
        //
        // DM4-1: the comparison borrows the entries directly. The previous form cloned the whole
        // four-line tail into a `Vec<String>` before comparing, so a burst of identical events paid
        // one `Vec` and four `String` clones per event only to discover that no row had changed. The
        // lock order (rows, then entries) and the write rule are unchanged: only a row whose text
        // actually differs is written, so a repeated event still costs zero native `set_text`.
        let start = entries.len().saturating_sub(rows.len());
        // Right-align, exactly as `attach_rows` does: with fewer entries than rows the newest
        // line belongs in the bottom row, and the leading rows are blank rather than keeping
        // whatever an earlier, longer log left there. `zip` alone put the first entry in the top
        // row, which disagreed with the panel `attach_rows` builds. In the demo the log is
        // already longer than the panel when it attaches, so the two never visibly differed; the
        // rule is aligned here so a future panel with a short log cannot show it.
        let leading_blank = rows.len().saturating_sub(entries.len());
        for row in rows.iter_mut().take(leading_blank) {
            if !row.shown.is_empty() {
                row.handle.set_text("");
                row.shown.clear();
            }
        }
        for (row, line) in rows.iter_mut().skip(leading_blank).zip(entries.range(start..)) {
            if row.shown.as_str() != line.as_str() {
                row.handle.set_text(line);
                row.shown.clone_from(line);
            }
        }
    }

    /// Wires the panel rows that mirror this log, and fills them with what is already there.
    fn attach_rows(&self, handles: Vec<LabelHandle>) {
        // Same lock order as `append`: rows first, then entries.
        let mut rows = self.rows.lock().unwrap();
        let tail: Vec<String> = {
            let entries = self.entries.lock().unwrap();
            let start = entries.len().saturating_sub(handles.len());
            entries.range(start..).cloned().collect()
        };
        // Right-align: the newest entry belongs in the bottom row even when fewer entries
        // exist than rows. `zip` used to put the first entry in the top row, which showed an
        // old line at the bottom of a freshly attached panel.
        let first_filled = handles.len().saturating_sub(tail.len());
        *rows = handles
            .into_iter()
            .enumerate()
            .map(|(index, handle)| {
                let text = if index >= first_filled {
                    tail[index - first_filled].clone()
                } else {
                    String::new()
                };
                if !text.is_empty() {
                    handle.set_text(&text);
                }
                LogRow { handle, shown: text }
            })
            .collect();
    }

    fn snapshot(&self) -> Vec<String> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 响应式布局 —— 窗口缩放后重排所有控件
// ═══════════════════════════════════════════════════════════════════════════════

/// One control's place in the window.
///
/// A slot records a **role** ([`Band`]), not a position: the layout decides where that role
/// sits at the current window size. The nominal rectangle is still carried because a row's
/// height and a fixed-width slot's width come from the size the demo was designed at.
#[derive(Debug, Clone, Copy)]
struct Slot {
    /// Control id.
    id: ObjectId,
    /// Which horizontal band of the window the control belongs to.
    band: Band,
    /// Row inside [`Band::Content`]; ignored by the chrome bands.
    row: u16,
    /// Share of the row's width. `0` keeps the nominal width (for a control that must not
    /// stretch); every slot in this demo is weighted.
    weight: u32,
    /// The rectangle this control had in the original fixed design.
    nominal: (i32, i32, u32, u32),
}

/// The window band a slot belongs to.
///
/// # Why a band rather than a row index
///
/// The status bar spans the window and has a fixed height; the control rows live above it.
/// Encoding chrome as "row 0, row 1, row 6" made the control rows start at the window's top
/// edge as well, so the status bar floated over the last row. A band makes the reserved
/// height a property of the layout rather than a coordinate coincidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Band {
    /// The menu bar, pinned to the top edge.
    Menu,
    /// The resizable grid of controls.
    Content,
    /// The status bar, at the very bottom.
    Status,
}

/// A control that lives inside another slot and follows it.
///
/// # Why this exists
///
/// The scroll area's rows are children of the area rather than window-level slots, but the
/// tree painter places a control at its **absolute** rectangle. Without this record the rows
/// kept the coordinates they were created with while the window layout moved the area, so
/// they were clipped away by the area's viewport and the demo showed an empty box. A follower
/// is the offset that keeps a child inside its host when the host moves.
#[derive(Debug, Clone, Copy)]
struct Follower {
    /// The slot whose rectangle the child follows.
    host: ObjectId,
    /// The child control.
    child: ObjectId,
    /// Offset from the host's top-left corner.
    dx: i32,
    /// Offset from the host's top-left corner.
    dy: i32,
    /// The child's own size.
    width: u32,
    /// The child's own size.
    height: u32,
}

/// Row 9's MDI area and the arrangement the user last chose.
///
/// Shared between the layout and the Shuffle button, which is what makes a resize re-apply the
/// arrangement instead of leaving the sub-window frames at the absolute positions they had when
/// the area was first mounted. A sub-window frame is stored in window coordinates (the same space
/// `add_sub_window`/`cascade_sub_windows` use), so it does not follow the area's own rectangle on
/// its own.
struct MdiRowState {
    /// The mounted area, once row 9 has one.
    area: Option<ObjectId>,
    /// `true` for a cascade arrangement, `false` for a tile arrangement.
    cascade: bool,
}

/// Window layout: fixed chrome plus a grid whose columns share the width by weight.
///
/// # Why this exists
///
/// Every control used to be created at absolute coordinates, and a resize left the controls
/// where they were while the window grew around them. A window layout re-runs the placement
/// on every size change (`app::handle` applies it after the platform queues `Resized`).
///
/// # Why the last applied size is remembered
///
/// One drag delivers the same client size many times, and each delivery reaches the layout.
/// Re-running the whole placement for an identical rectangle is invisible work that shows up
/// as a stutter; remembering the size makes a repeated delivery a no-op.
struct ControlGridLayout {
    /// Every control the layout positions.
    slots: Vec<Slot>,
    /// Controls that follow a slot's rectangle instead of owning one.
    followers: Vec<Follower>,
    /// Event log, so a placement pass is reported once.
    log: Arc<EventLog>,
    /// The last size this layout was run for; an identical size is skipped.
    ///
    /// `Cell` because `update` takes `&self`: a layout that cannot remember what it already
    /// did can only repeat it.
    last_applied: std::cell::Cell<Option<(u32, u32)>>,
    /// The size the last layout **log** line reported (DM4-2).
    ///
    /// A drag delivers a new client size every tick, so every tick reached the log and rewrote the
    /// four-row panel. The line is emitted at most once per [`LOG_SIZE_STEP`] pixels of change,
    /// which keeps the log readable and the panel untouched for the rest of the drag.
    last_logged: std::cell::Cell<Option<(u32, u32)>>,
    /// Scratch containers reused by one pass (DM4-2). `RefCell` for the same reason
    /// `last_applied` is a `Cell`: `update` takes `&self`. Cleared, never reallocated.
    scratch_rows: std::cell::RefCell<Vec<u16>>,
    scratch_heights: std::cell::RefCell<Vec<u32>>,
    scratch_placed: std::cell::RefCell<Vec<(ObjectId, Rect)>>,
    /// Indices into `slots` for the current row: the slot references cannot live in a field, so a
    /// row collects the indices and each accessor reads `self.slots[index]`.
    scratch_row_slots: std::cell::RefCell<Vec<usize>>,
    scratch_widths: std::cell::RefCell<Vec<u32>>,
    /// Row 9's MDI area and the arrangement to re-apply after the area is placed (GX2-1).
    mdi: Arc<Mutex<MdiRowState>>,
}

/// How much the client size must change before the layout writes another log line (DM4-2).
const LOG_SIZE_STEP: u32 = 32;

impl ControlGridLayout {
    /// Content rows that actually have slots, in ascending order, written into `out`.
    fn content_rows(&self, out: &mut Vec<u16>) {
        out.clear();
        out.extend(self.slots.iter().filter(|slot| slot.band == Band::Content).map(|s| s.row));
        out.sort_unstable();
        out.dedup();
    }

    /// The height each content row needs, before the available space is considered.
    fn nominal_row_heights(&self, rows: &[u16], out: &mut Vec<u32>) {
        out.clear();
        out.extend(rows.iter().map(|row| {
            self.slots
                .iter()
                .filter(|slot| slot.band == Band::Content && slot.row == *row)
                .map(|slot| slot.nominal.3)
                .max()
                .unwrap_or(1)
                .max(1)
        }));
    }

    /// Whether a size change is large enough to warrant another log line (DM4-2).
    fn should_log(&self, width: u32, height: u32) -> bool {
        match self.last_logged.get() {
            None => true,
            Some((w, h)) => {
                width.abs_diff(w) >= LOG_SIZE_STEP || height.abs_diff(h) >= LOG_SIZE_STEP
            }
        }
    }

    /// Fits `heights` and the gaps into `available`, shrinking both when they do not fit.
    ///
    /// Returns the heights and the gap to use. A compact grid that is centred in the
    /// available height reads better than one whose rows are spread proportionally: the old
    /// proportional bands made a tall row (the list) consume twice the empty space of a
    /// short one, which pulled the grid apart as the window grew.
    fn fit_rows(heights: &mut [u32], available: u32) -> u32 {
        if heights.is_empty() {
            return 0;
        }
        let gaps = (heights.len() as u32).saturating_sub(1);
        let nominal_total: u32 = heights.iter().sum();
        let wanted = nominal_total + ROW_GAP * gaps;
        if wanted <= available {
            return ROW_GAP;
        }
        // Gaps give way first: they are breathing room, not content.
        let room = available.saturating_sub(nominal_total);
        let gap = room.checked_div(gaps).map_or(0, |even| even.min(ROW_GAP));
        let still = nominal_total + gap * gaps;
        if still <= available {
            return gap;
        }
        // Then the rows themselves shrink proportionally. `max(1)` keeps every row drawable;
        // the rectangles may be tight, but none collapses to zero or turns negative.
        let usable = available.saturating_sub(gap * gaps).max(1);
        let scale = usable as f32 / nominal_total.max(1) as f32;
        for height in heights.iter_mut() {
            *height = ((*height as f32 * scale).floor() as u32).max(1);
        }
        gap
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
        // Slots are registered when the control is built; an id with no slot has no band and
        // placing it at `(0, 0)` would paint it over the menu bar.
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

        let width = rect.width.max(1);
        let height = rect.height.max(1);
        let status_h = STATUS_HEIGHT.min(height);
        let menu_h = MENU_HEIGHT.min(height.saturating_sub(status_h));
        let content_top = menu_h;
        let content_bottom = height.saturating_sub(status_h).max(content_top);

        let mut moved = 0usize;
        // Every placed rectangle, so a follower can be positioned from its host's final box
        // rather than from the host's nominal rectangle. Reused across passes (DM4-2).
        let mut placed_rects = self.scratch_placed.borrow_mut();
        placed_rects.clear();

        // ── Window chrome ────────────────────────────────────────────
        for slot in &self.slots {
            let placed = match slot.band {
                Band::Menu => Some(Rect::new(0, 0, width, menu_h.max(1))),
                Band::Status => Some(Rect::new(
                    0,
                    height.saturating_sub(status_h) as i32,
                    width,
                    status_h.max(1),
                )),
                Band::Content => None,
            };
            if let Some(placed) = placed {
                widgets(slot.id, placed);
                moved += 1;
                placed_rects.push((slot.id, placed));
            }
        }

        // ── Content grid ─────────────────────────────────────────────────
        let mut rows = self.scratch_rows.borrow_mut();
        self.content_rows(&mut rows);
        let mut heights = self.scratch_heights.borrow_mut();
        self.nominal_row_heights(&rows, &mut heights);
        let available = content_bottom.saturating_sub(content_top);
        let gap = Self::fit_rows(&mut heights, available);

        // Centre the compact grid in the space between the chrome bands. Extra room above
        // and below is a margin, not a stretched control: stretching a button's band does
        // not make the button taller (its height is its design height), it only moves it.
        let block = heights.iter().sum::<u32>() + gap * (rows.len() as u32).saturating_sub(1);
        let mut y = content_top as i32 + (available.saturating_sub(block) / 2) as i32;

        // The per-row slot list and column widths are scratch too (DM4-2). They hold indices, not
        // references, because a struct field cannot borrow `self.slots`.
        let mut row_slots = self.scratch_row_slots.borrow_mut();
        let mut widths = self.scratch_widths.borrow_mut();
        for (index, row) in rows.iter().enumerate() {
            let row_height = heights.get(index).copied().unwrap_or(1).max(1);
            row_slots.clear();
            row_slots.extend(
                self.slots
                    .iter()
                    .enumerate()
                    .filter(|(_, slot)| slot.band == Band::Content && slot.row == *row)
                    .map(|(index, _)| index),
            );
            if row_slots.is_empty() {
                continue;
            }
            let count = row_slots.len() as u32;
            let gaps = SLOT_GAP * count.saturating_sub(1);
            let weight_total: u32 =
                row_slots.iter().map(|index| self.slots[*index].weight).sum::<u32>().max(1);
            let fixed_width: u32 = row_slots
                .iter()
                .filter(|index| self.slots[**index].weight == 0)
                .map(|index| self.slots[*index].nominal.2)
                .sum();
            let flexible = width.saturating_sub(MARGIN * 2 + gaps + fixed_width).max(count);

            // Widths first, so the last weighted slot can absorb the rounding remainder and
            // the row ends exactly at the right margin instead of a few pixels short.
            widths.clear();
            widths.extend(row_slots.iter().map(|index| {
                let slot = &self.slots[*index];
                if slot.weight == 0 {
                    slot.nominal.2.max(1)
                } else {
                    (flexible * slot.weight / weight_total).max(1)
                }
            }));
            if let Some(last_weighted) =
                row_slots.iter().rposition(|index| self.slots[*index].weight > 0)
            {
                let planned: u32 = widths.iter().sum::<u32>() + gaps;
                let target = width.saturating_sub(MARGIN * 2);
                if planned < target {
                    widths[last_weighted] += target - planned;
                }
            }

            let mut x = MARGIN as i32;
            for (position, slot_index) in row_slots.iter().copied().enumerate() {
                let slot = &self.slots[slot_index];
                let slot_width = widths[position];
                // The control keeps its design height: a taller row band is vertical
                // breathing room, not permission to stretch a 32px button into a 90px one.
                let control_height = slot.nominal.3.min(row_height).max(1);
                // Top-align inside the row: a 26px field and a 90px list in one row then share
                // a top edge, which reads as a form. Centring made the short control float in
                // the middle of a tall row with no visual relationship to its neighbour.
                let control_y = y;
                widgets(slot.id, Rect::new(x, control_y, slot_width.max(1), control_height));
                moved += 1;
                placed_rects
                    .push((slot.id, Rect::new(x, control_y, slot_width.max(1), control_height)));
                x += slot_width as i32 + SLOT_GAP as i32;
            }
            y += row_height as i32 + gap as i32;
        }

        // Followers last: they need their host's final rectangle.
        for follower in &self.followers {
            if let Some((_, host)) = placed_rects.iter().find(|(id, _)| *id == follower.host) {
                widgets(
                    follower.child,
                    Rect::new(
                        host.x + follower.dx,
                        host.y + follower.dy,
                        follower.width.max(1),
                        follower.height.max(1),
                    ),
                );
                moved += 1;
            }
        }

        // Row 9's sub-window frames are stored in window coordinates, so a new area rectangle
        // does not move them. Re-applying the arrangement the user last chose is what keeps the
        // two sub-windows inside the area after a resize; the Shuffle button owns the choice.
        if moved > 0 {
            let (area, cascade) = {
                let state = self.mdi.lock().unwrap();
                (state.area, state.cascade)
            };
            if let Some(area_id) = area {
                let _ = rust_widgets::widget::runtime::with_widget_mut(area_id, |widget| {
                    let Some(area) = (widget as &mut dyn core::any::Any).downcast_mut::<MdiArea>()
                    else {
                        return;
                    };
                    if cascade {
                        area.cascade_sub_windows();
                    } else {
                        area.tile_sub_windows();
                    }
                });
            }
        }

        // DM4-2: one log line per coarse size change, not one per drag tick. The panel is four
        // labels, so every line costs up to four native writes on top of the `format!`.
        if moved > 0 && self.should_log(rect.width, rect.height) {
            self.last_logged.set(Some((rect.width, rect.height)));
            self.log.append(format!(
                "[Layout] window {}x{} -> {} control(s) placed",
                rect.width, rect.height, moved
            ));
        }
    }

    // The demo's grid places controls by its own slot model (bands + rows + per-slot weights),
    // not by what each child reports it wants. It therefore has no use for the hint channel or
    // the device context: forwarding keeps the one placement path authoritative, instead of
    // re-deriving geometry from `ChildInfo` the slots already encode.
    fn arrange(
        &self,
        rect: Rect,
        _children: &[rust_widgets::layout::ChildInfo],
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.update(rect, out);
    }

    fn arrange_with_context(
        &self,
        rect: Rect,
        _children: &[rust_widgets::layout::ChildInfo],
        _context: &rust_widgets::layout::LayoutContext,
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.update(rect, out);
    }

    fn update_with_context(
        &self,
        rect: Rect,
        _context: &rust_widgets::layout::LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.update(rect, widgets);
    }
}

/// Horizontal margin between the window edge and the grid, in pixels.
const MARGIN: u32 = 20;
/// Height of the menu bar band.
const MENU_HEIGHT: u32 = 28;
/// Height of the status bar band.
const STATUS_HEIGHT: u32 = 26;
/// Vertical gap between content rows.
const ROW_GAP: u32 = 12;
/// Horizontal gap between controls in the same row.
const SLOT_GAP: u32 = 10;

#[cfg(test)]
// The tests sit beside the layout they exercise so a reader meets the cases where the rules are
// stated. This is a demo crate, not a `src/` module: principle #9's 2000-line split bar is enforced
// by `tools/check_source_structure.sh` over the library, so the demo's own length is not a gate.
// The lint suppressed here is a file-layout preference, not a correctness signal.
#[allow(clippy::items_after_test_module)]
mod layout_tests {
    use super::*;

    /// 构造一个两行三列的版式：第 0 行三个等权控件，第 1 行一个满宽控件。
    fn slots() -> Vec<Slot> {
        vec![
            Slot { id: 1, band: Band::Content, row: 0, weight: 1, nominal: (20, 20, 100, 30) },
            Slot { id: 2, band: Band::Content, row: 0, weight: 1, nominal: (140, 20, 100, 30) },
            Slot { id: 3, band: Band::Content, row: 0, weight: 1, nominal: (260, 20, 100, 30) },
            Slot { id: 4, band: Band::Content, row: 1, weight: 1, nominal: (20, 60, 340, 30) },
        ]
    }

    fn layout() -> ControlGridLayout {
        ControlGridLayout {
            slots: slots(),
            followers: Vec::new(),
            log: Arc::new(EventLog::new()),
            last_applied: std::cell::Cell::new(None),
            last_logged: std::cell::Cell::new(None),
            scratch_rows: RefCell::new(Vec::new()),
            scratch_heights: RefCell::new(Vec::new()),
            scratch_placed: RefCell::new(Vec::new()),
            scratch_row_slots: RefCell::new(Vec::new()),
            scratch_widths: RefCell::new(Vec::new()),
            // No MDI row in these fixtures: the re-arrange block is a no-op without an id.
            mdi: Arc::new(Mutex::new(MdiRowState { area: None, cascade: true })),
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

        // The declared extent is what the control clamps against, and `scroll_to_bottom`
        // stops at the last screenful: `content - viewport`, not `content`. The old
        // assertion expected the offset to equal the content height, which contradicted the
        // control's own documented clamp (`set_scroll_position` clamps to `content - view`)
        // and could only pass while the clamp was broken.
        assert_eq!(handle.content_size(), (260, CONTENT_H), "声明的范围必须被记录");
        // Compile-time assertion: the fixture only has something to scroll when the content is
        // taller than the viewport it is declared against.
        const { assert!(CONTENT_H > 90, "内容必须高于视口，否则无滚可滚") };
        handle.scroll_to_bottom();
        assert_eq!(
            handle.scroll_position().1,
            (CONTENT_H - 90) as i32,
            "滚到底的偏移必须是「内容高 - 视口高」，且高于 0"
        );

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
fn register_window_layout(
    win: &WindowHandle,
    slots: Vec<Slot>,
    followers: Vec<Follower>,
    log: &Arc<EventLog>,
    mdi: Arc<Mutex<MdiRowState>>,
) {
    win.set_layout(ControlGridLayout {
        slots,
        followers,
        log: Arc::clone(log),
        last_applied: std::cell::Cell::new(None),
        last_logged: std::cell::Cell::new(None),
        scratch_rows: RefCell::new(Vec::new()),
        scratch_heights: RefCell::new(Vec::new()),
        scratch_placed: RefCell::new(Vec::new()),
        scratch_row_slots: RefCell::new(Vec::new()),
        scratch_widths: RefCell::new(Vec::new()),
        mdi,
    });
}

#[cfg(test)]
mod log_tests {
    use super::*;

    /// The log keeps a bounded tail rather than growing with the event count.
    ///
    /// Nothing in this demo ever drops a control or a window, so before the cap an evening of
    /// hovering and clicking grew the `Vec` without limit.
    #[test]
    fn the_event_log_keeps_only_its_most_recent_entries() {
        let log = EventLog::new();
        for index in 0..EVENT_LOG_CAPACITY + 50 {
            log.append(format!("event {index}"));
        }
        let entries = log.snapshot();
        assert_eq!(entries.len(), EVENT_LOG_CAPACITY, "the log must stop growing at its cap");
        assert_eq!(
            entries.first().map(String::as_str),
            Some("event 50"),
            "the oldest entries must be the ones dropped"
        );
        assert_eq!(
            entries.last().cloned(),
            Some(format!("event {}", EVENT_LOG_CAPACITY + 49)),
            "the newest entry must be the last one kept"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// In-memory save state — the 2.9.1 capability showcase
// ═══════════════════════════════════════════════════════════════════════════════

/// Key naming the pagination inside a saved snapshot.
const PAGINATION_KEY: &str = "demo.pagination";
/// Key naming the tab widget inside a saved snapshot.
const TABS_KEY: &str = "demo.tabs";

thread_local! {
    /// The most recent snapshot taken by the "Save state" button.
    ///
    /// `Rc<RefCell<..>>` is enough because the snapshot never leaves the loop thread: the widget
    /// registry, the layout and every control callback belong to it. The buttons reach the cell
    /// through this thread-local instead of capturing the `Rc`, because
    /// `WidgetHandle::on_click` requires a `Send` closure and an `Rc` is not `Send`.
    static SAVED_STATES: Rc<RefCell<Option<Vec<SavedState>>>> = Rc::new(RefCell::new(None));
}

/// The demo's whole save-state naming decision.
///
/// Only the two controls this demonstration persists answer with a key; every other widget in
/// the window answers `None`, so it contributes no bytes to the snapshot.
fn demo_key(pagination: Option<ObjectId>, tabs: ObjectId) -> impl Fn(ObjectId) -> Option<String> {
    move |id| {
        if Some(id) == pagination {
            Some(PAGINATION_KEY.to_string())
        } else if id == tabs {
            Some(TABS_KEY.to_string())
        } else {
            None
        }
    }
}

/// Collects the demo's opted-in controls into one snapshot.
///
/// Both controls are reachable from the window id: the tab widget is a tree child, and the
/// runtime records a surface's owning window, which the collector's walk follows. Sorting by
/// key makes the log line stable regardless of traversal order.
fn collect_demo_state(
    window: ObjectId,
    key_of: impl Fn(ObjectId) -> Option<String>,
) -> Vec<SavedState> {
    let mut states = collect_save_state(window, &key_of);
    states.sort_by(|left, right| left.key.cmp(&right.key));
    states
}

/// Applies a snapshot to the same two controls and returns how many accepted it.
fn restore_demo_state(
    window: ObjectId,
    states: &[SavedState],
    key_of: impl Fn(ObjectId) -> Option<String>,
) -> usize {
    restore_save_state(window, states, &key_of)
}

/// Deterministic "random-ish" bar values for the chart's shuffle button.
///
/// A hash of the round number and the bar index rather than a random-number dependency: the
/// values are reproducible, so the log line can name exactly the numbers that started the
/// easing. The labels never change, which is what lets `set_bars` ease instead of jumping.
fn shuffled_bars(round: u64) -> Vec<BarEntry> {
    (0..8u64)
        .map(|index| {
            let mixed =
                round.wrapping_mul(0x9E37_79B9).wrapping_add(index.wrapping_mul(0x85EB_CA6B));
            // 10..=100 keeps every bar visible while still moving each one a long way.
            BarEntry::new(format!("B{}", index + 1), 10.0 + (mixed % 91) as f64)
        })
        .collect()
}

/// Advances the shuffle counter and returns the new round.
///
/// The counter is shared with the button closure, so it lives behind a `Mutex`. A poisoned lock
/// is recovered rather than panicking: a demo must not die because an earlier callback failed.
fn next_shuffle_round(counter: &Mutex<u64>) -> u64 {
    let mut round = match counter.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let next = round.wrapping_add(1);
    *round = next;
    next
}

/// The pagination's page, one-based, when the control is mounted.
fn pagination_page(id: Option<ObjectId>) -> Option<usize> {
    let id = id?;
    with_widget(id, |widget| {
        (widget as &dyn core::any::Any).downcast_ref::<Pagination>().map(|bar| bar.page() + 1)
    })
    .flatten()
}

/// The tab widget's selected index, when the control is mounted.
fn selected_tab(id: ObjectId) -> Option<usize> {
    with_widget(id, |widget| {
        (widget as &dyn core::any::Any)
            .downcast_ref::<TabWidget>()
            .and_then(|tabs| tabs.selected_index())
    })
    .flatten()
}

// ═══════════════════════════════════════════════════════════════════════════════
// 构建所有控件 + 日志面板
// ═══════════════════════════════════════════════════════════════════════════════

/// What [`build_all_controls`] produced.
///
/// `slots` is what the layout positions; `mdi` is row 9's MDI state, which the layout and the
/// Shuffle button share (see [`MdiRowState`]). Returning it rather than reaching for a global
/// keeps every window's row 9 independent, which is what a test with more than one window needs.
struct BuiltControls {
    slots: Vec<Slot>,
    mdi: Arc<Mutex<MdiRowState>>,
}

fn build_all_controls(
    win: &WindowHandle,
    log: &Arc<EventLog>,
    followers: &mut Vec<Follower>,
) -> BuiltControls {
    // Each control registers its own row here so the window layout can re-place it on resize.
    // The row number only says "same horizontal band"; columns share the row's width by weight.
    let mut slots: Vec<Slot> = Vec::new();
    // One rectangle, one control.
    //
    // The rectangle is written once. It is used twice — for the constructor call and for the
    // layout's nominal box — so the control's real position and the rectangle the layout knows
    // about cannot drift; previously the same `(x, y, w, h)` literal appeared in both places at
    // every call site, and one edited copy silently disagreed with the other.
    //
    // `$constructor` names a `WindowHandle` method whose last four parameters are `(x, y, w, h)`;
    // every constructor used for a content row has that shape. The handle is bound once, so the
    // expression that builds the control still runs exactly once (an earlier version expanded
    // `$handle` at each use, which constructed the control twice).
    macro_rules! row {
        ($row:expr, $weight:expr, ($x:expr, $y:expr, $w:expr, $h:expr), $constructor:ident ( $($arg:expr),* $(,)? )) => {{
            let nominal = ($x, $y, $w, $h);
            let handle = WindowHandle::$constructor(
                win,
                $($arg,)*
                nominal.0,
                nominal.1,
                nominal.2,
                nominal.3,
            );
            slots.push(Slot {
                id: handle.raw_id(),
                band: Band::Content,
                row: $row,
                weight: $weight,
                nominal,
            });
            handle
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
        "演示信息",
        "这是一个演示消息框。\n它由「点击我」按钮打开。",
        20,
        386,
        300,
        100,
    );
    dialog.set_title("提示");
    let l = Arc::clone(log);
    let dialog_close = dialog.clone();
    dialog.on_click(move || {
        l.append("[MessageBox] button clicked -> closing");
        dialog_close.close();
    });
    log.append("[MessageBox] created (hidden; opened by 'Click Me')");

    // ── Row 0: Button Controls ───────────────────────────────────────
    log.append("=== Row: Button Controls ===");

    let btn = row!(0, 1, (20, 20, 150, 32), new_button("点击我"));
    let l = Arc::clone(log);
    let dialog_for_click = dialog.clone();
    btn.on_click(move || {
        l.append("[Button] clicked! -> showing dialog");
        dialog_for_click.show_modal();
    });
    log.append("[Button] at (20,20,150,32)");

    let tog = row!(0, 1, (180, 20, 150, 32), new_button("深色模式"));
    let l = Arc::clone(log);
    let tog_label = tog.clone();
    tog.on_click(move || {
        let dark = !preview_common::dark_active();
        preview_common::apply_appearance_mode(dark);
        let name = if dark { preview_common::MACOS_DARK } else { preview_common::MACOS_LIGHT };
        l.append(format!("[Theme] switched to '{name}'"));
        tog_label.set_text(if preview_common::dark_active() {
            "浅色模式"
        } else {
            "深色模式"
        });
    });
    // A `Button` that happens to toggle appearance, not a `ToggleButton`: this crate publishes
    // no `new_toggle_button`, and the log tag must name the control that is actually here so a
    // reader looking for it in the log is not sent after a family that does not exist.
    log.append("[Button] at (180,20,150,32) (toggles appearance)");

    // Disabled button: proves the enable/disable mirror reaches the native control.
    let disabled = row!(0, 1, (340, 20, 150, 32), new_button("已禁用"));
    disabled.disable();
    log.append(format!("[Button] disabled sample enabled={}", disabled.is_enabled()));

    // ── Row 1: Toggle Controls ───────────────────────────────────────
    log.append("=== Row: Toggle Controls ===");

    let cb = row!(1, 1, (20, 64, 200, 24), new_checkbox("启用通知"));
    let l = Arc::clone(log);
    let cb2 = cb.clone();
    cb2.on_value_changed(move |_val: &str| {
        let checked = cb.is_checked();
        l.append(format!("[CheckBox] checked={}", checked));
    });
    log.append("[CheckBox] at (20,64,200,24)");

    // Tri-state check-box: all three states are reachable (Unchecked / Checked /
    // PartiallyChecked), which the mixed state requires tri-state mode to be on.
    let tri = row!(1, 1, (230, 64, 130, 24), new_checkbox("三态"));
    tri.set_tristate(true);
    tri.set_check_state(rust_widgets::app::CheckState::PartiallyChecked);
    log.append(format!(
        "[CheckBox] tri-state enabled={:?} state={:?} is_checked={}",
        tri.is_tristate(),
        tri.check_state(),
        tri.is_checked()
    ));

    // ── Row 2: Text Input Controls ───────────────────────────────────
    log.append("=== Row: Text Input Controls ===");

    let le = row!(2, 3, (20, 104, 200, 26), new_line_edit(""));
    le.set_placeholder("在此输入");
    le.set_max_length(32);
    let l = Arc::clone(log);
    let le2 = le.clone();
    le2.on_value_changed(move |_val: &str| {
        l.append(format!("[LineEdit] text='{}'", le.text()));
    });
    log.append("[LineEdit] placeholder + max_length=32");

    // Password field: echo mode is a real widget property; a platform whose text
    // control cannot express it reports so via `false`/`None` instead of lying.
    let pw = row!(2, 2, (230, 104, 150, 26), new_line_edit("secret"));
    pw.set_echo_mode(rust_widgets::app::EchoMode::Password);
    log.append(format!("[LineEdit] echo_mode={:?}", pw.widget_echo_mode()));

    let ro = row!(2, 3, (390, 104, 180, 26), new_line_edit("只读值"));
    ro.set_read_only(true);
    log.append(format!("[LineEdit] read_only={:?}", ro.clone().is_read_only()));

    let sb = row!(2, 2, (580, 104, 120, 26), new_spin_box());
    sb.set_range(0, 100);
    sb.set_value(50);
    sb.set_prefix("$");
    sb.set_suffix(".00");
    sb.set_step(5);
    let l = Arc::clone(log);
    let sb2 = sb.clone();
    sb2.on_value_changed(move |_val: &str| {
        l.append(format!("[SpinBox] value={}", sb.value()));
    });
    log.append("[SpinBox] range=[0..100], value=50, step=5");

    // ── Row 3: Selection Controls ────────────────────────────────────
    log.append("=== Row: Selection Controls ===");

    let cbx = row!(3, 2, (20, 142, 180, 26), new_combo_box());
    cbx.add_item("红");
    cbx.add_item("绿");
    cbx.add_item("蓝");
    cbx.add_item("黄");
    cbx.set_current_index(0);
    let l = Arc::clone(log);
    let cbx2 = cbx.clone();
    cbx2.on_value_changed(move |_val: &str| {
        let idx = cbx.current_index().unwrap_or(0);
        let text = cbx.item_text(idx).unwrap_or_else(|| String::from("(none)"));
        l.append(format!("[ComboBox] selected '{}' (idx={})", text, idx));
    });
    log.append("[ComboBox] items=[红,绿,蓝,黄]");

    let lb = row!(3, 3, (210, 142, 200, 90), new_list_box());
    for item in ["苹果", "香蕉", "橙子", "葡萄", "西瓜"] {
        lb.add_item(item);
    }
    lb.set_current_index(0);
    let l = Arc::clone(log);
    let lb2 = lb.clone();
    let lb3 = lb.clone();
    lb2.on_value_changed(move |_val: &str| {
        let idx = lb3.current_index().unwrap_or(0);
        let text = lb3.item_text(idx).unwrap_or_else(|| String::from("(none)"));
        l.append(format!("[ListBox] selected '{}' (idx={})", text, idx));
    });
    log.append(format!("[ListBox] {} items", lb.item_count()));

    // Radio group: one horizontal row beside the toggle controls. Three vertical rows were
    // three separate "rows" to the old layout, which placed them at the same x and let them
    // collide with the list box; a group is one row with three equal shares.
    let rb1 = row!(1, 1, (420, 64, 100, 24), new_radio_button("选项 A"));
    rb1.set_group("opts");
    let rb2 = row!(1, 1, (530, 64, 100, 24), new_radio_button("选项 B"));
    rb2.set_group("opts");
    let rb3 = row!(1, 1, (640, 64, 100, 24), new_radio_button("选项 C"));
    rb3.set_group("opts");
    let l = Arc::clone(log);
    let r2 = rb2.clone();
    r2.on_value_changed(move |_val: &str| {
        if rb2.is_selected() {
            l.append("[RadioButton] 选项 B 已选中");
        }
    });
    log.append("[RadioButton] 3 options in group 'opts'");

    // ── Row 4: Range Controls ────────────────────────────────────────
    log.append("=== Row: Range Controls ===");

    // Orientation is a creation-time property on every native toolkit (Win32 has
    // no runtime message for it), so it is passed at construction instead of being
    // set afterwards.
    let sl = row!(4, 3, (20, 242, 260, 32), new_slider_with_orientation(Orientation::Horizontal));
    sl.set_range(0, 100);
    sl.set_value(50);
    sl.set_step(5);
    let l = Arc::clone(log);
    let sl2 = sl.clone();
    let sl3 = sl.clone();
    sl2.on_value_changed(move |_val: &str| {
        l.append(format!("[Slider] value={}", sl3.value()));
    });
    log.append(format!("[Slider] value=50 orientation={:?}", sl.orientation()));

    let pb = row!(4, 2, (290, 242, 200, 24), new_progress_bar());
    pb.set_min(0u32);
    pb.set_max(100u32);
    pb.set_value(75u32);
    let l = Arc::clone(log);
    let pb2 = pb.clone();
    pb2.on_value_changed(move |_val: &str| {
        l.append(format!("[ProgressBar] value={}", pb.value()));
    });
    log.append("[ProgressBar] range=[0..100], value=75");

    // Indeterminate (busy) bar: a distinct native code path from a fixed fraction.
    let busy = row!(4, 2, (500, 242, 200, 24), new_progress_bar());
    busy.set_indeterminate(true);
    log.append(format!("[ProgressBar] indeterminate={:?}", busy.is_indeterminate()));

    // ── Method-driven update: 一个按钮用**方法**改写其它控件 ────────
    //
    // 前面的行演示的是“控件 → 日志”方向（事件）。这里演示反向：宿主主动调用控件
    // 方法，而且是**先读后写**。两个方向都要有例子—— “能改”与“能读回”是两件事，
    // 只演示其中一侧会让人误以为另一侧不存在。
    //
    // 放在这里是因为它要引用 `tri`/`sl`/`busy`/`rb3`，四个控件都已创建。
    let sync = row!(0, 1, (540, 20, 140, 32), new_button("同步"));
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
    log.append("[Button] 'Sync' reads then writes 4 other controls (method path)");

    // ── Row 5: Scrollable Text Area ──────────────────────────────────
    log.append("=== Row: Scrollable Text Area ===");

    let area = row!(5, 3, (20, 286, 260, 90), new_scroll_area());
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
        // The separator is a fullwidth colon, which is inside the compiled CJK subset, so these
        // rows stay legible on a host that registered no system face (see `register_cjk_face`).
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
            &format!("第 {i} 行：滚动我"),
            4,
            4 + i * 20,
            244,
            18,
        );
        // The scroll area draws its own content frame; each row must still be
        // parented before it can be seen, and reporting a failure to mount keeps a
        // silently-empty area from looking like a layout accent.
        if label == 0 {
            log.append(format!("[ScrollArea] WARN: content row {i} failed to mount"));
        } else {
            followers.push(Follower {
                host: area.raw_id(),
                child: label,
                dx: 4,
                dy: 4 + i * 20,
                width: 244,
                height: 18,
            });
        }
    }
    log.append(format!(
        "[ScrollArea] viewport (20,286,260,90) with content 260x{content_h} and 6 stacked labels"
    ));

    // ── Row 6: Panel / Frame ─────────────────────────────────────────
    log.append("=== Row: Panel / Frame ===");

    let panel = row!(5, 2, (290, 286, 200, 90), new_panel());
    panel.set_title("面板");
    let frame = row!(5, 2, (500, 286, 200, 90), new_frame());
    frame.set_text("分组框");
    log.append(format!("[Panel] id={:?}  [Frame] id={:?}", panel.raw_id(), frame.raw_id()));

    // ── Row 6: Pagination / TabWidget (the new 2.9.1 cells) ──────
    log.append("=== Row: Pagination / TabWidget ===");

    let mut pagination = Pagination::new(Rect::new(20, 396, 500, 32));
    pagination.set_total(245);
    pagination.set_page_size(20);
    pagination.set_page_size_options(vec![10, 20, 50]);
    // Both trailing cells are opt-in: the jump-to-page cell and the page-size selector are the
    // two capabilities this row exists to show.
    pagination.set_show_jump_input(true);
    pagination.set_show_page_size_selector(true);
    let l = Arc::clone(log);
    pagination.page_changed.connect(move |page| {
        // The control counts from zero; the log counts pages the way a reader does.
        let page = *page;
        l.append(format!("[Pagination] page={} (zero-based {page})", page + 1));
    });
    let pagination_id = match win.mount_surface(Box::new(pagination), Rect::new(20, 396, 500, 32)) {
        Ok(surface) => {
            let id = surface.raw_id();
            slots.push(Slot {
                id,
                band: Band::Content,
                row: 6,
                weight: 5,
                nominal: (20, 396, 500, 32),
            });
            log.append(format!(
                "[Pagination] mounted id={id:?} total=245 page_size=20 options=[10,20,50] \
                 jump+size cells on"
            ));
            Some(id)
        }
        Err(error) => {
            log.append(format!("[Pagination] mount failed ({error}); the paging row is not shown"));
            None
        }
    };

    let tabs = win.new_tab_widget(540, 396, 300, 32);
    for title in ["One", "Two"] {
        match tabs.add_tab(title, None) {
            Some(index) => log.append(format!("[TabWidget] tab '{title}' at index {index}")),
            None => log.append(format!("[TabWidget] WARN: tab '{title}' could not be added")),
        }
    }
    slots.push(Slot {
        id: tabs.raw_id(),
        band: Band::Content,
        row: 6,
        weight: 3,
        nominal: (540, 396, 300, 32),
    });
    let tabs_id = tabs.raw_id();
    let l = Arc::clone(log);
    let wired = with_widget_mut(tabs_id, |widget| {
        let Some(tabs) = (widget as &mut dyn core::any::Any).downcast_mut::<TabWidget>() else {
            return false;
        };
        tabs.current_changed.connect(move |index| {
            l.append(format!("[TabWidget] selected index={}", *index));
        });
        true
    })
    .unwrap_or(false);
    if wired {
        log.append(format!("[TabWidget] id={tabs_id:?} with tabs One/Two; selection log wired"));
    } else {
        log.append(format!(
            "[TabWidget] WARN: id={tabs_id:?} is not a TabWidget; selection log is not wired"
        ));
    }

    // ── Row 7: BarChart with eased value changes ────────────────
    log.append("=== Row: BarChart (eased value changes) ===");

    let mut chart = BarChart::new(Rect::new(20, 436, 680, 110));
    chart.set_animate_value_changes(true);
    chart.set_bars(shuffled_bars(0));
    // Exercise the engine's headless SVG renderer, but only when asked (DM4-6). This runs inside
    // `build_all_controls`, which the offscreen preview also calls, so an unconditional export made
    // every preview serialise the whole chart and write a temp file that nothing reads. The gate is
    // the same shape finance's paint check uses; unset is the ordinary demo path.
    if std::env::var_os(EXPORT_SVG_ENV).is_some() {
        let svg_path = std::env::temp_dir().join("rust_widgets_demo_bar_chart.svg");
        match chart.export_svg(&svg_path.to_string_lossy()) {
            Ok(()) => log.append(format!("[BarChart] engine SVG export -> {}", svg_path.display())),
            Err(error) => log.append(format!("[BarChart] engine SVG export failed: {error}")),
        }
    }
    let chart_id = match win.mount_surface(Box::new(chart), Rect::new(20, 436, 680, 110)) {
        Ok(surface) => {
            let id = surface.raw_id();
            slots.push(Slot {
                id,
                band: Band::Content,
                row: 7,
                weight: 1,
                nominal: (20, 436, 680, 110),
            });
            log.append(format!("[BarChart] mounted id={id:?} with 8 bars; value easing on"));
            Some(id)
        }
        Err(error) => {
            log.append(format!("[BarChart] mount failed ({error}); the chart row is not shown"));
            None
        }
    };

    // ── Row 8: Shuffle / Save state / Restore state ─────────────
    log.append("=== Row: State Demo (shuffle + save/restore) ===");

    let window_id = win.raw_id();
    let shuffle_round = Arc::new(Mutex::new(0u64));
    // The MDI area mounts in the development-patterns row below, after this button exists. The
    // button reads the id through this slot when it runs, which is long after the mount. The same
    // slot carries the arrangement, so the window layout can re-apply the user's choice after a
    // resize. `Arc<Mutex<..>>` rather than `Rc<Cell<..>>` because `on_click` takes a `Send`
    // closure.
    let mdi_target: Arc<Mutex<MdiRowState>> =
        Arc::new(Mutex::new(MdiRowState { area: None, cascade: true }));

    let shuffle = row!(8, 1, (20, 556, 200, 32), new_button("Shuffle data"));
    let mdi_arrange = Arc::clone(&mdi_target);
    let l = Arc::clone(log);
    let round_counter = Arc::clone(&shuffle_round);
    let chart_target = chart_id;
    let pager_target = pagination_id;
    let tabs_target = tabs_id;
    shuffle.on_click(move || {
        let round = next_shuffle_round(&round_counter);
        let bars = shuffled_bars(round);
        let values: Vec<String> = bars.iter().map(|bar| format!("{:.0}", bar.value)).collect();
        let eased = match chart_target {
            Some(id) => with_widget_mut(id, |widget| {
                let Some(chart) = (widget as &mut dyn core::any::Any).downcast_mut::<BarChart>()
                else {
                    return false;
                };
                // Same labels, new values: `set_bars` can ease from what is on screen.
                chart.set_bars(bars);
                true
            })
            .unwrap_or(false),
            None => false,
        };
        l.append(format!(
            "[BarChart] shuffle round {round}: values=[{}] accepted={eased}",
            values.join(", ")
        ));

        // Move both persisted controls away from the saved state, so a later Restore has
        // something visible to return.
        if let Some(id) = pager_target {
            let page = with_widget_mut(id, |widget| {
                let bar = (widget as &mut dyn core::any::Any).downcast_mut::<Pagination>()?;
                let next = if bar.page() >= bar.last_page() { 0 } else { bar.page() + 1 };
                bar.set_page(next);
                Some(next + 1)
            })
            .flatten();
            match page {
                Some(page) => l.append(format!("[Pagination] shuffle -> page {page}")),
                None => l.append("[Pagination] shuffle could not reach the control"),
            }
        }

        let selected = with_widget_mut(tabs_target, |widget| {
            let tabs = (widget as &mut dyn core::any::Any).downcast_mut::<TabWidget>()?;
            if tabs.count() < 2 {
                return None;
            }
            let next = if tabs.selected_index() == Some(0) { 1 } else { 0 };
            tabs.set_current_index(next);
            Some(next)
        })
        .flatten();
        match selected {
            Some(index) => l.append(format!("[TabWidget] shuffle -> index {index}")),
            None => l.append("[TabWidget] shuffle could not move the selection"),
        }

        // GX2-1: the same button arranges the MDI sub-windows, alternating cascade and tile so one
        // click shows both layouts without a second control. The choice is stored in the shared
        // row state, which is also what a resize re-applies; deriving it from `round % 2` here
        // would let the two paths disagree after any resize.
        let (mdi_id, cascade) = {
            let mut state = mdi_arrange.lock().unwrap();
            state.cascade = !state.cascade;
            (state.area, state.cascade)
        };
        let arranged = mdi_id.and_then(|id| {
            with_widget_mut(id, |widget| {
                let area = (widget as &mut dyn core::any::Any).downcast_mut::<MdiArea>()?;
                if cascade {
                    area.cascade_sub_windows();
                } else {
                    area.tile_sub_windows();
                }
                Some(area.sub_window_count())
            })
            .flatten()
        });
        match arranged {
            Some(count) => {
                // The area and its sub-windows are painted by the mounted surface's own frame.
                // The runtime's invalidation is what asks the platform to repaint it: the
                // button's click changes no state fact of the area, so nothing else submits
                // the frame. (A control inside a window does not need this: the window's
                // invalidation covers it.)
                if let Some(id) = mdi_id {
                    rust_widgets::widget::runtime::request_repaint(id);
                }
                l.append(format!(
                    "[MdiArea] {} -> {count} sub-window(s)",
                    if cascade { "cascade" } else { "tile" }
                ))
            }
            None => l.append("[MdiArea] shuffle could not reach the area"),
        }
    });
    log.append("[Button] 'Shuffle data' updates the chart and both saved controls");

    let save = row!(8, 1, (230, 556, 180, 32), new_button("Save state"));
    let l = Arc::clone(log);
    save.on_click(move || {
        let states = collect_demo_state(window_id, demo_key(pager_target, tabs_target));
        let keys = states.iter().map(|entry| entry.key.as_str()).collect::<Vec<_>>().join(", ");
        let page = pagination_page(pager_target);
        let tab = selected_tab(tabs_target);
        l.append(format!("[SaveState] saved {} entries: [{}]", states.len(), keys));
        l.append(format!("[SaveState] snapshot page={page:?} tab={tab:?}"));
        // The tab order is part of the UI state this button reports, and reading it here is the
        // demo's real use of the runtime's focusable list (RT2-04).
        let tab_order = rust_widgets::widget::runtime::focusable_widgets();
        l.append(format!("[SaveState] tab order: {} focusable control(s)", tab_order.len()));
        SAVED_STATES.with(|slot| *slot.borrow_mut() = Some(states));
    });

    let restore = row!(8, 1, (420, 556, 180, 32), new_button("Restore state"));
    let l = Arc::clone(log);
    restore.on_click(move || {
        let saved = SAVED_STATES.with(|slot| slot.borrow().clone());
        let Some(states) = saved else {
            l.append("[SaveState] nothing saved yet; press 'Save state' first");
            return;
        };
        let applied = restore_demo_state(window_id, &states, demo_key(pager_target, tabs_target));
        let page = pagination_page(pager_target);
        let tab = selected_tab(tabs_target);
        l.append(format!(
            "[SaveState] restore applied {applied}/{} widgets; page={page:?} tab={tab:?}",
            states.len()
        ));
    });

    // ── Row 9: Development patterns (AdaptiveScaffold / MdiArea) (GX2-1) ──
    //
    // The two composition paradigms the library ships were only visible in the docs. They are
    // ordinary controls, so the demo mounts them the way it mounts every other self-painted
    // control: on a surface, and registered with the window layout so a resize moves them.
    log.append("=== Row: Development patterns (AdaptiveScaffold / MdiArea) ===");

    // Narrow window: bottom navigation bar; wide window: navigation rail. Dragging the demo
    // window across the breakpoint (599/840 px) shows the form switch.
    let mut scaffold = AdaptiveScaffold::new("移动脚手架", Rect::new(20, 0, 340, 90));
    scaffold.set_show_bottom_nav(true);
    scaffold.add_nav_item("home", "首页");
    scaffold.add_nav_item("search", "搜索");
    scaffold.add_nav_item("settings", "设置");
    let scaffold_id = match win.mount_surface(Box::new(scaffold), Rect::new(20, 0, 340, 90)) {
        Ok(surface) => {
            let id = surface.raw_id();
            slots.push(Slot {
                id,
                band: Band::Content,
                row: 9,
                weight: 1,
                nominal: (20, 0, 340, 90),
            });
            log.append(format!(
                "[AdaptiveScaffold] mounted id={id:?} with 3 bottom-nav items (narrow) / rail (wide)"
            ));
            Some(id)
        }
        Err(error) => {
            log.append(format!("[AdaptiveScaffold] mount failed ({error}); the row is not shown"));
            None
        }
    };

    // Desktop MDI: two sub-windows referencing two ordinary labels. `add_sub_window` reparents
    // them into the area, so the labels supply the sub-window contents and titles; the Shuffle
    // button below cascades/tiles the frames.
    let mut area = MdiArea::new(Rect::new(370, 0, 320, 90));
    let first = win.new_label("项目 A", 0, 0, 1, 1);
    let second = win.new_label("项目 B", 0, 0, 1, 1);
    // `add_sub_window` takes the frame in the area's own (absolute) coordinates — the same
    // space `cascade_sub_windows` computes from the area's geometry — so the frames are laid
    // out by the area instead of hand-written numbers. A first `cascade` is what puts the two
    // windows inside the area before the first frame is painted; tile/cascade from the Shuffle
    // button then re-uses the same arithmetic.
    area.add_sub_window(first.raw_id(), Rect::new(0, 0, 150, 64));
    area.add_sub_window(second.raw_id(), Rect::new(0, 0, 150, 64));
    area.cascade_sub_windows();
    area.set_sub_window_title(first.raw_id(), "项目 A".to_string());
    area.set_sub_window_title(second.raw_id(), "项目 B".to_string());
    let mdi_id = match win.mount_surface(Box::new(area), Rect::new(370, 0, 320, 90)) {
        Ok(surface) => {
            let id = surface.raw_id();
            slots.push(Slot {
                id,
                band: Band::Content,
                row: 9,
                weight: 1,
                nominal: (370, 0, 320, 90),
            });
            log.append(format!(
                "[MdiArea] mounted id={id:?} with 2 sub-windows; Shuffle arranges them"
            ));
            Some(id)
        }
        Err(error) => {
            log.append(format!("[MdiArea] mount failed ({error}); the row is not shown"));
            None
        }
    };

    // ── Log Panel (底部 4 行标签) ─────────────────────────────────────
    log.append("=== Log Panel ===");

    let _title = row!(10, 1, (20, 496, 680, 18), new_label("事件日志"));
    let mut log_rows = Vec::new();
    for i in 0..4 {
        let entry = row!(11 + i as u16, 1, (20, 518 + i * 20, 680, 18), new_label(""));
        log_rows.push(entry);
    }
    log.attach_rows(log_rows);
    log.append("[LogPanel] 4 label rows at bottom");

    // Report the paradigm row's ids so the Shuffle wiring below can reach the MDI area. The
    // scaffold id is only logged: its form switches on the window width by itself.
    let _ = scaffold_id;
    *mdi_target.lock().unwrap() = MdiRowState { area: mdi_id, cascade: true };

    BuiltControls { slots, mdi: mdi_target }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 工具栏 / 状态栏
// ═══════════════════════════════════════════════════════════════════════════════

/// Creates the window's chrome and returns the slots that place it.
///
/// The menu bar and status bar are ordinary controls in the same flat drawing
/// surface as the rest of the demo; the layout is what reserves their bands. Returning each
/// handle's slot (instead of only logging its id) is what keeps the menu bar pinned to the
/// top and the status bar at the bottom as the window resizes.
fn build_window_chrome(win: &WindowHandle, log: &Arc<EventLog>) -> Vec<Slot> {
    log.append("=== Window Chrome ===");
    let mut slots = Vec::new();

    // ── Menu bar ─────────────────────────────────────────────────────
    //
    // `new_menu` appends its title to the bar (that is what made this menu bar worth
    // drawing at all), and the host opens the matching popup when the title is chosen.
    // `entry_rect` is read back rather than guessed so the popup sits flush under the
    // entry the user clicked, in the same way AppKit places a menu.
    let menu_bar = win.new_menu_bar(0, 0, 760, MENU_HEIGHT);
    slots.push(Slot {
        id: menu_bar.raw_id(),
        band: Band::Menu,
        row: 0,
        weight: 1,
        nominal: (0, 0, 760, MENU_HEIGHT),
    });

    let file = win.new_menu(&menu_bar, "File", 0, 0, 200, 0);
    let edit = win.new_menu(&menu_bar, "Edit", 0, 0, 200, 0);
    let view = win.new_menu(&menu_bar, "View", 0, 0, 200, 0);
    for (menu, items) in [
        (&file, [("New", "Ctrl+N"), ("Open", "Ctrl+O"), ("Save", "Ctrl+S")]),
        (&edit, [("Cut", "Ctrl+X"), ("Copy", "Ctrl+C"), ("Paste", "Ctrl+V")]),
        (&view, [("Zoom In", "Ctrl++"), ("Zoom Out", "Ctrl+-"), ("Actual Size", "Ctrl+0")]),
    ] {
        for (label, chord) in items {
            win.new_menu_item(menu, label, Some(chord));
        }
        let l = Arc::clone(log);
        menu.on_triggered(move |label: String| {
            l.append(format!("[Menu] {label} chosen"));
        });
    }

    // The popup follows the entry. `View` is the one menu with a real action: it flips
    // the appearance, so the menu is not a row of inert labels. The dispatch key is the row's
    // **action id**, not its label: the `on_triggered` handler in the loop above still logs the
    // visible text for every row, while this handler flips the appearance only for the row named
    // below (DM3-2).
    let l = Arc::clone(log);
    view.on_item_action_triggered(move |action_id: String| {
        if action_id == "view.toggle-appearance" {
            let dark = !preview_common::dark_active();
            preview_common::apply_appearance_mode(dark);
            let name = if dark { preview_common::MACOS_DARK } else { preview_common::MACOS_LIGHT };
            l.append(format!("[Menu] appearance '{name}'"));
        }
    });
    // A View item that really does something. The row's `on_triggered` and action-signal
    // deliveries are independent: the loop above logs the label for every row, and this row's
    // id is what the handler above switches on. The item's creation order relative to the
    // handlers does not decide which of them runs — connections are appended and all of them
    // fire.
    win.new_menu_item_with_action_id(
        &view,
        "Toggle Appearance",
        Some("Ctrl+Shift+D"),
        Some("view.toggle-appearance"),
    );

    let menus =
        [("File".to_string(), file), ("Edit".to_string(), edit), ("View".to_string(), view)];
    let window_id = win.raw_id();
    let l = Arc::clone(log);
    // The event carries the entry rectangle, so the handler does not have to read the bar
    // (which is lent out exclusively while its own event is being dispatched).
    menu_bar.on_entry_triggered(move |title: String, entry: Rect| {
        let Some(index) = menus.iter().position(|(name, _)| *name == title) else {
            return;
        };
        let (w, h) = rust_widgets::widget_geometry(window_id)
            .map(|(_, _, w, h)| (w, h))
            .unwrap_or((1180, 760));
        // The popup's top-left is the entry's bottom-left: the macOS convention where
        // the menu appears to hang from the title that opened it.
        let opened =
            menus[index].1.open_at(entry.x, entry.y + entry.height as i32, Rect::new(0, 0, w, h));
        // Report the result and the entry count: an `on_triggered` that fires into a refused
        // open would look exactly like a click that did nothing.
        l.append(format!(
            "[MenuBar] '{title}' popup opened={opened} items={}",
            menus[index].1.item_count()
        ));
    });
    log.append("[MenuBar] File / Edit / View (click a title to open it)");

    let status = win.new_status_bar("就绪", 0, 0, 760, STATUS_HEIGHT);
    slots.push(Slot {
        id: status.raw_id(),
        band: Band::Status,
        row: 0,
        weight: 1,
        nominal: (0, 0, 760, STATUS_HEIGHT),
    });
    let l = Arc::clone(log);
    status.on_value_changed(move |val: &str| l.append(format!("[StatusBar] '{val}'")));
    log.append("[StatusBar] text='就绪'");

    slots
}

// ═══════════════════════════════════════════════════════════════════════════════
// ═══════════════════════════════════════════════════════════════════════════════
// run — App 入口，由 main() 调用
// ═══════════════════════════════════════════════════════════════════════════════

/// Registers a full-coverage CJK face, once, before anything is painted.
///
/// # Why this demo needs it
///
/// Every string below is painted by **this** library, not by the window manager: buttons,
/// labels, list rows and the status bar all go through `render::text`. The `desktop` profile's
/// `fonts-cjk` face is a subset (ASCII, CJK punctuation, kana, fullwidth forms and the first
/// 1000 Han codepoints), so 就绪, 启用通知, 滚动我 and most other characters here are outside
/// it. Without a registered full face they are drawn as tofu.
///
/// The library leaves the choice to the host — which family answers is a typography decision,
/// and reading a 20 MB collection is not something to do implicitly — so the demo asks once,
/// before the first control exists, and logs the answer either way.
fn register_cjk_face(log: &Arc<EventLog>) {
    let worker = CJK_SCAN
        .get()
        .and_then(|slot| slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take());
    match preview_common::finish_cjk_face(worker) {
        Some(family) => log.append(format!("[Fonts] 已注册系统 CJK 字体：{family}")),
        None => log.append("[Fonts] 未找到已安装的 CJK 字体；超出内置子集的汉字会显示为方框"),
    }
}

/// The background CJK-face registration started by [`prewarm_cjk_face`].
///
/// A process-wide slot rather than a parameter: the scan starts before `App::init` and is
/// consumed inside the window builder, which the live run and the offscreen preview both reach.
static CJK_SCAN: OnceLock<Mutex<Option<std::thread::JoinHandle<Option<String>>>>> = OnceLock::new();

/// Starts the CJK-face scan so it overlaps `App::init` and window creation (R-19).
fn prewarm_cjk_face() {
    let _ = CJK_SCAN.set(Mutex::new(preview_common::prewarm_cjk_face()));
}

/// Builds the demo's whole window: the registered faces, the palette, the chrome, every
/// control, and the layout that places them.
///
/// Shared by [`run`] and [`render_offscreen`]. The preview renders **this** construction rather
/// than a second copy of it, so a picture produced offscreen and the window a user sees cannot
/// describe two different layouts.
fn build_window(app: &mut App, dark: bool, log: &Arc<EventLog>) -> WindowHandle {
    // Before the first control exists: every label resolves its face at paint time, and a face
    // registered later would leave the first frame in tofu.
    register_cjk_face(log);

    // Register the macOS-inspired palettes before any control exists: a control picks its
    // colours up at construction, and switching afterwards would need a full repaint pass.
    preview_common::apply_appearance_mode(dark);
    log.append(format!(
        "[Theme] appearance '{}'",
        if dark { preview_common::MACOS_DARK } else { preview_common::MACOS_LIGHT }
    ));

    // 创建窗口（必须在 init 之后，run 之前）
    let win = app.new_window("Controls Demo - rust_widgets", 100, 100, 1180, 760);
    log.append(format!("[Window] created: id={:?}", win.raw_id()));

    // 窗口级 chrome：工具栏 + 状态栏
    let mut slots = build_window_chrome(&win, log);

    // 构建全部基础控件，并交给窗口一个 layout
    let mut followers = Vec::new();
    let built = build_all_controls(&win, log, &mut followers);
    slots.extend(built.slots);
    // 必须在控件都建好之后：layout 需要它们的 id。
    //
    // 没有这一步，窗口 resize 后**什么都不会重排**：库侧链路是完整的
    // （OS 报 resize → 后端入队 `Resized` → `app/handle.rs` 重跑窗口 layout），
    // 但 `LAYOUTS` 里没有本窗口的条目，`apply_window_layout` 拿到空列表，控件
    // 停留在启动时为 1180×760 算出的坐标。
    register_window_layout(&win, slots, followers, log, built.mdi);
    win
}

/// Renders the demo's window offscreen and returns its RGBA pixels.
///
/// # Why this exists
///
/// Every control in this demo is painted by the library, so the picture comes from the same
/// `render_window_frame` entry point the platform's painter uses (the tree, then each mounted
/// surface at its own rectangle) — no window needs to be shown, and none of the three platforms'
/// native shells is involved. That is what makes the demo's composition checkable on a host that
/// cannot open a window (and in CI), instead of only by looking at a screen.
///
/// It must run on the **main** thread: `mount_surface` (which the paging and chart rows use)
/// refuses off-main-thread mounts, because the Cocoa backend has to create a view.
pub fn render_offscreen(dark: bool, width: u32, height: u32) -> Option<Vec<u8>> {
    let log = Arc::new(EventLog::new());
    let mut app = App::with_config(
        AppConfig::default()
            .with_app_name("Controls Demo preview")
            .with_organization("rust_widgets"),
    );
    app.init();
    log.append("[App] init() done");
    let win = build_window(&mut app, dark, &log);

    // The same backdrop the SVG exporter uses: the active theme's own background, so a control
    // that paints nothing shows the surface it would really sit on.
    let clear = global_theme_manager()
        .current_theme()
        .map(|theme| theme.colors.background)
        .unwrap_or(rust_widgets::core::Color::WHITE);
    // `render_window_frame`, not `render_frame_tree`: the paging and chart rows are mounted
    // surfaces, not tree children, and a tree-only render leaves two empty rectangles where they
    // are (the platform gives each surface its own view and paints it separately).
    rust_widgets::widget::runtime::render_window_frame(
        win.raw_id(),
        rust_widgets::core::Size::new(width, height),
        clear,
    )
}

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

    // Start the disk-bound CJK face scan now; it overlaps App init and window creation. The
    // window builder joins it before the first control exists (R-19).
    prewarm_cjk_face();

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

    // Register the macOS-inspired palettes before any control exists, then build the window.
    // `build_window` is shared with `render_offscreen`, so an offscreen preview and the window
    // a user sees are one construction rather than two that can drift.
    let want_light = std::env::args().any(|arg| arg == "--light");
    let win = build_window(&mut app, !want_light, &log);
    log.append("[App] controls ready - starting event loop");

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

#[cfg(test)]
mod paint_tests {
    use super::*;
    use rust_widgets::core::{Color, Size};

    /// Probe: render the demo's real window tree and report what the background pixel is.
    #[test]
    fn probe_window_pixels() {
        let _guard = rust_widgets::theme::theme_test_guard();
        let log = Arc::new(EventLog::new());
        let mut app = App::with_config(AppConfig::default().with_app_name("probe"));
        app.init();
        preview_common::apply_appearance_mode(true);
        let win = app.new_window("probe", 100, 100, 1180, 760);
        let slots = build_window_chrome(&win, &log);
        let mut followers = Vec::new();
        let mut slots = slots;
        let built = build_all_controls(&win, &log, &mut followers);
        slots.extend(built.slots);
        register_window_layout(&win, slots, followers, &log, built.mdi);

        let size = Size::new(1180, 760);
        let frame = rust_widgets::widget::runtime::render_frame_tree(
            win.raw_id(),
            size,
            Color::rgb(240, 240, 240),
        )
        .expect("frame");
        let pixels = frame.as_chunks::<4>().0;
        let corner = pixels[0];
        eprintln!("[PROBE] corner pixel = {:?}", corner);
        let mut counts: Vec<([u8; 4], usize)> = Vec::new();
        for p in pixels {
            match counts.iter_mut().find(|(c, _)| *c == *p) {
                Some((_, n)) => *n += 1,
                None => counts.push((*p, 1)),
            }
        }
        counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        eprintln!("[PROBE] top colours: {:?}", &counts[..counts.len().min(8)]);
    }

    /// Row 9's sub-window frames are absolute, so a layout pass must re-apply the arrangement
    /// after the area has its new rectangle. Without it the frames stay where they were when the
    /// window was smaller: some fall outside the area's clip and the content disappears.
    ///
    /// # What is measured
    ///
    /// The layout's placement closure writes the area's geometry (as the framework's does), then
    /// the re-arrange block runs. Every sub-window frame must land inside the rectangle the
    /// layout just gave the area.
    #[test]
    fn a_layout_pass_rearranges_the_mdi_sub_windows() {
        use rust_widgets::widget::runtime::{register, with_widget};

        rust_widgets::init();
        let first = register(Box::new(rust_widgets::widget::Label::new(
            "A".to_string(),
            Rect::new(0, 0, 1, 1),
        )))
        .expect("register the first label");
        let second = register(Box::new(rust_widgets::widget::Label::new(
            "B".to_string(),
            Rect::new(0, 0, 1, 1),
        )))
        .expect("register the second label");
        let mut area = MdiArea::new(Rect::new(0, 0, 120, 80));
        area.add_sub_window(first, Rect::new(0, 0, 100, 40));
        area.add_sub_window(second, Rect::new(20, 20, 100, 40));
        let area_id = register(Box::new(area)).expect("register the area");

        let mdi = Arc::new(Mutex::new(MdiRowState { area: Some(area_id), cascade: true }));
        let layout = ControlGridLayout {
            slots: vec![Slot {
                id: area_id,
                band: Band::Content,
                row: 0,
                weight: 1,
                nominal: (20, 20, 120, 80),
            }],
            followers: Vec::new(),
            log: Arc::new(EventLog::new()),
            last_applied: std::cell::Cell::new(None),
            last_logged: std::cell::Cell::new(None),
            scratch_rows: RefCell::new(Vec::new()),
            scratch_heights: RefCell::new(Vec::new()),
            scratch_placed: RefCell::new(Vec::new()),
            scratch_row_slots: RefCell::new(Vec::new()),
            scratch_widths: RefCell::new(Vec::new()),
            mdi,
        };

        let mut placed = None;
        LayoutTrait::update(&layout, Rect::new(0, 0, 600, 300), &mut |id, rect| {
            // The framework's placement: the widget really moves before the re-arrange phase
            // runs, which is what the MDI re-arrange reads.
            rust_widgets::set_widget_geometry(id, rect.x, rect.y, rect.width, rect.height);
            if id == area_id {
                placed = Some(rect);
            }
        });
        let placed = placed.expect("the area is one of the layout's slots");

        let (left, right) = with_widget(area_id, |widget| {
            let area = (widget as &dyn core::any::Any)
                .downcast_ref::<MdiArea>()
                .expect("the registered widget is an MdiArea");
            (
                area.sub_window(0).expect("first sub-window").geometry(),
                area.sub_window(1).expect("second sub-window").geometry(),
            )
        })
        .expect("read the area back");

        for frame in [left, right] {
            assert!(
                frame.x >= placed.x
                    && frame.y >= placed.y
                    && frame.x + frame.width as i32 <= placed.x + placed.width as i32
                    && frame.y + frame.height as i32 <= placed.y + placed.height as i32,
                "sub-window {frame:?} is outside the placed area {placed:?}"
            );
        }
    }
}
