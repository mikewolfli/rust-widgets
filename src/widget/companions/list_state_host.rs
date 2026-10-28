//! `ListStateHost` — the shared idle/loading/empty/error surface for list views (COMP-A33).
//!
//! # Why a companion
//!
//! Every list-like control eventually needs the four states that are not rows: nothing has
//! been loaded yet, a refresh is in flight, the source is empty, or loading failed. Each widget
//! used to paint its own version — a different skeleton, a different empty message, a different
//! retry hint — so a product with three lists showed three visual languages for the same four
//! facts. This host owns that surface once.
//!
//! The host does not own the list. It owns only the state and the four strings that describe
//! it, and it reports the two intents a state surface can have: [`Self::refresh_requested`]
//! when an error asks to try again, and [`Self::empty_action_activated`] when the empty state's
//! call to action is triggered. The host applies them to its own loader.

use crate::compat::{String, ToString};
use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::key_codes::{ENTER, LINE_FEED, SPACE};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal};
use crate::widget::capability::coercion::{expect_f32, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::icon_source::{IconSource, IconSpec};
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// Height of one loading skeleton row.
const SKELETON_HEIGHT: u32 = 12;
/// Vertical space between two skeleton rows.
const SKELETON_GAP: u32 = 8;
/// Top and bottom inset of the skeleton list.
const SKELETON_TOP: u32 = 12;
/// Horizontal inset of the skeleton list.
const SKELETON_INSET: u32 = 12;
/// Size of the empty-state icon badge.
const ICON_BADGE: i32 = 32;

/// The four states a list surface can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListState {
    /// Nothing has been asked for yet; the host paints nothing.
    #[default]
    Idle,
    /// A refresh is in flight; the host paints skeleton rows.
    Loading,
    /// The source is empty; the host paints the empty content.
    Empty,
    /// Loading failed; the host paints the error text.
    Error,
}

impl ListState {
    /// The token used by the property contract.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Error => "error",
        }
    }

    /// Parses the token used by the property contract.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "idle" => Some(Self::Idle),
            "loading" => Some(Self::Loading),
            "empty" => Some(Self::Empty),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Clamps a pull-to-refresh progress into `0.0..=1.0`; a non-finite value becomes zero.
fn normalized_progress(progress: f32) -> f32 {
    if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A state surface for a list that is idle, loading, empty or failed.
///
/// The widget paints the state but never advances it: a host drives [`Self::set_state`] from
/// its loader. That split is what lets the same surface stand in for a network list, a local
/// query and a file scan without knowing any of them.
pub struct ListStateHost {
    base: BaseWidget,
    state: ListState,
    empty_icon: Option<IconSource>,
    empty_title: String,
    empty_hint: String,
    error_text: String,
    loading_rows: usize,
    pull_progress: f32,
    /// Emitted when an error surface asks to retry.
    pub refresh_requested: GenericSignal,
    /// Emitted when the empty surface's call to action is triggered.
    pub empty_action_activated: GenericSignal,
}

impl ListStateHost {
    /// Creates an idle host with no empty or error content.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::EmptyState, geometry, "ListStateHost"),
            state: ListState::Idle,
            empty_icon: None,
            empty_title: String::new(),
            empty_hint: String::new(),
            error_text: String::new(),
            loading_rows: 0,
            pull_progress: 0.0,
            refresh_requested: GenericSignal::new(),
            empty_action_activated: GenericSignal::new(),
        }
    }

    /// Switches the surface's state.
    ///
    /// The stored pull progress is clamped as part of every transition, so a host that leaves
    /// a mid-gesture value behind cannot present an out-of-range progress after the state
    /// changed underneath it.
    pub fn set_state(&mut self, state: ListState) {
        let progress = normalized_progress(self.pull_progress);
        let changed = self.state != state || self.pull_progress != progress;
        self.state = state;
        self.pull_progress = progress;
        if changed {
            self.base.request_redraw();
        }
    }

    /// Replaces the content painted by the empty state.
    ///
    /// The icon is copied rather than referenced: [`IconSource`] is `Copy` and owns no
    /// resources, so the host keeps no borrow of the caller's icon registry.
    pub fn set_empty_content(
        &mut self,
        icon: Option<IconSource>,
        title: impl Into<String>,
        hint: impl Into<String>,
    ) {
        let title = title.into();
        let hint = hint.into();
        if self.empty_icon == icon && self.empty_title == title && self.empty_hint == hint {
            return;
        }
        self.empty_icon = icon;
        self.empty_title = title;
        self.empty_hint = hint;
        self.base.request_redraw();
    }

    /// Replaces only the empty-state icon.
    pub fn set_empty_icon(&mut self, icon: Option<IconSource>) {
        if self.empty_icon == icon {
            return;
        }
        self.empty_icon = icon;
        self.base.request_redraw();
    }

    /// Sets the failure text and switches the surface to [`ListState::Error`].
    ///
    /// Setting the error content is the operation that makes the error visible, so it also
    /// performs the transition. A host that wants to stage the text before showing it can use
    /// [`Self::set_state`] afterwards.
    pub fn set_error(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.error_text == text && self.state == ListState::Error {
            return;
        }
        self.error_text = text;
        self.state = ListState::Error;
        self.base.request_redraw();
    }

    /// Sets how many skeleton rows the loading state paints.
    pub fn set_loading_rows(&mut self, rows: usize) {
        if self.loading_rows == rows {
            return;
        }
        self.loading_rows = rows;
        self.base.request_redraw();
    }

    /// Sets the pull-to-refresh progress, clamped into `0.0..=1.0`.
    pub fn set_pull_progress(&mut self, progress: f32) {
        let progress = normalized_progress(progress);
        if self.pull_progress == progress {
            return;
        }
        self.pull_progress = progress;
        self.base.request_redraw();
    }

    /// Returns the current state.
    pub fn state(&self) -> ListState {
        self.state
    }

    /// Returns the empty-state icon, if any.
    pub fn empty_icon(&self) -> Option<IconSource> {
        self.empty_icon
    }

    /// Returns the empty-state title.
    pub fn empty_title(&self) -> &str {
        &self.empty_title
    }

    /// Returns the empty-state hint.
    pub fn empty_hint(&self) -> &str {
        &self.empty_hint
    }

    /// Returns the failure text.
    pub fn error_text(&self) -> &str {
        &self.error_text
    }

    /// Returns the number of skeleton rows the loading state asks for.
    pub fn loading_rows(&self) -> usize {
        self.loading_rows
    }

    /// Returns the pull-to-refresh progress.
    pub fn pull_progress(&self) -> f32 {
        self.pull_progress
    }

    /// Paints the loading state's skeleton list.
    fn draw_loading(&self, context: &mut RenderContext, rect: Rect) {
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        context.fill_rect(rect, surface);

        let pitch = SKELETON_HEIGHT + SKELETON_GAP;
        let usable = rect.height.saturating_sub(SKELETON_TOP * 2);
        let max_rows = (usable / pitch).max(1) as usize;
        let rows = self.loading_rows.min(max_rows);
        let available = rect.width.saturating_sub(SKELETON_INSET * 2);
        let mut y = rect.y.saturating_add(SKELETON_TOP as i32);

        for index in 0..rows {
            if y.saturating_add(SKELETON_HEIGHT as i32) > rect.bottom() {
                break;
            }
            let width = match index % 3 {
                0 => available.saturating_sub(available / 4),
                1 => available,
                _ => available.saturating_sub(available / 2),
            };
            let row =
                Rect::new(rect.x.saturating_add(SKELETON_INSET as i32), y, width, SKELETON_HEIGHT);
            context.fill_rounded_rect(row, 4, surface.blend(&ink, 0.09));
            y = y.saturating_add(pitch as i32);
        }
    }

    /// Paints the empty state's icon, title and hint.
    fn draw_empty(&self, context: &mut RenderContext, rect: Rect) {
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        context.fill_rect(rect, surface);

        let mut top = rect.y.saturating_add(rect.height as i32 / 2 - 40);
        if let Some(source) = self.empty_icon {
            top = paint_icon_badge(
                context,
                rect.x.saturating_add(rect.width as i32 / 2),
                top,
                source,
                accent,
                surface,
                ink,
            );
            top = top.saturating_add(8);
        }

        let title = if self.empty_title.is_empty() {
            "Nothing here yet"
        } else {
            self.empty_title.as_str()
        };
        let title_rect =
            Rect::new(rect.x.saturating_add(16), top, rect.width.saturating_sub(32), 22);
        context.draw_text_fitted_cow(title_rect, title, font, ink, HorizontalAlignment::Center);

        if !self.empty_hint.is_empty() {
            let hint_rect = Rect::new(
                rect.x.saturating_add(16),
                top.saturating_add(26),
                rect.width.saturating_sub(32),
                18,
            );
            context.draw_text_fitted_cow(
                hint_rect,
                &self.empty_hint,
                font,
                surface.blend(&ink, 0.60),
                HorizontalAlignment::Center,
            );
        }
    }

    /// Paints the error state's message and retry hint.
    fn draw_error(&self, context: &mut RenderContext, rect: Rect) {
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let font = effective_font(style);
        context.fill_rect(rect, surface);

        let text = if self.error_text.is_empty() {
            "Something went wrong"
        } else {
            self.error_text.as_str()
        };
        let text_rect = Rect::new(
            rect.x.saturating_add(16),
            rect.y.saturating_add(rect.height as i32 / 2 - 12),
            rect.width.saturating_sub(32),
            24,
        );
        context.draw_text_fitted_cow(
            text_rect,
            text,
            font,
            Color::ERROR,
            HorizontalAlignment::Center,
        );

        let hint_rect = Rect::new(
            rect.x.saturating_add(16),
            text_rect.bottom().saturating_add(8),
            rect.width.saturating_sub(32),
            18,
        );
        context.draw_text_fitted_cow(
            hint_rect,
            "Press Enter or click to retry",
            font,
            surface.blend(&ink, 0.60),
            HorizontalAlignment::Center,
        );
    }
}

/// Paints an empty-state icon badge and returns the y coordinate below it.
fn paint_icon_badge(
    context: &mut RenderContext,
    center_x: i32,
    top: i32,
    source: IconSource,
    accent: Color,
    surface: Color,
    ink: Color,
) -> i32 {
    let tint = match source {
        IconSource::Builtin(_) => accent,
        IconSource::StaticGeometry(_) => ink,
    };
    let badge = Rect::new(
        center_x.saturating_sub(ICON_BADGE / 2),
        top,
        ICON_BADGE as u32,
        ICON_BADGE as u32,
    );
    context.fill_rounded_rect(badge, 8, surface.blend(&tint, 0.16));
    context.draw_rect(badge, surface.blend(&tint, 0.45));
    let glyph = Rect::new(
        badge.x.saturating_add(9),
        badge.y.saturating_add(9),
        (ICON_BADGE - 18) as u32,
        (ICON_BADGE - 18) as u32,
    );
    context.fill_rounded_rect(glyph, 3, tint);
    badge.bottom()
}

impl EventHandler for ListStateHost {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        let activated = match event {
            Event::MouseRelease { pos, button: 1, .. } => self.geometry().contains_point(*pos),
            Event::KeyPress { key, .. } => *key == ENTER || *key == LINE_FEED || *key == SPACE,
            _ => false,
        };
        if !activated {
            return;
        }

        match self.state {
            ListState::Empty => self.empty_action_activated.emit(),
            ListState::Error => self.refresh_requested.emit(),
            ListState::Idle | ListState::Loading => {}
        }
    }
}

impl Draw for ListStateHost {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        match self.state {
            ListState::Idle => {}
            ListState::Loading => self.draw_loading(context, rect),
            ListState::Empty => self.draw_empty(context, rect),
            ListState::Error => self.draw_error(context, rect),
        }
    }
}

impl Widget for ListStateHost {
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
        crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            Size::new(320, 240),
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "refresh_requested" => {
                Some(EventSignalRef::unit("refresh_requested", &self.refresh_requested))
            }
            "empty_action_activated" => {
                Some(EventSignalRef::unit("empty_action_activated", &self.empty_action_activated))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ListStateHost {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "state" => Ok(CapabilityValue::String(self.state.as_str().to_string())),
            "empty_icon" => Ok(match self.empty_icon {
                Some(IconSource::Builtin(name)) => {
                    CapabilityValue::IconSpec(IconSpec::Builtin(name))
                }
                _ => CapabilityValue::Null,
            }),
            "empty_title" => Ok(CapabilityValue::String(self.empty_title.clone())),
            "empty_hint" => Ok(CapabilityValue::String(self.empty_hint.clone())),
            "error_text" => Ok(CapabilityValue::String(self.error_text.clone())),
            "loading_rows" => Ok(CapabilityValue::UInt(self.loading_rows as u64)),
            "pull_progress" => Ok(CapabilityValue::Float(f64::from(self.pull_progress))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "state" => {
                let token = expect_string(value)?;
                self.set_state(
                    ListState::from_name(&token).ok_or(CapabilityAccessError::TypeMismatch)?,
                );
                Ok(())
            }
            "empty_icon" => match value {
                CapabilityValue::Null => {
                    self.set_empty_icon(None);
                    Ok(())
                }
                CapabilityValue::IconSpec(spec) => match spec.builtin_source() {
                    Some(source) => {
                        self.set_empty_icon(Some(source));
                        Ok(())
                    }
                    // Static geometry cannot be reconstructed from a persisted descriptor, so
                    // accepting the write would silently replace the caller's icon with nothing.
                    None => Err(CapabilityAccessError::TypeMismatch),
                },
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "empty_title" => {
                let title = expect_string(value)?;
                let hint = self.empty_hint.clone();
                self.set_empty_content(self.empty_icon, title, hint);
                Ok(())
            }
            "empty_hint" => {
                let title = self.empty_title.clone();
                let hint = expect_string(value)?;
                self.set_empty_content(self.empty_icon, title, hint);
                Ok(())
            }
            "error_text" => {
                self.set_error(expect_string(value)?);
                Ok(())
            }
            "loading_rows" => {
                self.set_loading_rows(expect_usize(value)?);
                Ok(())
            }
            "pull_progress" => {
                self.set_pull_progress(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "state",
            "empty_icon",
            "empty_title",
            "empty_hint",
            "error_text",
            "loading_rows",
            "pull_progress",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Arc;
    use crate::core::Point;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Counts the emissions of a unit signal.
    fn count_units(signal: &GenericSignal) -> Arc<AtomicUsize> {
        let count = Arc::new(AtomicUsize::new(0));
        let handle = Arc::clone(&count);
        signal.connect(move || {
            handle.fetch_add(1, Ordering::SeqCst);
        });
        count
    }

    fn unit_count(count: &Arc<AtomicUsize>) -> usize {
        count.load(Ordering::SeqCst)
    }

    #[test]
    fn state_transitions_clamp_the_pull_progress() {
        let mut host = ListStateHost::new(Rect::new(0, 0, 320, 240));

        host.set_pull_progress(2.0);
        assert_eq!(host.pull_progress(), 1.0);

        host.set_state(ListState::Loading);
        assert_eq!(host.pull_progress(), 1.0, "a transition keeps progress inside the range");

        host.set_pull_progress(-3.0);
        assert_eq!(host.pull_progress(), 0.0);

        host.set_pull_progress(f32::NAN);
        assert_eq!(host.pull_progress(), 0.0, "a non-finite progress becomes zero");

        host.set_pull_progress(0.75);
        host.set_state(ListState::Empty);
        assert_eq!(host.pull_progress(), 0.75);
    }

    #[test]
    fn empty_and_error_surfaces_raise_their_signals() {
        let mut host = ListStateHost::new(Rect::new(0, 0, 320, 240));
        let empty = count_units(&host.empty_action_activated);
        let refresh = count_units(&host.refresh_requested);

        host.set_state(ListState::Empty);
        host.handle_event(&Event::MouseRelease { pos: Point::new(8, 8), button: 1 });
        assert_eq!(unit_count(&empty), 1);
        assert_eq!(unit_count(&refresh), 0);

        host.handle_event(&Event::KeyPress { key: ENTER, modifiers: 0 });
        assert_eq!(unit_count(&empty), 2, "Enter activates the empty action too");

        host.set_error("Offline");
        assert_eq!(host.state(), ListState::Error);
        host.handle_event(&Event::MouseRelease { pos: Point::new(8, 8), button: 1 });
        assert_eq!(unit_count(&refresh), 1);

        host.set_state(ListState::Idle);
        host.handle_event(&Event::MouseRelease { pos: Point::new(8, 8), button: 1 });
        assert_eq!(unit_count(&refresh), 1, "the idle surface has no action");
    }

    #[test]
    fn the_property_contract_round_trips() {
        let mut host = ListStateHost::new(Rect::new(0, 0, 320, 240));
        host.set_empty_content(None, "No rows", "Add a row to begin");
        host.set("state", CapabilityValue::String("loading".to_string())).expect("writable");
        host.set("loading_rows", CapabilityValue::UInt(4)).expect("writable");
        host.set("pull_progress", CapabilityValue::Float(0.75)).expect("writable");

        assert_eq!(host.state(), ListState::Loading);
        assert_eq!(host.loading_rows(), 4);
        assert_eq!(host.pull_progress(), 0.75);
        assert_eq!(host.get("state"), Ok(CapabilityValue::String("loading".to_string())));
        assert_eq!(host.get("empty_title"), Ok(CapabilityValue::String("No rows".to_string())));
        assert_eq!(
            host.get("empty_hint"),
            Ok(CapabilityValue::String("Add a row to begin".to_string()))
        );
        assert_eq!(host.get("empty_icon"), Ok(CapabilityValue::Null));
        assert_eq!(host.get("loading_rows"), Ok(CapabilityValue::UInt(4)));

        let progress = host.get("pull_progress").expect("readable");
        match progress {
            CapabilityValue::Float(value) => assert!((value - 0.75).abs() < f64::EPSILON),
            other => panic!("pull_progress must be a float, got {other:?}"),
        }

        host.set("error_text", CapabilityValue::String("Offline".to_string())).expect("writable");
        assert_eq!(host.state(), ListState::Error);
        assert_eq!(host.error_text(), "Offline");
        assert!(host.property_names().contains(&"state"));
    }
}
