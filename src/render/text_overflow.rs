//! Text overflow handling — ellipsis truncation, multi-line clamping.
//!
//! Provides strategies for handling text that exceeds its allotted bounds:
//!
//! - `TextOverflow::Clip` — hard clip at the boundary.
//! - `TextOverflow::Ellipsis` — replace trailing characters with `…`.
//! - `TextOverflow::Fade` — gradually decrease alpha over the last portion
//!   (when downstream renderers support it; here we return the full string
//!   with a width hint).
//!
//! Multi-line clamping is handled by `TextClamp` and `apply_text_clamp`,
//! which line-break text at word boundaries.

use crate::render::text_shaper::TextShaper;

/// Text overflow handling strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextOverflow {
    /// Clip text at the boundary — no visual indicator.
    Clip,
    /// Add ellipsis character (`…`, U+2026) at the truncated boundary.
    Ellipsis,
    /// Fade out text at the boundary (alpha gradient over the last portion).
    /// When used without alpha-aware rendering, falls back to clip behaviour.
    Fade,
}

/// Multi-line text clamping configuration.
///
/// Controls the maximum number of lines and how overflow is signaled.
#[derive(Debug, Clone, Copy)]
pub struct TextClamp {
    /// Maximum number of lines to show.  `None` means no limit.
    pub max_lines: Option<usize>,
    /// Overflow strategy for the last visible line.
    pub overflow: TextOverflow,
    /// Line height in logical pixels (should match the rendered line height).
    pub line_height: f32,
}

impl Default for TextClamp {
    fn default() -> Self {
        Self { max_lines: Some(1), overflow: TextOverflow::Ellipsis, line_height: 1.2 }
    }
}

/// Apply text overflow to a single-line string.
///
/// Returns a string that fits within `max_width` pixels, with the chosen
/// overflow strategy applied to the right edge.
pub fn apply_text_overflow(
    text: &str,
    max_width: f32,
    font_size: f32,
    overflow: TextOverflow,
    shaper: &dyn TextShaper,
) -> String {
    if max_width <= 0.0 || text.is_empty() {
        return String::new();
    }

    let text_width = shaper.measure_width(text, font_size);
    if text_width <= max_width {
        return text.to_string();
    }

    match overflow {
        TextOverflow::Clip => clip_text(text, max_width, font_size, shaper, false),
        TextOverflow::Ellipsis => {
            // Reserve space for the ellipsis character.
            let ellipsis_width = shaper.char_advance('\u{2026}', font_size);
            let available = max_width - ellipsis_width;
            if available <= 0.0 {
                return '\u{2026}'.to_string();
            }
            let clipped = clip_text(text, available, font_size, shaper, true);
            clipped + "\u{2026}"
        }
        TextOverflow::Fade => {
            // Fade returns the full string metadata; the renderer handles
            // the actual alpha gradient.  The string itself is clipped at
            // max_width for safety.
            clip_text(text, max_width, font_size, shaper, false)
        }
    }
}

/// Apply multi-line clamping.
///
/// Splits the input text into lines (at word boundaries) no wider than
/// `max_width`.  Returns at most `clamp.max_lines` lines.  The last line
/// uses `clamp.overflow` to handle any remaining overflow.
pub fn apply_text_clamp(
    text: &str,
    clamp: &TextClamp,
    max_width: f32,
    font_size: f32,
    shaper: &dyn TextShaper,
) -> Vec<String> {
    if text.is_empty() || max_width <= 0.0 {
        return Vec::new();
    }

    let max_lines = clamp.max_lines.unwrap_or(usize::MAX);
    if max_lines == 0 {
        return Vec::new();
    }

    // Split the text into words first (on whitespace), then try to fill lines.
    let words: Vec<&str> = text.split_inclusive(char::is_whitespace).collect();
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut line_limit_reached = false;

    for word in &words {
        // Try appending the word to the current line.
        let candidate = if current_line.is_empty() {
            word.to_string()
        } else {
            format!("{}{}", current_line, word)
        };

        let line_width = shaper.measure_width(&candidate, font_size);
        if line_width <= max_width || current_line.is_empty() {
            // Word fits (or we must place it because the line is empty).
            current_line = candidate;
        } else {
            // Word doesn't fit — push the current line and start a new one.
            let finished = std::mem::take(&mut current_line);
            lines.push(finished);
            if lines.len() >= max_lines {
                line_limit_reached = true;
                current_line = word.to_string();
                break;
            }
            current_line = word.to_string();
        }
    }

    if line_limit_reached {
        // Apply overflow to the last line to indicate truncation
        if let Some(last) = lines.last_mut() {
            // Ensure the line actually overflows before truncating
            let last_width = shaper.measure_width(last, font_size);
            if last_width > max_width {
                *last = apply_text_overflow(last, max_width, font_size, clamp.overflow, shaper);
            } else if !current_line.is_empty() && !last.is_empty() {
                // The overflow happened in current_line; append its first word and truncate
                let first_overflow_word = current_line.split_whitespace().next().unwrap_or("");
                let combined = format!("{}{}", last.trim_end(), first_overflow_word);
                *last =
                    apply_text_overflow(&combined, max_width, font_size, clamp.overflow, shaper);
            } else {
                // Nothing to add, but still need ellipsis to indicate overflow
                *last = apply_text_overflow(last, max_width, font_size, clamp.overflow, shaper);
            }
        }
        return lines;
    }

    // Handle the last (or only) line.
    if !current_line.is_empty() {
        if lines.len() < max_lines {
            // Check if the last line overflows.
            let last_width = shaper.measure_width(&current_line, font_size);
            if last_width > max_width {
                lines.push(apply_text_overflow(
                    &current_line,
                    max_width,
                    font_size,
                    clamp.overflow,
                    shaper,
                ));
            } else {
                lines.push(current_line);
            }
        } else {
            // We already have max_lines, apply overflow to the last pushed line.
            if let Some(last) = lines.last_mut() {
                *last = apply_text_overflow(last, max_width, font_size, clamp.overflow, shaper);
            }
        }
    }

    lines
}

/// Clip `text` at the point where its width exceeds `max_width`.
///
/// When `word_boundary` is set, the clip tries to break at a word boundary
/// (last space before the clip point) rather than mid-word.
fn clip_text(
    text: &str,
    max_width: f32,
    font_size: f32,
    shaper: &dyn TextShaper,
    word_boundary: bool,
) -> String {
    if max_width <= 0.0 {
        return String::new();
    }

    let mut width = 0.0f32;
    let mut last_space_idx: Option<usize> = None;
    let mut clip_idx = text.len();

    for (i, c) in text.char_indices() {
        let adv = shaper.char_advance(c, font_size);
        if width + adv > max_width {
            clip_idx = if word_boundary { last_space_idx.unwrap_or(i) } else { i };
            break;
        }
        width += adv;
        if c.is_whitespace() {
            last_space_idx = Some(i + c.len_utf8());
        }
        clip_idx = i + c.len_utf8();
    }

    text[..clip_idx].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::text_shaper::SimpleTextShaper;

    #[test]
    fn test_overflow_clip_truncates_long_text() {
        let shaper = SimpleTextShaper::new();
        let result = apply_text_overflow("Hello World", 30.0, 14.0, TextOverflow::Clip, &shaper);
        assert!(result.len() < "Hello World".len());
        assert!(!result.contains('…'));
    }

    #[test]
    fn test_overflow_ellipsis_appends_ellipsis() {
        let shaper = SimpleTextShaper::new();
        let result =
            apply_text_overflow("Hello World", 30.0, 14.0, TextOverflow::Ellipsis, &shaper);
        assert!(result.ends_with('…'));
        assert!(result.len() <= "Hello World".len() + "…".len());
    }

    #[test]
    fn test_overflow_short_text_unchanged() {
        let shaper = SimpleTextShaper::new();
        let result = apply_text_overflow("Hi", 200.0, 14.0, TextOverflow::Ellipsis, &shaper);
        assert_eq!(result, "Hi");
    }

    #[test]
    fn test_overflow_empty_text_returns_empty() {
        let shaper = SimpleTextShaper::new();
        let result = apply_text_overflow("", 100.0, 14.0, TextOverflow::Ellipsis, &shaper);
        assert_eq!(result, "");
    }

    #[test]
    fn test_overflow_zero_max_width_returns_empty() {
        let shaper = SimpleTextShaper::new();
        let result = apply_text_overflow("Hello", 0.0, 14.0, TextOverflow::Ellipsis, &shaper);
        assert_eq!(result, "");
    }

    #[test]
    fn test_text_clamp_limits_lines() {
        let shaper = SimpleTextShaper::new();
        let clamp =
            TextClamp { max_lines: Some(2), overflow: TextOverflow::Clip, line_height: 20.0 };
        let lines = apply_text_clamp("one two three four five", &clamp, 60.0, 14.0, &shaper);
        assert!(lines.len() <= 2);
    }

    #[test]
    fn test_text_clamp_zero_lines_returns_empty() {
        let shaper = SimpleTextShaper::new();
        let clamp =
            TextClamp { max_lines: Some(0), overflow: TextOverflow::Clip, line_height: 20.0 };
        let lines = apply_text_clamp("Hello World", &clamp, 100.0, 14.0, &shaper);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_text_clamp_no_limit() {
        let shaper = SimpleTextShaper::new();
        let clamp = TextClamp { max_lines: None, overflow: TextOverflow::Clip, line_height: 20.0 };
        let lines = apply_text_clamp("a b c d", &clamp, 20.0, 14.0, &shaper);
        // With None max_lines, each short word should become its own line.
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_text_clamp_ellipsis_on_last_line() {
        let shaper = SimpleTextShaper::new();
        let clamp =
            TextClamp { max_lines: Some(1), overflow: TextOverflow::Ellipsis, line_height: 20.0 };
        let lines = apply_text_clamp("A very long text string here", &clamp, 20.0, 14.0, &shaper);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with('…'));
    }

    #[test]
    fn test_fade_overflow_clips_text() {
        let shaper = SimpleTextShaper::new();
        let result = apply_text_overflow("Hello World", 30.0, 14.0, TextOverflow::Fade, &shaper);
        assert!(result.len() < "Hello World".len());
    }
}
