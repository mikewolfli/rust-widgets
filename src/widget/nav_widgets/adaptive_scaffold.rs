// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AdaptiveScaffold — a cross-platform adaptive scaffold widget.
//!
//! The AdaptiveScaffold combines an AppBar (top bar), a main content area,
//! and an optional BottomNavigationBar into a single cohesive layout.
//! It is designed to work on both iOS (Cupertino) and Android (Material)
//! platforms by using the same underlying AppBar and BottomNavigationBar
//! primitives with platform-appropriate defaults.

use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, TextDirection};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::cupertino::MaterialNavigationRail;
use crate::widget::menu_toolbar::popup_reveal::PopupReveal;
use crate::widget::metrics::role_font;
use crate::widget::nav_widgets::app_bar::AppBar;
use crate::widget::nav_widgets::bottom_navigation_bar::{BottomNavigationBar, NavItem};
use crate::widget::{BaseWidget, Draw, IconItemError, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// Declared-content (id-based) slots. A child module rather than a sibling file so it can see
// this control's private slot fields and rect helpers without widening them to `pub(crate)`;
// split out because `adaptive_scaffold.rs` is otherwise over the source-structure limit
// (principle #9).
#[path = "adaptive_scaffold_ids.rs"]
mod ids;

/// Height of the scaffold's app-bar band, in logical pixels.
///
/// The Material app bar's own height, named once because the layout arithmetic, the
/// hit test and the child slots all key off the same band.
const APP_BAR_HEIGHT: u32 = 56;

/// Height of the bottom navigation band, in logical pixels.
const BOTTOM_NAV_HEIGHT: u32 = 56;

/// Height of the persistent footer band, in logical pixels.
///
/// The footer sits between the body and the bottom navigation band, so it reserves space from
/// the content area rather than floating over it.
const PERSISTENT_FOOTER_HEIGHT: u32 = 56;

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

/// Width of the leading band a navigation rail occupies, in logical pixels.
///
/// `MaterialNavigationRail` paints a fixed-width strip of its own; the band handed to it matches
/// that width so the scaffold's content inset and the painted strip cannot disagree. The value is
/// repeated here because the rail's own spelling is private to its module.
const NAVIGATION_RAIL_WIDTH: u32 = 80;

/// Reports a declared (id-based) snackbar slot's visibility.
///
/// A declared control belongs to the runtime, so the scaffold can only read it. The control's own
/// visibility is authoritative: a `MaterialSnackbar` that has timed out or been dismissed reports
/// hidden, and any other widget falls back to its base flag.
fn scaffold_snackbar_visible(id: ObjectId) -> bool {
    crate::widget::runtime::with_widget(id, |widget| {
        crate::widget::capability::widget_as::<crate::widget::MaterialSnackbar>(widget)
            .map_or_else(|| widget.base().is_visible(), |snackbar| snackbar.is_visible())
    })
    .unwrap_or_else(|| crate::is_widget_visible(id))
}

/// Reports a declared (id-based) bottom-sheet slot's open state.
///
/// A `BottomSheet` stays base-visible while closed (it paints a scrim), so its `open` flag is the
/// authoritative answer. Any other widget falls back to its base visibility.
fn scaffold_bottom_sheet_open(id: ObjectId) -> bool {
    crate::widget::runtime::with_widget(id, |widget| {
        crate::widget::capability::widget_as::<crate::widget::BottomSheet>(widget)
            .map_or_else(|| widget.base().is_visible(), |sheet| sheet.is_open())
    })
    .unwrap_or_else(|| crate::is_widget_visible(id))
}

/// Largest window width, in logical pixels, that still uses the bottom bar.
///
/// The default is Material's compact/medium boundary: `0..=599` is compact, `600..=839` is medium
/// and `840` upward is expanded.
const DEFAULT_NARROW_MAX_WIDTH: u32 = 599;

/// Smallest window width, in logical pixels, that uses the drawer.
const DEFAULT_WIDE_MIN_WIDTH: u32 = 840;

/// Which implementation paints the scaffold's bottom navigation bar.
///
/// [`Self::Classic`] is the scaffold's own compact M2-style strip and is the default,
/// so a scaffold that never asks for anything else paints exactly the bytes it always
/// did. [`Self::Material3`] hands the band to a real
/// `BottomNavigationBar`,
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

impl ScaffoldBottomNavStyle {
    /// The token this style is published under (N2-B1).
    pub fn token(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Material3 => "material3",
        }
    }

    /// Parses a published token; `None` for an unknown spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "classic" => Some(Self::Classic),
            "material3" => Some(Self::Material3),
            _ => None,
        }
    }
}

/// Which navigation surface the scaffold uses for its current width.
///
/// The form is derived from the scaffold's own geometry width through
/// [`AdaptiveScaffold::navigation_form_for`], so a host that resizes the window does not have to
/// tell the scaffold which surface to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaffoldNavigationForm {
    /// Narrow windows: the bottom navigation bar.
    #[default]
    BottomBar,
    /// Medium windows: the leading navigation rail, when one is installed.
    Rail,
    /// Wide windows: the navigation drawer.
    Drawer,
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
    /// The leading rail painted in the medium breakpoint, when the host installed one.
    navigation_rail: Option<MaterialNavigationRail>,
    /// Largest width (inclusive) that uses the bottom bar.
    narrow_max: u32,
    /// Smallest width (inclusive) that uses the drawer.
    wide_min: u32,
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
    /// Optional persistent footer: a band below the body and above the bottom navigation bar.
    persistent_footer: Option<Box<dyn Widget>>,
    /// Object-id content slots (issue-20261104 DM-06/DM-08).
    ///
    /// A JSON document's controls are mounted by the runtime, which owns them, so a
    /// declaration cannot hand a `Box<dyn Widget>` to the slot setters above. These fields
    /// record the id the mount assigned instead, and [`Self::sync_id_slots`] applies the
    /// geometry and visibility the `Box` slots receive in [`Self::draw`] — the tree painter
    /// then draws the child, exactly as it draws any other child of this control. The two
    /// content models are mutually exclusive per slot: when an id slot holds a value it wins,
    /// because it is the one the document declared.
    body_id: Option<ObjectId>,
    nav_body_ids: Vec<Option<ObjectId>>,
    fab_id: Option<ObjectId>,
    drawer_id: Option<ObjectId>,
    end_drawer_id: Option<ObjectId>,
    bottom_sheet_id: Option<ObjectId>,
    snackbar_id: Option<ObjectId>,
    persistent_footer_id: Option<ObjectId>,
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
    /// Emitted with the index of a trailing AppBar action button (N-08).
    pub action_index_pressed: Signal1<usize>,
    /// Emitted when the AppBar's overflow affordance is activated (N-08).
    pub action_overflow_pressed: GenericSignal,
    /// Whether a press landed on the modal scrim and its release must be swallowed (N-07).
    scrim_pressed: bool,
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
            navigation_rail: None,
            narrow_max: DEFAULT_NARROW_MAX_WIDTH,
            wide_min: DEFAULT_WIDE_MIN_WIDTH,
            m3_bar: None,
            nav_bodies: Vec::new(),
            fab: None,
            drawer: None,
            end_drawer: None,
            bottom_sheet: None,
            snackbar: None,
            persistent_footer: None,
            body_id: None,
            nav_body_ids: Vec::new(),
            fab_id: None,
            drawer_id: None,
            end_drawer_id: None,
            bottom_sheet_id: None,
            snackbar_id: None,
            persistent_footer_id: None,
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
            action_index_pressed: Signal1::new(),
            action_overflow_pressed: GenericSignal::new(),
            scrim_pressed: false,
        };

        // Mirror the AppBar's two signals onto the scaffold's own. The handles are
        // intentionally dropped: a `ConnectionHandle` only lets a caller disconnect, and
        // the slot lives for as long as the signal it was registered on does.
        let back_out = scaffold.back_pressed.clone();
        let _ = scaffold.app_bar.back_pressed.connect(move || back_out.emit());
        let action_out = scaffold.action_pressed.clone();
        let _ = scaffold.app_bar.action_pressed.connect(move || action_out.emit());
        // The trailing action signals are mirrored too, so a scaffold embedder observes the bar's
        // full capability rather than only its legacy two affordances (N-08).
        let index_out = scaffold.action_index_pressed.clone();
        let _ = scaffold.app_bar.action_index_pressed.connect(move |index| index_out.emit(*index));
        let overflow_out = scaffold.action_overflow_pressed.clone();
        let _ = scaffold.app_bar.action_overflow_pressed.connect(move || overflow_out.emit());
        scaffold
    }

    /// Sets the title displayed in the AppBar.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.app_bar.set_title(title);
        // The AppBar is painted by this scaffold's own `draw`, not mounted through `add_child`, so
        // its redraw request lands on a base nobody paints. The scaffold must repaint itself
        // (G-A06).
        self.base.request_redraw();
    }

    /// Returns the current title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets whether the AppBar back button is shown.
    pub fn set_show_back(&mut self, show_back: bool) {
        self.app_bar.set_show_back(show_back);
        self.base.request_redraw();
    }

    /// Returns whether the AppBar back button is shown.
    pub fn show_back(&self) -> bool {
        self.app_bar.show_back()
    }

    /// Sets the action text displayed on the right side of the AppBar.
    pub fn set_action_text(&mut self, text: &str) {
        self.app_bar.set_action_text(text);
        self.base.request_redraw();
    }

    /// Returns the current AppBar action text.
    pub fn action_text(&self) -> &str {
        self.app_bar.action_text()
    }

    /// Sets the AppBar's trailing action list (N-08).
    pub fn set_actions(&mut self, actions: Vec<String>) {
        self.app_bar.set_actions(actions);
        self.base.request_redraw();
    }

    /// Returns the AppBar's trailing action labels.
    pub fn actions(&self) -> &[String] {
        self.app_bar.actions()
    }

    /// Sets the AppBar's writing direction (N-08).
    pub fn set_direction(&mut self, direction: TextDirection) {
        self.app_bar.set_direction(direction);
        self.base.request_redraw();
    }

    /// Returns the AppBar's writing direction.
    pub fn direction(&self) -> TextDirection {
        self.app_bar.direction()
    }

    /// Sets the AppBar's bottom slot (N-08).
    pub fn set_bottom_widget(&mut self, widget: Option<Box<dyn Widget>>) {
        self.app_bar.set_bottom_widget(widget);
        self.base.request_redraw();
    }

    /// Fills the AppBar's bottom slot with an already-mounted control (issue-20261104 DM-11).
    ///
    /// The app bar's bottom slot was the last content position a document could not reach: every
    /// other slot on this control has an id setter, and this one only had the owning `Box` one.
    pub fn set_app_bar_bottom_id(&mut self, widget: Option<ObjectId>) {
        self.app_bar.set_bottom_id(widget);
        // The slot changes the bar's height, which moves the content band and every child
        // band derived from it, so a repaint alone would leave the old layout in place.
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The AppBar's bottom slot when a declaration filled it (DM-11).
    pub fn app_bar_bottom_id(&self) -> Option<ObjectId> {
        self.app_bar.bottom_id()
    }

    /// Returns the AppBar's bottom slot.
    pub fn bottom_widget(&self) -> Option<&dyn Widget> {
        self.app_bar.bottom_widget()
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
    /// away and back returns to the same widget rather than rebuilding it. The page is stored at
    /// exactly `index`, matching [`Self::set_nav_body_id`]: a page assigned before its destination
    /// exists must not be clamped onto destination 0.
    pub fn set_nav_body(&mut self, index: usize, widget: Option<Box<dyn Widget>>) {
        if self.nav_bodies.len() <= index {
            self.nav_bodies.resize_with(index + 1, || None);
        }
        self.nav_bodies[index] = widget;
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
    ///
    /// Setting a control does **not** show it: a `MaterialSnackbar` starts hidden and is made
    /// visible through [`Self::show_snackbar`], which keeps the control's own duration and dismiss
    /// semantics in charge.
    pub fn set_snackbar(&mut self, widget: Option<Box<dyn Widget>>) {
        self.snackbar = widget;
        self.base.request_redraw();
    }

    /// Returns the snackbar slot, if one is set.
    pub fn snackbar(&self) -> Option<&dyn Widget> {
        self.snackbar.as_deref()
    }

    /// Shows the snackbar slot.
    ///
    /// A `MaterialSnackbar` is shown through its own `show`, so its duration and queued
    /// messages keep working. Any other widget is shown through its base visibility. Returns
    /// `false` when the slot is empty or an id slot cannot be reached.
    pub fn show_snackbar(&mut self) -> bool {
        if let Some(widget) = self.snackbar.as_deref_mut() {
            if let Some(snackbar) =
                crate::widget::capability::widget_as_mut::<crate::widget::MaterialSnackbar>(widget)
            {
                snackbar.show();
            } else {
                widget.base_mut().show();
            }
            self.base.request_redraw();
            return true;
        }
        if let Some(id) = self.snackbar_id {
            let shown = crate::widget::runtime::with_widget_mut(id, |widget| {
                if let Some(snackbar) = crate::widget::capability::widget_as_mut::<
                    crate::widget::MaterialSnackbar,
                >(widget)
                {
                    snackbar.show();
                } else {
                    widget.base_mut().show();
                }
            })
            .is_some();
            if shown {
                self.base.request_redraw();
            }
            return shown;
        }
        false
    }

    /// Dismisses the snackbar slot immediately.
    ///
    /// A `MaterialSnackbar` is dismissed through its own `dismiss`, which emits its `dismissed`
    /// signal; any other widget is hidden. Returns `false` when no slot can be reached.
    pub fn dismiss_snackbar(&mut self) -> bool {
        if let Some(widget) = self.snackbar.as_deref_mut() {
            if let Some(snackbar) =
                crate::widget::capability::widget_as_mut::<crate::widget::MaterialSnackbar>(widget)
            {
                snackbar.dismiss();
            } else {
                widget.base_mut().hide();
            }
            self.base.request_redraw();
            return true;
        }
        if let Some(id) = self.snackbar_id {
            let dismissed = crate::widget::runtime::with_widget_mut(id, |widget| {
                if let Some(snackbar) = crate::widget::capability::widget_as_mut::<
                    crate::widget::MaterialSnackbar,
                >(widget)
                {
                    snackbar.dismiss();
                } else {
                    widget.base_mut().hide();
                }
            })
            .is_some();
            if dismissed {
                self.base.request_redraw();
            }
            return dismissed;
        }
        false
    }

    /// Whether the snackbar slot is currently visible.
    ///
    /// Reports `false` for an empty slot, so a host never mistakes "no control" for "on screen".
    pub fn is_snackbar_visible(&self) -> bool {
        if let Some(widget) = self.snackbar.as_deref() {
            return crate::widget::capability::widget_as::<crate::widget::MaterialSnackbar>(widget)
                .map_or_else(|| widget.base().is_visible(), |snackbar| snackbar.is_visible());
        }
        self.snackbar_id.is_some_and(scaffold_snackbar_visible)
    }

    /// Opens the bottom-sheet slot.
    ///
    /// A `BottomSheet` is opened through its own `open`, so its rise animation and drag handle
    /// keep working; any other widget is shown. Returns `false` when no slot can be reached.
    pub fn show_bottom_sheet(&mut self) -> bool {
        if let Some(widget) = self.bottom_sheet.as_deref_mut() {
            if let Some(sheet) =
                crate::widget::capability::widget_as_mut::<crate::widget::BottomSheet>(widget)
            {
                sheet.open();
            } else {
                widget.base_mut().show();
            }
            self.base.request_redraw();
            return true;
        }
        if let Some(id) = self.bottom_sheet_id {
            let opened = crate::widget::runtime::with_widget_mut(id, |widget| {
                if let Some(sheet) =
                    crate::widget::capability::widget_as_mut::<crate::widget::BottomSheet>(widget)
                {
                    sheet.open();
                } else {
                    widget.base_mut().show();
                }
            })
            .is_some();
            if opened {
                self.base.request_redraw();
            }
            return opened;
        }
        false
    }

    /// Dismisses (closes) the bottom-sheet slot.
    ///
    /// A `BottomSheet` is dismissed through its own `dismiss`, so its rise animation is kept;
    /// any other widget is hidden. Returns `false` when no slot can be reached.
    pub fn dismiss_bottom_sheet(&mut self) -> bool {
        if let Some(widget) = self.bottom_sheet.as_deref_mut() {
            if let Some(sheet) =
                crate::widget::capability::widget_as_mut::<crate::widget::BottomSheet>(widget)
            {
                sheet.dismiss();
            } else {
                widget.base_mut().hide();
            }
            self.base.request_redraw();
            return true;
        }
        if let Some(id) = self.bottom_sheet_id {
            let dismissed = crate::widget::runtime::with_widget_mut(id, |widget| {
                if let Some(sheet) =
                    crate::widget::capability::widget_as_mut::<crate::widget::BottomSheet>(widget)
                {
                    sheet.dismiss();
                } else {
                    widget.base_mut().hide();
                }
            })
            .is_some();
            if dismissed {
                self.base.request_redraw();
            }
            return dismissed;
        }
        false
    }

    /// Whether the bottom-sheet slot is open.
    pub fn is_bottom_sheet_open(&self) -> bool {
        if let Some(widget) = self.bottom_sheet.as_deref() {
            return crate::widget::capability::widget_as::<crate::widget::BottomSheet>(widget)
                .map_or_else(|| widget.base().is_visible(), |sheet| sheet.is_open());
        }
        self.bottom_sheet_id.is_some_and(scaffold_bottom_sheet_open)
    }

    /// Sets the persistent footer slot.
    ///
    /// The footer reserves a band below the body and above the bottom navigation bar, so the
    /// body loses exactly that height. It is not an overlay: the snackbar and the bottom sheet
    /// are laid out above it.
    pub fn set_persistent_footer(&mut self, widget: Option<Box<dyn Widget>>) {
        self.persistent_footer = widget;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns the persistent footer slot, if one is set.
    pub fn persistent_footer(&self) -> Option<&dyn Widget> {
        self.persistent_footer.as_deref()
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

    /// Installs (or removes) the navigation rail used by the medium form.
    ///
    /// The rail is the `MaterialNavigationRail` the host already builds for tablet widths; the
    /// scaffold only gives it the leading band its form owns and paints it there, so the rail's
    /// destinations, indicator and hit tests stay the rail's own. The rail's items are the host's
    /// to populate; the scaffold mirrors the selection both ways, so the rail's indicator follows
    /// the bar (and the reverse) and a press on the rail emits `nav_selected`. The bottom bar keeps
    /// its historical behaviour in the narrow and wide forms. Passing `None` restores the
    /// two-form behaviour exactly.
    pub fn set_navigation_rail(&mut self, rail: Option<MaterialNavigationRail>) {
        self.navigation_rail = rail;
        if let Some(rail) = self.navigation_rail.as_mut() {
            rail.set_selected(self.selected_nav_index);
        }
        self.base.request_redraw();
    }

    /// Returns the installed navigation rail, when there is one.
    pub fn navigation_rail(&self) -> Option<&MaterialNavigationRail> {
        self.navigation_rail.as_ref()
    }

    /// Returns the installed navigation rail for editing, when there is one.
    pub fn navigation_rail_mut(&mut self) -> Option<&mut MaterialNavigationRail> {
        self.navigation_rail.as_mut()
    }

    /// Sets the two width breakpoints that select the navigation form.
    ///
    /// `narrow_max` is the widest window (inclusive) that uses the bottom bar; `wide_min` is the
    /// narrowest window (inclusive) that uses the drawer. Widths between the two use the rail.
    /// A `wide_min` that would leave the medium band empty is raised to `narrow_max + 1`, so the
    /// three forms always partition the width axis.
    pub fn set_navigation_breakpoints(&mut self, narrow_max: u32, wide_min: u32) {
        let wide_min = wide_min.max(narrow_max.saturating_add(1));
        if self.narrow_max == narrow_max && self.wide_min == wide_min {
            return;
        }
        self.narrow_max = narrow_max;
        self.wide_min = wide_min;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns `(narrow_max, wide_min)`, the breakpoints in force.
    pub fn navigation_breakpoints(&self) -> (u32, u32) {
        (self.narrow_max, self.wide_min)
    }

    /// The navigation form a window `width` logical pixels wide uses.
    ///
    /// The bands are: `width <= narrow_max` -> bottom bar, `width >= wide_min` -> drawer, and the
    /// medium gap between them -> rail. This is the pure width lookup; [`Self::navigation_form`]
    /// additionally accounts for whether a rail is actually installed.
    pub fn navigation_form_for(&self, width: u32) -> ScaffoldNavigationForm {
        if width <= self.narrow_max {
            ScaffoldNavigationForm::BottomBar
        } else if width >= self.wide_min {
            ScaffoldNavigationForm::Drawer
        } else {
            ScaffoldNavigationForm::Rail
        }
    }

    /// The navigation form the scaffold paints at its current width.
    ///
    /// A medium width with no rail installed reports [`ScaffoldNavigationForm::BottomBar`]: the
    /// scaffold keeps its historical behaviour rather than claiming a rail it has not got.
    pub fn navigation_form(&self) -> ScaffoldNavigationForm {
        self.effective_navigation_form()
    }

    /// The navigation form after the "no rail installed" fallback.
    fn effective_navigation_form(&self) -> ScaffoldNavigationForm {
        let form = self.navigation_form_for(self.geometry().width);
        if form == ScaffoldNavigationForm::Rail && self.navigation_rail.is_none() {
            ScaffoldNavigationForm::BottomBar
        } else {
            form
        }
    }

    /// The band the navigation rail occupies, when the medium form is active.
    fn navigation_rail_rect(&self) -> Option<Rect> {
        if self.effective_navigation_form() != ScaffoldNavigationForm::Rail {
            return None;
        }
        self.navigation_rail.as_ref()?;
        let rect = self.geometry();
        let top = self.app_bar_height();
        let width = NAVIGATION_RAIL_WIDTH.min(rect.width);
        Some(Rect::new(rect.x, rect.y + top as i32, width, rect.height.saturating_sub(top)))
    }

    /// Whether the bottom bar is the scaffold's active navigation surface.
    ///
    /// A rail in the medium form replaces the bottom bar, so the band, its reserved space and
    /// its hit targets disappear together instead of disagreeing about which form is on screen.
    fn bottom_nav_visible(&self) -> bool {
        self.show_bottom_nav && !self.nav_items.is_empty() && self.navigation_rail_rect().is_none()
    }

    /// Advances the delegated bottom bar, the scaffold's own transitions and the overlay slots.
    ///
    /// The snackbar and bottom sheet are owned as `Box<dyn Widget>`, so the scaffold is their
    /// single animation owner: this is where a snackbar's duration counts down and where a bottom
    /// sheet's rise advances. The draw path paints them but does not also tick them.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let mut moving = self.m3_bar.as_mut().is_some_and(|bar| bar.tick(delta_ms));
        moving |= self.drawer_reveal.tick(delta_ms);
        moving |= self.end_drawer_reveal.tick(delta_ms);
        moving |= self.page_reveal.tick(delta_ms);
        moving |= self.snackbar.as_deref_mut().is_some_and(|widget| widget.tick(delta_ms));
        moving |= self.bottom_sheet.as_deref_mut().is_some_and(|widget| widget.tick(delta_ms));
        moving
    }

    /// Whether anything on the scaffold is between two states.
    pub fn is_animating(&self) -> bool {
        self.m3_bar.as_ref().is_some_and(|bar| bar.is_animating())
            || self.drawer_reveal.is_animating()
            || self.end_drawer_reveal.is_animating()
            || self.page_reveal.is_animating()
            || self.snackbar.as_deref().is_some_and(|widget| widget.is_animating())
            || self.bottom_sheet.as_deref().is_some_and(|widget| widget.is_animating())
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
            // The rail is a second view of the same selection, so a destination chosen through
            // the bottom bar moves the rail's indicator as well.
            if let Some(rail) = self.navigation_rail.as_mut() {
                rail.set_selected(clamped);
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

    /// The height of the AppBar band, including any bottom slot it carries.
    ///
    /// The band is derived from the bar's own slot rather than fixed at [`APP_BAR_HEIGHT`], so
    /// `set_bottom_widget` (N-08) actually reserves room instead of being drawn over by the body.
    /// The slot's *asked* height is used, not `AppBar::bottom_height`: that accessor clamps
    /// against the bar's current rectangle, which is the value this method is computing.
    fn app_bar_height(&self) -> u32 {
        APP_BAR_HEIGHT + self.app_bar.bottom_slot_height()
    }

    /// Whether this scaffold carries a persistent footer, in either content model.
    fn persistent_footer_visible(&self) -> bool {
        self.persistent_footer.is_some() || self.persistent_footer_id.is_some()
    }

    /// The footer band between the body and the bottom navigation bar.
    ///
    /// The band is inset by the navigation rail, exactly like the content band, so a footer and a
    /// rail cannot be painted over each other in the medium form.
    fn persistent_footer_rect(&self) -> Option<Rect> {
        if !self.persistent_footer_visible() {
            return None;
        }
        let rect = self.geometry();
        let rail_inset = self.navigation_rail_rect().map(|rail| rail.width as i32).unwrap_or(0);
        let bottom_offset = if self.bottom_nav_visible() { BOTTOM_NAV_HEIGHT as i32 } else { 0 };
        Some(Rect::new(
            rect.x + rail_inset,
            rect.y + rect.height as i32 - bottom_offset - PERSISTENT_FOOTER_HEIGHT as i32,
            (rect.width as i32 - rail_inset).max(0) as u32,
            PERSISTENT_FOOTER_HEIGHT,
        ))
    }

    /// Returns the content area rect (between AppBar and bottom nav, beside the rail).
    ///
    /// A persistent footer reserves its band from this rect, so the body loses exactly the height
    /// the footer occupies instead of being painted underneath it.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let top_offset: i32 = self.app_bar_height() as i32;
        let rail_inset = self.navigation_rail_rect().map(|rail| rail.width as i32).unwrap_or(0);
        let bottom_offset: i32 =
            if self.bottom_nav_visible() { BOTTOM_NAV_HEIGHT as i32 } else { 0 };
        let footer_offset: i32 =
            if self.persistent_footer_visible() { PERSISTENT_FOOTER_HEIGHT as i32 } else { 0 };
        Rect::new(
            rect.x + rail_inset,
            rect.y + top_offset,
            (rect.width as i32 - rail_inset).max(0) as u32,
            (rect.height as i32 - top_offset - bottom_offset - footer_offset).max(0) as u32,
        )
    }

    /// Returns the bottom nav bar rect.
    fn bottom_nav_rect(&self) -> Option<Rect> {
        if !self.bottom_nav_visible() {
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

    /// The mutable twin of [`Self::active_body`], for event forwarding.
    fn active_body_mut(&mut self) -> Option<&mut dyn Widget> {
        let selected =
            self.nav_bodies.get_mut(self.selected_nav_index).and_then(|page| page.as_deref_mut());
        match selected {
            Some(page) => Some(page),
            None => self.body_widget.as_deref_mut(),
        }
    }

    /// The position an event carries, when it has one.
    ///
    /// One derivation for every pointer, pointer-device and touch variant, so the routing table
    /// does not have to spell the same hit test per event kind.
    fn event_position(event: &Event) -> Option<Point> {
        match event {
            Event::MouseDown((pos, _))
            | Event::MouseUp((pos, _))
            | Event::MouseMoveLegacy((pos, _))
            | Event::MouseMove { pos }
            | Event::MousePress { pos, .. }
            | Event::MouseDoubleClick { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseEnter { pos }
            | Event::MouseLeave { pos }
            | Event::ContextMenu { pos }
            | Event::PointerPress { pos, .. }
            | Event::PointerMove { pos, .. }
            | Event::PointerRelease { pos, .. } => Some(*pos),
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. }
            | Event::TouchEnd { pos, .. }
            | Event::TouchMove { pos, .. }
            | Event::Tap { pos }
            | Event::DoubleTap { pos }
            | Event::LongPress { pos }
            | Event::TwoFingerTap { pos }
            | Event::Drag { pos, .. }
            | Event::Fling { pos, .. } => Some(*pos),
            #[cfg(feature = "touch")]
            Event::Swipe { end, .. } => Some(*end),
            #[cfg(feature = "touch")]
            Event::Pinch { center, .. } => Some(*center),
            #[cfg(feature = "touch")]
            Event::TwoFingerSwipe { centroid_end, .. }
            | Event::MultiFingerSwipe { centroid_end, .. } => Some(*centroid_end),
            #[cfg(feature = "holographic")]
            Event::HolographicTouch { pos, .. } => Some(*pos),
            _ => None,
        }
    }

    /// Routes a positional event to the topmost surface that owns `pos`.
    ///
    /// The overlay slots are visited in reverse paint order, then the app bar, the bottom band
    /// (only when it is the delegated Material3 bar), and finally the active body. `true` means
    /// some surface took it; the caller decides what the base should still observe.
    fn forward_to_pointer_target(&mut self, event: &Event, pos: Point) -> bool {
        let rect = self.geometry();
        if let Some(band) = self.fab_rect() {
            if band.contains_point(pos) {
                if let Some(fab) = self.fab.as_deref_mut() {
                    fab.set_geometry(band);
                    fab.handle_event(event);
                }
                return true;
            }
        }
        if self.snackbar_shown() {
            let band = self.snackbar_rect();
            if band.contains_point(pos) {
                if let Some(snackbar) = self.snackbar.as_deref_mut() {
                    snackbar.set_geometry(band);
                    snackbar.handle_event(event);
                }
                return true;
            }
        }
        if self.bottom_sheet_shown() {
            let band = self.bottom_sheet_rect();
            if band.contains_point(pos) {
                if let Some(sheet) = self.bottom_sheet.as_deref_mut() {
                    sheet.set_geometry(band);
                    sheet.handle_event(event);
                }
                return true;
            }
        }
        if self.end_drawer_open {
            let band = self.end_drawer_rect();
            if band.contains_point(pos) {
                if let Some(panel) = self.end_drawer.as_deref_mut() {
                    panel.set_geometry(band);
                    panel.handle_event(event);
                }
                return true;
            }
        }
        if self.drawer_open {
            let band = self.drawer_rect();
            if band.contains_point(pos) {
                if let Some(panel) = self.drawer.as_deref_mut() {
                    panel.set_geometry(band);
                    panel.handle_event(event);
                }
                return true;
            }
        }

        if let Some(band) = self.navigation_rail_rect() {
            if band.contains_point(pos) {
                if let Some(rail) = self.navigation_rail.as_mut() {
                    rail.set_geometry(band);
                    rail.handle_event(event);
                }
                return true;
            }
        }

        let app_bar_band = Rect::new(rect.x, rect.y, rect.width, self.app_bar_height());
        if app_bar_band.contains_point(pos) {
            self.app_bar.handle_event(event);
            return true;
        }

        if let Some(nav_band) = self.bottom_nav_rect() {
            if nav_band.contains_point(pos) {
                // Only the delegated bar has state to update here; the Classic strip's selection
                // is the scaffold's own and is committed by the press/release arm.
                if let Some(bar) = self.m3_bar.as_mut() {
                    bar.set_geometry(nav_band);
                    bar.handle_event(event);
                }
                return true;
            }
        }

        let content_rect = self.content_rect();
        if content_rect.contains_point(pos) {
            if let Some(body) = self.active_body_mut() {
                body.set_geometry(content_rect);
                body.handle_event(event);
                return true;
            }
        }
        false
    }

    /// Passes an event to the surfaces a focused interaction belongs to: the active page first,
    /// then the app bar (for its own shortcuts), matching the order the draw path stacks them.
    fn forward_to_focused_surfaces(&mut self, event: &Event) {
        let content_rect = self.content_rect();
        if let Some(body) = self.active_body_mut() {
            body.set_geometry(content_rect);
            body.handle_event(event);
        }
        self.app_bar.handle_event(event);
    }

    /// Routes a wheel without a position: an open drawer scrolls its own list; otherwise the
    /// focused surface receives it (a scrollable page is the common case).
    fn forward_wheel(&mut self, event: &Event) {
        if self.drawer_open {
            let band = self.drawer_rect();
            if let Some(panel) = self.drawer.as_deref_mut() {
                panel.set_geometry(band);
                panel.handle_event(event);
                return;
            }
        }
        if self.end_drawer_open {
            let band = self.end_drawer_rect();
            if let Some(panel) = self.end_drawer.as_deref_mut() {
                panel.set_geometry(band);
                panel.handle_event(event);
                return;
            }
        }
        self.forward_to_focused_surfaces(event);
    }

    /// Returns the floating action button's slot rectangle.
    ///
    /// The button sits above every reserved band, including a persistent footer, so it never
    /// covers the footer's content.
    fn fab_rect(&self) -> Option<Rect> {
        self.fab.as_ref()?;
        let rect = self.geometry();
        let bottom_inset = if self.bottom_nav_visible() { BOTTOM_NAV_HEIGHT as i32 } else { 0 };
        let footer_inset =
            if self.persistent_footer_visible() { PERSISTENT_FOOTER_HEIGHT as i32 } else { 0 };
        Some(Rect::new(
            rect.x + rect.width as i32 - FAB_SLOT_SIZE as i32 - FAB_SLOT_MARGIN,
            rect.y + rect.height as i32
                - FAB_SLOT_SIZE as i32
                - bottom_inset
                - footer_inset
                - FAB_SLOT_MARGIN,
            FAB_SLOT_SIZE,
            FAB_SLOT_SIZE,
        ))
    }

    /// Returns the leading drawer's panel rectangle.
    fn drawer_rect(&self) -> Rect {
        let rect = self.geometry();
        let width = DRAWER_WIDTH.min(rect.width);
        let top = self.app_bar_height();
        Rect::new(rect.x, rect.y + top as i32, width, rect.height.saturating_sub(top))
    }

    /// Returns the trailing drawer's panel rectangle.
    fn end_drawer_rect(&self) -> Rect {
        let rect = self.geometry();
        let width = DRAWER_WIDTH.min(rect.width);
        let top = self.app_bar_height();
        Rect::new(
            rect.x + rect.width as i32 - width as i32,
            rect.y + top as i32,
            width,
            rect.height.saturating_sub(top),
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

    /// Whether the snackbar slot should be painted and hit-tested.
    ///
    /// A `MaterialSnackbar` answers through its own visibility; any other widget answers through
    /// its base flag. An empty slot is never shown.
    fn snackbar_shown(&self) -> bool {
        self.snackbar.as_deref().is_some_and(|widget| {
            crate::widget::capability::widget_as::<crate::widget::MaterialSnackbar>(widget)
                .map_or_else(|| widget.base().is_visible(), |snackbar| snackbar.is_visible())
        })
    }

    /// Whether the bottom-sheet slot should be painted and hit-tested.
    ///
    /// A `BottomSheet` answers through `open` plus its drawn rise, so the closing half of a
    /// dismissal remains interactive; any other widget answers through its base flag.
    fn bottom_sheet_shown(&self) -> bool {
        self.bottom_sheet.as_deref().is_some_and(|widget| {
            crate::widget::capability::widget_as::<crate::widget::BottomSheet>(widget).map_or_else(
                || widget.base().is_visible(),
                |sheet| sheet.is_open() || sheet.rise_progress() > 0.0,
            )
        })
    }

    /// Returns the snackbar's rectangle, sitting above the persistent footer and bottom nav band.
    ///
    /// A footer reserves its band from the body, so the overlay is lifted above it as well; the
    /// two cannot overlap.
    fn snackbar_rect(&self) -> Rect {
        let rect = self.geometry();
        let bottom_inset = if self.bottom_nav_visible() { BOTTOM_NAV_HEIGHT as i32 } else { 0 };
        let footer_inset =
            if self.persistent_footer_visible() { PERSISTENT_FOOTER_HEIGHT as i32 } else { 0 };
        Rect::new(
            rect.x + SNACKBAR_MARGIN,
            rect.y + rect.height as i32
                - bottom_inset
                - footer_inset
                - SNACKBAR_HEIGHT as i32
                - SNACKBAR_MARGIN,
            rect.width.saturating_sub((SNACKBAR_MARGIN * 2) as u32),
            SNACKBAR_HEIGHT,
        )
    }
}

/// Builder for an [`AdaptiveScaffold`].
///
/// The builder is a spelling convenience, not a second construction path: [`Self::build`] creates
/// the scaffold with [`AdaptiveScaffold::new`] and applies the existing setters, so a value built
/// this way is indistinguishable from one assembled by hand.
#[derive(Default)]
pub struct AdaptiveScaffoldBuilder {
    title: String,
    geometry: Rect,
    show_back: bool,
    action_text: Option<String>,
    actions: Vec<String>,
    body: Option<Box<dyn Widget>>,
    destinations: Vec<(String, String)>,
    nav_bodies: Vec<(usize, Box<dyn Widget>)>,
    selected_nav_index: usize,
    show_bottom_nav: bool,
    drawer: Option<Box<dyn Widget>>,
    fab: Option<Box<dyn Widget>>,
    snackbar: Option<Box<dyn Widget>>,
    bottom_sheet: Option<Box<dyn Widget>>,
    persistent_footer: Option<Box<dyn Widget>>,
}

impl AdaptiveScaffoldBuilder {
    /// Starts a builder for a scaffold with `title` and `geometry`.
    pub fn new(title: &str, geometry: Rect) -> Self {
        Self { title: title.to_string(), geometry, ..Self::default() }
    }

    /// Sets whether the AppBar back affordance is shown.
    pub fn show_back(mut self, show_back: bool) -> Self {
        self.show_back = show_back;
        self
    }

    /// Sets the AppBar's action text.
    pub fn action_text(mut self, text: &str) -> Self {
        self.action_text = Some(text.to_string());
        self
    }

    /// Sets the AppBar's trailing action list.
    pub fn actions(mut self, actions: Vec<String>) -> Self {
        self.actions = actions;
        self
    }

    /// Sets the shared fallback body.
    pub fn body(mut self, widget: Box<dyn Widget>) -> Self {
        self.body = Some(widget);
        self
    }

    /// Adds one bottom-navigation destination, in declaration order.
    pub fn destination(mut self, icon: &str, label: &str) -> Self {
        self.destinations.push((icon.to_string(), label.to_string()));
        self
    }

    /// Assigns the page shown while destination `index` is selected.
    ///
    /// Pages may be assigned before their destination is declared; [`Self::build`] adds every
    /// destination first, so a page cannot be erased by the item list.
    pub fn nav_body(mut self, index: usize, widget: Box<dyn Widget>) -> Self {
        self.nav_bodies.push((index, widget));
        self
    }

    /// Selects the destination that starts active.
    pub fn selected_nav_index(mut self, index: usize) -> Self {
        self.selected_nav_index = index;
        self
    }

    /// Sets whether the bottom navigation bar is shown.
    pub fn show_bottom_nav(mut self, show: bool) -> Self {
        self.show_bottom_nav = show;
        self
    }

    /// Sets the leading drawer slot.
    pub fn drawer(mut self, widget: Box<dyn Widget>) -> Self {
        self.drawer = Some(widget);
        self
    }

    /// Sets the floating action button slot.
    pub fn fab(mut self, widget: Box<dyn Widget>) -> Self {
        self.fab = Some(widget);
        self
    }

    /// Sets the snackbar slot. The control is not shown until `show_snackbar` is called.
    pub fn snackbar(mut self, widget: Box<dyn Widget>) -> Self {
        self.snackbar = Some(widget);
        self
    }

    /// Sets the bottom-sheet slot. The control is not opened until `show_bottom_sheet` is called.
    pub fn bottom_sheet(mut self, widget: Box<dyn Widget>) -> Self {
        self.bottom_sheet = Some(widget);
        self
    }

    /// Sets the persistent footer slot.
    pub fn persistent_footer(mut self, widget: Box<dyn Widget>) -> Self {
        self.persistent_footer = Some(widget);
        self
    }

    /// Builds the scaffold through [`AdaptiveScaffold::new`] and its existing setters.
    pub fn build(self) -> AdaptiveScaffold {
        let mut scaffold = AdaptiveScaffold::new(&self.title, self.geometry);
        scaffold.set_show_back(self.show_back);
        if let Some(text) = self.action_text {
            scaffold.set_action_text(&text);
        }
        if !self.actions.is_empty() {
            scaffold.set_actions(self.actions);
        }
        if let Some(body) = self.body {
            scaffold.set_body_widget(Some(body));
        }
        // Destinations first: `add_nav_item` pushes an empty page slot, so a page assigned before
        // its destination would otherwise be replaced.
        for (icon, label) in self.destinations {
            scaffold.add_nav_item(&icon, &label);
        }
        for (index, body) in self.nav_bodies {
            scaffold.set_nav_body(index, Some(body));
        }
        scaffold.set_selected_nav_index(self.selected_nav_index);
        scaffold.set_show_bottom_nav(self.show_bottom_nav);
        if let Some(drawer) = self.drawer {
            scaffold.set_drawer(Some(drawer));
        }
        if let Some(fab) = self.fab {
            scaffold.set_fab(Some(fab));
        }
        if let Some(snackbar) = self.snackbar {
            scaffold.set_snackbar(Some(snackbar));
        }
        if let Some(bottom_sheet) = self.bottom_sheet {
            scaffold.set_bottom_sheet(Some(bottom_sheet));
        }
        if let Some(footer) = self.persistent_footer {
            scaffold.set_persistent_footer(Some(footer));
        }
        scaffold
    }
}

impl Widget for AdaptiveScaffold {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    /// The scaffold routes the wheel to the pane under the pointer (C2-D0).
    fn accepts_wheel(&self) -> bool {
        true
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
            "show_bottom_nav" => Ok(CapabilityValue::Bool(self.show_bottom_nav())),
            "bottom_nav_style" => {
                Ok(CapabilityValue::String(self.bottom_nav_style().token().to_string()))
            }
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
            "show_bottom_nav" => {
                self.set_show_bottom_nav(expect_bool(value)?);
                Ok(())
            }
            "bottom_nav_style" => {
                let token = expect_string(value)?;
                let style = ScaffoldBottomNavStyle::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_bottom_nav_style(style);
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
            "show_bottom_nav",
            "bottom_nav_style",
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
        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // Declared (id-based) content is placed before anything is painted: its children are
        // drawn by the tree painter after this control's own draw, so the band and visibility
        // computed here are what that pass reads in the same frame. See `sync_id_slots`.
        self.sync_id_slots();

        // ── AppBar ──
        // The bar carries any bottom slot it was given, so its band is set from the same
        // derivation `content_rect` uses (N-08).
        self.app_bar.set_geometry(Rect::new(rect.x, rect.y, rect.width, self.app_bar_height()));
        self.app_bar.draw(context);

        // ── Navigation rail (medium form) ──
        //
        // The rail is the widget the host installed, not a copy of its arithmetic: the scaffold
        // gives it the leading band and paints it. The content band is inset by the same width
        // (`content_rect`), so the rail and the page never overlap.
        if let Some(band) = self.navigation_rail_rect() {
            if let Some(rail) = self.navigation_rail.as_mut() {
                rail.set_geometry(band);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(rail) {
                    drawable.draw(context);
                }
            }
        }

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

        // ── Persistent footer ──
        //
        // The footer owns the band `content_rect` already removed from the body, so painting it
        // here cannot cover the page. It is drawn before the bottom navigation bar because the
        // footer sits above that bar.
        if let Some(band) = self.persistent_footer_rect() {
            if let Some(footer) = self.persistent_footer.as_deref_mut() {
                footer.set_geometry(band);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(footer) {
                    drawable.draw(context);
                }
            }
        }

        // ── Bottom navigation bar ──
        //
        // The rect is asked for once and the draw is guarded on the answer, so "is the nav bar
        // shown" is decided in **one** place.
        if let Some(nav_rect) = self.bottom_nav_rect() {
            match self.bottom_nav_style {
                ScaffoldBottomNavStyle::Classic => {
                    self.draw_bottom_nav(context, nav_rect, style, theme.as_deref())
                }
                ScaffoldBottomNavStyle::Material3 => {
                    // A rail in the medium form hides the bar, and the delegated bar is rebuilt
                    // from the scaffold's items when the window returns to a form that shows it.
                    if self.m3_bar.is_none() {
                        self.sync_m3_bar();
                    }
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
        // A bottom sheet paints only while it is open or still sliding, so `set_bottom_sheet` is
        // not the same as "drawn forever": the control's own `open`/`dismiss` state decides.
        if self.bottom_sheet_shown() {
            let band = self.bottom_sheet_rect();
            if let Some(sheet) = self.bottom_sheet.as_deref_mut() {
                sheet.set_geometry(band);
                // The scaffold ticks this slot in `tick`, so the paint path must not advance it a
                // second time in the same frame.
                if let Some(drawable) = sheet.as_draw_mut() {
                    drawable.draw(context);
                }
            }
        }
        // A snackbar is visible only while its own show/duration state says so; a freshly set
        // `MaterialSnackbar` is hidden and paints nothing.
        if self.snackbar_shown() {
            let band = self.snackbar_rect();
            if let Some(snackbar) = self.snackbar.as_deref_mut() {
                snackbar.set_geometry(band);
                if let Some(drawable) = snackbar.as_draw_mut() {
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
    ///
    /// The resolved style is passed in from the scaffold's own draw entry: resolving the same
    /// role a second time here was a second manager read per frame (G-A01).
    fn draw_bottom_nav(
        &self,
        context: &mut RenderContext,
        nav_rect: Rect,
        style: &crate::style::WidgetStyle,
        theme: Option<&crate::style::WidgetStyle>,
    ) {
        let item_count = self.nav_items.len();
        if item_count == 0 {
            return;
        }

        let tab_width = nav_rect.width / item_count as u32;

        // The bar draws with the scaffold's own resolved chrome, so the whole control
        // moves together on a theme switch rather than the bar keeping a literal white.
        let bar_background = style
            .background_color
            .or_else(|| theme.and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let bar_ink =
            style.text_color.or_else(|| theme.and_then(|t| t.text_color)).unwrap_or(Color::BLACK);
        let bar_border = style
            .border_color
            .or_else(|| theme.and_then(|t| t.border_color))
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
            // The shared measure cache, the same accessor `BottomNavigationBar` uses for its
            // own strip. Direct `context.measure_text` bypassed it, so the Classic bar
            // re-shaped every caption and badge on every frame (N-007).
            let mut icon_metrics = crate::widget::base_widgets::measure_cache::measure_text(
                context,
                if typed_source.is_some() { "M" } else { &item.icon },
                &icon_font,
            );
            if typed_source.is_some() {
                icon_metrics.width = icon_metrics.height.max(1);
            }
            let label_metrics = crate::widget::base_widgets::measure_cache::measure_text(
                context,
                &item.label,
                &label_font,
            );

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
                            let metrics = crate::widget::base_widgets::measure_cache::measure_text(
                                context,
                                badge,
                                &label_font,
                            );
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
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button, .. } => {
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

                // A modal drawer's scrim swallows a press outside the panel and closes the drawer,
                // the same contract the standalone `NavigationDrawer` follows (N-07). The release
                // that follows is consumed too, so the gesture cannot leak to the bar underneath.
                if self.scrim_pressed {
                    if matches!(event, Event::MouseRelease { .. }) {
                        self.scrim_pressed = false;
                    }
                    return;
                }
                if self.drawer_open
                    && self.drawer_reveal.value() > 0.0
                    && !self.drawer_rect().contains_point(*pos)
                {
                    self.close_drawer();
                    self.scrim_pressed = true;
                    return;
                }
                if self.end_drawer_open
                    && self.end_drawer_reveal.value() > 0.0
                    && !self.end_drawer_rect().contains_point(*pos)
                {
                    self.close_end_drawer();
                    self.scrim_pressed = true;
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
                if !routed && self.snackbar_shown() {
                    let band = self.snackbar_rect();
                    if band.contains_point(*pos) {
                        if let Some(snackbar) = self.snackbar.as_deref_mut() {
                            snackbar.set_geometry(band);
                            snackbar.handle_event(event);
                            routed = true;
                        }
                    }
                }
                if !routed && self.bottom_sheet_shown() {
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
                let app_bar_height: i32 = self.app_bar_height() as i32;

                // Navigation rail zone (medium form). The rail owns its destination bands and
                // updates its own selection on release; the scaffold adopts that selection so
                // `nav_selected` reports a rail choice exactly as it reports a bar choice.
                if let Some(band) = self.navigation_rail_rect() {
                    if band.contains_point(*pos) {
                        let selected = self.navigation_rail.as_mut().map(|rail| {
                            rail.set_geometry(band);
                            rail.handle_event(event);
                            rail.selected()
                        });
                        if let Some(index) = selected {
                            if matches!(event, Event::MouseRelease { .. }) {
                                self.set_selected_nav_index(index);
                            }
                        }
                        return;
                    }
                }

                // AppBar zone (top)
                if pos.y >= rect.y && pos.y < rect.y + app_bar_height {
                    // The AppBar's signals are mirrored onto this scaffold at construction,
                    // so forwarding the event is all this arm has to do.
                    self.app_bar.handle_event(event);
                    return;
                }

                // Bottom nav zone
                if self.bottom_nav_visible() {
                    let nav_top = rect.y + rect.height as i32 - BOTTOM_NAV_HEIGHT as i32;
                    if pos.y >= nav_top && pos.y <= rect.y + rect.height as i32 {
                        // Hit-test bottom nav items
                        let item_count = self.nav_items.len();
                        let tab_width = rect.width / item_count as u32;
                        // A bar narrower than its item count has no usable tab: the division
                        // below would panic, and no point can name an item anyway.
                        if tab_width == 0 {
                            return;
                        }
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
            // Escape closes an open drawer, the same key contract `SplashScreen` and the
            // pagination popup already follow (N-11).
            Event::KeyPress { key, .. }
                if *key == key_codes::ESCAPE && (self.drawer_open || self.end_drawer_open) =>
            {
                if self.drawer_open {
                    self.close_drawer();
                }
                if self.end_drawer_open {
                    self.close_end_drawer();
                }
            }
            // A pointer move is routed to whichever surface owns it, so overlays and the app
            // bar receive hover at the same coordinates the draw path paints them at. The
            // previous default dropped these entirely, which is why only the scaffold's own
            // base ever saw a hover.
            Event::MouseMove { pos } => {
                self.forward_to_pointer_target(event, *pos);
                self.base.handle_event(event);
            }
            Event::MouseEnter { pos } | Event::MouseLeave { pos } => {
                self.forward_to_pointer_target(event, *pos);
                self.base.handle_event(event);
            }
            Event::MouseDoubleClick { pos, .. } | Event::ContextMenu { pos } => {
                self.forward_to_pointer_target(event, *pos);
                self.base.handle_event(event);
            }
            Event::Wheel { .. } => {
                self.forward_wheel(event);
                // C2-D0: the scaffold owns the wheel over its panes; forwarding is its own
                // handling, so it claims the event rather than letting the router walk past it.
                crate::widget::runtime::claim_wheel();
                self.base.handle_event(event);
            }
            // Legacy pointer and stylus events carry a position too, so they follow the same
            // routing as the modern pointer variants.
            Event::MouseDown(_)
            | Event::MouseUp(_)
            | Event::MouseMoveLegacy(_)
            | Event::PointerPress { .. }
            | Event::PointerMove { .. }
            | Event::PointerRelease { .. } => {
                if let Some(pos) = Self::event_position(event) {
                    self.forward_to_pointer_target(event, pos);
                }
                self.base.handle_event(event);
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. }
            | Event::TouchEnd { .. }
            | Event::TouchMove { .. }
            | Event::Tap { .. }
            | Event::DoubleTap { .. }
            | Event::LongPress { .. }
            | Event::Swipe { .. }
            | Event::Pinch { .. }
            | Event::Rotate { .. }
            | Event::Drag { .. }
            | Event::TwoFingerTap { .. }
            | Event::TwoFingerSwipe { .. }
            | Event::MultiFingerSwipe { .. }
            | Event::Fling { .. } => {
                if let Some(pos) = Self::event_position(event) {
                    self.forward_to_pointer_target(event, pos);
                } else {
                    self.forward_to_focused_surfaces(event);
                }
                self.base.handle_event(event);
            }
            #[cfg(feature = "holographic")]
            Event::HolographicTouch { .. } => {
                if let Some(pos) = Self::event_position(event) {
                    self.forward_to_pointer_target(event, pos);
                }
                self.base.handle_event(event);
            }
            // Keyboard, text and IME traffic belongs to the focused surface. The app bar sees
            // it as well because its own affordances may publish shortcuts.
            Event::KeyPress { .. }
            | Event::KeyRelease { .. }
            | Event::KeyDown(_)
            | Event::KeyUp(_)
            | Event::TextInput { .. }
            | Event::ImePreedit { .. }
            | Event::ImeCommit { .. } => {
                self.forward_to_focused_surfaces(event);
                self.base.handle_event(event);
            }
            // Everything else is passed through rather than swallowed: the active page and the
            // app bar still observe it, and only then does the base. This replaces the old
            // default that sent the event to `base` alone, which made the scaffold a hole in
            // the event surface for every kind it did not name.
            _ => {
                self.forward_to_focused_surfaces(event);
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::render::{
        PaintBackend, RenderCommand, ShapedText, SoftwarePaintBackend, SoftwareRenderConfig,
        TextMetrics,
    };
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

    /// The AppBar's bottom slot used to overlap the body: the bar was laid out at the fixed
    /// `APP_BAR_HEIGHT` band while a slot asked for its own height, so pins/tabs were painted
    /// over the page. The derived band must push `content_rect` down by the slot height (N-08).
    #[test]
    fn adaptive_scaffold_bottom_slot_reserves_room() {
        use crate::widget::base_widgets::frame::Frame;
        use crate::widget::metrics::dimensions;

        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        scaffold.set_bottom_widget(Some(Box::new(Frame::new(Rect::new(0, 0, 375, 0)))));

        let slot = dimensions::FRAME_DEFAULT_HEIGHT;
        let content = scaffold.content_rect();
        assert_eq!(content.y, (APP_BAR_HEIGHT + slot) as i32, "the slot band precedes the body");
        assert_eq!(content.height, 812 - APP_BAR_HEIGHT - slot);

        // The same band reaches the bar itself, so the slot is painted inside the reserved area.
        let _ = render_to_svg(&mut scaffold);
        assert_eq!(scaffold.app_bar.geometry().height, APP_BAR_HEIGHT + slot);

        scaffold.set_bottom_widget(None);
        assert_eq!(scaffold.content_rect(), Rect::new(0, 56, 375, 756));
    }

    /// A **declared** bottom slot (an id) must reserve room exactly like the owned one.
    ///
    /// The scaffold used to add only the `Box` slot's hint, so an id slot left the bar at
    /// 56 px; `AppBar::bottom_height` then saw `room == 0` and never placed the child. The
    /// declared slot is now measured through `bottom_slot_height`, and the bar's own draw
    /// path places it inside the reserved band.
    #[test]
    fn a_declared_app_bar_bottom_slot_reserves_room() {
        use crate::widget::base_widgets::frame::Frame;
        use crate::widget::metrics::dimensions;

        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        let slot = crate::widget::runtime::register(Box::new(Frame::new(Rect::new(0, 0, 375, 0))))
            .expect("the runtime mounts the declared slot");

        scaffold.set_app_bar_bottom_id(Some(slot));
        let height = dimensions::FRAME_DEFAULT_HEIGHT;
        assert_eq!(
            scaffold.content_rect().y,
            (APP_BAR_HEIGHT + height) as i32,
            "the declared slot must push the body down"
        );

        let _ = render_to_svg(&mut scaffold);
        assert_eq!(scaffold.app_bar.geometry().height, APP_BAR_HEIGHT + height);
        assert_eq!(
            crate::widget::runtime::geometry_of(slot).map(|band| band.y),
            Some(APP_BAR_HEIGHT as i32),
            "the declared slot is placed inside the reserved band"
        );
        assert_eq!(
            crate::widget::runtime::with_widget(slot, |widget| widget.is_visible()),
            Some(true),
            "a placed declared slot is shown"
        );

        let _ = crate::widget::runtime::unregister(slot);
    }

    /// A page assigned before its destination exists belongs to that destination, not slot 0.
    #[test]
    fn a_page_assigned_ahead_of_its_destination_is_stored_at_that_index() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        scaffold.add_nav_item("★", "Favorites");

        scaffold.set_nav_body(
            1,
            Some(Box::new(crate::widget::base_widgets::label::Label::new(
                "Second".to_string(),
                Rect::new(0, 0, 10, 10),
            ))),
        );
        assert!(scaffold.nav_body(1).is_some(), "the page belongs to destination 1");
        assert!(
            scaffold.nav_body(0).is_none(),
            "destination 0 must not inherit destination 1's page"
        );
    }

    /// A bar narrower than its item count has no tab width; pressing it must not divide by zero.
    #[test]
    fn a_narrow_scaffold_bottom_bar_does_not_divide_by_zero() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 3, 112));
        scaffold.add_nav_item("★", "One");
        scaffold.add_nav_item("✉", "Two");
        scaffold.add_nav_item("⌂", "Three");
        scaffold.add_nav_item("⚙", "Four");
        scaffold.set_show_bottom_nav(true);

        // The bottom band is y 56..112; `3 / 4` truncates to a zero tab width.
        scaffold.handle_event(&Event::MousePress {
            pos: Point::new(1, 80),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(scaffold.selected_nav_index(), 0, "no point can name a tab in a zero-width tab");
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

        scaffold.handle_event(&Event::MouseRelease {
            pos: Point::new(10, 28),
            button: 1,
            modifiers: 0,
        });
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

        scaffold.handle_event(&Event::MouseRelease {
            pos: Point::new(340, 28),
            button: 1,
            modifiers: 0,
        });
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

    fn drawer_widget() -> Box<dyn Widget> {
        Box::new(crate::widget::base_widgets::label::Label::new(
            "drawer".to_string(),
            Rect::new(0, 0, 10, 10),
        ))
    }

    /// A widget that records how many times the scaffold painted it.
    struct DrawCounter {
        base: BaseWidget,
        draws: Arc<AtomicUsize>,
    }

    impl DrawCounter {
        fn new(draws: Arc<AtomicUsize>) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, Rect::new(0, 0, 10, 10), "DrawCounter"),
                draws,
            }
        }
    }

    impl Widget for DrawCounter {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        impl_draw_bridge!();
    }

    impl EventHandler for DrawCounter {
        fn handle_event(&mut self, _event: &Event) {}
    }

    impl Draw for DrawCounter {
        fn draw(&mut self, context: &mut RenderContext) {
            self.draws.fetch_add(1, Ordering::SeqCst);
            context.fill_rect(self.geometry(), Color::BLACK);
        }
    }

    /// A snackbar's own duration dismisses it and the slot reports hidden.
    #[test]
    fn a_snackbar_auto_dismisses_through_the_scaffold_slot() {
        let mut scaffold = make_scaffold();
        let mut snackbar = crate::widget::MaterialSnackbar::new(Rect::new(0, 0, 300, 48));
        snackbar.set_message("Saved");
        snackbar.set_duration_ms(50);
        scaffold.set_snackbar(Some(Box::new(snackbar)));

        assert!(
            !scaffold.is_snackbar_visible(),
            "setting a snackbar must not force it permanently visible"
        );
        assert!(scaffold.show_snackbar(), "the slot accepts show");
        assert!(scaffold.is_snackbar_visible(), "a shown snackbar is visible");

        while scaffold.tick(16) {}
        assert!(!scaffold.is_snackbar_visible(), "the snackbar's own duration must dismiss it");
    }

    /// An explicit dismiss hides the snackbar before its duration elapses.
    #[test]
    fn a_snackbar_dismisses_on_demand() {
        let mut scaffold = make_scaffold();
        let mut snackbar = crate::widget::MaterialSnackbar::new(Rect::new(0, 0, 300, 48));
        snackbar.set_duration_ms(0);
        scaffold.set_snackbar(Some(Box::new(snackbar)));
        assert!(scaffold.show_snackbar());
        assert!(scaffold.is_snackbar_visible());

        assert!(scaffold.dismiss_snackbar(), "the slot accepts dismiss");
        assert!(!scaffold.is_snackbar_visible(), "an explicit dismiss hides it");
    }

    /// The bottom-sheet slot follows the control's open/dismiss state.
    #[test]
    fn a_bottom_sheet_opens_and_dismisses_through_the_slot() {
        let mut scaffold = make_scaffold();
        scaffold.set_bottom_sheet(Some(Box::new(crate::widget::BottomSheet::new(Rect::new(
            0, 0, 375, 400,
        )))));
        assert!(!scaffold.is_bottom_sheet_open(), "a freshly set bottom sheet is closed");
        assert!(scaffold.show_bottom_sheet());
        assert!(scaffold.is_bottom_sheet_open());
        assert!(scaffold.dismiss_bottom_sheet());
        assert!(!scaffold.is_bottom_sheet_open());
    }

    /// A hidden slot paints nothing, and an unset slot has nothing to paint.
    #[test]
    fn a_hidden_or_unset_overlay_slot_paints_nothing() {
        let mut scaffold = make_scaffold();
        let draws = Arc::new(AtomicUsize::new(0));
        let mut counter = DrawCounter::new(Arc::clone(&draws));
        counter.base_mut().hide();
        scaffold.set_snackbar(Some(Box::new(counter)));

        let _ = render_to_svg(&mut scaffold);
        assert_eq!(draws.load(Ordering::SeqCst), 0, "a hidden slot is not painted");

        scaffold.set_snackbar(None);
        let _ = render_to_svg(&mut scaffold);
        assert_eq!(draws.load(Ordering::SeqCst), 0, "an unset slot is not painted");
    }

    /// The footer takes its height from the body and sits above the bottom bar.
    #[test]
    fn a_persistent_footer_shortens_the_body_and_stays_above_the_nav_bar() {
        let mut scaffold = make_scaffold();
        scaffold.set_show_bottom_nav(true);
        let body_without_footer = scaffold.content_rect();

        scaffold.set_persistent_footer(Some(drawer_widget()));
        let body_with_footer = scaffold.content_rect();
        assert_eq!(
            body_without_footer.height - body_with_footer.height,
            PERSISTENT_FOOTER_HEIGHT,
            "the footer reserves exactly its band from the body"
        );

        let footer = scaffold.persistent_footer_rect().expect("a footer band exists");
        let nav = scaffold.bottom_nav_rect().expect("the nav bar is shown");
        assert_eq!(
            footer.y + footer.height as i32,
            nav.y,
            "the footer ends where the nav bar begins"
        );
        assert_eq!(
            body_with_footer.y + body_with_footer.height as i32,
            footer.y,
            "the body ends where the footer begins"
        );
        let snackbar = scaffold.snackbar_rect();
        assert!(
            snackbar.y + snackbar.height as i32 <= footer.y,
            "the snackbar overlay must not overlap the footer"
        );
    }

    /// The builder uses the existing constructor and setters.
    #[test]
    fn the_builder_assembles_the_same_scaffold() {
        let scaffold = AdaptiveScaffoldBuilder::new("Home", Rect::new(0, 0, 375, 812))
            .show_back(true)
            .action_text("Save")
            .actions(vec!["One".to_string()])
            .body(Box::new(crate::widget::base_widgets::label::Label::new(
                "body".to_string(),
                Rect::new(0, 0, 10, 10),
            )))
            .destination("★", "Favorites")
            .destination("✉", "Messages")
            .nav_body(
                1,
                Box::new(crate::widget::base_widgets::label::Label::new(
                    "second".to_string(),
                    Rect::new(0, 0, 10, 10),
                )),
            )
            .selected_nav_index(1)
            .show_bottom_nav(true)
            .drawer(drawer_widget())
            .fab(drawer_widget())
            .snackbar(drawer_widget())
            .bottom_sheet(drawer_widget())
            .persistent_footer(drawer_widget())
            .build();

        assert_eq!(scaffold.title(), "Home");
        assert!(scaffold.show_back());
        assert_eq!(scaffold.action_text(), "Save");
        assert_eq!(scaffold.nav_item_count(), 2);
        assert_eq!(scaffold.selected_nav_index(), 1);
        assert!(scaffold.show_bottom_nav());
        assert!(scaffold.body_widget().is_some());
        assert!(scaffold.nav_body(1).is_some());
        assert!(scaffold.drawer().is_some());
        assert!(scaffold.fab().is_some());
        assert!(scaffold.snackbar().is_some());
        assert!(scaffold.bottom_sheet().is_some());
        assert!(scaffold.persistent_footer().is_some());
    }

    #[test]
    fn escape_closes_an_open_drawer() {
        let mut scaffold = make_scaffold();
        scaffold.set_drawer(Some(drawer_widget()));
        assert!(scaffold.open_drawer());
        while scaffold.tick(16) {}
        scaffold.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });
        assert!(!scaffold.is_drawer_open(), "Escape is the keyboard close");
    }

    #[test]
    fn a_press_on_the_scrim_closes_the_drawer() {
        let mut scaffold = make_scaffold();
        scaffold.set_drawer(Some(drawer_widget()));
        assert!(scaffold.open_drawer());
        while scaffold.tick(16) {}
        // The leading drawer is 280 px wide; x = 360 is on the scrim.
        scaffold.handle_event(&Event::MousePress {
            pos: Point::new(360, 400),
            button: 1,
            modifiers: 0,
        });
        assert!(!scaffold.is_drawer_open(), "the modal scrim dismisses on press");
        scaffold.handle_event(&Event::MouseRelease {
            pos: Point::new(360, 400),
            button: 1,
            modifiers: 0,
        });
    }

    /// A body that reports every event it is given.
    struct EventSink {
        base: BaseWidget,
        count: Arc<AtomicUsize>,
    }

    impl EventSink {
        fn new(count: Arc<AtomicUsize>) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, Rect::new(0, 0, 1, 1), "EventSink"),
                count,
            }
        }
    }

    impl Widget for EventSink {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        impl_draw_bridge!();
    }

    impl Draw for EventSink {
        fn draw(&mut self, context: &mut RenderContext) {
            context.fill_rect(self.geometry(), Color::BLACK);
        }
    }

    impl EventHandler for EventSink {
        fn handle_event(&mut self, _event: &Event) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Pointer events are routed to the surface that owns the position, not swallowed.
    #[test]
    fn a_hover_reaches_the_app_bar_band() {
        let mut scaffold = make_scaffold();
        assert!(!scaffold.app_bar.base().is_hovered());
        scaffold.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(
            scaffold.app_bar.base().is_hovered(),
            "the app bar owns the top band and must see the enter"
        );
    }

    /// Events the scaffold has no arm for still reach the active page.
    #[test]
    fn unhandled_events_pass_through_to_the_active_page() {
        let mut scaffold = make_scaffold();
        let count = Arc::new(AtomicUsize::new(0));
        scaffold.set_body_widget(Some(Box::new(EventSink::new(Arc::clone(&count)))));
        scaffold.handle_event(&Event::Timer { id: 7 });
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "the old default sent the event to the base alone"
        );
    }

    /// A backend that counts how many times text is measured.
    struct CountingBackend {
        inner: SoftwarePaintBackend,
        measure_calls: Arc<AtomicUsize>,
    }

    impl PaintBackend for CountingBackend {
        fn begin_frame(&mut self, clear: Color) {
            self.inner.begin_frame(clear);
        }
        fn end_frame(&mut self) {
            self.inner.end_frame();
        }
        fn execute_command(&mut self, command: &RenderCommand) {
            self.inner.execute_command(command);
        }
        fn size(&self) -> crate::core::Size {
            self.inner.size()
        }
        fn set_size(&mut self, size: crate::core::Size) {
            self.inner.set_size(size);
        }
        fn dpi_scale(&self) -> f32 {
            self.inner.dpi_scale()
        }
        fn set_dpi_scale(&mut self, scale: f32) {
            self.inner.set_dpi_scale(scale);
        }
        fn measure_text(&self, text: &str, font: &crate::core::Font) -> TextMetrics {
            self.measure_calls.fetch_add(1, Ordering::SeqCst);
            self.inner.measure_text(text, font)
        }
        fn shape_text(
            &self,
            text: &str,
            font: &crate::core::Font,
        ) -> crate::compat::Rc<ShapedText> {
            self.inner.shape_text(text, font)
        }
        fn frame_rgba(&self) -> &[u8] {
            self.inner.frame_rgba()
        }
        fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
            self.inner.apply_render_config(config);
        }
    }

    /// The Classic strip measures through the shared cache instead of the backend.
    ///
    /// Regression (N-007): it called `context.measure_text` directly for the icon, the label
    /// and the badge, so every frame re-shaped the same fixed captions.
    #[test]
    fn the_classic_bar_uses_the_shared_measure_cache() {
        let _guard = crate::style::theme_test_guard();
        let scaffold = make_scaffold();
        let style = crate::style::WidgetStyle::default();
        let nav_rect = scaffold.bottom_nav_rect().expect("the bar is shown");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut backend = CountingBackend {
            inner: SoftwarePaintBackend::new(crate::core::Size::new(375, 812), 1.0),
            measure_calls: Arc::clone(&calls),
        };
        // Warm the cache on the bar's own paint path, then measure a settled frame's work.
        // The scaffold's AppBar is deliberately excluded: its fitted title asks the backend
        // for a fit check, which is a different concern from the strip's per-item layout.
        {
            let mut context = RenderContext::new(&mut backend);
            scaffold.draw_bottom_nav(&mut context, nav_rect, &style, None);
        }
        calls.store(0, Ordering::SeqCst);
        {
            let mut context = RenderContext::new(&mut backend);
            scaffold.draw_bottom_nav(&mut context, nav_rect, &style, None);
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "a settled Classic bar must not reach the backend to measure text"
        );
    }

    /// COMP-C1: the width breakpoints select the bottom bar, rail or drawer band.
    #[test]
    fn navigation_form_follows_the_breakpoints() {
        let scaffold = make_scaffold();
        assert_eq!(scaffold.navigation_breakpoints(), (599, 840));
        // Just below, at and above the bottom-bar boundary.
        assert_eq!(scaffold.navigation_form_for(598), ScaffoldNavigationForm::BottomBar);
        assert_eq!(scaffold.navigation_form_for(599), ScaffoldNavigationForm::BottomBar);
        assert_eq!(scaffold.navigation_form_for(600), ScaffoldNavigationForm::Rail);
        // Just below, at and above the drawer boundary.
        assert_eq!(scaffold.navigation_form_for(839), ScaffoldNavigationForm::Rail);
        assert_eq!(scaffold.navigation_form_for(840), ScaffoldNavigationForm::Drawer);
        assert_eq!(scaffold.navigation_form_for(841), ScaffoldNavigationForm::Drawer);
    }

    /// Custom breakpoints keep all three bands reachable.
    #[test]
    fn custom_breakpoints_partition_the_width_axis() {
        let mut scaffold = make_scaffold();
        scaffold.set_navigation_breakpoints(400, 700);
        assert_eq!(scaffold.navigation_form_for(400), ScaffoldNavigationForm::BottomBar);
        assert_eq!(scaffold.navigation_form_for(401), ScaffoldNavigationForm::Rail);
        assert_eq!(scaffold.navigation_form_for(699), ScaffoldNavigationForm::Rail);
        assert_eq!(scaffold.navigation_form_for(700), ScaffoldNavigationForm::Drawer);

        // A wide breakpoint inside the narrow band would leave the rail unreachable, so it is
        // raised to the first width after the narrow band.
        scaffold.set_navigation_breakpoints(400, 200);
        assert_eq!(scaffold.navigation_breakpoints(), (400, 401));
        assert_eq!(scaffold.navigation_form_for(401), ScaffoldNavigationForm::Drawer);
    }

    /// COMP-C1: the medium form paints the installed rail and insets the page beside it.
    #[test]
    fn the_medium_form_draws_the_installed_rail_and_insets_the_page() {
        let mut scaffold = make_scaffold();
        let mut rail = MaterialNavigationRail::new(Rect::new(0, 0, 0, 0));
        rail.add_item("home", "Home");
        rail.add_item("search", "Search");
        scaffold.set_navigation_rail(Some(rail));
        scaffold.set_geometry(Rect::new(0, 0, 700, 800));

        assert_eq!(scaffold.navigation_form(), ScaffoldNavigationForm::Rail);
        let svg = render_to_svg(&mut scaffold);
        // The rail's captions are glyph outlines, so the paint is checked as text runs: the rail
        // band is 80 px wide, and a run whose ink ends inside it can only be a rail label.
        let runs = crate::widget::svg::text_ink_boxes(&svg);
        assert!(
            runs.iter().any(|(left, _, right, _)| *left >= 0 && *right <= 80),
            "the medium form paints the rail's destinations inside its band: {runs:?}"
        );

        let band = scaffold.navigation_rail().expect("the rail is installed").geometry();
        assert_eq!(band, Rect::new(0, 56, 80, 744));
        assert_eq!(scaffold.content_rect(), Rect::new(80, 56, 620, 744));
        assert!(
            scaffold.bottom_nav_rect().is_none(),
            "the rail replaces the bottom bar in the medium form"
        );

        // A narrow window returns to the bottom bar and gives the rail's band back to the page.
        scaffold.set_geometry(Rect::new(0, 0, 500, 800));
        let _ = render_to_svg(&mut scaffold);
        assert_eq!(scaffold.navigation_form(), ScaffoldNavigationForm::BottomBar);
        assert_eq!(scaffold.content_rect(), Rect::new(0, 56, 500, 688));
        assert!(scaffold.bottom_nav_rect().is_some());
    }

    /// The historical two-form behaviour is untouched when no rail is installed.
    #[test]
    fn a_medium_width_without_a_rail_keeps_the_bottom_bar() {
        let mut scaffold = make_scaffold();
        scaffold.set_geometry(Rect::new(0, 0, 700, 800));
        assert_eq!(scaffold.navigation_form_for(700), ScaffoldNavigationForm::Rail);
        assert_eq!(scaffold.navigation_form(), ScaffoldNavigationForm::BottomBar);
        assert!(scaffold.bottom_nav_rect().is_some());
        assert_eq!(scaffold.content_rect(), Rect::new(0, 56, 700, 688));
    }

    /// A destination chosen on the rail is the scaffold's selection too.
    #[test]
    fn a_rail_destination_selects_the_matching_scaffold_destination() {
        let mut scaffold = make_scaffold();
        let mut rail = MaterialNavigationRail::new(Rect::new(0, 0, 0, 0));
        for (icon, label) in [("home", "Home"), ("search", "Search"), ("star", "Favorites")] {
            rail.add_item(icon, label);
        }
        scaffold.set_navigation_rail(Some(rail));
        scaffold.set_geometry(Rect::new(0, 0, 700, 800));

        let captured = Arc::new(AtomicUsize::new(usize::MAX));
        let c = Arc::clone(&captured);
        scaffold.nav_selected.connect(move |index: Arc<usize>| {
            c.store(*index, Ordering::SeqCst);
        });

        // The rail's own destination pitch is 72; the second band starts one pitch below the
        // rail's top edge, which is where the app bar ends.
        let band = scaffold.navigation_rail_rect().expect("the medium form owns a rail band");
        let pos = Point::new(band.x + 20, band.y + 72 + 20);
        scaffold.handle_event(&Event::MouseRelease { pos, button: 1, modifiers: 0 });

        assert_eq!(captured.load(Ordering::SeqCst), 1);
        assert_eq!(scaffold.selected_nav_index(), 1);
        assert_eq!(scaffold.navigation_rail().expect("installed").selected(), 1);
    }
}
