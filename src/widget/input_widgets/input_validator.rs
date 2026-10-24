// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The input grammars a single-line text field can enforce.
//!
//! # Why this is its own module
//!
//! It is a *rule*, not a widget: two functions over a string (`filter` for the characters a
//! grammar cannot represent, `check` for whether a value satisfies it) plus the message shown
//! when it does not. `LineEdit` is already the crate's largest control; keeping the grammar
//! beside it would make the field the home of both the editing machinery and the validation
//! vocabulary, and every future grammar would grow the same file.
//!
//! # Why the grammar lives on the field rather than in a wrapper
//!
//! `phone_input` / `email_input` are the same control with a different accepted alphabet and
//! validity rule: a wrapper widget would have to duplicate the caret, selection, undo, IME and
//! decoration machinery to filter what reaches the model. The field owns the one insertion path
//! (`LineEdit::insert_str`) and the one assignment path (`LineEdit::set_text`), so the rule
//! belongs there. A plain `line_edit` carries no validator and reports itself valid.

use crate::compat::{String, ToString, Vec};

/// The three-way outcome of validating a value.
///
/// A boolean cannot describe the ordinary state of a field the user is still filling in: a
/// partially typed number is neither acceptable nor wrong, and painting it red while the user
/// types turns every entry into an error message. The third state is what lets a form keep its
/// submit affordance disabled without accusing the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidationState {
    /// The value cannot be made acceptable by further typing (for example, out of range).
    Invalid,
    /// The value is incomplete but not yet wrong (for example, a lone `-` in a number field).
    #[default]
    Intermediate,
    /// The value satisfies the rule.
    Acceptable,
}

impl ValidationState {
    /// Whether the value satisfies the rule.
    pub fn is_acceptable(self) -> bool {
        matches!(self, Self::Acceptable)
    }

    /// Whether the value is definitely wrong (as opposed to merely incomplete).
    pub fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }
}

/// A rule a text field can consult while the value changes.
///
/// # Why a trait rather than a closed enum
///
/// The two shipped grammars (phone and e-mail) answered a boolean question for one widget. A
/// host that owns a domain type — an order code, a range, a product identifier — has a rule the
/// library cannot enumerate, and wrapping the field was not an option: the wrapper would have to
/// reimplement the caret, selection, undo, IME and validation plumbing to filter what reaches the
/// model. The trait is the extension point the field consults, and the shipped grammars implement
/// it too, so there is one validation vocabulary rather than two.
///
/// Implementations must be deterministic and free of interior mutability: the field may call
/// [`Validator::state`] for every keystroke.
pub trait Validator: core::fmt::Debug + Send + Sync {
    /// The outcome for `text`.
    fn state(&self, text: &str) -> ValidationState;

    /// The message shown when [`Validator::state`] is not [`ValidationState::Acceptable`].
    fn message(&self) -> &'static str;

    /// The nearest acceptable value, when the rule can repair one.
    ///
    /// `None` means the value cannot be repaired without asking the user. The default is
    /// `None`, so a rule that has nothing sensible to offer does not have to say so.
    fn fixup(&self, _text: &str) -> Option<String> {
        None
    }

    /// Drops the characters the rule cannot represent from incoming text.
    ///
    /// The default keeps everything, which is right for a rule that constrains shape rather
    /// than alphabet (a range or a pattern).
    fn filter(&self, text: &str) -> String {
        text.to_string()
    }

    /// The shipped grammar this rule is, when it is one of them.
    ///
    /// A field uses this to keep assignment idempotent without downcasting: re-assigning the
    /// grammar it already holds must not revalidate and repaint. A host rule reports `None`
    /// and is therefore always applied.
    fn builtin(&self) -> Option<InputValidator> {
        None
    }
}

/// An input grammar a text field enforces as the user types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputValidator {
    /// A phone number: only digits and the separators `+ - ( ) space` are accepted,
    /// and a non-empty value must hold between 7 and 15 digits (the E.164 span).
    Phone,
    /// An e-mail address: whitespace and control characters are refused, and a
    /// non-empty value must be `local@domain` with a non-empty domain containing a
    /// dot, a non-empty local part, and at most 254 characters overall.
    Email,
}

impl InputValidator {
    /// Drops the characters this grammar cannot represent from incoming text.
    ///
    /// A phone number never starts or ends with its own separator, so a filtered edge space —
    /// the trace of a dropped word like "call " — is trimmed rather than kept as a leading gap.
    pub(crate) fn filter(self, text: &str) -> crate::compat::String {
        match self {
            Self::Phone => text
                .chars()
                .filter(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-' | ' ' | '(' | ')'))
                .collect::<crate::compat::String>()
                .trim()
                .chars()
                .collect(),
            Self::Email => {
                text.chars().filter(|ch| !ch.is_whitespace() && !ch.is_control()).collect()
            }
        }
    }

    /// Whether `text` satisfies the grammar, and the message to show when it does not.
    ///
    /// An **empty** value is valid for both grammars: "nothing typed yet" is not a
    /// misspelling, and a field that reports an error before the user has typed is noise.
    /// A caller that requires a value enforces that with its own rule (the `required`
    /// idiom belongs to the form, not to the field's character grammar).
    pub(crate) fn check(self, text: &str) -> (bool, &'static str) {
        if text.is_empty() {
            return (true, "");
        }
        match self {
            Self::Phone => {
                let digits = text.chars().filter(|ch| ch.is_ascii_digit()).count();
                if (7..=15).contains(&digits) {
                    (true, "")
                } else {
                    (false, "Enter a phone number (7 to 15 digits)")
                }
            }
            Self::Email => {
                let mut parts = text.split('@');
                let local = parts.next().unwrap_or_default();
                let domain = parts.next().unwrap_or_default();
                let single_at = parts.next().is_none();
                let domain_valid = !domain.is_empty()
                    && domain.contains('.')
                    && domain.split('.').all(|label| !label.is_empty());
                if single_at && !local.is_empty() && domain_valid && text.len() <= 254 {
                    (true, "")
                } else {
                    (false, "Enter an e-mail address (name@domain.com)")
                }
            }
        }
    }

    /// Repairs the value when the grammar has an unambiguous nearest form.
    ///
    /// A phone number is the one repairable grammar: separators at either edge are the trace of a
    /// word that happened to accompany the number (`"call 555-0100 "`), so trimming them yields
    /// the number the user meant. An e-mail address has no such rule — `name@domain` cannot be
    /// guessed into a top-level label — so it is returned unchanged and reported unrepairable.
    pub fn fixup(self, text: &str) -> Option<String> {
        match self {
            Self::Phone => {
                let trimmed =
                    text.trim_matches(|ch: char| matches!(ch, '+' | '-' | ' ' | '(' | ')'));
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Self::Email => None,
        }
    }
}

impl Validator for InputValidator {
    /// The bool rule mapped onto the three states.
    ///
    /// Both shipped grammars have no meaningful intermediate state: an empty value is accepted
    /// ("nothing typed yet" is not a misspelling) and anything else is either a complete value or
    /// a wrong one. A grammar with a prefix notion can report `Intermediate` instead.
    fn state(&self, text: &str) -> ValidationState {
        if self.check(text).0 {
            ValidationState::Acceptable
        } else {
            ValidationState::Invalid
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::Phone => "Enter a phone number (7 to 15 digits)",
            Self::Email => "Enter an e-mail address (name@domain.com)",
        }
    }

    fn fixup(&self, text: &str) -> Option<String> {
        (*self).fixup(text)
    }

    fn filter(&self, text: &str) -> String {
        (*self).filter(text)
    }

    fn builtin(&self) -> Option<InputValidator> {
        Some(*self)
    }
}

/// A numeric range rule for a text field.
///
/// The range spans are carried as `i64`/`f64` because that is what a field parses; a caller with a
/// larger domain type can implement [`Validator`] itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RangeValidator {
    /// An integer between `minimum` and `maximum`, inclusive.
    Int {
        /// The lowest accepted value.
        minimum: i64,
        /// The highest accepted value.
        maximum: i64,
    },
    /// A finite decimal between `minimum` and `maximum`, inclusive.
    Double {
        /// The lowest accepted value.
        minimum: f64,
        /// The highest accepted value.
        maximum: f64,
    },
}

impl RangeValidator {
    /// Creates an inclusive integer range rule.
    pub fn int(minimum: i64, maximum: i64) -> Self {
        Self::Int { minimum: minimum.min(maximum), maximum: maximum.max(minimum) }
    }

    /// Creates an inclusive decimal range rule.
    pub fn double(minimum: f64, maximum: f64) -> Self {
        Self::Double { minimum: minimum.min(maximum), maximum: maximum.max(minimum) }
    }

    /// The message for this rule.
    fn range_message(self) -> &'static str {
        match self {
            Self::Int { .. } => "Enter a whole number in range",
            Self::Double { .. } => "Enter a number in range",
        }
    }
}

impl Validator for RangeValidator {
    /// Parses the value and compares it with the bounds.
    ///
    /// An empty value is intermediate (a form may still be filled in), an unparsable non-empty
    /// value is intermediate as well — the user may be halfway through `-` or `1.` — and only a
    /// parsed value outside the bounds is definitely invalid. That distinction is what keeps a
    /// spinner from colouring the field red while the sign key is still the only thing typed.
    fn state(&self, text: &str) -> ValidationState {
        if text.is_empty() {
            return ValidationState::Intermediate;
        }
        match self {
            Self::Int { minimum, maximum } => match text.trim().parse::<i64>() {
                Ok(value) if (*minimum..=*maximum).contains(&value) => ValidationState::Acceptable,
                Ok(_) => ValidationState::Invalid,
                Err(_) => ValidationState::Intermediate,
            },
            Self::Double { minimum, maximum } => match text.trim().parse::<f64>() {
                Ok(value) if value.is_finite() && (*minimum..=*maximum).contains(&value) => {
                    ValidationState::Acceptable
                }
                Ok(value) if value.is_finite() => ValidationState::Invalid,
                _ => ValidationState::Intermediate,
            },
        }
    }

    fn message(&self) -> &'static str {
        self.range_message()
    }

    /// Clamps a parsable value into the range; an unparsable value is not repairable.
    fn fixup(&self, text: &str) -> Option<String> {
        match self {
            Self::Int { minimum, maximum } => text
                .trim()
                .parse::<i64>()
                .ok()
                .map(|value| value.clamp(*minimum, *maximum).to_string()),
            Self::Double { minimum, maximum } => text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(*minimum, *maximum).to_string()),
        }
    }
}

/// A small regular-expression rule.
///
/// # Supported syntax
///
/// The engine is deliberately a **subset** rather than a full regular-expression
/// implementation, because the crate has no regex dependency and the validation use case is
/// shape checking, not text processing. It supports:
///
/// * literal characters and `\` escapes;
/// * `.` for any character;
/// * character classes `[abc]`, ranges `[a-z]`, and their negation `[^abc]`;
/// * the quantifiers `*`, `+` and `?` on the preceding atom;
/// * the anchors `^` (start) and `$` (end).
///
/// Grouping, alternation and back-references are **not** supported. An unsupported construct is
/// matched literally, so a caller who needs it must use a full engine through the [`Validator`]
/// trait instead of being silently given the wrong answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternValidator {
    pattern: String,
    message: &'static str,
}

impl PatternValidator {
    /// Creates a pattern rule with the message shown when the value does not match.
    pub fn new(pattern: impl Into<String>, message: &'static str) -> Self {
        Self { pattern: pattern.into(), message }
    }

    /// The pattern this rule matches.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Whether `text` matches the pattern.
    pub fn is_match(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        match_pattern(&self.pattern, &chars)
    }
}

impl Validator for PatternValidator {
    /// An empty value is intermediate; a non-matching value is invalid.
    fn state(&self, text: &str) -> ValidationState {
        if text.is_empty() {
            ValidationState::Intermediate
        } else if self.is_match(text) {
            ValidationState::Acceptable
        } else {
            ValidationState::Invalid
        }
    }

    fn message(&self) -> &'static str {
        self.message
    }
}

/// One atom of a compiled pattern: the matcher plus how many times it may repeat.
#[derive(Debug, Clone)]
struct Atom {
    kind: AtomKind,
    min: usize,
    max: Option<usize>,
}

/// What one pattern position accepts.
#[derive(Debug, Clone)]
enum AtomKind {
    /// Any character.
    Any,
    /// One literal character.
    Literal(char),
    /// A member of a class, optionally negated.
    Class { negated: bool, members: Vec<(char, char)> },
}

impl AtomKind {
    fn matches(&self, ch: char) -> bool {
        match self {
            Self::Any => true,
            Self::Literal(literal) => *literal == ch,
            Self::Class { negated, members } => {
                let inside = members.iter().any(|(lo, hi)| (*lo..=*hi).contains(&ch));
                inside != *negated
            }
        }
    }
}

impl Atom {
    fn new(kind: AtomKind) -> Self {
        Self { kind, min: 1, max: Some(1) }
    }
}

/// Compiles the pattern into atoms, or returns `None` for a malformed class.
fn compile_pattern(pattern: &str) -> Option<(bool, bool, Vec<Atom>)> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut index = 0usize;
    let anchored_start = chars.first() == Some(&'^');
    if anchored_start {
        index = 1;
    }
    let anchored_end = chars.len() > index && chars.last() == Some(&'$');
    let limit = if anchored_end { chars.len() - 1 } else { chars.len() };
    let mut atoms: Vec<Atom> = Vec::new();
    while index < limit {
        let ch = chars[index];
        let kind = match ch {
            '.' => {
                index += 1;
                AtomKind::Any
            }
            '\\' => {
                index += 1;
                let escaped = chars.get(index).copied()?;
                index += 1;
                AtomKind::Literal(escaped)
            }
            '[' => {
                let (kind, next) = compile_class(&chars, index)?;
                index = next;
                kind
            }
            _ => {
                index += 1;
                AtomKind::Literal(ch)
            }
        };
        let mut atom = Atom::new(kind);
        if index < limit {
            match chars[index] {
                '*' => {
                    atom.min = 0;
                    atom.max = None;
                    index += 1;
                }
                '+' => {
                    atom.min = 1;
                    atom.max = None;
                    index += 1;
                }
                '?' => {
                    atom.min = 0;
                    atom.max = Some(1);
                    index += 1;
                }
                _ => {}
            }
        }
        atoms.push(atom);
    }
    Some((anchored_start, anchored_end, atoms))
}

/// Compiles one `[...]` class starting at `start`, returning it and the index after `]`.
fn compile_class(chars: &[char], start: usize) -> Option<(AtomKind, usize)> {
    let mut index = start + 1;
    let mut negated = false;
    if chars.get(index) == Some(&'^') {
        negated = true;
        index += 1;
    }
    let mut members: Vec<(char, char)> = Vec::new();
    let mut first = true;
    while index < chars.len() {
        let ch = chars[index];
        if ch == ']' && !first {
            return Some((AtomKind::Class { negated, members }, index + 1));
        }
        first = false;
        let lo = if ch == '\\' {
            index += 1;
            *chars.get(index)?
        } else {
            ch
        };
        index += 1;
        // A `-` followed by a member and then `]` is a literal hyphen.
        if chars.get(index) == Some(&'-') && chars.get(index + 1).is_some_and(|next| *next != ']') {
            index += 1;
            let hi_raw = chars.get(index).copied()?;
            let hi = if hi_raw == '\\' {
                index += 1;
                *chars.get(index)?
            } else {
                hi_raw
            };
            index += 1;
            members.push((lo.min(hi), lo.max(hi)));
        } else {
            members.push((lo, lo));
        }
    }
    None
}

/// Whether `text` matches `pattern` under the same grammar [`PatternValidator`] uses.
///
/// Shared with the completion matcher, so a host's pattern-style completion prefix and its
/// validation rule cannot disagree about what a pattern means.
#[cfg(widgets_unstripped)]
pub(crate) fn pattern_matches(pattern: &str, text: &str) -> bool {
    let chars: crate::compat::Vec<char> = text.chars().collect();
    match_pattern(pattern, &chars)
}

/// Matches the compiled pattern against `text` with backtracking over quantifiers.
fn match_pattern(pattern: &str, text: &[char]) -> bool {
    let Some((anchored_start, anchored_end, atoms)) = compile_pattern(pattern) else {
        return false;
    };
    if anchored_start {
        return match_at(&atoms, 0, text, 0, anchored_end);
    }
    (0..=text.len()).any(|start| match_at(&atoms, 0, text, start, anchored_end))
}

/// Tries to match `atoms[index..]` against `text[position..]`.
fn match_at(
    atoms: &[Atom],
    index: usize,
    text: &[char],
    position: usize,
    anchored_end: bool,
) -> bool {
    if index == atoms.len() {
        return !anchored_end || position == text.len();
    }
    let atom = &atoms[index];
    let max = atom.max.unwrap_or(text.len());
    // Greedy, then backtrack: try the longest repetition first so `.*x` matches the last `x`.
    let mut count = 0usize;
    while count < max && position + count < text.len() && atom.kind.matches(text[position + count])
    {
        count += 1;
    }
    loop {
        if count >= atom.min && match_at(atoms, index + 1, text, position + count, anchored_end) {
            return true;
        }
        if count == 0 {
            return false;
        }
        count -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phone_grammar_keeps_only_its_own_characters() {
        assert_eq!(InputValidator::Phone.filter("call +1 (555) 010-9999!"), "+1 (555) 010-9999");
        assert_eq!(InputValidator::Phone.filter("abc"), "");
    }

    #[test]
    fn an_empty_value_is_valid_for_both_grammars() {
        assert_eq!(InputValidator::Phone.check(""), (true, ""));
        assert_eq!(InputValidator::Email.check(""), (true, ""));
    }

    #[test]
    fn the_phone_grammar_measures_digits_not_characters() {
        assert!(InputValidator::Phone.check("1234567").0);
        assert!(InputValidator::Phone.check("+1 (555) 010-9999").0);
        assert!(!InputValidator::Phone.check("123456").0, "six digits is too short");
        assert!(!InputValidator::Phone.check("1234567890123456").0, "sixteen is too long");
    }

    #[test]
    fn the_email_grammar_requires_one_at_and_a_dotted_domain() {
        assert!(InputValidator::Email.check("name@domain.com").0);
        assert!(!InputValidator::Email.check("name@domain").0, "no top-level label");
        assert!(!InputValidator::Email.check("@domain.com").0, "no local part");
        assert!(!InputValidator::Email.check("a@b@c.com").0, "two at signs");
        assert!(!InputValidator::Email.check("name@.com").0, "an empty label");
    }

    #[test]
    fn the_email_grammar_refuses_whitespace_and_control_characters() {
        assert_eq!(InputValidator::Email.filter("a b\tc"), "abc");
        assert_eq!(InputValidator::Email.filter("name@domain.com"), "name@domain.com");
    }

    #[test]
    fn a_message_is_carried_only_when_the_value_is_refused() {
        assert_eq!(InputValidator::Phone.check("12").1, "Enter a phone number (7 to 15 digits)");
        assert_eq!(InputValidator::Email.check("x").1, "Enter an e-mail address (name@domain.com)");
    }

    #[test]
    fn the_grammars_report_the_three_way_state() {
        assert_eq!(InputValidator::Phone.state(""), ValidationState::Acceptable);
        assert_eq!(InputValidator::Phone.state("1234567"), ValidationState::Acceptable);
        assert_eq!(InputValidator::Email.state("not-an-address"), ValidationState::Invalid);
        assert!(!ValidationState::Intermediate.is_acceptable());
        assert!(!ValidationState::Intermediate.is_invalid());
    }

    #[test]
    fn the_phone_grammar_repairs_its_own_edge_separators() {
        assert_eq!(InputValidator::Phone.fixup(" 555-0100 "), Some("555-0100".to_string()));
        assert_eq!(InputValidator::Phone.fixup("---"), None);
        assert_eq!(InputValidator::Email.fixup("name@domain.com"), None);
    }

    #[test]
    fn a_range_rule_separates_incomplete_from_out_of_range() {
        let range = RangeValidator::int(1, 10);
        assert_eq!(range.state(""), ValidationState::Intermediate);
        assert_eq!(range.state("-"), ValidationState::Intermediate);
        assert_eq!(range.state("11"), ValidationState::Invalid);
        assert_eq!(range.state("5"), ValidationState::Acceptable);
        assert_eq!(range.fixup("42"), Some("10".to_string()));
    }

    #[test]
    fn a_double_range_rule_accepts_decimals_and_refuses_non_finite_values() {
        let range = RangeValidator::double(0.0, 1.0);
        assert_eq!(range.state("0.5"), ValidationState::Acceptable);
        assert_eq!(range.state("1.5"), ValidationState::Invalid);
        assert_eq!(range.state("NaN"), ValidationState::Intermediate);
        assert_eq!(range.fixup("1.5"), Some("1".to_string()));
    }

    #[test]
    fn a_pattern_rule_supports_anchors_classes_and_quantifiers() {
        let code = PatternValidator::new("^[A-Z]{0}", "bad code");
        // `{0}` is not part of the subset, so it is matched literally and this value fails.
        assert_eq!(code.state("ABC"), ValidationState::Invalid);

        let digits = PatternValidator::new("^[0-9]+$", "digits only");
        assert!(digits.is_match("0123"));
        assert!(!digits.is_match("0123a"));
        assert_eq!(digits.state(""), ValidationState::Intermediate);
        assert_eq!(digits.state("12"), ValidationState::Acceptable);

        let name = PatternValidator::new("[a-z]+\\.[a-z]+", "name.ext");
        assert!(name.is_match("report.txt"));
        assert!(!name.is_match("report"));
        assert_eq!(name.message(), "name.ext");
    }

    #[test]
    fn a_validator_can_be_used_behind_the_trait() {
        let rule: &dyn Validator = &RangeValidator::int(0, 3);
        assert!(rule.state("2").is_acceptable());
        assert_eq!(rule.filter(" anything "), " anything ");
    }
}
