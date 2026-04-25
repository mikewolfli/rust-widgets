//! PDF security serialization and parsing.

use crate::pdf::types::*;
use crate::pdf::writer::pdf_escape_literal;

/// Serialize security diagnostics entries.
///
/// # WARNING: Full PDF encryption not implemented
///
/// The `PdfSecurity` struct stores user/owner passwords and permission flags,
/// but this module does NOT perform actual PDF encryption per the PDF 2.0 spec.
///
/// Required encryption approaches (not yet implemented):
///   - AES-128 (PDF 2.0, Revision 2): Uses AES-128 in CBC mode with a 128-bit key,
///     computed via the SASL/PKCS#7 key derivation algorithm (Algorithm 3.2a/3.2b).
///   - AES-256 (PDF 2.0, Revision 5+): Uses AES-256 in CBC mode with a 256-bit key,
///     requiring the SASL/PKCS#7 key derivation algorithm (Algorithm 3.2c/3.2d).
///
/// Until encryption is implemented:
///   - Passwords stored in PdfSecurity are advisory only (no actual protection).
///   - Permission flags are serialized as metadata but not enforced by encryption.
///   - The document is produced as a standard unencrypted PDF.
///
/// When the current serializer encounters a non-default PdfSecurity, it emits
/// a placeholder comment indicating encryption is unsupported, rather than
/// writing fake /RW* custom keys that would confuse compliant readers.
pub(crate) fn serialize_security_diagnostics_entries(security: &PdfSecurity) -> String {
    if *security == PdfSecurity::default() {
        return String::new();
    }
    // Emit a comment-only marker so diagnostic tools can see a security intent
    // without adding non-standard keys to the document dictionary.
    let user_password = security.user_password.as_deref().unwrap_or("");
    let owner_password = security.owner_password.as_deref().unwrap_or("");
    format!(
        " % RW-NOTE: PDF encryption not implemented (password=\"{}\", owner=\"{}\", print={}, edit={}, copy={}, annot={})",
        pdf_escape_literal(user_password),
        pdf_escape_literal(owner_password),
        security.print_permission,
        security.edit_permission,
        security.copy_permission,
        security.annotation_permission,
    )
}

/// Parse security diagnostics from document info text.
///
/// Looks for the `% RW-NOTE: PDF encryption not implemented` comment that
/// was placed by [`serialize_security_diagnostics_entries`] and reconstructs
/// the original [`PdfSecurity`] from the embedded parameters.
pub(crate) fn parse_security_diagnostics(text: &str) -> Option<PdfSecurity> {
    if !text.contains("RW-NOTE: PDF encryption not implemented") {
        return None;
    }
    let user_password = parse_legacy_password(text).unwrap_or_default();
    let owner_password = parse_legacy_owner_password(text).unwrap_or_default();
    let print_permission = text.contains("print=true");
    let edit_permission = text.contains("edit=true");
    let copy_permission = text.contains("copy=true");
    let annotation_permission = text.contains("annot=true");
    Some(PdfSecurity {
        user_password: if user_password.is_empty() {
            None
        } else {
            Some(user_password)
        },
        owner_password: if owner_password.is_empty() {
            None
        } else {
            Some(owner_password)
        },
        print_permission,
        edit_permission,
        copy_permission,
        annotation_permission,
    })
}

/// Backward-compatible parser for the old /RWUserPassword-based format.
fn parse_legacy_password(text: &str) -> Option<String> {
    // Try the old custom-key format first, then fall back to the comment-based format.
    if text.contains("/RWUserPassword") {
        parse_pdf_literal_by_key(text, "/RWUserPassword")
    } else {
        parse_comment_password(text, "password=\"")
    }
}

/// Backward-compatible parser for the old /RWOwnerPassword-based format.
fn parse_legacy_owner_password(text: &str) -> Option<String> {
    if text.contains("/RWOwnerPassword") {
        parse_pdf_literal_by_key(text, "/RWOwnerPassword")
    } else {
        parse_comment_password(text, "owner=\"")
    }
}

/// Extract a quoted value from the RW-NOTE comment after a given key.
fn parse_comment_password(text: &str, key: &str) -> Option<String> {
    let start = text.find(key)? + key.len();
    let rest = text.get(start..)?;
    let end = rest.find('"')?;
    let value = rest[..end].to_string();
    if value.is_empty() || value == "''" {
        None
    } else {
        Some(value)
    }
}

#[allow(dead_code)]
fn parse_pdf_bool_by_key(text: &str, key: &str) -> Option<bool> {
    let start = text.find(key)? + key.len();
    let rest = text.get(start..)?.trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn parse_pdf_literal_by_key(text: &str, key: &str) -> Option<String> {
    let start = text.find(key)? + key.len();
    let rest = text.get(start..)?.trim_start();
    let literal_start = rest.find('(')? + 1;
    let literal_tail = rest.get(literal_start..)?;
    let literal_end = literal_tail.find(')')?;
    Some(literal_tail[..literal_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::types::PdfSecurity;

    #[test]
    fn test_default_security_returns_empty() {
        let security = PdfSecurity::default();
        let result = serialize_security_diagnostics_entries(&security);
        assert_eq!(result, "");
    }

    #[test]
    fn test_non_default_security_emits_note() {
        let security = PdfSecurity {
            user_password: Some("hello".to_string()),
            owner_password: Some("world".to_string()),
            print_permission: true,
            edit_permission: false,
            copy_permission: true,
            annotation_permission: false,
        };
        let result = serialize_security_diagnostics_entries(&security);
        assert!(result.contains("RW-NOTE: PDF encryption not implemented"));
        assert!(result.contains("password=\"hello\""));
        assert!(result.contains("owner=\"world\""));
        assert!(result.contains("print=true"));
        assert!(result.contains("edit=false"));
        assert!(result.contains("copy=true"));
        assert!(result.contains("annot=false"));
    }

    #[test]
    fn test_round_trip_via_comment_format() {
        let security = PdfSecurity {
            user_password: Some("test123".to_string()),
            owner_password: None,
            print_permission: false,
            edit_permission: true,
            copy_permission: false,
            annotation_permission: true,
        };
        let serialized = serialize_security_diagnostics_entries(&security);
        let parsed = parse_security_diagnostics(&serialized);
        assert!(parsed.is_some());
        let parsed = parsed.unwrap();
        assert_eq!(parsed.user_password, Some("test123".to_string()));
        assert_eq!(parsed.owner_password, None);
        assert!(!parsed.print_permission);
        assert!(parsed.edit_permission);
        assert!(!parsed.copy_permission);
        assert!(parsed.annotation_permission);
    }

    #[test]
    fn test_parse_old_custom_key_format() {
        let text = "% RW-NOTE: PDF encryption not implemented (password=\"secret\", owner=\"admin\", print=true, edit=false)";
        let parsed = parse_security_diagnostics(text);
        assert!(parsed.is_some());
        let parsed = parsed.unwrap();
        assert_eq!(parsed.user_password, Some("secret".to_string()));
        assert_eq!(parsed.owner_password, Some("admin".to_string()));
        assert!(parsed.print_permission);
        assert!(!parsed.edit_permission);
    }
}
