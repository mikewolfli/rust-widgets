//! `ZoomPanController` — zoom, pan and ROI state for an image surface.

use crate::compat::ToString;
use crate::core::{Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{
    accent_color, expect_optional_rect, expect_point, expect_token, optional_rect_value,
    point_value, scaled_extent, surface_ink,
};

// ---------------------------------------------------------------------------
// COMP-10 ZoomPanController
// ---------------------------------------------------------------------------

/// How a view fits its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZoomMode {
    /// No automatic fit; `zoom_level` is authoritative.
    #[default]
    Free,
    /// Content width fills the viewport width.
    FitWidth,
    /// Content height fills the viewport height.
    FitHeight,
    /// 1:1 pixels.
    Original,
}

impl ZoomMode {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            ZoomMode::Free => "free",
            ZoomMode::FitWidth => "fit_width",
            ZoomMode::FitHeight => "fit_height",
            ZoomMode::Original => "original",
        }
    }
    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "free" => Some(ZoomMode::Free),
            "fit_width" => Some(ZoomMode::FitWidth),
            "fit_height" => Some(ZoomMode::FitHeight),
            "original" => Some(ZoomMode::Original),
            _ => None,
        }
    }
}

/// Zoom/pan/ROI state for an image or drawing surface.
///
/// The controller owns the view transform and converts the gestures a host forwards into it:
/// drag pans, wheel zooms, [`Self::fit_to_width`] fits, and [`Self::set_roi`] marks a region.
/// The host remains responsible for rendering its content through
/// [`Self::zoom_level`]/[`Self::pan_offset`].
pub struct ZoomPanController {
    base: BaseWidget,
    zoom_level: f32,
    pan_offset: Point,
    zoom_mode: ZoomMode,
    min_zoom: f32,
    max_zoom: f32,
    roi: Option<Rect>,
    content_size: Size,
    drag_last: Option<Point>,
    /// Emitted with the new zoom level.
    pub zoom_changed: Signal1<f32>,
    /// Emitted with the new pan offset.
    pub pan_changed: Signal1<Point>,
    /// Emitted with the new region of interest (`None` means cleared).
    pub roi_changed: Signal1<Option<Rect>>,
}

impl ZoomPanController {
    /// Creates a 1:1 controller with the given viewport geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ZoomPanController, geometry, "ZoomPanController"),
            zoom_level: 1.0,
            pan_offset: Point::new(0, 0),
            zoom_mode: ZoomMode::Free,
            min_zoom: 0.05,
            max_zoom: 32.0,
            roi: None,
            content_size: Size::new(1024, 768),
            drag_last: None,
            zoom_changed: Signal1::new(),
            pan_changed: Signal1::new(),
            roi_changed: Signal1::new(),
        }
    }

    /// Returns the current zoom level.
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Sets the zoom level, clamped to the configured range.
    pub fn set_zoom_level(&mut self, zoom_level: f32) {
        if !zoom_level.is_finite() {
            return;
        }
        let zoom_level = zoom_level.clamp(self.min_zoom, self.max_zoom);
        if self.zoom_level != zoom_level {
            self.zoom_level = zoom_level;
            self.base.request_redraw();
            self.zoom_changed.emit(zoom_level);
        }
    }

    /// Returns the pan offset.
    pub fn pan_offset(&self) -> Point {
        self.pan_offset
    }

    /// Sets the pan offset.
    pub fn set_pan_offset(&mut self, pan_offset: Point) {
        if self.pan_offset != pan_offset {
            self.pan_offset = pan_offset;
            self.base.request_redraw();
            self.pan_changed.emit(pan_offset);
        }
    }

    /// Returns the current zoom mode.
    pub fn zoom_mode(&self) -> ZoomMode {
        self.zoom_mode
    }

    /// Sets the zoom mode; a fit mode does not recompute until its fit method is called.
    pub fn set_zoom_mode(&mut self, zoom_mode: ZoomMode) {
        self.zoom_mode = zoom_mode;
    }

    /// Returns the minimum zoom.
    pub fn min_zoom(&self) -> f32 {
        self.min_zoom
    }

    /// Returns the maximum zoom.
    pub fn max_zoom(&self) -> f32 {
        self.max_zoom
    }

    /// Sets the zoom range, swapping an inverted pair rather than producing an empty range.
    pub fn set_zoom_range(&mut self, min_zoom: f32, max_zoom: f32) {
        if !min_zoom.is_finite() || !max_zoom.is_finite() || min_zoom <= 0.0 {
            return;
        }
        let (min_zoom, max_zoom) =
            if min_zoom <= max_zoom { (min_zoom, max_zoom) } else { (max_zoom, min_zoom) };
        self.min_zoom = min_zoom;
        self.max_zoom = max_zoom;
        self.set_zoom_level(self.zoom_level);
    }

    /// Returns the region of interest.
    pub fn roi(&self) -> Option<Rect> {
        self.roi
    }

    /// Sets the region of interest.
    pub fn set_roi(&mut self, roi: Rect) {
        if self.roi != Some(roi) {
            self.roi = Some(roi);
            self.base.request_redraw();
            self.roi_changed.emit(self.roi);
        }
    }

    /// Clears the region of interest.
    pub fn clear_roi(&mut self) {
        if self.roi.is_some() {
            self.roi = None;
            self.base.request_redraw();
            self.roi_changed.emit(None);
        }
    }

    /// Sets the unscaled content size used by the fit modes.
    pub fn set_content_size(&mut self, content_size: Size) {
        self.content_size = content_size;
    }

    /// Returns the unscaled content size.
    pub fn content_size(&self) -> Size {
        self.content_size
    }

    /// Zooms in by one step.
    pub fn zoom_in(&mut self) {
        self.zoom_mode = ZoomMode::Free;
        self.set_zoom_level(self.zoom_level * 1.25);
    }

    /// Zooms out by one step.
    pub fn zoom_out(&mut self) {
        self.zoom_mode = ZoomMode::Free;
        self.set_zoom_level(self.zoom_level / 1.25);
    }

    /// Fits the content width to the viewport width.
    pub fn fit_to_width(&mut self) {
        self.zoom_mode = ZoomMode::FitWidth;
        let viewport = self.geometry().width.max(1) as f32;
        let content = self.content_size.width.max(1) as f32;
        self.set_pan_offset(Point::new(0, 0));
        self.set_zoom_level(viewport / content);
    }

    /// Fits the content height to the viewport height.
    pub fn fit_to_height(&mut self) {
        self.zoom_mode = ZoomMode::FitHeight;
        let viewport = self.geometry().height.max(1) as f32;
        let content = self.content_size.height.max(1) as f32;
        self.set_pan_offset(Point::new(0, 0));
        self.set_zoom_level(viewport / content);
    }

    /// Returns to 1:1 with no pan.
    pub fn reset_view(&mut self) {
        self.zoom_mode = ZoomMode::Original;
        self.set_pan_offset(Point::new(0, 0));
        self.set_zoom_level(1.0);
    }

    /// The transformed content rectangle inside the viewport.
    fn content_rect(&self) -> Rect {
        let viewport = self.geometry();
        let width = scaled_extent(self.content_size.width, self.zoom_level);
        let height = scaled_extent(self.content_size.height, self.zoom_level);
        let x = viewport.x as i64
            + (viewport.width as i64 - i64::from(width)) / 2
            + i64::from(self.pan_offset.x);
        let y = viewport.y as i64
            + (viewport.height as i64 - i64::from(height)) / 2
            + i64::from(self.pan_offset.y);
        Rect::new(
            x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            width,
            height,
        )
    }
}

impl EventHandler for ZoomPanController {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => self.drag_last = Some(*pos),
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                if let Some(last) = self.drag_last.take() {
                    let pan = Point::new(
                        self.pan_offset.x.saturating_add(pos.x.saturating_sub(last.x)),
                        self.pan_offset.y.saturating_add(pos.y.saturating_sub(last.y)),
                    );
                    self.set_pan_offset(pan);
                    self.drag_last = Some(*pos);
                }
            }
            Event::MouseRelease { .. }
            | Event::PointerRelease { .. }
            | Event::MouseLeave { .. } => {
                self.drag_last = None;
            }
            Event::Wheel { delta, .. } if delta.y != 0 => {
                self.zoom_mode = ZoomMode::Free;
                let factor = if delta.y < 0 { 1.25 } else { 0.8 };
                self.set_zoom_level(self.zoom_level * factor);
            }
            _ => {}
        }
    }
}

/// The overlap of two rectangles, or an empty rectangle when they do not meet.
fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width as i32).min(b.x + b.width as i32);
    let bottom = (a.y + a.height as i32).min(b.y + b.height as i32);
    Rect::new(x, y, right.saturating_sub(x).max(0) as u32, bottom.saturating_sub(y).max(0) as u32)
}

impl Draw for ZoomPanController {
    fn draw(&mut self, context: &mut RenderContext) {
        let viewport = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        context.fill_rect(viewport, surface);
        context.draw_rect(viewport, surface.blend(&ink, 0.20));

        let content = intersect_rect(self.content_rect(), viewport);
        if content.width == 0 || content.height == 0 {
            return;
        }
        context.push_clip(viewport.x, viewport.y, viewport.width, viewport.height);
        context.fill_rect(content, surface.blend(&ink, 0.06));
        context.draw_rect(content, surface.blend(&ink, 0.35));
        // A coarse grid makes pan and zoom observable without image pixels.
        for step in 1..4 {
            let x = content.x + (content.width as i64 * i64::from(step) / 4) as i32;
            let y = content.y + (content.height as i64 * i64::from(step) / 4) as i32;
            context.draw_line(
                Point::new(x, content.y),
                Point::new(x, content.y + content.height as i32),
                surface.blend(&ink, 0.12),
            );
            context.draw_line(
                Point::new(content.x, y),
                Point::new(content.x + content.width as i32, y),
                surface.blend(&ink, 0.12),
            );
        }

        if let Some(roi) = self.roi {
            let x = content.x as i64 + (roi.x as f32 * self.zoom_level).round() as i64;
            let y = content.y as i64 + (roi.y as f32 * self.zoom_level).round() as i64;
            let width = scaled_extent(roi.width, self.zoom_level);
            let height = scaled_extent(roi.height, self.zoom_level);
            let rect = Rect::new(
                x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                width,
                height,
            );
            let rect = intersect_rect(rect, viewport);
            if rect.width > 0 && rect.height > 0 {
                context.fill_rect(rect, accent.with_alpha(40));
                context.draw_rect(rect, accent);
            }
        }
        context.pop_clip();
    }
}

impl Widget for ZoomPanController {
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
        Size::new(320, 240)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "zoom_changed" => {
                Some(EventSignalRef::mapped("zoom_changed", &self.zoom_changed, |value| {
                    CapabilityValue::Float(f64::from(*value))
                }))
            }
            "pan_changed" => {
                Some(EventSignalRef::mapped("pan_changed", &self.pan_changed, |value| {
                    point_value(*value)
                }))
            }
            "roi_changed" => {
                Some(EventSignalRef::mapped("roi_changed", &self.roi_changed, |value| {
                    optional_rect_value(*value)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ZoomPanController {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "zoom_level" => Ok(CapabilityValue::Float(f64::from(self.zoom_level))),
            "pan_offset" => Ok(point_value(self.pan_offset)),
            "zoom_mode" => Ok(CapabilityValue::String(self.zoom_mode.as_str().to_string())),
            "min_zoom" => Ok(CapabilityValue::Float(f64::from(self.min_zoom))),
            "max_zoom" => Ok(CapabilityValue::Float(f64::from(self.max_zoom))),
            "roi" => Ok(optional_rect_value(self.roi)),
            // The content extent is one `Size`; the property vocabulary has no size kind, so it is
            // published as the two scalars every pair-shaped value in this crate uses.
            "content_width" => Ok(CapabilityValue::UInt(u64::from(self.content_size.width))),
            "content_height" => Ok(CapabilityValue::UInt(u64::from(self.content_size.height))),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "zoom_level" => {
                self.set_zoom_level(expect_f32(value)?);
                Ok(())
            }
            "pan_offset" => {
                self.set_pan_offset(expect_point(value)?);
                Ok(())
            }
            "zoom_mode" => {
                self.set_zoom_mode(expect_token(value, ZoomMode::from_name)?);
                Ok(())
            }
            "min_zoom" => {
                let min = expect_f32(value)?;
                self.set_zoom_range(min, self.max_zoom);
                Ok(())
            }
            "max_zoom" => {
                let max = expect_f32(value)?;
                self.set_zoom_range(self.min_zoom, max);
                Ok(())
            }
            "roi" => {
                match expect_optional_rect(value)? {
                    Some(roi) => self.set_roi(roi),
                    None => self.clear_roi(),
                }
                Ok(())
            }
            // Each half takes the live value of the other, exactly like the viewport pair on
            // `gantt_widget`; the content extent the fit modes read is the one set here.
            "content_width" => {
                let width = match value {
                    CapabilityValue::UInt(width) => {
                        u32::try_from(width).map_err(|_| CapabilityAccessError::TypeMismatch)?
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_content_size(Size::new(width, self.content_size.height));
                Ok(())
            }
            "content_height" => {
                let height = match value {
                    CapabilityValue::UInt(height) => {
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_content_size(Size::new(self.content_size.width, height));
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "zoom_level",
            "pan_offset",
            "zoom_mode",
            "min_zoom",
            "max_zoom",
            "roi",
            "content_width",
            "content_height",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_is_clamped_to_the_configured_range() {
        let mut controller = ZoomPanController::new(Rect::new(0, 0, 320, 240));
        controller.set_zoom_range(0.5, 4.0);
        controller.set_zoom_level(100.0);
        assert_eq!(controller.zoom_level(), 4.0);
        // A non-finite write is ignored rather than stored.
        controller.set_zoom_level(f32::NEG_INFINITY);
        assert_eq!(controller.zoom_level(), 4.0);
        // An inverted range is swapped instead of becoming empty.
        controller.set_zoom_range(8.0, 2.0);
        assert_eq!(controller.min_zoom(), 2.0);
        assert_eq!(controller.max_zoom(), 8.0);
        assert_eq!(controller.zoom_level(), 4.0);
    }

    /// The content extent is writable through its two published scalars, each taking the live
    /// value of the other axis.
    #[test]
    fn content_size_round_trips_through_its_scalar_pair() {
        let mut controller = ZoomPanController::new(Rect::new(0, 0, 320, 240));
        assert_eq!(controller.get("content_width").unwrap(), CapabilityValue::UInt(1024));
        assert_eq!(controller.get("content_height").unwrap(), CapabilityValue::UInt(768));
        controller
            .set("content_width", CapabilityValue::UInt(640))
            .expect("content_width writable");
        controller
            .set("content_height", CapabilityValue::UInt(480))
            .expect("content_height writable");
        assert_eq!(controller.content_size(), Size::new(640, 480));
        assert!(controller.set("content_width", CapabilityValue::Float(1.0)).is_err());
    }
}
