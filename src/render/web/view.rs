//! Visual rendering helpers for WebView widget.
//!
//! Provides rendering support for `WebViewEnhanced` by delegating
//! visual commands to the core web rendering pipeline.

use std::cell::Cell;

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
    /// Current scroll offset.
    scroll_offset: Point,
    /// Flag set when a redraw has been requested.
    redraw_requested: Cell<bool>,
}

impl WebView {
    /// Create a new web view rendering adapter.
    pub fn new(widget_id: ObjectId) -> Self {
        Self {
            widget_id,
            rect: Rect::default(),
            scroll_offset: Point::origin(),
            redraw_requested: Cell::new(false),
        }
    }

    /// Return the widget id this renderer is attached to.
    pub fn widget_id(&self) -> ObjectId {
        self.widget_id
    }

    /// Return the widget kind constant.
    pub fn widget_kind(&self) -> WidgetKind {
        WidgetKind::WebEngineView
    }

    /// Update the display rectangle.
    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    /// Return the current display rectangle, adjusted for scroll offset.
    pub fn rect(&self) -> Rect {
        Rect::new(
            self.rect.x + self.scroll_offset.x,
            self.rect.y + self.scroll_offset.y,
            self.rect.width,
            self.rect.height,
        )
    }

    /// Request a redraw of this web view.
    pub fn request_redraw(&self) {
        self.redraw_requested.set(true);
    }

    /// Returns whether a redraw has been requested and clears the flag.
    pub fn take_redraw_requested(&self) -> bool {
        self.redraw_requested.replace(false)
    }

    /// Update the internal scroll offset.
    pub fn set_scroll_offset(&mut self, offset: Point) {
        self.scroll_offset = offset;
    }

    /// Return the current scroll offset.
    pub fn scroll_offset(&self) -> Point {
        self.scroll_offset
    }

    /// Return the preferred visual size for this view, adjusted for scroll offset.
    pub fn preferred_size(&self) -> Size {
        let base = Size::new(800, 600);
        Size::new(
            base.width.wrapping_add(self.scroll_offset.x.unsigned_abs()),
            base.height.wrapping_add(self.scroll_offset.y.unsigned_abs()),
        )
    }
}

impl Default for WebView {
    fn default() -> Self {
        Self::new(ObjectId::default())
    }
}
