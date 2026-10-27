// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MasonryLayout widget — a Pinterest-style waterfall grid layout.
//!
//! Items are arranged in a vertical waterfall across a configurable number of
//! columns. Each item is drawn as a filled rounded rectangle with a label.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_u32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// An individual item in the masonry layout.
#[derive(Debug, Clone)]
pub struct MasonryItem {
    /// Display label text.
    pub label: String,
    /// Height of the item in logical pixels.
    pub height: u32,
    /// Fill color of the item.
    pub color: Color,
    /// A real control mounted in this card, if any.
    ///
    /// `None` keeps the historical card: the cell is painted from [`Self::label`] and
    /// [`Self::color`] and nothing is mounted. When set, the layout links the control as its own
    /// child (both directions, through [`BaseWidget::add_child_linked`]) and writes the cell's
    /// rectangle into it on every arrangement, so the tree painter draws the control in the cell.
    /// See [`MasonryLayout::set_item_widget`].
    pub widget_id: Option<ObjectId>,
    /// How many columns this card spans (C-13). `1` — the default — keeps the historical
    /// single-column card; a larger value is clamped to the layout's column count at
    /// arrangement time.
    pub column_span: u32,
}

impl MasonryItem {
    /// A single-column card, the shape [`MasonryLayout::add_item`] produces.
    pub fn new(label: impl Into<String>, height: u32, color: Color) -> Self {
        Self { label: label.into(), height, color, widget_id: None, column_span: 1 }
    }
}

/// The default vertical gap between two stacked cards, in logical pixels.
///
/// The per-control value is [`MasonryLayout::item_spacing`]; this is what a control uses until
/// [`MasonryLayout::set_item_spacing`] says otherwise, and it is the value the waterfall used
/// before the gap became configurable.
const DEFAULT_ITEM_SPACING: u32 = 4;

/// The corner radius every card is drawn with.
///
/// Named because it was previously *two* values: a `let _corner_radius: u32 = 4` in the layout
/// pass — bound and never read — beside a `6` in the draw. A reader who found the first would
/// have concluded cards are rounded by 4 px. There is one radius and it belongs here.
const CARD_CORNER_RADIUS: u32 = 6;

/// A cached arrangement paired with the layout version that produced it.
///
/// The entries are `(item_index, rect)`, not `(item, rect)`: a cache rebuild used to clone every
/// card's label into the cached vector, so a long list paid one `String` clone per card per
/// invalidation. The index borrows the item from [`MasonryLayout::items`] on the way out, and the
/// only way that can go stale is the item list changing — which bumps the version and rebuilds.
type CachedArrangement = Option<(u64, std::rc::Rc<Vec<(usize, Rect)>>)>;

/// How the waterfall divides its width between columns (C-020).
///
/// [`Equal`](Self::Equal) is the historical behaviour and the default. [`Content`](Self::Content)
/// gives each column a share proportional to the widest item it holds, so a column of short cards
/// does not claim the same width as a column of long ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MasonryColumnWidth {
    /// Every column gets the same width.
    #[default]
    Equal,
    /// Column widths follow the content's measured width.
    Content,
}

/// MasonryLayout widget — a Pinterest-style waterfall grid.
pub struct MasonryLayout {
    base: BaseWidget,
    columns: u32,
    items: Vec<MasonryItem>,
    /// Bumped by every input that changes the arrangement (items, columns, geometry, spacing).
    layout_version: u64,
    /// Cached arrangement and the version it was produced under.
    cache: std::cell::RefCell<CachedArrangement>,
    /// The vertical gap between two stacked cards in one column.
    item_spacing: u32,
    /// The horizontal gap between two columns (C-12).
    horizontal_gap: u32,
    /// How the column widths are derived (C-020).
    column_width: MasonryColumnWidth,
    /// Shared forwarding table for the controls mounted in cards (C-004).
    registry: Option<std::rc::Rc<std::cell::RefCell<SimpleRegistry>>>,
    /// The card that last received a pointer press, so keyboard/focus events reach it.
    active_item: Option<usize>,
    /// Emitted with the item index when a card is clicked.
    pub item_clicked: Signal1<usize>,
}

impl MasonryLayout {
    /// Creates a new MasonryLayout widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MasonryLayout, geometry, "MasonryLayout"),
            columns: 2,
            items: Vec::new(),
            layout_version: 0,
            cache: std::cell::RefCell::new(None),
            item_spacing: DEFAULT_ITEM_SPACING,
            horizontal_gap: DEFAULT_ITEM_SPACING,
            column_width: MasonryColumnWidth::Equal,
            registry: None,
            active_item: None,
            item_clicked: Signal1::new(),
        }
    }

    /// Binds the shared forwarding table used to deliver events to card controls (C-004).
    pub fn set_registry(&mut self, registry: std::rc::Rc<std::cell::RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }

    /// The bound forwarding table, if any.
    pub fn registry(&self) -> Option<&std::rc::Rc<std::cell::RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }

    /// The column-width strategy (C-020).
    pub fn column_width_strategy(&self) -> MasonryColumnWidth {
        self.column_width
    }

    /// Sets how column widths are derived (C-020).
    ///
    /// [`MasonryColumnWidth::Equal`] is the default, so a layout that never calls this keeps its
    /// historical appearance.
    pub fn set_column_width_strategy(&mut self, strategy: MasonryColumnWidth) {
        if self.column_width == strategy {
            return;
        }
        self.column_width = strategy;
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Delivers `event` to the control mounted in card `index`, if one is mounted.
    fn forward_item_event(&self, index: usize, event: &Event) {
        let (Some(registry), Some(widget_id)) =
            (self.registry.as_ref(), self.items.get(index).and_then(|item| item.widget_id))
        else {
            return;
        };
        let _ = registry.borrow_mut().forward_event(widget_id, event);
    }

    /// Returns the number of columns.
    pub fn columns(&self) -> u32 {
        self.columns
    }

    /// Sets the number of columns (minimum 1).
    pub fn set_columns(&mut self, columns: u32) {
        let columns = columns.max(1);
        if self.columns == columns {
            return;
        }
        self.columns = columns;
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The vertical gap between two stacked cards, in logical pixels.
    pub fn item_spacing(&self) -> u32 {
        self.item_spacing
    }

    /// Sets the vertical gap between two stacked cards.
    ///
    /// Zero is a legal value (cards touch); the gap is a caller choice rather than a floor, so it
    /// is stored as given. The arrangement is versioned, so the next `draw`/`item_at` rebuilds it
    /// instead of serving the previous gap out of the cache.
    pub fn set_item_spacing(&mut self, spacing: u32) {
        if self.item_spacing == spacing {
            return;
        }
        self.item_spacing = spacing;
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns a reference to the items slice.
    pub fn items(&self) -> &[MasonryItem] {
        &self.items
    }

    /// The horizontal gap between two columns, in logical pixels (C-12).
    pub fn horizontal_gap(&self) -> u32 {
        self.horizontal_gap
    }

    /// Sets the horizontal gap between two columns (C-12).
    ///
    /// It is independent of [`Self::item_spacing`] because the two are different measures: the
    /// vertical gap spaces cards inside one column, the horizontal one spaces the columns
    /// themselves. The default equals the vertical default, so a layout that sets nothing
    /// keeps a balanced look; zero is legal (columns touch).
    pub fn set_horizontal_gap(&mut self, gap: u32) {
        if self.horizontal_gap == gap {
            return;
        }
        self.horizontal_gap = gap;
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Sets a card's column span, clamped to at least one column (C-13).
    ///
    /// Returns `false` for an out-of-range index. A span wider than the layout has columns is
    /// clamped when the arrangement is computed rather than rejected here, so a card added
    /// before the column count is set still lands sensibly.
    pub fn set_item_column_span(&mut self, index: usize, span: u32) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        let span = span.max(1);
        if item.column_span == span {
            return true;
        }
        item.column_span = span;
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// The column span of the card at `index` (C-13).
    pub fn item_column_span(&self, index: usize) -> Option<u32> {
        self.items.get(index).map(|item| item.column_span.max(1))
    }

    /// Adds an item to the masonry layout.
    pub fn add_item(&mut self, label: &str, height: u32, color: Color) {
        self.items.push(MasonryItem::new(label, height, color));
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Adds an item that hosts `widget` in its cell.
    ///
    /// The widget is mounted through [`Self::set_item_widget`], so it is linked as this control's
    /// child and given the cell rectangle; the label and colour still paint the card face behind
    /// it, which keeps the waterfall readable while the control fills the cell.
    pub fn add_item_with_widget(
        &mut self,
        label: &str,
        height: u32,
        color: Color,
        widget: Option<ObjectId>,
    ) -> usize {
        self.add_item(label, height, color);
        let index = self.items.len().saturating_sub(1);
        self.set_item_widget(index, widget);
        index
    }

    /// Mounts `widget` (or clears it, with `None`) in the card at `index`.
    ///
    /// Returns `false` for an out-of-range index, so a caller cannot mistake a dropped mount for
    /// a successful one. A replacement unlinks the previous control and hides it, exactly as
    /// [`crate::widget::container_widgets::TabWidget::set_tab_widget`] does: a detached control
    /// keeps its own visibility flag, and a host that renders by id would still paint it in the
    /// old cell.
    pub fn set_item_widget(&mut self, index: usize, widget: Option<ObjectId>) -> bool {
        if index >= self.items.len() {
            return false;
        }
        let previous = self.items[index].widget_id;
        if previous == widget {
            return true;
        }
        if let Some(previous) = previous {
            self.base.remove_child_linked(previous);
            crate::set_widget_visible(previous, false);
        }
        self.items[index].widget_id = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
            crate::set_widget_visible(widget_id, true);
        }
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Removes the item at `index`, returning it.
    ///
    /// A removed card takes its mounted control out of the tree with it: the child link is
    /// cleared in both directions and the control is hidden, so the waterfall does not keep
    /// painting a widget whose cell no longer exists.
    pub fn remove_item(&mut self, index: usize) -> Option<MasonryItem> {
        if index >= self.items.len() {
            return None;
        }
        let item = self.items.remove(index);
        if let Some(widget_id) = item.widget_id {
            self.base.remove_child_linked(widget_id);
            crate::set_widget_visible(widget_id, false);
        }
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
        Some(item)
    }

    /// Removes all items from the layout.
    pub fn clear_items(&mut self) {
        if self.items.is_empty() {
            return;
        }
        // Every mounted card control is unlinked before the list is dropped, so clearing the
        // layout cannot leave children whose cells are gone (the same rule `remove_item` states).
        for item in self.items.drain(..) {
            if let Some(widget_id) = item.widget_id {
                self.base.remove_child_linked(widget_id);
                crate::set_widget_visible(widget_id, false);
            }
        }
        self.layout_version = self.layout_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The cached arrangement, recomputed only when an input changed.
    ///
    /// The draw path calls this every frame; `layout_items` used to walk the whole list
    /// (and rebuild every `MasonryItem`) on each of those frames. The cache is keyed by a
    /// version bumped by the three mutation entry points and by `set_geometry`.
    fn layout_items_shared(&self) -> std::rc::Rc<Vec<(usize, Rect)>> {
        if let Some((version, cached)) = self.cache.borrow().as_ref() {
            if *version == self.layout_version {
                return std::rc::Rc::clone(cached);
            }
        }
        let layout = std::rc::Rc::new(self.compute_items());
        *self.cache.borrow_mut() = Some((self.layout_version, std::rc::Rc::clone(&layout)));
        layout
    }

    /// The item index under `point`, or `None` when the point misses every card.
    pub fn item_at(&self, point: Point) -> Option<usize> {
        let layout = self.layout_items_shared();
        layout.iter().find(|(_, rect)| rect.contains_point(point)).map(|(index, _)| *index)
    }

    /// Writes each mounted control's cell rectangle into it.
    ///
    /// # Why the write lives here and not in `compute_items`
    ///
    /// The arrangement is cached and may be served unchanged for many frames, while the *widget*
    /// geometry still has to reach the control before the tree painter draws it. This walks the
    /// cached arrangement (an `Rc`, cloned out of the cache, so no borrow is held) and writes
    /// through the public geometry entry — the same half of the contract
    /// `TabWidget::place_current_page` performs, rather than a second drawing path.
    fn place_item_widgets(&self) {
        let layout = self.layout_items_shared();
        for (index, rect) in layout.iter() {
            let Some(widget_id) = self.items.get(*index).and_then(|item| item.widget_id) else {
                continue;
            };
            // C-005: an unchanged cell is not written again. This loop runs from `draw` and from
            // `set_geometry`, so a repaint of a static waterfall used to perform one global
            // geometry write per card for no new information (the same short-circuit
            // `accordion` uses).
            if crate::widget_geometry(widget_id) == Some((rect.x, rect.y, rect.width, rect.height))
            {
                continue;
            }
            crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
        }
    }

    /// The height a card occupies, measured from its mounted control when it has one (C-020).
    ///
    /// The declared `height` remains the fallback: a card with no control, or one whose runtime
    /// entry is unavailable, keeps the height the caller gave it.
    fn measured_item_height(&self, item: &MasonryItem) -> u32 {
        let measured = item.widget_id.and_then(|widget_id| {
            crate::widget::runtime::with_widget(widget_id, |widget| widget.size_hint().height)
        });
        measured.filter(|height| *height > 0).unwrap_or(item.height).max(20)
    }

    /// The width a card's content asks for (C-020).
    fn measured_item_width(&self, item: &MasonryItem) -> u32 {
        let measured = item.widget_id.and_then(|widget_id| {
            crate::widget::runtime::with_widget(widget_id, |widget| widget.size_hint().width)
        });
        let fallback = crate::widget::metrics::estimate_text_width(
            &item.label,
            &Font::simple("Arial", 12.0),
            1.0,
        )
        .saturating_add(12);
        measured.filter(|width| *width > 0).unwrap_or(fallback)
    }

    /// Lays the cards out, returning `(item_index, cell_rect)` in insertion order.
    ///
    /// The index is stored instead of a cloned item: the cached vector used to own a copy of every
    /// `MasonryItem`, so a rebuild cloned every card's label `String`. The item is borrowed from
    /// [`Self::items`] when it is needed, and a list change bumps the version and rebuilds.
    fn compute_items(&self) -> Vec<(usize, Rect)> {
        if self.items.is_empty() {
            return Vec::new();
        }

        let columns = self.columns.max(1);
        let rect = self.base.geometry();
        let gap = self.horizontal_gap;
        // The gaps are taken out of the usable width before the columns are divided, so the
        // rightmost column ends exactly on the control's trailing edge (C-12). `saturating_sub`
        // keeps a control narrower than its gaps from underflowing; the columns then collapse
        // to zero width rather than wrapping around.
        let total_gap = gap.saturating_mul(columns.saturating_sub(1));
        let budget = rect.width.saturating_sub(total_gap);
        let spacing = self.item_spacing;

        // Track the current y-offset for each column, and the placement facts the width pass
        // needs: `(item_index, start_column, span, height, y_in_content)`.
        let mut col_heights = vec![0u32; columns as usize];
        let mut placed: Vec<(usize, u32, u32, u32, u32)> = Vec::with_capacity(self.items.len());

        for (index, item) in self.items.iter().enumerate() {
            // A span wider than the column count would place a card outside the control, so it
            // is clamped rather than trusted (C-13).
            let span = item.column_span.max(1).min(columns);
            // The start column whose spanned band ends highest; ties go to the leftmost, which
            // is what makes the single-column case reduce to the original "shortest column"
            // rule exactly.
            let mut best_start = 0u32;
            let mut best_bottom = u32::MAX;
            for start in 0..=(columns - span) {
                let bottom = col_heights[start as usize..(start + span) as usize]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0);
                if bottom < best_bottom {
                    best_bottom = bottom;
                    best_start = start;
                }
            }

            let item_height = self.measured_item_height(item);
            placed.push((index, best_start, span, item_height, best_bottom));

            // Every spanned column advances past this card: the next card may not start above
            // the tallest of them, or the columns would overlap.
            let next = best_bottom.saturating_add(item_height.saturating_add(spacing));
            for height in &mut col_heights[best_start as usize..(best_start + span) as usize] {
                *height = next;
            }
        }

        // Column widths: equal by default; content-driven when requested (C-020). Both paths
        // allocate the same integer budget, so the rightmost column still ends on the control's
        // trailing edge.
        let col_widths = match self.column_width {
            MasonryColumnWidth::Equal => vec![budget / columns; columns as usize],
            MasonryColumnWidth::Content => {
                let mut needs = vec![0u32; columns as usize];
                for (index, start, span, _, _) in &placed {
                    let Some(item) = self.items.get(*index) else {
                        continue;
                    };
                    let share = self.measured_item_width(item) / (*span).max(1);
                    for slot in needs.iter_mut().skip(*start as usize).take(*span as usize) {
                        *slot = (*slot).max(share);
                    }
                }
                allocate_column_widths(&needs, budget)
            }
        };
        let mut column_x = Vec::with_capacity(columns as usize);
        let mut cursor = 0u32;
        for width in &col_widths {
            column_x.push(cursor);
            cursor = cursor.saturating_add(*width).saturating_add(gap);
        }

        let mut result = Vec::with_capacity(placed.len());
        for (index, start, span, item_height, y) in placed {
            let start = start as usize;
            let span = span as usize;
            let width = col_widths
                .iter()
                .skip(start)
                .take(span)
                .copied()
                .fold(0u32, u32::saturating_add)
                .saturating_add(gap.saturating_mul(span.saturating_sub(1) as u32));
            let x = rect.x + column_x.get(start).copied().unwrap_or(0) as i32;
            result.push((index, Rect::new(x, rect.y + y as i32, width, item_height)));
        }
        result
    }

    /// Calculate positions for all items (waterfall layout).
    ///
    /// Returns an owned copy of the arrangement paired with a clone of each card, for callers that
    /// need both together; the draw and hit-test paths share the cached `Rc` of indices instead.
    #[cfg(test)]
    fn layout_items(&self) -> Vec<(MasonryItem, Rect)> {
        self.layout_items_shared()
            .iter()
            .filter_map(|(index, rect)| self.items.get(*index).map(|item| (item.clone(), *rect)))
            .collect()
    }
}

impl Widget for MasonryLayout {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.layout_version = self.layout_version.wrapping_add(1);
        // The cells moved with the control, so the controls mounted in them have to move too.
        self.place_item_widgets();
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::MASONRY_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::MASONRY_DEFAULT_HEIGHT,
        )
    }
    /// Resolves a published event name to the signal this layout actually emits.
    ///
    /// The capability publishes `item_clicked` with a `UInt` payload and the
    /// control emits the clicked tile's index as a `usize`, so the conversion
    /// keeps the subscriber's number identical to the emitted one.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "item_clicked" => {
                Some(EventSignalRef::mapped("item_clicked", &self.item_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MasonryLayout`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `item_count` is derived from
/// the item list, so it is readable but read-only.
impl WidgetProperties for MasonryLayout {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "column_count" => Ok(CapabilityValue::UInt(self.columns() as u64)),
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "item_spacing" => Ok(CapabilityValue::UInt(u64::from(self.item_spacing()))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "column_count" => {
                self.set_columns(expect_u32(value)?);
                Ok(())
            }
            "item_spacing" => {
                self.set_item_spacing(expect_u32(value)?);
                Ok(())
            }
            // The item total is implied by how many items were added, so there is
            // nothing sensible to assign to it.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["column_count", "item_count", "item_spacing", BASE_PROPERTY_NAMES]
    }
}

impl Draw for MasonryLayout {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let font = Font::simple("Arial", 12.0);

        // The container behind the cards is this control's chrome, so it follows the
        // theme. `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not re-entrant).
        //
        // `masonry_layout` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. Painting that leaves the panel byte-identical to the frame behind it,
        // so the resolved surface is re-derived a visible step away from the window's ink
        // colour; a colour the caller set still wins.
        let style = self.base.style();
        let themed = crate::style::resolved_theme_style_shared("masonry_layout");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        let themed_text = themed.as_ref().and_then(|r| r.text_color);
        // Precedence: explicit style → theme → the ORIGINAL LITERAL. The literal has to
        // stay: an inactive theme still needs a defined appearance, and keeping the old
        // value means the existing pixel baselines cannot regress.
        let base = style.background_color.or(themed_bg).unwrap_or(Color::rgba(180, 180, 180, 200));
        let ink = style.text_color.or(themed_text).unwrap_or_else(|| base.contrast_color());
        let container_bg = base.blend(&ink, 0.04);

        let layout = self.layout_items_shared();
        // The mounted controls are placed before they are painted: the tree painter draws a child
        // at the child's own geometry, and the cell rectangle is this control's answer.
        self.place_item_widgets();

        // The panel is drawn first so an empty layout still shows where the control is
        // rather than being indistinguishable from the frame behind it.
        context.face_with_gradient_corners(
            rect,
            container_bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // A masonry column runs as tall as its items make it, so a list longer than the control
        // would paint cards past the bottom edge — and nothing clips a widget at this layer, so
        // they would land on whatever is behind it (BLUE21 D12). The clip is the control's own
        // rectangle, and `layout_items` unchanged; only what is *painted* is bounded.
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        for (index, item_rect) in layout.iter() {
            // A card that starts below the visible area is dropped rather than clipped: the
            // clip already handles the partial case, and skipping the fully-hidden ones keeps a
            // long list from emitting drawing commands for pixels nobody can see.
            if item_rect.y >= rect.y.saturating_add(rect.height as i32) {
                continue;
            }
            let Some(item) = self.items.get(*index) else {
                continue;
            };
            // Draw the item card background. `item.color` is the per-card colour the
            // CALLER supplied — it identifies the card, so it is data and is passed
            // through untouched rather than resolved from the theme.
            context.fill_rounded_rect(*item_rect, CARD_CORNER_RADIUS, item.color);

            // A cell with a mounted control has no label: the control is what fills the cell, and a
            // caption drawn underneath it would be painted over by the child anyway.
            if item.widget_id.is_some() {
                continue;
            }

            // Draw the label text centered in the item, in whichever ink is legible on
            // that card's own colour.
            let text_color = item.color.contrast_color();
            let text_x = item_rect.x + 6;
            // The cell's own line box, centred: a glyph origin is the box's top-left edge,
            // so the previous `item_rect.y + item_rect.height / 2 - 6` put that edge on the
            // cell's middle line — half a line low, offset by a hand-tuned constant.
            let item_line = context.text_line(*item_rect, &font);
            let text_y = item_line.y;
            context.draw_text(
                crate::core::Point::new(text_x, text_y),
                &item.label,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
        context.pop_clip();

        // If there are no items, draw an empty-state hint. The hint is chrome, so it reads
        // the resolved ink instead of a literal grey.
        if self.items.is_empty() {
            let hint = "No items";
            let hint_color = ink.blend(&base, 0.35);
            let hint_font = Font::simple("Arial", 16.0);
            let hint_x = rect.x + (rect.width / 4) as i32;
            let hint_y = rect.y + (rect.height / 3) as i32;
            context.draw_text(
                crate::core::Point::new(hint_x, hint_y),
                hint,
                &hint_font,
                hint_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

/// Allocates `budget` pixels across columns in proportion to `needs` (C-020).
///
/// Integer arithmetic with the trailing column taking the remainder, so the widths always sum to
/// exactly `budget`. A zero total (no measured content) falls back to an equal division, which is
/// the same answer the equal strategy would give.
fn allocate_column_widths(needs: &[u32], budget: u32) -> Vec<u32> {
    let count = needs.len();
    if count == 0 {
        return Vec::new();
    }
    let total: u64 = needs.iter().map(|need| *need as u64).sum();
    if total == 0 {
        return vec![budget / count as u32; count];
    }
    let mut widths = Vec::with_capacity(count);
    let mut assigned = 0u32;
    for (index, need) in needs.iter().enumerate() {
        let share = if index + 1 == count {
            budget.saturating_sub(assigned)
        } else {
            (budget as u64 * *need as u64 / total) as u32
        };
        let share = share.min(budget.saturating_sub(assigned));
        widths.push(share);
        assigned = assigned.saturating_add(share);
    }
    widths
}

impl EventHandler for MasonryLayout {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // C-004: a card with a mounted control forwards the event to it. Pointer events are
        // routed to the card under the pointer; key/focus events go to the card that was last
        // pressed.
        let pointer_target = match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos }
            | Event::MouseDoubleClick { pos, .. }
            | Event::MouseEnter { pos }
            | Event::MouseLeave { pos } => self.item_at(*pos),
            _ => self.active_item,
        };
        if let Some(index) = pointer_target {
            if matches!(event, Event::MousePress { .. }) {
                self.active_item = Some(index);
            }
            self.forward_item_event(index, event);
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if let Some(index) = self.item_at(*pos) {
                self.item_clicked.emit(index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    /// The published name must resolve, and an unpublished one must not.
    #[test]
    fn published_item_clicked_resolves_to_a_signal() {
        let layout = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        assert!(layout.event_signal_dyn("item_clicked").is_some());
        assert!(layout.event_signal_dyn("no_such_event").is_none());
    }

    #[test]
    fn masonry_creation() {
        let ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        assert_eq!(ml.columns(), 2);
        assert!(ml.items().is_empty());
        assert_eq!(ml.kind(), WidgetKind::MasonryLayout);
    }

    #[test]
    fn masonry_set_columns() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        ml.set_columns(3);
        assert_eq!(ml.columns(), 3);
        ml.set_columns(0); // should clamp to 1
        assert_eq!(ml.columns(), 1);
    }

    #[test]
    fn masonry_add_item() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        ml.add_item("Item A", 80, Color::rgba(52, 152, 219, 255));
        ml.add_item("Item B", 120, Color::rgba(231, 76, 60, 255));
        assert_eq!(ml.items().len(), 2);
        assert_eq!(ml.items()[0].label, "Item A");
        assert_eq!(ml.items()[0].height, 80);
        assert_eq!(ml.items()[1].label, "Item B");
        assert_eq!(ml.items()[1].height, 120);
    }

    #[test]
    fn masonry_clear_items() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        ml.add_item("Item", 80, Color::rgba(52, 152, 219, 255));
        assert_eq!(ml.items().len(), 1);
        ml.clear_items();
        assert!(ml.items().is_empty());
    }

    #[test]
    fn masonry_layout_items_returns_correct_count() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        ml.add_item("A", 60, Color::RED);
        ml.add_item("B", 80, Color::GREEN);
        ml.add_item("C", 100, Color::BLUE);
        let layout = ml.layout_items();
        assert_eq!(layout.len(), 3);
    }

    #[test]
    fn masonry_svg_output() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 400));
        ml.add_item("Card 1", 80, Color::rgba(52, 152, 219, 255));
        ml.add_item("Card 2", 120, Color::rgba(231, 76, 60, 255));
        let svg = crate::widget::svg::render_to_svg(&mut ml);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // Should contain fill operations for the rounded rects.
        assert!(svg.contains("fill="));
    }

    #[test]
    fn masonry_event_handler_delegates() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 400));
        // Should not panic.
        ml.handle_event(&Event::MouseMove { pos: Point::new(50, 50) });
        ml.handle_event(&Event::KeyDown((65, 0)));
    }

    /// The cached arrangement is reusable, and a click names the card under the pointer.
    #[test]
    fn the_arrangement_is_cached_and_cards_are_clickable() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 400));
        ml.set_columns(1);
        ml.add_item("A", 60, Color::RED);
        ml.add_item("B", 80, Color::GREEN);
        let first = ml.layout_items_shared();
        let second = ml.layout_items_shared();
        assert!(std::rc::Rc::ptr_eq(&first, &second), "a second frame reuses the cache");
        let first_rect = first[0].1;
        assert_eq!(ml.item_at(Point::new(first_rect.x + 1, first_rect.y + 1)), Some(0));
        assert_eq!(ml.item_at(Point::new(299, 399)), None, "past the last card");

        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
        let sink = std::sync::Arc::clone(&seen);
        ml.item_clicked.connect(move |index| {
            sink.store(*index, std::sync::atomic::Ordering::SeqCst);
        });
        ml.handle_event(&Event::MousePress {
            pos: Point::new(first_rect.x + 1, first_rect.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 0);

        ml.add_item("C", 40, Color::BLUE);
        let third = ml.layout_items_shared();
        assert!(!std::rc::Rc::ptr_eq(&first, &third), "a mutation invalidates the cache");
    }

    /// A list taller than the control is painted inside it, not over whatever is behind it.
    ///
    /// Regression (BLUE21 D12): the draw had no `push_clip`, and a masonry column runs as tall as
    /// its items make it — so a long list emitted cards past the bottom edge, on top of the
    /// sibling below the control. The clip bounds every card to the control's own rectangle.
    #[test]
    fn cards_past_the_bottom_edge_are_clipped_away() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 120));
        ml.set_columns(1);
        for i in 0..10 {
            ml.add_item(&std::format!("row {i}"), 100, Color::rgba(52, 152, 219, 255));
        }
        let svg = crate::widget::svg::render_to_svg(&mut ml);

        assert!(svg.contains("<clipPath"), "the cards must be clipped to the control: {svg}");
        // Every card's rounded rect is inside the clip group, so no card paints below y = 120.
        let card_ys: Vec<i32> = svg
            .split("<rect")
            .skip(1)
            .filter_map(|rest| rest.split("y=\"").nth(1))
            .filter_map(|rest| rest.split('"').next())
            .filter_map(|value| value.parse().ok())
            .collect();
        assert!(
            card_ys.iter().all(|y| *y < 120),
            "no card may start below the control's bottom edge: {card_ys:?}"
        );
    }

    /// C-12: the horizontal gap is its own value and is taken out of the column width.
    #[test]
    fn the_horizontal_gap_separates_columns() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 400));
        ml.set_columns(2);
        ml.set_horizontal_gap(10);
        ml.add_item("a", 50, Color::RED);
        ml.add_item("b", 50, Color::RED);
        let rects = ml.layout_items_shared();
        assert_eq!(rects[0].1.width, 145, "(300 - 10) / 2");
        assert_eq!(rects[1].1.x, 155, "the second column starts after the gap");
    }

    /// C-13: a spanning card takes the width of every column it covers and advances them all.
    #[test]
    fn a_spanning_card_covers_every_column_it_names() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 400));
        ml.set_columns(2);
        ml.set_horizontal_gap(10);
        ml.add_item("wide", 50, Color::RED);
        ml.add_item("tall", 40, Color::GREEN);
        assert!(ml.set_item_column_span(0, 2));
        assert_eq!(ml.item_column_span(0), Some(2));
        let rects = ml.layout_items_shared();
        assert_eq!(rects[0].1.width, 300, "both columns plus the gap between them");
        assert!(rects[1].1.y >= rects[0].1.y + 50, "the next card clears the spanning one");
    }

    /// C-020: the content strategy gives a wider column to wider content, and an item control's
    /// measured height replaces the declared hint.
    #[test]
    fn content_column_width_and_measured_height_follow_the_content() {
        use crate::widget::base_widgets::label::Label;

        assert_eq!(MasonryColumnWidth::default(), MasonryColumnWidth::Equal);
        assert_eq!(allocate_column_widths(&[10, 30], 100), vec![25, 75]);
        assert_eq!(
            allocate_column_widths(&[0, 0], 90),
            vec![45, 45],
            "no measured content falls back to an equal division"
        );

        let id = crate::widget::runtime::register(Box::new(Label::new(
            "measured".to_string(),
            Rect::new(0, 0, 150, 90),
        )))
        .expect("the test control mounts");
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 400, 400));
        ml.set_columns(1);
        ml.add_item_with_widget("measured", 20, Color::WHITE, Some(id));
        ml.set_column_width_strategy(MasonryColumnWidth::Content);
        assert_eq!(ml.column_width_strategy(), MasonryColumnWidth::Content);
        let rects = ml.layout_items_shared();
        assert_eq!(rects[0].1.height, 90, "the card follows its control's size hint");
        assert_eq!(rects[0].1.width, 400, "one content column takes the whole width");
        let _ = crate::widget::runtime::unregister(id);
    }

    /// C-004: a mounted card control receives forwarded pointer and key events.
    #[test]
    fn a_mounted_card_receives_forwarded_events() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let registry = std::rc::Rc::new(std::cell::RefCell::new(SimpleRegistry::new()));
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        registry.borrow_mut().register(
            21,
            |_| {},
            move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            },
        );

        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 400));
        ml.set_columns(1);
        ml.add_item_with_widget("card", 50, Color::WHITE, Some(21u64));
        ml.set_registry(std::rc::Rc::clone(&registry));
        ml.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        ml.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
}
