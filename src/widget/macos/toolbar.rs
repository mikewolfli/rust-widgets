// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MacToolbar` — a horizontal toolbar strip with item slots.
//!
//! A desktop toolbar is one row of equal-width slots above the content it acts on, separated
//! from that content by a hairline. This control draws the strip, arranges the slots, reports
//! which slot a pointer is over, and emits the activated slot's index.

use crate::compat::Cow;
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{profile_metrics, role_font, ControlMetricsProfile};
use crate::widget::text_utils::elide_to_width;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::{
    centred_band, draw_focus_ring, palette, MAC_TOOLBAR_DEFAULT_WIDTH, MAC_TOOLBAR_HEIGHT,
    MAC_TOOLBAR_INSET, MAC_TOOLBAR_ITEM_PADDING,
};

/// The gap between two toolbar slots.
const SLOT_GAP: i32 = 4;

/// The narrowest a slot may be, so a short caption is never elided away in a default strip.
const MIN_SLOT_WIDTH: u32 = 56;

/// The vertical inset of a slot from the strip's edge.
const SLOT_VERTICAL_INSET: u32 = 4;

/// How a toolbar slot presents its icon and caption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolbarItemDisplayMode {
    /// Caption only (the historical behaviour).
    #[default]
    TextOnly,
    /// Icon only.
    IconOnly,
    /// Icon above the caption.
    IconAndText,
}

/// What a toolbar slot is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolbarItemKind {
    /// A clickable action slot.
    #[default]
    Item,
    /// A vertical rule between groups; not clickable.
    Separator,
    /// A gap that absorbs the strip's spare width.
    FlexibleSpace,
    /// A host-supplied widget occupying the slot.
    WidgetSlot,
}

/// One toolbar slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacToolbarItem {
    /// The stable identifier the host uses to recognise the item.
    pub id: String,
    /// The caption shown in the slot.
    pub label: String,
    /// An optional icon name, kept for hosts that resolve glyphs themselves.
    pub icon: Option<String>,
    /// How the icon and caption are presented.
    pub display_mode: ToolbarItemDisplayMode,
    /// Whether this slot is an action, a separator or a flexible space.
    pub kind: ToolbarItemKind,
    /// Whether the slot's action is available. A disabled slot paints in disabled ink and is
    /// not activated.
    pub enabled: bool,
}

impl MacToolbarItem {
    /// A slot with an identifier and a caption.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            display_mode: ToolbarItemDisplayMode::TextOnly,
            kind: ToolbarItemKind::Item,
            enabled: true,
        }
    }

    /// The same slot with an icon name attached.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// The same slot with a display mode.
    pub fn with_display_mode(mut self, mode: ToolbarItemDisplayMode) -> Self {
        self.display_mode = mode;
        self
    }

    /// A separator slot, which is never activated.
    pub fn separator(id: impl Into<String>) -> Self {
        Self { kind: ToolbarItemKind::Separator, ..Self::new(id, "") }
    }

    /// A flexible space, which absorbs the strip's spare width.
    pub fn flexible_space(id: impl Into<String>) -> Self {
        Self { kind: ToolbarItemKind::FlexibleSpace, ..Self::new(id, "") }
    }

    /// An empty slot for a host-supplied widget.
    pub fn widget_slot(id: impl Into<String>) -> Self {
        Self { kind: ToolbarItemKind::WidgetSlot, ..Self::new(id, "") }
    }

    /// The same slot, disabled.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// The width of a separator slot.
const SEPARATOR_WIDTH: u32 = 9;

/// One cached toolbar layout: every slot's rectangle and the folded-away indices.
///
/// `item_rect` used to recompute the running x from index 0 on every call and re-scan the
/// whole item list for each flexible space, and `overflow_indices` called `item_rect` once per
/// item — an almost O(n^2) chain on every draw and every pointer move (L-P11).
struct ToolbarLayout {
    strip: Rect,
    icon_bits: u32,
    rects: Vec<Option<Rect>>,
    /// The x of every slot, in item order, accumulated once while the rects are built.
    ///
    /// Drawing a separator used to call [`MacToolbar::slot_x`], which re-summed every preceding
    /// slot (and scanned all items twice per flexible space) for each separator on every frame.
    /// The rects cover only Item/WidgetSlot, so separators read this parallel list instead.
    separator_x: Vec<i32>,
    overflow: Vec<usize>,
}

/// A horizontal toolbar strip.
///
/// Slots are laid out left to right at a width derived from [`Self::icon_size`], so the painted
/// slot and the slot a click lands in are the same rectangle by construction — the hit test
/// never measures text. Activating a slot emits [`Self::item_activated`] with its index.
pub struct MacToolbar {
    base: BaseWidget,
    items: Vec<MacToolbarItem>,
    icon_size: f32,
    selectable: bool,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    pressed_index: Option<usize>,
    /// Emitted when a slot is activated, with the slot's index.
    pub item_activated: Signal1<usize>,
    /// Emitted with the first folded item's index when the overflow control is activated.
    pub overflow_activated: Signal1<usize>,
    /// Emitted with the number of items folded away whenever that count changes.
    pub overflow_changed: Signal1<usize>,
    /// The overflow count last announced, so an unchanged layout stays quiet.
    last_overflow_count: usize,
    /// Widgets installed into [`ToolbarItemKind::WidgetSlot`] slots, keyed by item index.
    slot_widgets: Vec<(usize, crate::compat::Box<dyn Widget>)>,
    /// Cached geometry for every slot, rebuilt when items or metrics change.
    layout_cache: Option<ToolbarLayout>,
}

impl MacToolbar {
    /// Creates an empty toolbar with the desktop default slot size.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolBar, geometry, "MacToolbar"),
            items: Vec::new(),
            icon_size: 24.0,
            selectable: false,
            selected_index: None,
            hovered_index: None,
            pressed_index: None,
            item_activated: Signal1::new(),
            overflow_activated: Signal1::new(),
            overflow_changed: Signal1::new(),
            last_overflow_count: 0,
            slot_widgets: Vec::new(),
            layout_cache: None,
        }
    }

    /// Replaces the slots, dropping any selection that no longer addresses one.
    pub fn set_items(&mut self, items: Vec<MacToolbarItem>) {
        if self.items == items {
            return;
        }
        self.items = items;
        self.layout_cache = None;
        self.hovered_index = self.hovered_index.filter(|index| *index < self.items.len());
        self.pressed_index = self.pressed_index.filter(|index| *index < self.items.len());
        self.selected_index = self.selected_index.filter(|index| *index < self.items.len());
        self.base.request_layout();
        self.base.request_redraw();
        let overflow = self.overflow_indices().len();
        if overflow != self.last_overflow_count {
            self.last_overflow_count = overflow;
            self.overflow_changed.emit(overflow);
        }
    }

    /// The slots, in display order.
    pub fn items(&self) -> &[MacToolbarItem] {
        &self.items
    }

    /// The number of slots.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// The icon size in points; the slot width is derived from it.
    pub fn icon_size(&self) -> f32 {
        self.icon_size
    }

    /// Sets the icon size. Slots are re-laid out, so the control requests a new layout.
    pub fn set_icon_size(&mut self, size: f32) {
        let size = if size.is_finite() { size.max(1.0) } else { return };
        if (size - self.icon_size).abs() < f32::EPSILON {
            return;
        }
        self.icon_size = size;
        self.layout_cache = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether activating a slot also selects it.
    pub fn selectable(&self) -> bool {
        self.selectable
    }

    /// Turns slot selection on or off. Turning it off clears the current selection.
    pub fn set_selectable(&mut self, selectable: bool) {
        if selectable == self.selectable {
            return;
        }
        self.selectable = selectable;
        if !selectable {
            self.selected_index = None;
        }
        self.base.request_redraw();
    }

    /// The selected slot, or `None` when the toolbar is not selectable or nothing is selected.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.filter(|index| *index < self.items.len())
    }

    /// Selects `index`, or clears the selection with a value at or beyond the item count.
    ///
    /// Only a selectable toolbar keeps a selection; a plain action toolbar reports `None`.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        let next = index.filter(|index| self.selectable && *index < self.items.len());
        if next == self.selected_index {
            return;
        }
        self.selected_index = next;
        self.base.request_redraw();
    }

    /// The strip the control paints: full width, one toolbar row tall, centred.
    fn strip(&self) -> Rect {
        centred_band(self.geometry(), MAC_TOOLBAR_HEIGHT)
    }

    /// The width of one slot: the icon's size plus its padding on both sides, with a floor
    /// wide enough to hold a short caption.
    ///
    /// The floor matters because the slot's rectangle is also its hit region: deriving the width
    /// from the measured caption would make the clickable area depend on the text, so the same
    /// toolbar could be clicked in different places depending on its labels.
    fn slot_width(&self) -> u32 {
        (self.icon_size.round().max(1.0) as u32 + 2 * MAC_TOOLBAR_ITEM_PADDING as u32)
            .max(MIN_SLOT_WIDTH)
    }

    /// The height of one slot, from the desktop profile's push-button floor.
    fn slot_height(&self) -> u32 {
        profile_metrics(ControlMetricsProfile::Macos)
            .button_min
            .height
            .min(self.strip().height.saturating_sub(2 * SLOT_VERTICAL_INSET))
    }

    /// The corner radius of a slot's hover or selection fill, from the desktop profile.
    fn slot_radius(&self) -> u32 {
        profile_metrics(ControlMetricsProfile::Macos).button_radius.min(self.slot_height() / 2)
    }

    /// The rectangle of slot `index`, or `None` when it is absent or runs past the strip.
    fn item_rect(&self, index: usize) -> Option<Rect> {
        if let Some(layout) = self.layout_cache.as_ref() {
            if layout.icon_bits == self.icon_size.to_bits()
                && layout.strip == self.strip()
                && layout.rects.len() == self.items.len()
            {
                return layout.rects.get(index).copied().flatten();
            }
        }
        let item = self.items.get(index)?;
        if !matches!(item.kind, ToolbarItemKind::Item | ToolbarItemKind::WidgetSlot) {
            return None;
        }
        let strip = self.strip();
        let width = self.slot_width();
        let height = self.slot_height();
        let rect = Rect::new(
            self.slot_x(index),
            strip.y + ((strip.height - height) / 2) as i32,
            width,
            height,
        );
        // A slot that does not fit is neither painted nor clickable: a half-visible slot the
        // hit test still answers for would be worse than one the host has to lay out itself.
        if rect.x < strip.x || rect.right() > strip.right() {
            return None;
        }
        Some(rect)
    }

    /// Rebuilds [`Self::layout_cache`] when the items or metrics changed.
    fn ensure_layout(&mut self) {
        let strip = self.strip();
        let icon_bits = self.icon_size.to_bits();
        if let Some(layout) = self.layout_cache.as_ref() {
            if layout.icon_bits == icon_bits
                && layout.strip == strip
                && layout.rects.len() == self.items.len()
            {
                return;
            }
        }
        let width = self.slot_width();
        let height = self.slot_height();
        let flexible = self.flexible_extent();
        let mut x = strip.x + MAC_TOOLBAR_INSET;
        let mut rects = vec![None; self.items.len()];
        let mut separator_x = Vec::with_capacity(self.items.len());
        for (index, item) in self.items.iter().enumerate() {
            let extent = match item.kind {
                ToolbarItemKind::Separator => SEPARATOR_WIDTH,
                ToolbarItemKind::FlexibleSpace => flexible,
                ToolbarItemKind::Item | ToolbarItemKind::WidgetSlot => width,
            };
            if matches!(item.kind, ToolbarItemKind::Item | ToolbarItemKind::WidgetSlot) {
                let rect =
                    Rect::new(x, strip.y + ((strip.height - height) / 2) as i32, width, height);
                if rect.x >= strip.x && rect.right() <= strip.right() {
                    rects[index] = Some(rect);
                }
            }
            separator_x.push(x);
            x += extent as i32 + SLOT_GAP;
        }
        let overflow = (0..self.items.len())
            .filter(|index| {
                self.items[*index].kind == ToolbarItemKind::Item && rects[*index].is_none()
            })
            .collect();
        self.layout_cache = Some(ToolbarLayout { strip, icon_bits, rects, separator_x, overflow });
    }

    /// Installs a widget into the [`ToolbarItemKind::WidgetSlot`] at `index`.
    ///
    /// Returns `false` when the index does not name a widget slot, so a caller cannot put a
    /// widget into an action slot where nothing would paint it.
    pub fn set_slot_widget(
        &mut self,
        index: usize,
        widget: crate::compat::Box<dyn Widget>,
    ) -> bool {
        if self.items.get(index).map(|item| item.kind) != Some(ToolbarItemKind::WidgetSlot) {
            return false;
        }
        if let Some(slot) = self.slot_widgets.iter_mut().find(|(slot, _)| *slot == index) {
            slot.1 = widget;
        } else {
            self.slot_widgets.push((index, widget));
        }
        if let Some(rect) = self.item_rect(index) {
            if let Some((_, widget)) = self.slot_widgets.iter_mut().find(|(slot, _)| *slot == index)
            {
                widget.set_geometry(rect);
            }
        }
        self.base.request_redraw();
        true
    }

    /// The widget installed in slot `index`, when one exists.
    pub fn slot_widget_mut(&mut self, index: usize) -> Option<&mut (dyn Widget + '_)> {
        self.slot_widgets
            .iter_mut()
            .find(|(slot, _)| *slot == index)
            .map(|(_, widget)| widget.as_mut())
    }

    /// The x of slot `index`, accounting for separators and flexible space.
    ///
    /// Kept as the fallback for callers that run before [`Self::ensure_layout`] built the
    /// cache; the per-frame separator paint reads [`ToolbarLayout::separator_x`] instead.
    fn slot_x(&self, index: usize) -> i32 {
        let strip = self.strip();
        let mut x = strip.x + MAC_TOOLBAR_INSET;
        for position in 0..index {
            x += self.slot_extent(position) as i32 + SLOT_GAP;
        }
        x
    }

    /// The cached x of slot `index`, falling back to [`Self::slot_x`] without a current layout.
    fn cached_slot_x(&self, index: usize) -> i32 {
        if let Some(layout) = self.layout_cache.as_ref() {
            if layout.icon_bits == self.icon_size.to_bits()
                && layout.strip == self.strip()
                && layout.separator_x.len() == self.items.len()
            {
                if let Some(x) = layout.separator_x.get(index) {
                    return *x;
                }
            }
        }
        self.slot_x(index)
    }

    /// The width slot `index` occupies.
    fn slot_extent(&self, index: usize) -> u32 {
        match self.items.get(index).map(|item| item.kind) {
            Some(ToolbarItemKind::Separator) => SEPARATOR_WIDTH,
            Some(ToolbarItemKind::FlexibleSpace) => self.flexible_extent(),
            _ => self.slot_width(),
        }
    }

    /// The width each flexible space takes: the strip's spare width split between them.
    fn flexible_extent(&self) -> u32 {
        let flex_count =
            self.items.iter().filter(|item| item.kind == ToolbarItemKind::FlexibleSpace).count()
                as u32;
        if flex_count == 0 {
            return 0;
        }
        let strip = self.strip();
        let fixed: i32 = self
            .items
            .iter()
            .map(|item| match item.kind {
                ToolbarItemKind::Separator => SEPARATOR_WIDTH as i32,
                ToolbarItemKind::FlexibleSpace => 0,
                ToolbarItemKind::Item | ToolbarItemKind::WidgetSlot => self.slot_width() as i32,
            })
            .sum::<i32>()
            + SLOT_GAP * self.items.len().saturating_sub(1) as i32;
        let spare = strip.width as i32 - 2 * MAC_TOOLBAR_INSET - fixed;
        (spare.max(0) / flex_count as i32) as u32
    }

    /// The indices of action slots that do not fit in the strip.
    pub fn overflow_indices(&self) -> Vec<usize> {
        if let Some(layout) = self.layout_cache.as_ref() {
            if layout.icon_bits == self.icon_size.to_bits()
                && layout.strip == self.strip()
                && layout.rects.len() == self.items.len()
            {
                return layout.overflow.clone();
            }
        }
        (0..self.items.len())
            .filter(|index| {
                self.items[*index].kind == ToolbarItemKind::Item && self.item_rect(*index).is_none()
            })
            .collect()
    }

    /// The rectangle of the overflow control, when at least one action slot is folded away.
    pub fn overflow_rect(&self) -> Option<Rect> {
        if self.overflow_indices().is_empty() {
            return None;
        }
        let strip = self.strip();
        let width = self.slot_width();
        let height = self.slot_height();
        Some(Rect::new(
            strip.right() - MAC_TOOLBAR_INSET - width as i32,
            strip.y + ((strip.height - height) / 2) as i32,
            width,
            height,
        ))
    }

    /// The number of action slots folded into the overflow control.
    pub fn overflow_count(&self) -> usize {
        self.overflow_indices().len()
    }

    /// The slot under `pos`, if any.
    fn item_at(&self, pos: Point) -> Option<usize> {
        (0..self.items.len())
            .find(|index| self.item_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// Activates slot `index`: selects it when selectable, then emits its activation.
    fn activate(&mut self, index: usize) {
        if index >= self.items.len()
            || self.items[index].kind != ToolbarItemKind::Item
            || !self.items[index].enabled
        {
            return;
        }
        if self.selectable {
            self.selected_index = Some(index);
        }
        self.base.request_redraw();
        self.item_activated.emit(index);
    }
}

impl Widget for MacToolbar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(MAC_TOOLBAR_DEFAULT_WIDTH, MAC_TOOLBAR_HEIGHT)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::ToolBar
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves a published event name to the control's own signal.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "item_activated" => {
                Some(EventSignalRef::mapped("item_activated", &self.item_activated, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "overflow_activated" => Some(EventSignalRef::mapped(
                "overflow_activated",
                &self.overflow_activated,
                |value| CapabilityValue::UInt(*value as u64),
            )),
            "overflow_changed" => {
                Some(EventSignalRef::mapped("overflow_changed", &self.overflow_changed, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for MacToolbar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "icon_size" => Ok(CapabilityValue::Float(self.icon_size() as f64)),
            // A toolbar laid out by this control is horizontal by construction.
            "orientation" => Ok(CapabilityValue::String("horizontal".to_string())),
            "selectable" => Ok(CapabilityValue::Bool(self.selectable())),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "overflow_count" => Ok(CapabilityValue::UInt(self.overflow_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "icon_size" => {
                self.set_icon_size(expect_f32(value)?);
                Ok(())
            }
            "selectable" => {
                self.set_selectable(expect_bool(value)?);
                Ok(())
            }
            "selected_index" => match value {
                CapabilityValue::Null => {
                    self.set_selected_index(None);
                    Ok(())
                }
                other => {
                    self.set_selected_index(Some(expect_usize(other)?));
                    Ok(())
                }
            },
            "item_count" | "orientation" | "overflow_count" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "item_count",
            "icon_size",
            "orientation",
            "selectable",
            "selected_index",
            "overflow_count",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for MacToolbar {
    fn draw(&mut self, context: &mut RenderContext) {
        let strip = self.strip();
        if strip.width == 0 || strip.height == 0 {
            return;
        }
        self.ensure_layout();

        let colors = palette();
        let style = self.base.style();
        let font = role_font(style, 12.0);
        let ink = if self.base.is_enabled() { colors.ink } else { colors.muted };

        context.fill_rect(strip, colors.raised);
        // The hairline that separates the strip from the content below it.
        context.draw_line(
            Point::new(strip.x, strip.bottom() - 1),
            Point::new(strip.right() - 1, strip.bottom() - 1),
            colors.separator,
        );

        for index in 0..self.items.len() {
            let Some(item) = self.items.get(index) else {
                continue;
            };
            match item.kind {
                ToolbarItemKind::FlexibleSpace => continue,
                ToolbarItemKind::Separator => {
                    let strip_inner = self.strip();
                    let x = self.cached_slot_x(index) + SEPARATOR_WIDTH as i32 / 2;
                    context.draw_line(
                        Point::new(x, strip_inner.y + SLOT_VERTICAL_INSET as i32),
                        Point::new(x, strip_inner.bottom() - SLOT_VERTICAL_INSET as i32 - 1),
                        colors.separator,
                    );
                    continue;
                }
                ToolbarItemKind::Item => {}
                // A widget slot paints the host's widget in the slot rectangle below.
                ToolbarItemKind::WidgetSlot => {}
            }
            let Some(slot) = self.item_rect(index) else {
                continue;
            };
            if item.kind == ToolbarItemKind::WidgetSlot {
                if let Some((_, widget)) =
                    self.slot_widgets.iter_mut().find(|(slot_index, _)| *slot_index == index)
                {
                    widget.set_geometry(slot);
                    if let Some(drawable) = crate::widget::draw_bridge::draw_of(widget.as_mut()) {
                        drawable.draw(context);
                    }
                }
                continue;
            }
            let selected = self.selected_index() == Some(index);
            let active = selected || self.pressed_index == Some(index);
            let hovered = self.hovered_index == Some(index);
            if active {
                context.fill_rounded_rect(slot, self.slot_radius(), colors.accent);
            } else if hovered && item.enabled {
                context.fill_rounded_rect(slot, self.slot_radius(), colors.surface);
            }

            let label_ink = if active {
                colors.accent.contrast_color()
            } else if !item.enabled {
                colors.muted
            } else {
                ink
            };
            let available = slot.width.saturating_sub(2 * MAC_TOOLBAR_ITEM_PADDING as u32);
            if available == 0 {
                continue;
            }
            let icon_side = (self.icon_size.round().max(1.0) as u32).min(slot.height);
            let icon_rect =
                Rect::new(slot.center().x - icon_side as i32 / 2, slot.y + 2, icon_side, icon_side);
            let mut drew_icon = false;
            if item.display_mode != ToolbarItemDisplayMode::TextOnly {
                if let Some(name) = item.icon.as_deref() {
                    drew_icon = crate::widget::draw_icon_named(context, icon_rect, label_ink, name);
                }
            }
            if item.display_mode == ToolbarItemDisplayMode::IconOnly {
                if !drew_icon {
                    let mut measure = |text: &str| context.measure_text(text, &font).width;
                    let shown: Cow<'_, str> = elide_to_width(&item.label, available, &mut measure);
                    let line = context.text_line(slot, &font);
                    context.draw_text(
                        Point::new(slot.center().x, line.y),
                        shown.as_ref(),
                        &font,
                        label_ink,
                        HorizontalAlignment::Center,
                    );
                }
                continue;
            }
            // Text-only, or the caption below an icon.
            let label_font = if drew_icon { role_font(style, 10.0) } else { font.clone() };
            let mut measure = |text: &str| context.measure_text(text, &label_font).width;
            let shown: Cow<'_, str> = elide_to_width(&item.label, available, &mut measure);
            let line = if drew_icon {
                let label_h = context.measure_text("M", &label_font).height;
                Rect::new(
                    slot.x,
                    icon_rect.bottom() + 1,
                    slot.width,
                    label_h.min(slot.height.saturating_sub(icon_side + 3)),
                )
            } else {
                context.text_line(slot, &font)
            };
            context.draw_text(
                Point::new(slot.center().x, line.y),
                shown.as_ref(),
                &label_font,
                label_ink,
                HorizontalAlignment::Center,
            );
        }

        // ── Overflow control ──
        // A slot that does not fit is folded into this trailing control instead of vanishing, so
        // its command remains reachable.
        if let Some(overflow) = self.overflow_rect() {
            context.fill_rounded_rect(overflow, self.slot_radius(), colors.surface);
            context.draw_rounded_rect_stroke(overflow, self.slot_radius(), colors.separator, 1);
            let line = context.text_line(overflow, &font);
            context.draw_text(
                Point::new(overflow.center().x, line.y),
                "...",
                &font,
                ink,
                HorizontalAlignment::Center,
            );
        }

        draw_focus_ring(context, strip, 0, &self.base, colors.accent);
    }
}

impl EventHandler for MacToolbar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                let hovered = self.item_at(*pos);
                if hovered != self.hovered_index {
                    self.hovered_index = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_index.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(overflow) = self.overflow_rect() {
                    if overflow.contains_point(*pos) {
                        if let Some(&first) = self.overflow_indices().first() {
                            self.overflow_activated.emit(first);
                            self.base.request_redraw();
                        }
                        return;
                    }
                }
                if let Some(index) = self.item_at(*pos) {
                    if self.items.get(index).map(|item| item.kind)
                        == Some(ToolbarItemKind::WidgetSlot)
                    {
                        if let Some((_, widget)) =
                            self.slot_widgets.iter_mut().find(|(slot, _)| *slot == index)
                        {
                            widget.handle_event(event);
                        }
                        return;
                    }
                    self.pressed_index = Some(index);
                    self.activate(index);
                }
            }
            // The item the pointer went down on, if any; the release redraws only when a press
            // was actually latched.
            Event::MouseRelease { .. } if self.pressed_index.take().is_some() => {
                self.base.request_redraw();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Recorder;
    use super::*;
    use crate::core::Color;
    use crate::render::RenderCommand;
    use std::sync::{Arc, Mutex};

    fn sample() -> MacToolbar {
        let mut toolbar = MacToolbar::new(Rect::new(0, 0, 320, 38));
        toolbar.set_items(vec![
            MacToolbarItem::new("new", "New"),
            MacToolbarItem::new("open", "Open"),
            MacToolbarItem::new("save", "Save"),
        ]);
        toolbar
    }

    #[test]
    fn a_fresh_toolbar_has_no_slots_and_the_desktop_metrics() {
        let toolbar = MacToolbar::new(Rect::new(0, 0, 320, 38));
        assert_eq!(toolbar.item_count(), 0);
        assert_eq!(toolbar.icon_size(), 24.0);
        assert!(!toolbar.selectable());
        assert_eq!(toolbar.size_hint(), Size::new(MAC_TOOLBAR_DEFAULT_WIDTH, MAC_TOOLBAR_HEIGHT));
    }

    #[test]
    fn slots_are_laid_out_left_to_right_without_overlapping() {
        let toolbar = sample();
        let first = toolbar.item_rect(0).expect("the first slot fits");
        let second = toolbar.item_rect(1).expect("the second slot fits");
        assert_eq!(first.width, toolbar.slot_width());
        assert_eq!(second.x, first.right() + SLOT_GAP);
        assert!(toolbar.item_rect(3).is_none(), "there is no fourth slot");
    }

    #[test]
    fn a_slot_that_runs_past_the_strip_is_neither_painted_nor_clickable() {
        let mut toolbar = MacToolbar::new(Rect::new(0, 0, 60, 38));
        toolbar.set_items(vec![
            MacToolbarItem::new("a", "A"),
            MacToolbarItem::new("b", "B"),
            MacToolbarItem::new("c", "C"),
        ]);
        let slots: Vec<_> = (0..3).filter_map(|index| toolbar.item_rect(index)).collect();
        for slot in &slots {
            assert!(slot.right() <= 60);
        }
        assert!(slots.len() < 3, "a narrow strip cannot show every slot");
    }

    #[test]
    fn clicking_a_slot_emits_its_index() {
        let mut toolbar = sample();
        let seen = Arc::new(Mutex::new(usize::MAX));
        toolbar.item_activated.connect({
            let seen = Arc::clone(&seen);
            move |index: Arc<usize>| {
                *seen.lock().unwrap() = *index;
            }
        });
        let slot = toolbar.item_rect(1).expect("the second slot fits");
        toolbar.handle_event(&Event::MousePress { pos: slot.center(), button: 1, modifiers: 0 });
        assert_eq!(*seen.lock().unwrap(), 1);
    }

    #[test]
    fn a_selectable_toolbar_keeps_the_activated_slot_selected() {
        let mut toolbar = sample();
        assert!(toolbar.selected_index().is_none());
        toolbar.set_selectable(true);
        let slot = toolbar.item_rect(2).expect("the third slot fits");
        toolbar.handle_event(&Event::MousePress { pos: slot.center(), button: 1, modifiers: 0 });
        assert_eq!(toolbar.selected_index(), Some(2));

        // Turning selection off clears it rather than leaving a stale index behind.
        toolbar.set_selectable(false);
        assert!(toolbar.selected_index().is_none());
    }

    #[test]
    fn the_property_route_reads_and_writes_every_published_name() {
        let mut toolbar = sample();
        assert_eq!(toolbar.get("item_count"), Ok(CapabilityValue::UInt(3)));
        assert_eq!(
            toolbar.get("orientation"),
            Ok(CapabilityValue::String("horizontal".to_string()))
        );
        toolbar.set("icon_size", CapabilityValue::Float(32.0)).unwrap();
        assert_eq!(toolbar.get("icon_size"), Ok(CapabilityValue::Float(32.0)));
        toolbar.set("selectable", CapabilityValue::Bool(true)).unwrap();
        assert_eq!(toolbar.get("selectable"), Ok(CapabilityValue::Bool(true)));
        toolbar.set("selected_index", CapabilityValue::UInt(1)).unwrap();
        assert_eq!(toolbar.get("selected_index"), Ok(CapabilityValue::UInt(1)));
        toolbar.set("selected_index", CapabilityValue::Null).unwrap();
        assert_eq!(toolbar.get("selected_index"), Ok(CapabilityValue::Null));
        assert_eq!(
            toolbar.set("item_count", CapabilityValue::UInt(0)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn the_strip_paints_its_hairline_and_every_visible_slot() {
        let mut toolbar = sample();
        let mut recorder = Recorder::new(320, 38);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            toolbar.draw(&mut context);
        }
        recorder.end_frame();

        let fills = recorder
            .commands
            .iter()
            .filter(|command| matches!(command, RenderCommand::FillRect { .. }))
            .count();
        assert!(fills >= 1, "the strip itself is filled");
        let labels: Vec<&str> = recorder
            .commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::DrawText { text, .. } => Some(text.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, vec!["New", "Open", "Save"]);
    }

    #[test]
    fn a_hovered_slot_paints_an_indicator_and_leaving_clears_it() {
        let mut toolbar = sample();
        let slot = toolbar.item_rect(0).expect("the first slot fits");
        toolbar.handle_event(&Event::MouseMove { pos: slot.center() });
        assert_eq!(toolbar.hovered_index, Some(0));
        toolbar.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert_eq!(toolbar.hovered_index, None);
    }

    #[test]
    fn overflow_slots_stay_reachable_through_the_overflow_control() {
        let mut toolbar = MacToolbar::new(Rect::new(0, 0, 120, 38));
        toolbar.set_items(vec![
            MacToolbarItem::new("a", "A"),
            MacToolbarItem::new("b", "B"),
            MacToolbarItem::new("c", "C"),
        ]);
        assert!(toolbar.overflow_count() > 0, "a narrow strip folds slots");
        let first = *toolbar.overflow_indices().first().expect("something is folded");
        let overflow = toolbar.overflow_rect().expect("the overflow control is present");

        let seen = Arc::new(Mutex::new(usize::MAX));
        let s = Arc::clone(&seen);
        toolbar.overflow_activated.connect(move |index: Arc<usize>| {
            *s.lock().unwrap() = *index;
        });
        toolbar.handle_event(&Event::MousePress {
            pos: overflow.center(),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(*seen.lock().unwrap(), first);
    }

    #[test]
    fn separators_and_flexible_space_have_their_own_layout() {
        let mut toolbar = MacToolbar::new(Rect::new(0, 0, 320, 38));
        toolbar.set_items(vec![
            MacToolbarItem::new("a", "A"),
            MacToolbarItem::separator("sep"),
            MacToolbarItem::flexible_space("flex"),
            MacToolbarItem::new("b", "B"),
        ]);
        assert!(toolbar.item_rect(1).is_none(), "a separator is not an action slot");
        assert!(toolbar.item_rect(2).is_none(), "a flexible space is not an action slot");
        let first = toolbar.item_rect(0).expect("the first action fits");
        let last = toolbar.item_rect(3).expect("the last action is pushed right by the flex");
        assert!(
            last.x > first.right() + SEPARATOR_WIDTH as i32,
            "flexible space pushes later slots to the right: {last:?} vs {first:?}"
        );
    }

    #[test]
    fn display_modes_and_overflow_changes_are_observable() {
        let mut toolbar = MacToolbar::new(Rect::new(0, 0, 120, 38));
        let counts = Arc::new(Mutex::new(Vec::new()));
        let c = Arc::clone(&counts);
        toolbar.overflow_changed.connect(move |count: Arc<usize>| {
            c.lock().unwrap().push(*count);
        });
        toolbar.set_items(vec![
            MacToolbarItem::new("a", "A")
                .with_icon("home")
                .with_display_mode(ToolbarItemDisplayMode::IconAndText),
            MacToolbarItem::new("b", "B"),
            MacToolbarItem::new("c", "C"),
        ]);
        assert!(!counts.lock().unwrap().is_empty(), "the overflow count was announced");
        let mut recorder = Recorder::new(120, 38);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            toolbar.draw(&mut context);
        }
        recorder.end_frame();
        let texts: Vec<&str> = recorder
            .commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::DrawText { text, .. } => Some(text.as_ref()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"..."), "the overflow control is painted: {texts:?}");
    }
}
