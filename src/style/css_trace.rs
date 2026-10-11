// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Where a computed style value came from: the read-only provenance a designer's CSS inspector
//! needs (BLUE26 B26.10).
//!
//! # What problem this solves
//!
//! [`CssParser::parse_and_apply`](crate::style::css::CssParser::parse_and_apply) merges every
//! matching rule into one [`WidgetStyle`](crate::style::WidgetStyle) and keeps the *result*. A CSS
//! inspector in a designer must show the **winner**: for each property, which rule set it, from
//! which sheet, and with what priority — the "why is this value what it is?" question a cascade is
//! impossible to debug without.
//!
//! This module answers that question by walking the *same* rules through the *same* selector match
//! and reporting, per property, the last declaration that won. It reuses
//! [`CssParser::parse_rules`](crate::style::css::CssParser::parse_rules),
//! [`CssParser::parse_selector`](crate::style::css::CssParser::parse_selector) and
//! [`CssSelector::matches`](crate::style::css::CssSelector::matches), so the trace cannot disagree
//! with what was applied — there is one cascade, and this reads it.
//!
//! # Why the trace is separate from the apply
//!
//! Applying is on the paint path and must stay allocation-light; a trace builds a per-property map
//! and is only wanted when a human is looking at the inspector. Keeping them apart means the
//! normal path pays nothing for a facility it never uses, while the two still describe one cascade
//! rather than two that could drift.
//!
//! # What stays in the host
//!
//! The CSS editor, three-way text merge and the model transaction are the host's — this is a
//! **read-only** seam. It reports; it does not edit a stylesheet.
//!
//! # Invalid rules are reported, not silently traced
//!
//! A declaration the apply cannot act on is **not** reported as the winner: an unknown property is
//! ignored by the apply (CSS's own rule) and a refused value leaves the previous value in place, so
//! reporting either as the computed origin would describe a style the control does not have. Both
//! become [`TraceNote`]s carrying the parser's own reason, which is what a designer's inspector
//! shows when a rule has no effect (`BLUE26` B26.10's "支持无效规则错误").

use crate::compat::{format, String, Vec};
use crate::style::css::{CssDeclaration, CssParser};

/// One sheet's contribution to a trace, as the caller described it.
///
/// The name and priority are supplied by the caller rather than read from the global manager, so a
/// designer tracing an *unsaved* editor buffer gets the same facility as one tracing a registered
/// sheet — and so this function has no global state and is trivially testable.
#[derive(Debug, Clone, Copy)]
pub struct TracedSheet<'a> {
    /// The sheet's name, for the report.
    pub name: &'a str,
    /// The sheet's CSS text.
    pub css: &'a str,
    /// The sheet's priority; higher wins, and equal priorities resolve by later registration.
    pub priority: u8,
}

/// Where one computed property value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedOrigin {
    /// The property name, as the winning declaration spelled it.
    pub property: String,
    /// The raw value text of the winning declaration.
    pub value: String,
    /// The selector that matched, as written in the sheet.
    pub selector: String,
    /// The name of the sheet the winning rule came from.
    pub sheet: String,
    /// That sheet's priority.
    pub priority: u8,
}

impl ComputedOrigin {
    /// A one-line rendering, for an inspector tooltip or a log.
    pub fn describe(&self) -> String {
        format!(
            "{}: {} (from `{}` in sheet `{}`, priority {})",
            self.property, self.value, self.selector, self.sheet, self.priority
        )
    }
}

/// Why a traced declaration was not applied, when it could not be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceNote {
    /// A rule's selector did not parse, so the rule was skipped.
    ///
    /// Reported rather than silently dropped: a rule that does not match because its selector is
    /// malformed looks identical to one that genuinely does not match, and only one of those is the
    /// author's mistake.
    UnparsedSelector {
        /// The selector text as written.
        selector: String,
        /// The sheet it came from.
        sheet: String,
    },
    /// A declaration whose property this build does not act on.
    ///
    /// The apply ignores an unknown property (CSS's own rule), so a trace that reported it as the
    /// winning value would disagree with the runtime: the inspector would show a value the control
    /// does not hold. Naming it, with its value, is what lets a designer mark the line as inert.
    UnknownProperty {
        /// The property as written.
        property: String,
        /// The value that was attached to it.
        value: String,
        /// The sheet it came from.
        sheet: String,
    },
    /// A declaration whose value the parser refused.
    ///
    /// The apply returns this error and leaves the previous value in place, so the declaration is
    /// not the winner and must not be reported as one. The reason is carried verbatim from the
    /// parser, which is the only component that knows why the value is unacceptable.
    InvalidValue {
        /// The property as written.
        property: String,
        /// The refused value.
        value: String,
        /// The parser's own reason.
        reason: String,
        /// The sheet it came from.
        sheet: String,
    },
}

/// The result of tracing a cascade: the winning origin per property, plus any notes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyleTrace {
    /// The winners, sorted by property name for a stable report.
    pub origins: Vec<ComputedOrigin>,
    /// Rules that could not be considered, with the reason.
    pub notes: Vec<TraceNote>,
}

impl StyleTrace {
    /// The origin of one property's computed value, if a rule set it.
    pub fn origin_of(&self, property: &str) -> Option<&ComputedOrigin> {
        self.origins.iter().find(|origin| origin.property == property)
    }

    /// Whether any rule set the property.
    pub fn has(&self, property: &str) -> bool {
        self.origin_of(property).is_some()
    }
}

/// Traces which rule wins each property for a widget, across `sheets` in priority order.
///
/// # Ordering, and why it is the cascade's own
///
/// Sheets are applied in ascending priority, and within a sheet rules apply in source order, so the
/// **last** declaration for a property is the winner — exactly the order
/// [`StyleSheetManager::apply_to`](crate::style::stylesheet::StyleSheetManager::apply_to) uses.
/// Tracing in a different order would report a winner the apply did not choose, which is the one
/// way a provenance tool can be worse than none.
///
/// A later sheet with a **lower** priority is still applied after an earlier higher-priority one
/// only when its priority is higher; the sort below is stable, so two sheets at equal priority
/// resolve by the order the caller supplied them — matching registration order.
pub fn trace_cascade(
    kind: &str,
    class: Option<&str>,
    id: Option<&str>,
    state: Option<crate::style::selector::PseudoState>,
    sheets: &[TracedSheet<'_>],
) -> StyleTrace {
    // Sort by priority ascending, stably, so equal priorities keep caller order.
    let mut ordered: Vec<&TracedSheet<'_>> = sheets.iter().collect();
    ordered.sort_by_key(|sheet| sheet.priority);

    let mut origins: Vec<ComputedOrigin> = Vec::new();
    let mut notes: Vec<TraceNote> = Vec::new();

    for sheet in ordered {
        let Ok(rules) = CssParser::parse_rules(sheet.css) else {
            continue;
        };
        for rule in rules {
            let Some(selector) = CssParser::parse_selector(&rule.selector_text) else {
                notes.push(TraceNote::UnparsedSelector {
                    selector: rule.selector_text.clone(),
                    sheet: String::from(sheet.name),
                });
                continue;
            };
            if !selector.matches(kind, class, id, state) {
                continue;
            }
            for declaration in &rule.declarations {
                // A declaration only wins if the apply could act on it. The two ways it cannot are
                // reported rather than recorded: an unknown property is ignored by the apply
                // (CSS's own rule) and a refused value leaves the previous value in place, so
                // either one reported as the winner would make this trace describe a style the
                // control does not have — the one failure a provenance tool cannot afford.
                if !CssParser::is_known_property(&declaration.property) {
                    notes.push(TraceNote::UnknownProperty {
                        property: declaration.property.clone(),
                        value: declaration.value.clone(),
                        sheet: String::from(sheet.name),
                    });
                    continue;
                }
                // The value is judged by the parser's own applier on a scratch style, so the
                // verdict is the apply's and not a second rule set's. The scratch style is
                // discarded: this asks "would this apply?", not "to what?".
                let mut scratch = crate::style::WidgetStyle::default();
                if let Err(reason) =
                    CssParser::apply_declarations(core::slice::from_ref(declaration), &mut scratch)
                {
                    notes.push(TraceNote::InvalidValue {
                        property: declaration.property.clone(),
                        value: declaration.value.clone(),
                        reason,
                        sheet: String::from(sheet.name),
                    });
                    continue;
                }
                record(&mut origins, declaration, &rule.selector_text, sheet);
            }
        }
    }

    origins.sort_by(|left, right| left.property.cmp(&right.property));
    notes.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
    StyleTrace { origins, notes }
}

/// Records `declaration` as the current winner of its property.
///
/// A later declaration always wins, because the caller walks rules in cascade order — the same
/// "last write wins" the merge applies. Keeping the winner rather than the first is what makes the
/// trace describe the *cascade* and not merely the first match.
fn record(
    origins: &mut Vec<ComputedOrigin>,
    declaration: &CssDeclaration,
    selector_text: &str,
    sheet: &TracedSheet<'_>,
) {
    let origin = ComputedOrigin {
        property: declaration.property.clone(),
        value: declaration.value.clone(),
        selector: String::from(selector_text),
        sheet: String::from(sheet.name),
        priority: sheet.priority,
    };
    if let Some(existing) = origins.iter_mut().find(|o| o.property == origin.property) {
        *existing = origin;
    } else {
        origins.push(origin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A single matching rule's value is reported with its rule and sheet.
    #[test]
    fn a_matching_rule_reports_its_origin() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet {
                name: "base",
                css: "Button { background-color: #ff0000; }",
                priority: 0,
            }],
        );
        let origin = trace.origin_of("background-color").expect("the rule set the property");
        assert_eq!(origin.value, "#ff0000");
        assert_eq!(origin.selector, "Button");
        assert_eq!(origin.sheet, "base");
        assert_eq!(origin.priority, 0);
    }

    /// A higher-priority sheet's declaration must win over a lower one.
    #[test]
    fn a_higher_priority_sheet_wins() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[
                TracedSheet { name: "base", css: "Button { color: #111111; }", priority: 0 },
                TracedSheet { name: "theme", css: "Button { color: #222222; }", priority: 5 },
            ],
        );
        let origin = trace.origin_of("color").expect("a rule set the property");
        assert_eq!(origin.value, "#222222", "the higher-priority sheet's value must win");
        assert_eq!(origin.sheet, "theme");
    }

    /// The trace must agree with the apply: the winner it names is the value the apply produces.
    ///
    /// This is the property that makes the trace trustworthy — a provenance tool that disagrees
    /// with the cascade is worse than none.
    #[test]
    fn the_trace_agrees_with_the_applied_value() {
        use crate::style::WidgetStyle;
        let css = "Button { background-color: #abcdef; }";
        let mut applied = WidgetStyle::default();
        CssParser::apply_declaration_text("background-color: #abcdef", &mut applied)
            .expect("a known property applies");
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet { name: "s", css, priority: 0 }],
        );
        assert_eq!(applied.background_color, Some(crate::core::Color::rgb(0xab, 0xcd, 0xef)));
        assert_eq!(trace.origin_of("background-color").expect("traced").value, "#abcdef");
    }

    /// A rule whose selector does not parse must be reported, not silently skipped.
    #[test]
    fn an_unparsed_selector_is_noted() {
        // A bare `:` is not a valid selector this parser accepts.
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet { name: "s", css: ": { color: #111111; }", priority: 0 }],
        );
        assert!(trace.origins.is_empty());
        assert_eq!(trace.notes.len(), 1);
        assert!(matches!(trace.notes[0], TraceNote::UnparsedSelector { .. }));
    }

    /// A rule for another kind must not appear, but a rule that matches must.
    #[test]
    fn only_matching_rules_are_traced() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet {
                name: "s",
                css: "Label { color: #111111; } Button { color: #222222; }",
                priority: 0,
            }],
        );
        assert_eq!(trace.origins.len(), 1);
        assert_eq!(trace.origin_of("color").expect("traced").value, "#222222");
    }

    /// Within one rule, a property written twice must report the **last** value — the cascade's own
    /// "last write wins".
    #[test]
    fn the_last_declaration_of_a_property_wins() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet {
                name: "s",
                css: "Button { color: #111111; color: #222222; }",
                priority: 0,
            }],
        );
        assert_eq!(trace.origin_of("color").expect("traced").value, "#222222");
    }

    /// The origins must be sorted, so a report is stable.
    #[test]
    fn the_origins_are_sorted_by_property() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet {
                name: "s",
                css: "Button { text-color: #ffffff; background-color: #000000; }",
                priority: 0,
            }],
        );
        let names: Vec<&str> = trace.origins.iter().map(|o| o.property.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "the origins must be sorted for a stable report");
    }

    /// A declaration with an unknown property must be **noted**, not reported as the winner: the
    /// apply ignores the property, so a trace that called it the winner would disagree with the
    /// style the control actually holds.
    #[test]
    fn an_unknown_property_is_noted_and_not_a_winner() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet {
                name: "s",
                css: "Button { text-sahdow: 1px; color: #111111; }",
                priority: 0,
            }],
        );
        assert!(!trace.has("text-sahdow"), "an ignored property must not be reported as applied");
        assert!(trace.has("color"), "the valid declaration still wins");
        assert_eq!(trace.notes.len(), 1, "{:?}", trace.notes);
        match &trace.notes[0] {
            TraceNote::UnknownProperty { property, value, sheet } => {
                assert_eq!(property, "text-sahdow");
                assert_eq!(value, "1px");
                assert_eq!(sheet, "s");
            }
            other => panic!("expected an unknown-property note, got {other:?}"),
        }
    }

    /// A declaration whose value the parser refuses must be noted with the parser's own reason, and
    /// must not displace a value an earlier rule set successfully.
    #[test]
    fn a_refused_value_is_noted_with_its_reason_and_loses_to_a_valid_one() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet {
                name: "s",
                css: "Button { color: #111111; color: notacolour; }",
                priority: 0,
            }],
        );
        assert_eq!(
            trace.origin_of("color").expect("a rule set the property").value,
            "#111111",
            "the refused declaration must not win"
        );
        assert_eq!(trace.notes.len(), 1, "{:?}", trace.notes);
        match &trace.notes[0] {
            TraceNote::InvalidValue { property, value, reason, sheet } => {
                assert_eq!(property, "color");
                assert_eq!(value, "notacolour");
                assert_eq!(sheet, "s");
                assert!(reason.contains("color"), "the parser's reason must travel: {reason}");
            }
            other => panic!("expected an invalid-value note, got {other:?}"),
        }
    }

    /// The notes must be sorted, so a report is stable across runs.
    #[test]
    fn the_notes_are_deterministically_ordered() {
        let sheets = [TracedSheet {
            name: "s",
            css: "Button { aaa-property: 1; zzz-property: 2; }",
            priority: 0,
        }];
        let first = trace_cascade("Button", None, None, None, &sheets);
        let second = trace_cascade("Button", None, None, None, &sheets);
        assert_eq!(first.notes, second.notes, "two runs must render the same report");
    }

    /// The description must name the sheet and the value, so a tooltip is useful.
    #[test]
    fn describe_names_the_source() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet { name: "theme", css: "Button { color: #123456; }", priority: 3 }],
        );
        let rendered = trace.origin_of("color").expect("traced").describe();
        assert!(rendered.contains("#123456"), "{rendered}");
        assert!(rendered.contains("theme"), "{rendered}");
    }
}
