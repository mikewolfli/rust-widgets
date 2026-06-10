//! Text shaping trait — measures and shapes text before rendering.
//!
//! Provides a trait abstraction over text shaping operations (glyph layout,
//! width/height measurement) plus a simple default implementation that
//! approximates metrics using average character widths.

/// A shaped glyph run with position information for each glyph.
#[derive(Debug, Clone)]
pub struct ShapedGlyphRun {
    /// Glyph indices (IDs) in the font.
    pub glyph_ids: Vec<u32>,
    /// `(x, y)` positions for each glyph, relative to the run origin.
    pub positions: Vec<(f32, f32)>,
    /// Font size used for this run.
    pub font_size: f32,
    /// Total visual width of the run in logical pixels.
    pub width: f32,
    /// Total visual height of the run in logical pixels.
    pub height: f32,
}

/// Text shaping trait for measuring and shaping text.
///
/// Implementors provide font-dependent glyph shaping.  The trait is `Send + Sync`
/// so it can be shared across rendering threads.
pub trait TextShaper: Send + Sync {
    /// Shape a string of text, returning glyph runs.
    ///
    /// Each run contains glyph IDs and their positions as laid out by the
    /// shaping engine.
    fn shape(&self, text: &str, font_size: f32) -> Vec<ShapedGlyphRun>;

    /// Measure the width of a string at the given font size.
    fn measure_width(&self, text: &str, font_size: f32) -> f32;

    /// Measure the height of text at the given font size.
    fn measure_height(&self, text: &str, font_size: f32) -> f32;

    /// Get the advance width of a single character.
    fn char_advance(&self, c: char, font_size: f32) -> f32;
}

/// A simple default text shaper that approximates metrics.
///
/// Each character is estimated at `0.6 * font_size` wide with a fixed
/// `1.2` line-height factor.  Glyph IDs are synthetic (sequential), which
/// is sufficient for layout measurement but not for actual glyph rasterization.
pub struct SimpleTextShaper;

impl SimpleTextShaper {
    /// Creates a new `SimpleTextShaper`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SimpleTextShaper {
    fn default() -> Self {
        Self::new()
    }
}

impl TextShaper for SimpleTextShaper {
    /// Approximate: each char is ~0.6 * font_size wide; glyphs are sequential.
    fn shape(&self, text: &str, font_size: f32) -> Vec<ShapedGlyphRun> {
        let char_count = text.chars().count();
        let avg_width = font_size * 0.6;
        let total_width = char_count as f32 * avg_width;
        let height = font_size * 1.2;

        let glyph_ids: Vec<u32> = (0..char_count as u32).collect();
        let positions: Vec<(f32, f32)> =
            (0..char_count).map(|i| (i as f32 * avg_width, 0.0)).collect();

        vec![ShapedGlyphRun { glyph_ids, positions, font_size, width: total_width, height }]
    }

    fn measure_width(&self, text: &str, font_size: f32) -> f32 {
        let mc = monospace_char_count(text);
        if mc > 0 {
            // If the string is mostly monospace-like, use exact char count
            text.chars().count() as f32 * font_size * 0.6
        } else {
            text.len() as f32 * font_size * 0.5
        }
    }

    fn measure_height(&self, _text: &str, font_size: f32) -> f32 {
        font_size * 1.2
    }

    fn char_advance(&self, c: char, font_size: f32) -> f32 {
        match c {
            // Narrow characters
            ' ' => font_size * 0.3,
            '.' | ',' | ':' | ';' | '\'' | '!' | ')' | ']' | '}' => font_size * 0.25,
            // Wide characters (CJK, full-width)
            c if (c as u32) >= 0x4E00 && (c as u32) <= 0x9FFF => font_size * 1.0,
            c if (c as u32) >= 0x3040 && (c as u32) <= 0x9FFF => font_size * 1.0,
            c if (c as u32) >= 0xFF01 && (c as u32) <= 0xFF60 => font_size * 1.0,
            // Uppercase letters are slightly wider
            'A'..='Z' => font_size * 0.7,
            // Default width
            _ => font_size * 0.6,
        }
    }
}

/// Returns the number of non-whitespace characters in `text`.
fn monospace_char_count(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shaper_creates_glyph_run() {
        let shaper = SimpleTextShaper::new();
        let runs = shaper.shape("Hello", 16.0);
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.glyph_ids.len(), 5);
        assert_eq!(run.positions.len(), 5);
        assert!((run.width - 48.0).abs() < 0.01); // 5 * 16 * 0.6
        assert!((run.height - 19.2).abs() < 0.01); // 16 * 1.2
    }

    #[test]
    fn test_measure_width_increases_with_length() {
        let shaper = SimpleTextShaper::new();
        let short = shaper.measure_width("Hi", 16.0);
        let long = shaper.measure_width("Hello World", 16.0);
        assert!(long > short);
    }

    #[test]
    fn test_char_advance_varies_by_character() {
        let shaper = SimpleTextShaper::new();
        let space = shaper.char_advance(' ', 16.0);
        let wide = shaper.char_advance('中', 16.0);
        let narrow = shaper.char_advance('.', 16.0);
        assert!(space < wide);
        assert!(narrow < wide);
        assert!(narrow < space);
    }

    #[test]
    fn test_empty_string_returns_zero_width() {
        let shaper = SimpleTextShaper::new();
        let runs = shaper.shape("", 16.0);
        assert_eq!(runs.len(), 1);
        assert!((runs[0].width - 0.0).abs() < f32::EPSILON);
        assert!((shaper.measure_width("", 16.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_height_independent_of_text() {
        let shaper = SimpleTextShaper::new();
        let h1 = shaper.measure_height("A", 12.0);
        let h2 = shaper.measure_height("A long text here", 12.0);
        assert!((h1 - h2).abs() < f32::EPSILON);
        assert!((h1 - 14.4).abs() < 0.01);
    }

    #[test]
    fn test_shaper_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SimpleTextShaper>();
    }
}
