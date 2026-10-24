// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MacWindowChrome` — a desktop title bar with traffic-light controls.
//!
//! The control paints the bar itself: a raised strip, a centred title, three round status
//! controls at the leading edge, and a hairline along the bottom. It also publishes the
//! *draggable band* — the part of the bar that is not a control — so a host can attach its
//! window-move gesture to the region the bar actually leaves free.

use crate::compat::Cow;
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{profile_metrics, role_font, ControlMetricsProfile};
use crate::widget::text_utils::elide_to_width;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::{
    draw_focus_ring, palette, MAC_TITLE_BAR_HEIGHT, MAC_TRAFFIC_LIGHT_GAP, MAC_TRAFFIC_LIGHT_INSET,
};

/// Which traffic-light control a position addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowChromeButton {
    /// The leading control: requests that the window close.
    Close,
    /// The middle control: requests that the window minimise.
    Minimize,
    /// The trailing control: requests that the window zoom.
    Zoom,
}

impl WindowChromeButton {
    /// The three controls, in display order from the leading edge.
    const ALL: [Self; 3] = [Self::Close, Self::Minimize, Self::Zoom];

    /// The index of this control in the strip, used to place it.
    fn index(self) -> i32 {
        match self {
            Self::Close => 0,
            Self::Minimize => 1,
            Self::Zoom => 2,
        }
    }
}

/// A desktop window title bar.
///
/// The bar carries a title, three traffic-light controls and a draggable band between and around
/// them. Each control emits its own request signal when pressed:
/// [`Self::close_requested`], [`Self::minimize_requested`] and [`Self::zoom_requested`]. The
/// control never acts on those requests itself — closing or moving a window belongs to the host.
pub struct MacWindowChrome {
    base: BaseWidget,
    title: String,
    buttons_visible: bool,
    draggable: bool,
    bar_height: u32,
    hovered: Option<WindowChromeButton>,
    pressed: Option<WindowChromeButton>,
    /// Emitted when the close control is pressed.
    pub close_requested: GenericSignal,
    /// Emitted when the minimise control is pressed.
    pub minimize_requested: GenericSignal,
    /// Emitted when the zoom control is pressed.
    pub zoom_requested: GenericSignal,
}

impl MacWindowChrome {
    /// Creates a title bar with no title, all three controls shown and dragging enabled.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Window, geometry, "MacWindowChrome"),
            // Matches the schema's declared default, so a fresh bar and its manifest agree.
            title: "Window".to_string(),
            buttons_visible: true,
            draggable: true,
            bar_height: MAC_TITLE_BAR_HEIGHT,
            hovered: None,
            pressed: None,
            close_requested: GenericSignal::new(),
            minimize_requested: GenericSignal::new(),
            zoom_requested: GenericSignal::new(),
        }
    }

    /// The title shown in the middle of the bar.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the title. The bar re-paints when the text changes.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if title == self.title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// Whether the three traffic-light controls are painted and clickable.
    pub fn buttons_visible(&self) -> bool {
        self.buttons_visible
    }

    /// Shows or hides the traffic-light controls.
    pub fn set_buttons_visible(&mut self, visible: bool) {
        if visible == self.buttons_visible {
            return;
        }
        self.buttons_visible = visible;
        if !visible {
            self.hovered = None;
            self.pressed = None;
        }
        self.base.request_redraw();
    }

    /// Whether the free part of the bar is offered as a window-move handle.
    pub fn draggable(&self) -> bool {
        self.draggable
    }

    /// Turns the draggable band on or off.
    pub fn set_draggable(&mut self, draggable: bool) {
        if draggable == self.draggable {
            return;
        }
        self.draggable = draggable;
    }

    /// The height of the bar in points.
    pub fn bar_height(&self) -> u32 {
        self.bar_height
    }

    /// The rectangle the bar occupies: full width, one title-bar tall, at the top of the
    /// control's allocation.
    pub fn bar_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, self.bar_height.min(rect.height))
    }

    /// The diameter of one traffic-light control.
    ///
    /// Taken from the desktop profile's button radius, whose value is exactly the radius of the
    /// platform's 12-point status controls — one metric, two uses, so the two cannot drift.
    pub fn traffic_light_diameter(&self) -> u32 {
        2 * profile_metrics(ControlMetricsProfile::Macos).button_radius
    }

    /// The square one traffic-light control occupies, or `None` when it is hidden.
    pub fn button_rect(&self, button: WindowChromeButton) -> Option<Rect> {
        if !self.buttons_visible {
            return None;
        }
        let bar = self.bar_rect();
        let diameter = self.traffic_light_diameter();
        if diameter == 0 || diameter > bar.height || diameter > bar.width {
            return None;
        }
        let x = bar.x
            + MAC_TRAFFIC_LIGHT_INSET
            + button.index() * (diameter as i32 + MAC_TRAFFIC_LIGHT_GAP);
        Some(Rect::new(x, bar.y + ((bar.height - diameter) / 2) as i32, diameter, diameter))
    }

    /// The band a host may attach its window-move gesture to.
    ///
    /// The band is the bar with the traffic-light controls cut off: dragging from a control must
    /// act on the control, and dragging from anywhere else must move the window. An empty
    /// rectangle is returned when dragging is switched off or the bar has no free width.
    pub fn drag_band(&self) -> Rect {
        if !self.draggable {
            return Rect::new(0, 0, 0, 0);
        }
        let bar = self.bar_rect();
        let trailing_edge = WindowChromeButton::ALL
            .iter()
            .filter_map(|button| self.button_rect(*button))
            .map(|rect| rect.right())
            .max()
            .unwrap_or(bar.x);
        let start =
            if self.buttons_visible { trailing_edge + MAC_TRAFFIC_LIGHT_GAP } else { bar.x };
        let width = bar.right().saturating_sub(start).max(0) as u32;
        Rect::new(start, bar.y, width, bar.height)
    }

    /// Whether `pos` lies in the draggable band.
    pub fn is_in_drag_band(&self, pos: Point) -> bool {
        let band = self.drag_band();
        band.width > 0 && band.contains_point(pos)
    }

    /// The traffic-light control under `pos`, if any.
    fn button_at(&self, pos: Point) -> Option<WindowChromeButton> {
        WindowChromeButton::ALL
            .into_iter()
            .find(|button| self.button_rect(*button).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// Emits the request signal `button` owns.
    fn request(&self, button: WindowChromeButton) {
        match button {
            WindowChromeButton::Close => self.close_requested.emit(),
            WindowChromeButton::Minimize => self.minimize_requested.emit(),
            WindowChromeButton::Zoom => self.zoom_requested.emit(),
        }
    }

    /// The fill colour of `button`.
    fn button_color(&self, button: WindowChromeButton) -> crate::core::Color {
        let colors = palette();
        match button {
            WindowChromeButton::Close => colors.close,
            WindowChromeButton::Minimize => colors.minimize,
            WindowChromeButton::Zoom => colors.zoom,
        }
    }

    /// Paints the small symbol inside `button`'s circle, shown only while it is hovered.
    fn draw_button_symbol(
        &self,
        context: &mut RenderContext,
        button: WindowChromeButton,
        rect: Rect,
        ink: crate::core::Color,
    ) {
        let center = rect.center();
        let reach = (rect.width / 4) as i32;
        match button {
            WindowChromeButton::Close => {
                context.draw_line(
                    Point::new(center.x - reach, center.y - reach),
                    Point::new(center.x + reach, center.y + reach),
                    ink,
                );
                context.draw_line(
                    Point::new(center.x + reach, center.y - reach),
                    Point::new(center.x - reach, center.y + reach),
                    ink,
                );
            }
            WindowChromeButton::Minimize => {
                context.draw_line(
                    Point::new(center.x - reach, center.y),
                    Point::new(center.x + reach, center.y),
                    ink,
                );
            }
            WindowChromeButton::Zoom => {
                context.draw_line(
                    Point::new(center.x - reach, center.y - reach),
                    Point::new(center.x + reach, center.y + reach),
                    ink,
                );
                context.draw_line(
                    Point::new(center.x + reach, center.y - reach),
                    Point::new(center.x - reach, center.y + reach),
                    ink,
                );
            }
        }
    }
}

impl Widget for MacWindowChrome {
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
        Size::new(320, self.bar_height)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::Window
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves a published event name to the control's own signal.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "close_requested" => {
                Some(EventSignalRef::unit("close_requested", &self.close_requested))
            }
            "minimize_requested" => {
                Some(EventSignalRef::unit("minimize_requested", &self.minimize_requested))
            }
            "zoom_requested" => Some(EventSignalRef::unit("zoom_requested", &self.zoom_requested)),
            _ => None,
        }
    }
}

impl WidgetProperties for MacWindowChrome {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "chrome_height" => Ok(CapabilityValue::UInt(self.bar_height() as u64)),
            "buttons_visible" => Ok(CapabilityValue::Bool(self.buttons_visible())),
            "draggable" => Ok(CapabilityValue::Bool(self.draggable())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "buttons_visible" => {
                self.set_buttons_visible(expect_bool(value)?);
                Ok(())
            }
            "draggable" => {
                self.set_draggable(expect_bool(value)?);
                Ok(())
            }
            // Fixed by the control's own metrics; a caller that needs a taller bar draws a
            // different chrome component rather than stretching this one.
            "chrome_height" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "chrome_height",
            "buttons_visible",
            "draggable",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for MacWindowChrome {
    fn draw(&mut self, context: &mut RenderContext) {
        let bar = self.bar_rect();
        if bar.width == 0 || bar.height == 0 {
            return;
        }

        let colors = palette();
        let style = self.base.style();
        let font = role_font(style, 13.0);

        context.fill_rect(bar, colors.raised);
        // The hairline separating the bar from the window's content.
        context.draw_line(
            Point::new(bar.x, bar.bottom() - 1),
            Point::new(bar.right() - 1, bar.bottom() - 1),
            colors.separator,
        );

        // The title is centred in the bar and elided to the space the controls leave, so it can
        // never run under a traffic light.
        let leading = self
            .button_rect(WindowChromeButton::Zoom)
            .map(|rect| rect.right() + MAC_TRAFFIC_LIGHT_GAP)
            .unwrap_or(bar.x + MAC_TRAFFIC_LIGHT_INSET);
        let available = bar.right().saturating_sub(leading).max(0) as u32;
        if available > 0 && !self.title.is_empty() {
            let mut measure = |text: &str| context.measure_text(text, &font).width;
            let shown: Cow<'_, str> = elide_to_width(&self.title, available, &mut measure);
            let line = context.text_line(bar, &font);
            let ink = if self.base.is_enabled() { colors.ink } else { colors.muted };
            context.draw_text(
                Point::new(bar.center().x, line.y),
                shown.as_ref(),
                &font,
                ink,
                HorizontalAlignment::Center,
            );
        }

        for button in WindowChromeButton::ALL {
            let Some(rect) = self.button_rect(button) else {
                continue;
            };
            let fill = self.button_color(button);
            // The symbol only shows on hover, which keeps a resting bar clean and tells the
            // user which control a press would hit.
            let visible_symbol = self.hovered == Some(button) || self.pressed == Some(button);
            context.fill_circle(rect.center(), rect.width / 2, fill);
            if visible_symbol {
                self.draw_button_symbol(context, button, rect, fill.contrast_color());
            }
            if self.pressed == Some(button) {
                context.draw_circle(rect.center(), rect.width / 2, colors.separator);
            }
        }

        draw_focus_ring(context, bar, 0, &self.base, colors.accent);
    }
}

impl EventHandler for MacWindowChrome {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                let hovered = self.button_at(*pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(control) = self.button_at(*pos) {
                    self.pressed = Some(control);
                    self.base.request_redraw();
                    self.request(control);
                }
            }
            // The control the pointer went down on, if any; the release redraws only when a
            // press was actually latched.
            Event::MouseRelease { .. } if self.pressed.take().is_some() => {
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
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn sample() -> MacWindowChrome {
        MacWindowChrome::new(Rect::new(0, 0, 320, 28))
    }

    #[test]
    fn a_fresh_bar_carries_its_title_and_three_controls() {
        let bar = sample();
        assert_eq!(bar.title(), "Window");
        assert_eq!(bar.bar_height(), MAC_TITLE_BAR_HEIGHT);
        assert!(bar.buttons_visible());
        assert!(bar.draggable());
        assert_eq!(bar.size_hint(), Size::new(320, MAC_TITLE_BAR_HEIGHT));

        let close = bar.button_rect(WindowChromeButton::Close).expect("close is shown");
        let minimize = bar.button_rect(WindowChromeButton::Minimize).expect("minimise is shown");
        let zoom = bar.button_rect(WindowChromeButton::Zoom).expect("zoom is shown");
        assert_eq!(close.x, MAC_TRAFFIC_LIGHT_INSET);
        assert_eq!(minimize.x, close.right() + MAC_TRAFFIC_LIGHT_GAP);
        assert_eq!(zoom.x, minimize.right() + MAC_TRAFFIC_LIGHT_GAP);
        assert_eq!(close.width, bar.traffic_light_diameter());
    }

    #[test]
    fn the_draggable_band_starts_after_the_last_control() {
        let bar = sample();
        let zoom = bar.button_rect(WindowChromeButton::Zoom).expect("zoom is shown");
        let band = bar.drag_band();
        assert_eq!(band.x, zoom.right() + MAC_TRAFFIC_LIGHT_GAP);
        assert_eq!(band.right(), bar.bar_rect().right());
        assert!(bar.is_in_drag_band(Point::new(250, 14)));
        assert!(!bar.is_in_drag_band(Point::new(5, 14)), "a control is not a drag handle");
    }

    #[test]
    fn turning_dragging_off_empties_the_band() {
        let mut bar = sample();
        bar.set_draggable(false);
        assert_eq!(bar.drag_band(), Rect::new(0, 0, 0, 0));
        assert!(!bar.is_in_drag_band(Point::new(250, 14)));
    }

    #[test]
    fn hiding_the_controls_extends_the_band_and_removes_the_buttons() {
        let mut bar = sample();
        bar.set_buttons_visible(false);
        assert!(bar.button_rect(WindowChromeButton::Close).is_none());
        assert_eq!(bar.drag_band().x, bar.bar_rect().x);
    }

    #[test]
    fn each_control_emits_its_own_request() {
        let mut bar = sample();
        let closes = Arc::new(AtomicUsize::new(0));
        let minimizes = Arc::new(AtomicUsize::new(0));
        let zooms = Arc::new(AtomicUsize::new(0));
        bar.close_requested.connect({
            let counter = Arc::clone(&closes);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        bar.minimize_requested.connect({
            let counter = Arc::clone(&minimizes);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        bar.zoom_requested.connect({
            let counter = Arc::clone(&zooms);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });

        for control in WindowChromeButton::ALL {
            let rect = bar.button_rect(control).expect("the control is shown");
            bar.handle_event(&Event::MousePress { pos: rect.center(), button: 1, modifiers: 0 });
        }
        assert_eq!(closes.load(Ordering::SeqCst), 1);
        assert_eq!(minimizes.load(Ordering::SeqCst), 1);
        assert_eq!(zooms.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn pressing_the_draggable_band_requests_nothing() {
        let mut bar = sample();
        let closes = Arc::new(AtomicUsize::new(0));
        bar.close_requested.connect({
            let counter = Arc::clone(&closes);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        bar.handle_event(&Event::MousePress { pos: Point::new(250, 14), button: 1, modifiers: 0 });
        assert_eq!(closes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_property_route_reads_and_writes_every_published_name() {
        let mut bar = sample();
        assert_eq!(bar.get("title"), Ok(CapabilityValue::String("Window".to_string())));
        bar.set("title", CapabilityValue::String("Notes".to_string())).unwrap();
        assert_eq!(bar.get("title"), Ok(CapabilityValue::String("Notes".to_string())));
        assert_eq!(
            bar.get("chrome_height"),
            Ok(CapabilityValue::UInt(MAC_TITLE_BAR_HEIGHT as u64))
        );
        bar.set("buttons_visible", CapabilityValue::Bool(false)).unwrap();
        assert_eq!(bar.get("buttons_visible"), Ok(CapabilityValue::Bool(false)));
        bar.set("draggable", CapabilityValue::Bool(false)).unwrap();
        assert_eq!(bar.get("draggable"), Ok(CapabilityValue::Bool(false)));
        assert_eq!(
            bar.set("chrome_height", CapabilityValue::UInt(40)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn the_bar_paints_its_surface_title_and_three_controls() {
        let mut bar = sample();
        let mut recorder = Recorder::new(320, 28);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            bar.draw(&mut context);
        }
        recorder.end_frame();

        let titles: Vec<&str> = recorder
            .commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::DrawText { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(titles, vec!["Window"]);
        let circles = recorder
            .commands
            .iter()
            .filter(|command| matches!(command, RenderCommand::FillCircle { .. }))
            .count();
        assert_eq!(circles, 3, "all three traffic lights are painted");
    }

    #[test]
    fn every_painted_control_stays_inside_the_bar() {
        let mut bar = sample();
        let mut recorder = Recorder::new(320, 28);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            bar.draw(&mut context);
        }
        recorder.end_frame();
        for command in &recorder.commands {
            if let RenderCommand::FillCircle { center, radius, .. } = command {
                assert!(center.x - *radius as i32 >= 0);
                assert!(center.y - *radius as i32 >= 0);
                assert!(center.y + *radius as i32 <= 28);
            }
        }
    }
}
