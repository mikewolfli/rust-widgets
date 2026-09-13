// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scroll area widget.
use crate::core::{Alignment, Color, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;

fn translate_content_event(event: &Event, dx: i32, dy: i32) -> Event {
    let translate = |point: Point| Point::new(point.x + dx, point.y + dy);
    match event {
        Event::MouseDown((point, button)) => Event::MouseDown((translate(*point), *button)),
        Event::MouseUp((point, button)) => Event::MouseUp((translate(*point), *button)),
        Event::MouseMoveLegacy((point, device)) => {
            Event::MouseMoveLegacy((translate(*point), *device))
        }
        Event::MouseMove { pos } => Event::MouseMove { pos: translate(*pos) },
        Event::MousePress { pos, button } => {
            Event::MousePress { pos: translate(*pos), button: *button }
        }
        Event::MouseDoubleClick { pos, button } => {
            Event::MouseDoubleClick { pos: translate(*pos), button: *button }
        }
        Event::MouseRelease { pos, button } => {
            Event::MouseRelease { pos: translate(*pos), button: *button }
        }
        Event::MouseEnter { pos } => Event::MouseEnter { pos: translate(*pos) },
        Event::MouseLeave { pos } => Event::MouseLeave { pos: translate(*pos) },
        Event::PointerPress { pos, button, pressure, tilt_x, tilt_y } => Event::PointerPress {
            pos: translate(*pos),
            button: *button,
            pressure: *pressure,
            tilt_x: *tilt_x,
            tilt_y: *tilt_y,
        },
        Event::PointerMove { pos, pressure, tilt_x, tilt_y } => Event::PointerMove {
            pos: translate(*pos),
            pressure: *pressure,
            tilt_x: *tilt_x,
            tilt_y: *tilt_y,
        },
        Event::PointerRelease { pos, button, pressure } => {
            Event::PointerRelease { pos: translate(*pos), button: *button, pressure: *pressure }
        }
        #[cfg(feature = "touch")]
        Event::TouchBegin { pos, touch_id } => {
            Event::TouchBegin { pos: translate(*pos), touch_id: *touch_id }
        }
        #[cfg(feature = "touch")]
        Event::TouchEnd { pos, touch_id } => {
            Event::TouchEnd { pos: translate(*pos), touch_id: *touch_id }
        }
        #[cfg(feature = "touch")]
        Event::TouchMove { pos, touch_id } => {
            Event::TouchMove { pos: translate(*pos), touch_id: *touch_id }
        }
        #[cfg(feature = "touch")]
        Event::Tap { pos } => Event::Tap { pos: translate(*pos) },
        #[cfg(feature = "touch")]
        Event::DoubleTap { pos } => Event::DoubleTap { pos: translate(*pos) },
        #[cfg(feature = "touch")]
        Event::LongPress { pos } => Event::LongPress { pos: translate(*pos) },
        #[cfg(feature = "touch")]
        Event::Swipe { start, end, velocity } => {
            Event::Swipe { start: translate(*start), end: translate(*end), velocity: *velocity }
        }
        #[cfg(feature = "touch")]
        Event::Drag { pos, touch_id, delta } => {
            Event::Drag { pos: translate(*pos), touch_id: *touch_id, delta: *delta }
        }
        #[cfg(feature = "touch")]
        Event::TwoFingerTap { pos } => Event::TwoFingerTap { pos: translate(*pos) },
        #[cfg(feature = "touch")]
        Event::Fling { pos, velocity, touch_id } => {
            Event::Fling { pos: translate(*pos), velocity: *velocity, touch_id: *touch_id }
        }
        #[cfg(feature = "holographic")]
        Event::HolographicTouch { pos, depth, touch_id } => {
            Event::HolographicTouch { pos: translate(*pos), depth: *depth, touch_id: *touch_id }
        }
        _ => event.clone(),
    }
}

/// Scroll area widget.
pub struct ScrollArea {
    base: BaseWidget,
    widget_resizable: bool,
    alignment: Alignment,
    horizontal_scroll_bar_policy: ScrollBarPolicy,
    vertical_scroll_bar_policy: ScrollBarPolicy,
    viewport: Rect,
    widget: Option<ObjectId>,
    /// The total size of the content (for AsNeeded scroll bar calculation).
    content_size: Size,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// Current scroll position (x, y) in content coordinates.
    scroll_position: (i32, i32),
    /// Emitted whenever scroll position changes.
    pub scroll_position_changed: Signal1<(i32, i32)>,
}
/// Scroll bar policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBarPolicy {
    /// Scroll bar is always shown
    AlwaysOn,
    /// Scroll bar is always hidden
    AlwaysOff,
    /// Scroll bar is shown when needed
    #[default]
    AsNeeded,
}
impl ScrollArea {
    /// Creates a scroll area.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScrollArea, geometry, "ScrollArea"),
            widget_resizable: true,
            alignment: Alignment::Center,
            horizontal_scroll_bar_policy: ScrollBarPolicy::AsNeeded,
            vertical_scroll_bar_policy: ScrollBarPolicy::AsNeeded,
            viewport: geometry,
            widget: None,
            content_size: Size::new(0, 0),
            registry: None,
            scroll_position: (0, 0),
            scroll_position_changed: Signal1::new(),
        }
    }
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
    /// Returns whether the widget is resizable.
    pub fn widget_resizable(&self) -> bool {
        self.widget_resizable
    }
    /// Sets whether the widget is resizable.
    pub fn set_widget_resizable(&mut self, resizable: bool) {
        self.widget_resizable = resizable;
    }
    /// Returns alignment.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }
    /// Sets alignment.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
    }
    /// Returns horizontal scroll bar policy.
    pub fn horizontal_scroll_bar_policy(&self) -> ScrollBarPolicy {
        self.horizontal_scroll_bar_policy
    }
    /// Sets horizontal scroll bar policy.
    pub fn set_horizontal_scroll_bar_policy(&mut self, policy: ScrollBarPolicy) {
        self.horizontal_scroll_bar_policy = policy;
    }
    /// Returns vertical scroll bar policy.
    pub fn vertical_scroll_bar_policy(&self) -> ScrollBarPolicy {
        self.vertical_scroll_bar_policy
    }
    /// Sets vertical scroll bar policy.
    pub fn set_vertical_scroll_bar_policy(&mut self, policy: ScrollBarPolicy) {
        self.vertical_scroll_bar_policy = policy;
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Returns viewport rectangle.
    pub fn viewport(&self) -> Rect {
        self.viewport
    }
    /// Sets viewport rectangle.
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
        self.set_scroll_position(self.scroll_position.0, self.scroll_position.1);
    }
    /// Returns the current scroll position.
    pub fn scroll_position(&self) -> (i32, i32) {
        self.scroll_position
    }

    /// Sets the scroll position, clamped within the content extent.
    pub fn set_scroll_position(&mut self, x: i32, y: i32) {
        let view_w = self.viewport.width as i32;
        let view_h = self.viewport.height as i32;
        let content_w = self.content_size.width as i32;
        let content_h = self.content_size.height as i32;
        let max_x = (content_w - view_w).max(0);
        let max_y = (content_h - view_h).max(0);
        let clamped = (x.clamp(0, max_x), y.clamp(0, max_y));
        if self.scroll_position == clamped {
            return;
        }
        self.scroll_position = clamped;
        self.scroll_position_changed.emit(self.scroll_position);
        self.base.request_redraw();
    }

    /// Sets the total content size (used for AsNeeded scroll bar calculation).
    pub fn set_content_size(&mut self, size: Size) {
        self.content_size = size;
        self.set_scroll_position(self.scroll_position.0, self.scroll_position.1);
    }

    /// Returns the total content size.
    pub fn content_size(&self) -> Size {
        self.content_size
    }
    /// Ensures rectangle is visible.
    pub fn ensure_visible(&mut self, rect: Rect) {
        // Adjust the content scroll position, not the viewport's geometry.
        let mut x = self.scroll_position.0;
        let mut y = self.scroll_position.1;
        let view_right = x.saturating_add(self.viewport.width as i32);
        let view_bottom = y.saturating_add(self.viewport.height as i32);
        let rect_right = rect.x.saturating_add(rect.width as i32);
        let rect_bottom = rect.y.saturating_add(rect.height as i32);
        if rect.x < x {
            x = rect.x;
        } else if rect_right > view_right {
            x = rect_right.saturating_sub(self.viewport.width as i32);
        }
        if rect.y < y {
            y = rect.y;
        } else if rect_bottom > view_bottom {
            y = rect_bottom.saturating_sub(self.viewport.height as i32);
        }
        self.set_scroll_position(x, y);
    }
    /// Ensures widget is visible.
    pub fn ensure_widget_visible(&mut self, widget_id: ObjectId) {
        // If the widget is the child widget, center the content viewport by
        // changing scroll_position rather than mutating viewport geometry.
        if Some(widget_id) == self.widget {
            let max_x = (self.content_size.width.saturating_sub(self.viewport.width)) as i32;
            let max_y = (self.content_size.height.saturating_sub(self.viewport.height)) as i32;
            self.set_scroll_position(max_x / 2, max_y / 2);
        }
    }

    /// Scrolls to the top of the content.
    pub fn scroll_to_top(&mut self) {
        self.set_scroll_position(self.scroll_position.0, 0);
    }

    /// Scrolls to the bottom of the content.
    pub fn scroll_to_bottom(&mut self) {
        if self.content_size.height > self.viewport.height {
            self.set_scroll_position(
                self.scroll_position.0,
                (self.content_size.height - self.viewport.height) as i32,
            );
        } else {
            self.set_scroll_position(self.scroll_position.0, 0);
        }
    }

    /// Scrolls to the left edge of the content.
    pub fn scroll_to_left(&mut self) {
        self.set_scroll_position(0, self.scroll_position.1);
    }

    /// Scrolls to the right edge of the content.
    pub fn scroll_to_right(&mut self) {
        if self.content_size.width > self.viewport.width {
            self.set_scroll_position(
                (self.content_size.width - self.viewport.width) as i32,
                self.scroll_position.1,
            );
        } else {
            self.set_scroll_position(0, self.scroll_position.1);
        }
    }
    /// Returns whether horizontal scroll bar is visible.
    fn horizontal_scroll_bar_visible(&self) -> bool {
        match self.horizontal_scroll_bar_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => self.content_size.width > self.viewport.width,
        }
    }
    /// Returns whether vertical scroll bar is visible.
    fn vertical_scroll_bar_visible(&self) -> bool {
        match self.vertical_scroll_bar_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => self.content_size.height > self.viewport.height,
        }
    }
    /// Updates scroll bars — redraws so scroll bar visibility changes take effect.
    /// Note: Full scroll bar widget creation would require access to a widget factory
    /// and is not yet implemented. Visual scroll bar drawing is handled in `Draw`.
    fn update_scroll_bars(&mut self) {
        self.base.request_redraw();
    }

    /// Computes scroll-bar thumb geometry for a track of `track_len` logical
    /// pixels over content of `content_len` shown in a `view_len` viewport at
    /// scroll offset `scroll`. Returns `(thumb_len, thumb_offset)` where the
    /// offset is measured from the start of the track.
    ///
    /// The thumb length is proportional to the visible fraction and never
    /// collapses below a minimal usable size. When the content fits inside the
    /// viewport the thumb spans the whole track and cannot move.
    fn thumb_metrics(track_len: u32, content_len: u32, view_len: u32, scroll: i32) -> (u32, i32) {
        let track_len = track_len.max(1);
        if view_len == 0 || content_len <= view_len {
            return (track_len, 0);
        }
        let max_scroll = (content_len - view_len) as i32;
        let proportional = (track_len as u64 * view_len as u64) / content_len as u64;
        let thumb = proportional.clamp(10, track_len as u64) as u32;
        let max_thumb_pos = (track_len - thumb) as i32;
        let scroll = scroll.clamp(0, max_scroll);
        let pos = (max_thumb_pos as i64 * scroll as i64) / max_scroll as i64;
        (thumb, pos as i32)
    }
}
// Implement Widget trait
impl Widget for ScrollArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.viewport.width = geometry.width;
        self.viewport.height = geometry.height;
        self.update_scroll_bars();
    }
}
impl EventHandler for ScrollArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // Handle scroll events (mouse wheel + touch swipe)
        match event {
            Event::Wheel { delta, modifiers: _ } => {
                // Scroll the content using clamped content coordinates.
                self.set_scroll_position(
                    self.scroll_position.0 + delta.x * 20,
                    self.scroll_position.1 + delta.y * 20,
                );
            }
            #[cfg(feature = "touch")]
            Event::Swipe { start, end, velocity: _ } => {
                // Map swipe to scroll
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                self.set_scroll_position(self.scroll_position.0 + dx, self.scroll_position.1 + dy);
            }
            #[cfg(feature = "touch")]
            Event::Drag { delta, .. } => {
                // Map finger drag to scroll
                self.set_scroll_position(
                    self.scroll_position.0 + delta.x,
                    self.scroll_position.1 + delta.y,
                );
            }
            _ => { /* Other events are not relevant */ }
        }
        // Forward events to widget via registry (with viewport offset)
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                let content_event =
                    translate_content_event(event, self.scroll_position.0, self.scroll_position.1);
                reg.borrow_mut().forward_event(widget_id, &content_event);
            }
        }
    }
}
impl Draw for ScrollArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let style = self.style();
        // Draw background
        context.fill_rect(rect, style.background_color.unwrap_or(Color::rgb(255, 255, 255)));
        // Draw border
        context.draw_rect(rect, style.border_color.unwrap_or(Color::rgb(200, 200, 200)));
        // Set viewport for clipping
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        // Draw widget via registry, translated by the negative scroll offset so
        // the viewport shows the scrolled content window.
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                context.push_offset(-self.scroll_position.0, -self.scroll_position.1);
                reg.borrow_mut().draw_widget(widget_id, context);
                context.pop_offset();
            }
        }
        context.pop_clip();
        // Draw scroll bars if visible
        let h_scroll_visible = self.horizontal_scroll_bar_visible();
        let v_scroll_visible = self.vertical_scroll_bar_visible();
        if h_scroll_visible {
            // Draw horizontal scroll bar
            let scroll_bar_height = 16;
            let scroll_bar_y = rect.y as f32 + rect.height as f32 - scroll_bar_height as f32;
            context.fill_rect(
                Rect::new(rect.x, scroll_bar_y as i32, rect.width, scroll_bar_height),
                Color::rgb(240, 240, 240),
            );
            context.draw_rect(
                Rect::new(rect.x, scroll_bar_y as i32, rect.width, scroll_bar_height),
                Color::rgb(200, 200, 200),
            );
            // Draw scroll bar thumb, tracking the horizontal scroll position.
            let h_track =
                rect.width.saturating_sub(if v_scroll_visible { scroll_bar_height } else { 0 });
            let (thumb_width, thumb_dx) = Self::thumb_metrics(
                h_track,
                self.content_size.width,
                self.viewport.width.max(1),
                self.scroll_position.0,
            );
            let thumb_x = rect.x + thumb_dx;
            context.fill_rect(
                Rect::new(thumb_x, scroll_bar_y as i32, thumb_width, scroll_bar_height),
                Color::rgb(180, 180, 180),
            );
        }
        if v_scroll_visible {
            // Draw vertical scroll bar
            let scroll_bar_width = 16;
            let scroll_bar_x = rect.x as f32 + rect.width as f32 - scroll_bar_width as f32;
            context.fill_rect(
                Rect::new(scroll_bar_x as i32, rect.y, scroll_bar_width, rect.height),
                Color::rgb(240, 240, 240),
            );
            context.draw_rect(
                Rect::new(scroll_bar_x as i32, rect.y, scroll_bar_width, rect.height),
                Color::rgb(200, 200, 200),
            );
            // Draw scroll bar thumb, tracking the vertical scroll position.
            let v_track =
                rect.height.saturating_sub(if h_scroll_visible { scroll_bar_width } else { 0 });
            let (thumb_height, thumb_dy) = Self::thumb_metrics(
                v_track,
                self.content_size.height,
                self.viewport.height.max(1),
                self.scroll_position.1,
            );
            let thumb_y = rect.y + thumb_dy;
            context.fill_rect(
                Rect::new(scroll_bar_x as i32, thumb_y, scroll_bar_width, thumb_height),
                Color::rgb(180, 180, 180),
            );
        }
        // Draw corner between scroll bars
        if h_scroll_visible && v_scroll_visible {
            let corner_size = 16;
            let corner_x = rect.x as f32 + rect.width as f32 - corner_size as f32;
            let corner_y = rect.y as f32 + rect.height as f32 - corner_size as f32;
            context.fill_rect(
                Rect::new(corner_x as i32, corner_y as i32, corner_size as u32, corner_size as u32),
                Color::rgb(240, 240, 240),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect};

    #[test]
    fn scrollarea_creation_defaults() {
        let sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        assert_eq!(sa.geometry(), Rect::new(0, 0, 200, 200));
        assert_eq!(sa.viewport(), Rect::new(0, 0, 200, 200));
        assert_eq!(sa.scroll_position(), (0, 0));
        assert_eq!(sa.horizontal_scroll_bar_policy(), ScrollBarPolicy::AsNeeded);
        assert_eq!(sa.vertical_scroll_bar_policy(), ScrollBarPolicy::AsNeeded);
    }

    #[test]
    fn scrollarea_set_scroll_position_clamps_content_space() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(300, 260));
        sa.set_viewport(Rect::new(0, 0, 100, 100));

        sa.set_scroll_position(500, 400);
        assert_eq!(sa.scroll_position(), (200, 160));

        sa.set_scroll_position(-10, -8);
        assert_eq!(sa.scroll_position(), (0, 0));
    }

    #[test]
    fn scrollarea_ensure_visible_updates_scroll_position() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(500, 500));
        sa.set_viewport(Rect::new(0, 0, 100, 100));
        sa.set_scroll_position(100, 120);

        sa.ensure_visible(Rect::new(250, 275, 50, 50));
        assert_eq!(sa.scroll_position(), (200, 225));

        sa.ensure_visible(Rect::new(10, 20, 20, 20));
        assert_eq!(sa.scroll_position(), (10, 20));
        assert_eq!(sa.viewport(), Rect::new(0, 0, 100, 100));
    }

    #[test]
    fn scrollarea_extent_changes_reclamp_scroll_position() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_viewport(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(500, 500));
        sa.set_scroll_position(400, 400);

        sa.set_content_size(Size::new(150, 160));
        assert_eq!(sa.scroll_position(), (50, 60));

        sa.set_viewport(Rect::new(0, 0, 200, 200));
        assert_eq!(sa.scroll_position(), (0, 0));
    }

    #[test]
    fn scrollarea_ensure_widget_visible_changes_scroll_not_viewport() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(500, 500));
        sa.set_widget(Some(77));
        let viewport = sa.viewport();
        sa.ensure_widget_visible(77);
        assert_eq!(sa.viewport(), viewport);
        assert_eq!(sa.scroll_position(), (200, 200));
    }

    #[test]
    fn thumb_metrics_returns_full_track_when_content_fits() {
        // Content (50) fits inside the viewport (100): thumb spans the track
        // and never moves regardless of the requested scroll.
        let (len, pos) = ScrollArea::thumb_metrics(100, 50, 100, 30);
        assert_eq!(len, 100);
        assert_eq!(pos, 0);

        let (len, pos) = ScrollArea::thumb_metrics(100, 50, 100, -10);
        assert_eq!(len, 100);
        assert_eq!(pos, 0);
    }

    #[test]
    fn thumb_metrics_proportional_and_tracks_scroll() {
        // track 80, content 200, viewport 100 => thumb = 80*100/200 = 40.
        let (len0, pos0) = ScrollArea::thumb_metrics(80, 200, 100, 0);
        assert_eq!(len0, 40);
        assert_eq!(pos0, 0);

        // Halfway through the scrollable range the thumb is centered.
        let (len, pos) = ScrollArea::thumb_metrics(80, 200, 100, 50);
        assert_eq!(len, 40);
        assert_eq!(pos, 20);

        // Maximum scroll pins the thumb at the far end of the track.
        let (len, pos) = ScrollArea::thumb_metrics(80, 200, 100, 100);
        assert_eq!(len, 40);
        assert_eq!(pos, 40);

        // Out-of-range scroll values are clamped to the track.
        let (_, pos) = ScrollArea::thumb_metrics(80, 200, 100, -5);
        assert_eq!(pos, 0);
        let (_, pos) = ScrollArea::thumb_metrics(80, 200, 100, 999);
        assert_eq!(pos, 40);
    }

    #[test]
    fn thumb_metrics_never_collapses_below_minimum() {
        let (len, _) = ScrollArea::thumb_metrics(1000, 100_000, 100, 0);
        assert_eq!(len, 10);

        let (len, _) = ScrollArea::thumb_metrics(50, 100_000, 50, 0);
        assert_eq!(len, 10);
    }

    #[test]
    fn scrollarea_content_is_translated_by_scroll_position() {
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        use crate::widget::SimpleRegistry;
        use std::cell::RefCell;
        use std::rc::Rc;

        let size = Size::new(100, 100);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        let stride = 100 * 4;
        let pixel = |rgba: &[u8], x: usize, y: usize| {
            let idx = y * stride + x * 4;
            (rgba[idx], rgba[idx + 1], rgba[idx + 2])
        };

        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_geometry(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(200, 200));
        let reg = Rc::new(RefCell::new(SimpleRegistry::new()));
        sa.set_registry(reg.clone());
        let child_id = 4242;
        sa.set_widget(Some(child_id));
        reg.borrow_mut().register(
            child_id,
            |ctx| {
                // A red marker inside the content at content coordinates (60..90).
                ctx.fill_rect(Rect::new(60, 60, 30, 30), Color::RED);
            },
            |_| {},
        );

        // Unscrolled: the red marker stays at its content coordinates.
        backend.begin_frame(Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut backend);
            sa.draw(&mut ctx);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        assert_eq!(pixel(rgba, 70, 70), (255, 0, 0), "marker visible at content origin");
        assert_eq!(pixel(rgba, 10, 10), (255, 255, 255), "empty viewport area stays white");

        // Scroll down/right by (50, 50): the marker moves to (10..40).
        backend.begin_frame(Color::WHITE);
        sa.set_scroll_position(50, 50);
        {
            let mut ctx = RenderContext::new(&mut backend);
            sa.draw(&mut ctx);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        assert_eq!(pixel(rgba, 20, 20), (255, 0, 0), "marker translated into view");
        assert_eq!(pixel(rgba, 70, 70), (255, 255, 255), "marker scrolled out of its old spot");
    }

    #[test]
    fn scrollarea_forwards_pointer_coordinates_in_content_space() {
        use std::sync::{Arc, Mutex};

        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(300, 300));
        sa.set_scroll_position(40, 25);
        let registry = Rc::new(RefCell::new(SimpleRegistry::new()));
        sa.set_registry(registry.clone());
        let received = Arc::new(Mutex::new(None));
        let sink = received.clone();
        registry.borrow_mut().register(
            99,
            |_| {},
            move |event| {
                if let Event::MousePress { pos, .. } = event {
                    if let Ok(mut value) = sink.lock() {
                        *value = Some(*pos);
                    }
                }
            },
        );
        sa.set_widget(Some(99));

        sa.handle_event(&Event::mouse_press(10, 12, 1));
        assert_eq!(received.lock().ok().and_then(|value| *value), Some(Point::new(50, 37)));
    }
}
