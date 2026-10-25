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
//! [`CssParser::parse_prepared_rules`](crate::style::css::CssParser::parse_prepared_rules) and
//! [`CssSelector::matches`](crate::style::css::CssSelector::matches), so the trace cannot disagree
//! with what was applied — there is one cascade, and this reads it.
//!
//! # Parse once, match many (G-164)
//!
//! A registered sheet has already parsed its rules, so a trace over it must not re-expand and
//! re-parse the CSS text. [`trace_cascade_prepared`] takes the parsed rule sets and is the entry
//! point for that case; [`trace_cascade`] is the on-demand form for an editor buffer that has no
//! registered copy yet, and it parses each sheet exactly once before delegating to the same walk.
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
use crate::style::css::{CssDeclaration, CssParser, ParsedCssRule};

/// One sheet's contribution to a trace, as the caller described it.
///
/// The name and priority are supplied by the caller rather than read from the global manager, so a
/// designer tracing an *unsaved* editor buffer gets the same facility as one tracing a registered
/// sheet — and so this function has no global state and is trivially testable.
///
/// A caller that already holds parsed rules (a registered sheet) should use
/// [`TracedRuleSheet`] instead, which skips the parse entirely (G-164).
#[derive(Debug, Clone, Copy)]
pub struct TracedSheet<'a> {
    /// The sheet's name, for the report.
    pub name: &'a str,
    /// The sheet's CSS text.
    pub css: &'a str,
    /// The sheet's priority; higher wins, and equal priorities resolve by later registration.
    pub priority: u8,
}

/// One sheet's **already parsed** rules, for a trace that must not re-parse (G-164).
///
/// [`StyleSheetManager::traced_sheets`](crate::style::stylesheet::StyleSheetManager::traced_sheets)
/// produces these from the registry, so tracing an application's real stylesheets costs a selector
/// match per rule and no tokenising at all.
#[derive(Debug, Clone, Copy)]
pub struct TracedRuleSheet<'a> {
    /// The sheet's name, for the report.
    pub name: &'a str,
    /// The sheet's priority; higher wins, and equal priorities resolve by caller order.
    pub priority: u8,
    /// The sheet's rules, with their selectors already parsed.
    pub rules: &'a [ParsedCssRule],
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
/// The winner of a property is the last declaration written, so this walks the rules in exactly
/// the order [`StyleSheetManager::apply_to`](crate::style::stylesheet::StyleSheetManager::apply_to)
/// applies them: ascending **specificity**, then sheet priority, then source position. Tracing in a
/// different order would report a winner the apply did not choose, which is the one way a
/// provenance tool can be worse than none.
///
/// Sheets are ordered stably by priority first, so two sheets at equal priority resolve by the
/// order the caller supplied them — matching registration order.
pub fn trace_cascade(
    kind: &str,
    class: Option<&str>,
    id: Option<&str>,
    state: Option<crate::style::selector::PseudoState>,
    sheets: &[TracedSheet<'_>],
) -> StyleTrace {
    // Each sheet is parsed exactly once, then the walk is the shared prepared path. A sheet whose
    // text does not parse contributes nothing, which is what the previous per-sheet `continue`
    // did; the parse error itself has no note variant because it is a property of the whole
    // sheet rather than of one declaration.
    let mut parsed: Vec<(&str, u8, Vec<ParsedCssRule>)> = Vec::new();
    for sheet in sheets {
        let Ok(rules) = CssParser::parse_prepared_rules(sheet.css) else {
            continue;
        };
        parsed.push((sheet.name, sheet.priority, rules));
    }
    let prepared: Vec<TracedRuleSheet<'_>> = parsed
        .iter()
        .map(|(name, priority, rules)| TracedRuleSheet {
            name,
            priority: *priority,
            rules: rules.as_slice(),
        })
        .collect();
    trace_cascade_prepared(kind, class, id, state, &prepared)
}

/// [`trace_cascade`] over rules that have already been parsed (G-164).
///
/// This is the single cascade walk both entry points use: rules are ordered exactly as
/// [`StyleSheetManager::apply_to`](crate::style::stylesheet::StyleSheetManager::apply_to) orders
/// them — ascending specificity, then sheet priority, then source position — so the **last**
/// declaration written for a property is the one the apply left in place.
pub fn trace_cascade_prepared(
    kind: &str,
    class: Option<&str>,
    id: Option<&str>,
    state: Option<crate::style::selector::PseudoState>,
    sheets: &[TracedRuleSheet<'_>],
) -> StyleTrace {
    // Sort by priority ascending, stably, so equal priorities keep caller order.
    let mut ordered: Vec<&TracedRuleSheet<'_>> = sheets.iter().collect();
    ordered.sort_by_key(|sheet| sheet.priority);

    let mut origins: Vec<ComputedOrigin> = Vec::new();
    let mut notes: Vec<TraceNote> = Vec::new();

    // Selectors that did not parse are reported whatever the element looks like: the author's
    // mistake is independent of whether the rule would have matched.
    for sheet in &ordered {
        for rule in sheet.rules {
            if rule.selector.is_none() {
                notes.push(TraceNote::UnparsedSelector {
                    selector: rule.selector_text.clone(),
                    sheet: String::from(sheet.name),
                });
            }
        }
    }

    // The apply sorts its matched set by `(specificity, sheet order, rule position)` and writes
    // declarations in that order, so the winner is the last write. Tracing in source order
    // instead would name a rule the apply did not choose whenever a more specific rule appears
    // **before** a less specific one — the one way a provenance tool can be worse than none.
    let mut matched: Vec<(u32, usize, usize, &ParsedCssRule, &str, u8)> = Vec::new();
    for (sheet_order, sheet) in ordered.iter().enumerate() {
        for (position, rule) in sheet.rules.iter().enumerate() {
            let Some(selector) = &rule.selector else {
                continue;
            };
            if selector.matches(kind, class, id, state) {
                matched.push((
                    rule.specificity,
                    sheet_order,
                    position,
                    rule,
                    sheet.name,
                    sheet.priority,
                ));
            }
        }
    }
    matched.sort_by_key(|(specificity, sheet_order, position, ..)| {
        (*specificity, *sheet_order, *position)
    });

    for (_, _, _, rule, sheet_name, priority) in matched {
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
                    sheet: String::from(sheet_name),
                });
                continue;
            }
            // The value is judged by the parser's own arms through a pure parse: the verdict is
            // the apply's, no scratch `WidgetStyle` is built, and nothing is written (G-164). The
            // scratch-style approach this replaces constructed a style per declaration just to
            // throw it away.
            if let Err(reason) = CssParser::validate_declaration(declaration) {
                notes.push(TraceNote::InvalidValue {
                    property: declaration.property.clone(),
                    value: declaration.value.clone(),
                    reason,
                    sheet: String::from(sheet_name),
                });
                continue;
            }
            record(&mut origins, declaration, &rule.selector_text, sheet_name, priority);
        }
    }

    origins.sort_by(|left, right| left.property.cmp(&right.property));
    // A stable key computed from borrowed fields, so ordering a report performs no formatting and
    // no allocation per comparison (G-164). The previous `format!("{note:?}")` allocated twice
    // per comparison and made the ordering depend on `Debug` output.
    notes.sort_by(|left, right| note_sort_key(left).cmp(&note_sort_key(right)));
    StyleTrace { origins, notes }
}

/// The stable, allocation-free sort key for a note (G-164).
///
/// Ordered by variant first so the three kinds form contiguous groups, then by the fields that
/// identify the declaration.
fn note_sort_key(note: &TraceNote) -> (u8, &str, &str, &str, &str) {
    match note {
        TraceNote::UnparsedSelector { selector, sheet } => {
            (0, selector.as_str(), "", sheet.as_str(), "")
        }
        TraceNote::UnknownProperty { property, value, sheet } => {
            (1, property.as_str(), value.as_str(), sheet.as_str(), "")
        }
        TraceNote::InvalidValue { property, value, reason, sheet } => {
            (2, property.as_str(), value.as_str(), sheet.as_str(), reason.as_str())
        }
    }
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
    sheet_name: &str,
    priority: u8,
) {
    let origin = ComputedOrigin {
        property: declaration.property.clone(),
        value: declaration.value.clone(),
        selector: String::from(selector_text),
        sheet: String::from(sheet_name),
        priority,
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

    /// G-164: tracing rules that were already parsed produces exactly the report the text entry
    /// point produces, without a second parse.
    #[test]
    fn the_prepared_entry_point_traces_the_same_winner() {
        let css = "Button { color: #111111; } Button:hover { color: #222222; }";
        let parsed = CssParser::parse_prepared_rules(css).expect("the sheet parses");
        let prepared = [TracedRuleSheet { name: "s", priority: 0, rules: &parsed }];

        let from_prepared = trace_cascade_prepared(
            "Button",
            None,
            None,
            Some(crate::style::selector::PseudoState::Hover),
            &prepared,
        );
        let from_text = trace_cascade(
            "Button",
            None,
            None,
            Some(crate::style::selector::PseudoState::Hover),
            &[TracedSheet { name: "s", css, priority: 0 }],
        );
        assert_eq!(from_prepared, from_text);
        assert_eq!(
            from_prepared.origin_of("color").expect("traced").value,
            "#222222",
            "the state rule is later in source order and more specific"
        );
    }

    /// The trace must honour specificity, not merely source order: a more specific rule written
    /// **first** is the one the apply leaves in place, so it is the one the trace must name.
    #[test]
    fn a_more_specific_rule_wins_even_when_it_is_written_first() {
        let trace = trace_cascade(
            "Button",
            Some("primary"),
            None,
            None,
            &[TracedSheet {
                name: "s",
                css: ".primary { color: #222222; } Button { color: #111111; }",
                priority: 0,
            }],
        );
        assert_eq!(
            trace.origin_of("color").expect("traced").value,
            "#222222",
            "specificity, not source order, decides the winner"
        );
    }

    /// G-404: the two properties the known list claimed but the apply dropped are now winners,
    /// and the trace reports them rather than silently omitting them.
    #[test]
    fn the_previously_unimplemented_properties_are_traced_as_winners() {
        let trace = trace_cascade(
            "Button",
            None,
            None,
            None,
            &[TracedSheet {
                name: "s",
                css: "Button { border-style: dashed; font-weight: 700; }",
                priority: 0,
            }],
        );
        assert!(
            trace.notes.is_empty(),
            "neither property is inert any more, so neither is a note: {:?}",
            trace.notes
        );
        assert_eq!(
            trace.origin_of("border-style").map(|origin| origin.value.as_str()),
            Some("dashed")
        );
        assert_eq!(trace.origin_of("font-weight").map(|origin| origin.value.as_str()), Some("700"));
    }
}
