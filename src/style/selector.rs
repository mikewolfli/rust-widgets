// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CSS-style selector engine for widget styling.
//!
//! Allows matching widgets by kind, class name, ID, and state,
//! enabling CSS-like cascading style sheets for the widget system.

use crate::compat::{Box, HashMap, String, Vec};
use crate::widget::WidgetKind;

/// The narrowing key a rule is stored under.
///
/// Matching starts from the element's own tag/class/id and only looks at rules filed under one of
/// those (or under `Any`), instead of scanning every rule in the sheet (G-22).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum SelectorIndexKey {
    Kind(WidgetKind),
    Class(String),
    Id(String),
    /// A selector with no tag/class/id constraint: universal, state-only, or a compound whose
    /// only constraints are those.
    Any,
}

/// The most selective simple selector in `selector` that the styled element itself must satisfy.
///
/// For a combinator only the **subject** (right-hand compound) is a fact about the element, so the
/// ancestor side never narrows the index. For an `And`, any one constituent is enough: an element
/// that matches the compound must satisfy every constituent, so filing under one of them cannot
/// hide the rule from a matching element.
fn selector_index_key(selector: &Selector) -> SelectorIndexKey {
    match selector {
        Selector::Kind(kind) => SelectorIndexKey::Kind(*kind),
        Selector::Class(class) => SelectorIndexKey::Class(class.clone()),
        Selector::Id(id) => SelectorIndexKey::Id(id.clone()),
        Selector::State(_) | Selector::Universal => SelectorIndexKey::Any,
        Selector::And(parts) => {
            if let Some(id) = parts.iter().find_map(|part| match part {
                Selector::Id(id) => Some(id.clone()),
                _ => None,
            }) {
                return SelectorIndexKey::Id(id);
            }
            if let Some(class) = parts.iter().find_map(|part| match part {
                Selector::Class(class) => Some(class.clone()),
                _ => None,
            }) {
                return SelectorIndexKey::Class(class);
            }
            if let Some(kind) = parts.iter().find_map(|part| match part {
                Selector::Kind(kind) => Some(*kind),
                _ => None,
            }) {
                return SelectorIndexKey::Kind(kind);
            }
            SelectorIndexKey::Any
        }
        Selector::Descendant { subject, .. } | Selector::Child { subject, .. } => {
            selector_index_key(subject)
        }
    }
}

/// A CSS-style selector that can match widgets by kind, class, ID, and state.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Selector {
    /// Match by widget kind (e.g., `Button`, `Label`).
    Kind(WidgetKind),
    /// Match by CSS class name (e.g., `.my-class`).
    Class(String),
    /// Match by widget ID (e.g., `#my-id`).
    Id(String),
    /// Match a widget that has a specific state (e.g., `:hover`, `:disabled`).
    State(PseudoState),
    /// Match any widget (universal selector `*`).
    #[default]
    Universal,
    /// AND-combination of multiple selectors (e.g., `Button.primary:hover`).
    And(Vec<Selector>),
    /// A descendant combinator: the subject matches the element and `ancestor` matches any of
    /// its ancestors, at any depth.
    ///
    /// The subject is the **right-hand** compound in the source (`Panel Button` selects the
    /// button), which is why the two sides are named `ancestor`/`subject` rather than left/right:
    /// a reader of a selector chain asks "which element gets styled", and that is always the
    /// subject.
    Descendant {
        /// The selector an ancestor must satisfy.
        ancestor: Box<Selector>,
        /// The selector the styled element itself must satisfy.
        subject: Box<Selector>,
    },
    /// A child combinator: the subject matches the element and `parent` matches its immediate
    /// parent only (`Panel > Button`).
    Child {
        /// The selector the immediate parent must satisfy.
        parent: Box<Selector>,
        /// The selector the styled element itself must satisfy.
        subject: Box<Selector>,
    },
}

/// Widget pseudo-states that can be matched by CSS selectors.
///
/// These represent dynamic widget states that selectors can target
/// (e.g., `:hover`, `:disabled`). Distinct from `theme_state::WidgetState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoState {
    /// The widget's resting state, with no interaction modifiers active.
    /// Selectors targeting `:normal` match only when no other state is set.
    Normal,
    /// The pointer is over the widget.
    Hover,
    /// The widget is being held down (or otherwise in an active press).
    Pressed,
    /// The widget refuses input.
    Disabled,
    /// The widget holds keyboard focus.
    Focused,
    /// The widget is checked, in the sense of a toggle, checkbox, or radio
    /// button.
    Checked,
    /// The widget (or one of its items) is selected, as in a list or tab.
    Selected,
    /// The widget is displaying a validation error.
    ///
    /// # Why the CSS path needs the three validation states
    ///
    /// `WidgetState` has carried `Error`/`Warning`/`Success` — and the theme's override keys have
    /// accepted `"<kind>:error"` — for as long as those states have existed. The **CSS** path could
    /// not spell them: `parse_pseudo_state` understood six names and none of the three, so a sheet
    /// targeting `line_edit:error` was not merely unsupported, it was silently parsed as something
    /// else. Inline validation is the most common state a form has, and it was the one state a
    /// stylesheet could not reach.
    Error,
    /// The widget is displaying a non-fatal warning.
    Warning,
    /// The widget is confirming a completed action.
    Success,
}

/// A complete CSS rule: selector → style overrides.
#[derive(Debug, Clone)]
pub struct StyleRule {
    /// The selector that determines which widgets this rule applies to.
    pub selector: Selector,
    /// A human-readable name for debugging.
    pub name: String,
    /// Specificity weight for cascade ordering (higher = more specific).
    pub specificity: u32,
}

impl StyleRule {
    /// Creates a rule and derives its specificity from `selector`.
    ///
    /// The specificity field is **computed here**, not supplied: changing
    /// `selector` afterwards leaves `specificity` describing the old selector,
    /// so mutate it through this constructor rather than in place.
    pub fn new(selector: Selector, name: impl Into<String>) -> Self {
        let name = name.into();
        let specificity = selector.specificity();
        Self { selector, name, specificity }
    }
}

impl Selector {
    /// Compute specificity: kind=1, class=10, id=100, state=1.
    pub fn specificity(&self) -> u32 {
        match self {
            Selector::Kind(_) => 1,
            Selector::Class(_) => 10,
            Selector::Id(_) => 100,
            Selector::State(_) => 1,
            Selector::Universal => 0,
            Selector::And(selectors) => selectors.iter().map(|s| s.specificity()).sum(),
            // A combinator adds no weight of its own in the CSS cascade: specificity is the sum
            // of the compounds' weights, and the combinator only describes how they are related.
            Selector::Descendant { ancestor, subject }
            | Selector::Child { parent: ancestor, subject } => {
                ancestor.specificity() + subject.specificity()
            }
        }
    }

    /// Check if this selector matches a widget with the given properties.
    ///
    /// `class` and `id` are matched by exact string equality, not by CSS
    /// substring or prefix rules, and a widget can carry at most one of each.
    /// `state` must be `Some` for a [`Selector::State`] to match, so a caller
    /// that passes `None` will see every state selector fail.
    ///
    /// # Combinators need context
    ///
    /// This signature carries one element, so [`Selector::Descendant`] and [`Selector::Child`]
    /// answer `false` here: without the ancestor chain there is no honest answer. Use
    /// [`Selector::matches_in`] when the caller has the chain; a caller that does not is asking
    /// a question the selector cannot answer, and matching nothing is safer than matching
    /// everything.
    pub fn matches(
        &self,
        kind: WidgetKind,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
    ) -> bool {
        self.matches_in(ElementRef { kind, class, id, state }, &[])
    }

    /// Matches against an element **and its ancestor chain**.
    ///
    /// `ancestors` is ordered root-first (the outermost ancestor at index 0), which is the order
    /// a caller that walks parents upward collects naturally after reversing. For
    /// [`Selector::Descendant`] any ancestor may match; for [`Selector::Child`] only the last
    /// entry (the immediate parent) is consulted.
    pub fn matches_in(&self, element: ElementRef<'_>, ancestors: &[ElementRef<'_>]) -> bool {
        match self {
            Selector::Kind(k) => *k == element.kind,
            Selector::Class(c) => element.class == Some(c.as_str()),
            Selector::Id(i) => element.id == Some(i.as_str()),
            Selector::State(s) => element.state == Some(*s),
            Selector::Universal => true,
            Selector::And(selectors) => selectors.iter().all(|s| s.matches_in(element, ancestors)),
            Selector::Descendant { ancestor, subject } => {
                subject.matches_in(element, ancestors)
                    && ancestors.iter().any(|candidate| ancestor.matches_in(*candidate, &[]))
            }
            Selector::Child { parent, subject } => {
                subject.matches_in(element, ancestors)
                    && ancestors.last().is_some_and(|candidate| parent.matches_in(*candidate, &[]))
            }
        }
    }
}

/// One element's identity, as a selector sees it.
///
/// A borrowed, `Copy` descriptor rather than a widget reference: the selector engine must be
/// usable from the CSS parser's tests without constructing controls, and a combinator chain is
/// then a slice of these values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementRef<'a> {
    /// The widget kind, resolved.
    pub kind: WidgetKind,
    /// The element's class, when it has one.
    pub class: Option<&'a str>,
    /// The element's id, when it has one.
    pub id: Option<&'a str>,
    /// The element's interaction state, when known.
    pub state: Option<PseudoState>,
}

/// A collection of style rules that can be matched against widgets.
#[derive(Debug, Clone)]
pub struct StyleSheet {
    rules: Vec<StyleRule>,
    /// Rule indices by the narrowing key described on [`SelectorIndexKey`]. Rebuilt whenever the
    /// rule list changes; the list is built once at startup, so an add may pay O(n) to keep both
    /// structures in step (G-22).
    index: HashMap<SelectorIndexKey, Vec<usize>>,
}

impl StyleSheet {
    /// Creates a stylesheet with no rules.
    pub fn new() -> Self {
        Self { rules: Vec::new(), index: HashMap::new() }
    }

    /// Adds a rule and keeps the list sorted by ascending specificity.
    ///
    /// Sorting ascending means later, more specific rules appear last, so a
    /// consumer that applies rules in iteration order gets standard cascade
    /// behaviour. The sort is stable, so rules of equal specificity keep their
    /// insertion order. Adding a rule therefore costs a re-sort of the whole
    /// list, which is fine for stylesheets built once at startup.
    pub fn add_rule(&mut self, rule: StyleRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| r.specificity);
        self.rebuild_index();
    }

    /// Rebuilds the inverted index so its stored positions follow the sorted rule list.
    fn rebuild_index(&mut self) {
        self.index.clear();
        for (position, rule) in self.rules.iter().enumerate() {
            self.index.entry(selector_index_key(&rule.selector)).or_default().push(position);
        }
    }

    /// Indices of the rules whose narrowing key can match this element, in cascade order.
    fn candidate_indices(
        &self,
        kind: WidgetKind,
        class: Option<&str>,
        id: Option<&str>,
    ) -> Vec<usize> {
        let mut candidates = Vec::new();
        for key in [
            Some(SelectorIndexKey::Kind(kind)),
            class.map(|class| SelectorIndexKey::Class(String::from(class))),
            id.map(|id| SelectorIndexKey::Id(String::from(id))),
            Some(SelectorIndexKey::Any),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(indices) = self.index.get(&key) {
                candidates.extend_from_slice(indices);
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }

    /// Find all matching rules for a widget, sorted by specificity.
    pub fn match_rules(
        &self,
        kind: WidgetKind,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
    ) -> Vec<&StyleRule> {
        self.candidate_indices(kind, class, id)
            .into_iter()
            .filter_map(|position| self.rules.get(position))
            .filter(|rule| rule.selector.matches(kind, class, id, state))
            .collect()
    }

    /// Find all matching rules for an element **with its ancestor chain**, sorted by
    /// specificity.
    ///
    /// The context-aware counterpart of [`Self::match_rules`]: a rule whose selector uses a
    /// descendant or child combinator can only be answered with the chain, so a caller that has
    /// one should use this entry point.
    pub fn match_rules_in(
        &self,
        element: ElementRef<'_>,
        ancestors: &[ElementRef<'_>],
    ) -> Vec<&StyleRule> {
        self.candidate_indices(element.kind, element.class, element.id)
            .into_iter()
            .filter_map(|position| self.rules.get(position))
            .filter(|rule| rule.selector.matches_in(element, ancestors))
            .collect()
    }

    /// Returns the stored rules in cascade order (ascending specificity;
    /// insertion order within equal specificity).
    pub fn rules(&self) -> &[StyleRule] {
        &self.rules
    }
}

crate::impl_default_via_new!(StyleSheet);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::MiniToString;

    #[test]
    fn selector_universal_matches_anything() {
        assert!(Selector::Universal.matches(WidgetKind::Button, None, None, None));
    }

    #[test]
    fn selector_kind_matches_correct_kind() {
        let s = Selector::Kind(WidgetKind::Button);
        assert!(s.matches(WidgetKind::Button, None, None, None));
        assert!(!s.matches(WidgetKind::Label, None, None, None));
    }

    #[test]
    fn selector_class_matches() {
        let s = Selector::Class("primary".to_string());
        assert!(s.matches(WidgetKind::Button, Some("primary"), None, None));
        assert!(!s.matches(WidgetKind::Button, Some("secondary"), None, None));
        assert!(!s.matches(WidgetKind::Button, None, None, None));
    }

    #[test]
    fn selector_id_matches() {
        let s = Selector::Id("submit-btn".to_string());
        assert!(s.matches(WidgetKind::Button, None, Some("submit-btn"), None));
        assert!(!s.matches(WidgetKind::Button, None, Some("other"), None));
    }

    #[test]
    fn selector_state_matches() {
        let s = Selector::State(PseudoState::Hover);
        assert!(s.matches(WidgetKind::Button, None, None, Some(PseudoState::Hover)));
        assert!(!s.matches(WidgetKind::Button, None, None, Some(PseudoState::Normal)));
    }

    #[test]
    fn selector_and_combines() {
        let s = Selector::And(vec![
            Selector::Kind(WidgetKind::Button),
            Selector::Class("primary".to_string()),
            Selector::State(PseudoState::Hover),
        ]);
        assert!(s.matches(WidgetKind::Button, Some("primary"), None, Some(PseudoState::Hover)));
        assert!(!s.matches(WidgetKind::Button, Some("secondary"), None, Some(PseudoState::Hover)));
        assert!(!s.matches(WidgetKind::Label, Some("primary"), None, Some(PseudoState::Hover)));
    }

    #[test]
    fn specificity_kind_is_1() {
        assert_eq!(Selector::Kind(WidgetKind::Button).specificity(), 1);
    }

    #[test]
    fn specificity_class_is_10() {
        assert_eq!(Selector::Class("btn".to_string()).specificity(), 10);
    }

    #[test]
    fn specificity_id_is_100() {
        assert_eq!(Selector::Id("main".to_string()).specificity(), 100);
    }

    #[test]
    fn specificity_and_sums() {
        let s = Selector::And(vec![
            Selector::Kind(WidgetKind::Button),
            Selector::Class("primary".to_string()),
        ]);
        assert_eq!(s.specificity(), 11);
    }

    #[test]
    fn stylesheet_add_rule_and_match() {
        let mut ss = StyleSheet::new();
        ss.add_rule(StyleRule::new(Selector::Kind(WidgetKind::Button), "button"));
        ss.add_rule(StyleRule::new(Selector::Class("primary".to_string()), "primary"));
        let matched = ss.match_rules(WidgetKind::Button, Some("primary"), None, None);
        assert_eq!(matched.len(), 2);
    }

    /// The inverted index must find class/id/compound/combinator rules just as the old linear
    /// scan did, and still return them in cascade order (G-22).
    #[test]
    fn the_indexed_match_returns_every_rule_the_linear_scan_would() {
        let mut ss = StyleSheet::new();
        ss.add_rule(StyleRule::new(Selector::Universal, "universal"));
        ss.add_rule(StyleRule::new(
            Selector::And(vec![
                Selector::Kind(WidgetKind::Button),
                Selector::Class("primary".to_string()),
            ]),
            "button.primary",
        ));
        ss.add_rule(StyleRule::new(Selector::Id("submit".to_string()), "id"));
        ss.add_rule(StyleRule::new(
            Selector::Descendant {
                ancestor: Box::new(Selector::Kind(WidgetKind::Panel)),
                subject: Box::new(Selector::Class("primary".to_string())),
            },
            "panel primary",
        ));
        ss.add_rule(StyleRule::new(Selector::Kind(WidgetKind::Label), "label"));

        let matched = ss.match_rules(WidgetKind::Button, Some("primary"), Some("submit"), None);
        let names: Vec<&str> = matched.iter().map(|rule| rule.name.as_str()).collect();
        assert_eq!(names, vec!["universal", "button.primary", "id"]);

        let button = ElementRef {
            kind: WidgetKind::Button,
            class: Some("primary"),
            id: Some("submit"),
            state: None,
        };
        let panel = ElementRef { kind: WidgetKind::Panel, ..button };
        let matched = ss.match_rules_in(button, &[panel]);
        assert!(matched.iter().any(|rule| rule.name == "panel primary"));
        assert!(!matched.iter().any(|rule| rule.name == "label"));
    }

    #[test]
    fn stylesheet_empty_rules() {
        let ss = StyleSheet::new();
        assert!(ss.rules().is_empty());
    }

    /// A descendant combinator matches at any depth, which is what distinguishes it from the
    /// child combinator tested beside it.
    #[test]
    fn a_descendant_combinator_matches_at_any_depth() {
        let selector = Selector::Descendant {
            ancestor: Box::new(Selector::Kind(WidgetKind::Panel)),
            subject: Box::new(Selector::Kind(WidgetKind::Button)),
        };
        let button = ElementRef { kind: WidgetKind::Button, class: None, id: None, state: None };
        let panel = ElementRef { kind: WidgetKind::Panel, ..button };
        let label = ElementRef { kind: WidgetKind::Label, ..button };

        assert!(selector.matches_in(button, &[panel, label]), "the panel is two levels up");
        assert!(!selector.matches_in(button, &[label]));
        assert!(!selector.matches_in(panel, &[panel]), "the subject itself must be a button");
    }

    /// A child combinator matches only the immediate parent.
    #[test]
    fn a_child_combinator_matches_only_the_immediate_parent() {
        let selector = Selector::Child {
            parent: Box::new(Selector::Kind(WidgetKind::Panel)),
            subject: Box::new(Selector::Kind(WidgetKind::Button)),
        };
        let button = ElementRef { kind: WidgetKind::Button, class: None, id: None, state: None };
        let panel = ElementRef { kind: WidgetKind::Panel, ..button };
        let label = ElementRef { kind: WidgetKind::Label, ..button };

        assert!(selector.matches_in(button, &[panel]), "panel is the immediate parent");
        assert!(
            !selector.matches_in(button, &[panel, label]),
            "a label sits between the panel and the button"
        );
    }

    /// Without a chain the combinator cannot answer, so it declines instead of guessing.
    #[test]
    fn a_combinator_without_context_matches_nothing() {
        let selector = Selector::Descendant {
            ancestor: Box::new(Selector::Universal),
            subject: Box::new(Selector::Kind(WidgetKind::Button)),
        };
        assert!(!selector.matches(WidgetKind::Button, None, None, None));
    }

    /// Specificity sums both compounds and is unaffected by which combinator joins them.
    #[test]
    fn a_combinator_weights_its_compounds_and_nothing_else() {
        let descendant = Selector::Descendant {
            ancestor: Box::new(Selector::Kind(WidgetKind::Panel)),
            subject: Box::new(Selector::Class("primary".to_string())),
        };
        let child = Selector::Child {
            parent: Box::new(Selector::Kind(WidgetKind::Panel)),
            subject: Box::new(Selector::Class("primary".to_string())),
        };
        assert_eq!(descendant.specificity(), 11);
        assert_eq!(child.specificity(), descendant.specificity());
    }
}
