// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AppBar (Top Bar) widget — a mobile-style top navigation bar with title,
//! optional back button, and optional action text.
//!
//! The AppBar is a Material Design-inspired top app bar that displays a title
//! centered in the bar, an optional back arrow, and optional action text. It
//! emits `back_pressed` when the back area is tapped and `action_pressed` when
//! the action area is tapped.
//!
//! # Which side the affordances sit on
//!
//! The two text affordances are placed by *reading order*, not by geometry: the back arrow is the
//! bar's **leading** affordance and the action is its **trailing** one, so in a right-to-left
//! interface the arrow belongs at the right edge and the action at the left.
//! [`AppBar::direction`] is the one place that is decided, and both the drawing and the tap zones
//! read it — a bar whose arrow moved without its hit test moving would look right and be unusable.
//!
//! The title's *centring* is deliberately not affected: a centred box has the same left edge in
//! either direction, which is why the defect survived review on the middle of the bar.

use crate::compat::{Rc, RefCell};
use crate::core::{Color, HorizontalAlignment, Point, Rect, TextDirection};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_string, expect_text_direction, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconName, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The width of the bar's leading zone, which holds the back arrow.
const APP_BAR_LEADING_ZONE: i32 = 48;

/// The width of the bar's trailing zone, which holds the action.
const APP_BAR_TRAILING_ZONE: i32 = 80;

/// Space kept between the bar's leading edge and the back arrow's glyph.
const APP_BAR_ARROW_INSET: i32 = 12;

/// Space reserved either side of the title so it does not collide with the affordances.
///
/// These are the *reservations* the centring arithmetic uses, and they are narrower than the tap
/// zones on purpose: a 48 px zone holding a 12 px glyph would push the title a third of the bar
/// inward. The pair is applied through [`TextDirection`] like everything else on the horizontal axis.
const APP_BAR_TITLE_LEADING_RESERVE: i32 = 40;
const APP_BAR_TITLE_TRAILING_RESERVE: i32 = 80;

/// The title's reserve when there is nothing on that side to clear.
const APP_BAR_TITLE_BARE_RESERVE: i32 = 16;

/// Space kept between the action's glyph and the bar's trailing edge.
const APP_BAR_ACTION_INSET: i32 = 16;

/// Which affordance a point on the bar commits when the press is released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TapTarget {
    /// The leading back arrow, or the bar's fallback back region.
    Back,
    /// A caller-supplied leading icon (N-02).
    Leading,
    /// The trailing action text.
    Action,
    /// One of the trailing action buttons, by position in the action list.
    ActionAt(usize),
    /// The overflow affordance that stands in for the actions that did not fit.
    Overflow,
}

/// The narrowest a trailing action button may be drawn, in pixels.
const APP_BAR_ACTION_MIN_WIDTH: i32 = 48;

/// Space between adjacent trailing action buttons, in pixels.
const APP_BAR_ACTION_GAP: i32 = 4;

/// The action boxes derived from one action list and one trailing band.
struct ActionBoxes {
    /// Each fitting action, ordered from the trailing edge inward.
    boxes: Vec<(usize, Rect)>,
    /// The overflow affordance when the actions did not fit.
    overflow: Option<Rect>,
}

/// The inputs [`AppBar::action_boxes`] was last derived from.
struct ActionBoxesCache {
    revision: u32,
    font: crate::core::Font,
    band: Rect,
    boxes: Rc<ActionBoxes>,
}

/// The shortest main bar band, in pixels.
///
/// A bar with a bottom slot adds the slot's own height to this; a bar without one is
/// exactly this tall, which is the height every existing picture was drawn at.
const APP_BAR_MIN_HEIGHT: u32 = 56;

/// AppBar / Top Bar widget — mobile-style top navigation bar.
///
/// Displays a title centered in the bar, an optional back arrow in the bar's
/// *leading* zone, and optional action text in its *trailing* zone. Emits signals
/// when the back or action areas are clicked; the click is committed by the
/// release, so one press/release pair reports one event.
pub struct AppBar {
    base: BaseWidget,
    /// The title text centered in the bar.
    title: String,
    /// Whether to show the back arrow in the bar's leading zone.
    show_back: bool,
    /// Optional action text displayed in the bar's trailing zone.
    action_text: String,
    /// The writing direction the bar's two affordances are placed by.
    ///
    /// # Why this is not a cosmetic setting
    ///
    /// The back arrow and the action are the bar's *leading* and *trailing* affordances. Placing them
    /// at fixed screen edges was not a translation problem but a wrong one: in an Arabic or Hebrew
    /// interface the back arrow belongs where the reader starts, and an action that sits on the
    /// wrong side reads as a different control. Both the drawing and the tap zones are derived from
    /// this field, so the picture and the hit test cannot drift apart.
    ///
    /// Defaults to left-to-right, so a bar that never asks behaves exactly as it did.
    direction: TextDirection,
    /// Emitted when the back arrow area is pressed.
    pub back_pressed: GenericSignal,
    /// Emitted when the action text area is pressed.
    pub action_pressed: GenericSignal,
    /// Emitted with the index of a trailing action button when it is activated.
    pub action_index_pressed: Signal1<usize>,
    /// Emitted when the overflow affordance is activated.
    pub action_overflow_pressed: GenericSignal,
    /// Additional trailing actions, drawn right-to-left before the action text slot.
    actions: Vec<String>,
    /// Typed icon sources parallel to `actions` (N-01).
    action_icon_sources: Vec<Option<IconSource>>,
    /// A caller-supplied leading icon that replaces the fixed back arrow (N-02).
    leading_icon: Option<IconSource>,
    /// Bumped whenever the action list changes, so the layout cache can tell.
    actions_revision: u32,
    /// The last derived action boxes, shared by drawing and hit testing (G-A03).
    action_boxes_cache: RefCell<Option<ActionBoxesCache>>,
    /// Emitted when a caller-supplied leading icon is activated.
    pub leading_pressed: GenericSignal,
    /// Optional widget drawn in a band below the main bar.
    ///
    /// The band's height is the widget's own size hint (clamped to the bar's rectangle), and
    /// the main bar keeps [`APP_BAR_MIN_HEIGHT`]. A bar with no bottom slot paints exactly the
    /// band it always did.
    bottom: Option<Box<dyn Widget>>,
    /// Which affordance a primary-button press armed, if any.
    ///
    /// The press only arms the bar; the signal is emitted on the release, so one click
    /// reports one event. Without this gate a press followed by a release fired the same
    /// signal twice.
    pressed: Option<TapTarget>,
}

impl AppBar {
    /// Creates a new AppBar widget with the given title and geometry.
    ///
    /// By default, the back arrow is hidden and the action text is empty.
    pub fn new(title: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AppBar, geometry, "AppBar"),
            title: title.to_string(),
            show_back: false,
            action_text: String::new(),
            direction: TextDirection::default(),
            back_pressed: GenericSignal::new(),
            action_pressed: GenericSignal::new(),
            action_index_pressed: Signal1::new(),
            action_overflow_pressed: GenericSignal::new(),
            actions: Vec::new(),
            action_icon_sources: Vec::new(),
            leading_icon: None,
            actions_revision: 0,
            action_boxes_cache: RefCell::new(None),
            leading_pressed: GenericSignal::new(),
            bottom: None,
            pressed: None,
        }
    }

    /// Sets the title text displayed centered in the bar.
    pub fn set_title(&mut self, title: &str) {
        if self.title == title {
            return;
        }
        self.title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the current title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets whether the back arrow is shown in the bar's leading zone.
    pub fn set_show_back(&mut self, show_back: bool) {
        if self.show_back == show_back {
            return;
        }
        self.show_back = show_back;
        self.base.request_redraw();
    }

    /// Returns whether the back arrow is currently shown.
    pub fn show_back(&self) -> bool {
        self.show_back
    }

    /// Sets the action text displayed in the bar's trailing zone.
    ///
    /// Pass an empty string to hide the action area.
    pub fn set_action_text(&mut self, text: &str) {
        if self.action_text == text {
            return;
        }
        self.action_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current action text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }

    /// Returns the writing direction the bar's affordances are placed by.
    pub fn direction(&self) -> TextDirection {
        self.direction
    }

    /// Sets the trailing action buttons, replacing any previous list.
    ///
    /// The buttons are drawn from the bar's trailing edge inward, each as wide as its own
    /// measured label (never narrower than [`APP_BAR_ACTION_MIN_WIDTH`]). When the total does
    /// not fit, a single overflow affordance takes the innermost slot and
    /// [`Self::action_overflow_pressed`] reports a tap on it — the actions themselves stay in
    /// the model and are still reachable by index through [`Self::actions`].
    ///
    /// The legacy single [`Self::set_action_text`] slot is independent: it is drawn only when
    /// the action list is empty, which keeps every existing bar byte-identical.
    pub fn set_actions(&mut self, actions: Vec<String>) {
        if self.actions == actions {
            return;
        }
        self.actions = actions;
        self.actions_revision = self.actions_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// Returns the trailing action labels.
    pub fn actions(&self) -> &[String] {
        &self.actions
    }

    /// Sets a typed icon source per trailing action (N-01).
    ///
    /// The vector is parallel to [`Self::actions`]; a shorter vector leaves the remaining
    /// actions on their text path, which keeps an existing text-only bar byte-identical.
    pub fn set_action_icon_sources(&mut self, sources: Vec<Option<IconSource>>) {
        if self.action_icon_sources == sources {
            return;
        }
        self.action_icon_sources = sources;
        // Widths are unchanged (the slot is still the minimum action width), but the cache is
        // keyed by the action *revision*, and a new icon changes the pixels in the slot.
        self.base.request_redraw();
    }

    /// Returns the per-action icon sources.
    pub fn action_icon_sources(&self) -> &[Option<IconSource>] {
        &self.action_icon_sources
    }

    /// Sets a caller-supplied leading icon, replacing the fixed back arrow (N-02).
    pub fn set_leading_icon(&mut self, icon: Option<IconSource>) {
        if self.leading_icon == icon {
            return;
        }
        self.leading_icon = icon;
        self.base.request_redraw();
    }

    /// Returns the caller-supplied leading icon.
    pub fn leading_icon(&self) -> Option<IconSource> {
        self.leading_icon
    }

    /// Sets an optional widget shown in a band below the main bar.
    ///
    /// The main bar keeps [`APP_BAR_MIN_HEIGHT`]; the slot takes the rest of the bar's
    /// rectangle, so a host that wants a nested bottom strip lays the bar out taller. Passing
    /// `None` restores the single-band bar and therefore the original pixels.
    pub fn set_bottom_widget(&mut self, widget: Option<Box<dyn Widget>>) {
        self.bottom = widget;
        self.base.request_redraw();
    }

    /// Returns the widget shown in the bar's bottom slot, if any.
    pub fn bottom_widget(&self) -> Option<&dyn Widget> {
        self.bottom.as_deref()
    }

    /// The height a host should give this bar so its bottom slot is fully visible.
    pub fn preferred_height(&self) -> u32 {
        APP_BAR_MIN_HEIGHT + self.bottom_height()
    }

    /// The height of the bottom slot's band, zero when there is no slot.
    fn bottom_height(&self) -> u32 {
        let Some(bottom) = self.bottom.as_deref() else {
            return 0;
        };
        let wanted = bottom.size_hint().height;
        let room = self.geometry().height.saturating_sub(APP_BAR_MIN_HEIGHT);
        wanted.min(room)
    }

    /// The main bar band: the top of the control, without the bottom slot.
    fn main_band(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(self.bottom_height()))
    }

    /// The font the trailing action buttons are measured and painted with.
    fn action_font(&self) -> crate::core::Font {
        let size = (self.main_band().height as f32 * 0.32).clamp(12.0, 18.0);
        role_font(self.base.style(), size)
    }

    /// The trailing action boxes that fit, and the overflow affordance when one is needed.
    ///
    /// The boxes are ordered from the trailing edge inward: element zero is nearest the edge,
    /// which is the position a primary action occupies. Paint and hit test both read this one
    /// derivation, so a button cannot be drawn where it cannot be tapped.
    fn action_boxes(&self) -> Rc<ActionBoxes> {
        let band = self.trailing_zone();
        let font = self.action_font();
        if let Some(cache) = self.action_boxes_cache.borrow().as_ref() {
            if cache.revision == self.actions_revision && cache.font == font && cache.band == band {
                return Rc::clone(&cache.boxes);
            }
        }
        if self.actions.is_empty() || band.width == 0 {
            let empty = Rc::new(ActionBoxes { boxes: Vec::new(), overflow: None });
            *self.action_boxes_cache.borrow_mut() = Some(ActionBoxesCache {
                revision: self.actions_revision,
                font,
                band,
                boxes: Rc::clone(&empty),
            });
            return empty;
        }
        let mut cursor = band.right();
        let mut boxes: Vec<(usize, Rect)> = Vec::new();
        let mut overflow = None;
        for (index, label) in self.actions.iter().enumerate() {
            let width = ((crate::widget::metrics::estimate_text_width(label, &font, 1.0) as i32)
                + 16)
                .max(APP_BAR_ACTION_MIN_WIDTH);
            let left = cursor - width;
            if left < band.x + APP_BAR_ACTION_MIN_WIDTH + APP_BAR_ACTION_GAP {
                overflow =
                    Some(Rect::new(band.x, band.y, APP_BAR_ACTION_MIN_WIDTH as u32, band.height));
                break;
            }
            cursor = left;
            boxes.push((index, Rect::new(cursor, band.y, width as u32, band.height)));
            cursor -= APP_BAR_ACTION_GAP;
        }
        let boxes = Rc::new(ActionBoxes { boxes, overflow });
        *self.action_boxes_cache.borrow_mut() = Some(ActionBoxesCache {
            revision: self.actions_revision,
            font,
            band,
            boxes: Rc::clone(&boxes),
        });
        boxes
    }

    /// Sets the writing direction, and repaints.
    ///
    /// A right-to-left bar puts its back arrow at the **right** edge and its action at the left — the
    /// mirror of the left-to-right arrangement, applied to both the drawing and the tap zones. See the
    /// field for why the two must move together.
    pub fn set_direction(&mut self, direction: TextDirection) {
        if self.direction != direction {
            self.direction = direction;
            self.base.request_redraw();
        }
    }

    /// The leading zone: where the back arrow is drawn and hit-tested.
    ///
    /// # Why this is a method rather than two inline expressions
    ///
    /// `draw` and `handle_event` both need the same answer to "which side is the leading edge".
    /// Deriving it twice is how a mirrored bar ends up with its arrow on one side and its hit test on
    /// the other — a defect that looks correct in a screenshot. One derivation, two consumers.
    fn leading_zone(&self) -> Rect {
        let rect = self.main_band();
        let x = if self.direction.is_right_to_left() {
            rect.right() - APP_BAR_LEADING_ZONE
        } else {
            rect.x
        };
        Rect::new(x, rect.y, APP_BAR_LEADING_ZONE as u32, rect.height)
    }

    /// The trailing zone: where the action is drawn and hit-tested.
    fn trailing_zone(&self) -> Rect {
        let rect = self.main_band();
        let x = if self.direction.is_right_to_left() {
            rect.x
        } else {
            rect.right() - APP_BAR_TRAILING_ZONE
        };
        Rect::new(x, rect.y, APP_BAR_TRAILING_ZONE as u32, rect.height)
    }

    /// Resolves a point on the bar to the affordance a tap there commits.
    ///
    /// The press and the release both use this, so the bar can tell whether the pointer
    /// left the affordance it armed between the two events.
    fn tap_target_at(&self, pos: Point) -> Option<TapTarget> {
        let boxes = self.action_boxes();
        for (index, box_rect) in boxes.boxes.iter() {
            if box_rect.contains_point(pos) {
                return Some(TapTarget::ActionAt(*index));
            }
        }
        if boxes.overflow.is_some_and(|box_rect| box_rect.contains_point(pos)) {
            return Some(TapTarget::Overflow);
        }
        let leading_present = self.show_back || self.leading_icon.is_some();
        if leading_present && self.leading_zone().contains_point(pos) {
            return Some(if self.leading_icon.is_some() {
                TapTarget::Leading
            } else {
                TapTarget::Back
            });
        }
        if !self.action_text.is_empty()
            && self.actions.is_empty()
            && self.trailing_zone().contains_point(pos)
        {
            return Some(TapTarget::Action);
        }
        // If neither back nor action matched, the rest of the bar is a back gesture
        // when a back arrow is shown; otherwise it is inert.
        if self.show_back {
            return Some(TapTarget::Back);
        }
        None
    }
}

impl Widget for AppBar {
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
        crate::core::Size::new(400, 56)
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
            "back_pressed" => Some(EventSignalRef::unit("back_pressed", &self.back_pressed)),
            "action_pressed" => Some(EventSignalRef::unit("action_pressed", &self.action_pressed)),
            "action_index_pressed" => Some(EventSignalRef::mapped(
                "action_index_pressed",
                &self.action_index_pressed,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
            "action_overflow_pressed" => {
                Some(EventSignalRef::unit("action_overflow_pressed", &self.action_overflow_pressed))
            }
            "leading_pressed" => {
                Some(EventSignalRef::unit("leading_pressed", &self.leading_pressed))
            }
            _ => None,
        }
    }
}

/// `AppBar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for AppBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
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
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "direction", BASE_PROPERTY_NAMES]
    }
}

impl Draw for AppBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let full_rect = self.geometry();
        let rect = self.main_band();
        let is_enabled = self.base.is_enabled();
        let bar_height = rect.height;

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because every colour below
        // was previously hardcoded.
        //
        // The theme reads are separate manager locks, each taken and released inside
        // `resolved_theme_style`, so none is held across the draw or across another
        // accessor — the global manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("app_bar");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(if is_enabled {
                Color::rgba(248, 248, 250, 255)
            } else {
                Color::DISABLED_BACKGROUND
            });
        let border_color = if is_enabled {
            style
                .border_color
                .or_else(|| theme.as_ref().and_then(|t| t.border_color))
                .unwrap_or(Color::DIVIDER)
        } else {
            // A disabled border is derived from the bar's own fill rather than a fixed grey:
            // a fixed `DISABLED_FOREGROUND` measures 2.48:1 against a light bar, so the rule
            // all but vanished on that appearance while staying legible on the dark one.
            background.blend(
                &background.contrast_color(),
                crate::widget::BaseWidget::disabled_ink_weight(),
            )
        };
        let text_color = if is_enabled {
            style
                .text_color
                .or_else(|| theme.as_ref().and_then(|t| t.text_color))
                .unwrap_or(Color::FOREGROUND)
        } else {
            background.blend(
                &background.contrast_color(),
                crate::widget::BaseWidget::disabled_ink_weight(),
            )
        };
        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // Draw bottom border line
        let border_y = rect.y + bar_height as i32 - 1;
        context.draw_line_stroke(
            Point::new(rect.x, border_y),
            Point::new(rect.x + rect.width as i32, border_y),
            border_color,
            1,
        );

        // Determine font sizes based on bar height, **scaled by the device's text-size preference**.
        //
        // The upper clamp of 22 was a silent ceiling on text scaling: `2x` text on a standard
        // 56 px bar asks for 42 px, the clamp returned 22, and the control looked unchanged — an
        // accessibility setting that appears to do nothing is worse than one that is not offered.
        // The ceiling is now raised in proportion to the requested scale, so the control follows
        // the preference it is given while still not letting a 3x preference produce a title taller
        // than its own bar. `bar_height` itself grows under a scaled layout, which is what makes the
        // raised ceiling reachable rather than merely permitted.
        let text_scale = crate::platform::profile::text_scale();
        let title_font_size =
            (bar_height as f32 * 0.38 * text_scale).clamp(14.0, 22.0 * text_scale);
        let action_font_size =
            (bar_height as f32 * 0.32 * text_scale).clamp(12.0, 18.0 * text_scale);

        // ── Back arrow (the bar's leading zone) ──
        //
        // The arrow is an **icon outline**, not the `←` (U+2190) text glyph: no bundled face covers
        // the arrows block, so `draw_text("←")` fell back to an 8x8 bitmap and drew a blocky arrow.
        // An icon is geometry both backends agree on, and the same shape an
        // `Icon::new(.., IconName::ArrowLeft)` widget draws.
        //
        // `icon_side` is also the arrow's *reserved width*: the boxes below read it to keep their
        // layout clear of the icon rather than of a glyph's advance.
        let icon_side = ((action_font_size + 2.0).round() as i32).clamp(16, 28);
        if self.show_back || self.leading_icon.is_some() {
            // The arrow sits `APP_BAR_ARROW_INSET` from the bar's *leading* edge, whichever edge that
            // is: the inset is measured from the edge to the icon box, and the box is squared, so the
            // RTL placement subtracts its side rather than a glyph advance.
            let back_x = if self.direction.is_right_to_left() {
                rect.right() - APP_BAR_ARROW_INSET - icon_side
            } else {
                rect.x + APP_BAR_ARROW_INSET
            };
            let back_y = rect.y + (bar_height as i32 - icon_side) / 2;
            let back_rect = Rect::new(back_x, back_y, icon_side as u32, icon_side as u32);
            // A caller-supplied leading icon replaces the fixed arrow (N-02); without one the
            // back affordance keeps its existing outline.
            match self.leading_icon {
                Some(source) => {
                    if let Err(error) =
                        crate::widget::paint_icon(context, source.into(), back_rect, text_color)
                    {
                        log::warn!("app bar leading icon could not be painted: {error:?}");
                    }
                }
                None => {
                    crate::widget::draw_icon_at(
                        context,
                        back_rect,
                        text_color,
                        IconName::ArrowLeft,
                    );
                }
            }
        }

        // ── Centered title ──
        //
        // The two reserves are named for the *role* they clear (leading / trailing) and mapped
        // through the direction together with everything else, so an RTL bar reserves the same total
        // width on the same sides its affordances actually occupy. A centered box is unaffected by the
        // direction, which is why an unmirrored title on an unmirrored bar went unnoticed.
        if !self.title.is_empty() {
            // Derive the title's face from the **effective font** — the theme's family/weight/leading —
            // and scale the role-derived title size by the same factor, so a text-scale preference
            // reaches the title (D09-STYLE-01).
            let title_font = role_font(style, title_font_size);
            let metrics = crate::widget::base_widgets::measure_cache::measure_text(
                context,
                &self.title,
                &title_font,
            );

            let (leading_reserve, trailing_reserve) = (
                if self.show_back {
                    APP_BAR_TITLE_LEADING_RESERVE
                } else {
                    APP_BAR_TITLE_BARE_RESERVE
                },
                if self.action_text.is_empty() {
                    APP_BAR_TITLE_BARE_RESERVE
                } else {
                    APP_BAR_TITLE_TRAILING_RESERVE
                },
            );
            let title_width = metrics.width as i32;
            // Overflow falls back to "left-align inside the leading reserve", which in RTL means the
            // title grows from its right edge — the same rule read in the other direction.
            let overflow_x = if self.direction.is_right_to_left() {
                rect.right() - leading_reserve - title_width
            } else {
                rect.x + leading_reserve
            };
            let available_width = rect.width as i32 - leading_reserve - trailing_reserve;
            let title_x = if title_width > available_width {
                overflow_x
            } else {
                rect.x + (rect.width as i32 / 2) - (title_width / 2)
            };
            let title_y = rect.y + (bar_height as i32 - metrics.height as i32) / 2;

            // Fitted between the two reserves. The overflow branch used to move the *origin* and
            // then draw the full width, so a long localised title still ran under the trailing
            // action.
            //
            // The band is widened by the fitted draw's own inset so a title that fits starts at
            // `title_x` (and therefore stays centred) instead of three pixels to its right; only
            // the elided case uses the extra room.
            let fit_inset = crate::render::TEXT_FIT_MARGIN as i32;
            context.draw_text_fitted_cow(
                Rect::new(
                    title_x - fit_inset,
                    title_y,
                    (available_width + 2 * fit_inset).max(1) as u32,
                    metrics.height.max(1),
                ),
                &self.title,
                &title_font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // ── Action text (the bar's trailing zone) ──
        if !self.action_text.is_empty() && self.actions.is_empty() {
            // Same rule as the title: the theme face at the action role's own size, scaled by the
            // text preference (D09-STYLE-01).
            let action_font = role_font(style, action_font_size);
            let metrics = crate::widget::base_widgets::measure_cache::measure_text(
                context,
                &self.action_text,
                &action_font,
            );

            let action_x = if self.direction.is_right_to_left() {
                rect.x + APP_BAR_ACTION_INSET
            } else {
                rect.right() - metrics.width as i32 - APP_BAR_ACTION_INSET
            };
            let action_y = rect.y + (bar_height as i32 - metrics.height as i32) / 2;
            let fit_inset = crate::render::TEXT_FIT_MARGIN as i32;

            // The action is the bar's one accented affordance: when the app bar is
            // enabled it is tinted toward the resolved ink so it reads as an action
            // against the bar's fill, and it moves with the appearance like the rest
            // of the chrome.
            let action_color = if is_enabled {
                text_color.blend(&background, 0.25)
            } else {
                background.blend(
                    &background.contrast_color(),
                    crate::widget::BaseWidget::disabled_ink_weight(),
                )
            };
            context.draw_text_fitted_cow(
                Rect::new(
                    action_x - fit_inset,
                    action_y,
                    (metrics.width as i32 + 2 * fit_inset).max(1) as u32,
                    metrics.height.max(1),
                ),
                &self.action_text,
                &action_font,
                action_color,
                HorizontalAlignment::Left,
            );
        }

        // ── Trailing action buttons ──
        //
        // The boxes come from `action_boxes`, the same derivation the hit test reads. A label
        // that does not fit its own box is elided by the fitted draw, never painted over its
        // neighbour.
        let boxes = self.action_boxes();
        if !boxes.boxes.is_empty() || boxes.overflow.is_some() {
            let action_font = role_font(style, action_font_size);
            let action_color = if is_enabled {
                text_color.blend(&background, 0.25)
            } else {
                background.blend(
                    &background.contrast_color(),
                    crate::widget::BaseWidget::disabled_ink_weight(),
                )
            };
            for (index, box_rect) in boxes.boxes.iter() {
                // A typed icon replaces the label inside the same box (N-01); a bar that never sets
                // one keeps its text-only picture.
                if let Some(Some(source)) = self.action_icon_sources.get(*index) {
                    let side =
                        action_font.size_u32().clamp(1, box_rect.width.min(box_rect.height).max(1));
                    let icon_rect = Rect::new(
                        box_rect.x + (box_rect.width.saturating_sub(side) / 2) as i32,
                        box_rect.y + (box_rect.height.saturating_sub(side) / 2) as i32,
                        side,
                        side,
                    );
                    if let Err(error) = crate::widget::paint_icon(
                        context,
                        (*source).into(),
                        icon_rect,
                        action_color,
                    ) {
                        log::warn!("app bar action icon could not be painted: {error:?}");
                    }
                    continue;
                }
                if let Some(label) = self.actions.get(*index) {
                    context.draw_text_fitted_cow(
                        context.text_line(*box_rect, &action_font),
                        label,
                        &action_font,
                        action_color,
                        HorizontalAlignment::Center,
                    );
                }
            }
            if let Some(box_rect) = boxes.overflow {
                context.draw_text_fitted_cow(
                    context.text_line(box_rect, &action_font),
                    "\u{2026}",
                    &action_font,
                    action_color,
                    HorizontalAlignment::Center,
                );
            }
        }

        // ── Nested bottom slot ──
        if self.bottom.is_some() {
            let band_height = self.bottom_height();
            if band_height > 0 {
                let band = Rect::new(
                    full_rect.x,
                    full_rect.y + full_rect.height as i32 - band_height as i32,
                    full_rect.width,
                    band_height,
                );
                if let Some(bottom) = self.bottom.as_deref_mut() {
                    bottom.set_geometry(band);
                    if let Some(drawable) = crate::widget::draw_bridge::draw_of(bottom) {
                        drawable.draw(context);
                    }
                }
            }
        }
    }
}

impl EventHandler for AppBar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            // A press arms the bar and records which affordance it touched; it commits
            // nothing. The action belongs to the release, which is the crate's click
            // convention and the only way one physical click can report one event.
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                let target = self.tap_target_at(*pos);
                if self.pressed != target {
                    self.pressed = target;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                let Some(armed) = self.pressed.take() else {
                    return;
                };
                self.base.request_redraw();

                // The zones come from the same accessors the drawing uses, so a mirrored bar's arrow
                // is tappable where it is *drawn*. Deriving them twice is how "looks right, is
                // unusable" happens.
                //
                // Tap zones use the crate's **exclusive** far edge convention:
                // `Rect::contains_point` uses `x < x + width`, so a hand-written `<=`
                // made the boundary column belong to both zones. The leading zone is
                // tested first, so testing it as a rect is what keeps the middle zone's
                // first column out of it.
                //
                // A release over a different affordance (or off the bar) cancels the click,
                // the same way a drag off a button cancels it.
                if self.tap_target_at(*pos) != Some(armed) {
                    return;
                }
                match armed {
                    TapTarget::Back => self.back_pressed.emit(),
                    TapTarget::Leading => self.leading_pressed.emit(),
                    TapTarget::Action => self.action_pressed.emit(),
                    TapTarget::ActionAt(index) => {
                        self.action_index_pressed.emit(index);
                        self.action_pressed.emit();
                    }
                    TapTarget::Overflow => self.action_overflow_pressed.emit(),
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
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    fn make_app_bar() -> AppBar {
        AppBar::new("Home", Rect::new(0, 0, 375, 56))
    }

    #[test]
    fn app_bar_default_creation() {
        let bar = make_app_bar();
        assert_eq!(bar.kind(), WidgetKind::AppBar);
        assert_eq!(bar.title(), "Home");
        assert!(!bar.show_back());
        assert_eq!(bar.action_text(), "");
        assert!(bar.is_visible());
        assert!(bar.is_enabled());
        assert_eq!(bar.geometry(), Rect::new(0, 0, 375, 56));
    }

    #[test]
    fn app_bar_title_accessors() {
        let mut bar = make_app_bar();
        assert_eq!(bar.title(), "Home");

        bar.set_title("Settings");
        assert_eq!(bar.title(), "Settings");

        bar.set_title("");
        assert_eq!(bar.title(), "");
    }

    #[test]
    fn app_bar_show_back_accessors() {
        let mut bar = make_app_bar();
        assert!(!bar.show_back());

        bar.set_show_back(true);
        assert!(bar.show_back());

        bar.set_show_back(false);
        assert!(!bar.show_back());
    }

    #[test]
    fn app_bar_action_text_accessors() {
        let mut bar = make_app_bar();
        assert_eq!(bar.action_text(), "");

        bar.set_action_text("Save");
        assert_eq!(bar.action_text(), "Save");

        bar.set_action_text("");
        assert_eq!(bar.action_text(), "");
    }

    #[test]
    fn app_bar_back_pressed_signal_emits_on_left_tap() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // Tap in left zone (first 48px). The press arms the bar, the release commits.
        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 1, modifiers: 0 });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "a press alone must not emit");
        bar.handle_event(&Event::MouseRelease { pos: Point::new(10, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn app_bar_back_pressed_emits_on_center_tap_when_back_shown() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // Tap in center — falls through to back when show_back is true
        bar.handle_event(&Event::MousePress { pos: Point::new(188, 28), button: 1, modifiers: 0 });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "a press alone must not emit");
        bar.handle_event(&Event::MouseRelease { pos: Point::new(188, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn app_bar_action_pressed_signal_emits_on_right_tap() {
        let mut bar = make_app_bar();
        bar.set_action_text("Save");

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        bar.action_pressed.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // Tap in right action zone (last 80px)
        bar.handle_event(&Event::MousePress { pos: Point::new(340, 28), button: 1, modifiers: 0 });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "a press alone must not emit");
        bar.handle_event(&Event::MouseRelease { pos: Point::new(340, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn app_bar_action_pressed_not_emitted_on_center_tap() {
        let mut bar = make_app_bar();
        bar.set_action_text("Save");
        bar.set_show_back(true);

        let action_fired = Arc::new(AtomicUsize::new(0));
        let a = action_fired.clone();
        bar.action_pressed.connect(move || {
            a.fetch_add(1, Ordering::SeqCst);
        });

        // Tap in center — action should NOT fire
        bar.handle_event(&Event::MousePress { pos: Point::new(188, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(188, 28), button: 1 });
        assert_eq!(action_fired.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn app_bar_disabled_blocks_events() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);
        bar.set_enabled(false);
        bar.set_action_text("Save");

        let back_fired = Arc::new(AtomicUsize::new(0));
        let b = back_fired.clone();
        bar.back_pressed.connect(move || {
            b.fetch_add(1, Ordering::SeqCst);
        });

        let action_fired = Arc::new(AtomicUsize::new(0));
        let a = action_fired.clone();
        bar.action_pressed.connect(move || {
            a.fetch_add(1, Ordering::SeqCst);
        });

        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(10, 28), button: 1 });
        assert_eq!(back_fired.load(Ordering::SeqCst), 0);
        assert_eq!(action_fired.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn app_bar_svg_output() {
        let mut bar = make_app_bar();
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"375\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn app_bar_svg_with_back_and_action() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);
        bar.set_action_text("Cancel");
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"375\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn app_bar_back_pressed_signal_accessor() {
        let bar = make_app_bar();
        let signal = &bar.back_pressed;
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        signal.connect(move || {
            f.store(true, Ordering::SeqCst);
        });
        signal.emit();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_action_pressed_signal_accessor() {
        let bar = make_app_bar();
        let signal = &bar.action_pressed;
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        signal.connect(move || {
            f.store(true, Ordering::SeqCst);
        });
        signal.emit();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_other_button_noop() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // Right-button click should be ignored
        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 2, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(10, 28), button: 2 });
        assert_eq!(fired.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn app_bar_release_commits_a_prior_press_exactly_once() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // A release with no press behind it is not a click.
        bar.handle_event(&Event::MouseRelease { pos: Point::new(10, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "a bare release must not emit");

        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(10, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "one press+release emits once");
    }

    /// The back arrow is the bar's **leading** affordance, so a right-to-left bar draws it at the
    /// right edge and one drawn at the left would contradict the reader's own order.
    ///
    /// # The defect this pins
    ///
    /// Both affordances were placed at fixed *screen* edges — the arrow always at `rect.x`, the action
    /// always at `rect.right()`. In an Arabic or Hebrew interface that puts the way *forward* where the
    /// reader looks last, and there was no way to say otherwise.
    ///
    /// # How the ink is located
    ///
    /// The SVG backend draws text as glyph geometry, not `<text>` elements (a `<text>` would be
    /// rendered by the viewer's own font rather than this crate's), so the assertion reads the
    /// extent of the run the bar emitted. Giving each affordance its own render keeps the two apart
    /// without inventing a text layout.
    ///
    /// The two affordances are located by **different readers**, because they are now different
    /// kinds of drawing: the action is a text run (`text_ink_boxes`, which knows the `font8x8`
    /// grammar, an outline face's fractional `M…L…Z` and the `data-text` tag), while the back arrow
    /// is an **icon** — a non-text `<path>` — read with `first_shape_bounds`. Reading the arrow with
    /// the text reader would find nothing, which is the honest report that it stopped being text.
    #[test]
    fn a_right_to_left_bar_mirrors_both_affordances() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();

        /// The bitmap face fills its glyph box, so its ink left *is* the box left. An outline face
        /// draws a real glyph whose ink is inset, so the ink left sits one or two pixels inside the
        /// box left at these sizes. Pinning the two equal encoded a property of the bitmap face,
        /// not of the layout.
        const INK_INSET_TOLERANCE: i32 = 3;

        fn glyph_xs(svg: &str) -> (i32, i32) {
            let boxes = crate::widget::svg::text_ink_boxes(svg);
            let (left, right) = boxes
                .iter()
                .fold((i32::MAX, i32::MIN), |(min, max), b| (min.min(b.0), max.max(b.2)));
            assert!(left <= right, "the bar painted no ink at all");
            (left, right)
        }

        /// The back arrow's ink, read as the non-text shape it now is.
        fn icon_x(svg: &str) -> (i32, i32) {
            let (left, _, right, _) =
                crate::widget::svg::first_shape_bounds(svg).expect("the arrow icon draws a shape");
            (left, right)
        }

        // The title is useless for this comparison because it is centred and must not move, so each
        // case is rendered with one affordance only.
        fn only_back(direction: TextDirection) -> String {
            let mut bar = AppBar::new("", Rect::new(0, 0, 400, 56));
            bar.set_show_back(true);
            bar.set_direction(direction);
            render_to_svg(&mut bar)
        }
        fn only_action(direction: TextDirection) -> String {
            let mut bar = AppBar::new("", Rect::new(0, 0, 400, 56));
            bar.set_action_text("Save");
            bar.set_direction(direction);
            render_to_svg(&mut bar)
        }

        let (ltr_back_left, ltr_back_right) = icon_x(&only_back(TextDirection::LeftToRight));
        let (rtl_back_left, _) = icon_x(&only_back(TextDirection::RightToLeft));
        // The icon's **box** left is the inset; the glyph's *ink* sits inside that box, because a
        // Material arrow does not touch its own 24-unit grid's edge. The band below therefore
        // allows the box (side up to 28) to sit at the inset: the ink must fall inside
        // `inset .. inset + side`, which holds the placement the rule is about without asserting a
        // property of the glyph's internal padding.
        assert!(
            (APP_BAR_ARROW_INSET..=APP_BAR_ARROW_INSET + 28).contains(&ltr_back_left)
                && ltr_back_right > ltr_back_left,
            "LTR draws the arrow box at the left inset {APP_BAR_ARROW_INSET}; its ink spanned \
             {ltr_back_left}..{ltr_back_right}"
        );
        assert!(
            rtl_back_left > 400 / 2,
            "RTL must draw the arrow on the right half, drew its ink at {rtl_back_left}"
        );

        let (_, ltr_action_right) = glyph_xs(&only_action(TextDirection::LeftToRight));
        let (rtl_action_left, _) = glyph_xs(&only_action(TextDirection::RightToLeft));
        assert!(
            ltr_action_right > 400 - APP_BAR_ACTION_INSET - 40,
            "LTR must draw the action against the right edge, its ink ended at {ltr_action_right}"
        );
        assert!(
            (rtl_action_left - APP_BAR_ACTION_INSET).abs() <= INK_INSET_TOLERANCE,
            "RTL draws the action at the left inset: ink left {rtl_action_left}, inset {APP_BAR_ACTION_INSET}"
        );
    }

    /// The tap zones are the same derivation the drawing uses, so they move with it.
    #[test]
    fn the_leading_and_trailing_zones_follow_the_direction() {
        let mut bar = AppBar::new("Home", Rect::new(0, 0, 400, 56));
        bar.set_show_back(true);
        bar.set_action_text("Save");
        assert_eq!(bar.leading_zone().x, 0);
        assert_eq!(bar.trailing_zone().x, 400 - APP_BAR_TRAILING_ZONE);

        bar.set_direction(TextDirection::RightToLeft);
        assert_eq!(bar.leading_zone().right(), 400);
        assert_eq!(bar.leading_zone().width, APP_BAR_LEADING_ZONE as u32);
        assert_eq!(bar.trailing_zone().x, 0);
    }

    /// The picture and the hit test must move together.
    ///
    /// A mirrored arrow that still answers only at the old edge is the worst of both: the bar reads
    /// correctly and nothing happens when the user taps what they see. This drives real events at the
    /// mirrored position rather than reading the zone accessor, so a `draw`/`handle_event` divergence
    /// fails here.
    #[test]
    fn a_mirrored_bars_tap_zones_follow_its_arrow() {
        let mut bar = AppBar::new("Home", Rect::new(0, 0, 400, 56));
        bar.set_show_back(true);
        bar.set_action_text("Save");
        bar.set_direction(TextDirection::RightToLeft);

        let back = Arc::new(AtomicUsize::new(0));
        let b = back.clone();
        bar.back_pressed.connect(move || {
            b.fetch_add(1, Ordering::SeqCst);
        });
        let action = Arc::new(AtomicUsize::new(0));
        let a = action.clone();
        bar.action_pressed.connect(move || {
            a.fetch_add(1, Ordering::SeqCst);
        });

        // Right edge: the arrow. Its zone is 48 px wide, so 380 is inside it.
        bar.handle_event(&Event::MousePress { pos: Point::new(380, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(380, 28), button: 1 });
        assert_eq!(back.load(Ordering::SeqCst), 1, "tapping the drawn arrow must press back");
        assert_eq!(action.load(Ordering::SeqCst), 0, "and must not press the action");

        // Left edge: the action, in an 80 px zone.
        back.store(0, Ordering::SeqCst);
        bar.handle_event(&Event::MousePress { pos: Point::new(20, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(20, 28), button: 1 });
        assert_eq!(
            action.load(Ordering::SeqCst),
            1,
            "tapping the drawn action must fire the action"
        );
        assert_eq!(back.load(Ordering::SeqCst), 0, "and must not press back");
    }

    /// The default must remain byte-identical so a bar that never asks behaves exactly as it did.
    #[test]
    fn the_default_bar_is_still_left_to_right() {
        // Both halves are rendered and compared, so pin the appearance across them as every other
        // two-render comparison in the crate does; otherwise a concurrent theme switch between the
        // two renders fails this for a reason unrelated to direction.
        let _guard = crate::style::theme_test_guard();
        let mut untouched = make_app_bar();
        untouched.set_show_back(true);
        untouched.set_action_text("Save");
        let mut ltr = make_app_bar();
        ltr.set_show_back(true);
        ltr.set_action_text("Save");
        ltr.set_direction(TextDirection::LeftToRight);
        assert_eq!(render_to_svg(&mut untouched), render_to_svg(&mut ltr));
        assert_eq!(
            AppBar::new("x", Rect::new(0, 0, 10, 10)).direction(),
            TextDirection::LeftToRight
        );
    }

    /// The direction is reachable from the property API with a matching read-back token.
    #[test]
    fn the_direction_round_trips_through_the_property_api() {
        let mut bar = make_app_bar();
        assert_eq!(bar.get("direction").unwrap().as_str(), Some("ltr"));
        bar.set("direction", CapabilityValue::String("rtl".to_string())).unwrap();
        assert_eq!(bar.direction(), TextDirection::RightToLeft);
        assert_eq!(bar.get("direction").unwrap().as_str(), Some("rtl"));
    }

    #[test]
    fn a_crowded_action_list_folds_into_an_overflow_affordance() {
        let mut bar = make_app_bar();
        let mut actions: Vec<String> = Vec::new();
        for index in 0..8 {
            actions.push(format!("Action {index}"));
        }
        bar.set_actions(actions);
        assert_eq!(bar.actions().len(), 8);
        // Tapping the trailing edge activates the first action; the overflow zone reports
        // separately. Both are reachable through the public signals.
        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        bar.action_index_pressed.connect(move |index: Arc<usize>| {
            f.store(*index, Ordering::SeqCst);
        });
        bar.handle_event(&Event::MousePress { pos: Point::new(370, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(370, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "the trailing edge is action zero");
    }

    #[test]
    fn a_bottom_slot_extends_the_bar_without_moving_the_main_band() {
        let mut bar = AppBar::new("Home", Rect::new(0, 0, 375, 96));
        bar.set_bottom_widget(Some(Box::new(crate::widget::base_widgets::label::Label::new(
            "tabs".to_string(),
            Rect::new(0, 0, 375, 40),
        ))));
        assert!(bar.preferred_height() >= 56);
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn a_custom_leading_icon_replaces_the_back_arrow_and_reports_its_own_signal() {
        let mut bar = make_app_bar();
        assert_eq!(bar.leading_icon(), None);
        bar.set_leading_icon(Some(IconSource::Builtin(IconName::Menu)));
        assert_eq!(bar.leading_icon(), Some(IconSource::Builtin(IconName::Menu)));
        let leading = Arc::new(AtomicUsize::new(0));
        let sink = leading.clone();
        bar.leading_pressed.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        let back = Arc::new(AtomicUsize::new(0));
        let back_sink = back.clone();
        bar.back_pressed.connect(move || {
            back_sink.fetch_add(1, Ordering::SeqCst);
        });
        bar.handle_event(&Event::MousePress { pos: Point::new(20, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(20, 28), button: 1 });
        assert_eq!(leading.load(Ordering::SeqCst), 1);
        assert_eq!(back.load(Ordering::SeqCst), 0, "the custom icon is not the back affordance");
    }

    #[test]
    fn trailing_actions_accept_icons_without_moving_the_hit_test() {
        let mut bar = make_app_bar();
        bar.set_actions(vec!["Save".to_string()]);
        bar.set_action_icon_sources(vec![Some(IconSource::Builtin(IconName::Save))]);
        assert_eq!(bar.action_icon_sources().len(), 1);
        let fired = Arc::new(AtomicUsize::new(0));
        let sink = fired.clone();
        bar.action_index_pressed.connect(move |index: Arc<usize>| {
            sink.store(*index, Ordering::SeqCst);
        });
        bar.handle_event(&Event::MousePress { pos: Point::new(370, 28), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(370, 28), button: 1 });
        assert_eq!(fired.load(Ordering::SeqCst), 0);
    }
}
