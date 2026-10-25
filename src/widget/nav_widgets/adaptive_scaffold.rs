// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AdaptiveScaffold — a cross-platform adaptive scaffold widget.
//!
//! The AdaptiveScaffold combines an AppBar (top bar), a main content area,
//! and an optional BottomNavigationBar into a single cohesive layout.
//! It is designed to work on both iOS (Cupertino) and Android (Material)
//! platforms by using the same underlying AppBar and BottomNavigationBar
//! primitives with platform-appropriate defaults.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_f32, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::PopupReveal;
use crate::widget::metrics::role_font;
use crate::widget::nav_widgets::app_bar::AppBar;
use crate::widget::nav_widgets::bottom_navigation_bar::{BottomNavigationBar, NavItem};
use crate::widget::{BaseWidget, Draw, IconItemError, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Height of the scaffold's app-bar band, in logical pixels.
///
/// The Material app bar's own height, named once because the layout arithmetic, the
/// hit test and the child slots all key off the same band.
const APP_BAR_HEIGHT: u32 = 56;

/// Height of the bottom navigation band, in logical pixels.
const BOTTOM_NAV_HEIGHT: u32 = 56;

/// Edge length of the floating action button slot, in logical pixels.
const FAB_SLOT_SIZE: u32 = 56;

/// Margin between the floating action button and the scaffold's edges.
const FAB_SLOT_MARGIN: i32 = 16;

/// Width of a navigation drawer panel, in logical pixels.
const DRAWER_WIDTH: u32 = 280;

/// Height of the snackbar slot, in logical pixels.
const SNACKBAR_HEIGHT: u32 = 48;

/// Margin between the snackbar and the scaffold's edges.
const SNACKBAR_MARGIN: i32 = 8;

/// Which implementation paints the scaffold's bottom navigation bar.
///
/// [`Self::Classic`] is the scaffold's own compact M2-style strip and is the default,
/// so a scaffold that never asks for anything else paints exactly the bytes it always
/// did. [`Self::Material3`] hands the band to a real
/// [`BottomNavigationBar`](crate::widget::nav_widgets::bottom_navigation_bar::BottomNavigationBar),
/// which paints the sliding M3 pill — one implementation instead of two that could
/// disagree about where the selection indicator is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaffoldBottomNavStyle {
    /// The scaffold's own strip: a flat indicator line under the selected item.
    #[default]
    Classic,
    /// A real [`BottomNavigationBar`] with its sliding M3 pill.
    Material3,
}

/// How a navigation item's badge is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaffoldBadgeStyle {
    /// A capsule carrying the badge text (a count, usually).
    #[default]
    Text,
    /// A small filled dot, for "there is something" without a number.
    Dot,
}

impl ScaffoldBadgeStyle {
    /// The token this style is spelled as in a property write.
    pub fn as_str(self) -> &'static str {
        match self {
            ScaffoldBadgeStyle::Text => "text",
            ScaffoldBadgeStyle::Dot => "dot",
        }
    }

    /// Parses the token back, rejecting anything else.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "text" => ScaffoldBadgeStyle::Text,
            "dot" => ScaffoldBadgeStyle::Dot,
            _ => return None,
        })
    }
}

/// AdaptiveScaffold — a cross-platform adaptive scaffold combining AppBar +
/// content area + optional BottomNavigationBar.
///
/// The scaffold manages layout internally: the AppBar occupies the top region,
/// the content area fills the space between the AppBar and the bottom navigation
/// bar, and the BottomNavigationBar (when enabled) sits at the bottom.
///
/// # Example
///
/// ```text
/// let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
/// scaffold.add_nav_item("★", "Favorites");
/// scaffold.add_nav_item("✉", "Messages");
/// scaffold.set_show_bottom_nav(true);
/// ```
pub struct AdaptiveScaffold {
    base: BaseWidget,
    /// The title text displayed in the AppBar.
    title: String,
    /// Internal AppBar widget.
    app_bar: AppBar,
    /// Optional body widget to render in the content area.
    body_widget: Option<Box<dyn Widget>>,
    /// Whether to show the bottom navigation bar.
    show_bottom_nav: bool,
    /// Navigation items for the bottom bar.
    nav_items: Vec<NavItem>,
    nav_item_icon_sources: Vec<Option<IconSource>>,
    /// Currently selected bottom nav index.
    selected_nav_index: usize,
    /// Which bottom-bar implementation paints the band.
    bottom_nav_style: ScaffoldBottomNavStyle,
    /// The delegate that paints [`ScaffoldBottomNavStyle::Material3`].
    ///
    /// Materialised when the style is selected (and rebuilt whenever the item list or the
    /// geometry changes) so the sliding pill is a real `BottomNavigationBar`, not a copy of
    /// its arithmetic.
    m3_bar: Option<BottomNavigationBar>,
    /// One body widget per bottom-navigation destination, kept mounted so each tab keeps
    /// its own state (an IndexedStack rather than a rebuilt page).
    nav_bodies: Vec<Option<Box<dyn Widget>>>,
    /// Optional floating action button slot.
    fab: Option<Box<dyn Widget>>,
    /// Optional leading drawer slot.
    drawer: Option<Box<dyn Widget>>,
    /// Optional trailing (end) drawer slot.
    end_drawer: Option<Box<dyn Widget>>,
    /// Optional bottom-sheet slot.
    bottom_sheet: Option<Box<dyn Widget>>,
    /// Optional snackbar slot.
    snackbar: Option<Box<dyn Widget>>,
    /// Whether the leading drawer is open.
    drawer_open: bool,
    /// Whether the trailing drawer is open.
    end_drawer_open: bool,
    /// Per-item badges, parallel to `nav_items`.
    nav_item_badges: Vec<Option<String>>,
    /// How a badge is painted.
    badge_style: ScaffoldBadgeStyle,
    /// Whether the drawers slide in and out.
    drawer_animated: bool,
    /// How opaque the modal drawer scrim becomes at full slide, `0.0..=1.0`.
    scrim_opacity: f32,
    /// Whether a destination change animates its page.
    page_transition: bool,
    /// The leading drawer's slide, `0.0` closed and `1.0` fully in.
    drawer_reveal: PopupReveal,
    /// The trailing drawer's slide.
    end_drawer_reveal: PopupReveal,
    /// The destination page's arrival, `0.0` just switched and `1.0` settled.
    page_reveal: PopupReveal,
    /// Emitted when the bottom navigation selection changes.
    pub nav_selected: Signal1<usize>,
    /// Emitted when the AppBar's back affordance is pressed.
    ///
    /// The AppBar itself stays private, so this mirrors its signal outward: an embedder
    /// observes navigation on the scaffold rather than reaching into a child it does not own.
    pub back_pressed: GenericSignal,
    /// Emitted when the AppBar's action affordance is pressed.
    pub action_pressed: GenericSignal,
}

impl AdaptiveScaffold {
    /// Creates a new AdaptiveScaffold with the given title and geometry.
    ///
    /// The scaffold is created with the AppBar visible and bottom navigation
    /// hidden by default. Use [`set_show_bottom_nav`](Self::set_show_bottom_nav)
    /// to enable the bottom bar and [`add_nav_item`](Self::add_nav_item) to
    /// populate it.
    pub fn new(title: &str, geometry: Rect) -> Self {
        let app_bar_height: u32 = 56;
        let app_bar =
            AppBar::new(title, Rect::new(geometry.x, geometry.y, geometry.width, app_bar_height));

        let scaffold = Self {
            base: BaseWidget::new(WidgetKind::AdaptiveScaffold, geometry, "AdaptiveScaffold"),
            title: title.to_string(),
            app_bar,
            body_widget: None,
            show_bottom_nav: false,
            nav_items: Vec::new(),
            nav_item_icon_sources: Vec::new(),
            selected_nav_index: 0,
            bottom_nav_style: ScaffoldBottomNavStyle::Classic,
            m3_bar: None,
            nav_bodies: Vec::new(),
            fab: None,
            drawer: None,
            end_drawer: None,
            bottom_sheet: None,
            snackbar: None,
            drawer_open: false,
            end_drawer_open: false,
            nav_item_badges: Vec::new(),
            badge_style: ScaffoldBadgeStyle::default(),
            drawer_animated: true,
            scrim_opacity: 0.32,
            page_transition: true,
            drawer_reveal: PopupReveal::new(false),
            end_drawer_reveal: PopupReveal::new(false),
            page_reveal: PopupReveal::new(true),
            nav_selected: Signal1::new(),
            back_pressed: GenericSignal::new(),
            action_pressed: GenericSignal::new(),
        };

        // Mirror the AppBar's two signals onto the scaffold's own. The handles are
        // intentionally dropped: a `ConnectionHandle` only lets a caller disconnect, and
        // the slot lives for as long as the signal it was registered on does.
        let back_out = scaffold.back_pressed.clone();
        let _ = scaffold.app_bar.back_pressed.connect(move || back_out.emit());
        let action_out = scaffold.action_pressed.clone();
        let _ = scaffold.app_bar.action_pressed.connect(move || action_out.emit());
        scaffold
    }

    /// Sets the title displayed in the AppBar.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.app_bar.set_title(title);
    }

    /// Returns the current title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets whether the AppBar back button is shown.
    pub fn set_show_back(&mut self, show_back: bool) {
        self.app_bar.set_show_back(show_back);
    }

    /// Returns whether the AppBar back button is shown.
    pub fn show_back(&self) -> bool {
        self.app_bar.show_back()
    }

    /// Sets the action text displayed on the right side of the AppBar.
    pub fn set_action_text(&mut self, text: &str) {
        self.app_bar.set_action_text(text);
    }

    /// Returns the current AppBar action text.
    pub fn action_text(&self) -> &str {
        self.app_bar.action_text()
    }

    /// Sets the optional body widget displayed in the content area.
    ///
    /// This is the fallback body used when the selected bottom-navigation destination has no
    /// page of its own; see [`Self::set_nav_body`] for per-destination pages.
    pub fn set_body_widget(&mut self, widget: Option<Box<dyn Widget>>) {
        self.body_widget = widget;
        self.base.request_redraw();
    }

    /// Assigns the body page shown while `index` is the selected destination.
    ///
    /// Every page stays mounted, which is what gives each destination its own state: switching
    /// away and back returns to the same widget rather than rebuilding it. The index is clamped
    /// into the current item range, and the vector is grown to that range so a page added before
    /// its item still lines up with it.
    pub fn set_nav_body(&mut self, index: usize, widget: Option<Box<dyn Widget>>) {
        let upper = index.min(self.nav_items.len().saturating_sub(1));
        if self.nav_bodies.len() <= upper {
            self.nav_bodies.resize_with(upper + 1, || None);
        }
        self.nav_bodies[upper] = widget;
        self.base.request_redraw();
    }

    /// Returns the page assigned to `index`, when there is one.
    pub fn nav_body(&self, index: usize) -> Option<&dyn Widget> {
        self.nav_bodies.get(index).and_then(|body| body.as_deref())
    }

    /// Returns the body the scaffold currently paints: the selected destination's page when it
    /// has one, otherwise the shared fallback body.
    pub fn active_body(&self) -> Option<&dyn Widget> {
        self.nav_bodies
            .get(self.selected_nav_index)
            .and_then(|body| body.as_deref())
            .or(self.body_widget.as_deref())
    }

    /// Sets the floating action button slot.
    pub fn set_fab(&mut self, widget: Option<Box<dyn Widget>>) {
        self.fab = widget;
        self.base.request_redraw();
    }

    /// Returns the floating action button slot, if one is set.
    pub fn fab(&self) -> Option<&dyn Widget> {
        self.fab.as_deref()
    }

    /// Sets the leading drawer slot. The drawer is closed until [`Self::open_drawer`] is called.
    pub fn set_drawer(&mut self, widget: Option<Box<dyn Widget>>) {
        self.drawer = widget;
        if self.drawer.is_none() {
            self.drawer_open = false;
        }
        self.base.request_redraw();
    }

    /// Returns the leading drawer slot, if one is set.
    pub fn drawer(&self) -> Option<&dyn Widget> {
        self.drawer.as_deref()
    }

    /// Sets the trailing (end) drawer slot.
    pub fn set_end_drawer(&mut self, widget: Option<Box<dyn Widget>>) {
        self.end_drawer = widget;
        if self.end_drawer.is_none() {
            self.end_drawer_open = false;
        }
        self.base.request_redraw();
    }

    /// Returns the trailing drawer slot, if one is set.
    pub fn end_drawer(&self) -> Option<&dyn Widget> {
        self.end_drawer.as_deref()
    }

    /// Sets the bottom-sheet slot.
    pub fn set_bottom_sheet(&mut self, widget: Option<Box<dyn Widget>>) {
        self.bottom_sheet = widget;
        self.base.request_redraw();
    }

    /// Returns the bottom-sheet slot, if one is set.
    pub fn bottom_sheet(&self) -> Option<&dyn Widget> {
        self.bottom_sheet.as_deref()
    }

    /// Sets the snackbar slot.
    pub fn set_snackbar(&mut self, widget: Option<Box<dyn Widget>>) {
        self.snackbar = widget;
        self.base.request_redraw();
    }

    /// Returns the snackbar slot, if one is set.
    pub fn snackbar(&self) -> Option<&dyn Widget> {
        self.snackbar.as_deref()
    }

    /// Opens the leading drawer, if one is set. Returns whether it is now open.
    pub fn open_drawer(&mut self) -> bool {
        let opened = self.drawer.is_some() && !self.drawer_open;
        if opened {
            self.drawer_open = true;
            if self.drawer_animated {
                self.drawer_reveal.set_open(true);
            } else {
                self.drawer_reveal.jump_to(true);
            }
            self.base.request_redraw();
        }
        self.drawer_open
    }

    /// Closes the leading drawer. Returns whether it had been open.
    pub fn close_drawer(&mut self) -> bool {
        let closed = self.drawer_open;
        if closed {
            self.drawer_open = false;
            if self.drawer_animated {
                self.drawer_reveal.set_open(false);
            } else {
                self.drawer_reveal.jump_to(false);
            }
            self.base.request_redraw();
        }
        closed
    }

    /// Returns whether the leading drawer is open.
    pub fn is_drawer_open(&self) -> bool {
        self.drawer_open
    }

    /// Opens the trailing drawer, if one is set. Returns whether it is now open.
    pub fn open_end_drawer(&mut self) -> bool {
        let opened = self.end_drawer.is_some() && !self.end_drawer_open;
        if opened {
            self.end_drawer_open = true;
            if self.drawer_animated {
                self.end_drawer_reveal.set_open(true);
            } else {
                self.end_drawer_reveal.jump_to(true);
            }
            self.base.request_redraw();
        }
        self.end_drawer_open
    }

    /// Closes the trailing drawer. Returns whether it had been open.
    pub fn close_end_drawer(&mut self) -> bool {
        let closed = self.end_drawer_open;
        if closed {
            self.end_drawer_open = false;
            if self.drawer_animated {
                self.end_drawer_reveal.set_open(false);
            } else {
                self.end_drawer_reveal.jump_to(false);
            }
            self.base.request_redraw();
        }
        closed
    }

    /// Returns whether the trailing drawer is open.
    pub fn is_end_drawer_open(&self) -> bool {
        self.end_drawer_open
    }

    /// Selects which implementation paints the bottom navigation band.
    ///
    /// Switching to [`ScaffoldBottomNavStyle::Material3`] materialises a real
    /// [`BottomNavigationBar`] from the scaffold's items and selection; switching back drops it
    /// and restores the classic strip. The default is the classic strip, so existing pictures
    /// are unchanged.
    pub fn set_bottom_nav_style(&mut self, style: ScaffoldBottomNavStyle) {
        if self.bottom_nav_style == style {
            return;
        }
        self.bottom_nav_style = style;
        self.sync_m3_bar();
        self.base.request_redraw();
    }

    /// Returns the bottom navigation painting style.
    pub fn bottom_nav_style(&self) -> ScaffoldBottomNavStyle {
        self.bottom_nav_style
    }

    /// Advances the delegated bottom bar's pill and the scaffold's own transitions.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let mut moving = self.m3_bar.as_mut().is_some_and(|bar| bar.tick(delta_ms));
        moving |= self.drawer_reveal.tick(delta_ms);
        moving |= self.end_drawer_reveal.tick(delta_ms);
        moving |= self.page_reveal.tick(delta_ms);
        moving
    }

    /// Whether anything on the scaffold is between two states.
    pub fn is_animating(&self) -> bool {
        self.m3_bar.as_ref().is_some_and(|bar| bar.is_animating())
            || self.drawer_reveal.is_animating()
            || self.end_drawer_reveal.is_animating()
            || self.page_reveal.is_animating()
    }

    /// Rebuilds the delegated M3 bar from the scaffold's own items, selection and band.
    fn sync_m3_bar(&mut self) {
        if self.bottom_nav_style != ScaffoldBottomNavStyle::Material3 {
            self.m3_bar = None;
            return;
        }
        let Some(band) = self.bottom_nav_rect() else {
            self.m3_bar = None;
            return;
        };
        let mut bar = BottomNavigationBar::new(band);
        for (index, item) in self.nav_items.iter().enumerate() {
            bar.add_item(&item.icon, &item.label);
            if let Some(source) = self.nav_item_icon_sources.get(index).copied().flatten() {
                let _ = bar.set_item_icon_source(index, Some(source));
            }
            if let Some(badge) = self.nav_item_badges.get(index).cloned().flatten() {
                bar.set_badge(index, Some(badge));
            }
        }
        if !self.nav_items.is_empty() {
            bar.set_selected_index(self.selected_nav_index);
        }
        self.m3_bar = Some(bar);
    }

    /// Returns a reference to the optional body widget.
    pub fn body_widget(&self) -> Option<&dyn Widget> {
        self.body_widget.as_deref()
    }

    /// Returns a mutable reference to the optional body widget.
    pub fn body_widget_mut(&mut self) -> Option<&mut Box<dyn Widget>> {
        self.body_widget.as_mut()
    }

    /// Sets whether the bottom navigation bar is visible.
    pub fn set_show_bottom_nav(&mut self, show: bool) {
        if self.show_bottom_nav == show {
            return;
        }
        self.show_bottom_nav = show;
        self.base.request_redraw();
    }

    /// Returns whether the bottom navigation bar is visible.
    pub fn show_bottom_nav(&self) -> bool {
        self.show_bottom_nav
    }

    /// Adds a bottom navigation item with the given icon and label.
    pub fn add_nav_item(&mut self, icon: &str, label: &str) {
        self.nav_items.push(NavItem { icon: icon.to_string(), label: label.to_string() });
        self.nav_item_icon_sources.push(None);
        self.nav_item_badges.push(None);
        self.nav_bodies.push(None);
        self.sync_m3_bar();
        self.base.request_redraw();
    }

    /// Sets a badge on a bottom-navigation item; an empty string clears it.
    ///
    /// The Classic strip had no badge data at all while the delegated Material3 bar did, so the
    /// same scaffold showed a count in one style and nothing in the other.
    pub fn set_nav_item_badge(&mut self, index: usize, text: &str) -> bool {
        let Some(slot) = self.nav_item_badges.get_mut(index) else {
            return false;
        };
        let badge = if text.is_empty() { None } else { Some(text.to_string()) };
        if *slot == badge {
            return true;
        }
        *slot = badge.clone();
        if let Some(bar) = self.m3_bar.as_mut() {
            bar.set_badge(index, badge);
        }
        self.base.request_redraw();
        true
    }

    /// The badge on `index`, when there is one.
    pub fn nav_item_badge(&self, index: usize) -> Option<&str> {
        self.nav_item_badges.get(index).and_then(|badge| badge.as_deref())
    }

    /// How badges are painted.
    pub fn badge_style(&self) -> ScaffoldBadgeStyle {
        self.badge_style
    }

    /// Sets how badges are painted.
    pub fn set_badge_style(&mut self, style: ScaffoldBadgeStyle) {
        if self.badge_style == style {
            return;
        }
        self.badge_style = style;
        self.base.request_redraw();
    }

    /// Whether the drawers animate.
    pub fn drawer_animated(&self) -> bool {
        self.drawer_animated
    }

    /// Enables or disables the drawer slide.
    pub fn set_drawer_animated(&mut self, animated: bool) {
        if self.drawer_animated == animated {
            return;
        }
        self.drawer_animated = animated;
        if !animated {
            self.drawer_reveal.jump_to(self.drawer_open);
            self.end_drawer_reveal.jump_to(self.end_drawer_open);
        }
        self.base.request_redraw();
    }

    /// The scrim's opacity at a fully open drawer.
    pub fn scrim_opacity(&self) -> f32 {
        self.scrim_opacity
    }

    /// Sets the scrim's opacity at a fully open drawer, clamped to `0.0..=1.0`.
    pub fn set_scrim_opacity(&mut self, opacity: f32) {
        if !opacity.is_finite() {
            return;
        }
        let opacity = opacity.clamp(0.0, 1.0);
        if (self.scrim_opacity - opacity).abs() < f32::EPSILON {
            return;
        }
        self.scrim_opacity = opacity;
        self.base.request_redraw();
    }

    /// Whether a destination change animates its page.
    pub fn page_transition(&self) -> bool {
        self.page_transition
    }

    /// Enables or disables the page-change transition.
    pub fn set_page_transition(&mut self, animated: bool) {
        if self.page_transition == animated {
            return;
        }
        self.page_transition = animated;
        if !animated {
            self.page_reveal.jump_to(true);
        }
        self.base.request_redraw();
    }

    /// How far the leading drawer has slid in, `0.0` closed to `1.0` open.
    pub fn drawer_progress(&self) -> f32 {
        self.drawer_reveal.value()
    }

    /// Assigns a typed icon source to an existing navigation item index.
    pub fn set_nav_item_icon_source(
        &mut self,
        index: usize,
        source: Option<IconSource>,
    ) -> Result<(), IconItemError> {
        let len = self.nav_items.len();
        let Some(slot) = self.nav_item_icon_sources.get_mut(index) else {
            return Err(IconItemError::IndexOutOfBounds { index, len });
        };
        *slot = source;
        self.sync_m3_bar();
        self.base.request_redraw();
        Ok(())
    }

    /// Removes all bottom navigation items.
    pub fn clear_nav_items(&mut self) {
        self.nav_items.clear();
        self.nav_item_icon_sources.clear();
        self.nav_item_badges.clear();
        self.nav_bodies.clear();
        self.selected_nav_index = 0;
        self.sync_m3_bar();
        self.base.request_redraw();
    }

    /// Returns the number of bottom navigation items.
    pub fn nav_item_count(&self) -> usize {
        self.nav_items.len()
    }

    /// Sets the selected bottom navigation index.
    pub fn set_selected_nav_index(&mut self, index: usize) {
        let clamped =
            if self.nav_items.is_empty() { 0 } else { index.min(self.nav_items.len() - 1) };
        if self.selected_nav_index != clamped {
            self.selected_nav_index = clamped;
            if let Some(bar) = self.m3_bar.as_mut() {
                bar.set_selected_index(clamped);
            }
            // A destination change is a page *arrival*: the new page slides in and fades up
            // from the scaffold's own background, so a switch is visible as a direction rather
            // than as an instantaneous swap.
            if self.page_transition {
                self.page_reveal.jump_to(false);
                self.page_reveal.set_open(true);
            }
            self.nav_selected.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the current selected bottom navigation index.
    pub fn selected_nav_index(&self) -> usize {
        self.selected_nav_index
    }

    /// Returns the content area rect (between AppBar and bottom nav).
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let top_offset: i32 = APP_BAR_HEIGHT as i32;
        let bottom_offset: i32 = if self.show_bottom_nav { BOTTOM_NAV_HEIGHT as i32 } else { 0 };
        Rect::new(
            rect.x,
            rect.y + top_offset,
            rect.width,
            (rect.height as i32 - top_offset - bottom_offset).max(0) as u32,
        )
    }

    /// Returns the bottom nav bar rect.
    fn bottom_nav_rect(&self) -> Option<Rect> {
        if !self.show_bottom_nav || self.nav_items.is_empty() {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(
            rect.x,
            rect.y + rect.height as i32 - BOTTOM_NAV_HEIGHT as i32,
            rect.width,
            BOTTOM_NAV_HEIGHT,
        ))
    }

    /// Returns the floating action button's slot rectangle.
    fn fab_rect(&self) -> Option<Rect> {
        self.fab.as_ref()?;
        let rect = self.geometry();
        let bottom_inset = if self.show_bottom_nav && !self.nav_items.is_empty() {
            BOTTOM_NAV_HEIGHT as i32
        } else {
            0
        };
        Some(Rect::new(
            rect.x + rect.width as i32 - FAB_SLOT_SIZE as i32 - FAB_SLOT_MARGIN,
            rect.y + rect.height as i32 - FAB_SLOT_SIZE as i32 - bottom_inset - FAB_SLOT_MARGIN,
            FAB_SLOT_SIZE,
            FAB_SLOT_SIZE,
        ))
    }

    /// Returns the leading drawer's panel rectangle.
    fn drawer_rect(&self) -> Rect {
        let rect = self.geometry();
        let width = DRAWER_WIDTH.min(rect.width);
        Rect::new(
            rect.x,
            rect.y + APP_BAR_HEIGHT as i32,
            width,
            rect.height.saturating_sub(APP_BAR_HEIGHT),
        )
    }

    /// Returns the trailing drawer's panel rectangle.
    fn end_drawer_rect(&self) -> Rect {
        let rect = self.geometry();
        let width = DRAWER_WIDTH.min(rect.width);
        Rect::new(
            rect.x + rect.width as i32 - width as i32,
            rect.y + APP_BAR_HEIGHT as i32,
            width,
            rect.height.saturating_sub(APP_BAR_HEIGHT),
        )
    }

    /// Returns the bottom sheet's rectangle: the lower half of the content area.
    fn bottom_sheet_rect(&self) -> Rect {
        let content = self.content_rect();
        let height = (content.height / 2).max(1);
        Rect::new(
            content.x,
            content.y + content.height as i32 - height as i32,
            content.width,
            height,
        )
    }

    /// Returns the snackbar's rectangle, sitting above the bottom navigation band.
    fn snackbar_rect(&self) -> Rect {
        let rect = self.geometry();
        let bottom_inset = if self.show_bottom_nav && !self.nav_items.is_empty() {
            BOTTOM_NAV_HEIGHT as i32
        } else {
            0
        };
        Rect::new(
            rect.x + SNACKBAR_MARGIN,
            rect.y + rect.height as i32 - bottom_inset - SNACKBAR_HEIGHT as i32 - SNACKBAR_MARGIN,
            rect.width.saturating_sub((SNACKBAR_MARGIN * 2) as u32),
            SNACKBAR_HEIGHT,
        )
    }
}

impl Widget for AdaptiveScaffold {
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
        crate::core::Size::new(400, 600)
    }

    /// Advances the delegated bottom bar's selection pill; `true` while it is still moving.
    fn tick(&mut self, delta_ms: u32) -> bool {
        AdaptiveScaffold::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        AdaptiveScaffold::is_animating(self)
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
            "nav_selected" => {
                Some(EventSignalRef::mapped("nav_selected", &self.nav_selected, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `AdaptiveScaffold`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The title is forwarded to the
/// scaffold's internal `AppBar` by the setter.
impl WidgetProperties for AdaptiveScaffold {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "badge_style" => Ok(CapabilityValue::String(self.badge_style().as_str().to_string())),
            "drawer_animated" => Ok(CapabilityValue::Bool(self.drawer_animated())),
            "scrim_opacity" => Ok(CapabilityValue::Float(f64::from(self.scrim_opacity()))),
            "page_transition" => Ok(CapabilityValue::Bool(self.page_transition())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(&expect_string(value)?);
                Ok(())
            }
            "badge_style" => {
                let text = match value {
                    CapabilityValue::String(text) => text,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                let style = ScaffoldBadgeStyle::from_name(&text)
                    .ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_badge_style(style);
                Ok(())
            }
            "drawer_animated" => {
                let animated = match value {
                    CapabilityValue::Bool(animated) => animated,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_drawer_animated(animated);
                Ok(())
            }
            "scrim_opacity" => {
                let opacity = expect_f32(value)?;
                if !(0.0..=1.0).contains(&opacity) {
                    return Err(CapabilityAccessError::OutOfRange);
                }
                self.set_scrim_opacity(opacity);
                Ok(())
            }
            "page_transition" => {
                let animated = match value {
                    CapabilityValue::Bool(animated) => animated,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_page_transition(animated);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "badge_style",
            "drawer_animated",
            "scrim_opacity",
            "page_transition",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for AdaptiveScaffold {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because the scaffold fill
        // was previously hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("adaptive_scaffold");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgba(245, 246, 248, 255));

        // ── Background ──
        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // ── AppBar ──
        self.app_bar.draw(context);

        // ── Body content area ──
        //
        // The selected destination's page is painted into the content band. A shared fallback
        // body is used when the destination has no page of its own, so a scaffold that only
        // ever set `body_widget` behaves as before.
        let content_rect = self.content_rect();
        context.fill_rect(content_rect, background);
        let body: Option<&mut dyn Widget> = {
            let selected = self
                .nav_bodies
                .get_mut(self.selected_nav_index)
                .and_then(|page| page.as_deref_mut());
            match selected {
                Some(page) => Some(page),
                None => self.body_widget.as_deref_mut(),
            }
        };
        if let Some(body) = body {
            body.set_geometry(content_rect);
            if let Some(drawable) = crate::widget::draw_bridge::draw_of(body) {
                // The page arrives from slightly below and fades up from the scaffold's own
                // background. At rest (`progress == 1`) both the offset and the veil are zero,
                // so a settled frame is byte-identical to the un-animated one.
                let progress = self.page_reveal.value().clamp(0.0, 1.0);
                let offset =
                    if self.page_transition { ((1.0 - progress) * 10.0) as i32 } else { 0 };
                context.push_clip(
                    content_rect.x,
                    content_rect.y,
                    content_rect.width,
                    content_rect.height,
                );
                context.push_offset(0, offset);
                drawable.draw(context);
                context.pop_offset();
                if self.page_transition && progress < 1.0 {
                    // A veil in the scaffold's background colour: painting it over the body on
                    // each frame of the reveal is what makes the page fade **up from** the
                    // surface rather than pop in at a lighter weight.
                    context.fill_rect(
                        content_rect,
                        background.with_alpha(((1.0 - progress) * 235.0) as u8),
                    );
                }
                context.pop_clip();
            }
        }

        // ── Bottom navigation bar ──
        //
        // The rect is asked for once and the draw is guarded on the answer, so "is the nav bar
        // shown" is decided in **one** place.
        if let Some(nav_rect) = self.bottom_nav_rect() {
            match self.bottom_nav_style {
                ScaffoldBottomNavStyle::Classic => self.draw_bottom_nav(context, nav_rect),
                ScaffoldBottomNavStyle::Material3 => {
                    if let Some(mut bar) = self.m3_bar.take() {
                        bar.set_geometry(nav_rect);
                        bar.draw(context);
                        self.m3_bar = Some(bar);
                    }
                }
            }
        }

        // ── Overlay slots, painted in z order ──
        //
        // Each slot owns its band; the widget's own geometry is set to that band before it
        // draws, which is the same contract the content child follows.
        //
        // A drawer is painted for every non-zero slide, not only while `open`, so the closing
        // half of the transition has something to draw. The scrim fades with the same fraction.
        let drawer_progress = self.drawer_reveal.value().clamp(0.0, 1.0);
        if self.drawer_open || drawer_progress > 0.0 {
            let scrim_alpha = (self.scrim_opacity * drawer_progress * 255.0) as u8;
            if scrim_alpha > 0 {
                context.fill_rect(rect, Color::BLACK.with_alpha(scrim_alpha));
            }
            let band = self.drawer_rect();
            let offset = -(((1.0 - drawer_progress) * band.width as f32) as i32);
            if let Some(panel) = self.drawer.as_deref_mut() {
                panel.set_geometry(band);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(panel) {
                    context.push_clip(rect.x, rect.y, rect.width, rect.height);
                    context.push_offset(offset, 0);
                    drawable.draw(context);
                    context.pop_offset();
                    context.pop_clip();
                }
            }
        }
        let end_progress = self.end_drawer_reveal.value().clamp(0.0, 1.0);
        if self.end_drawer_open || end_progress > 0.0 {
            let scrim_alpha = (self.scrim_opacity * end_progress * 255.0) as u8;
            if scrim_alpha > 0 {
                context.fill_rect(rect, Color::BLACK.with_alpha(scrim_alpha));
            }
            let band = self.end_drawer_rect();
            let offset = ((1.0 - end_progress) * band.width as f32) as i32;
            if let Some(panel) = self.end_drawer.as_deref_mut() {
                panel.set_geometry(band);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(panel) {
                    context.push_clip(rect.x, rect.y, rect.width, rect.height);
                    context.push_offset(offset, 0);
                    drawable.draw(context);
                    context.pop_offset();
                    context.pop_clip();
                }
            }
        }
        if self.bottom_sheet.is_some() {
            let band = self.bottom_sheet_rect();
            if let Some(sheet) = self.bottom_sheet.as_deref_mut() {
                sheet.set_geometry(band);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(sheet) {
                    drawable.draw(context);
                }
            }
        }
        if self.snackbar.is_some() {
            let band = self.snackbar_rect();
            if let Some(snackbar) = self.snackbar.as_deref_mut() {
                snackbar.set_geometry(band);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(snackbar) {
                    drawable.draw(context);
                }
            }
        }
        if let Some(band) = self.fab_rect() {
            if let Some(fab) = self.fab.as_deref_mut() {
                fab.set_geometry(band);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(fab) {
                    drawable.draw(context);
                }
            }
        }
    }
}

impl AdaptiveScaffold {
    /// Draws the bottom navigation bar manually within the scaffold.
    fn draw_bottom_nav(&mut self, context: &mut RenderContext, nav_rect: Rect) {
        let item_count = self.nav_items.len();
        if item_count == 0 {
            return;
        }

        let tab_width = nav_rect.width / item_count as u32;

        // The bar draws with the scaffold's own resolved chrome, so the whole control
        // moves together on a theme switch rather than the bar keeping a literal white.
        // These are further manager locks, each taken and released inside the accessor.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("adaptive_scaffold");
        let bar_background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let bar_ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let bar_border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::DIVIDER);

        // Background
        context.fill_rect(nav_rect, bar_background);

        // Top border
        context.draw_line(
            Point::new(nav_rect.x, nav_rect.y),
            Point::new(nav_rect.x + nav_rect.width as i32, nav_rect.y),
            bar_border,
        );

        let icon_font_size = (nav_rect.height as f32 * 0.32).clamp(14.0, 28.0);
        let label_font_size = (nav_rect.height as f32 * 0.18).clamp(9.0, 14.0);

        let icon_font = role_font(style, icon_font_size);
        let label_font = role_font(style, label_font_size);

        for (i, item) in self.nav_items.iter().enumerate() {
            let tab_x = nav_rect.x + (i as u32 * tab_width) as i32;
            let tab_rect = Rect::new(tab_x, nav_rect.y, tab_width, nav_rect.height);

            let is_selected = i == self.selected_nav_index;
            // Selected tab is the bar's ink; the unselected tabs are the same ink
            // damped toward the bar, so both move with the appearance.
            let icon_color =
                if is_selected { bar_ink } else { bar_ink.blend(&bar_background, 0.45) };
            let label_color = icon_color;

            let typed_source = self.nav_item_icon_sources.get(i).copied().flatten();
            let mut icon_metrics = context
                .measure_text(if typed_source.is_some() { "M" } else { &item.icon }, &icon_font);
            if typed_source.is_some() {
                icon_metrics.width = icon_metrics.height.max(1);
            }
            let label_metrics = context.measure_text(&item.label, &label_font);

            let total_content_height = icon_metrics.height + label_metrics.height + 4;
            let content_y = tab_rect.y + (tab_rect.height as i32 - total_content_height as i32) / 2;

            // Icon. The string is resolved as an **icon** first (`draw_icon_named`), falling back to
            // text only when it names no icon at all — so a host passing `"star"` gets the real
            // outline while one still passing `"★"` keeps the text path it always had.
            //
            // `content_y` is the *top* of the icon+label stack and the glyph origin is itself
            // the box's top edge, so the ascent term that used to be added here pushed the
            // icon a full line down and the label with it, walking the pair out of the bar.
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
                    log::warn!("adaptive navigation item {i} icon failed: {error:?}");
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

            // Label
            let label_x = tab_rect.x + (tab_rect.width as i32 - label_metrics.width as i32) / 2;
            let label_y = content_y + icon_metrics.height as i32 + 4;
            context.draw_text(
                Point::new(label_x, label_y),
                &item.label,
                &label_font,
                label_color,
                HorizontalAlignment::Left,
            );

            // Badge: a capsule (or a dot) at the icon's trailing-top corner. The delegated M3
            // bar has always had this; the Classic strip had no badge data at all, so the same
            // scaffold answered differently depending on the style.
            if let Some(badge) = self.nav_item_badges.get(i).and_then(|badge| badge.as_deref()) {
                if !badge.is_empty() {
                    let badge_fill =
                        crate::style::semantic_color(crate::style::SemanticColor::Error)
                            .unwrap_or(bar_ink);
                    let badge_ink = badge_fill.contrast_color();
                    match self.badge_style {
                        ScaffoldBadgeStyle::Dot => {
                            let side = 8u32.min(icon_box.width.max(1));
                            let dot = Rect::new(
                                icon_box.x + icon_box.width as i32 - side as i32 / 2,
                                icon_box.y - 2,
                                side,
                                side,
                            );
                            context.fill_rounded_rect(dot, side / 2, badge_fill);
                        }
                        ScaffoldBadgeStyle::Text => {
                            let metrics = context.measure_text(badge, &label_font);
                            let height = label_font.size().max(10.0) as u32 + 4;
                            let width = (metrics.width + 8).max(height);
                            let badge_rect = Rect::new(
                                icon_box.x + icon_box.width as i32 - width as i32 / 2,
                                icon_box.y - 2,
                                width,
                                height,
                            );
                            context.fill_rounded_rect(badge_rect, height / 2, badge_fill);
                            context.draw_text(
                                Point::new(
                                    badge_rect.x + 4,
                                    context.text_line(badge_rect, &label_font).y,
                                ),
                                badge,
                                &label_font,
                                badge_ink,
                                HorizontalAlignment::Left,
                            );
                        }
                    }
                }
            }

            // Selected indicator line
            if is_selected {
                let indicator_height = 3u32;
                let indicator_width = (tab_width * 3 / 5).max(20).min(tab_width);
                let indicator_x = tab_rect.x + (tab_rect.width as i32 - indicator_width as i32) / 2;
                let indicator_rect =
                    Rect::new(indicator_x, nav_rect.y, indicator_width, indicator_height);
                context.fill_rounded_rect(indicator_rect, 1, bar_ink);
            }
        }
    }
}

impl EventHandler for AdaptiveScaffold {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button } => {
                // A disabled scaffold must not navigate between destinations, nor
                // forward the click to its app bar. Without this gate
                // `set_enabled(false)` had no effect on this widget.
                if !self.base.is_enabled() {
                    self.base.handle_event(event);
                    return;
                }
                if *button != 1 {
                    return;
                }

                // Overlay slots win over the page beneath them, in reverse z order. Each slot is
                // told the band it was painted in, so its own hit test sees the pixels it owns.
                let mut routed = false;
                if let Some(band) = self.fab_rect() {
                    if band.contains_point(*pos) {
                        if let Some(fab) = self.fab.as_deref_mut() {
                            fab.set_geometry(band);
                            fab.handle_event(event);
                            routed = true;
                        }
                    }
                }
                if !routed && self.snackbar.is_some() {
                    let band = self.snackbar_rect();
                    if band.contains_point(*pos) {
                        if let Some(snackbar) = self.snackbar.as_deref_mut() {
                            snackbar.set_geometry(band);
                            snackbar.handle_event(event);
                            routed = true;
                        }
                    }
                }
                if !routed && self.bottom_sheet.is_some() {
                    let band = self.bottom_sheet_rect();
                    if band.contains_point(*pos) {
                        if let Some(sheet) = self.bottom_sheet.as_deref_mut() {
                            sheet.set_geometry(band);
                            sheet.handle_event(event);
                            routed = true;
                        }
                    }
                }
                if !routed && self.end_drawer_open {
                    let band = self.end_drawer_rect();
                    if band.contains_point(*pos) {
                        if let Some(panel) = self.end_drawer.as_deref_mut() {
                            panel.set_geometry(band);
                            panel.handle_event(event);
                            routed = true;
                        }
                    }
                }
                if !routed && self.drawer_open {
                    let band = self.drawer_rect();
                    if band.contains_point(*pos) {
                        if let Some(panel) = self.drawer.as_deref_mut() {
                            panel.set_geometry(band);
                            panel.handle_event(event);
                            routed = true;
                        }
                    }
                }
                if routed {
                    return;
                }
                let rect = self.geometry();
                let app_bar_height: i32 = APP_BAR_HEIGHT as i32;

                // AppBar zone (top)
                if pos.y >= rect.y && pos.y < rect.y + app_bar_height {
                    // The AppBar's signals are mirrored onto this scaffold at construction,
                    // so forwarding the event is all this arm has to do.
                    self.app_bar.handle_event(event);
                    return;
                }

                // Bottom nav zone
                if self.show_bottom_nav && !self.nav_items.is_empty() {
                    let nav_top = rect.y + rect.height as i32 - BOTTOM_NAV_HEIGHT as i32;
                    if pos.y >= nav_top && pos.y <= rect.y + rect.height as i32 {
                        // Hit-test bottom nav items
                        let item_count = self.nav_items.len();
                        let tab_width = rect.width / item_count as u32;
                        let relative_x = pos.x - rect.x;
                        if relative_x >= 0 {
                            let index = (relative_x as u32 / tab_width) as usize;
                            if index < item_count {
                                self.set_selected_nav_index(index);
                                return;
                            }
                        }
                    }
                }

                // Body zone — delegate to the selected destination's page, then the fallback.
                let content_rect = self.content_rect();
                if content_rect.contains_point(*pos) {
                    let target = self
                        .nav_bodies
                        .get_mut(self.selected_nav_index)
                        .and_then(|page| page.as_deref_mut());
                    let body = match target {
                        Some(page) => Some(page),
                        None => self.body_widget.as_deref_mut(),
                    };
                    if let Some(body) = body {
                        body.set_geometry(content_rect);
                        body.handle_event(event);
                    }
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
    use crate::core::Point;
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn make_scaffold() -> AdaptiveScaffold {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        scaffold.add_nav_item("★", "Favorites");
        scaffold.add_nav_item("✉", "Messages");
        scaffold.add_nav_item("⌂", "Home");
        scaffold.set_show_bottom_nav(true);
        scaffold
    }

    #[test]
    fn adaptive_scaffold_creation() {
        let scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert_eq!(scaffold.kind(), WidgetKind::AdaptiveScaffold);
        assert_eq!(scaffold.title(), "Home");
        assert!(!scaffold.show_back());
        assert!(!scaffold.show_bottom_nav());
        assert_eq!(scaffold.geometry(), Rect::new(0, 0, 375, 812));
    }

    #[test]
    fn adaptive_scaffold_title_accessors() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert_eq!(scaffold.title(), "Home");

        scaffold.set_title("Settings");
        assert_eq!(scaffold.title(), "Settings");
    }

    #[test]
    fn adaptive_scaffold_back_accessors() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert!(!scaffold.show_back());

        scaffold.set_show_back(true);
        assert!(scaffold.show_back());
    }

    #[test]
    fn adaptive_scaffold_action_text_accessors() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert_eq!(scaffold.action_text(), "");

        scaffold.set_action_text("Save");
        assert_eq!(scaffold.action_text(), "Save");
    }

    #[test]
    fn adaptive_scaffold_bottom_nav_toggle() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert!(!scaffold.show_bottom_nav());

        scaffold.set_show_bottom_nav(true);
        assert!(scaffold.show_bottom_nav());

        scaffold.set_show_bottom_nav(false);
        assert!(!scaffold.show_bottom_nav());
    }

    #[test]
    fn adaptive_scaffold_add_nav_items() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert_eq!(scaffold.nav_item_count(), 0);

        scaffold.add_nav_item("★", "Favorites");
        assert_eq!(scaffold.nav_item_count(), 1);

        scaffold.add_nav_item("✉", "Messages");
        assert_eq!(scaffold.nav_item_count(), 2);
    }

    #[test]
    fn adaptive_nav_item_typed_source_is_index_checked_and_rendered() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "adaptive-nav-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut scaffold = make_scaffold();
        scaffold.set_show_bottom_nav(true);
        assert_eq!(
            scaffold.set_nav_item_icon_source(5, Some(IconSource::static_geometry(&RING))),
            Err(IconItemError::IndexOutOfBounds { index: 5, len: 3 })
        );
        scaffold
            .set_nav_item_icon_source(1, Some(IconSource::static_geometry(&RING)))
            .expect("existing item accepts a typed source");
        let svg = crate::widget::svg::render_to_svg(&mut scaffold);
        assert!(svg.contains("fill-rule=\"nonzero\""), "typed geometry must reach SVG");

        scaffold.clear_nav_items();
        assert!(scaffold.nav_item_icon_sources.is_empty());
    }

    #[test]
    fn adaptive_scaffold_clear_nav_items() {
        let mut scaffold = make_scaffold();
        assert_eq!(scaffold.nav_item_count(), 3);

        scaffold.clear_nav_items();
        assert_eq!(scaffold.nav_item_count(), 0);
    }

    #[test]
    fn adaptive_scaffold_select_nav_index() {
        let mut scaffold = make_scaffold();
        assert_eq!(scaffold.selected_nav_index(), 0);

        scaffold.set_selected_nav_index(2);
        assert_eq!(scaffold.selected_nav_index(), 2);

        scaffold.set_selected_nav_index(0);
        assert_eq!(scaffold.selected_nav_index(), 0);
    }

    #[test]
    fn adaptive_scaffold_nav_selected_signal_emits() {
        let mut scaffold = make_scaffold();
        let captured = Arc::new(AtomicUsize::new(usize::MAX));
        let c = captured.clone();
        scaffold.nav_selected.connect(move |val: Arc<usize>| {
            c.store(*val, Ordering::SeqCst);
        });

        scaffold.set_selected_nav_index(1);
        assert_eq!(captured.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn adaptive_scaffold_mouse_click_nav() {
        let mut scaffold = make_scaffold();
        // Bottom nav is at y 812-56 = 756 to 812
        // Tab width = 375/3 = 125 per tab
        // Click on tab 2 (x in [250..374])
        scaffold.handle_event(&Event::MousePress {
            pos: Point::new(300, 780),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(scaffold.selected_nav_index(), 2);
    }

    /// A disabled scaffold must not change its selected destination on a click.
    ///
    /// `handle_event` hit-tested the bottom nav bar without consulting `is_enabled()`,
    /// so `set_enabled(false)` left navigation fully live — the flag was stored and
    /// never honoured.
    #[test]
    fn adaptive_scaffold_disabled_ignores_nav_click() {
        let mut scaffold = make_scaffold();
        scaffold.set_enabled(false);

        scaffold.handle_event(&Event::MousePress {
            pos: Point::new(300, 780),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(scaffold.selected_nav_index(), 0, "a disabled scaffold must keep its selection");

        // Re-enabling restores navigation, so the gate suspends rather than locks.
        scaffold.set_enabled(true);
        scaffold.handle_event(&Event::MousePress {
            pos: Point::new(300, 780),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(scaffold.selected_nav_index(), 2, "re-enabling must restore navigation");
    }

    #[test]
    fn adaptive_scaffold_svg_output() {
        let mut scaffold = make_scaffold();
        let svg = render_to_svg(&mut scaffold);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn adaptive_scaffold_svg_without_bottom_nav() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        let svg = render_to_svg(&mut scaffold);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn adaptive_scaffold_content_rect() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        // Without bottom nav: content = y 56, h 812-56 = 756
        let content = scaffold.content_rect();
        assert_eq!(content, Rect::new(0, 56, 375, 756));

        scaffold.set_show_bottom_nav(true);
        scaffold.add_nav_item("★", "Test");
        // With bottom nav: content = y 56, h 812-56-56 = 700
        let content = scaffold.content_rect();
        assert_eq!(content, Rect::new(0, 56, 375, 700));
    }

    /// The scaffold used to hold its AppBar privately and drop its signals, so a page
    /// built on the scaffold had no way to observe a back tap.
    #[test]
    fn adaptive_scaffold_forwards_app_bar_back_pressed() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        scaffold.set_show_back(true);

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        scaffold.back_pressed.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // The AppBar commits on release, so a press alone must not reach the scaffold.
        scaffold.handle_event(&Event::MousePress {
            pos: Point::new(10, 28),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "a press is not a committed tap");

        scaffold.handle_event(&Event::MouseRelease { pos: Point::new(10, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "one tap forwards exactly one back event");
    }

    #[test]
    fn adaptive_scaffold_forwards_app_bar_action_pressed() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        scaffold.set_action_text("Save");

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        scaffold.action_pressed.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        scaffold.handle_event(&Event::MousePress {
            pos: Point::new(340, 28),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "a press is not a committed tap");

        scaffold.handle_event(&Event::MouseRelease { pos: Point::new(340, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "one tap forwards exactly one action event");
    }

    #[test]
    fn selecting_a_destination_shows_that_destinations_page() {
        let mut scaffold = make_scaffold();
        scaffold.set_nav_body(
            1,
            Some(Box::new(crate::widget::base_widgets::label::Label::new(
                "Messages".to_string(),
                Rect::new(0, 0, 10, 10),
            ))),
        );
        assert!(scaffold.nav_body(1).is_some());
        assert!(scaffold.active_body().is_none(), "the shared body is unused when pages exist");
        scaffold.set_selected_nav_index(1);
        assert!(scaffold.active_body().is_some(), "selection must switch the painted page");
    }

    #[test]
    fn a_material3_bottom_bar_is_a_real_bottom_navigation_bar() {
        let mut scaffold = make_scaffold();
        assert_eq!(scaffold.bottom_nav_style(), ScaffoldBottomNavStyle::Classic);
        scaffold.set_bottom_nav_style(ScaffoldBottomNavStyle::Material3);
        assert_eq!(scaffold.bottom_nav_style(), ScaffoldBottomNavStyle::Material3);
        scaffold.set_bottom_nav_style(ScaffoldBottomNavStyle::Classic);
        assert!(!scaffold.is_animating(), "the classic strip owes no frames");
    }

    #[test]
    fn each_overlay_slot_has_its_own_band() {
        let mut scaffold = make_scaffold();
        fn widget() -> Box<dyn Widget> {
            Box::new(crate::widget::base_widgets::label::Label::new(
                "slot".to_string(),
                Rect::new(0, 0, 10, 10),
            ))
        }
        scaffold.set_fab(Some(widget()));
        scaffold.set_snackbar(Some(widget()));
        scaffold.set_bottom_sheet(Some(widget()));
        scaffold.set_drawer(Some(widget()));
        scaffold.set_end_drawer(Some(widget()));
        assert!(scaffold.fab().is_some());
        assert!(scaffold.snackbar().is_some());
        assert!(scaffold.bottom_sheet().is_some());
        assert!(scaffold.drawer().is_some());
        assert!(scaffold.end_drawer().is_some());
        assert!(scaffold.open_drawer());
        assert!(scaffold.is_drawer_open());
        assert!(scaffold.open_end_drawer());
        assert!(scaffold.is_end_drawer_open());
        let svg = render_to_svg(&mut scaffold);
        assert!(svg.starts_with("<svg"));
    }
}
