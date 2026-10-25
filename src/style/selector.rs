// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CSS-style selector engine for widget styling.
//!
//! Allows matching widgets by kind, class name, ID, and state,
//! enabling CSS-like cascading style sheets for the widget system.

use crate::compat::{Box, HashMap, String, Vec};
use crate::widget::WidgetKind;

use core::cell::RefCell;

use crate::style::theme_state::WidgetState;

/// The narrowing key a rule is filed under in a [`RuleIndex`].
///
/// Matching starts from the element's own tag/class/id and only looks at rules filed under one of
/// those (or under `Any`), instead of scanning every rule in the sheet (G-22).
///
/// The keys borrow their text: a lookup builds no temporary `String`, which is what made the
/// previous `SelectorIndexKey` enum allocate twice per match even on a miss (G-163).
#[derive(Debug, Clone, Copy)]
pub(crate) enum RuleIndexKey<'a> {
    /// A widget kind, in any spelling the factory accepts; normalised on insert and on lookup.
    Kind(&'a str),
    /// A CSS class name, matched exactly.
    Class(&'a str),
    /// A widget id, matched exactly.
    Id(&'a str),
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
fn selector_index_key(selector: &Selector) -> RuleIndexKey<'_> {
    match selector {
        Selector::Kind(kind) => RuleIndexKey::Kind(kind.css_name()),
        Selector::Class(class) => RuleIndexKey::Class(class),
        Selector::Id(id) => RuleIndexKey::Id(id),
        Selector::State(_) | Selector::Universal => RuleIndexKey::Any,
        Selector::And(parts) => {
            if let Some(id) = parts.iter().find_map(|part| match part {
                Selector::Id(id) => Some(id.as_str()),
                _ => None,
            }) {
                return RuleIndexKey::Id(id);
            }
            if let Some(class) = parts.iter().find_map(|part| match part {
                Selector::Class(class) => Some(class.as_str()),
                _ => None,
            }) {
                return RuleIndexKey::Class(class);
            }
            if let Some(kind) = parts.iter().find_map(|part| match part {
                Selector::Kind(kind) => Some(kind.css_name()),
                _ => None,
            }) {
                return RuleIndexKey::Kind(kind);
            }
            RuleIndexKey::Any
        }
        Selector::Descendant { subject, .. } | Selector::Child { subject, .. } => {
            selector_index_key(subject)
        }
    }
}

/// Writes the normalised form of a kind spelling into `out`.
///
/// The normalisation is [`crate::widget::capability::coercion::normalize_key`]'s rule (drop `_`,
/// `-` and spaces, lowercase) applied into a caller-owned buffer, so a lookup reuses one
/// allocation instead of making a new `String` per match (G-163).
fn normalize_kind_key(input: &str, out: &mut String) {
    out.clear();
    for ch in input.chars() {
        if matches!(ch, '_' | '-' | ' ') {
            continue;
        }
        out.extend(ch.to_lowercase());
    }
}

/// The reusable buffers one selector match needs.
///
/// Both fields are cleared rather than reallocated on each use, so a steady-state match performs
/// no heap allocation at all; only the result the caller asked for is allocated (G-163).
#[derive(Default)]
pub(crate) struct MatchScratch {
    /// Candidate rule positions, always ascending.
    pub(crate) positions: Vec<usize>,
    /// The normalised kind key used to probe the index.
    kind_key: String,
}

/// Runs `f` with this thread's match scratch.
///
/// The fallback is for a nested match. The current call graph never performs one, but a future
/// caller must get a correct answer rather than a borrow panic, and a fresh scratch costs one
/// allocation on a path that is already wrong for a reason worth fixing.
pub(crate) fn with_match_scratch<R>(f: impl FnOnce(&mut MatchScratch) -> R) -> R {
    thread_local! {
        static SCRATCH: RefCell<MatchScratch> = RefCell::new(MatchScratch::default());
    }
    SCRATCH.with(|cell| {
        let mut borrowed = cell.try_borrow_mut().ok();
        let mut fallback = MatchScratch::default();
        let scratch: &mut MatchScratch = borrowed.as_deref_mut().unwrap_or(&mut fallback);
        f(scratch)
    })
}

/// An inverted index over one stylesheet's rules, shared by both selector representations.
///
/// # Why one index type for two selector enums
///
/// [`StyleSheet`] (typed [`Selector`]) and [`crate::style::stylesheet::StyleSheetManager`]
/// (string [`CssSelector`](crate::style::css::CssSelector)) used to be the same concept with two
/// implementations, and only one of them was indexed. The index is built by whoever owns the
/// rules and only reads borrowed text at match time, so both can use it (G-162, principle #101).
///
/// Buckets hold positions in ascending order because rules are inserted in rule-list order. The
/// four buckets a lookup consults are **disjoint** — every rule is filed under exactly one key —
/// which is what lets [`Self::candidates_into`] merge them without a de-duplication pass.
#[derive(Debug, Clone, Default)]
pub(crate) struct RuleIndex {
    by_kind: HashMap<String, Vec<usize>>,
    by_class: HashMap<String, Vec<usize>>,
    by_id: HashMap<String, Vec<usize>>,
    any: Vec<usize>,
}

impl RuleIndex {
    /// An index with no rules.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Removes every entry, keeping the allocated buckets.
    pub(crate) fn clear(&mut self) {
        self.by_kind.clear();
        self.by_class.clear();
        self.by_id.clear();
        self.any.clear();
    }

    /// Files `position` under `key`.
    ///
    /// Called once per rule per sheet mutation, which is why the owned key is acceptable here:
    /// the match path is the one that must stay allocation-free.
    pub(crate) fn insert(&mut self, position: usize, key: RuleIndexKey<'_>) {
        match key {
            RuleIndexKey::Kind(kind) => {
                let mut normalized = String::new();
                normalize_kind_key(kind, &mut normalized);
                self.by_kind.entry(normalized).or_default().push(position);
            }
            RuleIndexKey::Class(class) => {
                self.by_class.entry(String::from(class)).or_default().push(position);
            }
            RuleIndexKey::Id(id) => {
                self.by_id.entry(String::from(id)).or_default().push(position);
            }
            RuleIndexKey::Any => self.any.push(position),
        }
    }

    /// Writes the positions whose narrowing key can match this element, in ascending order.
    ///
    /// The lookups borrow `kind`/`class`/`id`, so the only allocation is the one `scratch`
    /// already owns (G-163).
    pub(crate) fn candidates_into(
        &self,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        scratch: &mut MatchScratch,
    ) {
        normalize_kind_key(kind, &mut scratch.kind_key);
        let kind_bucket =
            self.by_kind.get(scratch.kind_key.as_str()).map(Vec::as_slice).unwrap_or(&[]);
        let class_bucket =
            class.and_then(|class| self.by_class.get(class)).map(Vec::as_slice).unwrap_or(&[]);
        let id_bucket = id.and_then(|id| self.by_id.get(id)).map(Vec::as_slice).unwrap_or(&[]);
        merge_buckets([kind_bucket, class_bucket, id_bucket, &self.any], &mut scratch.positions);
    }
}

/// Merges four ascending position lists into one ascending list.
///
/// The buckets are disjoint by construction, so a plain multi-way merge is enough. This replaces
/// the `sort_unstable` + `dedup` that ran on every match (G-163).
fn merge_buckets<const N: usize>(buckets: [&[usize]; N], out: &mut Vec<usize>) {
    out.clear();
    let mut cursors = [0usize; N];
    loop {
        let mut best: Option<(usize, usize)> = None;
        for (index, bucket) in buckets.iter().enumerate() {
            let Some(&position) = bucket.get(cursors[index]) else {
                continue;
            };
            if best.is_none_or(|(current, _)| position < current) {
                best = Some((position, index));
            }
        }
        let Some((position, index)) = best else {
            break;
        };
        out.push(position);
        cursors[index] += 1;
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
/// This is a **name for [`WidgetState`]**, not a second enum (G-405). The two used to be separate
/// 10- and 12-variant enumerations with two string tables beside them, so adding one state meant
/// editing four places and any missed edit made the CSS path and the theme's
/// `"<kind>:<state>"` keys disagree about what a word means (principle #54).
///
/// Keeping the name means every existing `PseudoState::Hover` call site keeps compiling, and the
/// canonical suffix table in [`WidgetState`] is now the only spelling source both directions read.
pub type PseudoState = WidgetState;

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
    /// Rule positions by the narrowing key described on [`RuleIndexKey`]. Rebuilt whenever the
    /// rule list changes; the list is built once at startup, so an add may pay O(n) to keep both
    /// structures in step (G-22).
    index: RuleIndex,
}

impl StyleSheet {
    /// Creates a stylesheet with no rules.
    pub fn new() -> Self {
        Self { rules: Vec::new(), index: RuleIndex::new() }
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
            self.index.insert(position, selector_index_key(&rule.selector));
        }
    }

    /// Find all matching rules for a widget, sorted by specificity.
    pub fn match_rules(
        &self,
        kind: WidgetKind,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
    ) -> Vec<&StyleRule> {
        with_match_scratch(|scratch| {
            self.index.candidates_into(kind.css_name(), class, id, scratch);
            scratch
                .positions
                .iter()
                .filter_map(|&position| self.rules.get(position))
                .filter(|rule| rule.selector.matches(kind, class, id, state))
                .collect()
        })
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
        with_match_scratch(|scratch| {
            self.index.candidates_into(element.kind.css_name(), element.class, element.id, scratch);
            scratch
                .positions
                .iter()
                .filter_map(|&position| self.rules.get(position))
                .filter(|rule| rule.selector.matches_in(element, ancestors))
                .collect()
        })
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

    /// G-405: the CSS state name **is** the theme's state type, so a state added to
    /// [`WidgetState`] is selectable from a sheet without a second enum or a second name table.
    #[test]
    fn the_css_state_name_is_the_theme_state() {
        let from_theme: PseudoState = WidgetState::Active;
        assert_eq!(from_theme.state_suffix(), "active");
        assert_eq!(PseudoState::from_state_suffix("hover"), Some(WidgetState::Hover));
        // The two former spellings that described the same thing now resolve through one table.
        assert_eq!(PseudoState::from_state_suffix("disabled"), Some(WidgetState::Disabled));
        assert_eq!(PseudoState::from_state_suffix("focus"), Some(WidgetState::Focused));
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

    /// The four buckets are merged back into one ascending list, so a match still returns rules in
    /// cascade order without a sort (G-163).
    #[test]
    fn candidates_from_several_buckets_come_back_in_rule_order() {
        let mut ss = StyleSheet::new();
        ss.add_rule(StyleRule::new(Selector::Kind(WidgetKind::Button), "kind"));
        ss.add_rule(StyleRule::new(Selector::Universal, "universal"));
        ss.add_rule(StyleRule::new(
            Selector::And(vec![
                Selector::Kind(WidgetKind::Button),
                Selector::Class("primary".to_string()),
            ]),
            "compound",
        ));
        ss.add_rule(StyleRule::new(Selector::Class("primary".to_string()), "class"));

        let names: Vec<&str> = ss
            .match_rules(WidgetKind::Button, Some("primary"), None, None)
            .iter()
            .map(|rule| rule.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["universal", "kind", "class", "compound"],
            "specificity ascending (0, 1, 10, 11) is the cascade order"
        );
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
