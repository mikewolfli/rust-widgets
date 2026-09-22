// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu bar widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// A top-level menu entry in the menu bar.
///
/// Entries carry only a title and an enabled flag; the menu contents they open
/// live in a separate menu model, not here.
#[derive(Debug, Clone)]
pub struct MenuBarEntry {
    title: String,
    enabled: bool,
}
impl MenuBarEntry {
    /// Creates an enabled entry with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), enabled: true }
    }

    // --- Accessors ---

    /// Returns the label drawn in the bar.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the label. The bar's layout is derived from title width, so
    /// changing this shifts the entries that follow it.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Returns whether the entry accepts mouse input.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the entry.
    ///
    /// A disabled entry is still drawn, in a greyed colour, but click and hover
    /// do not activate it — in particular no `triggered` or `hovered_entry`
    /// signal is emitted for it.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}
/// Menu bar widget.
///
/// A horizontal strip of top-level entries, exactly one of which may be active
/// (i.e. its drop-down is considered open). The bar manages only the entries,
/// the pointer cursor, and the signals; it does not own or display drop-down
/// menus, so a consumer is expected to open the matching menu in response to
/// [`MenuBar::triggered`].
///
/// Entry hit areas are estimated from title length rather than measured against
/// real glyph advances, so very wide or narrow fonts can make the clickable
/// regions disagree with the drawn text.
///
pub struct MenuBar {
    base: BaseWidget,
    entries: Vec<MenuBarEntry>,
    active_index: Option<usize>,
    hovered_index: Option<usize>,
    /// Emitted with the clicked entry's title when an enabled entry is pressed
    /// with the primary mouse button. The payload is the title text, not the
    /// index — duplicate titles are therefore indistinguishable, and renaming
    /// an entry after the fact makes previously received payloads stale.
    pub triggered: Signal1<String>,
    /// Emitted with the title of the entry the pointer newly entered. Not
    /// emitted when the pointer moves to a different disabled entry, and not
    /// re-emitted while the pointer stays on the same entry.
    pub hovered_entry: Signal1<String>,
}
impl MenuBar {
    /// Creates an empty bar with no active or hovered entry.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 400x28.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MenuBar, geometry, "MenuBar"),
            entries: Vec::new(),
            active_index: None,
            hovered_index: None,
            triggered: Signal1::new(),
            hovered_entry: Signal1::new(),
        }
    }
    /// Returns the entries in visual order.
    pub fn entries(&self) -> &[MenuBarEntry] {
        &self.entries
    }
    /// Returns the index of the active entry, or `None` when no entry is
    /// active.
    ///
    /// "Active" means the entry was last clicked. It is cleared by pressing
    /// Escape and by [`MenuBar::clear`], but **not** by clicking a different
    /// entry (that entry simply becomes the new active one) nor by
    /// [`MenuBar::remove_menu`].
    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }
    /// Returns the index currently under the pointer, or `None` when the
    /// pointer is outside every entry.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }
    /// Appends an entry and returns its index.
    ///
    /// Indices are positions in the list and therefore shift when an earlier
    /// entry is removed.
    pub fn add_menu(&mut self, title: impl Into<String>) -> usize {
        let idx = self.entries.len();
        self.entries.push(MenuBarEntry::new(title));
        idx
    }
    /// Removes the entry at `index`; out-of-range indices are ignored.
    ///
    /// Indices after the removal point shift down by one, but
    /// [`MenuBar::active_index`] and [`MenuBar::hovered_index`] are **not**
    /// adjusted, so a cursor can end up pointing at a different entry than the
    /// one it was set for. The active entry can also be removed entirely,
    /// leaving `active_index` out of range of the list.
    pub fn remove_menu(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }
    /// Enables or disables the entry at `index`. Out-of-range indices are
    /// ignored, but a redraw is still requested in that case.
    pub fn set_menu_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(e) = self.entries.get_mut(index) {
            e.set_enabled(enabled);
        }
        self.base.request_redraw();
    }
    /// Returns whether the entry at `index` is enabled, or `None` when the
    /// index is out of range.
    pub fn menu_enabled(&self, index: usize) -> Option<bool> {
        self.entries.get(index).map(|entry| entry.is_enabled())
    }
    /// Removes every entry and clears both the active and hovered cursors.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.active_index = None;
        self.hovered_index = None;
    }
    /// The strip the bar actually paints: a full-width band
    /// [`dimensions::MENU_BAR_HEIGHT`] tall, pinned to the **top** of the area the control was
    /// given.
    ///
    /// # Why the bar is a top band and not a centred one
    ///
    /// A menu bar is not centred chrome: the menu contents it opens begin *below* it, so
    /// whatever the layout handed the bar, the strip belongs on the control's top edge.
    /// `draw` painted `geometry()` while `size_hint` reported 28 — the two disagreed about how
    /// thick a menu bar is, and the 240x120 census cell drew a 120 px band, four times the
    /// strip a layout was told to expect (`menu_bar.svg` was a full-canvas fill). The two ends
    /// now share one derivation. This is also what `entry_width` and `hit_entry` read, so the
    /// clickable strip is the painted strip.
    fn band_rect(&self) -> Rect {
        ControlMetrics::top_band(self.geometry(), dimensions::MENU_BAR_HEIGHT)
    }

    fn entry_width(title: &str) -> f32 {
        // Approximate width: 8 pixels per char + 16 padding
        title.len() as f32 * 8.0 + 16.0
    }
    fn _entry_rect(&self, index: usize) -> Rect {
        let rect = self.band_rect();
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = Self::entry_width(entry.title()) as i32;
            if i == index {
                return Rect { x, y: rect.y, width: w as u32, height: rect.height };
            }
            x += w;
        }
        Rect { x: 0, y: 0, width: 0, height: 0 }
    }
    fn hit_entry(&self, pos: Point) -> Option<usize> {
        // The **painted band**, so a click below a 28 px strip is not a click on the bar.
        let rect = self.band_rect();
        if pos.y < rect.y || pos.y > rect.y + rect.height as f32 as i32 {
            return None;
        }
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = Self::entry_width(entry.title()) as i32;
            if pos.x >= x && pos.x < x + w {
                return Some(i);
            }
            x += w;
        }
        None
    }
}
impl Widget for MenuBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, dimensions::MENU_BAR_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MenuBar`'s property contract.
///
/// Every published property is a read-only projection of the bar's current
/// contents and pointer state; the schema advertises none of them as writable,
/// so the write path forwards straight to the base helpers.
impl WidgetProperties for MenuBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entry_count" => Ok(CapabilityValue::UInt(self.entries().len() as u64)),
            "active_index" => match self.active_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "hovered_index" => match self.hovered_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The entry vector and the hover/active cursors are owned by the
            // control's own input handling, so they exist but refuse writes.
            "entry_count" | "active_index" | "hovered_index" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["entry_count", "active_index", "hovered_index", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `menu_bar` publishes.
    ///
    /// `clear` is the one genuine zero-argument action here: it drops every entry
    /// and resets both cursors, which is exactly what [`MenuBar::clear`] does.
    /// `add_menu` takes the entry's title and `remove_menu` takes the index to
    /// remove, so a payload-less invocation of either is refused as
    /// [`CapabilityAccessError::OutOfRange`] — the names are right and the argument
    /// is what is missing, which is not `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "add_menu" | "remove_menu" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for MenuBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let prev = self.hovered_index;
                self.hovered_index = self.hit_entry(*pos);
                if self.hovered_index != prev {
                    if let Some(idx) = self.hovered_index {
                        if self.entries[idx].is_enabled() {
                            let title = self.entries[idx].title().to_string();
                            self.hovered_entry.emit(title);
                        }
                    }
                }
            }
            Event::MousePress { pos, button: 1 } => {
                if let Some(idx) = self.hit_entry(*pos) {
                    if self.entries[idx].is_enabled() {
                        self.active_index = Some(idx);
                        let title = self.entries[idx].title().to_string();
                        self.triggered.emit(title);
                    }
                }
            }
            Event::KeyPress { key, .. } if *key == 27 => {
                self.active_index = None;
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for MenuBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **band**, not the control's rectangle: see `band_rect`. The fill, the bottom
        // rule, the entry highlights and the labels are all placed from this one box, so a
        // 240x120 census cell draws a 28 px strip rather than a 120 px slab.
        let rect = self.band_rect();

        // A menu bar is very nearly all chrome — its fill, its separators, its entry
        // highlights and its labels. Every one of those was a literal, so a light/dark
        // switch repainted nothing and the rendering census reported the control as
        // theme-blind.
        //
        // Chrome colours resolve the explicit style first, then the theme's resolved style
        // for this control, and only then the original literal. The literal stays as the
        // fallback so an inactive theme still has a defined appearance. The theme read is a
        // separate manager lock, taken and released inside `resolved_theme_style`, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let themed = crate::style::resolved_theme_style("menu_bar");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        let themed_border = themed.as_ref().and_then(|r| r.border_color);
        let themed_text = themed.as_ref().and_then(|r| r.text_color);
        // `menu_bar` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A bar
        // painted in that colour is byte-identical to the frame behind it, so the resolved
        // surface is re-derived a visible step towards the theme's ink; a colour the caller
        // set still wins.
        let base = style.background_color.or(themed_bg).unwrap_or(Color::rgb(240, 240, 240));
        let ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(0, 0, 0));
        let background = base.blend(&ink, 0.06);
        // The bottom rule is chrome, and its literal was darker than the bar; deriving it
        // from the resolved pair keeps that relationship in either appearance.
        let separator =
            style.border_color.or(themed_border).unwrap_or_else(|| background.blend(&ink, 0.32));
        // The active entry is filled with the accent token (the "open menu" indicator),
        // the hovered one with a lighter step of the same family, and the label over each
        // is picked for contrast rather than hardcoded white-on-accent.
        let active_fill = crate::style::theme_manager()
            .current_theme()
            .map(|theme| theme.colors.primary)
            .unwrap_or(Color::rgb(0, 120, 215));
        let hover_fill = background.blend(&active_fill, 0.25);
        let disabled_ink = ink.blend(&background, 0.5);

        // Menu bar background
        context.fill_rect(rect, background);
        context.draw_line(
            Point::new(rect.x, rect.y + rect.height as i32 - 1),
            Point::new(rect.x + rect.width as i32, rect.y + rect.height as i32 - 1),
            separator,
        );
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = Self::entry_width(entry.title()) as i32;
            let is_hovered = self.hovered_index == Some(i);
            let is_active = self.active_index == Some(i);
            let entry_rect = Rect { x, y: rect.y, width: w as u32, height: rect.height };
            if is_active {
                context.fill_rect(entry_rect, active_fill);
            } else if is_hovered {
                context.fill_rect(entry_rect, hover_fill);
            }
            let fg = if !entry.is_enabled() {
                disabled_ink
            } else if is_active {
                active_fill.contrast_color()
            } else {
                ink
            };
            // Centre the label inside its own entry **on both axes**.
            //
            // The previous form was `Point::new(x + w / 2, rect.y + rect.height / 2)` with
            // `HorizontalAlignment::Left`. Two separate errors came out of that one line: the
            // horizontal term computed the entry's *midpoint* and then used it as a left-hand
            // origin, and the vertical term put the glyph box's top edge on the bar's middle
            // line. `entry_width` is `len * 8 + 16`, so a four-character title in a 48 px entry
            // advanced 32 px from `x + 24` — past the end of its own entry and 9.6 px over the
            // next title. Passing the entry rectangle and asking for `Center` states the
            // intent, and there is no midpoint left to misuse.
            let font = Font::default();
            let line = context.text_line(entry_rect, &font);
            context.draw_text_fitted(line, entry.title(), &font, fg, HorizontalAlignment::Center);
            x += w;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menubar_menu_enabled_accessor_handles_valid_and_oob_indices() {
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        let idx = menu_bar.add_menu("File");

        assert_eq!(menu_bar.menu_enabled(idx), Some(true));
        menu_bar.set_menu_enabled(idx, false);
        assert_eq!(menu_bar.menu_enabled(idx), Some(false));
        assert_eq!(menu_bar.menu_enabled(99), None);
    }
}
