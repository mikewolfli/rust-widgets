// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Banner — a persistent, full-width notice.
//!
//! # Why this is not `Snackbar`
//!
//! A snackbar is transient: it appears at the bottom of the screen, hides itself
//! after a timeout, and is at most a nudge. A banner is the Material 3
//! *persistent* notice instead — it sits in the layout flow, spans the full
//! width, and stays until the user deals with it. That difference is not a timer
//! setting: nothing in this control hides itself, because a notice the user has
//! not acknowledged simply is not acknowledged. The two also differ in shape —
//! a banner carries a severity, a dismiss control the user must click, and any
//! number of actions.
//!
//! # Reachability
//!
//! This is a new control, so it is registered in the widget factory (and
//! therefore reachable by name from the declarative path and CSS selectors),
//! publishes a property contract, and has interaction tests.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Side of the square dismiss hit area, in logical pixels.
const DISMISS_SIZE: u32 = 24;
/// Horizontal padding between the banner edge and the dismiss area.
const EDGE_PADDING: u32 = 8;
/// Horizontal gap between two adjacent action hit areas.
const ACTION_GAP: u32 = 8;
/// Width of one action hit area, in logical pixels.
const ACTION_WIDTH: u32 = 72;

/// Severity of a banner.
///
/// The severity is the whole reason a banner needs no icon: it selects the
/// palette the bar is painted with, so the tone of the message is readable
/// before the text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerSeverity {
    /// Neutral, informational.
    #[default]
    Info,
    /// A completed operation.
    Success,
    /// Something that needs attention but is not a failure.
    Warning,
    /// A failure.
    Error,
}

impl BannerSeverity {
    /// The lower-case token used by the property contract.
    ///
    /// Enumerated values travel as their token spelling, so this is the single
    /// place the spelling is decided for both reading and writing.
    pub fn token(self) -> &'static str {
        match self {
            BannerSeverity::Info => "info",
            BannerSeverity::Success => "success",
            BannerSeverity::Warning => "warning",
            BannerSeverity::Error => "error",
        }
    }

    /// Parses a token produced by [`BannerSeverity::token`].
    ///
    /// An unknown token is rejected rather than defaulted, because silently
    /// accepting it would make a typo in a stylesheet change the severity back to
    /// `Info` without any indication.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "info" => Some(BannerSeverity::Info),
            "success" => Some(BannerSeverity::Success),
            "warning" => Some(BannerSeverity::Warning),
            "error" => Some(BannerSeverity::Error),
            _ => None,
        }
    }

    /// The background the bar is filled with.
    fn background(self) -> Color {
        match self {
            BannerSeverity::Info => Color::rgb(219, 229, 249),
            BannerSeverity::Success => Color::rgb(214, 239, 221),
            BannerSeverity::Warning => Color::rgb(252, 234, 205),
            BannerSeverity::Error => Color::rgb(250, 219, 219),
        }
    }

    /// The colour of the bar's border, a darker shade of the background so the
    /// bar separates from the surface behind it.
    fn border(self) -> Color {
        match self {
            BannerSeverity::Info => Color::rgb(157, 184, 230),
            BannerSeverity::Success => Color::rgb(155, 210, 170),
            BannerSeverity::Warning => Color::rgb(226, 183, 106),
            BannerSeverity::Error => Color::rgb(226, 158, 158),
        }
    }

    /// The default text colour on top of [`BannerSeverity::background`]. Used
    /// only when the theme does not supply one.
    fn default_text(self) -> Color {
        match self {
            BannerSeverity::Info => Color::rgb(31, 51, 88),
            BannerSeverity::Success => Color::rgb(24, 69, 39),
            BannerSeverity::Warning => Color::rgb(94, 63, 12),
            BannerSeverity::Error => Color::rgb(104, 29, 29),
        }
    }
}

/// A persistent full-width notice with a severity, a message, an optional
/// dismiss control, and optional actions.
pub struct Banner {
    base: BaseWidget,
    text: String,
    severity: BannerSeverity,
    /// Whether the control offers a dismiss affordance at all. A non-dismissible
    /// banner `dismiss()`es to `false` and keeps reporting `false`, rather than
    /// hiding, because a notice with no dismiss control is one the user is
    /// expected to resolve another way.
    dismissible: bool,
    /// Whether the user has acknowledged the notice. Named distinctly from the
    /// `dismissed` signal so the two do not collide on the struct.
    is_dismissed: bool,
    actions: Vec<String>,
    /// Index of the action under the pointer, tracked so a hover can be
    /// highlighted without re-deriving the hit test in the drawing code.
    hovered_action: Option<usize>,
    /// Emitted with the action's label when an action is activated.
    pub action_clicked: Signal1<String>,
    /// Emitted when `dismiss()` succeeds. Not emitted for a rejected dismiss, so
    /// a listener only ever observes acknowledgements that actually happened.
    pub dismissed: GenericSignal,
}

impl Banner {
    /// Creates a dismissible `Info` banner with no text and no actions.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Banner, geometry, "Banner"),
            text: String::new(),
            severity: BannerSeverity::Info,
            dismissible: true,
            is_dismissed: false,
            actions: Vec::new(),
            hovered_action: None,
            action_clicked: Signal1::new(),
            dismissed: GenericSignal::new(),
        }
    }

    /// Returns the message.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the message.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }

    /// Returns the severity.
    pub fn severity(&self) -> BannerSeverity {
        self.severity
    }

    /// Sets the severity, which selects the bar's palette.
    pub fn set_severity(&mut self, severity: BannerSeverity) {
        self.severity = severity;
        self.base.request_redraw();
    }

    /// Returns whether the control offers a dismiss affordance.
    pub fn dismissible(&self) -> bool {
        self.dismissible
    }

    /// Sets whether the control offers a dismiss affordance.
    pub fn set_dismissible(&mut self, dismissible: bool) {
        self.dismissible = dismissible;
        self.base.request_redraw();
    }

    /// Returns whether the banner has been acknowledged.
    pub fn is_dismissed(&self) -> bool {
        self.is_dismissed
    }

    /// Dismisses the banner, returning whether it was dismissed by this call.
    ///
    /// Returns `false` for a non-dismissible banner and for one that is already
    /// dismissed, so the caller can tell an acknowledgement that happened from
    /// one that was rejected.
    pub fn dismiss(&mut self) -> bool {
        if !self.dismissible || self.is_dismissed {
            return false;
        }
        self.is_dismissed = true;
        self.dismissed_signal();
        self.base.request_redraw();
        true
    }

    /// Un-dismisses the banner so it can be shown again.
    ///
    /// This is what makes a banner reusable: a caller that owns one banner for a
    /// status area resolves a new message into it instead of rebuilding it.
    pub fn show(&mut self) {
        self.is_dismissed = false;
        self.hovered_action = None;
        self.base.request_redraw();
    }

    /// Returns the action labels.
    pub fn actions(&self) -> &[String] {
        &self.actions
    }

    /// Sets the action labels. Any previously hovered index is dropped because
    /// it may no longer name the same action.
    pub fn set_actions(&mut self, actions: Vec<String>) {
        self.actions = actions;
        self.hovered_action = None;
        self.base.request_redraw();
    }

    /// Number of actions.
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// Activates the action at `index`, returning whether it was activated.
    ///
    /// Returns `false` for an out-of-range index and for any action on a
    /// dismissed banner, so a stale index or a click that raced a dismissal
    /// cannot fire a signal for a button the user can no longer see.
    pub fn activate_action(&mut self, index: usize) -> bool {
        if self.is_dismissed {
            return false;
        }
        let Some(label) = self.actions.get(index).cloned() else {
            return false;
        };
        self.action_clicked.emit(label);
        true
    }

    /// The dismiss hit area, or `None` when the control offers no dismiss
    /// affordance or the banner is already dismissed and would draw nothing.
    pub fn dismiss_action_rect(&self) -> Option<Rect> {
        if !self.dismissible || self.is_dismissed {
            return None;
        }
        let rect = self.geometry();
        if rect.width < DISMISS_SIZE + EDGE_PADDING || rect.height < DISMISS_SIZE {
            return None;
        }
        Some(Rect::new(
            rect.x + rect.width as i32 - (DISMISS_SIZE + EDGE_PADDING) as i32,
            rect.y + (rect.height as i32 - DISMISS_SIZE as i32) / 2,
            DISMISS_SIZE,
            DISMISS_SIZE,
        ))
    }

    /// The hit area of the action at `index`, or `None` when there is no such
    /// action.
    ///
    /// Actions are laid out right to left so that index 0 sits nearest the
    /// dismiss control, which is the order Material places them in: the primary
    /// action closest to where the eye already is.
    pub fn action_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.actions.len() {
            return None;
        }
        let rect = self.geometry();
        let height = DISMISS_SIZE.min(rect.height);
        if height == 0 {
            return None;
        }
        let reserved = if self.dismissible { DISMISS_SIZE + EDGE_PADDING } else { EDGE_PADDING };
        let right = rect.x + rect.width as i32 - reserved as i32;
        let step = (ACTION_WIDTH + ACTION_GAP) as i32;
        let x = right - step * (index as i32 + 1) + ACTION_GAP as i32;
        Some(Rect::new(x, rect.y + (rect.height as i32 - height as i32) / 2, ACTION_WIDTH, height))
    }

    /// Emits the dismissal signal.
    ///
    /// Split out because the field and the method share a name, and the signal
    /// cannot be reached unqualified from inside the method body.
    fn dismissed_signal(&self) {
        self.dismissed.emit();
    }

    /// Returns the action index under `pos`, if any.
    fn action_index_at(&self, pos: Point) -> Option<usize> {
        if self.is_dismissed {
            return None;
        }
        (0..self.actions.len())
            .find(|index| self.action_rect(*index).is_some_and(|rect| point_in_rect(pos, rect)))
    }

    /// Returns whether `pos` lies in the dismiss area.
    fn hits_dismiss(&self, pos: Point) -> bool {
        self.dismiss_action_rect().is_some_and(|rect| point_in_rect(pos, rect))
    }
}

/// Returns whether `pos` lies inside `rect`.
fn point_in_rect(pos: Point, rect: Rect) -> bool {
    pos.x >= rect.x
        && pos.x < rect.x + rect.width as i32
        && pos.y >= rect.y
        && pos.y < rect.y + rect.height as i32
}

impl Widget for Banner {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// A full-width bar, so the width hint is the largest a caller is likely to
    /// give it while the height is one text row plus padding.
    fn size_hint(&self) -> Size {
        Size::new(360, 48)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Banner`'s property contract.
///
/// `severity` round-trips as its lower-case token so a stylesheet can name it.
/// `dismissed` and `action_count` are readable but not writable: the first is
/// the record of an acknowledgement the user made, and the second is derived
/// from the action list. Writing either would let a caller report state the
/// control never entered.
impl WidgetProperties for Banner {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "severity" => Ok(CapabilityValue::String(self.severity().token().to_string())),
            "dismissible" => Ok(CapabilityValue::Bool(self.dismissible())),
            "dismissed" => Ok(CapabilityValue::Bool(self.is_dismissed())),
            "action_count" => Ok(CapabilityValue::UInt(self.action_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "severity" => {
                let token = expect_string(value)?;
                let severity = BannerSeverity::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_severity(severity);
                Ok(())
            }
            "dismissible" => {
                self.set_dismissible(expect_bool(value)?);
                Ok(())
            }
            // A record of what the user did, and a derived count.
            "dismissed" | "action_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "severity",
            "dismissible",
            "dismissed",
            "action_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `banner` publishes.
    ///
    /// `show` maps onto the widget's real `show`, re-showing the same control
    /// for a new message. `dismiss` runs the real `dismiss`, which reports
    /// `false` when the banner is already acknowledged or offers no dismiss
    /// affordance, so that case is answered as needing one rather than as a
    /// successful acknowledgement. `set_actions` and `activate_action` need the
    /// labels and the action index respectively.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "show" => {
                self.show();
                Ok(())
            }
            "dismiss" => {
                if self.dismiss() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "set_actions" | "activate_action" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Banner {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || self.is_dismissed {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if self.hits_dismiss(*pos) {
                    let _ = self.dismiss();
                    return;
                }
                if let Some(index) = self.action_index_at(*pos) {
                    let _ = self.activate_action(index);
                }
            }
            Event::MouseMove { pos } => {
                self.hovered_action = self.action_index_at(*pos);
            }
            // The pointer is no longer over any action, so nothing should stay
            // highlighted.
            Event::MouseLeave { .. } => {
                self.hovered_action = None;
            }
            _ => {}
        }
    }
}

impl Draw for Banner {
    fn draw(&mut self, context: &mut RenderContext) {
        // A dismissed banner is gone: drawing a backdrop or a border here would
        // leave a strip of the notice behind after the user acknowledged it.
        if self.is_dismissed() {
            return;
        }
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let style = self.base.style().clone();
        let background = self.severity.background();
        let text_color = style.text_color.unwrap_or_else(|| self.severity.default_text());
        let border_color = style.border_color.unwrap_or_else(|| self.severity.border());

        context.fill_rect(rect, background);

        // The dismiss control is the right-most element, so the text stops short
        // of it and of any actions, otherwise a long message would run under the
        // controls the user is meant to click.
        let controls_left = self.controls_left_edge();
        let text_width = (controls_left - (rect.x + EDGE_PADDING as i32)).max(0) as u32;
        if text_width > 0 {
            let font = Font::simple("Sans", (rect.height as f32 * 0.3).clamp(10.0, 16.0));
            context.draw_text(
                Point::new(rect.x + EDGE_PADDING as i32, rect.y + (rect.height as i32) / 2),
                &self.text,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        for (index, label) in self.actions.iter().enumerate() {
            let Some(action_rect) = self.action_rect(index) else {
                continue;
            };
            let hovered = self.hovered_action == Some(index);
            if hovered {
                context.fill_rect(action_rect, background.blend(&text_color, 0.16));
            }
            context.draw_rect(action_rect, border_color);
            context.draw_text(
                Point::new(action_rect.x + (action_rect.width as i32) / 2, action_rect.y + 16),
                label,
                &Font::simple("Sans", 12.0),
                text_color,
                HorizontalAlignment::Center,
            );
        }

        if let Some(dismiss_rect) = self.dismiss_action_rect() {
            context.draw_rect(dismiss_rect, border_color);
            // Drawn as two strokes rather than a glyph so the affordance does not
            // depend on the font containing a multiplication sign.
            let inset = 7;
            let left = dismiss_rect.x + inset;
            let right = dismiss_rect.x + dismiss_rect.width as i32 - inset;
            let top = dismiss_rect.y + inset;
            let bottom = dismiss_rect.y + dismiss_rect.height as i32 - inset;
            context.draw_line(Point::new(left, top), Point::new(right, bottom), text_color);
            context.draw_line(Point::new(right, top), Point::new(left, bottom), text_color);
        }

        context.draw_rect(rect, border_color);
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

impl Banner {
    /// The left edge of the control strip on the right of the bar: the left edge
    /// of the left-most action when there are actions, otherwise of the dismiss
    /// area, otherwise the right padding.
    fn controls_left_edge(&self) -> i32 {
        if !self.actions.is_empty() {
            if let Some(last) = self.action_rect(self.actions.len() - 1) {
                return last.x - ACTION_GAP as i32;
            }
        }
        match self.dismiss_action_rect() {
            Some(rect) => rect.x - EDGE_PADDING as i32,
            None => self.geometry().x + self.geometry().width as i32 - EDGE_PADDING as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn banner() -> Banner {
        Banner::new(Rect::new(0, 0, 400, 48))
    }

    /// A collection point for signal payloads, so a test can assert on what a
    /// listener actually observed rather than on internal state.
    fn action_sink(banner: &mut Banner) -> Arc<Mutex<Vec<String>>> {
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&seen);
        banner.action_clicked.connect(move |label| {
            sink.lock().expect("signal sink poisoned").push(label.as_ref().clone());
        });
        seen
    }

    fn dismiss_sink(banner: &mut Banner) -> Arc<Mutex<u32>> {
        let count = Arc::new(Mutex::new(0u32));
        let sink = Arc::clone(&count);
        banner.dismissed.connect(move || {
            *sink.lock().expect("signal sink poisoned") += 1;
        });
        count
    }

    /// Dismissal must be refused when the control offers no dismiss affordance,
    /// and the refused call must not record a dismissal the user never made.
    #[test]
    fn dismiss_is_refused_when_not_dismissible() {
        let mut banner = banner();
        banner.set_dismissible(false);

        assert!(!banner.dismiss(), "a non-dismissible banner cannot be dismissed");
        assert!(!banner.is_dismissed(), "a refused dismiss must not change state");
    }

    /// A successful dismissal must set the flag and announce it exactly once.
    #[test]
    fn dismiss_succeeds_and_emits() {
        let mut banner = banner();
        let count = dismiss_sink(&mut banner);

        assert!(banner.dismiss());
        assert!(banner.is_dismissed());
        assert_eq!(*count.lock().expect("signal sink poisoned"), 1);
    }

    /// A second dismissal is not a new acknowledgement, so it must report
    /// refusal and must not re-announce the first one.
    #[test]
    fn dismiss_twice_reports_refusal_the_second_time() {
        let mut banner = banner();
        let count = dismiss_sink(&mut banner);

        assert!(banner.dismiss());
        assert!(!banner.dismiss(), "an already dismissed banner cannot be dismissed again");
        assert!(banner.is_dismissed());
        assert_eq!(
            *count.lock().expect("signal sink poisoned"),
            1,
            "the dismissal must be announced once, not once per call"
        );
    }

    /// Reuse depends on being able to bring a dismissed banner back.
    #[test]
    fn show_clears_the_dismissed_state() {
        let mut banner = banner();
        banner.dismiss();
        assert!(banner.is_dismissed());

        banner.show();
        assert!(!banner.is_dismissed(), "show must un-dismiss");

        // And the revived banner must be dismissible again.
        assert!(banner.dismiss());
    }

    /// The dismiss rect is what the user clicks, so a press inside it must
    /// dismiss the banner.
    #[test]
    fn clicking_the_dismiss_rect_dismisses() {
        let mut banner = banner();
        let rect =
            banner.dismiss_action_rect().expect("a dismissible banner offers a dismiss area");

        banner.handle_event(&Event::mouse_press(rect.x + 2, rect.y + 2, 1));
        assert!(banner.is_dismissed());
    }

    /// Clicking an action must emit that action's label, which is the whole
    /// contract of the signal.
    #[test]
    fn clicking_an_action_emits_its_label() {
        let mut banner = banner();
        banner.set_actions(vec!["Retry".to_string(), "Details".to_string()]);
        let seen = action_sink(&mut banner);

        let rect = banner.action_rect(0).expect("action 0 has a rect");
        banner.handle_event(&Event::mouse_press(rect.x + 2, rect.y + 2, 1));

        let got = seen.lock().expect("signal sink poisoned").clone();
        assert_eq!(got, vec!["Retry".to_string()]);
        assert!(!banner.is_dismissed(), "activating an action is not a dismissal");
    }

    /// Each action rect must map to its own action, otherwise the labels and the
    /// buttons would disagree.
    #[test]
    fn each_action_rect_maps_to_its_own_action() {
        let mut banner = banner();
        banner.set_actions(vec!["First".to_string(), "Second".to_string(), "Third".to_string()]);
        let seen = action_sink(&mut banner);

        let rect = banner.action_rect(0).expect("action 0 has a rect");
        let second = banner.action_rect(1).expect("action 1 has a rect");
        assert_ne!(rect, second, "two actions must not share a hit area");

        banner.handle_event(&Event::mouse_press(second.x + 2, second.y + 2, 1));
        let got = seen.lock().expect("signal sink poisoned").clone();
        assert_eq!(got, vec!["Second".to_string()]);
        assert_eq!(banner.action_rect(3), None, "there is no fourth action");
    }

    /// A dismissed banner draws nothing and must therefore react to nothing —
    /// otherwise an invisible button could still fire.
    #[test]
    fn a_dismissed_banner_ignores_clicks() {
        let mut banner = banner();
        banner.set_actions(vec!["Retry".to_string()]);
        let seen = action_sink(&mut banner);
        let action = banner.action_rect(0).expect("action 0 has a rect");
        // Captured before the dismissal, because a dismissed banner publishes no
        // rects at all.
        let dismiss = banner.dismiss_action_rect().expect("a dismiss area exists");

        banner.dismiss();

        banner.handle_event(&Event::mouse_press(action.x + 2, action.y + 2, 1));
        banner.handle_event(&Event::mouse_press(dismiss.x + 2, dismiss.y + 2, 1));

        assert!(banner.is_dismissed());
        assert!(
            seen.lock().expect("signal sink poisoned").is_empty(),
            "a dismissed banner must not activate an action"
        );
        assert_eq!(banner.dismiss_action_rect(), None, "nothing is drawn, so nothing is clickable");
    }

    /// An out-of-range index must be refused rather than panicking or silently
    /// firing the last action.
    #[test]
    fn activate_action_rejects_an_out_of_range_index() {
        let mut banner = banner();
        banner.set_actions(vec!["Only".to_string()]);
        let seen = action_sink(&mut banner);

        assert!(!banner.activate_action(1), "index 1 does not exist");
        assert!(!banner.activate_action(usize::MAX));
        assert!(banner.activate_action(0));
        assert_eq!(banner.action_count(), 1);

        let got = seen.lock().expect("signal sink poisoned").clone();
        assert_eq!(got, vec!["Only".to_string()], "only the valid activation may emit");
    }

    /// A click that misses every control must be inert, otherwise the banner
    /// would dismiss itself when the user clicked its text.
    #[test]
    fn a_click_away_from_any_control_does_nothing() {
        let mut banner = banner();
        banner.set_actions(vec!["Retry".to_string()]);
        let seen = action_sink(&mut banner);
        let count = dismiss_sink(&mut banner);

        banner.handle_event(&Event::mouse_press(4, 24, 1));

        assert!(!banner.is_dismissed(), "clicking the message is not a dismissal");
        assert!(seen.lock().expect("signal sink poisoned").is_empty());
        assert_eq!(*count.lock().expect("signal sink poisoned"), 0);
    }

    /// Only the left button activates controls; a right-click is a context-menu
    /// gesture and must not dismiss a notice.
    #[test]
    fn only_the_left_button_activates_controls() {
        let mut banner = banner();
        let rect = banner.dismiss_action_rect().expect("a dismiss area exists");

        banner.handle_event(&Event::mouse_press(rect.x + 2, rect.y + 2, 3));
        assert!(!banner.is_dismissed());
    }

    /// A non-dismissible banner must not reserve the dismiss column, so its
    /// actions shift right into the space and no invisible button is left over.
    #[test]
    fn a_non_dismissible_banner_offers_no_dismiss_area() {
        let mut banner = banner();
        banner.set_actions(vec!["Retry".to_string()]);
        assert!(banner.dismiss_action_rect().is_some());

        banner.set_dismissible(false);
        assert_eq!(banner.dismiss_action_rect(), None);
        let action = banner.action_rect(0).expect("the action is still laid out");
        assert!(
            action.x + action.width as i32 <= banner.geometry().x + banner.geometry().width as i32,
            "an action must stay inside the bar"
        );
    }

    /// The severity must survive a write and a read through the property
    /// contract using the documented tokens.
    #[test]
    fn severity_round_trips_through_the_property_contract() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };

        for severity in [
            BannerSeverity::Info,
            BannerSeverity::Success,
            BannerSeverity::Warning,
            BannerSeverity::Error,
        ] {
            let mut banner = banner();
            let token = severity.token();
            widget_property_set(
                &mut banner,
                "severity",
                CapabilityValue::String(token.to_string()),
            )
            .unwrap_or_else(|_| panic!("{token} must be accepted"));
            assert_eq!(banner.severity(), severity);
            assert_eq!(
                widget_property_get(&banner, "severity"),
                Ok(CapabilityValue::String(token.to_string()))
            );
        }

        let mut banner = banner();
        assert_eq!(
            widget_property_set(
                &mut banner,
                "severity",
                CapabilityValue::String("loud".to_string())
            ),
            Err(CapabilityAccessError::TypeMismatch),
            "an unknown token must be refused rather than defaulted"
        );
        assert_eq!(banner.severity(), BannerSeverity::Info);
    }

    /// The rest of the contract: the writable names must round-trip and the
    /// derived ones must be refused.
    #[test]
    fn properties_round_trip() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };

        let mut banner = banner();
        widget_property_set(&mut banner, "text", CapabilityValue::String("Saved".to_string()))
            .expect("text");
        assert_eq!(
            widget_property_get(&banner, "text"),
            Ok(CapabilityValue::String("Saved".to_string()))
        );

        widget_property_set(&mut banner, "dismissible", CapabilityValue::Bool(false))
            .expect("dismissible");
        assert_eq!(widget_property_get(&banner, "dismissible"), Ok(CapabilityValue::Bool(false)));

        banner.dismiss();
        assert_eq!(widget_property_get(&banner, "dismissed"), Ok(CapabilityValue::Bool(false)));

        // Read-only names must be refused rather than silently accepted.
        assert_eq!(
            widget_property_set(&mut banner, "dismissed", CapabilityValue::Bool(true)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
        assert_eq!(
            widget_property_set(&mut banner, "action_count", CapabilityValue::UInt(3)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
        assert_eq!(widget_property_get(&banner, "action_count"), Ok(CapabilityValue::UInt(0)));
    }

    /// The default state is what `new` promises: informational, dismissible, and
    /// undismissed so the banner is usable without any configuration.
    #[test]
    fn a_new_banner_is_an_undismissed_info_banner() {
        let banner = banner();
        assert_eq!(banner.severity(), BannerSeverity::Info);
        assert_eq!(BannerSeverity::default(), BannerSeverity::Info);
        assert!(banner.dismissible());
        assert!(!banner.is_dismissed());
        assert_eq!(banner.text(), "");
        assert_eq!(banner.action_count(), 0);
        assert!(banner.actions().is_empty());
        assert_eq!(banner.action_rect(0), None, "there are no actions to lay out");
    }

    /// Actions are owned by the control, so a caller cannot read back a label it
    /// never set — and a shorter list must drop the hover that referred to the
    /// old one.
    #[test]
    fn set_actions_replaces_the_action_list() {
        let mut banner = banner();
        banner.set_actions(vec!["A".to_string(), "B".to_string()]);
        banner.handle_event(&Event::mouse_move(0, 0));
        assert_eq!(banner.action_count(), 2);

        banner.set_actions(vec!["C".to_string()]);
        assert_eq!(banner.action_count(), 1);
        assert_eq!(banner.actions(), ["C".to_string()]);
        assert_eq!(banner.action_rect(1), None);
        assert!(banner.action_rect(0).is_some());
    }
}
