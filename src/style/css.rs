//! CSS stylesheet parser — parses CSS text into rules with property declarations
//! that can be matched against widgets and applied to `WidgetStyle`.
//!
//! # Example
//! ```rust
//! use rust_widgets::style::css::CssParser;
//!
//! let css = r#"
//!     Button { color: #333; font-size: 14px; }
//!     .primary { background: #0066cc; color: #fff; }
//!     Button:hover { background: #0052a3; }
//! "#;
//! let sheet = CssParser::parse(css).unwrap();
//! assert!(!sheet.rules().is_empty());
//! ```

use crate::core::Color;
use crate::style::{PseudoState, Selector, StyleRule, StyleSheet, WidgetStyle};

/// A CSS-parsed selector that stores raw selector parts as strings.
/// Used for matching during CSS rule application.
#[derive(Debug, Clone, PartialEq)]
pub enum CssSelector {
    /// Match any widget.
    Universal,
    /// Match by widget kind name (e.g., "Button", "Label").
    Kind(String),
    /// Match by CSS class name (e.g., ".my-class").
    Class(String),
    /// Match by widget ID (e.g., "#my-id").
    Id(String),
    /// Match a widget that has a specific state (e.g., ":hover").
    State(PseudoState),
    /// AND-combination of multiple selectors.
    And(Vec<CssSelector>),
}

impl CssSelector {
    /// Check if this selector matches a widget with the given properties.
    pub fn matches(
        &self,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
    ) -> bool {
        match self {
            CssSelector::Universal => true,
            CssSelector::Kind(k) => k.eq_ignore_ascii_case(kind),
            CssSelector::Class(c) => class == Some(c.as_str()),
            CssSelector::Id(i) => id == Some(i.as_str()),
            CssSelector::State(s) => state == Some(*s),
            CssSelector::And(selectors) => {
                selectors.iter().all(|s| s.matches(kind, class, id, state))
            }
        }
    }
}

/// A parsed CSS declaration: property name → raw value string.
#[derive(Debug, Clone, PartialEq)]
pub struct CssDeclaration {
    pub property: String,
    pub value: String,
}

/// A parsed CSS rule: selector text → declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct CssRule {
    pub selector_text: String,
    pub declarations: Vec<CssDeclaration>,
}

/// CSS parser that converts CSS text into `StyleSheet` + property application.
pub struct CssParser;

impl CssParser {
    /// Parse CSS text into a `StyleSheet` with `StyleRule` entries.
    /// Each rule stores its selector and property declarations.
    pub fn parse(css: &str) -> Result<StyleSheet, String> {
        let mut sheet = StyleSheet::new();
        let rules = Self::parse_rules(css)?;
        for rule in &rules {
            store_declarations(&rule.selector_text, rule.declarations.clone());
            // Add a StyleRule entry using Universal selector so the parsed
            // rules are reflected in sheet.rules().
            let rule_entry = StyleRule::new(Selector::Universal, &rule.selector_text);
            sheet.add_rule(rule_entry);
        }
        Ok(sheet)
    }

    /// Parse CSS text into raw `CssRule` entries.
    fn parse_rules(css: &str) -> Result<Vec<CssRule>, String> {
        let mut rules = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = css.chars().collect();

        while pos < chars.len() {
            // Skip whitespace and comments
            pos = Self::skip_whitespace_and_comments(&chars, pos);
            if pos >= chars.len() {
                break;
            }

            // Read selector text until '{'
            let start = pos;
            while pos < chars.len() && chars[pos] != '{' {
                pos += 1;
            }
            if pos >= chars.len() {
                return Err("Unterminated rule: missing '{'".to_string());
            }
            let selector_text: String = chars[start..pos].iter().collect();
            let selector_text = selector_text.trim().to_string();
            if selector_text.is_empty() {
                return Err("Empty selector".to_string());
            }

            pos += 1; // skip '{'

            // Read declarations until '}'
            let decl_start = pos;
            while pos < chars.len() && chars[pos] != '}' {
                pos += 1;
            }
            if pos >= chars.len() {
                return Err("Unterminated rule: missing '}'".to_string());
            }
            let decl_text: String = chars[decl_start..pos].iter().collect();
            pos += 1; // skip '}'

            let declarations = Self::parse_declarations(&decl_text);
            rules.push(CssRule { selector_text, declarations });
        }

        Ok(rules)
    }

    /// Parse a selector string into a `CssSelector`.
    /// Supports: `Button`, `.class`, `#id`, `:hover`, `Button.class`, `Button:hover`, etc.
    pub fn parse_selector(text: &str) -> Option<CssSelector> {
        let text = text.trim();
        if text.is_empty() || text == "*" {
            return Some(CssSelector::Universal);
        }

        let parts = Self::split_selector_parts(text);
        let mut selectors = Vec::new();

        for part in parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some(class_name) = part.strip_prefix('.') {
                selectors.push(CssSelector::Class(class_name.to_string()));
            } else if let Some(id_name) = part.strip_prefix('#') {
                selectors.push(CssSelector::Id(id_name.to_string()));
            } else if let Some(state_name) = part.strip_prefix(':') {
                if let Some(state) = Self::parse_pseudo_state(state_name) {
                    selectors.push(CssSelector::State(state));
                }
            } else if part == "*" {
                selectors.push(CssSelector::Universal);
            } else {
                // Try parsing as widget kind
                selectors.push(CssSelector::Kind(part.to_string()));
            }
        }

        match selectors.len() {
            0 => None,
            1 => Some(selectors.into_iter().next().unwrap()),
            _ => Some(CssSelector::And(selectors)),
        }
    }

    /// Split a compound selector into parts (e.g., "Button.primary:hover" → ["Button", ".primary", ":hover"])
    fn split_selector_parts(text: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        for ch in text.chars() {
            if ch == '.' || ch == '#' || ch == ':' {
                if !current.is_empty() {
                    parts.push(current);
                    current = String::new();
                }
                current.push(ch);
            } else if ch.is_whitespace() {
                if !current.is_empty() {
                    parts.push(current);
                    current = String::new();
                }
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            parts.push(current);
        }
        parts
    }

    fn parse_pseudo_state(name: &str) -> Option<PseudoState> {
        match name {
            "hover" => Some(PseudoState::Hover),
            "pressed" | "active" => Some(PseudoState::Pressed),
            "disabled" => Some(PseudoState::Disabled),
            "focused" | "focus" => Some(PseudoState::Focused),
            "checked" => Some(PseudoState::Checked),
            "selected" => Some(PseudoState::Selected),
            _ => None,
        }
    }

    /// Parse declaration block text into a list of declarations.
    fn parse_declarations(text: &str) -> Vec<CssDeclaration> {
        let mut decls = Vec::new();
        for line in text.split(';') {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(idx) = line.find(':') {
                let property = line[..idx].trim().to_string();
                let value = line[idx + 1..].trim().to_string();
                if !property.is_empty() && !value.is_empty() {
                    decls.push(CssDeclaration { property, value });
                }
            }
        }
        decls
    }

    /// Apply parsed declarations from matched rules to a `WidgetStyle`.
    pub fn apply_declarations(
        declarations: &[CssDeclaration],
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        for decl in declarations {
            Self::apply_one(decl, style)?;
        }
        Ok(())
    }

    /// Apply a single CSS declaration to a WidgetStyle.
    fn apply_one(decl: &CssDeclaration, style: &mut WidgetStyle) -> Result<(), String> {
        match decl.property.as_str() {
            "color" | "text-color" | "foreground" => {
                style.text_color = Some(Self::parse_color(&decl.value)?);
            }
            "background" | "background-color" => {
                style.background_color = Some(Self::parse_color(&decl.value)?);
            }
            "border-color" => {
                style.border_color = Some(Self::parse_color(&decl.value)?);
            }
            "border-width" => {
                style.border_width = Self::parse_length(&decl.value)?;
            }
            "border-radius" => {
                style.border_radius = Self::parse_length(&decl.value)?;
            }
            "padding" => {
                let vals = Self::parse_space_separated_lengths(&decl.value, 4)?;
                style.padding = crate::style::Padding::new(vals[0], vals[1], vals[2], vals[3]);
            }
            "padding-top" => style.padding.top = Self::parse_length(&decl.value)?,
            "padding-right" => style.padding.right = Self::parse_length(&decl.value)?,
            "padding-bottom" => style.padding.bottom = Self::parse_length(&decl.value)?,
            "padding-left" => style.padding.left = Self::parse_length(&decl.value)?,
            "margin" => {
                let vals = Self::parse_space_separated_lengths(&decl.value, 4)?;
                style.margin = crate::style::Margin::new(vals[0], vals[1], vals[2], vals[3]);
            }
            "margin-top" => style.margin.top = Self::parse_length(&decl.value)?,
            "margin-right" => style.margin.right = Self::parse_length(&decl.value)?,
            "margin-bottom" => style.margin.bottom = Self::parse_length(&decl.value)?,
            "margin-left" => style.margin.left = Self::parse_length(&decl.value)?,
            "opacity" => {
                let val: f32 = decl
                    .value
                    .trim()
                    .parse()
                    .map_err(|_| format!("Invalid opacity: {}", decl.value))?;
                let val = val.clamp(0.0, 1.0);
                // Opacity handling is rendering-pipeline specific; store for reference
                style.opacity = Some(val);
            }
            "font-size" => {
                if let Some(font) = &mut style.font {
                    font.size = Self::parse_float(&decl.value)?;
                } else {
                    style.font = Some(crate::core::Font {
                        size: Self::parse_float(&decl.value)?,
                        ..Default::default()
                    });
                }
            }
            "font-family" => {
                let family = decl.value.trim().trim_matches('"').trim_matches('\'').to_string();
                if let Some(font) = &mut style.font {
                    font.family = family;
                } else {
                    style.font =
                        Some(crate::core::Font { family: family.clone(), ..Default::default() });
                }
            }
            _ => {
                // Unknown property — ignore (CSS spec: ignore unknown properties)
            }
        }
        Ok(())
    }

    /// Parse a CSS color value (#fff, #ffffff, rgb(r,g,b), rgba(r,g,b,a), named colors)
    pub fn parse_color(value: &str) -> Result<Color, String> {
        let v = value.trim();
        if let Some(hex) = v.strip_prefix('#') {
            Self::parse_hex_color(hex)
        } else if v.starts_with("rgb") {
            Self::parse_rgb_color(v)
        } else {
            // Named colors — common subset
            match v.to_lowercase().as_str() {
                "red" => Ok(Color::rgba(255, 0, 0, 255)),
                "green" | "lime" => Ok(Color::rgba(0, 255, 0, 255)),
                "blue" => Ok(Color::rgba(0, 0, 255, 255)),
                "white" => Ok(Color::rgba(255, 255, 255, 255)),
                "black" => Ok(Color::rgba(0, 0, 0, 255)),
                "gray" | "grey" => Ok(Color::rgba(128, 128, 128, 255)),
                "yellow" => Ok(Color::rgba(255, 255, 0, 255)),
                "cyan" | "aqua" => Ok(Color::rgba(0, 255, 255, 255)),
                "magenta" | "fuchsia" => Ok(Color::rgba(255, 0, 255, 255)),
                "orange" => Ok(Color::rgba(255, 165, 0, 255)),
                "purple" => Ok(Color::rgba(128, 0, 128, 255)),
                "navy" => Ok(Color::rgba(0, 0, 128, 255)),
                "teal" => Ok(Color::rgba(0, 128, 128, 255)),
                "maroon" => Ok(Color::rgba(128, 0, 0, 255)),
                "olive" => Ok(Color::rgba(128, 128, 0, 255)),
                "transparent" => Ok(Color::rgba(0, 0, 0, 0)),
                _ => Err(format!("Unknown color: {}", v)),
            }
        }
    }

    fn parse_hex_color(hex: &str) -> Result<Color, String> {
        let hex = hex.trim();
        let (r, g, b, a) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?
                    * 17;
                let g = u8::from_str_radix(&hex[1..2], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?
                    * 17;
                let b = u8::from_str_radix(&hex[2..3], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?
                    * 17;
                (r, g, b, 255)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                (r, g, b, 255)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let a = u8::from_str_radix(&hex[6..8], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                (r, g, b, a)
            }
            _ => return Err(format!("Invalid hex color length: #{}", hex)),
        };
        Ok(Color::rgba(r, g, b, a))
    }

    fn parse_rgb_color(v: &str) -> Result<Color, String> {
        let inner = v.trim_start_matches("rgba").trim_start_matches("rgb");
        let inner = inner.trim_start_matches('(').trim_end_matches(')').trim();
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        match parts.len() {
            3 => {
                let r: u8 = parts[0].parse().map_err(|_| format!("Invalid rgb: {}", v))?;
                let g: u8 = parts[1].parse().map_err(|_| format!("Invalid rgb: {}", v))?;
                let b: u8 = parts[2].parse().map_err(|_| format!("Invalid rgb: {}", v))?;
                Ok(Color::rgba(r, g, b, 255))
            }
            4 => {
                let r: u8 = parts[0].parse().map_err(|_| format!("Invalid rgba: {}", v))?;
                let g: u8 = parts[1].parse().map_err(|_| format!("Invalid rgba: {}", v))?;
                let b: u8 = parts[2].parse().map_err(|_| format!("Invalid rgba: {}", v))?;
                let a: f32 = parts[3].parse().map_err(|_| format!("Invalid rgba: {}", v))?;
                Ok(Color::rgba(r, g, b, (a * 255.0).round() as u8))
            }
            _ => Err(format!("Invalid rgb(a) format: {}", v)),
        }
    }

    /// Parse a CSS length value (e.g., "10px", "2", "0")
    fn parse_length(value: &str) -> Result<u32, String> {
        let v = value.trim();
        let num_str = v.trim_end_matches("px").trim_end_matches("pt").trim_end_matches("em").trim();
        let f: f32 = num_str.parse().map_err(|_| format!("Invalid length: {}", value))?;
        Ok(f.max(0.0) as u32)
    }

    fn parse_float(value: &str) -> Result<f32, String> {
        let v = value.trim().trim_end_matches("px").trim_end_matches("pt").trim();
        v.parse::<f32>().map_err(|_| format!("Invalid number: {}", value))
    }

    /// Parse 1-4 space-separated CSS values into exactly N values (cloned if fewer).
    /// E.g. "10px 20px" with count=4 returns [10, 20, 10, 20] (CSS trbl pattern).
    fn parse_space_separated_lengths(value: &str, count: usize) -> Result<Vec<u32>, String> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        let parsed: Vec<u32> =
            parts.iter().map(|p| Self::parse_length(p)).collect::<Result<Vec<_>, _>>()?;
        match parsed.len() {
            1 => Ok(vec![parsed[0]; count]),
            2 => Ok(vec![parsed[0], parsed[1], parsed[0], parsed[1]]),
            3 => Ok(vec![parsed[0], parsed[1], parsed[2], parsed[1]]),
            4 => Ok(parsed),
            _ if count == 4 && parts.len() == 1 && parts[0] == "0" => Ok(vec![0; 4]),
            _ => Err(format!("Expected 1-4 values for spacing, got {}", parts.len())),
        }
    }

    fn skip_whitespace_and_comments(chars: &[char], mut pos: usize) -> usize {
        while pos < chars.len() {
            // Skip multi-line comment
            if pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '*' {
                pos += 2;
                while pos + 1 < chars.len() && !(chars[pos] == '*' && chars[pos + 1] == '/') {
                    pos += 1;
                }
                pos += 2; // skip */
                continue;
            }
            // Skip single-line comment
            if pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '/' {
                pos += 2;
                while pos < chars.len() && chars[pos] != '\n' {
                    pos += 1;
                }
                pos += 1;
                continue;
            }
            if !chars[pos].is_whitespace() {
                break;
            }
            pos += 1;
        }
        pos
    }
}

// Need to add declarations field to StyleRule — use extension or modify selector
// For now, add a helper extension trait

// Extension mechanism to store CSS declarations — uses global HashMap keyed by rule name
// because StyleRule doesn't have a declarations field.
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

static DECLARATIONS: LazyLock<Mutex<HashMap<String, Vec<CssDeclaration>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn store_declarations(rule_name: &str, decls: Vec<CssDeclaration>) {
    DECLARATIONS.lock().unwrap().insert(rule_name.to_string(), decls);
}

pub fn get_declarations(rule_name: &str) -> Option<Vec<CssDeclaration>> {
    DECLARATIONS.lock().unwrap().get(rule_name).cloned()
}

impl CssParser {
    /// Parse CSS text into StyleSheet with stored declarations.
    /// Then use `apply_matched_rules` to apply matching rules to a WidgetStyle.
    pub fn parse_and_apply(
        css: &str,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        let rules = Self::parse_rules(css)?;
        // Filter and apply matching rules in order
        for rule in &rules {
            if let Some(selector) = Self::parse_selector(&rule.selector_text) {
                if selector.matches(kind, class, id, state) {
                    Self::apply_declarations(&rule.declarations, style)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let css = "Button { color: #333; font-size: 14px; }";
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].selector_text, "Button");
        assert_eq!(rules[0].declarations.len(), 2);
    }

    #[test]
    fn test_parse_class_selector() {
        let css = ".primary { background: #0066cc; }";
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules[0].selector_text, ".primary");
    }

    #[test]
    fn test_parse_id_selector() {
        let css = "#submit { color: red; }";
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules[0].selector_text, "#submit");
    }

    #[test]
    fn test_parse_pseudo_selector() {
        let css = "Button:hover { background: blue; }";
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules[0].selector_text, "Button:hover");
    }

    #[test]
    fn test_parse_multiple_rules() {
        let css = r#"
            Button { color: black; }
            Label { color: gray; font-size: 12px; }
            .warn { color: red; }
        "#;
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules.len(), 3);
    }

    #[test]
    fn test_parse_hex_color_3() {
        let c = CssParser::parse_color("#f00").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn test_parse_hex_color_6() {
        let c = CssParser::parse_color("#ff0000").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn test_parse_hex_color_8() {
        let c = CssParser::parse_color("#ff000080").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 128));
    }

    #[test]
    fn test_parse_rgb_color() {
        let c = CssParser::parse_color("rgb(255, 0, 0)").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn test_parse_rgba_color() {
        let c = CssParser::parse_color("rgba(255, 0, 0, 0.5)").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 128));
    }

    #[test]
    fn test_parse_named_colors() {
        let c = CssParser::parse_color("red").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
        let c = CssParser::parse_color("white").unwrap();
        assert_eq!(c, Color::rgba(255, 255, 255, 255));
        let c = CssParser::parse_color("transparent").unwrap();
        assert_eq!(c, Color::rgba(0, 0, 0, 0));
    }

    #[test]
    fn test_parse_length() {
        assert_eq!(CssParser::parse_length("10px").unwrap(), 10);
        assert_eq!(CssParser::parse_length("0").unwrap(), 0);
        assert_eq!(CssParser::parse_length("5.5px").unwrap(), 5);
    }

    #[test]
    fn test_parse_spacing_shorthand_1() {
        let vals = CssParser::parse_space_separated_lengths("10px", 4).unwrap();
        assert_eq!(vals, vec![10, 10, 10, 10]);
    }

    #[test]
    fn test_parse_spacing_shorthand_2() {
        let vals = CssParser::parse_space_separated_lengths("10px 20px", 4).unwrap();
        assert_eq!(vals, vec![10, 20, 10, 20]);
    }

    #[test]
    fn test_parse_spacing_shorthand_4() {
        let vals = CssParser::parse_space_separated_lengths("1px 2px 3px 4px", 4).unwrap();
        assert_eq!(vals, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_apply_color_declaration() {
        let mut style = WidgetStyle::default();
        let decl = CssDeclaration { property: "color".into(), value: "#333".into() };
        CssParser::apply_declarations(&[decl], &mut style).unwrap();
        assert_eq!(style.text_color, Some(Color::rgba(51, 51, 51, 255)));
    }

    #[test]
    fn test_apply_background_declaration() {
        let mut style = WidgetStyle::default();
        let decl = CssDeclaration { property: "background".into(), value: "#0066cc".into() };
        CssParser::apply_declarations(&[decl], &mut style).unwrap();
        assert_eq!(style.background_color, Some(Color::rgba(0, 102, 204, 255)));
    }

    #[test]
    fn test_parse_complex_stylesheet() {
        let css = r#"
            /* Button base style */
            Button {
                color: #333;
                font-size: 14px;
                padding: 8px 16px;
                border-radius: 4px;
            }
            /* Primary variant */
            .primary {
                background: #0066cc;
                color: white;
            }
            /* Hover state */
            Button:hover {
                background: #0052a3;
            }
            /* Disabled state */
            Button:disabled {
                opacity: 0.5;
            }
        "#;
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules.len(), 4);

        // Test applying rules to a Button with .primary class
        let mut style = WidgetStyle::default();
        for rule in &rules {
            if let Some(sel) = CssParser::parse_selector(&rule.selector_text) {
                if sel.matches("Button", Some("primary"), None, None) {
                    CssParser::apply_declarations(&rule.declarations, &mut style).unwrap();
                }
            }
        }
        // Button base applied
        assert_eq!(style.border_radius, 4);
        // .primary overrides
        assert_eq!(style.background_color, Some(Color::rgba(0, 102, 204, 255)));
    }

    #[test]
    fn test_selector_parser() {
        let sel = CssParser::parse_selector("Button").unwrap();
        assert_eq!(sel, CssSelector::Kind("Button".to_string()));

        let sel = CssParser::parse_selector(".primary").unwrap();
        assert_eq!(sel, CssSelector::Class("primary".to_string()));

        let sel = CssParser::parse_selector("#main").unwrap();
        assert_eq!(sel, CssSelector::Id("main".to_string()));

        let sel = CssParser::parse_selector(":hover").unwrap();
        assert_eq!(sel, CssSelector::State(PseudoState::Hover));
    }

    #[test]
    fn test_parse_empty_returns_no_rules() {
        let rules = CssParser::parse_rules("").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_error_unclosed_brace() {
        let result = CssParser::parse_rules("Button { color: red; ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_comments_are_ignored() {
        let css = r#"
            /* this is a comment */
            Button { color: red; }
            /* another comment */
            Label { color: blue; }
        "#;
        let rules = CssParser::parse_rules(css).unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_opacity_declaration() {
        let mut style = WidgetStyle::default();
        let decl = CssDeclaration { property: "opacity".into(), value: "0.5".into() };
        CssParser::apply_declarations(&[decl], &mut style).unwrap();
        assert!((style.opacity.unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_margin_shorthand() {
        let mut style = WidgetStyle::default();
        let decl = CssDeclaration { property: "margin".into(), value: "10px 20px".into() };
        CssParser::apply_declarations(&[decl], &mut style).unwrap();
        assert_eq!(style.margin.top, 10);
        assert_eq!(style.margin.right, 20);
        assert_eq!(style.margin.bottom, 10);
        assert_eq!(style.margin.left, 20);
    }
}
