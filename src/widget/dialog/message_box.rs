//! Message box dialog widget.
use crate::core::{Color, Font, Point, Rect};
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
            MessageBoxIcon::Information => Color::from_rgb(0, 120, 215),
            MessageBoxIcon::Question => Color::from_rgb(0, 120, 215),
            MessageBoxIcon::Warning => Color::from_rgb(255, 140, 0),
            MessageBoxIcon::Critical => Color::from_rgb(196, 43, 28),
            MessageBoxIcon::NoIcon => Color::from_rgb(0, 0, 0),
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
            Color::from_rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(160, 160, 160),
        );
        // Title bar
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, 28u32),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &self.title,
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        // Icon
        let icon_sym = self.icon_symbol();
        if !icon_sym.is_empty() {
            context.draw_text(
                Point::new(rect.x + 20, rect.y + 60),
                icon_sym,
                &Font::default(),
                self.icon_color(),
            );
        }
        // Message text
        let text_x = if self.icon == MessageBoxIcon::NoIcon {
            rect.x + 12
        } else {
            rect.x + 60
        };
        context.draw_text(
            Point::new(text_x, rect.y + 60),
            &self.text,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
        // Buttons
        let btn_h = 28f32;
        let btn_w = 80f32;
        let btn_y = rect.y as f32 + rect.height as f32 - btn_h - 12.0;
        let total_btn_w = self.buttons.len() as f32 * (btn_w + 8.0);
        let mut btn_x = rect.x as f32 + rect.width as f32 - total_btn_w;
        for btn in &self.buttons {
            let is_default = self.default_button == Some(*btn);
            let bg = if is_default {
                Color::from_rgb(0, 120, 215)
            } else {
                Color::from_rgb(225, 225, 225)
            };
            let fg = if is_default {
                Color::from_rgb(255, 255, 255)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            context.fill_rect(Rect::from_f32(btn_x, btn_y, btn_w, btn_h), bg);
            context.draw_rect(
                Rect::from_f32(btn_x, btn_y, btn_w, btn_h),
                Color::from_rgb(100, 100, 100),
            );
            context.draw_text(
                Point::from_f32(btn_x + btn_w / 2.0, btn_y + btn_h / 2.0),
                &btn.translated_label(),
                &Font::default(),
                fg,
            );
            btn_x += btn_w + 8.0;
        }
    }
}
