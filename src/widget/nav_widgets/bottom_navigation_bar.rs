// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BottomNavigationBar widget — a mobile-style bottom tab bar with icons and labels.
//!
//! This widget provides a Material Design-style bottom navigation bar commonly
//! found in mobile applications. It displays a horizontal bar with equally-sized
//! tabs, each showing an icon character and a label below. The selected tab is
//! highlighted with the accent color, and tapping a tab emits `current_changed`
//! with the new index.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconItemError, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A single item in the bottom navigation bar.
///
/// Each item displays a single icon character above a text label.
#[derive(Debug, Clone)]
pub struct NavItem {
    /// Icon character displayed above the label (e.g., "★", "☰", "✉").
    pub icon: String,
    /// Text label displayed below the icon.
    pub label: String,
}

/// Which items draw their label (N-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavLabelBehavior {
    /// Every item draws its label, selected or not.
    #[default]
    Always,
    /// Only the selected item draws its label.
    SelectedOnly,
    /// No item draws its label: an icon-only bar.
    Never,
}

impl NavLabelBehavior {
    /// The token the property contract publishes.
    pub fn as_str(self) -> &'static str {
        match self {
            NavLabelBehavior::Always => "always",
            NavLabelBehavior::SelectedOnly => "selected_only",
            NavLabelBehavior::Never => "never",
        }
    }

    /// Parses a published token.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "always" => NavLabelBehavior::Always,
            "selected_only" => NavLabelBehavior::SelectedOnly,
            "never" => NavLabelBehavior::Never,
            _ => return None,
        })
    }
}

/// BottomNavigationBar widget — a mobile-style bottom tab bar.
///
/// Renders a horizontal bar across the bottom of the screen with equal-width
/// tabs. Each tab shows an icon character and a label. The selected tab is
/// drawn with the PRIMARY color for icon and label, while unselected tabs
/// use a muted MIDDLE_GRAY color. A thin accent indicator line is drawn
/// above the selected tab's content area.
pub struct BottomNavigationBar {
    base: BaseWidget,
    items: Vec<NavItem>,
    item_icon_sources: Vec<Option<IconSource>>,
    selected_index: usize,
    /// How far the selection pill has travelled, `0.0` just after leaving its tab and `1.0`
    /// arrived.
    ///
    /// # Why this is a slide and not a repaint
    ///
    /// The pill used to be drawn inside the loop, at whichever tab was selected, so changing tab
    /// made it disappear from one and appear under another with nothing in between. On a bar
    /// whose entire job is to say *where you are*, that transition is the only thing that says
    /// **which way** you moved — a hard cut leaves the user to re-read all five tabs.
    ///
    /// A fraction rather than a pixel offset, because a `PropertyDriver` interpolates a progress
    /// (its target is clamped to `0..=1`) and because a fraction survives a relayout that moves
    /// every tab.
    slide: PropertyDriver,
    /// The tab the current slide started from, so `slide`'s fraction has a left endpoint.
    slide_from: usize,
    /// Per-item badge text, parallel to `items`.
    badges: Vec<Option<String>>,
    /// Per-item availability, parallel to `items` (N-04).
    item_enabled: Vec<bool>,
    /// Which items draw their label (N-03).
    label_behavior: NavLabelBehavior,
    /// Material's upper bound on destinations, when the host opts into it.
    max_items: Option<usize>,
    /// Whether scrolling down hides the whole bar.
    hide_on_scroll: bool,
    /// How much of the bar is visible, `0.0` hidden to `1.0` shown.
    visibility: PropertyDriver,
    /// Emitted when the selected tab changes. The payload is the new index.
    pub current_changed: Signal1<usize>,
}

impl BottomNavigationBar {
    /// Creates a new empty BottomNavigationBar with the given geometry.
    ///
    /// No items are added initially. Use `add_item()` to populate tabs.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::BottomNavigationBar, geometry, "BottomNavigationBar"),
            items: Vec::new(),
            item_icon_sources: Vec::new(),
            selected_index: 0,
            slide: PropertyDriver::at(1.0, MotionSlot::Normal),
            slide_from: 0,
            badges: Vec::new(),
            item_enabled: Vec::new(),
            label_behavior: NavLabelBehavior::Always,
            max_items: None,
            hide_on_scroll: false,
            visibility: PropertyDriver::at(1.0, MotionSlot::Normal),
            current_changed: Signal1::new(),
        }
    }

    /// Compatibility spelling of [`Self::current_changed`] (W-20): the same emitted signal.
    pub fn selected_changed(&self) -> &Signal1<usize> {
        &self.current_changed
    }

    /// Adds a navigation item with the given icon character and label text.
    ///
    /// # Arguments
    ///
    /// * `icon` — A single-character icon string (e.g., `"★"`, `"☰"`, `"✉"`, `"⌂"`).
    /// * `label` — Text label displayed below the icon.
    pub fn add_item(&mut self, icon: &str, label: &str) {
        let _ = self.try_add_item(icon, label);
    }

    /// Adds an item when the optional destination limit allows it.
    ///
    /// Returns `false` when [`Self::set_max_items`] has installed a bound and the bar is already
    /// at it. The check lives here so a host that cares can report the refusal instead of
    /// silently ending up with a list longer than the bar can present.
    pub fn try_add_item(&mut self, icon: &str, label: &str) -> bool {
        if self.max_items.is_some_and(|limit| self.items.len() >= limit) {
            return false;
        }
        self.items.push(NavItem { icon: icon.to_string(), label: label.to_string() });
        self.item_icon_sources.push(None);
        self.badges.push(None);
        self.item_enabled.push(true);
        self.base.request_redraw();
        true
    }

    /// Sets a badge on an existing item, or clears it with `None`.
    ///
    /// Returns `false` for an index that is not on the bar.
    pub fn set_badge(&mut self, index: usize, badge: Option<impl Into<String>>) -> bool {
        let Some(slot) = self.badges.get_mut(index) else {
            return false;
        };
        let badge = badge.map(Into::into);
        if *slot == badge {
            return true;
        }
        *slot = badge;
        self.base.request_redraw();
        true
    }

    /// Returns the badge on `index`, when there is one.
    pub fn badge(&self, index: usize) -> Option<&str> {
        self.badges.get(index).and_then(|badge| badge.as_deref())
    }

    /// Whether unselected items still draw their labels.
    pub fn show_unselected_labels(&self) -> bool {
        matches!(self.label_behavior, NavLabelBehavior::Always)
    }

    /// Sets whether unselected items draw their labels.
    ///
    /// Hiding them is the compact Material shape: the selected destination is named, the rest
    /// are read from their icons. `false` maps to [`NavLabelBehavior::SelectedOnly`], so every
    /// existing caller keeps the two states it already knew.
    pub fn set_show_unselected_labels(&mut self, show: bool) {
        let behavior = if show { NavLabelBehavior::Always } else { NavLabelBehavior::SelectedOnly };
        self.set_label_behavior(behavior);
    }

    /// Returns the three-state label policy (N-03).
    pub fn label_behavior(&self) -> NavLabelBehavior {
        self.label_behavior
    }

    /// Sets the three-state label policy.
    ///
    /// [`NavLabelBehavior::Never`] is the icon-only bar that the two-state boolean could not
    /// express.
    pub fn set_label_behavior(&mut self, behavior: NavLabelBehavior) {
        if self.label_behavior == behavior {
            return;
        }
        self.label_behavior = behavior;
        self.base.request_redraw();
    }

    /// Whether `index` may be selected (N-04).
    ///
    /// An index that is not on the bar reports `false`, so a caller cannot mistake an
    /// out-of-range index for an enabled one.
    pub fn is_item_enabled(&self, index: usize) -> bool {
        self.item_enabled.get(index).copied().unwrap_or(false)
    }

    /// Enables or disables one destination (N-04).
    ///
    /// Returns `false` for an index that is not on the bar. The overall control's `enabled`
    /// state still applies on top: a disabled bar disables every item.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) -> bool {
        let Some(slot) = self.item_enabled.get_mut(index) else {
            return false;
        };
        if *slot == enabled {
            return true;
        }
        *slot = enabled;
        self.base.request_redraw();
        true
    }

    /// The optional destination limit, if one is installed.
    pub fn max_items(&self) -> Option<usize> {
        self.max_items
    }

    /// Installs an upper bound on the number of destinations.
    ///
    /// Material's bottom bar is specified for three to five destinations; a host that wants
    /// that rule enforced can install the bound and use [`Self::try_add_item`]. `None` removes
    /// the bound and keeps the historical unbounded list.
    pub fn set_max_items(&mut self, limit: Option<usize>) {
        if self.max_items == limit {
            return;
        }
        self.max_items = limit;
    }

    /// Enables hiding the bar while the page scrolls down.
    pub fn set_hide_on_scroll(&mut self, hide: bool) {
        if self.hide_on_scroll == hide {
            return;
        }
        self.hide_on_scroll = hide;
        self.base.request_redraw();
    }

    /// Reports a scroll in wheel notches: downward hides, upward shows.
    pub fn report_scroll(&mut self, delta_y: i32) {
        if !self.hide_on_scroll || delta_y == 0 {
            return;
        }
        self.visibility.set_target(if delta_y > 0 { 0.0 } else { 1.0 });
        self.base.request_redraw();
    }

    /// How much of the bar is currently shown, `0.0` to `1.0`.
    pub fn visibility_progress(&self) -> f32 {
        self.visibility.value()
    }

    /// Assigns a typed icon source to an existing item index.
    ///
    /// The index is stable for the item's lifetime: this widget only appends items and clears
    /// the entire collection, so an invalid index is reported rather than silently stored.
    pub fn set_item_icon_source(
        &mut self,
        index: usize,
        source: Option<IconSource>,
    ) -> Result<(), IconItemError> {
        let len = self.items.len();
        let Some(slot) = self.item_icon_sources.get_mut(index) else {
            return Err(IconItemError::IndexOutOfBounds { index, len });
        };
        *slot = source;
        self.base.request_redraw();
        Ok(())
    }

    /// Sets the currently selected tab index.
    ///
    /// If the new index is different from the current selection and valid,
    /// the `current_changed` signal is emitted and the widget redraws.
    ///
    /// Out-of-range indices are clamped to the valid range.
    pub fn set_selected_index(&mut self, index: usize) {
        let clamped = if self.items.is_empty() { 0 } else { index.min(self.items.len() - 1) };
        if self.selected_index != clamped {
            // The slide resumes from the pill's **fractional** position, not from the tab it left
            // rounded to: a user who clicks tab 3 while the pill is 40% of the way to tab 1 would
            // otherwise see it snap back to tab 0 first, which is the one thing an indicator of
            // "where you are" must not do.
            let from = self.indicator_position();
            self.slide_from = from.floor().max(0.0) as usize;
            let span = (clamped as f32 - self.slide_from as f32).abs();
            let fraction = if span < f32::EPSILON {
                1.0
            } else {
                ((from - self.slide_from as f32) / (clamped as f32 - self.slide_from as f32))
                    .clamp(0.0, 1.0)
            };
            self.selected_index = clamped;
            self.slide.jump_to(fraction);
            self.slide.set_target(1.0);
            self.current_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// The tab index the pill is currently drawn at, as a **fractional** value.
    ///
    /// This is the value an animation test samples: strictly between two whole indices while the
    /// pill is travelling, which is exactly what "the pill slid rather than jumped" means.
    pub fn indicator_position(&self) -> f32 {
        let from = self.slide_from as f32;
        let to = self.selected_index as f32;
        from + (to - from) * self.slide.value()
    }

    /// Returns the currently selected tab index.
    pub fn selected_index(&self) -> Option<usize> {
        self.items.get(self.selected_index).map(|_| self.selected_index)
    }

    /// Removes all navigation items from the bar.
    ///
    /// The selected index is reset to 0.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.item_icon_sources.clear();
        self.badges.clear();
        self.item_enabled.clear();
        self.selected_index = 0;
        self.base.request_redraw();
    }

    /// Returns the number of navigation items currently in the bar.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

impl Widget for BottomNavigationBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 48)
    }

    /// Advances the selection pill and the hide-on-scroll fade; `true` while either moves.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let sliding = self.slide.tick(delta_ms);
        let fading = self.visibility.tick(delta_ms);
        sliding || fading
    }

    fn is_animating(&self) -> bool {
        self.slide.is_moving() || self.visibility.is_moving()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `BottomNavigationBar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `item_count` is derived from
/// the item list, so it is readable but read-only.
impl WidgetProperties for BottomNavigationBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "selected_index" => {
                Ok(CapabilityValue::UInt(self.selected_index().unwrap_or(0) as u64))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_selected_index(expect_usize(value)?);
                Ok(())
            }
            // The item total is implied by how many items were added, so there is
            // nothing sensible to assign to it.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["item_count", "selected_index", BASE_PROPERTY_NAMES]
    }
}

impl Draw for BottomNavigationBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The hide-on-scroll transition moves the whole bar out of its own band. At rest the
        // fraction is exactly one, so the shift is zero and the picture is unchanged.
        let visibility = self.visibility.value().clamp(0.0, 1.0);
        if visibility <= 0.001 {
            return;
        }
        let hide_shift = ((1.0 - visibility) * rect.height as f32) as i32;
        let rect = Rect::new(rect.x, rect.y + hide_shift, rect.width, rect.height);

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the bar fill and both tab colours used to be hardcoded literals, so
        // light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("bottom_navigation_bar");
        // `bottom_navigation_bar` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A bar painted in that colour would be byte-identical to the frame behind
        // it, so a resolved surface equal to the window fill is re-derived a visible step
        // away from it, the same distinction `Colors::input_background` draws for a field.
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The selected tab and its indicator are the accent: the hue a theme is expected to
        // vary most, so selection follows the appearance rather than a literal blue. One
        // palette read serves both this and the window fill above (G-A01).
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let bar_color = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| bar_color.blend(&ink, 0.25));

        let is_enabled = self.base.is_enabled();
        let bar_height = rect.height;

        // Draw background bar. This is painted even with no items: the bar is a persistent
        // chrome surface whose extent the layout reserved, and returning early left a bar
        // with an empty item list completely invisible.
        context.face_with_gradient_corners(
            rect,
            bar_color,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // Draw top border line
        context.draw_line(
            Point::new(rect.x, rect.y),
            Point::new(rect.x + rect.width as i32, rect.y),
            border_color,
        );

        let item_count = self.items.len();
        if item_count == 0 {
            return;
        }

        let tab_width = rect.width / item_count as u32;

        // Determine font sizes proportional to bar height
        let icon_font_size = (bar_height as f32 * 0.32).clamp(14.0, 28.0);
        let label_font_size = (bar_height as f32 * 0.18).clamp(9.0, 14.0);

        let icon_font = role_font(style, icon_font_size);
        let label_font = role_font(style, label_font_size);

        for (i, item) in self.items.iter().enumerate() {
            let tab_x = rect.x + (i as u32 * tab_width) as i32;
            let tab_rect = Rect::new(tab_x, rect.y, tab_width, bar_height);

            let is_selected = i == self.selected_index;
            // A per-item disabled destination is muted and not selectable, while the whole bar's
            // enabled flag still disables everything (N-04).
            let item_on = is_enabled && self.is_item_enabled(i);

            // Determine colors based on selection and enabled state
            let (icon_color, label_color) = if !item_on {
                (ink.blend(&bar_color, 0.6), ink.blend(&bar_color, 0.6))
            } else if is_selected {
                (accent, accent)
            } else {
                (ink.blend(&bar_color, 0.4), ink.blend(&bar_color, 0.4))
            };

            // Measure icon and label to center them in the tab. The shared measure cache keeps a
            // fixed caption from being re-measured on every repaint (G-A02).
            let typed_source = self.item_icon_sources.get(i).copied().flatten();
            let mut icon_metrics = crate::widget::base_widgets::measure_cache::measure_text(
                context,
                if typed_source.is_some() { "M" } else { &item.icon },
                &icon_font,
            );
            if typed_source.is_some() {
                icon_metrics.width = icon_metrics.height.max(1);
            }
            let show_label = match self.label_behavior {
                NavLabelBehavior::Always => true,
                NavLabelBehavior::SelectedOnly => is_selected,
                NavLabelBehavior::Never => false,
            };
            // A hidden label is not measured at all: the two-state form measured it anyway and
            // then reserved no room for it.
            let label_metrics = if show_label {
                crate::widget::base_widgets::measure_cache::measure_text(
                    context,
                    &item.label,
                    &label_font,
                )
            } else {
                crate::render::TextMetrics::default()
            };

            // Vertical layout: icon centered vertically with label below. A hidden label does
            // not reserve room, so the icon centres on the bar's middle line.
            let total_content_height =
                icon_metrics.height + if show_label { label_metrics.height + 4 } else { 0 };
            let content_y = tab_rect.y + (tab_rect.height as i32 - total_content_height as i32) / 2;

            // The selected tab's indicator is Material M3's **pill**: a 64×32 rounded capsule
            // behind the icon, in the accent, rather than the 3 px underline this used to draw.
            //
            // The underline was the M2 shape and it also read ambiguously: pinned to the bar's
            // top edge, it looked like the bar's own border in the one place a reader expects to
            // find "which tab am I on". The pill states it where the eye already is — on the
            // icon — and it is drawn *before* the icon so the glyph sits on it. Its size follows
            // the icon's own box, so a bar with a larger font gets a larger pill instead of a
            // fixed capsule the icon outgrows.
            if is_selected && item_on {
                const PILL_WIDTH: u32 = 64;
                const PILL_HEIGHT: u32 = 32;
                let pill_width = PILL_WIDTH.min(tab_rect.width);
                let pill_height = PILL_HEIGHT.min(tab_rect.height);
                // # Why the pill is interpolated between tab *centres*
                //
                // The pill used to be centred on whichever tab was selected, so a change of tab
                // moved it by teleport. It is now placed at `indicator_position()` — the tab index
                // the slide has reached, as a fraction — applied to the **centre-to-centre**
                // distance. Interpolating the centre rather than the left edge is what keeps the
                // capsule the same width throughout: interpolating an edge would need a width that
                // changed if the tabs ever did.
                let centre_of =
                    |index: f32| -> f32 { (rect.x as f32) + (index + 0.5) * tab_width as f32 };
                let centre = centre_of(self.indicator_position()) as i32;
                let pill_x = centre - pill_width as i32 / 2;
                // Centred on the icon rather than on the tab, so the glyph is optically inside
                // it: the label below must not make the pill drift downward.
                let pill_y = content_y + (icon_metrics.height as i32 - pill_height as i32) / 2;
                let pill = Rect::new(pill_x, pill_y, pill_width, pill_height);
                // The capsule is a *tint* of the accent, not the accent at full strength: the
                // icon on top of it is drawn in the accent, and an accent on an accent is
                // unreadable. Blending it into the bar is the same treatment `chip`'s selected
                // state uses.
                context.fill_rounded_rect(pill, pill_height / 2, accent.blend(&bar_color, 0.24));
            }

            // Draw icon. The string is resolved as an **icon** first (`draw_icon_named`), and only
            // falls back to text when it names no icon at all. This is what lets a host pass
            // `"star"` and get the real outline; a host still passing the old `"★"` symbol keeps
            // the text path it always had, so the migration is opt-in rather than breaking.
            //
            // `content_y` is the top of the icon+label stack and the glyph origin is
            // the box's top edge, so no ascent term belongs here — the one that used to be
            // added pushed the icon a full line down and the label with it.
            let icon_box = Rect::new(
                tab_rect.x + (tab_rect.width as i32 - icon_metrics.width as i32) / 2,
                content_y,
                icon_metrics.width.max(1),
                icon_metrics.height.max(1),
            );
            if let Some(source) = typed_source {
                if let Err(error) =
                    crate::widget::paint_icon(context, source.into(), icon_box, icon_color)
                {
                    log::warn!("bottom navigation item {i} icon could not be painted: {error:?}");
                }
            } else if !crate::widget::draw_icon_named(context, icon_box, icon_color, &item.icon) {
                context.draw_text(
                    Point::new(icon_box.x, icon_box.y),
                    &item.icon,
                    &icon_font,
                    icon_color,
                    HorizontalAlignment::Left,
                );
            }

            // Badge: a small capsule at the icon's trailing-top corner. It is data, not chrome,
            // so it reads the theme's error colour (the semantic "attention" tone) with a
            // legible ink chosen against the fill it actually paints.
            if let Some(badge) = self.badges.get(i).and_then(|badge| badge.as_deref()) {
                if !badge.is_empty() {
                    let metrics = crate::widget::base_widgets::measure_cache::measure_text(
                        context,
                        badge,
                        &label_font,
                    );
                    let badge_height = label_font.size().max(10.0) as u32 + 4;
                    let badge_width = (metrics.width + 8).max(badge_height);
                    let badge_rect = Rect::new(
                        icon_box.x + icon_box.width as i32 - badge_width as i32 / 2,
                        icon_box.y - 2,
                        badge_width,
                        badge_height,
                    );
                    let badge_fill =
                        crate::style::semantic_color(crate::style::SemanticColor::Error)
                            .unwrap_or(accent);
                    context.fill_rounded_rect(badge_rect, badge_height / 2, badge_fill);
                    context.draw_text(
                        Point::new(badge_rect.x + 4, context.text_line(badge_rect, &label_font).y),
                        badge,
                        &label_font,
                        badge_fill.contrast_color(),
                        HorizontalAlignment::Left,
                    );
                }
            }

            // Draw label
            if show_label {
                let label_x = tab_rect.x + (tab_rect.width as i32 - label_metrics.width as i32) / 2;
                let label_y = content_y + icon_metrics.height as i32 + 4;
                context.draw_text(
                    Point::new(label_x, label_y),
                    &item.label,
                    &label_font,
                    label_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for BottomNavigationBar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                let item_count = self.items.len();
                if item_count == 0 {
                    return;
                }
                let rect = self.geometry();
                if !rect.contains_point(*pos) {
                    return;
                }

                // Determine which tab was clicked based on x coordinate
                let tab_width = rect.width / item_count as u32;
                let relative_x = pos.x - rect.x;
                if relative_x < 0 {
                    return;
                }
                let index = (relative_x as u32 / tab_width) as usize;
                // A disabled destination ignores the press entirely (N-04).
                if index < item_count && self.is_item_enabled(index) {
                    self.set_selected_index(index);
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn make_bar() -> BottomNavigationBar {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        bar.add_item("★", "Favorites");
        bar.add_item("✉", "Messages");
        bar.add_item("⌂", "Home");
        bar.add_item("⚙", "Settings");
        bar
    }

    #[test]
    fn bottom_nav_bar_default_creation() {
        let bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        assert_eq!(bar.kind(), WidgetKind::BottomNavigationBar);
        assert_eq!(bar.geometry(), Rect::new(0, 0, 375, 56));
        assert_eq!(bar.item_count(), 0);
        assert_eq!(bar.selected_index().unwrap_or(0), 0);
        assert!(bar.is_visible());
        assert!(bar.is_enabled());
    }

    #[test]
    fn bottom_nav_bar_add_items() {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        assert_eq!(bar.item_count(), 0);

        bar.add_item("★", "Favorites");
        assert_eq!(bar.item_count(), 1);
        assert_eq!(bar.items[0].icon, "★");
        assert_eq!(bar.items[0].label, "Favorites");

        bar.add_item("✉", "Messages");
        assert_eq!(bar.item_count(), 2);
        assert_eq!(bar.items[1].icon, "✉");
        assert_eq!(bar.items[1].label, "Messages");
    }

    #[test]
    fn bottom_nav_item_typed_source_is_index_checked_and_rendered() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "nav-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut bar = make_bar();
        assert_eq!(
            bar.set_item_icon_source(9, Some(IconSource::static_geometry(&RING))),
            Err(IconItemError::IndexOutOfBounds { index: 9, len: 4 })
        );
        bar.set_item_icon_source(1, Some(IconSource::static_geometry(&RING)))
            .expect("existing item accepts a typed source");
        let svg = render_to_svg(&mut bar);
        assert!(svg.contains("fill-rule=\"nonzero\""), "typed geometry must reach SVG");

        bar.clear_items();
        assert!(bar.item_icon_sources.is_empty(), "clearing items clears their sources");
    }

    #[test]
    fn bottom_nav_bar_clear_items() {
        let mut bar = make_bar();
        assert_eq!(bar.item_count(), 4);
        assert_eq!(bar.selected_index().unwrap_or(0), 0);

        bar.set_selected_index(2);
        assert_eq!(bar.selected_index().unwrap_or(0), 2);

        bar.clear_items();
        assert_eq!(bar.item_count(), 0);
        assert_eq!(bar.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn bottom_nav_bar_set_selected_index() {
        let mut bar = make_bar();
        assert_eq!(bar.selected_index().unwrap_or(0), 0);

        bar.set_selected_index(2);
        assert_eq!(bar.selected_index().unwrap_or(0), 2);

        bar.set_selected_index(0);
        assert_eq!(bar.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn bottom_nav_bar_set_selected_index_clamps_out_of_range() {
        let mut bar = make_bar();
        assert_eq!(bar.item_count(), 4);

        // Out-of-range high should clamp to last valid index
        bar.set_selected_index(100);
        assert_eq!(bar.selected_index().unwrap_or(0), 3);

        // Back to valid range
        bar.set_selected_index(1);
        assert_eq!(bar.selected_index().unwrap_or(0), 1);
    }

    #[test]
    fn bottom_nav_bar_current_changed_signal_emits() {
        let mut bar = make_bar();
        let captured = Arc::new(AtomicUsize::new(usize::MAX));
        let c = captured.clone();
        bar.current_changed.connect(move |val: Arc<usize>| {
            c.store(*val, Ordering::SeqCst);
        });

        bar.set_selected_index(2);
        assert_eq!(captured.load(Ordering::SeqCst), 2);

        bar.set_selected_index(0);
        assert_eq!(captured.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn bottom_nav_bar_current_changed_not_emitted_for_same_index() {
        let mut bar = make_bar();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bar.current_changed.connect(move |_: Arc<usize>| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        bar.set_selected_index(0); // same as default, should not emit
        assert_eq!(count.load(Ordering::SeqCst), 0);

        bar.set_selected_index(1); // different, should emit
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn bottom_nav_bar_mouse_press_selects_tab() {
        let mut bar = make_bar();
        // Tab 0: x in [0..93], Tab 1: x in [94..187], Tab 2: x in [188..281], Tab 3: x in [282..374]
        // Click on tab at index 2
        bar.handle_event(&Event::MousePress { pos: Point::new(200, 28), button: 1, modifiers: 0 });
        assert_eq!(bar.selected_index().unwrap_or(0), 2);

        // Click on tab at index 0
        bar.handle_event(&Event::MousePress { pos: Point::new(30, 28), button: 1, modifiers: 0 });
        assert_eq!(bar.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn bottom_nav_bar_mouse_release_selects_tab() {
        let mut bar = make_bar();
        bar.handle_event(&Event::MouseRelease { pos: Point::new(300, 28), button: 1 });
        assert_eq!(bar.selected_index().unwrap_or(0), 3);
    }

    #[test]
    fn bottom_nav_bar_mouse_press_other_button_noop() {
        let mut bar = make_bar();
        bar.set_selected_index(1);
        bar.handle_event(&Event::MousePress { pos: Point::new(30, 28), button: 2, modifiers: 0 });
        assert_eq!(bar.selected_index().unwrap_or(0), 1); // should remain unchanged
    }

    #[test]
    fn bottom_nav_bar_mouse_click_outside_does_nothing() {
        let mut bar = make_bar();
        bar.handle_event(&Event::MousePress { pos: Point::new(500, 100), button: 1, modifiers: 0 });
        assert_eq!(bar.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn bottom_nav_bar_disabled_blocks_events() {
        let mut bar = make_bar();
        bar.set_enabled(false);

        bar.handle_event(&Event::MousePress { pos: Point::new(200, 28), button: 1, modifiers: 0 });
        assert_eq!(bar.selected_index().unwrap_or(0), 0); // should not change
    }

    #[test]
    fn bottom_nav_bar_empty_no_items_does_not_panic() {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        // These should not panic with no items
        bar.handle_event(&Event::MousePress { pos: Point::new(30, 28), button: 1, modifiers: 0 });
        bar.set_selected_index(5);
        assert_eq!(bar.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn bottom_nav_bar_selected_index_out_of_range_clamps() {
        let mut bar = make_bar();
        bar.set_selected_index(10);
        assert_eq!(bar.selected_index().unwrap_or(0), 3);

        bar.clear_items();
        bar.set_selected_index(0);
        assert_eq!(bar.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn bottom_nav_bar_nav_item_debug_and_clone() {
        let item = NavItem { icon: "★".to_string(), label: "Test".to_string() };
        let cloned = item.clone();
        assert_eq!(cloned.icon, "★");
        assert_eq!(cloned.label, "Test");
        let debug = format!("{:?}", item);
        assert!(debug.contains("★"));
        assert!(debug.contains("Test"));
    }

    #[test]
    fn bottom_nav_bar_svg_output() {
        let mut bar = make_bar();
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"375\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn a_token_icon_name_draws_an_outline_and_a_legacy_symbol_still_draws_text() {
        // The item `icon` is a host string, so this is the compatibility contract of
        // `draw_icon_named`: a name that resolves to an icon draws real geometry (and carries no
        // `data-text` tag), while a name that resolves to nothing keeps the text path so a host
        // still passing the old symbol does not lose its icon.
        let _theme_guard = crate::style::theme_test_guard();

        let mut token_bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        token_bar.add_item("star", "Favorites");
        let token_svg = render_to_svg(&mut token_bar);
        // The icon is a filled outline (`stroke="none"`, no `data-text` tag), which the text
        // path never emits. The *label* below it is still text, so the assertion is about the icon
        // shape rather than about the whole document having no text run.
        assert!(
            token_svg.contains("stroke=\"none\""),
            "a token icon name must draw outline geometry: {token_svg}"
        );
        let outline_paths = token_svg.matches("stroke=\"none\"").count();
        assert_eq!(
            outline_paths, 1,
            "the star's two contours must be submitted as one compound path"
        );
        assert!(token_svg.contains("fill-rule=\"nonzero\""));

        // The legacy symbol resolves to no icon, so the text path still runs — the behaviour a
        // pre-existing host depends on.
        let mut symbol_bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        symbol_bar.add_item("★", "Favorites");
        let symbol_svg = render_to_svg(&mut symbol_bar);
        assert!(
            !symbol_svg.contains("stroke-width=\"0\""),
            "a symbol string must keep the text path rather than gaining an icon: {symbol_svg}"
        );
        assert!(
            symbol_svg.contains("data-text"),
            "the symbol is still drawn as text, so the host does not lose its icon: {symbol_svg}"
        );
    }

    #[test]
    fn bottom_nav_bar_svg_output_empty() {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 200, 48));
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    /// The selected tab is marked by an M3 pill, not by the 3 px underline it used to draw.
    ///
    /// Regression (BLUE21 AR2): the indicator was a 3 px accent line pinned to the bar's top
    /// edge — the M2 shape, and in the one place a reader is most likely to read it as the bar's
    /// own border rather than as "which tab am I on". The pill is a 64×32 capsule behind the
    /// icon, so the assertion is on that geometry: exactly one fully-rounded 64×32 rect, and no
    /// 3 px full-height line at the bar's top edge.
    #[test]
    fn the_selected_tab_is_marked_by_a_pill_not_an_underline() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut bar = make_bar();
        bar.set_selected_index(1);
        let svg = render_to_svg(&mut bar);

        let pill = svg
            .split("<rect")
            .skip(1)
            .find(|rect| rect.contains("width=\"64\"") && rect.contains("height=\"32\""))
            .unwrap_or_else(|| panic!("the pill capsule must be painted: {svg}"));
        assert!(pill.contains("rx=\"16\""), "a capsule is half its height in radius: {pill}");

        // Only the selected tab carries a pill, so there is exactly one.
        assert_eq!(svg.matches("width=\"64\"").count(), 1, "one selected tab, one pill: {svg}");
    }

    #[test]
    fn bottom_nav_bar_request_redraw_on_item_change() {
        let mut bar = make_bar();
        let redrawn = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = redrawn.clone();
        bar.redraw_requested_signal().connect(move || {
            r.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        bar.add_item("♥", "Likes");
        assert!(redrawn.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn bottom_nav_bar_redraw_on_selection_change() {
        let mut bar = make_bar();
        let redraw_count = Arc::new(AtomicUsize::new(0));
        let rc = redraw_count.clone();
        bar.redraw_requested_signal().connect(move || {
            rc.fetch_add(1, Ordering::SeqCst);
        });

        bar.set_selected_index(1);
        assert!(redraw_count.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn a_badge_belongs_to_one_item_and_survives_clearing() {
        let mut bar = make_bar();
        assert!(bar.set_badge(1, Some("3")));
        assert_eq!(bar.badge(1), Some("3"));
        assert!(!bar.set_badge(9, Some("3")));
        bar.clear_items();
        assert_eq!(bar.badge(1), None);
    }

    #[test]
    fn unselected_labels_can_be_hidden() {
        let mut bar = make_bar();
        assert!(bar.show_unselected_labels());
        bar.set_show_unselected_labels(false);
        assert!(!bar.show_unselected_labels());
        let svg = crate::widget::svg::render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn the_destination_limit_is_opt_in() {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 320, 48));
        bar.add_item("a", "A");
        bar.add_item("b", "B");
        bar.set_max_items(Some(2));
        assert!(!bar.try_add_item("c", "C"));
        assert_eq!(bar.item_count(), 2);
        bar.set_max_items(None);
        assert!(bar.try_add_item("c", "C"));
    }

    #[test]
    fn scrolling_hides_the_bar_only_when_asked() {
        let mut bar = make_bar();
        bar.report_scroll(2);
        assert_eq!(bar.visibility_progress(), 1.0);
        bar.set_hide_on_scroll(true);
        bar.report_scroll(2);
        while bar.tick(16) {}
        assert_eq!(bar.visibility_progress(), 0.0);
        bar.report_scroll(-2);
        while bar.tick(16) {}
        assert_eq!(bar.visibility_progress(), 1.0);
    }

    #[test]
    fn label_visibility_has_three_states() {
        let mut bar = make_bar();
        assert_eq!(bar.label_behavior(), NavLabelBehavior::Always);
        bar.set_label_behavior(NavLabelBehavior::Never);
        assert_eq!(bar.label_behavior(), NavLabelBehavior::Never);
        assert!(!bar.show_unselected_labels(), "the icon-only bar names nothing");
        bar.set_show_unselected_labels(true);
        assert_eq!(bar.label_behavior(), NavLabelBehavior::Always);
        bar.set_show_unselected_labels(false);
        assert_eq!(bar.label_behavior(), NavLabelBehavior::SelectedOnly);
    }

    #[test]
    fn a_disabled_destination_ignores_a_press() {
        let mut bar = make_bar();
        assert!(bar.set_item_enabled(1, false));
        assert!(!bar.is_item_enabled(1));
        // The second tab occupies x = 93..186 of a 375 px bar with four items.
        bar.handle_event(&Event::MousePress { pos: Point::new(140, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(140, 28), button: 1 });
        assert_eq!(bar.selected_index(), Some(0), "a disabled destination must not be chosen");
        assert!(bar.set_item_enabled(1, true));
        bar.handle_event(&Event::MousePress { pos: Point::new(140, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(140, 28), button: 1 });
        assert_eq!(bar.selected_index(), Some(1));
    }
}
