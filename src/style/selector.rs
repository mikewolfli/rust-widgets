//! CSS-style selector engine for widget styling.
//!
//! Allows matching widgets by kind, class name, ID, and state,
//! enabling CSS-like cascading style sheets for the widget system.

use crate::widget::WidgetKind;

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
}

/// Widget pseudo-states that can be matched by CSS selectors.
///
/// These represent dynamic widget states that selectors can target
/// (e.g., `:hover`, `:disabled`). Distinct from `theme_state::WidgetState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoState {
    Normal,
    Hover,
    Pressed,
    Disabled,
    Focused,
    Checked,
    Selected,
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
        }
    }

    /// Check if this selector matches a widget with the given properties.
    pub fn matches(
        &self,
        kind: WidgetKind,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
    ) -> bool {
        match self {
            Selector::Kind(k) => *k == kind,
            Selector::Class(c) => class == Some(c.as_str()),
            Selector::Id(i) => id == Some(i.as_str()),
            Selector::State(s) => state == Some(*s),
            Selector::Universal => true,
            Selector::And(selectors) => selectors.iter().all(|s| s.matches(kind, class, id, state)),
        }
    }
}

/// A collection of style rules that can be matched against widgets.
#[derive(Debug, Clone)]
pub struct StyleSheet {
    rules: Vec<StyleRule>,
}

impl StyleSheet {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: StyleRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| r.specificity);
    }

    /// Find all matching rules for a widget, sorted by specificity.
    pub fn match_rules(
        &self,
        kind: WidgetKind,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
    ) -> Vec<&StyleRule> {
        self.rules.iter().filter(|r| r.selector.matches(kind, class, id, state)).collect()
    }

    pub fn rules(&self) -> &[StyleRule] {
        &self.rules
    }
}

impl Default for StyleSheet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn stylesheet_empty_rules() {
        let ss = StyleSheet::new();
        assert!(ss.rules().is_empty());
    }
}
