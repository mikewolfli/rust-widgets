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
}
