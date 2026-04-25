#![allow(dead_code)]

//! Visual rendering helpers for WebView widget.
//!
//! Provides rendering support for `WebViewEnhanced` by delegating
//! visual commands to the core web rendering pipeline.

use crate::core::{ObjectId, Point, Rect, Size};
use crate::widget::WidgetKind;

/// Web view rendering adapter.
///
/// Bridges the `WebViewEnhanced` widget with the render pipeline.
/// Stores the widget id and delegates visual commands to the
/// core web module rendering functions.
#[derive(Debug, Clone)]
pub struct WebView {
    /// The widget id this view renderer is attached to.
    widget_id: ObjectId,
    /// The display rectangle of the web view.
    rect: Rect,
}

impl WebView {
    /// Create a new web view rendering adapter.
    pub fn new(widget_id: ObjectId) -> Self {
        Self {
            widget_id,
            rect: Rect::default(),
        }
    }

    /// Return the widget id this renderer is attached to.
    pub fn widget_id(&self) -> ObjectId {
        self.widget_id
    }

    /// Return the widget kind constant.
    pub fn widget_kind(&self) -> WidgetKind {
        WidgetKind::WebView
    }

    /// Update the display rectangle.
    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    /// Return the current display rectangle.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Request a redraw of this web view.
    pub fn request_redraw(&self) {
        // Delegates to the widget system; the widget itself tracks
        // the redraw signal via its BaseWidget.
    }

    /// Update the internal scroll offset (no-op for flat web views).
    pub fn set_scroll_offset(&mut self, _offset: Point) {}

    /// Return the preferred visual size for this view.
    pub fn preferred_size(&self) -> Size {
        Size::new(800, 600)
    }
}

impl Default for WebView {
    fn default() -> Self {
        Self::new(ObjectId::default())
    }
}
