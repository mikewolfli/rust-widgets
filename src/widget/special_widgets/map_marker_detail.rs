// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MapMarkerDetail` — the anchored information window a map opens for a marker (COMP-I02).
//!
//! # Why this is a separate control
//!
//! `MapView` reports *which* marker was clicked (`marker_selected`, `marker_clicked`,
//! `cluster_clicked`) and paints a selected pin, but where the marker's own content is shown is
//! the host's decision: a bubble, a side panel, a route card. Painting it inside the map would
//! have made the map own a second layout, and every host that wanted a different presentation
//! would have had to hide it. This control is the standard presentation, and it recomputes its
//! own screen position from an anchor the host feeds it, so a pan or a zoom keeps the window on
//! the place it describes.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// One line of a marker's detail window.
///
/// The shape mirrors `companions::TooltipLine` — text plus an optional colour — so a host that
/// already builds tooltip content can hand the same values to a map detail window.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailLine {
    /// The line's text.
    pub text: String,
    /// The line's ink, or `None` for the window's default ink.
    pub color: Option<Color>,
}

impl DetailLine {
    /// Creates a line painted in the window's default ink.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), color: None }
    }

    /// Creates a line in an explicit colour.
    pub fn colored(text: impl Into<String>, color: Color) -> Self {
        Self { text: text.into(), color: Some(color) }
    }
}

/// Padding inside the window, in pixels.
const DETAIL_PADDING: i32 = 8;
/// Height of one line of text, in pixels.
const DETAIL_LINE_HEIGHT: i32 = 16;
/// Height of the optional action button row.
const DETAIL_ACTION_HEIGHT: i32 = 20;

/// An anchored information window for a map marker or cluster.
pub struct MapMarkerDetail {
    base: BaseWidget,
    /// The marker this window describes, when it was opened for one.
    anchor: Option<String>,
    /// The world point this window describes, for a cluster with no single marker.
    anchor_point: Option<(f32, f32)>,
    /// The anchor's screen position, fed by the host from the map's projection.
    anchor_screen: Option<Point>,
    /// Offset from the anchor, in pixels.
    offset: (i32, i32),
    title: String,
    lines: Vec<DetailLine>,
    visible: bool,
    /// Whether the window follows its anchor when the host reports a new projection.
    follow_marker: bool,
    /// Whether a press on the map dismisses the window.
    dismiss_on_map_press: bool,
    /// The window's maximum width in pixels.
    max_width: u32,
    /// An optional action button's label.
    action_label: Option<String>,
    /// Emitted when the window is closed, by the user or by [`Self::close`].
    pub closed: Signal1<()>,
    /// Emitted with the action label when the window's action button is pressed.
    pub action_triggered: Signal1<String>,
}

impl MapMarkerDetail {
    /// Creates a hidden detail window.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Tooltip, geometry, "MapMarkerDetail"),
            anchor: None,
            anchor_point: None,
            anchor_screen: None,
            offset: (0, -12),
            title: String::new(),
            lines: Vec::new(),
            visible: false,
            follow_marker: true,
            dismiss_on_map_press: true,
            max_width: 220,
            action_label: None,
            closed: Signal1::new(),
            action_triggered: Signal1::new(),
        }
    }

    /// Whether the window is showing.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// The marker id the window was opened for, if any.
    pub fn anchor(&self) -> Option<&str> {
        self.anchor.as_deref()
    }

    /// The world point the window was opened at, if any.
    pub fn anchor_point(&self) -> Option<(f32, f32)> {
        self.anchor_point
    }

    /// The window's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The window's content lines.
    pub fn lines(&self) -> &[DetailLine] {
        &self.lines
    }

    /// The offset applied to the anchor's screen position.
    pub fn offset(&self) -> (i32, i32) {
        self.offset
    }

    /// Sets the offset applied to the anchor's screen position.
    pub fn set_offset(&mut self, offset: (i32, i32)) {
        if self.offset == offset {
            return;
        }
        self.offset = offset;
        self.base.request_redraw();
    }

    /// Whether the window tracks its anchor.
    pub fn follow_marker(&self) -> bool {
        self.follow_marker
    }

    /// Enables or disables following the anchor.
    pub fn set_follow_marker(&mut self, follow: bool) {
        self.follow_marker = follow;
    }

    /// Whether a map press dismisses the window.
    pub fn dismiss_on_map_press(&self) -> bool {
        self.dismiss_on_map_press
    }

    /// Enables or disables dismissal on a map press.
    pub fn set_dismiss_on_map_press(&mut self, dismiss: bool) {
        self.dismiss_on_map_press = dismiss;
    }

    /// The window's maximum width, floored at 80 pixels.
    pub fn max_width(&self) -> u32 {
        self.max_width
    }

    /// Sets the window's maximum width.
    pub fn set_max_width(&mut self, width: u32) {
        let width = width.max(80);
        if self.max_width == width {
            return;
        }
        self.max_width = width;
        self.base.request_redraw();
    }

    /// The action button's label, if any.
    pub fn action_label(&self) -> Option<&str> {
        self.action_label.as_deref()
    }

    /// Sets or clears the action button's label.
    pub fn set_action_label(&mut self, label: Option<impl Into<String>>) {
        let next = label.map(Into::into).filter(|label| !label.is_empty());
        if self.action_label == next {
            return;
        }
        self.action_label = next;
        self.base.request_redraw();
    }

    /// Opens the window for `marker_id` at `title` with `lines`.
    ///
    /// The anchor's screen position is not known until the host reports the map's projection; see
    /// [`Self::set_projection_anchor`]. Until then the window stays hidden, because a bubble at a
    /// guessed position is worse than no bubble.
    pub fn open_for_marker(
        &mut self,
        marker_id: impl Into<String>,
        title: impl Into<String>,
        lines: Vec<DetailLine>,
    ) {
        self.anchor = Some(marker_id.into());
        self.anchor_point = None;
        self.title = title.into();
        self.lines = lines;
        self.visible = true;
        self.base.request_redraw();
    }

    /// Opens the window at a world point, for a cluster with no single marker.
    pub fn open_at(
        &mut self,
        world_x: f32,
        world_y: f32,
        title: impl Into<String>,
        lines: Vec<DetailLine>,
    ) {
        self.anchor = None;
        self.anchor_point = Some((world_x, world_y));
        self.title = title.into();
        self.lines = lines;
        self.visible = true;
        self.base.request_redraw();
    }

    /// Replaces the window's content without moving its anchor.
    pub fn set_content(&mut self, title: impl Into<String>, lines: Vec<DetailLine>) {
        self.title = title.into();
        self.lines = lines;
        self.base.request_redraw();
    }

    /// Records the anchor's screen position, as the map projects it.
    ///
    /// The host calls this whenever the camera moves; with `follow_marker` set, this is what keeps
    /// the window on the place it describes instead of on a stale point.
    pub fn set_projection_anchor(&mut self, point: Point) {
        if !self.visible {
            return;
        }
        if self.anchor_screen == Some(point) {
            return;
        }
        if self.follow_marker || self.anchor_screen.is_none() {
            self.anchor_screen = Some(point);
            self.base.request_redraw();
        }
    }

    /// Closes the window and emits [`Self::closed`].
    pub fn close(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.anchor = None;
        self.anchor_point = None;
        self.anchor_screen = None;
        self.base.request_redraw();
        self.closed.emit(());
    }

    /// The window's rectangle, or `None` while it has nothing to show.
    fn detail_rect(&self) -> Option<Rect> {
        if !self.visible {
            return None;
        }
        let anchor = self.anchor_screen?;
        let text_rows = self.lines.len() as i32 + i32::from(!self.title.is_empty());
        let action_rows = i32::from(self.action_label.is_some());
        let height = DETAIL_PADDING * 2
            + text_rows * DETAIL_LINE_HEIGHT
            + action_rows * DETAIL_ACTION_HEIGHT;
        let width = self.max_width.min(self.geometry().width.max(1));
        Some(Rect::new(
            anchor.x + self.offset.0,
            anchor.y + self.offset.1 - height,
            width,
            height.max(DETAIL_LINE_HEIGHT) as u32,
        ))
    }

    /// The action button's rectangle, inside the window.
    fn action_rect(&self) -> Option<Rect> {
        let window = self.detail_rect()?;
        self.action_label.as_ref()?;
        let width = (window.width as i32 - DETAIL_PADDING * 2).max(1);
        Some(Rect::new(
            window.x + DETAIL_PADDING,
            window.y + window.height as i32 - DETAIL_PADDING - DETAIL_ACTION_HEIGHT,
            width as u32,
            DETAIL_ACTION_HEIGHT as u32,
        ))
    }
}

impl Widget for MapMarkerDetail {
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
        crate::core::Size::new(self.max_width, 80)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "closed" => Some(EventSignalRef::mapped("closed", &self.closed, |_| {
                CapabilityValue::Null
            })),
            "action_triggered" => Some(EventSignalRef::mapped(
                "action_triggered",
                &self.action_triggered,
                |value| CapabilityValue::String(value.clone()),
            )),
            _ => None,
        }
    }
}

impl WidgetProperties for MapMarkerDetail {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "anchor" => Ok(match self.anchor() {
                Some(id) => CapabilityValue::String(id.to_string()),
                None => CapabilityValue::Null,
            }),
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "visible" => Ok(CapabilityValue::Bool(self.visible)),
            "follow_marker" => Ok(CapabilityValue::Bool(self.follow_marker)),
            "dismiss_on_map_press" => Ok(CapabilityValue::Bool(self.dismiss_on_map_press)),
            "max_width" => Ok(CapabilityValue::UInt(u64::from(self.max_width))),
            "action_label" => Ok(match self.action_label() {
                Some(label) => CapabilityValue::String(label.to_string()),
                None => CapabilityValue::Null,
            }),
            "line_count" => Ok(CapabilityValue::UInt(self.lines.len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The anchor and the content are written through the `open_*` / `set_content`
            // methods, which are what put the window on screen.
            "anchor" | "line_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "title" => {
                let CapabilityValue::String(title) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_content(title, self.lines.clone());
                Ok(())
            }
            "visible" => {
                let CapabilityValue::Bool(visible) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                if visible {
                    self.visible = true;
                    self.base.request_redraw();
                } else {
                    self.close();
                }
                Ok(())
            }
            "follow_marker" => {
                let CapabilityValue::Bool(follow) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_follow_marker(follow);
                Ok(())
            }
            "dismiss_on_map_press" => {
                let CapabilityValue::Bool(dismiss) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_dismiss_on_map_press(dismiss);
                Ok(())
            }
            "max_width" => {
                let CapabilityValue::UInt(width) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_max_width(u32::try_from(width).unwrap_or(u32::MAX));
                Ok(())
            }
            "action_label" => {
                match value {
                    CapabilityValue::Null => self.set_action_label(None::<String>),
                    CapabilityValue::String(label) => self.set_action_label(Some(label)),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "anchor",
            "title",
            "visible",
            "follow_marker",
            "dismiss_on_map_press",
            "max_width",
            "action_label",
            "line_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `map_marker_detail` publishes.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "close" => {
                self.close();
                Ok(())
            }
            // Opening needs the anchor and the content, so a bare invocation is reported as
            // needing a payload rather than as unknown.
            "open_for_marker" | "open_at" | "set_content" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for MapMarkerDetail {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(action) = self.action_rect() {
                    if action.contains_point(*pos) {
                        if let Some(label) = self.action_label.clone() {
                            self.action_triggered.emit(label);
                        }
                        return;
                    }
                }
                // A press inside the window but outside its action keeps it open; a press outside
                // it closes it, which is the same rule a popover follows.
                let inside = self.detail_rect().is_some_and(|rect| rect.contains_point(*pos));
                if !inside {
                    self.close();
                }
            }
            Event::KeyPress { key: 27, .. } => self.close(),
            _ => {}
        }
    }
}

impl Draw for MapMarkerDetail {
    fn draw(&mut self, context: &mut RenderContext) {
        let Some(window) = self.detail_rect() else {
            return;
        };
        let style = self.base.style();
        // The window deliberately inverts against the page, exactly as a snackbar does, so it reads
        // as floating *over* the map rather than as another map layer.
        let (surface, ink) = crate::style::theme_manager()
            .current_theme()
            .map(|theme| (theme.colors.inverse_surface, theme.colors.on_inverse_surface))
            .unwrap_or_else(|| (style.background_color.unwrap_or(Color::rgb(48, 52, 58)), Color::WHITE));
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);
        let font = effective_font(style);

        context.fill_rounded_rect(window, 6, surface);
        context.draw_rect(window, surface.blend(&ink, 0.25));

        let mut y = window.y + DETAIL_PADDING;
        let text_width = window.width.saturating_sub((DETAIL_PADDING * 2) as u32);
        if !self.title.is_empty() {
            context.draw_text_fitted_cow(
                Rect::new(window.x + DETAIL_PADDING, y, text_width, DETAIL_LINE_HEIGHT as u32),
                &self.title,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            y += DETAIL_LINE_HEIGHT;
        }
        for line in &self.lines {
            // A line painted over the inverted surface needs its own colour to stay legible, so a
            // caller-supplied colour is used verbatim and a default line takes the paired ink.
            let color = line.color.unwrap_or(ink);
            context.draw_text_fitted_cow(
                Rect::new(window.x + DETAIL_PADDING, y, text_width, DETAIL_LINE_HEIGHT as u32),
                &line.text,
                font,
                color,
                HorizontalAlignment::Left,
            );
            y += DETAIL_LINE_HEIGHT;
        }

        if let (Some(action), Some(label)) = (self.action_rect(), self.action_label.as_deref()) {
            context.fill_rect(action, accent.with_alpha(40));
            context.draw_rect(action, accent);
            context.draw_text_line_cow(
                action,
                label,
                font,
                accent.contrast_color(),
                HorizontalAlignment::Center,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_for_a_marker_shows_a_window_once_the_anchor_is_projected() {
        let mut detail = MapMarkerDetail::new(Rect::new(0, 0, 300, 200));
        assert!(!detail.is_visible());
        detail.open_for_marker("m1", "Station", vec![DetailLine::new("Line 2")]);
        assert!(detail.is_visible());
        assert_eq!(detail.anchor(), Some("m1"));
        // Nothing is painted until the host supplies the projection.
        assert!(detail.detail_rect().is_none());
        detail.set_projection_anchor(Point::new(120, 90));
        let rect = detail.detail_rect().expect("the window has an anchor");
        assert_eq!(rect.width, detail.max_width());
        assert!(rect.height > 0);
    }

    #[test]
    fn closing_reports_once() {
        let mut detail = MapMarkerDetail::new(Rect::new(0, 0, 300, 200));
        detail.open_at(10.0, 20.0, "Cluster", Vec::new());
        detail.set_projection_anchor(Point::new(50, 50));
        let closed = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let sink = closed.clone();
        detail.closed.connect(move |_| {
            if let Ok(mut guard) = sink.lock() {
                *guard += 1;
            }
        });
        detail.close();
        detail.close();
        assert!(!detail.is_visible());
        assert_eq!(*closed.lock().expect("closed lock"), 1);
    }

    #[test]
    fn the_action_button_reports_its_label() {
        let mut detail = MapMarkerDetail::new(Rect::new(0, 0, 300, 200));
        detail.open_for_marker("m1", "Station", Vec::new());
        detail.set_projection_anchor(Point::new(40, 80));
        detail.set_action_label(Some("Directions"));
        let action = detail.action_rect().expect("the action row is laid out");
        let emitted = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        detail.action_triggered.connect(move |label| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(label.as_ref().clone());
            }
        });
        detail.handle_event(&Event::mouse_press(action.x + 2, action.y + 2, 1));
        assert_eq!(*emitted.lock().expect("emitted lock"), vec!["Directions".to_string()]);
        assert!(detail.is_visible(), "an action press must not dismiss the window");
    }
}
