// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! FontPreview widget — a font preview panel for font selection dialogs.
//!
//! Displays a preview of a font family at configurable sizes, with sample
//! text, alphabet samples, and pangrams. Supports bold and italic styles.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_f64, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Default pangram used for font preview.
const DEFAULT_PANGRAM: &str = "The quick brown fox jumps over the lazy dog";

/// Alphabet sample string.
const ALPHABET_SAMPLE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz";

/// Digit sample string.
const DIGIT_SAMPLE: &str = "0123456789";

/// A font preview widget that displays a font family at configurable sizes
/// with sample text, alphabet samples, and pangrams.
pub struct FontPreview {
    base: BaseWidget,
    font_family: String,
    font_size: f32,
    preview_text: String,
    sample_texts: Vec<String>,
    bold: bool,
    italic: bool,
}

impl FontPreview {
    /// Creates a new FontPreview widget with the given geometry.
    pub fn new(font_family: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FontPreview, geometry, "FontPreview"),
            font_family: font_family.to_string(),
            font_size: 24.0,
            preview_text: DEFAULT_PANGRAM.to_string(),
            sample_texts: vec![
                DEFAULT_PANGRAM.to_string(),
                ALPHABET_SAMPLE.to_string(),
                DIGIT_SAMPLE.to_string(),
            ],
            bold: false,
            italic: false,
        }
    }

    /// Sets the font family name.
    pub fn set_font_family(&mut self, name: &str) {
        self.font_family = name.to_string();
        self.base.request_redraw();
    }

    /// Returns the font family name.
    pub fn font_family(&self) -> &str {
        &self.font_family
    }

    /// Sets the font size for the main preview text.
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size.max(4.0);
        self.base.request_redraw();
    }

    /// Returns the font size.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Sets the preview text displayed at the top.
    pub fn set_preview_text(&mut self, text: &str) {
        self.preview_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current preview text.
    pub fn preview_text(&self) -> &str {
        &self.preview_text
    }

    /// Sets whether the font is bold.
    pub fn set_bold(&mut self, bold: bool) {
        if self.bold != bold {
            self.bold = bold;
            self.base.request_redraw();
        }
    }

    /// Returns whether the font is bold.
    pub fn is_bold(&self) -> bool {
        self.bold
    }

    /// Sets whether the font is italic.
    pub fn set_italic(&mut self, italic: bool) {
        if self.italic != italic {
            self.italic = italic;
            self.base.request_redraw();
        }
    }

    /// Returns whether the font is italic.
    pub fn is_italic(&self) -> bool {
        self.italic
    }

    /// Returns the sample texts.
    pub fn sample_texts(&self) -> &[String] {
        &self.sample_texts
    }

    /// Sets the sample texts for the preview.
    pub fn set_sample_texts(&mut self, texts: Vec<String>) {
        self.sample_texts = texts;
        self.base.request_redraw();
    }

    /// Builds a font from current settings.
    fn build_font(&self, size: f32) -> Font {
        Font::new(&self.font_family, size, self.bold, self.italic)
    }
}

impl Widget for FontPreview {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 100)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `FontPreview`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for FontPreview {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "font_family" => Ok(CapabilityValue::String(self.font_family().to_string())),
            "font_size" => Ok(CapabilityValue::Float(f64::from(self.font_size()))),
            "preview_text" => Ok(CapabilityValue::String(self.preview_text().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "font_family" => {
                self.set_font_family(&expect_string(value)?);
                Ok(())
            }
            "font_size" => {
                self.set_font_size(expect_f64(value)? as f32);
                Ok(())
            }
            "preview_text" => {
                self.set_preview_text(&expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["font_family", "font_size", "preview_text", BASE_PROPERTY_NAMES]
    }
}

impl Draw for FontPreview {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Draw background
        context.fill_rect(rect, Color::WHITE);

        let mut y: u32 = (rect.y + 10) as u32;
        let margin = 10u32;
        // Every string in this panel is a *sample* of the configured font, not a label:
        // a pangram is 43 characters whatever the widget is wide, and at the default 24 pt
        // it advances ten times the control's width. The budget is therefore the panel's own
        // inner width, shared by all three runs so the info line, the preview and the sample
        // rows are truncated by the same rule instead of each drifting past the right edge.
        let text_w = rect.width.saturating_sub(margin * 2);

        // --- Font family name and style info ---
        let info_font = Font::new("sans-serif", 12.0, false, false);
        let info_text = format!(
            "{} ({}pt{}{})",
            self.font_family,
            self.font_size as u32,
            if self.bold { ", Bold" } else { "" },
            if self.italic { ", Italic" } else { "" },
        );
        // Fitted to the panel's inner width: the info line carries the family, the size and
        // any style suffix, so it grows with the caller's own inputs and can outrun the panel
        // just as the samples can. It painted in the same grey as a sample row, so the two
        // overflows were indistinguishable in the snapshot.
        context.draw_text_fitted(
            Rect::new(rect.x + margin as i32, y as i32, text_w, info_font.size() as u32),
            &info_text,
            &info_font,
            Color::rgba(100, 100, 100, 255),
            HorizontalAlignment::Left,
        );

        let line_height = 18;
        y += line_height as u32 + 8;

        // --- Preview text at configured size ---
        if !self.preview_text.is_empty() {
            // Truncate the sample rather than shrink the type: the preview's whole purpose is
            // to show the family at `font_size`, and a font that silently rendered smaller
            // than the size the panel's own info line reports would be a worse lie than an
            // ellipsis. Fitting is therefore the correct fix here, not scaling.
            let preview_font = self.build_font(self.font_size);
            context.draw_text_fitted(
                Rect::new(rect.x + margin as i32, y as i32, text_w, preview_font.size() as u32),
                &self.preview_text,
                &preview_font,
                Color::BLACK,
                HorizontalAlignment::Left,
            );
            y += (self.font_size * 1.4) as u32;
        }

        // --- Separator line ---
        y += 4;
        let baseline_y = rect.y as u32 + y;
        context.draw_line(
            Point::new(rect.x + margin as i32, baseline_y as i32),
            Point::new(rect.x + rect.width as i32 - margin as i32, baseline_y as i32),
            Color::rgba(200, 200, 200, 255),
        );
        y += 10;

        // --- Sample texts ---
        let sample_font = self.build_font(14.0);
        for text in &self.sample_texts {
            if rect.y as u32 + y + 20 > rect.y as u32 + rect.height {
                break; // Don't draw past the widget boundary
            }
            // The alphabet and the pangram are longer than any panel this widget is sized
            // for, so each row is fitted to the same inner width as the preview above it.
            context.draw_text_fitted(
                Rect::new(rect.x + margin as i32, y as i32, text_w, sample_font.size() as u32),
                text,
                &sample_font,
                Color::rgba(60, 60, 60, 255),
                HorizontalAlignment::Left,
            );
            y += 22;
        }
    }
}

impl EventHandler for FontPreview {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_preview_initial_state() {
        let fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        assert_eq!(fp.font_family(), "Arial");
        assert!((fp.font_size() - 24.0).abs() < 0.01);
        assert_eq!(fp.preview_text(), DEFAULT_PANGRAM);
        assert!(!fp.is_bold());
        assert!(!fp.is_italic());
        assert_eq!(fp.kind(), WidgetKind::FontPreview);
    }

    #[test]
    fn font_preview_set_font_family() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        fp.set_font_family("Helvetica");
        assert_eq!(fp.font_family(), "Helvetica");
    }

    #[test]
    fn font_preview_set_font_size() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        fp.set_font_size(18.0);
        assert!((fp.font_size() - 18.0).abs() < 0.01);

        // Size should clamp to minimum
        fp.set_font_size(0.0);
        assert!((fp.font_size() - 4.0).abs() < 0.01);
    }

    #[test]
    fn font_preview_set_preview_text() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        fp.set_preview_text("Hello World");
        assert_eq!(fp.preview_text(), "Hello World");

        fp.set_preview_text("");
        assert_eq!(fp.preview_text(), "");
    }

    #[test]
    fn font_preview_bold_italic() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));

        fp.set_bold(true);
        assert!(fp.is_bold());

        fp.set_italic(true);
        assert!(fp.is_italic());

        fp.set_bold(false);
        assert!(!fp.is_bold());
    }

    #[test]
    fn font_preview_sample_texts() {
        let mut fp = FontPreview::new("Arial", Rect::new(0, 0, 300, 200));
        let samples = fp.sample_texts().to_vec();
        assert_eq!(samples.len(), 3);

        let new_samples = vec!["Sample A".to_string(), "Sample B".to_string()];
        fp.set_sample_texts(new_samples.clone());
        assert_eq!(fp.sample_texts(), &new_samples);
    }

    #[test]
    fn font_preview_build_font() {
        let fp = FontPreview::new("Times New Roman", Rect::new(0, 0, 300, 200));
        let font = fp.build_font(12.0);
        assert_eq!(font.family(), "Times New Roman");
        assert!((font.size() - 12.0).abs() < 0.01);
    }
}
