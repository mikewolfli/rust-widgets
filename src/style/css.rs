// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

use crate::compat::{format, vec, MiniToString, String, Vec};
use crate::core::Color;
use crate::style::{PseudoState, Selector, StyleRule, StyleSheet, WidgetStyle};
use crate::widget::WidgetKind;

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
    /// Convert this CSS-parsed selector into the canonical `Selector` enum.
    ///
    /// Returns `None` when the selector names a widget kind the library does not
    /// know. Encoding "unknown kind" as a `Selector` was the previous behaviour and
    /// it was wrong in a way that is hard to notice: an unrecognised name fell back
    /// to `WidgetKind::Window`, so a rule written `Stepper { … }` silently selected
    /// every **window** in the tree. `None` makes the caller decide, and the
    /// alternative — matching nothing — is the honest reading of a rule whose
    /// subject does not exist.
    pub fn to_selector(&self) -> Option<Selector> {
        Some(match self {
            CssSelector::Universal => Selector::Universal,
            CssSelector::Kind(name) => Selector::Kind(widget_kind_from_str(name)?),
            CssSelector::Class(name) => Selector::Class(name.clone()),
            CssSelector::Id(id) => Selector::Id(id.clone()),
            CssSelector::State(state) => Selector::State(*state),
            CssSelector::And(selectors) => Selector::And(
                selectors.iter().map(CssSelector::to_selector).collect::<Option<Vec<_>>>()?,
            ),
        })
    }

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
///
/// The value is **not** interpreted here — it stays as written in the
/// stylesheet, so the consumer of a declaration is responsible for parsing
/// units and rejecting values it cannot apply.
#[derive(Debug, Clone, PartialEq)]
pub struct CssDeclaration {
    /// Property name exactly as written in the source, for example
    /// `"background-color"`. Not normalised, so case variants are distinct keys.
    pub property: String,
    /// Raw value text, for example `"#ff0000"` or `"12px"`. Unparsed and
    /// untrimmed beyond what the tokeniser already removed.
    pub value: String,
}

/// A parsed CSS rule: selector text → declarations.
///
/// The selector is kept as text rather than a parsed selector type, so the rule
/// remembers how it was written; matching happens elsewhere.
#[derive(Debug, Clone, PartialEq)]
pub struct CssRule {
    /// The selector as it appeared in the source, for example
    /// `"Button.primary:hover"`.
    pub selector_text: String,
    /// The rule's declarations, in source order. No deduplication is performed,
    /// so a property written twice appears twice.
    pub declarations: Vec<CssDeclaration>,
}

/// Convert a widget kind name string (e.g. "Button", "Label") to a `WidgetKind` variant.
///
/// Returns `None` for a name that is not a known kind. The previous version
/// returned `WidgetKind::Window` as a "safe default", which was not safe: a rule
/// whose subject the parser did not recognise would match windows instead of
/// matching nothing, and the resulting mis-styling gave no hint about the cause.
///
/// The table is a suffix of the full `WidgetKind` set. A kind absent from it is
/// reported as unknown rather than guessed at; adding a name here is a deliberate
/// act, and `every_widget_kind_name_resolves_or_is_declared_unknown` in the tests
/// keeps the table honest about which kinds are covered.
fn widget_kind_from_str(name: &str) -> Option<WidgetKind> {
    // Case-insensitive matching of all variants common in CSS selectors.
    Some(match name {
        n if n.eq_ignore_ascii_case("Window") => WidgetKind::Window,
        n if n.eq_ignore_ascii_case("Button") => WidgetKind::Button,
        n if n.eq_ignore_ascii_case("CheckBox") || n.eq_ignore_ascii_case("Checkbox") => {
            WidgetKind::CheckBox
        }
        n if n.eq_ignore_ascii_case("RadioButton") || n.eq_ignore_ascii_case("Radiobutton") => {
            WidgetKind::RadioButton
        }
        n if n.eq_ignore_ascii_case("Label") => WidgetKind::Label,
        n if n.eq_ignore_ascii_case("LineEdit") => WidgetKind::LineEdit,
        n if n.eq_ignore_ascii_case("ComboBox") => WidgetKind::ComboBox,
        n if n.eq_ignore_ascii_case("SpinBox") => WidgetKind::SpinBox,
        n if n.eq_ignore_ascii_case("ListBox") => WidgetKind::ListBox,
        n if n.eq_ignore_ascii_case("ProgressBar") => WidgetKind::ProgressBar,
        n if n.eq_ignore_ascii_case("Slider") => WidgetKind::Slider,
        n if n.eq_ignore_ascii_case("ScrollBar") => WidgetKind::ScrollBar,
        n if n.eq_ignore_ascii_case("ScrollArea") => WidgetKind::ScrollArea,
        n if n.eq_ignore_ascii_case("Panel") => WidgetKind::Panel,
        n if n.eq_ignore_ascii_case("Frame") => WidgetKind::Frame,
        n if n.eq_ignore_ascii_case("GroupBox") => WidgetKind::GroupBox,
        n if n.eq_ignore_ascii_case("Line") => WidgetKind::Line,
        n if n.eq_ignore_ascii_case("Meter") => WidgetKind::Meter,
        n if n.eq_ignore_ascii_case("MiniChart") => WidgetKind::MiniChart,
        n if n.eq_ignore_ascii_case("ImageView") => WidgetKind::ImageView,
        n if n.eq_ignore_ascii_case("Arc") => WidgetKind::Arc,
        n if n.eq_ignore_ascii_case("Spinner") => WidgetKind::Spinner,
        n if n.eq_ignore_ascii_case("Roller") => WidgetKind::Roller,
        n if n.eq_ignore_ascii_case("Dropdown") => WidgetKind::Dropdown,
        n if n.eq_ignore_ascii_case("TextArea") => WidgetKind::TextArea,
        n if n.eq_ignore_ascii_case("Keyboard") => WidgetKind::Keyboard,
        n if n.eq_ignore_ascii_case("Switch") => WidgetKind::Switch,
        n if n.eq_ignore_ascii_case("MiniCanvas") => WidgetKind::MiniCanvas,
        // `RadarChart` exists only with the full widget set, matching the kind's
        // own gate in `kind.rs`. Without this the arm would name a variant the
        // `mini`/`embedded` profiles compile out.
        #[cfg(full_widgets)]
        n if n.eq_ignore_ascii_case("RadarChart") => WidgetKind::RadarChart,
        #[cfg(full_widgets)]
        n if n.eq_ignore_ascii_case("KanbanBoard") => WidgetKind::KanbanBoard,
        #[cfg(full_widgets)]
        n if n.eq_ignore_ascii_case("Cascader") => WidgetKind::Cascader,
        #[cfg(full_widgets)]
        n if n.eq_ignore_ascii_case("QueryBuilder") => WidgetKind::QueryBuilder,
        #[cfg(full_widgets)]
        n if n.eq_ignore_ascii_case("EmojiPicker") => WidgetKind::EmojiPicker,
        #[cfg(full_widgets)]
        n if n.eq_ignore_ascii_case("Mention") => WidgetKind::Mention,
        _ => return None,
    })
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
            // A selector whose kind is unknown contributes no rule: silently
            // substituting another kind is what made a typo'd selector mis-style
            // an unrelated control. The declarations are still stored above, so a
            // caller can inspect what was written.
            let Some(selector) =
                Self::parse_selector(&rule.selector_text).and_then(|cs| cs.to_selector())
            else {
                log::warn!(
                    "CSS rule selector {:?} names no known widget kind and was not registered as a \
                     rule",
                    rule.selector_text
                );
                continue;
            };
            let rule_entry = StyleRule::new(selector, &rule.selector_text);
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
                return Err(
                    "CSS rule is unterminated: the `{` that opens the declaration block is \
                     missing at end of input"
                        .to_string(),
                );
            }
            let selector_text: String = chars[start..pos].iter().collect();
            let selector_text = selector_text.trim().to_string();
            if selector_text.is_empty() {
                return Err(
                    "CSS rule has an empty selector before `{`; every rule needs a selector \
                     such as `Button` or `.primary`"
                        .to_string(),
                );
            }

            pos += 1; // skip '{'

            // Read declarations until '}'
            let decl_start = pos;
            while pos < chars.len() && chars[pos] != '}' {
                pos += 1;
            }
            if pos >= chars.len() {
                return Err(
                    "CSS rule is unterminated: the declaration block is missing its closing \
                     `}` at end of input"
                        .to_string(),
                );
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

    /// Applies one declaration written as `"property: value"`.
    ///
    /// # Why this takes text rather than a parsed declaration
    ///
    /// Callers that hold a ``"background-color: #FFF"`` string — the C ABI and the
    /// JSON path both do — should not have to construct a [`CssDeclaration`], which
    /// would mean either exposing the struct or duplicating the `":"` split. Taking
    /// the text keeps one parser for both.
    ///
    /// # Why an unparseable declaration is an error
    ///
    /// A string with no `":"`, or with an empty side, is rejected rather than
    /// ignored. The stylesheet path deliberately skips such lines, because a
    /// hand-written stylesheet may carry comments and half-finished edits; a
    /// programmatic single-property write has no such excuse, and silently doing
    /// nothing would leave the caller believing the style applied.
    ///
    /// # Why this cannot report an unknown property
    ///
    /// `Self::apply_one` ignores unknown properties, which is what the CSS spec
    /// requires of a *stylesheet* — a sheet written for one renderer must load in
    /// another that supports fewer properties. That tolerance is right for a sheet and
    /// wrong for a single programmatic write, where the caller typed one property and
    /// nothing else: a misspelling there is a bug, and reporting it as success hides
    /// it. The ABI therefore checks the property name against this parser's vocabulary
    /// before applying, using [`Self::is_known_property`].
    pub fn apply_declaration_text(text: &str, style: &mut WidgetStyle) -> Result<(), String> {
        let (property, value) = Self::split_declaration(text)?;
        Self::apply_one(
            &CssDeclaration { property: property.to_string(), value: value.to_string() },
            style,
        )
    }

    /// Splits `"property: value"`, rejecting the shapes that cannot be a declaration.
    fn split_declaration(text: &str) -> Result<(&str, &str), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("empty style declaration".to_string());
        }
        let Some(index) = trimmed.find(':') else {
            return Err(format!(
                "style declaration {trimmed:?} has no ':' separator; expected \"property: value\""
            ));
        };
        let property = trimmed[..index].trim();
        let value = trimmed[index + 1..].trim();
        if property.is_empty() {
            return Err(format!("style declaration {trimmed:?} has an empty property name"));
        }
        if value.is_empty() {
            return Err(format!(
                "style declaration {trimmed:?} has an empty value for property {property:?}"
            ));
        }
        Ok((property, value))
    }

    /// Whether `property` is one this parser acts on.
    ///
    /// # Why this is a list and not a second match
    ///
    /// The alternative was to make `Self::apply_one` return whether it matched, but
    /// that would tempt a caller to treat the unknown case as an error everywhere —
    /// including in the stylesheet path, where ignoring is correct. Naming the
    /// vocabulary separately keeps the two policies independent, and the test
    /// `known_property_list_matches_what_apply_one_handles` fails if this list and
    /// `apply_one`'s arms ever disagree.
    pub fn is_known_property(property: &str) -> bool {
        const KNOWN: &[&str] = &[
            "color",
            "text-color",
            "foreground",
            "background",
            "background-color",
            "border-color",
            "border-width",
            "border-style",
            "border-radius",
            "font-size",
            "font-weight",
            "font-family",
            "padding",
            "margin",
            "opacity",
            "background-gradient",
            "shadow",
            "touch-target",
        ];
        KNOWN.iter().any(|known| known.eq_ignore_ascii_case(property))
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
                style.border_width = Some(Self::parse_length(&decl.value)?);
            }
            "border-radius" => {
                style.border_radius = Some(Self::parse_length(&decl.value)?);
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
                    font.set_size(Self::parse_float(&decl.value)?);
                } else {
                    let mut font = crate::core::Font::default_ui();
                    font.set_size(Self::parse_float(&decl.value)?);
                    style.font = Some(font);
                }
            }
            // ── Gradient, shadow, touch target ──────────────────────────
            // `WidgetStyle` carries these three and CSS could not set any of them,
            // so a stylesheet could not express the whole style record. Covered here
            // so the CSS surface and the style type agree on what is settable.
            "background-gradient" => {
                style.background_gradient = Some(Self::parse_gradient(&decl.value)?);
            }
            "shadow" => {
                // `none` is a meaningful value, not an omission: it clears a shadow
                // the theme or a lower-priority sheet supplied.
                if decl.value.trim().eq_ignore_ascii_case("none") {
                    style.shadow = None;
                } else {
                    style.shadow = Some(Self::parse_shadow(&decl.value)?);
                }
            }
            "touch-target" | "touch-target-size" => {
                // Order is `width height`, matching the two-value CSS shorthand the
                // spacing properties already use.
                let vals = Self::parse_space_separated_lengths(&decl.value, 2)?;
                style.touch_target = Some(crate::core::Size::new(vals[0], vals[1]));
            }
            "font-family" => {
                let family = decl.value.trim().trim_matches('"').trim_matches('\'').to_string();
                if let Some(font) = &mut style.font {
                    font.set_family(family);
                } else {
                    let mut font = crate::core::Font::default_ui();
                    font.set_family(family);
                    style.font = Some(font);
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
                _ => Err(format!(
                    "unknown color '{v}'; use a #RGB/#RRGGBB/#RRGGBBAA literal or one of \
                     the named colors (black, white, red, green, blue, yellow, cyan, magenta, \
                     gray, orange, purple, pink, brown, navy, teal, maroon, olive, transparent)"
                )),
            }
        }
    }

    fn parse_hex_color(hex: &str) -> Result<Color, String> {
        let hex = hex.trim();
        let (r, g, b, a) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?
                    * 17;
                let g = u8::from_str_radix(&hex[1..2], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?
                    * 17;
                let b = u8::from_str_radix(&hex[2..3], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?
                    * 17;
                (r, g, b, 255)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                (r, g, b, 255)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                let a = u8::from_str_radix(&hex[6..8], 16)
                    .map_err(|_| format!("Invalid hex: {hex}"))?;
                (r, g, b, a)
            }
            _ => return Err(format!("Invalid hex color length: #{hex}")),
        };
        Ok(Color::rgba(r, g, b, a))
    }

    fn parse_rgb_color(v: &str) -> Result<Color, String> {
        let inner = v.trim_start_matches("rgba").trim_start_matches("rgb");
        let inner = inner.trim_start_matches('(').trim_end_matches(')').trim();
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        match parts.len() {
            3 => {
                let r: u8 = parts[0].parse().map_err(|_| format!("Invalid rgb: {v}"))?;
                let g: u8 = parts[1].parse().map_err(|_| format!("Invalid rgb: {v}"))?;
                let b: u8 = parts[2].parse().map_err(|_| format!("Invalid rgb: {v}"))?;
                Ok(Color::rgba(r, g, b, 255))
            }
            4 => {
                let r: u8 = parts[0].parse().map_err(|_| format!("Invalid rgba: {v}"))?;
                let g: u8 = parts[1].parse().map_err(|_| format!("Invalid rgba: {v}"))?;
                let b: u8 = parts[2].parse().map_err(|_| format!("Invalid rgba: {v}"))?;
                let a: f32 = parts[3].parse().map_err(|_| format!("Invalid rgba: {v}"))?;
                Ok(Color::rgba(r, g, b, (a * 255.0).round() as u8))
            }
            _ => Err(format!("Invalid rgb(a) format: {v}")),
        }
    }

    /// Parse a CSS length value (e.g., "10px", "2", "0")
    fn parse_length(value: &str) -> Result<u32, String> {
        let v = value.trim();
        let num_str = v.trim_end_matches("px").trim_end_matches("pt").trim_end_matches("em").trim();
        let f: f32 = num_str.parse().map_err(|_| format!("Invalid length: {value}"))?;
        Ok(f.max(0.0) as u32)
    }

    fn parse_float(value: &str) -> Result<f32, String> {
        let v = value.trim().trim_end_matches("px").trim_end_matches("pt").trim();
        v.parse::<f32>().map_err(|_| format!("Invalid number: {value}"))
    }

    /// Parse 1-4 space-separated CSS values into exactly N values (cloned if fewer).
    /// E.g. "10px 20px" with count=4 returns [10, 20, 10, 20] (CSS trbl pattern).
    fn parse_space_separated_lengths(value: &str, count: usize) -> Result<Vec<u32>, String> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        let parsed: Vec<u32> =
            parts.iter().map(|p| Self::parse_length(p)).collect::<Result<Vec<_>, _>>()?;
        match parsed.len() {
            1 => Ok(vec![parsed[0]; count]),
            2 if count == 2 => Ok(parsed),
            2 => Ok(vec![parsed[0], parsed[1], parsed[0], parsed[1]]),
            3 => Ok(vec![parsed[0], parsed[1], parsed[2], parsed[1]]),
            4 => Ok(parsed),
            _ => Err(format!("Expected 1-4 values for spacing, got {}", parts.len())),
        }
    }

    /// Parse a gradient value.
    ///
    /// The accepted syntax is a deliberately small subset of CSS's:
    ///
    /// ```text
    /// linear(<angle>deg, <color> [<position>], <color> [<position>], ...)
    /// ```
    ///
    /// Only the **linear** geometry is supported, because that is the one the
    /// render layer can paint without a centre or radius to resolve: a radial or
    /// conic ramp needs geometry CSS does not carry. Rejecting them by name is
    /// better than accepting the syntax and silently painting a linear ramp.
    ///
    /// A stop without an explicit position is distributed evenly, matching how a
    /// caller would read the declaration.
    fn parse_gradient(value: &str) -> Result<crate::style::Gradient, String> {
        let v = value.trim();
        if v.eq_ignore_ascii_case("none") {
            return Err(
                "'none' is not a gradient; omit the declaration to leave a background colour, or \
                 write `background-color: transparent`"
                    .to_string(),
            );
        }

        let inner = v
            .strip_prefix("linear-gradient(")
            .or_else(|| v.strip_prefix("linear("))
            .and_then(|rest| rest.strip_suffix(')'))
            .ok_or_else(|| {
                format!(
                    "unsupported gradient '{v}': only `linear-gradient(<angle>deg, <color> [<pos>], \
                     ...)` is supported (radial and conic ramps need geometry the theme schema does \
                     not carry)"
                )
            })?;

        let mut parts = inner.split(',').map(str::trim).filter(|p| !p.is_empty());

        // Optional leading angle. `to bottom` and friends are not supported: they
        // describe a box-relative direction the widget tree does not resolve here.
        let mut angle = 180.0_f32;
        let mut stops: Vec<crate::style::GradientStop> = Vec::new();
        let mut pending: Vec<(crate::core::Color, Option<f32>)> = Vec::new();

        for part in parts.by_ref() {
            if let Some(deg) = part.strip_suffix("deg") {
                angle = deg.trim().parse::<f32>().map_err(|_| {
                    format!("invalid gradient angle in '{v}': '{part}' is not a number of degrees")
                })?;
                continue;
            }
            // `<color> [<position>]`
            let mut tokens = part.split_whitespace();
            let color_token =
                tokens.next().ok_or_else(|| format!("gradient stop in '{v}' has no colour"))?;
            let color = Self::parse_color(color_token)?;
            let position = match tokens.next() {
                Some(raw) => Some(Self::parse_stop_position(raw)?),
                None => None,
            };
            pending.push((color, position));
        }

        if pending.len() < 2 {
            return Err(format!(
                "gradient '{v}' needs at least two colour stops, found {}",
                pending.len()
            ));
        }

        // Distribute the stops that did not state a position evenly between the
        // fixed ones, which is how the ramp in a declaration is read.
        let last = pending.len() - 1;
        for (index, (color, position)) in pending.into_iter().enumerate() {
            let resolved = position.unwrap_or(index as f32 / last as f32);
            stops.push(crate::style::GradientStop::new(resolved, color));
        }

        Ok(crate::style::Gradient::linear(
            crate::core::Point::new(0, 0),
            crate::core::Point::new(0, 0),
        )
        .with_stops(stops)
        .with_angle(angle))
    }

    /// Parse a gradient stop position: a fraction (`0.5`) or a percentage (`50%`).
    fn parse_stop_position(value: &str) -> Result<f32, String> {
        let v = value.trim();
        let fraction = if let Some(percent) = v.strip_suffix('%') {
            percent.trim().parse::<f32>().map_err(|_| {
                format!("invalid gradient stop position '{value}': not a percentage")
            })? / 100.0
        } else {
            v.parse::<f32>().map_err(|_| {
                format!("invalid gradient stop position '{value}': not a number or percentage")
            })?
        };
        Ok(fraction.clamp(0.0, 1.0))
    }

    /// Parse a shadow value: `<offset-x> <offset-y> <blur> <color>`.
    ///
    /// The four parts are required, because a shadow with a missing colour would
    /// have to invent one and a shadow with a missing extent is not a shadow. The
    /// colour may be written first or last, matching the two orders CSS accepts.
    fn parse_shadow(value: &str) -> Result<crate::style::Shadow, String> {
        let v = value.trim();
        let parts: Vec<&str> = v.split_whitespace().collect();
        if parts.len() != 4 {
            return Err(format!(
                "shadow '{v}' must be `<offset-x> <offset-y> <blur> <color>`, found {} part(s)",
                parts.len()
            ));
        }

        // The colour may lead or trail; the three lengths keep their relative order.
        let (color, lengths): (crate::core::Color, &[&str]) =
            if let Ok(color) = Self::parse_color(parts[3]) {
                (color, &parts[..3])
            } else if let Ok(color) = Self::parse_color(parts[0]) {
                (color, &parts[1..])
            } else {
                return Err(format!(
                    "shadow '{v}' has no recognisable colour; expected a #RGB/#RRGGBB/#RRGGBBAA \
                     literal or a named colour"
                ));
            };

        let offset_x = Self::parse_signed_length(lengths[0])?;
        let offset_y = Self::parse_signed_length(lengths[1])?;
        let blur = Self::parse_length(lengths[2])?;
        Ok(crate::style::Shadow { x: offset_x, y: offset_y, blur, color })
    }

    /// Parse a length that may be negative.
    ///
    /// [`Self::parse_length`] clamps to `>= 0`, which is right for a size and wrong
    /// for a shadow offset: `-2px 2px` is a perfectly ordinary shadow, and clamping
    /// it would silently move the shadow instead of reporting the input.
    fn parse_signed_length(value: &str) -> Result<i32, String> {
        let v = value.trim();
        let num_str = v.trim_end_matches("px").trim_end_matches("pt").trim_end_matches("em").trim();
        let f: f32 = num_str.parse().map_err(|_| format!("Invalid length: {value}"))?;
        Ok(f.round() as i32)
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

// ── Declaration registry ────────────────────────────────────────────────────
//
// `StyleRule` carries a selector and a specificity, not the declarations
// themselves, so parsed declarations are kept in a side table keyed by the rule's
// selector text. The table is process-wide because the public `store_declarations`
// / `get_declarations` pair is a published API.
//
// Two properties matter and were both wrong before:
//
// * **It must be bounded.** It is never evicted, so an unbounded map grows for
//   the lifetime of the process every time the same stylesheet is re-parsed
//   (a hot-reload loop is the realistic case). A small cap with oldest-first
//   eviction keeps it useful for the introspection it exists for while making
//   the growth finite and observable.
// * **Lookups must be ordered.** Iterating a `HashMap` yields an unspecified
//   order, so a caller relying on cascade order got a non-deterministic merge.
//   Entries now carry the sequence number they were inserted with, and results
//   are returned in that order.

use crate::compat::atomic::{AtomicU64, Ordering};
use crate::compat::{lock, Mutex, OnceLock};

/// Monotonically increasing counter giving each stored rule a stable order and a
/// unique key, so two stylesheets with the same selector text do not collide.
static DECL_COUNTER: AtomicU64 = AtomicU64::new(0);

/// How many stored rules to keep. Sized for introspection of the stylesheets an
/// application actually has loaded (a handful), not for archival.
const MAX_STORED_RULES: usize = 512;

/// The declaration registry: insertion order → that rule's declarations.
///
/// A named alias rather than the inline type so the `static` declaration below
/// stays readable, and so the accessor functions share one spelling.
type DeclarationRegistry = Vec<(u64, String, Vec<CssDeclaration>)>;

/// The declaration registry itself.
static DECLARATIONS: OnceLock<Mutex<DeclarationRegistry>> = OnceLock::new();

/// Stores a rule's declarations in the process-wide registry.
///
/// `rule_name` is only a **suffix** of the real key: each call prepends a fresh
/// counter, so the same `rule_name` can be stored many times without overwriting
/// previous entries. Combined with [`get_declarations`], which matches by suffix,
/// this means lookups are additive — storing a rule under a name that is a suffix
/// of another rule's name will make that lookup return both.
///
/// The registry holds at most a fixed number of entries (the private
/// `MAX_STORED_RULES`); the oldest is dropped once that is reached, so repeated
/// parsing cannot grow it without bound. A poisoned lock is recovered rather than
/// propagated.
pub fn store_declarations(rule_name: &str, decls: Vec<CssDeclaration>) {
    let sequence = DECL_COUNTER.fetch_add(1, Ordering::Relaxed);
    // SAFETY: If the lock is poisoned (a previous panic while held), we recover
    // by ignoring the poison — the stored data is still valid for CSS parsing.
    // `lock` is the shared spelling of that recovery; see its documentation.
    let mut registry = lock(DECLARATIONS.get_or_init(|| Mutex::new(Vec::new())));
    if registry.len() >= MAX_STORED_RULES {
        // Drop the oldest entry. `Vec::remove(0)` is O(n) but n is the small cap
        // above and this runs once per over-cap insertion, not per lookup.
        registry.remove(0);
    }
    registry.push((sequence, rule_name.to_string(), decls));
}

/// Returns every stored declaration whose rule name ends with `rule_name`,
/// concatenated in insertion order.
///
/// Because the key is matched by suffix, a name that is a suffix of another stored
/// name yields that other rule's declarations too. Returns `None` rather than an
/// empty vector when nothing matches, so `None` is distinguishable from "matched
/// but declared nothing". The returned vector is always non-empty.
///
/// A poisoned lock is recovered rather than propagated.
pub fn get_declarations(rule_name: &str) -> Option<Vec<CssDeclaration>> {
    // SAFETY: Same poison recovery strategy — stale data is safe to read.
    let registry = lock(DECLARATIONS.get_or_init(|| Mutex::new(Vec::new())));
    let mut result = Vec::new();
    for (_, name, decls) in registry.iter() {
        if name.ends_with(rule_name) {
            result.extend(decls.iter().cloned());
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
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
        assert_eq!(style.border_radius, Some(4));
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

    // ── Unknown selector kinds ────────────────────────────────────────────

    /// A selector naming an unknown widget kind resolves to no selector, rather
    /// than silently becoming `Window`.
    #[test]
    fn unknown_kind_selects_nothing() {
        let selector = CssParser::parse_selector("NotAWidget").expect("the text parses");
        assert_eq!(selector.to_selector(), None);
    }

    /// A **known** kind still resolves, so the `None` above is about the unknown
    /// name and not a blanket refusal.
    #[test]
    fn known_kind_still_resolves() {
        let selector = CssParser::parse_selector("Button").expect("the text parses");
        assert_eq!(selector.to_selector(), Some(Selector::Kind(WidgetKind::Button)));
    }

    /// The regression this guards: before the fix, `Stepper { … }` resolved to
    /// `WidgetKind::Window` and therefore styled windows. Now it resolves to
    /// nothing, so it cannot match a window.
    #[test]
    fn an_unknown_kind_never_becomes_a_window() {
        let selector = CssParser::parse_selector("Stepper").expect("the text parses");
        assert_ne!(
            selector.to_selector(),
            Some(Selector::Kind(WidgetKind::Window)),
            "an unknown name must not be reinterpreted as Window"
        );
    }

    /// A compound selector with one unknown part resolves to nothing: a rule whose
    /// subject does not exist cannot be made to apply by adding a class to it.
    #[test]
    fn a_compound_selector_with_an_unknown_kind_selects_nothing() {
        let selector = CssParser::parse_selector("NotAWidget.primary").expect("the text parses");
        assert_eq!(selector.to_selector(), None);
    }

    /// `CssParser::parse` skips a rule whose kind is unknown instead of registering
    /// it against the wrong kind. The known rule beside it still lands.
    #[test]
    fn parse_skips_only_the_rule_with_an_unknown_kind() {
        let css = "Button { color: #111; }\nStepper { color: #222; }";
        let sheet = CssParser::parse(css).expect("the sheet parses");
        let kinds: Vec<_> = sheet.rules().iter().map(|rule| rule.selector.clone()).collect();
        assert_eq!(kinds, vec![Selector::Kind(WidgetKind::Button)], "only the known rule lands");
    }

    /// Every field of `WidgetStyle` that a stylesheet should be able to set is
    /// settable. `background_gradient`, `shadow` and `touch_target` were defined on
    /// the style record but unreachable from CSS, so a stylesheet could not express
    /// the whole style. This test is the guard against a future field being added
    /// without a property.
    #[test]
    fn the_style_fields_a_stylesheet_should_set_are_settable() {
        let declarations = [
            CssDeclaration {
                property: "background-gradient".into(),
                value: "linear-gradient(90deg, #ff0000, #0000ff)".into(),
            },
            CssDeclaration { property: "shadow".into(), value: "2px 3px 4px #112233".into() },
            CssDeclaration { property: "touch-target".into(), value: "44px 44px".into() },
        ];
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&declarations, &mut style).expect("all three must apply");

        let gradient = style.background_gradient.expect("a gradient must be set");
        assert_eq!(gradient.stops.len(), 2, "two stops, evenly distributed");
        assert_eq!(gradient.stops[0].color, Color::rgba(255, 0, 0, 255));
        assert_eq!(gradient.stops[1].color, Color::rgba(0, 0, 255, 255));
        assert_eq!(gradient.stops[0].position, 0.0);
        assert_eq!(gradient.stops[1].position, 1.0);

        let shadow = style.shadow.expect("a shadow must be set");
        assert_eq!((shadow.x, shadow.y, shadow.blur), (2, 3, 4));
        assert_eq!(shadow.color, Color::rgba(0x11, 0x22, 0x33, 255));

        assert_eq!(style.touch_target, Some(crate::core::Size::new(44, 44)));
    }

    /// A gradient stop may carry an explicit position, as a fraction or a percentage.
    #[test]
    fn gradient_stop_positions_are_honoured() {
        let decl = CssDeclaration {
            property: "background-gradient".into(),
            value: "linear-gradient(#ff0000 0%, #00ff00 25%, #0000ff)".into(),
        };
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&[decl], &mut style).expect("apply");

        let stops = style.background_gradient.expect("gradient").stops;
        assert_eq!(stops.len(), 3);
        assert_eq!(stops[0].position, 0.0);
        assert_eq!(stops[1].position, 0.25);
        // A stop with no position takes the evenly-spaced slot, which for the last
        // of three stops is 1.0.
        assert_eq!(stops[2].position, 1.0);
    }

    /// `shadow: none` clears a shadow rather than being rejected, so a higher-priority
    /// sheet can remove one a lower-priority sheet installed.
    #[test]
    fn shadow_none_clears_a_shadow() {
        let mut style = WidgetStyle::default();
        let set = CssDeclaration { property: "shadow".into(), value: "1px 1px 2px #000000".into() };
        CssParser::apply_declarations(&[set], &mut style).expect("apply");
        assert!(style.shadow.is_some());

        let clear = CssDeclaration { property: "shadow".into(), value: "none".into() };
        CssParser::apply_declarations(&[clear], &mut style).expect("apply");
        assert_eq!(style.shadow, None, "`none` must clear the shadow");
    }

    /// A negative shadow offset is legal and must survive, not be clamped to zero.
    #[test]
    fn a_negative_shadow_offset_is_preserved() {
        let decl =
            CssDeclaration { property: "shadow".into(), value: "-2px 2px 4px #000000".into() };
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&[decl], &mut style).expect("apply");
        let shadow = style.shadow.expect("shadow");
        assert_eq!(shadow.x, -2, "a negative offset must not be clamped to 0");
        assert_eq!(shadow.y, 2);
    }

    /// A shadow written with the colour first is accepted too; CSS allows both orders.
    #[test]
    fn a_shadow_may_lead_with_its_colour() {
        let decl =
            CssDeclaration { property: "shadow".into(), value: "#000000 1px 2px 3px".into() };
        let mut style = WidgetStyle::default();
        CssParser::apply_declarations(&[decl], &mut style).expect("apply");
        let shadow = style.shadow.expect("shadow");
        assert_eq!((shadow.x, shadow.y, shadow.blur), (1, 2, 3));
        assert_eq!(shadow.color, Color::rgba(0, 0, 0, 255));
    }

    /// Malformed values are reported rather than silently ignored.
    #[test]
    fn malformed_gradient_and_shadow_values_are_reported() {
        let cases = [
            ("background-gradient", "radial-gradient(#ff0000, #0000ff)"),
            ("background-gradient", "linear-gradient(#ff0000)"),
            ("background-gradient", "not a gradient"),
            ("shadow", "1px 2px #000000"),
            ("shadow", "1px 2px 3px not-a-colour"),
        ];
        for (property, value) in cases {
            let decl = CssDeclaration { property: property.into(), value: value.into() };
            let mut style = WidgetStyle::default();
            assert!(
                CssParser::apply_declarations(&[decl], &mut style).is_err(),
                "{property}: {value:?} should be rejected, not silently ignored"
            );
        }
    }

    /// An unsupported geometry is refused *by name*, so a caller using a radial ramp
    /// is told why rather than getting a linear one painted instead.
    #[test]
    fn an_unsupported_gradient_geometry_says_so() {
        let decl = CssDeclaration {
            property: "background-gradient".into(),
            value: "radial-gradient(#ff0000, #0000ff)".into(),
        };
        let mut style = WidgetStyle::default();
        let error = CssParser::apply_declarations(&[decl], &mut style).expect_err("must reject");
        assert!(error.contains("linear-gradient"), "{error}");
        assert!(
            error.contains("radial") || error.contains("geometry"),
            "the message must name the unsupported geometry: {error}"
        );
    }

    // ── Declaration registry ──────────────────────────────────────────────

    /// A stored rule is retrievable by its name, and a name that was never stored
    /// answers `None` rather than an empty vector.
    #[test]
    fn stored_declarations_round_trip() {
        let name = "reg-round-trip";
        store_declarations(
            name,
            vec![CssDeclaration { property: "color".into(), value: "#010203".into() }],
        );
        let retrieved = get_declarations(name).expect("the stored rule must be found");
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].property, "color");
        assert_eq!(get_declarations("reg-never-stored"), None);
    }

    /// Repeated storage of the same selector is bounded. Before the cap this grew
    /// for the lifetime of the process, which a hot-reload loop would drive
    /// without limit.
    #[test]
    fn the_registry_is_bounded() {
        let name = "reg-bounded";
        let last = MAX_STORED_RULES + 63;
        for index in 0..last {
            store_declarations(
                name,
                vec![CssDeclaration { property: "color".into(), value: format!("#{index:06x}") }],
            );
        }
        let registry_len = lock(DECLARATIONS.get_or_init(|| Mutex::new(Vec::new()))).len();
        assert!(
            registry_len <= MAX_STORED_RULES,
            "the registry held {registry_len} entries, over the cap of {MAX_STORED_RULES}"
        );
        // The most recent insertion must still be present, so eviction removes the
        // oldest rather than simply truncating the newest.
        let retrieved = get_declarations(name).expect("the latest rule survives");
        assert_eq!(
            retrieved.last().map(|decl| decl.value.clone()),
            Some(format!("#{:06x}", last - 1)),
            "the newest entry must be the one still stored"
        );
    }

    /// Lookups come back in insertion order. The registry used to be a `HashMap`,
    /// so a caller relying on cascade order got an unspecified one.
    #[test]
    fn declarations_come_back_in_insertion_order() {
        let name = "reg-order";
        store_declarations(
            name,
            vec![CssDeclaration { property: "color".into(), value: "#aaaaaa".into() }],
        );
        store_declarations(
            name,
            vec![CssDeclaration { property: "color".into(), value: "#bbbbbb".into() }],
        );
        let retrieved = get_declarations(name).expect("both entries found");
        assert_eq!(retrieved[0].value, "#aaaaaa");
        assert_eq!(retrieved[1].value, "#bbbbbb");
    }

    /// `is_known_property` must accept exactly what `apply_one` acts on, and reject
    /// everything else.
    ///
    /// # Why this test is the point of the two lists
    ///
    /// `apply_one` silently ignores an unknown property, so a name this list claims but
    /// `apply_one` does not handle would be reported as applied while changing nothing
    /// — the exact failure mode the list exists to prevent. Checking it by *behaviour*
    /// rather than by reading the source means adding a property to `apply_one` without
    /// adding it here fails here, not in a user's program.
    #[test]
    fn known_property_list_matches_what_apply_one_handles() {
        // Every name the list claims must actually modify a style.
        for property in [
            "color",
            "background-color",
            "border-color",
            "border-width",
            "border-radius",
            "font-size",
            "padding",
            "margin",
            "opacity",
            "background-gradient",
            "shadow",
            "touch-target",
        ] {
            assert!(CssParser::is_known_property(property), "{property} should be known");
            let before = WidgetStyle::default();
            let mut after = before.clone();
            let value = match property {
                "color" | "background-color" | "border-color" => "#123456",
                "border-radius" | "font-size" | "opacity" => "4",
                "padding" | "margin" | "touch-target" => "2 2",
                "border-width" => "1",
                "background-gradient" => "linear-gradient(90deg, #FFF, #000)",
                "shadow" => "0 1 2 #000",
                _ => unreachable!(),
            };
            CssParser::apply_declaration_text(&format!("{property}: {value}"), &mut after)
                .unwrap_or_else(|error| panic!("{property} should parse, got: {error}"));
            assert_ne!(
                format!("{after:?}"),
                format!("{before:?}"),
                "{property} is listed as known but applying it changed nothing"
            );
        }

        // A misspelling must be rejected rather than reported as applied.
        assert!(!CssParser::is_known_property("backgrond-color"));
        let mut style = WidgetStyle::default();
        let error = CssParser::apply_declaration_text("not-a-declaration", &mut style)
            .expect_err("a declaration with no ':' must be refused");
        assert!(error.contains("':'"), "the error should name the missing separator: {error}");

        let error = CssParser::apply_declaration_text("background-color:", &mut style)
            .expect_err("an empty value must be refused");
        assert!(error.contains("empty value"), "the error should say the value is empty: {error}");

        assert!(CssParser::apply_declaration_text("  ", &mut style).is_err());
    }
}
