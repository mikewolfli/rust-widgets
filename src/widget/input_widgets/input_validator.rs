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

use crate::compat::{Box, Cow, String, ToString, Vec};

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

    /// The message for `text`, which may quote the value that was refused.
    ///
    /// [`Validator::message`] is a static sentence, so a host rule could not say *which* value
    /// was out of range and the field's error row could only show a generic hint (I-27). A rule
    /// that has nothing value-specific to say inherits the static message, so this is an optional
    /// refinement rather than a second required method.
    fn message_for(&self, _text: &str) -> Cow<'_, str> {
        Cow::Borrowed(self.message())
    }

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

    /// Names the bound and the refused value, so the field's error row can say *why* it refused
    /// rather than repeating "in range" (I-27).
    fn message_for(&self, text: &str) -> Cow<'_, str> {
        let trimmed = text.trim();
        match self {
            Self::Int { minimum, maximum } => match trimmed.parse::<i64>() {
                Ok(value) if !(*minimum..=*maximum).contains(&value) => {
                    Cow::Owned(format!("{value} is outside {minimum}..={maximum}"))
                }
                _ => Cow::Borrowed(self.range_message()),
            },
            Self::Double { minimum, maximum } => match trimmed.parse::<f64>() {
                Ok(value) if value.is_finite() && !(*minimum..=*maximum).contains(&value) => {
                    Cow::Owned(format!("{value} is outside {minimum}..={maximum}"))
                }
                _ => Cow::Borrowed(self.range_message()),
            },
        }
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

/// A character-count rule for a text field (I-011).
///
/// Counts **characters**, not bytes, so a value of CJK text is judged by what the user sees and
/// what the field's own counter reports. `new(min, max)` swaps inverted bounds rather than
/// rejecting them, matching [`RangeValidator`]'s constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LengthValidator {
    minimum: usize,
    maximum: usize,
}

impl LengthValidator {
    /// Creates an inclusive character-length rule.
    pub fn new(minimum: usize, maximum: usize) -> Self {
        Self { minimum: minimum.min(maximum), maximum: maximum.max(minimum) }
    }

    /// The lowest accepted character count.
    pub fn minimum(&self) -> usize {
        self.minimum
    }

    /// The highest accepted character count.
    pub fn maximum(&self) -> usize {
        self.maximum
    }
}

impl Validator for LengthValidator {
    /// Empty is intermediate when a minimum is set, acceptable when the field may stay empty;
    /// a non-empty value outside the bounds is invalid.
    fn state(&self, text: &str) -> ValidationState {
        let count = text.chars().count();
        if count == 0 {
            return if self.minimum == 0 {
                ValidationState::Acceptable
            } else {
                ValidationState::Intermediate
            };
        }
        if (self.minimum..=self.maximum).contains(&count) {
            ValidationState::Acceptable
        } else {
            ValidationState::Invalid
        }
    }

    fn message(&self) -> &'static str {
        "Enter a value of the required length"
    }

    fn message_for(&self, text: &str) -> Cow<'_, str> {
        let count = text.chars().count();
        Cow::Owned(format!("{count} characters is outside {}..={}", self.minimum, self.maximum))
    }

    /// Truncates an over-long value to the maximum; a short value cannot be repaired.
    fn fixup(&self, text: &str) -> Option<String> {
        let count = text.chars().count();
        if count <= self.maximum {
            return None;
        }
        Some(text.chars().take(self.maximum).collect())
    }
}

/// A non-empty rule for a text field (I-011).
///
/// Whitespace-only input counts as empty: a value a form would store as a blank string is not a
/// value the user supplied. The message is the conventional form refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequiredValidator;

impl RequiredValidator {
    /// Creates a required/non-empty rule.
    pub fn new() -> Self {
        Self
    }
}

impl Validator for RequiredValidator {
    fn state(&self, text: &str) -> ValidationState {
        if text.trim().is_empty() {
            ValidationState::Invalid
        } else {
            ValidationState::Acceptable
        }
    }

    fn message(&self) -> &'static str {
        "This field is required"
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
/// * groupings `(...)` and alternation `a|b|c`, nested to any depth (I-05);
/// * the quantifiers `*`, `+` and `?` on the preceding atom or group;
/// * the anchors `^` (start) and `$` (end), at the top level.
///
/// Back-references and look-around are **not** supported; an unsupported construct is matched
/// literally, so a caller who needs it must use a full engine through the [`Validator`] trait
/// instead of being silently given the wrong answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternValidator {
    pattern: String,
    message: &'static str,
    /// The pattern compiled once, when the rule is built.
    ///
    /// `is_match` is consulted for every candidate and on every keystroke; compiling the pattern
    /// inside it re-parsed the same string each time (I-008). The rule never mutates its pattern,
    /// so an eagerly compiled value is always the current one and, unlike a `RefCell`, keeps the
    /// `Send + Sync` bound the `Validator` trait requires.
    compiled: Option<CompiledPattern>,
}

impl PatternValidator {
    /// Creates a pattern rule with the message shown when the value does not match.
    pub fn new(pattern: impl Into<String>, message: &'static str) -> Self {
        let pattern = pattern.into();
        let compiled = CompiledPattern::compile(&pattern);
        Self { pattern, message, compiled }
    }

    /// The pattern this rule matches.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Whether `text` matches the pattern.
    ///
    /// `false` for a malformed pattern, and also when the engine's step budget ran out before it
    /// could decide. [`Validator::state`] reports that second case as
    /// [`ValidationState::Intermediate`], because the rule did not prove the value wrong.
    pub fn is_match(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        match self.compiled.as_ref() {
            Some(compiled) => compiled.is_match(&chars),
            None => false,
        }
    }
}

impl Validator for PatternValidator {
    /// An empty value is intermediate; a non-matching value is invalid.
    fn state(&self, text: &str) -> ValidationState {
        if text.is_empty() {
            return ValidationState::Intermediate;
        }
        let Some(compiled) = self.compiled.as_ref() else {
            return ValidationState::Invalid;
        };
        let chars: Vec<char> = text.chars().collect();
        match compiled.match_outcome(&chars) {
            MatchOutcome::Matched => ValidationState::Acceptable,
            MatchOutcome::NoMatch => ValidationState::Invalid,
            // The engine stopped before deciding. Painting the value as wrong would accuse the
            // user of a mistake the rule itself could not confirm.
            MatchOutcome::Exhausted => ValidationState::Intermediate,
        }
    }

    fn message(&self) -> &'static str {
        self.message
    }
}

/// One node of a compiled pattern.
///
/// The engine used to be a flat atom list, which could not express grouping or alternation at
/// all: `"A|B"` was matched as the five characters it is spelled with, so the commonest
/// completion filter a host writes silently never matched (I-05). A tree of sequences,
/// alternations and repetitions is the smallest shape that supports both constructs while the
/// rest of the grammar (classes, quantifiers, anchors) is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    /// One matcher with its repetition span.
    Atom { kind: AtomKind, min: usize, max: Option<usize> },
    /// A parenthesised sequence, matched as a unit so a quantifier can repeat all of it.
    Group(Vec<Node>),
    /// An alternation of sequences, `a|b|c`.
    Alt(Vec<Vec<Node>>),
    /// A node repeated a bounded number of times — a quantified group.
    Repeat { node: Box<Node>, min: usize, max: Option<usize> },
}

/// What one pattern position accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// A pattern compiled once and matched against many candidates.
///
/// Compiling a pattern is the one part of matching that does not depend on the candidate, and the
/// completion filter used to pay it once per candidate per keystroke (I-P05). A caller that
/// matches a list against one pattern compiles it once here and reuses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledPattern {
    anchored_start: bool,
    anchored_end: bool,
    nodes: Vec<Node>,
    /// The node count, taken once at compile time so the per-candidate step budget never walks
    /// the tree.
    node_count: usize,
}

impl CompiledPattern {
    /// Compiles `pattern`, or returns `None` when it is malformed.
    pub(crate) fn compile(pattern: &str) -> Option<Self> {
        let chars: Vec<char> = pattern.chars().collect();
        let anchored_start = chars.first() == Some(&'^');
        let start = if anchored_start { 1 } else { 0 };
        let anchored_end = chars.len() > start && chars.last() == Some(&'$');
        let limit = if anchored_end { chars.len() - 1 } else { chars.len() };
        let (branches, next) = parse_alternation(&chars, start, limit)?;
        // A `)` with no opening `(` is a malformed pattern rather than a literal.
        if next < limit {
            return None;
        }
        let nodes = collapse(branches);
        let node_count = count_nodes(&nodes);
        Some(Self { anchored_start, anchored_end, nodes, node_count })
    }

    /// Whether `chars` matches, with the same unanchored semantics as before.
    pub(crate) fn is_match(&self, chars: &[char]) -> bool {
        self.match_outcome(chars) == MatchOutcome::Matched
    }

    /// The verdict for `chars`, including the "could not decide" case the boolean cannot carry.
    pub(crate) fn match_outcome(&self, chars: &[char]) -> MatchOutcome {
        let mut matcher = Matcher {
            text: chars,
            width: chars.len() + 1,
            remaining: self.step_budget(chars.len()),
        };
        let mut starts = PositionSet::new(matcher.width);
        if self.anchored_start {
            starts.insert(0);
        } else {
            // An unanchored pattern may begin anywhere. Seeding every position runs the sequence
            // once instead of once per start offset; the fold over a position set distributes
            // over a union of starts, so the reached set is exactly the union of what the old
            // per-start passes produced.
            for start in 0..=chars.len() {
                starts.insert(start);
            }
        }
        let Some(ends) = matcher.ends_of(&self.nodes, starts) else {
            return MatchOutcome::Exhausted;
        };
        let accepted =
            if self.anchored_end { ends.contains(chars.len()) } else { !ends.is_empty() };
        if accepted {
            MatchOutcome::Matched
        } else {
            MatchOutcome::NoMatch
        }
    }

    /// Steps one match may take: proportional to the value length and the pattern's node count.
    fn step_budget(&self, text_len: usize) -> u64 {
        let per_char = text_len.max(1) as u64;
        let nodes = self.node_count.max(1) as u64;
        per_char
            .saturating_mul(nodes)
            .saturating_mul(STEP_BUDGET_PER_CHAR_PER_NODE)
            .min(STEP_BUDGET_CEILING)
    }
}

/// Collapses a one-branch alternation to its sequence, keeping the branch list otherwise.
fn collapse(mut branches: Vec<Vec<Node>>) -> Vec<Node> {
    if branches.len() == 1 {
        branches.pop().unwrap_or_default()
    } else {
        vec![Node::Alt(branches)]
    }
}

/// Parses `a|b|c` from `index`, stopping at `)` or `limit`.
fn parse_alternation(
    chars: &[char],
    start: usize,
    limit: usize,
) -> Option<(Vec<Vec<Node>>, usize)> {
    let mut branches = Vec::new();
    let mut index = start;
    loop {
        let (sequence, next) = parse_sequence(chars, index, limit)?;
        branches.push(sequence);
        index = next;
        if index < limit && chars.get(index) == Some(&'|') {
            index += 1;
            continue;
        }
        return Some((branches, index));
    }
}

/// Parses atoms until `|`, `)` or `limit`.
fn parse_sequence(chars: &[char], mut index: usize, limit: usize) -> Option<(Vec<Node>, usize)> {
    let mut nodes = Vec::new();
    while index < limit {
        if matches!(chars[index], '|' | ')') {
            break;
        }
        let (node, next) = parse_node(chars, index, limit)?;
        nodes.push(node);
        index = next;
    }
    Some((nodes, index))
}

/// Parses one atom, class or group plus the quantifier that follows it.
fn parse_node(chars: &[char], index: usize, limit: usize) -> Option<(Node, usize)> {
    let ch = chars[index];
    let mut next = index;
    let base = match ch {
        '.' => {
            next += 1;
            Node::Atom { kind: AtomKind::Any, min: 1, max: Some(1) }
        }
        '\\' => {
            next += 1;
            let escaped = *chars.get(next)?;
            next += 1;
            Node::Atom { kind: AtomKind::Literal(escaped), min: 1, max: Some(1) }
        }
        '[' => {
            let (kind, after) = compile_class(chars, index)?;
            next = after;
            Node::Atom { kind, min: 1, max: Some(1) }
        }
        '(' => {
            let (branches, after) = parse_alternation(chars, index + 1, limit)?;
            if chars.get(after) != Some(&')') {
                return None;
            }
            next = after + 1;
            Node::Group(collapse(branches))
        }
        _ => {
            next += 1;
            Node::Atom { kind: AtomKind::Literal(ch), min: 1, max: Some(1) }
        }
    };
    if next >= limit {
        return Some((base, next));
    }
    let (min, max) = match chars[next] {
        '*' => (0, None),
        '+' => (1, None),
        '?' => (0, Some(1)),
        _ => return Some((base, next)),
    };
    let node = match base {
        Node::Atom { kind, .. } => Node::Atom { kind, min, max },
        other => Node::Repeat { node: Box::new(other), min, max },
    };
    Some((node, next + 1))
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

/// The engine's verdict for one candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MatchOutcome {
    /// The candidate does not match.
    NoMatch,
    /// The candidate matches.
    Matched,
    /// The step budget ran out before the engine could decide.
    ///
    /// A pattern such as `(a+)+b` makes a position simulation explore a combinatorial number of
    /// states on adversarial input. The rule runs on the UI thread for every keystroke, so the
    /// engine stops at a fixed cost and says it could not decide rather than blocking the frame.
    Exhausted,
}

/// Steps a match may take per input character and per pattern node.
///
/// A step is one character comparison or one reachable-position visit, so the budget bounds real
/// work rather than recursive calls. The factor is deliberately generous: an ordinary value
/// against an ordinary grammar uses a few hundred steps at most.
const STEP_BUDGET_PER_CHAR_PER_NODE: u64 = 256;

/// The ceiling on one match's steps, so a pasted megabyte cannot buy a megabyte of work.
const STEP_BUDGET_CEILING: u64 = 2_000_000;

/// A set of match positions, one bit per index in `0..=text.len()`.
///
/// The engine used to carry reachable positions in a `Vec<usize>` and dedupe with
/// `Vec::contains`, which made every insert linear in the positions already reached. A bitmap
/// turns that dedupe into a bit test and makes a whole-set union `text.len() / 64` words wide.
#[derive(Debug, Clone)]
struct PositionSet {
    words: Vec<u64>,
    count: usize,
}

impl PositionSet {
    /// An empty set covering the positions `0..positions`.
    fn new(positions: usize) -> Self {
        Self { words: vec![0u64; positions.div_ceil(64)], count: 0 }
    }

    /// Adds `position`, ignoring one outside the set's range.
    fn insert(&mut self, position: usize) {
        let Some(word) = self.words.get_mut(position / 64) else {
            return;
        };
        let mask = 1u64 << (position % 64);
        if *word & mask == 0 {
            *word |= mask;
            self.count += 1;
        }
    }

    /// Whether `position` is in the set.
    fn contains(&self, position: usize) -> bool {
        self.words.get(position / 64).is_some_and(|word| word & (1u64 << (position % 64)) != 0)
    }

    /// Whether the set holds no position.
    fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Removes every position, keeping the set's buffers.
    fn clear(&mut self) {
        self.words.fill(0);
        self.count = 0;
    }

    /// Adds every position `other` holds.
    fn union_with(&mut self, other: &Self) {
        let mut count = 0usize;
        for (word, other_word) in self.words.iter_mut().zip(other.words.iter()) {
            *word |= *other_word;
            count += word.count_ones() as usize;
        }
        self.count = count;
    }

    /// The positions held, ascending.
    fn iter(&self) -> PositionIter<'_> {
        PositionIter { words: &self.words, word: 0, bits: 0 }
    }
}

/// Iterates a [`PositionSet`]'s positions, ascending.
struct PositionIter<'a> {
    words: &'a [u64],
    /// The index of the next word to load.
    word: usize,
    /// The unconsumed bits of the last loaded word.
    bits: u64,
}

impl Iterator for PositionIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        loop {
            if self.bits != 0 {
                let bit = self.bits.trailing_zeros() as usize;
                self.bits &= self.bits - 1;
                return Some((self.word - 1) * 64 + bit);
            }
            self.bits = *self.words.get(self.word)?;
            self.word += 1;
        }
    }
}

/// One run of the position simulation over a candidate, with its step accounting.
struct Matcher<'a> {
    text: &'a [char],
    /// `text.len() + 1`: the number of positions a [`PositionSet`] covers.
    width: usize,
    /// Steps left before the run reports [`MatchOutcome::Exhausted`].
    remaining: u64,
}

impl Matcher<'_> {
    /// Spends `steps`, reporting `None` once the budget is gone.
    fn spend(&mut self, steps: u64) -> Option<()> {
        if self.remaining < steps {
            self.remaining = 0;
            return None;
        }
        self.remaining -= steps;
        Some(())
    }

    /// Every position `nodes` can reach when started from any position in `starts`.
    fn ends_of(&mut self, nodes: &[Node], starts: PositionSet) -> Option<PositionSet> {
        let mut reached = starts;
        for node in nodes {
            let mut next = PositionSet::new(self.width);
            let mut ends = PositionSet::new(self.width);
            for position in reached.iter() {
                self.spend(1)?;
                ends.clear();
                self.node_ends(node, position, &mut ends)?;
                next.union_with(&ends);
            }
            if next.is_empty() {
                return Some(next);
            }
            reached = next;
        }
        Some(reached)
    }

    /// Adds every position `node` can reach from `position` to `out`.
    ///
    /// `out` belongs to the caller so one buffer serves every position of a node instead of a
    /// fresh `Vec` per position; it must be cleared before the call.
    fn node_ends(&mut self, node: &Node, position: usize, out: &mut PositionSet) -> Option<()> {
        self.spend(1)?;
        match node {
            Node::Atom { kind, min, max } => {
                if *min == 0 {
                    out.insert(position);
                }
                let mut count = 0usize;
                let mut pos = position;
                let limit = max.unwrap_or(self.width);
                while count < limit && pos < self.text.len() && kind.matches(self.text[pos]) {
                    self.spend(1)?;
                    pos += 1;
                    count += 1;
                    if count >= *min {
                        out.insert(pos);
                    }
                }
            }
            Node::Group(nodes) => {
                let mut starts = PositionSet::new(self.width);
                starts.insert(position);
                let ends = self.ends_of(nodes, starts)?;
                out.union_with(&ends);
            }
            Node::Alt(branches) => {
                let mut starts = PositionSet::new(self.width);
                starts.insert(position);
                for branch in branches {
                    let ends = self.ends_of(branch, starts.clone())?;
                    out.union_with(&ends);
                }
            }
            Node::Repeat { node, min, max } => {
                if *min == 0 {
                    out.insert(position);
                }
                // Every repetition step shares one frontier bitmap: a position reached twice costs
                // one bit, so nesting a repeat is charged against the step budget instead of
                // multiplying per-position frontier lists.
                let max_count = max.unwrap_or(self.width);
                let mut frontier = PositionSet::new(self.width);
                frontier.insert(position);
                let mut step = PositionSet::new(self.width);
                let mut next = PositionSet::new(self.width);
                let mut count = 0usize;
                while count < max_count && !frontier.is_empty() {
                    next.clear();
                    for pos in frontier.iter() {
                        self.spend(1)?;
                        step.clear();
                        self.node_ends(node, pos, &mut step)?;
                        for end in step.iter() {
                            // A repetition that does not advance would loop forever; only a step
                            // that consumes input counts as a repetition.
                            if end > pos {
                                next.insert(end);
                            }
                        }
                    }
                    count += 1;
                    if count >= *min {
                        out.union_with(&next);
                    }
                    core::mem::swap(&mut frontier, &mut next);
                }
            }
        }
        Some(())
    }
}

/// The number of nodes in `nodes`, counting groups, branches and repeats once each.
fn count_nodes(nodes: &[Node]) -> usize {
    let mut count = 0usize;
    for node in nodes {
        count += 1;
        match node {
            Node::Atom { .. } => {}
            Node::Group(inner) => count += count_nodes(inner),
            Node::Alt(branches) => {
                for branch in branches {
                    count += count_nodes(branch);
                }
            }
            Node::Repeat { node, .. } => count += count_nodes(core::slice::from_ref(node.as_ref())),
        }
    }
    count
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

    #[test]
    fn a_pattern_supports_groups_and_alternation() {
        // The constructs `"A|B"` needs, which the flat-atom engine could not express (I-05).
        let images = PatternValidator::new("^(png|jpg|webp)$", "image extension");
        assert!(images.is_match("png"));
        assert!(images.is_match("jpg"));
        assert!(images.is_match("webp"));
        assert!(!images.is_match("gif"));
        assert!(!images.is_match("pngjpg"), "the anchors apply to the whole alternation");

        // A quantified group repeats the whole sequence rather than its last atom.
        let pairs = PatternValidator::new("^(ab)+$", "pairs");
        assert!(pairs.is_match("ab"));
        assert!(pairs.is_match("abab"));
        assert!(!pairs.is_match("a"));
        assert!(!pairs.is_match("aba"));

        // Groups nest.
        let nested = PatternValidator::new("^((a|b)c)+$", "nested");
        assert!(nested.is_match("acbc"));
        assert!(!nested.is_match("acb"));
    }

    #[test]
    fn a_compiled_pattern_matches_many_values() {
        let compiled = CompiledPattern::compile("a|b").expect("a valid pattern");
        assert!(compiled.is_match(&['a']) && compiled.is_match(&['b']));
        assert!(!compiled.is_match(&['c']));
        // A malformed pattern is refused rather than matched as literals.
        assert!(CompiledPattern::compile("(a").is_none());
    }

    #[test]
    fn a_long_value_against_an_ordinary_pattern_is_still_decided() {
        let digits = PatternValidator::new("^[0-9]+$", "digits only");
        let value = "7".repeat(4096);
        assert_eq!(digits.state(&value), ValidationState::Acceptable);
        assert!(digits.is_match(&value));
    }

    #[test]
    fn a_pathological_pattern_reports_intermediate_instead_of_blocking() {
        // `(a+)+b` cannot match a run of `a`, yet a simulation without a step budget explores a
        // combinatorial number of states before saying so. The rule must answer, and the answer
        // is "not decided" rather than an accusation the engine did not prove.
        let rule = PatternValidator::new("^(a+)+b$", "no");
        let value = "a".repeat(512);
        assert_eq!(rule.state(&value), ValidationState::Intermediate);
        assert!(!rule.is_match(&value));
    }

    #[test]
    fn a_range_message_names_the_bound_and_the_refused_value() {
        // The static message is still available, and the value-specific one refines it (I-27).
        let range = RangeValidator::int(1, 100);
        assert_eq!(range.message(), "Enter a whole number in range");
        assert_eq!(range.message_for("999"), "999 is outside 1..=100");
        // A rule with nothing value-specific to say keeps the static sentence.
        assert_eq!(range.message_for("50"), "Enter a whole number in range");
        let builtin: &dyn Validator = &InputValidator::Phone;
        assert_eq!(builtin.message_for("12"), builtin.message());
    }
}
