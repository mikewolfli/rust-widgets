//! Unicode grapheme cluster support for complex text rendering.
//!
//! Provides utilities for splitting, counting, and truncating text by grapheme
//! cluster boundaries, with emoji and ZWJ sequence detection.
//!
//! This is a simplified implementation that approximates Unicode grapheme
//! boundaries using known Unicode ranges. It does not depend on ICU or
//! external C libraries.

/// A single grapheme cluster — the smallest user-perceived character unit.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphemeCluster {
    /// The UTF-8 string content of this cluster.
    pub content: String,
    /// Number of Rust `char` values in this cluster (count of code points).
    pub char_count: usize,
    /// Display width in logical pixels (approximate).
    pub width: f32,
}

/// Processes text into Unicode grapheme clusters with emoji awareness.
pub struct GraphemeProcessor;

impl GraphemeProcessor {
    /// Splits text into a vector of [`GraphemeCluster`] units.
    ///
    /// This respects Unicode grapheme cluster boundaries including:
    /// - Base characters followed by combining marks
    /// - Emoji sequences (emoji + modifiers)
    /// - ZWJ (Zero-Width Joiner) sequences
    /// - Flag sequences (regional indicator pairs)
    pub fn split_graphemes(text: &str) -> Vec<GraphemeCluster> {
        let chars: Vec<char> = text.chars().collect();
        let mut clusters: Vec<GraphemeCluster> = Vec::new();
        let mut i = 0;
        let len = chars.len();

        while i < len {
            let mut cluster_chars = Vec::new();
            cluster_chars.push(chars[i]);

            // Collect combining marks and modifier sequences
            let mut j = i + 1;
            while j < len {
                let c = chars[j];

                // Combining marks (zero-width diacritics)
                if Self::is_combining_mark(c) {
                    cluster_chars.push(c);
                    j += 1;
                    continue;
                }

                // Emoji modifier (skin tones, hair styles)
                if Self::is_emoji_modifier(c) && Self::is_emoji(chars[j - 1]) {
                    cluster_chars.push(c);
                    j += 1;
                    continue;
                }

                // Zero-Width Joiner sequence
                if c == '\u{200D}' {
                    cluster_chars.push(c);
                    j += 1;
                    // After ZWJ, the next char is part of the sequence
                    if j < len {
                        cluster_chars.push(chars[j]);
                        j += 1;
                    }
                    continue;
                }

                // Variation selectors
                if Self::is_variation_selector(c) {
                    cluster_chars.push(c);
                    j += 1;
                    continue;
                }

                // Regional indicator flag sequences (pair only)
                if Self::is_regional_indicator(chars[i])
                    && Self::is_regional_indicator(c)
                    && cluster_chars.len() == 1
                {
                    cluster_chars.push(c);
                    j += 1;
                    break;
                }

                // Keycap sequences (digit/# + U+20E3 combining enclosing keycap)
                if c == '\u{20E3}' && cluster_chars.len() == 1 {
                    cluster_chars.push(c);
                    j += 1;
                    break;
                }

                break;
            }

            let content: String = cluster_chars.iter().collect();
            let char_count = cluster_chars.len();
            let width = Self::cluster_display_width(&cluster_chars);

            clusters.push(GraphemeCluster { content, char_count, width });

            i = j;
        }

        clusters
    }

    /// Returns the number of grapheme clusters in text.
    pub fn grapheme_count(text: &str) -> usize {
        Self::split_graphemes(text).len()
    }

    /// Truncates text to at most `max` grapheme clusters.
    ///
    /// The result is guaranteed to break at a grapheme cluster boundary.
    /// If `text` has fewer clusters than `max`, the original text is returned.
    pub fn truncate_to_graphemes(text: &str, max: usize) -> String {
        let clusters = Self::split_graphemes(text);
        if clusters.len() <= max {
            return text.to_string();
        }
        clusters.into_iter().take(max).map(|c| c.content).collect()
    }

    /// Returns `true` if `c` is likely an emoji character.
    ///
    /// Covers the common emoji ranges; not exhaustive but sufficient for
    /// typical UI use.
    pub fn is_emoji(c: char) -> bool {
        let code = c as u32;
        matches!(code,
            // Miscellaneous Symbols and Pictographs
            0x1F300..=0x1F5FF
            // Emoticons
            | 0x1F600..=0x1F64F
            // Transport and Map Symbols
            | 0x1F680..=0x1F6FF
            // Miscellaneous Symbols
            | 0x2600..=0x26FF
            // Dingbats
            | 0x2700..=0x27BF
            // Supplemental Symbols and Pictographs
            | 0x1F900..=0x1F9FF
            // Chess symbols
            | 0x1FA00..=0x1FA6F
            // Symbols and Pictographs Extended-A
            | 0x1FA70..=0x1FAFF
            // Enclosed Ideographic Supplement
            | 0x1F200..=0x1F2FF
            // Mahjong / Domino / Playing Cards
            | 0x1F000..=0x1F02F
            | 0x1F030..=0x1F09F
            | 0x1F0A0..=0x1F0FF
            // Misc additional (non-overlapping ranges only)
            | 0x231A..=0x23FF
            | 0x24C2..=0x24C2
            | 0x25AA..=0x25AB
            | 0x25B6..=0x25B6
            | 0x25C0..=0x25C0
            | 0x25FB..=0x25FE
            | 0x2934..=0x2935
            | 0x2B05..=0x2B07
            | 0x2B1B..=0x2B1C
            | 0x2B50..=0x2B50
            | 0x2B55..=0x2B55
            | 0x3030..=0x3030
            | 0x303D..=0x303D
            | 0x3297..=0x3297
            | 0x3299..=0x3299
            // Regional indicators (flag letters)
            | 0x1F1E6..=0x1F1FF
        )
    }

    /// Returns `true` if `c` is an emoji modifier (skin tone, hair style).
    pub fn is_emoji_modifier(c: char) -> bool {
        let code = c as u32;
        // Emoji modifiers: skin tones (U+1F3FB..U+1F3FF)
        matches!(code, 0x1F3FB..=0x1F3FF)
    }

    /// Returns `true` if the character slice forms a ZWJ sequence.
    ///
    /// A ZWJ sequence is two or more emoji joined by Zero-Width Joiners
    /// (U+200D), e.g. family emoji, profession emoji.
    pub fn is_zwj_sequence(chars: &[char]) -> bool {
        if chars.len() < 3 {
            return false;
        }
        // Must contain at least one ZWJ
        let has_zwj = chars.contains(&'\u{200D}');
        if !has_zwj {
            return false;
        }
        // Every segment between ZWJs should be an emoji
        let segments: Vec<&[char]> = chars.split(|&c| c == '\u{200D}').collect();
        if segments.len() < 2 {
            return false;
        }
        segments.iter().all(|seg| !seg.is_empty() && seg.iter().all(|&c| Self::is_emoji(c)))
    }

    /// Returns the approximate display width of an emoji text in columns.
    ///
    /// Most emoji are 2 columns wide; ASCII and combining marks are 1.
    /// ZWJ sequences and flag pairs are also 2 columns wide.
    pub fn emoji_width(text: &str) -> u32 {
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return 0;
        }
        if Self::is_zwj_sequence(&chars) {
            return 2;
        }
        if chars.len() == 2
            && Self::is_regional_indicator(chars[0])
            && Self::is_regional_indicator(chars[1])
        {
            return 2;
        }
        if Self::is_emoji(chars[0]) {
            return 2;
        }
        1
    }

    // ─── Private helpers ─────────────────────────────────────────────────

    /// Returns `true` if `c` is a combining mark (zero-width diacritic).
    fn is_combining_mark(c: char) -> bool {
        is_combining_mark(c)
    }

    /// Returns `true` if `c` is a variation selector (U+FE00..U+FE0F).
    fn is_variation_selector(c: char) -> bool {
        is_variation_selector(c)
    }

    /// Returns `true` if `c` is a regional indicator symbol (flag letters).
    fn is_regional_indicator(c: char) -> bool {
        let code = c as u32;
        matches!(code, 0x1F1E6..=0x1F1FF)
    }

    /// Calculates approximate display width for a list of chars forming a cluster.
    fn cluster_display_width(chars: &[char]) -> f32 {
        if chars.is_empty() {
            return 0.0;
        }
        let first = chars[0];
        if Self::is_emoji(first) {
            return 2.0; // Emoji are typically twice as wide
        }
        if Self::is_regional_indicator(first) && chars.len() >= 2 {
            return 2.0;
        }
        1.0
    }
}

// ─── Standalone helpers (shared with pixel_ops) ─────────────────────

/// Returns `true` if `c` is a combining mark (zero-width diacritic).
pub(crate) fn is_combining_mark(c: char) -> bool {
    let code = c as u32;
    matches!(code,
        // Combining Diacritical Marks
        0x0300..=0x036F
        // Combining Diacritical Marks Extended
        | 0x1AB0..=0x1AFF
        // Combining Diacritical Marks Supplement
        | 0x1DC0..=0x1DFF
        // Combining Half Marks
        | 0xFE20..=0xFE2F
        // Devanagari combining marks (subset)
        | 0x0901..=0x0903
        | 0x093E..=0x094D
        // Thai combining marks
        | 0x0E31..=0x0E3A
        | 0x0E47..=0x0E4E
        // General combining range for Indic scripts
        | 0x0981..=0x0983
        | 0x09BE..=0x09CD
        | 0x0A01..=0x0A03
        | 0x0A3E..=0x0A4D
        | 0x0B01..=0x0B03
        | 0x0B3E..=0x0B4D
        // Tibetan combining marks
        | 0x0F82..=0x0F84
        | 0x0F86..=0x0F8B
    )
}

/// Returns `true` if `c` is a variation selector (U+FE00..U+FE0F).
pub(crate) fn is_variation_selector(c: char) -> bool {
    let code = c as u32;
    matches!(code, 0xFE00..=0xFE0F)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_ascii_graphemes() {
        let clusters = GraphemeProcessor::split_graphemes("Hello");
        assert_eq!(clusters.len(), 5);
        for (i, cluster) in clusters.iter().enumerate() {
            let expected = ["H", "e", "l", "l", "o"];
            assert_eq!(cluster.content, expected[i]);
            assert_eq!(cluster.char_count, 1);
        }
    }

    #[test]
    fn test_empty_string() {
        let clusters = GraphemeProcessor::split_graphemes("");
        assert!(clusters.is_empty());
        assert_eq!(GraphemeProcessor::grapheme_count(""), 0);
    }

    #[test]
    fn test_grapheme_count() {
        assert_eq!(GraphemeProcessor::grapheme_count("Hello"), 5);
        assert_eq!(GraphemeProcessor::grapheme_count("世界"), 2);
        assert_eq!(GraphemeProcessor::grapheme_count(""), 0);
    }

    #[test]
    fn test_truncate_to_graphemes() {
        assert_eq!(GraphemeProcessor::truncate_to_graphemes("Hello World", 5), "Hello");
        assert_eq!(GraphemeProcessor::truncate_to_graphemes("Hi", 5), "Hi");
        assert_eq!(GraphemeProcessor::truncate_to_graphemes("", 3), "");
    }

    #[test]
    fn test_simple_emoji() {
        // Simple emoji like 😀 (U+1F600)
        let clusters = GraphemeProcessor::split_graphemes("a😀b");
        assert_eq!(clusters.len(), 3);
        assert_eq!(clusters[0].content, "a");
        assert_eq!(clusters[1].content, "😀");
        assert_eq!(clusters[1].char_count, 1);
        assert_eq!(clusters[2].content, "b");

        assert!(GraphemeProcessor::is_emoji('😀'));
    }

    #[test]
    fn test_emoji_with_modifier_skin_tone() {
        // Thumbs up + skin tone modifier (U+1F44D U+1F3FD)
        let text = "\u{1F44D}\u{1F3FD}";
        let clusters = GraphemeProcessor::split_graphemes(text);
        assert_eq!(clusters.len(), 1, "emoji + skin tone should be one cluster");
        assert_eq!(clusters[0].char_count, 2);

        assert!(GraphemeProcessor::is_emoji('\u{1F44D}'));
        assert!(GraphemeProcessor::is_emoji_modifier('\u{1F3FD}'));
    }

    #[test]
    fn test_zwj_sequences() {
        // Family emoji: man + ZWJ + woman + ZWJ + girl + ZWJ + boy
        // U+1F468 U+200D U+1F469 U+200D U+1F467 U+200D U+1F466
        let chars: Vec<char> = [
            '\u{1F468}',
            '\u{200D}',
            '\u{1F469}',
            '\u{200D}',
            '\u{1F467}',
            '\u{200D}',
            '\u{1F466}',
        ]
        .to_vec();
        assert!(
            GraphemeProcessor::is_zwj_sequence(&chars),
            "family emoji should be a ZWJ sequence"
        );

        // Non-ZWJ sequence should fail
        let non_zwj: Vec<char> = "abc".chars().collect();
        assert!(!GraphemeProcessor::is_zwj_sequence(&non_zwj));
    }

    #[test]
    fn test_flag_sequences() {
        // Flag of France: regional indicators U+1F1EB U+1F1F7
        let text = "\u{1F1EB}\u{1F1F7}";
        let clusters = GraphemeProcessor::split_graphemes(text);
        assert_eq!(clusters.len(), 1, "flag pair should be one cluster");
        assert_eq!(clusters[0].char_count, 2);

        assert_eq!(GraphemeProcessor::emoji_width(text), 2);
    }

    #[test]
    fn test_emoji_with_zwj_sequence_in_text() {
        // Text with a ZWJ sequence embedded
        let text = "Hello\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}World";
        let clusters = GraphemeProcessor::split_graphemes(text);
        // H, e, l, l, o, <family>, W, o, r, l, d = 11 clusters
        assert_eq!(clusters.len(), 11);
        // The family emoji should be one cluster
        assert!(clusters[5].content.contains('\u{200D}'));
    }

    #[test]
    fn test_is_emoji() {
        assert!(GraphemeProcessor::is_emoji('😀')); // Grinning face
        assert!(GraphemeProcessor::is_emoji('❤')); // Heart
        assert!(GraphemeProcessor::is_emoji('🚀')); // Rocket
        assert!(!GraphemeProcessor::is_emoji('a'));
        assert!(!GraphemeProcessor::is_emoji(' '));
    }

    #[test]
    fn test_is_emoji_modifier() {
        assert!(GraphemeProcessor::is_emoji_modifier('\u{1F3FB}')); // Light skin tone
        assert!(GraphemeProcessor::is_emoji_modifier('\u{1F3FF}')); // Dark skin tone
        assert!(!GraphemeProcessor::is_emoji_modifier('a'));
    }

    #[test]
    fn test_emoji_width() {
        assert_eq!(GraphemeProcessor::emoji_width("😀"), 2);
        assert_eq!(GraphemeProcessor::emoji_width("a"), 1);
        assert_eq!(GraphemeProcessor::emoji_width(""), 0);
    }

    #[test]
    fn test_combining_mark_graphemes() {
        // 'e' + combining acute accent (U+0065 U+0301) = é
        let text = "e\u{0301}";
        let clusters = GraphemeProcessor::split_graphemes(text);
        assert_eq!(clusters.len(), 1, "e + combining accent should be one cluster");
        assert_eq!(clusters[0].char_count, 2);
        assert_eq!(clusters[0].content.chars().count(), 2);
    }

    #[test]
    fn test_emoji_keycap_sequence() {
        // Digit '1' + combining enclosing keycap (U+0031 U+20E3)
        // Note: '1' is not an emoji, but the sequence forms a keycap emoji
        let text = "1\u{20E3}";
        let clusters = GraphemeProcessor::split_graphemes(text);
        assert_eq!(clusters.len(), 1, "digit + keycap should be one cluster");
    }

    #[test]
    fn test_grapheme_truncate_respects_boundary() {
        // Truncate a string with emoji - should not break inside cluster
        let text = "ab😀cd";
        let truncated = GraphemeProcessor::truncate_to_graphemes(text, 3);
        assert_eq!(truncated, "ab😀");
    }

    #[test]
    fn test_variation_selector() {
        // '©' + variation selector (U+00A9 U+FE0F) forms emoji presentation
        let text = "\u{00A9}\u{FE0F}";
        let clusters = GraphemeProcessor::split_graphemes(text);
        assert_eq!(clusters.len(), 1, "copyright + VS16 should be one cluster");
    }
}
