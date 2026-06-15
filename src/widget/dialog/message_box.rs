//! Message box dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
#[cfg(feature = "desktop")]
use crate::tr;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Message box icon type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxIcon {
    NoIcon,
    Information,
    Question,
    Warning,
    Critical,
}
/// Standard buttons for message boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardButton {
    Ok,
    Cancel,
    Yes,
    No,
    YesAll,
    NoAll,
    Save,
    Discard,
    Apply,
    Close,
    Abort,
    Retry,
    Ignore,
    Help,
}
impl StandardButton {
    pub fn label(&self) -> &'static str {
        match self {
            StandardButton::Ok => "OK",
            StandardButton::Cancel => "Cancel",
            StandardButton::Yes => "Yes",
            StandardButton::No => "No",
            StandardButton::YesAll => "Yes to All",
            StandardButton::NoAll => "No to All",
            StandardButton::Save => "Save",
            StandardButton::Discard => "Discard",
            StandardButton::Apply => "Apply",
            StandardButton::Close => "Close",
            StandardButton::Abort => "Abort",
            StandardButton::Retry => "Retry",
            StandardButton::Ignore => "Ignore",
            StandardButton::Help => "Help",
        }
    }

    /// Returns the translated label for this button using the i18n system.
    /// Available only on desktop target with i18n support enabled.
    #[cfg(feature = "desktop")]
    pub fn translated_label(&self) -> String {
        match self {
            StandardButton::Ok => tr!("common.button.ok"),
            StandardButton::Cancel => tr!("common.button.cancel"),
            StandardButton::Yes => tr!("common.button.yes"),
            StandardButton::No => tr!("common.button.no"),
            StandardButton::YesAll => tr!("common.button.yes_all"),
            StandardButton::NoAll => tr!("common.button.no_all"),
            StandardButton::Save => tr!("common.button.save"),
            StandardButton::Discard => tr!("common.button.discard"),
            StandardButton::Apply => tr!("common.button.apply"),
            StandardButton::Close => tr!("common.button.close"),
            StandardButton::Abort => tr!("common.button.abort"),
            StandardButton::Retry => tr!("common.button.retry"),
            StandardButton::Ignore => tr!("common.button.ignore"),
            StandardButton::Help => tr!("common.button.help"),
        }
    }

    /// Returns the translated label for this button using the i18n system.
    /// Fallback version when i18n is not compiled in — returns English label.
    #[cfg(not(feature = "desktop"))]
    pub fn translated_label(&self) -> String {
        self.label().to_string()
    }
}
/// Message box dialog.
pub struct MessageBox {
    base: BaseWidget,
    title: String,
    text: String,
    icon: MessageBoxIcon,
    buttons: Vec<StandardButton>,
    default_button: Option<StandardButton>,
    modal: bool,
    pub button_clicked: Signal1<StandardButton>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}
impl MessageBox {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MessageBox, geometry, "MessageBox"),
            title: String::new(),
            text: String::new(),
            icon: MessageBoxIcon::NoIcon,
            buttons: vec![StandardButton::Ok],
            default_button: Some(StandardButton::Ok),
            modal: true,
            button_clicked: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    pub fn question(geometry: Rect, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut mb = Self::new(geometry);
        mb.title = title.into();
        mb.text = text.into();
        mb.icon = MessageBoxIcon::Question;
        mb.buttons = vec![StandardButton::Yes, StandardButton::No];
        mb.default_button = Some(StandardButton::Yes);
        mb
    }
    pub fn information(geometry: Rect, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut mb = Self::new(geometry);
        mb.title = title.into();
        mb.text = text.into();
        mb.icon = MessageBoxIcon::Information;
        mb
    }
    pub fn warning(geometry: Rect, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut mb = Self::new(geometry);
        mb.title = title.into();
        mb.text = text.into();
        mb.icon = MessageBoxIcon::Warning;
        mb
    }
    pub fn critical(geometry: Rect, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut mb = Self::new(geometry);
        mb.title = title.into();
        mb.text = text.into();
        mb.icon = MessageBoxIcon::Critical;
        mb
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn icon(&self) -> MessageBoxIcon {
        self.icon
    }
    pub fn buttons(&self) -> &[StandardButton] {
        &self.buttons
    }
    pub fn default_button(&self) -> Option<StandardButton> {
        self.default_button
    }
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
    pub fn set_icon(&mut self, icon: MessageBoxIcon) {
        self.icon = icon;
    }
    pub fn set_buttons(&mut self, buttons: Vec<StandardButton>) {
        self.buttons = buttons;
    }
    pub fn set_default_button(&mut self, btn: StandardButton) {
        self.default_button = Some(btn);
    }
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }

    pub fn click_button(&mut self, btn: StandardButton) {
        self.button_clicked.emit(btn);
        match btn {
            StandardButton::Ok
            | StandardButton::Yes
            | StandardButton::Save
            | StandardButton::Apply => {
                self.accepted.emit();
            }
            _ => {
                self.rejected.emit();
            }
        }
    }
    fn icon_symbol(&self) -> &'static str {
        match self.icon {
            MessageBoxIcon::Information => "ℹ",
            MessageBoxIcon::Question => "?",
            MessageBoxIcon::Warning => "⚠",
            MessageBoxIcon::Critical => "✗",
            MessageBoxIcon::NoIcon => "",
        }
    }
    fn icon_color(&self) -> Color {
        match self.icon {
            MessageBoxIcon::Information => Color::rgb(0, 120, 215),
            MessageBoxIcon::Question => Color::rgb(0, 120, 215),
            MessageBoxIcon::Warning => Color::rgb(255, 140, 0),
            MessageBoxIcon::Critical => Color::rgb(196, 43, 28),
            MessageBoxIcon::NoIcon => Color::rgb(0, 0, 0),
        }
    }
}
impl Widget for MessageBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for MessageBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, .. } = event {
            if *key == 13 {
                // Enter → default button
                if let Some(btn) = self.default_button {
                    self.click_button(btn);
                }
            } else if *key == 27 {
                // Escape → Cancel/No
                if self.buttons.contains(&StandardButton::Cancel) {
                    self.click_button(StandardButton::Cancel);
                } else if self.buttons.contains(&StandardButton::No) {
                    self.click_button(StandardButton::No);
                } else if self.buttons.contains(&StandardButton::Close) {
                    self.click_button(StandardButton::Close);
                }
            }
        }
    }
}
impl Draw for MessageBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // Dialog background
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(160, 160, 160),
        );
        // Title bar
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, 28u32), Color::rgb(0, 120, 215));
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &self.title,
            &Font::default(),
            Color::rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        // Icon
        let icon_sym = self.icon_symbol();
        if !icon_sym.is_empty() {
            context.draw_text(
                Point::new(rect.x + 20, rect.y + 60),
                icon_sym,
                &Font::default(),
                self.icon_color(),
                HorizontalAlignment::Left,
            );
        }
        // Message text
        let text_x = if self.icon == MessageBoxIcon::NoIcon { rect.x + 12 } else { rect.x + 60 };
        context.draw_text(
            Point::new(text_x, rect.y + 60),
            &self.text,
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
        // Buttons
        let btn_h = 28f32;
        let btn_w = 80f32;
        let btn_y = rect.y as f32 + rect.height as f32 - btn_h - 12.0;
        let total_btn_w = self.buttons.len() as f32 * (btn_w + 8.0);
        let mut btn_x = rect.x as f32 + rect.width as f32 - total_btn_w;
        for btn in &self.buttons {
            let is_default = self.default_button == Some(*btn);
            let bg = if is_default { Color::rgb(0, 120, 215) } else { Color::rgb(225, 225, 225) };
            let fg = if is_default { Color::rgb(255, 255, 255) } else { Color::rgb(0, 0, 0) };
            context.fill_rect(Rect::from_f32(btn_x, btn_y, btn_w, btn_h), bg);
            context
                .draw_rect(Rect::from_f32(btn_x, btn_y, btn_w, btn_h), Color::rgb(100, 100, 100));
            context.draw_text(
                Point::from_f32(btn_x + btn_w / 2.0, btn_y + btn_h / 2.0),
                &btn.translated_label(),
                &Font::default(),
                fg,
                HorizontalAlignment::Left,
            );
            btn_x += btn_w + 8.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::event::Event;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    // ── 1. Creating default message box ─────────────────────────────

    #[test]
    fn test_default_creation() {
        let mb = MessageBox::new(Rect::new(100, 100, 300, 150));

        assert_eq!(mb.kind(), WidgetKind::MessageBox);
        assert_eq!(mb.geometry(), Rect::new(100, 100, 300, 150));
        assert!(mb.title().is_empty());
        assert!(mb.text().is_empty());
        assert_eq!(mb.icon(), MessageBoxIcon::NoIcon);
        assert_eq!(mb.buttons(), &[StandardButton::Ok]);
        assert_eq!(mb.default_button(), Some(StandardButton::Ok));
        assert!(mb.is_modal());
        assert!(mb.is_visible());
        assert!(mb.is_enabled());
    }

    // ── 2. Setting title and message text ───────────────────────────

    #[test]
    fn test_set_title_and_text() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert!(mb.title().is_empty());
        assert!(mb.text().is_empty());

        mb.set_title("Warning");
        assert_eq!(mb.title(), "Warning");

        mb.set_text("Are you sure?");
        assert_eq!(mb.text(), "Are you sure?");
    }

    // ── 3. Setting icon type ────────────────────────────────────────

    #[test]
    fn test_set_icon() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert_eq!(mb.icon(), MessageBoxIcon::NoIcon);

        mb.set_icon(MessageBoxIcon::Information);
        assert_eq!(mb.icon(), MessageBoxIcon::Information);

        mb.set_icon(MessageBoxIcon::Warning);
        assert_eq!(mb.icon(), MessageBoxIcon::Warning);

        mb.set_icon(MessageBoxIcon::Critical);
        assert_eq!(mb.icon(), MessageBoxIcon::Critical);

        mb.set_icon(MessageBoxIcon::Question);
        assert_eq!(mb.icon(), MessageBoxIcon::Question);

        mb.set_icon(MessageBoxIcon::NoIcon);
        assert_eq!(mb.icon(), MessageBoxIcon::NoIcon);
    }

    // ── 4. Setting buttons ──────────────────────────────────────────

    #[test]
    fn test_set_buttons() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        // Default is Ok
        assert_eq!(mb.buttons(), &[StandardButton::Ok]);

        // OK/Cancel
        mb.set_buttons(vec![StandardButton::Ok, StandardButton::Cancel]);
        assert_eq!(mb.buttons(), &[StandardButton::Ok, StandardButton::Cancel]);

        // Yes/No
        mb.set_buttons(vec![StandardButton::Yes, StandardButton::No]);
        assert_eq!(mb.buttons(), &[StandardButton::Yes, StandardButton::No]);

        // Yes/No/Cancel
        mb.set_buttons(vec![StandardButton::Yes, StandardButton::No, StandardButton::Cancel]);
        assert_eq!(mb.buttons().len(), 3);
    }

    // ── 5. Default button ───────────────────────────────────────────

    #[test]
    fn test_default_button() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert_eq!(mb.default_button(), Some(StandardButton::Ok));

        mb.set_default_button(StandardButton::Cancel);
        assert_eq!(mb.default_button(), Some(StandardButton::Cancel));
    }

    // ── 6. Signal accessors ─────────────────────────────────────────

    #[test]
    fn test_accepted_signal() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        let fired = Arc::new(Mutex::new(false));
        mb.accepted.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        mb.click_button(StandardButton::Ok);
        assert!(*fired.lock().unwrap());
    }

    #[test]
    fn test_rejected_signal() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        let fired = Arc::new(Mutex::new(false));
        mb.rejected.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        mb.click_button(StandardButton::Cancel);
        assert!(*fired.lock().unwrap());
    }

    #[test]
    fn test_button_clicked_signal() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        let captured = Arc::new(Mutex::new(None::<StandardButton>));
        mb.button_clicked.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<StandardButton>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        mb.click_button(StandardButton::Yes);
        assert_eq!(*captured.lock().unwrap(), Some(StandardButton::Yes));
    }

    // ── 7. Result retrieval (accepted/rejected) ─────────────────────

    #[test]
    fn test_click_button_accept_reject_pattern() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        let accepted = Arc::new(Mutex::new(false));
        let rejected = Arc::new(Mutex::new(false));

        mb.accepted.connect({
            let accepted = Arc::clone(&accepted);
            move || {
                *accepted.lock().unwrap() = true;
            }
        });
        mb.rejected.connect({
            let rejected = Arc::clone(&rejected);
            move || {
                *rejected.lock().unwrap() = true;
            }
        });

        // Ok, Yes, Save, Apply emit accepted
        mb.click_button(StandardButton::Ok);
        assert!(*accepted.lock().unwrap());
        assert!(!*rejected.lock().unwrap());

        // Reset
        *accepted.lock().unwrap() = false;

        mb.click_button(StandardButton::Cancel);
        assert!(!*accepted.lock().unwrap());
        assert!(*rejected.lock().unwrap());
    }

    // ── 8. Factory constructors ─────────────────────────────────────

    #[test]
    fn test_factory_constructors() {
        let info = MessageBox::information(Rect::new(0, 0, 300, 150), "Information", "File saved.");
        assert_eq!(info.title(), "Information");
        assert_eq!(info.text(), "File saved.");
        assert_eq!(info.icon(), MessageBoxIcon::Information);
        assert_eq!(info.buttons(), &[StandardButton::Ok]);
        assert_eq!(info.default_button(), Some(StandardButton::Ok));

        let warn = MessageBox::warning(Rect::new(0, 0, 300, 150), "Warning", "Low disk space");
        assert_eq!(warn.title(), "Warning");
        assert_eq!(warn.text(), "Low disk space");
        assert_eq!(warn.icon(), MessageBoxIcon::Warning);

        let err = MessageBox::critical(Rect::new(0, 0, 300, 150), "Error", "Operation failed");
        assert_eq!(err.title(), "Error");
        assert_eq!(err.text(), "Operation failed");
        assert_eq!(err.icon(), MessageBoxIcon::Critical);

        let q = MessageBox::question(Rect::new(0, 0, 300, 150), "Question", "Continue?");
        assert_eq!(q.title(), "Question");
        assert_eq!(q.text(), "Continue?");
        assert_eq!(q.icon(), MessageBoxIcon::Question);
        assert_eq!(q.buttons(), &[StandardButton::Yes, StandardButton::No]);
        assert_eq!(q.default_button(), Some(StandardButton::Yes));
    }

    // ── 9. Geometry delegation ─────────────────────────────────────

    #[test]
    fn test_geometry_delegation() {
        let mut mb = MessageBox::new(Rect::new(10, 20, 300, 150));

        assert_eq!(mb.geometry(), Rect::new(10, 20, 300, 150));

        mb.set_geometry(Rect::new(0, 0, 400, 200));
        assert_eq!(mb.geometry(), Rect::new(0, 0, 400, 200));
        assert_eq!(mb.geometry(), Rect::new(0, 0, 400, 200));
        assert_eq!(mb.position(), Point::new(0, 0));
        assert_eq!(mb.size(), crate::core::Size::new(400, 200));
    }

    // ── 10. Widget ID and kind ──────────────────────────────────────

    #[test]
    fn test_widget_id_and_kind() {
        let mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert_eq!(mb.kind(), WidgetKind::MessageBox);
        assert_ne!(mb.id(), 0);

        let mb2 = MessageBox::new(Rect::new(0, 0, 200, 100));
        assert_ne!(mb.id(), mb2.id());
    }

    // ── 11. SVG output ──────────────────────────────────────────────

    #[test]
    fn test_svg_output() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        // Default output
        let svg = render_to_svg(&mut mb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"300\""));
        assert!(svg.contains("height=\"150\""));

        // With title and text set
        let mut mb2 = MessageBox::new(Rect::new(0, 0, 400, 200));
        mb2.set_title("Test Title");
        mb2.set_text("Hello");
        mb2.set_icon(MessageBoxIcon::Warning);
        let svg2 = render_to_svg(&mut mb2);
        assert!(svg2.starts_with("<svg"));
    }

    // ── 12. Modality setting ────────────────────────────────────────

    #[test]
    fn test_modality_setting() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert!(mb.is_modal());

        mb.set_modal(false);
        assert!(!mb.is_modal());

        mb.set_modal(true);
        assert!(mb.is_modal());
    }

    // ── 13. Disabled state ──────────────────────────────────────────

    #[test]
    fn test_disabled_state_blocks_events() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        mb.set_enabled(false);
        assert!(!mb.is_enabled());

        // Enter key should not trigger default button when disabled
        let accepted_fired = Arc::new(Mutex::new(false));
        mb.accepted.connect({
            let accepted_fired = Arc::clone(&accepted_fired);
            move || {
                *accepted_fired.lock().unwrap() = true;
            }
        });

        mb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(!*accepted_fired.lock().unwrap());

        // Re-enable and verify it works
        mb.set_enabled(true);
        mb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(*accepted_fired.lock().unwrap());
    }

    // ── 14. Keyboard event handling (Enter/ESC) ─────────────────────

    #[test]
    fn test_keyboard_enter_triggers_default_button() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));
        mb.set_buttons(vec![StandardButton::Yes, StandardButton::No]);
        mb.set_default_button(StandardButton::Yes);

        let captured = Arc::new(Mutex::new(None::<StandardButton>));
        mb.button_clicked.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<StandardButton>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        // Enter (key 13) triggers default button (Yes)
        mb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(*captured.lock().unwrap(), Some(StandardButton::Yes));
    }

    #[test]
    fn test_keyboard_escape_triggers_cancel() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));
        mb.set_buttons(vec![StandardButton::Ok, StandardButton::Cancel]);

        let captured = Arc::new(Mutex::new(None::<StandardButton>));
        mb.button_clicked.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<StandardButton>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        // Escape (key 27) triggers Cancel
        mb.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(*captured.lock().unwrap(), Some(StandardButton::Cancel));
    }

    #[test]
    fn test_keyboard_escape_falls_back_to_no_then_close() {
        // No Cancel button → falls back to No
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));
        mb.set_buttons(vec![StandardButton::Yes, StandardButton::No]);

        let captured = Arc::new(Mutex::new(None::<StandardButton>));
        mb.button_clicked.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<StandardButton>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        mb.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(*captured.lock().unwrap(), Some(StandardButton::No));

        // Neither Cancel nor No → Close
        let mut mb2 = MessageBox::new(Rect::new(0, 0, 300, 150));
        mb2.set_buttons(vec![StandardButton::Ok]);
        mb2.set_default_button(StandardButton::Ok);

        let captured2 = Arc::new(Mutex::new(false));
        mb2.button_clicked.connect({
            let captured2 = Arc::clone(&captured2);
            move |_: Arc<StandardButton>| {
                *captured2.lock().unwrap() = true;
            }
        });

        mb2.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        // With only Ok, no Cancel/No/Close, ESC is no-op
        // (the event handler only checks Cancel/No/Close)
        // Actually it won't match anything since buttons only contains Ok
        // Let's just verify button_clicked wasn't called
        assert!(!*captured2.lock().unwrap());
    }

    // ── 15. StandardButton label ────────────────────────────────────

    #[test]
    fn test_standard_button_labels() {
        assert_eq!(StandardButton::Ok.label(), "OK");
        assert_eq!(StandardButton::Cancel.label(), "Cancel");
        assert_eq!(StandardButton::Yes.label(), "Yes");
        assert_eq!(StandardButton::No.label(), "No");
        assert_eq!(StandardButton::Save.label(), "Save");
        assert_eq!(StandardButton::Apply.label(), "Apply");
        assert_eq!(StandardButton::Close.label(), "Close");
        assert_eq!(StandardButton::Abort.label(), "Abort");
        assert_eq!(StandardButton::Retry.label(), "Retry");
        assert_eq!(StandardButton::Ignore.label(), "Ignore");
        assert_eq!(StandardButton::Help.label(), "Help");
        assert_eq!(StandardButton::YesAll.label(), "Yes to All");
        assert_eq!(StandardButton::NoAll.label(), "No to All");
        assert_eq!(StandardButton::Discard.label(), "Discard");
    }
}
