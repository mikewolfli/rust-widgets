// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Global stylesheet manager for CSS-based widget theming (BLUE13 R1.6).
//!
//! StyleSheetManager holds a global parsed stylesheet and applies matching
//! CSS rules to widget styles. This enables app-wide theming similar to
//! LVGL's `lv_theme_t` but using Rust's type-safe CSS parser.

use crate::style::css::{CssDeclaration, CssParser, ParsedCssRule};
use crate::style::css_trace::TracedRuleSheet;
use crate::style::selector::{with_match_scratch, ElementRef, RuleIndex};
use crate::style::PseudoState;
use crate::style::WidgetStyle;

use crate::compat::Mutex;
use crate::compat::MutexGuard;
use crate::compat::OnceLock;
use crate::compat::{lock, MiniToString, String, Vec};

/// A registered stylesheet with its raw CSS text, priority, and parsed rules.
///
/// # Why the rules are parsed here
///
/// `apply_to` used to call `CssParser::parse_and_apply`, which re-tokenised the entire sheet and
/// re-parsed every selector for **every control that asked for a style**. A theme registered once
/// was therefore re-parsed thousands of times a frame. Parsing at `register` makes the apply path
/// a selector match plus the declarations it already matched.
///
/// # Why the index lives here too
///
/// The sheet's rules are parsed once, so the narrowing index over them can be built once as well
/// (G-162). Without it every apply walked every rule of every sheet, which is the same concept
/// [`StyleSheet`](crate::style::selector::StyleSheet) already solved with an index — one concept,
/// one implementation (principle #101).
#[derive(Debug)]
struct RegisteredSheet {
    /// Name for debugging.
    name: String,
    /// Priority (higher overrides lower).
    priority: u8,
    /// The sheet's rules, in source order; a selector the parser refused was dropped at
    /// registration so every position this sheet holds is matchable.
    rules: Vec<ParsedCssRule>,
    /// Rule positions by narrowing key, built when the sheet is registered.
    index: RuleIndex,
    /// The parse error, when the sheet was accepted but could not be parsed.
    parse_error: Option<String>,
}

/// Global stylesheet manager for CSS-based theming.
///
/// Manages a set of named CSS stylesheets with associated priorities.
/// When applying styles, sheets are processed in ascending priority order
/// so that higher-priority sheets can override lower-priority ones.
#[derive(Debug)]
pub struct StyleSheetManager {
    sheets: Vec<RegisteredSheet>,
}

impl StyleSheetManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self { sheets: Vec::new() }
    }

    /// Register a CSS stylesheet with a name and optional priority.
    /// Higher priority sheets override lower ones when rules conflict.
    pub fn register(&mut self, name: &str, css: &str, priority: u8) {
        // Remove existing sheet with the same name, then add the new one.
        self.sheets.retain(|s| s.name != name);
        let (rules, parse_error) = match CssParser::parse_prepared_rules(css) {
            // A selector whose kind is unknown contributes no rule: silently substituting another
            // kind is what made a typo'd selector mis-style an unrelated control. The trace path
            // reports the same rule instead of dropping it, which is why `parse_prepared_rules`
            // keeps the `None` rather than filtering.
            Ok(rules) => (rules.into_iter().filter(|rule| rule.selector.is_some()).collect(), None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let mut index = RuleIndex::new();
        for (position, rule) in rules.iter().enumerate() {
            if let Some(selector) = &rule.selector {
                index.insert(position, selector.index_key());
            }
        }
        self.sheets.push(RegisteredSheet {
            name: name.to_string(),
            priority,
            rules,
            index,
            parse_error,
        });
        // Keep the list in apply order, so `apply_to` neither sorts nor allocates per call.
        self.sheets.sort_by_key(|sheet| sheet.priority);
    }

    /// Remove a registered stylesheet by name.
    /// Returns `true` if a sheet was removed.
    pub fn unregister(&mut self, name: &str) -> bool {
        let len_before = self.sheets.len();
        self.sheets.retain(|s| s.name != name);
        self.sheets.len() < len_before
    }

    /// Apply all matching stylesheet rules to a widget style.
    ///
    /// Sheets are applied in priority order (lower first) so a higher-priority sheet overrides a
    /// lower one. Within the matched set, rules are applied in cascade order — ascending
    /// specificity, then source order — so `.primary` beats a bare `Button` rule even when the
    /// class rule is written first.
    ///
    /// A sheet that failed to parse is **skipped** and reported after the valid sheets have been
    /// applied. Returning early used to throw away every registered sheet at once, and the JSON
    /// loader then discarded the widget's whole style, so one malformed app-wide sheet disabled
    /// the entire CSS layer.
    pub fn apply_to(
        &self,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        self.apply_to_with_chain(kind, class, id, state, &[], style)
    }

    /// Apply all matching stylesheet rules with the element's **ancestor chain** available.
    ///
    /// A selector using a descendant or child combinator is a fact about the tree, so a caller
    /// that knows the chain should use this entry point rather than [`Self::apply_to`]; the
    /// latter passes no chain and therefore matches combinator rules never. `ancestors` is
    /// ordered root-first.
    pub fn apply_to_context(
        &self,
        element: ElementRef<'_>,
        ancestors: &[ElementRef<'_>],
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        self.apply_to_with_chain(
            element.kind.css_name(),
            element.class,
            element.id,
            element.state,
            ancestors,
            style,
        )
    }

    /// [`Self::apply_to_context`] plus a final **inline** layer.
    ///
    /// # Why inline is a separate call and not part of a sheet
    ///
    /// The cascade has three layers in this library: the theme, the registered sheets, and the
    /// element's own declaration. Inline text has to win over every sheet regardless of
    /// specificity — that is what makes an explicit `setProperty`/`apply_css` on one control an
    /// override rather than a hopeful suggestion — so it cannot be modelled as a high-specificity
    /// rule without inventing a specificity larger than every possible selector. Applying it
    /// after the sheets, through the same declaration writer, states the precedence directly.
    ///
    /// `inline_css` is CSS declaration text (`"background: #333; color: white"`), the same form
    /// `Widget::apply_css` takes, so a caller does not need a second syntax to override a sheet.
    pub fn apply_to_context_with_inline(
        &self,
        element: ElementRef<'_>,
        ancestors: &[ElementRef<'_>],
        inline_css: Option<&str>,
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        self.apply_to_context(element, ancestors, style)?;
        if let Some(text) = inline_css {
            CssParser::apply_declaration_text(text, style)?;
        }
        Ok(())
    }

    /// The shared apply path, so the chain-aware and chain-less entry points cannot drift.
    fn apply_to_with_chain(
        &self,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
        ancestors: &[ElementRef<'_>],
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        let mut parse_error: Option<String> = None;
        // (specificity, sheet order, rule position within the sheet, declarations). The rule
        // counts are small once the index has narrowed them, so a candidate-set sort is the
        // cascade order itself (G-162).
        let mut matched: Vec<(u32, usize, usize, &[CssDeclaration])> = Vec::new();

        for (sheet_order, sheet) in self.sheets.iter().enumerate() {
            if let Some(error) = &sheet.parse_error {
                if parse_error.is_none() {
                    parse_error = Some(error.clone());
                }
                continue;
            }
            with_match_scratch(|scratch| {
                sheet.index.candidates_into(kind, class, id, scratch);
                for &position in scratch.positions.iter() {
                    let Some(rule) = sheet.rules.get(position) else {
                        continue;
                    };
                    let Some(selector) = &rule.selector else {
                        continue;
                    };
                    if selector.matches_in(kind, class, id, state, ancestors) {
                        matched.push((rule.specificity, sheet_order, position, &rule.declarations));
                    }
                }
            });
        }

        matched.sort_by_key(|(specificity, sheet_order, position, _)| {
            (*specificity, *sheet_order, *position)
        });
        for (_, _, _, declarations) in matched {
            CssParser::apply_declarations(declarations, style)?;
        }

        if let Some(error) = parse_error {
            return Err(error);
        }
        Ok(())
    }

    /// Clear all registered stylesheets.
    pub fn clear(&mut self) {
        self.sheets.clear();
    }

    /// The registered sheets in the form [`crate::style::css_trace`] consumes, without
    /// re-parsing them (G-164).
    ///
    /// A designer tracing an application's actual stylesheets should use this rather than
    /// [`TracedSheet`](crate::style::css_trace::TracedSheet), which exists for an unsaved editor
    /// buffer where no registered copy exists yet.
    pub fn traced_sheets(&self) -> Vec<TracedRuleSheet<'_>> {
        self.sheets
            .iter()
            .map(|sheet| TracedRuleSheet {
                name: &sheet.name,
                priority: sheet.priority,
                rules: &sheet.rules,
            })
            .collect()
    }

    /// Returns the number of registered sheets.
    pub fn len(&self) -> usize {
        self.sheets.len()
    }

    /// Returns true if no sheets are registered.
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
    }
}

crate::impl_default_via_new!(StyleSheetManager);

/// Get the global stylesheet manager instance (thread-safe).
///
/// This lazily initializes a singleton `StyleSheetManager` behind a `Mutex`,
/// allowing safe access from multiple threads.
pub fn global_stylesheet_manager() -> MutexGuard<'static, StyleSheetManager> {
    static MANAGER: OnceLock<Mutex<StyleSheetManager>> = OnceLock::new();
    lock(MANAGER.get_or_init(|| Mutex::new(StyleSheetManager::new())))
}

/// Serialises tests that register into, clear, or otherwise depend on the
/// process-wide stylesheet manager.
///
/// The manager is shared state, so two tests that each register a sheet (or one
/// that clears the registry while another applies a rule) race and observe each
/// other's writes. Same precedent as the theme guard: shared state in a
/// process-wide singleton needs explicit serialisation in tests, not a hope that
/// the scheduling happens to work out. Compiled only for tests.
#[cfg(test)]
pub(crate) fn stylesheet_test_guard() -> crate::compat::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    lock(GUARD.get_or_init(|| Mutex::new(())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Color;

    /// Helper: create a basic WidgetStyle with known values for testing.
    fn make_style() -> WidgetStyle {
        WidgetStyle::default()
    }

    #[test]
    fn stylesheet_register_and_apply() {
        let mut manager = StyleSheetManager::new();
        let css = r#"
            Button {
                background-color: #ff0000;
                text-color: #ffffff;
            }
        "#;
        manager.register("test-sheet", css, 0);

        let mut style = make_style();
        manager.apply_to("Button", None, None, None, &mut style).expect("apply_to should succeed");

        assert_eq!(
            style.background_color,
            Some(Color::rgb(255, 0, 0)),
            "background_color should be red"
        );
        assert_eq!(style.text_color, Some(Color::rgb(255, 255, 255)), "text_color should be white");
    }

    #[test]
    fn stylesheet_priority() {
        let mut manager = StyleSheetManager::new();

        // Low priority: red background
        let css_low = r#"
            Button {
                background-color: #ff0000;
                text-color: #000000;
            }
        "#;
        manager.register("low", css_low, 0);

        // High priority: blue background (should override red)
        let css_high = r#"
            Button {
                background-color: #0000ff;
            }
        "#;
        manager.register("high", css_high, 100);

        let mut style = make_style();
        manager.apply_to("Button", None, None, None, &mut style).expect("apply_to should succeed");

        // background-color comes from high-priority sheet (blue overrides red)
        assert_eq!(
            style.background_color,
            Some(Color::rgb(0, 0, 255)),
            "high-priority blue should override low-priority red"
        );
        // text_color comes from low-priority sheet (high didn't set it)
        assert_eq!(
            style.text_color,
            Some(Color::rgb(0, 0, 0)),
            "text_color from low-priority sheet should be preserved"
        );
    }

    #[test]
    fn stylesheet_unregister() {
        let mut manager = StyleSheetManager::new();
        manager.register("theme", "Button { background-color: #00ff00; }", 0);
        assert!(!manager.is_empty(), "manager should not be empty after register");

        let removed = manager.unregister("theme");
        assert!(removed, "unregister should return true when sheet exists");
        assert!(manager.is_empty(), "manager should be empty after unregister");

        // Removing a non-existent sheet returns false.
        let removed_again = manager.unregister("nonexistent");
        assert!(!removed_again, "unregister should return false for unknown name");
    }

    #[test]
    fn stylesheet_clear() {
        let mut manager = StyleSheetManager::new();
        manager.register("a", "Button { background-color: #ff0000; }", 0);
        manager.register("b", "Label { text-color: #00ff00; }", 1);
        assert_eq!(manager.len(), 2, "should have 2 sheets");

        manager.clear();
        assert_eq!(manager.len(), 0, "should be empty after clear");
        assert!(manager.is_empty(), "is_empty should be true");
    }

    #[test]
    fn global_manager_thread_safe() {
        // Serialised: this test asserts the registry is empty afterwards, so it
        // must not race a sheet another test registers concurrently.
        let _guard = stylesheet_test_guard();
        // Get the global manager and verify it works.
        {
            let mut mgr = global_stylesheet_manager();
            mgr.clear();
            mgr.register("global-test", "Button { border-color: #ff00ff; }", 0);
        }

        // Verify the sheet is accessible (unregister to clean up).
        {
            let mut mgr = global_stylesheet_manager();
            let removed = mgr.unregister("global-test");
            assert!(removed, "should find the sheet registered in previous scope");
            assert!(mgr.is_empty(), "manager should be clean after unregister");
        }
    }

    #[test]
    fn stylesheet_register_replaces_existing() {
        let mut manager = StyleSheetManager::new();
        manager.register("sheet", "Button { background-color: #ff0000; }", 0);
        manager.register("sheet", "Button { background-color: #00ff00; }", 0);

        assert_eq!(manager.len(), 1, "re-registering same name should replace, not add");

        let mut style = make_style();
        manager.apply_to("Button", None, None, None, &mut style).expect("apply_to should succeed");

        assert_eq!(
            style.background_color,
            Some(Color::rgb(0, 255, 0)),
            "should use the latest registered CSS for the same name"
        );
    }

    #[test]
    fn stylesheet_class_selector_match() {
        let mut manager = StyleSheetManager::new();
        let css = r#"
            .primary {
                background-color: #ff8800;
            }
        "#;
        manager.register("class-test", css, 0);

        let mut style = make_style();
        manager
            .apply_to("Button", Some("primary"), None, None, &mut style)
            .expect("apply_to should succeed");

        assert_eq!(
            style.background_color,
            Some(Color::rgb(0xff, 0x88, 0x00)),
            "class selector .primary should match"
        );
    }

    #[test]
    fn stylesheet_no_match_does_not_apply() {
        let mut manager = StyleSheetManager::new();
        let css = r#"
            Label {
                background-color: #ff0000;
            }
        "#;
        manager.register("no-match", css, 0);

        let mut style = make_style();
        manager.apply_to("Button", None, None, None, &mut style).expect("apply_to should succeed");

        assert_eq!(style.background_color, None, "Label CSS should not apply to Button");
    }

    /// A malformed sheet is reported but must not cancel the valid sheets registered next to it.
    #[test]
    fn a_broken_sheet_does_not_discard_the_valid_sheets() {
        let mut manager = StyleSheetManager::new();
        manager.register("bad", "Button { background-color: #ff0000; ", 0);
        manager.register("good", "Button { text-color: #123456; }", 1);

        let mut style = make_style();
        let result = manager.apply_to("Button", None, None, None, &mut style);

        assert!(result.is_err(), "the malformed sheet must still be reported");
        assert_eq!(
            style.text_color,
            Some(Color::rgb(0x12, 0x34, 0x56)),
            "the valid sheet must still apply"
        );
    }

    /// The CSS cascade applies the more specific rule last, even when it is written first.
    #[test]
    fn a_class_rule_overrides_a_kind_rule_written_after_it() {
        let mut manager = StyleSheetManager::new();
        manager.register(
            "cascade",
            ".primary { background-color: #ff8800; } Button { background-color: #0000ff; }",
            0,
        );

        let mut style = make_style();
        manager
            .apply_to("Button", Some("primary"), None, None, &mut style)
            .expect("apply_to should succeed");

        assert_eq!(
            style.background_color,
            Some(Color::rgb(0xff, 0x88, 0x00)),
            "`.primary` has higher specificity than `Button` and must win"
        );
    }

    #[test]
    fn stylesheet_invalid_css_returns_error() {
        let mut manager = StyleSheetManager::new();
        // Missing closing brace — invalid CSS.
        manager.register("bad", "Button { background-color: #ff0000; ", 0);

        let mut style = make_style();
        let result = manager.apply_to("Button", None, None, None, &mut style);
        assert!(result.is_err(), "invalid CSS should produce an error");
    }

    /// A descendant selector matches through the chain, while the chain-less entry point
    /// deliberately does not.
    #[test]
    fn a_descendant_rule_needs_the_ancestor_chain() {
        use crate::style::selector::ElementRef;
        use crate::widget::WidgetKind;

        let mut manager = StyleSheetManager::new();
        manager.register("nav", "Panel Button { background-color: #010203; }", 0);
        manager.register("direct", "Panel > Button { text-color: #040506; }", 0);

        let button = ElementRef { kind: WidgetKind::Button, class: None, id: None, state: None };
        let panel = ElementRef { kind: WidgetKind::Panel, ..button };
        let label = ElementRef { kind: WidgetKind::Label, ..button };

        let mut style = make_style();
        manager
            .apply_to_context(button, &[panel, label], &mut style)
            .expect("apply_to_context should succeed");
        assert_eq!(
            style.background_color,
            Some(Color::rgb(1, 2, 3)),
            "the panel is an ancestor at depth 2"
        );
        assert_eq!(
            style.text_color, None,
            "the child combinator must not match with a label between panel and button"
        );

        let mut immediate = make_style();
        manager
            .apply_to_context(button, &[panel], &mut immediate)
            .expect("apply_to_context should succeed");
        assert_eq!(
            immediate.text_color,
            Some(Color::rgb(4, 5, 6)),
            "with the panel as the immediate parent the child rule applies"
        );

        let mut chain_less = make_style();
        manager
            .apply_to("Button", None, None, None, &mut chain_less)
            .expect("apply_to should succeed");
        assert_eq!(
            chain_less.background_color, None,
            "without a chain a combinator rule matches nothing"
        );
    }

    /// Inline declarations win over every sheet, regardless of specificity.
    #[test]
    fn inline_declarations_override_the_sheet() {
        use crate::style::selector::ElementRef;
        use crate::widget::WidgetKind;

        let mut manager = StyleSheetManager::new();
        manager.register("id-rule", "#main { background-color: #ff0000; }", 0);
        let element =
            ElementRef { kind: WidgetKind::Button, class: None, id: Some("main"), state: None };

        let mut style = make_style();
        manager
            .apply_to_context_with_inline(
                element,
                &[],
                Some("background-color: #0000ff;"),
                &mut style,
            )
            .expect("inline apply should succeed");
        assert_eq!(
            style.background_color,
            Some(Color::rgb(0, 0, 255)),
            "the element's own declaration is the last layer"
        );
    }

    /// G-162: the sheet's own index is consulted before matching, and it must never hide a rule
    /// the linear scan would have found.
    #[test]
    fn the_registered_index_never_hides_a_matching_rule() {
        let mut manager = StyleSheetManager::new();
        manager.register(
            "index",
            "* { opacity: 0.5; } Button.primary { border-width: 3px; } \
             #go { border-radius: 4px; } Button:hover { border-width: 5px; }",
            0,
        );

        let mut style = make_style();
        manager
            .apply_to("Button", Some("primary"), Some("go"), None, &mut style)
            .expect("apply should succeed");
        assert_eq!(style.opacity, Some(0.5), "the universal rule is filed under `Any`");
        assert_eq!(style.border_width, Some(3), "the compound is filed under its class");
        assert_eq!(style.border_radius, Some(4), "the id rule is filed under its id");

        let mut hovered = make_style();
        manager
            .apply_to(
                "Button",
                Some("primary"),
                Some("go"),
                Some(crate::style::PseudoState::Hover),
                &mut hovered,
            )
            .expect("apply should succeed");
        assert_eq!(
            hovered.border_width,
            Some(3),
            "the state rule is filed where the element can find it; the compound still outranks it"
        );
    }

    /// G-164: the registry hands the trace the rules it already parsed, and that trace names the
    /// same winner the apply produced.
    #[test]
    fn traced_sheets_trace_the_rules_the_apply_uses() {
        use crate::style::css_trace::trace_cascade_prepared;

        let mut manager = StyleSheetManager::new();
        manager.register("s", ".primary { color: #222222; } Button { color: #111111; }", 0);

        let sheets = manager.traced_sheets();
        assert_eq!(sheets.len(), 1);
        let trace = trace_cascade_prepared("Button", Some("primary"), None, None, &sheets);
        assert_eq!(
            trace.origin_of("color").map(|origin| origin.value.as_str()),
            Some("#222222"),
            "the class rule is more specific, and it is written first"
        );

        let mut applied = make_style();
        manager
            .apply_to("Button", Some("primary"), None, None, &mut applied)
            .expect("apply should succeed");
        assert_eq!(applied.text_color, Some(Color::rgba(0x22, 0x22, 0x22, 255)));
    }
}
