// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Popup window widget.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Popup window widget.
pub struct PopupWindow {
    base: BaseWidget,
    content_widget: Option<ObjectId>,
    /// Title shown in the popup's own chrome.
    ///
    /// Kept on the control rather than in a host-side map: the title is part of
    /// what the popup *is*, and a host that held it separately could not paint it.
    title: String,
    /// Emitted when the popup is opened.
    pub opened: GenericSignal,
    /// Emitted when the popup is closed.
    pub closed: GenericSignal,
}
impl PopupWindow {
    /// Creates a popup window with geometry and no title.
    pub fn new(geometry: Rect) -> Self {
        Self::with_title(String::new(), geometry)
    }

    /// Creates a popup window with a title and geometry.
    pub fn with_title(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupWindow"),
            content_widget: None,
            title,
            opened: GenericSignal::new(),
            closed: GenericSignal::new(),
        }
    }

    /// Returns the popup's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the popup's title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }

    /// Returns the content widget ID, if any.
    pub fn content_widget(&self) -> Option<ObjectId> {
        self.content_widget
    }

    /// Sets the content widget for this popup.
    pub fn set_content_widget(&mut self, widget: Option<ObjectId>) {
        if let Some(old) = self.content_widget {
            self.base.remove_child(old);
        }
        self.content_widget = widget;
        if let Some(id) = widget {
            self.base.add_child(id);
        }
        self.base.request_redraw();
    }

    /// Shows the popup and emits `opened`.
    pub fn open(&mut self) {
        self.show();
        self.opened.emit();
    }

    /// Hides the popup and emits `closed`.
    pub fn close(&mut self) {
        self.hide();
        self.closed.emit();
    }
}
impl Widget for PopupWindow {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `PopupWindow` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
}

/// `PopupWindow`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. `has_content` is read-only: a popup's
/// content is supplied by its owner, not through the property layer.
impl WidgetProperties for PopupWindow {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "has_content" => Ok(CapabilityValue::Bool(self.content_widget().is_some())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            // A popup's content is supplied by its owner via `set_content_widget`,
            // not through the property layer: the property reports presence only.
            "has_content" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "has_content", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `popup_window` publishes.
    ///
    /// `set_content_widget` takes an `ObjectId` payload, so it is answered through
    /// the caller's own handle rather than the scalar property route; reporting
    /// `OutOfRange` for a payload-less call is the established convention.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_content_widget" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl Draw for PopupWindow {
    /// Paints the popup's own chrome, then its content child.
    ///
    /// # Why the title is painted here
    ///
    /// The struct documents the title as "part of what the popup *is*" and keeps it
    /// on the control "rather than in a host-side map: ... a host that held it
    /// separately could not paint it". That reasoning only holds if this function
    /// actually paints it, and it did not — the title was stored, published as a
    /// read/write property, and never rendered. An empty title now means "no title
    /// bar", so a popup created through [`PopupWindow::new`] keeps the titleless
    /// chrome it had before.
    ///
    /// # Child clipping
    ///
    /// A content child is drawn by the host's tree walk, not from here, so this only
    /// needs to leave the content area free of chrome. The title bar is taken out of
    /// the top of the content rect so a child laid out at the popup's origin does not
    /// sit under the title text.
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so
        // a light/dark switch left the popup and its title bar unchanged — the rendering
        // census reported the control as theme-blind.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("popup_window");
        // `popup_window` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // popup painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from
        // it: the popup is raised *above* the window, so the step is taken toward the
        // foreground, the same distinction `Colors::input_background` draws for a field.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(40, 40, 40));
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.35));
        // The title bar is a distinct band on the popup, derived from it so the two stay one
        // visible step apart in either appearance.
        let title_bar = surface.blend(&ink, 0.06);

        // Background and border. The fill is opaque on purpose — a popup that wants
        // translucency has to say so through its own background property, and the
        // earlier "semi-transparent effect" comment described something this code
        // never did.
        context.fill_rect(rect, surface);
        context.draw_rect(rect, border);

        if self.title.is_empty() {
            return;
        }
        // Title bar height is clamped to the popup so a short popup shows the title
        // rather than painting outside itself.
        let bar_height = TITLE_BAR_HEIGHT.min(rect.height);
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, bar_height), title_bar);
        context.draw_line(
            Point::new(rect.x, rect.y + bar_height as i32),
            Point::new(rect.x + rect.width as i32, rect.y + bar_height as i32),
            border,
        );
        // The title is centred inside the bar. The origin is the glyph's **top** edge, so the
        // offset is half the difference between the bar and the line box; using the bar's own
        // midline put the glyph's top *at* the centre, leaving a 14 px title spanning 12..26 in
        // a 24 px bar — two pixels over the separator below it.
        let title_font = Font::default();
        let title_h = context.measure_text("M", &title_font).height;
        context.draw_text_fitted(
            Rect::new(
                rect.x + 8,
                rect.y + (bar_height as i32 - title_h as i32) / 2,
                rect.width.saturating_sub(16),
                title_h,
            ),
            &self.title,
            &title_font,
            ink,
            HorizontalAlignment::Left,
        );
    }
}

/// Height of the popup's title bar, in pixels.
///
/// Also the amount the content area is inset by when a title is present, so the two
/// cannot drift apart.
pub const TITLE_BAR_HEIGHT: u32 = 24;

impl PopupWindow {
    /// The rectangle available to the content child.
    ///
    /// Insets the top by the title bar **only when a title is set**, so a titleless
    /// popup gives its child the full rect.
    pub fn content_rect(&self) -> Rect {
        let rect = self.base.geometry();
        if self.title.is_empty() {
            return rect;
        }
        let inset = TITLE_BAR_HEIGHT.min(rect.height);
        Rect::new(rect.x, rect.y + inset as i32, rect.width, rect.height - inset)
    }
}
impl crate::event::EventHandler for PopupWindow {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
            }
            crate::event::Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::Object;
    use std::sync::{Arc, Mutex};

    #[test]
    fn popup_open_close_emits_lifecycle_signals() {
        let mut popup = PopupWindow::new(Rect::new(0, 0, 120, 80));
        let opened = Arc::new(Mutex::new(0usize));
        let closed = Arc::new(Mutex::new(0usize));

        let opened_sink = opened.clone();
        popup.opened.connect(move || {
            if let Ok(mut count) = opened_sink.lock() {
                *count += 1;
            }
        });

        let closed_sink = closed.clone();
        popup.closed.connect(move || {
            if let Ok(mut count) = closed_sink.lock() {
                *count += 1;
            }
        });

        popup.open();
        popup.close();

        assert!(!popup.is_visible());
        assert_eq!(*opened.lock().expect("opened lock poisoned"), 1);
        assert_eq!(*closed.lock().expect("closed lock poisoned"), 1);
    }

    #[test]
    fn popup_replaces_content_widget_child_binding() {
        let mut popup = PopupWindow::new(Rect::new(0, 0, 120, 80));
        let old_id = Object::new("OldContent").id();
        let new_id = Object::new("NewContent").id();

        popup.set_content_widget(Some(old_id));
        assert_eq!(popup.content_widget(), Some(old_id));
        assert_eq!(popup.children(), &[old_id]);

        popup.set_content_widget(Some(new_id));
        assert_eq!(popup.content_widget(), Some(new_id));
        assert_eq!(popup.children(), &[new_id]);
    }

    /// The title must reach the pixels, not just the property table.
    ///
    /// `PopupWindow` stores a title, publishes it as a read/write property, and
    /// documents it as something the control owns *because* a host could not paint it.
    /// None of that was true while `draw` ignored the field. This asserts the rendered
    /// frame differs once a title is set, which is the only evidence that survives a
    /// future refactor of the chrome.
    #[test]
    fn popup_with_a_title_paints_chrome_the_titleless_one_does_not() {
        let mut titleless = PopupWindow::new(Rect::new(0, 0, 160, 100));
        let mut titled = PopupWindow::with_title("Details".to_string(), Rect::new(0, 0, 160, 100));

        let plain = crate::widget::svg::render_to_svg(&mut titleless);
        let decorated = crate::widget::svg::render_to_svg(&mut titled);

        assert_ne!(plain, decorated, "a titled popup must paint more than a titleless one");
        // The title is `font8x8` glyph geometry rather than a `<text>` element, so the string is
        // not in the document and the check is on **where the ink landed**: the titled popup
        // gains a run on its title bar, and the titleless one paints no text at all.
        let (left, top, right, bottom) = crate::widget::svg::text_ink_box(&decorated)
            .unwrap_or_else(|| panic!("a titled popup must paint its title: {decorated}"));
        assert!(right > left, "the title laid down ink: {left}..{right}");
        assert!(
            (0..TITLE_BAR_HEIGHT as i32).contains(&top) && bottom <= TITLE_BAR_HEIGHT as i32,
            "the title must sit on the title bar, got {top}..{bottom}"
        );
        assert!(
            crate::widget::svg::text_ink_box(&plain).is_none(),
            "a titleless popup paints no title ink: {plain}"
        );
    }

    /// A titleless popup keeps the whole rect for its content.
    #[test]
    fn popup_content_rect_insets_only_when_a_title_is_present() {
        let titleless = PopupWindow::new(Rect::new(0, 0, 160, 100));
        assert_eq!(titleless.content_rect(), Rect::new(0, 0, 160, 100));

        let titled = PopupWindow::with_title("T".to_string(), Rect::new(0, 0, 160, 100));
        assert_eq!(
            titled.content_rect(),
            Rect::new(0, TITLE_BAR_HEIGHT as i32, 160, 100 - TITLE_BAR_HEIGHT)
        );
    }

    /// A popup shorter than its own title bar must still render inside itself.
    #[test]
    fn popup_title_bar_clamps_to_a_short_popup() {
        let mut tiny = PopupWindow::with_title("T".to_string(), Rect::new(0, 0, 60, 8));
        // The assertion is that drawing does not panic and does not escape the rect;
        // the clamp is what makes the second half true.
        let _ = crate::widget::svg::render_to_svg(&mut tiny);
        assert_eq!(tiny.content_rect().height, 0);
    }
}
