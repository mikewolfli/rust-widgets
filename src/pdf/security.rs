//! PDF security serialization and parsing.

use crate::pdf::types::*;
use crate::pdf::writer::pdf_escape_literal;

pub(crate) fn serialize_security_diagnostics_entries(security: &PdfSecurity) -> String {
    if *security == PdfSecurity::default() {
        return String::new();
    }
    let user_password = security.user_password.as_deref().unwrap_or("");
    let owner_password = security.owner_password.as_deref().unwrap_or("");
    format!(
        " /RWSecurityUnsupported true /RWUserPassword ({}) /RWOwnerPassword ({}) /RWPermPrint {} /RWPermEdit {} /RWPermCopy {} /RWPermAnnot {}",
        pdf_escape_literal(user_password),
        pdf_escape_literal(owner_password),
        security.print_permission,
        security.edit_permission,
        security.copy_permission,
        security.annotation_permission,
    )
}
pub(crate) fn parse_security_diagnostics(text: &str) -> Option<PdfSecurity> {
    if !text.contains("/RWSecurityUnsupported true") {
        return None;
    }
    let user_password = parse_pdf_literal_by_key(text, "/RWUserPassword").filter(|v| !v.is_empty());
    let owner_password =
        parse_pdf_literal_by_key(text, "/RWOwnerPassword").filter(|v| !v.is_empty());
    Some(PdfSecurity {
        user_password,
        owner_password,
        print_permission: parse_pdf_bool_by_key(text, "/RWPermPrint").unwrap_or(true),
        edit_permission: parse_pdf_bool_by_key(text, "/RWPermEdit").unwrap_or(true),
        copy_permission: parse_pdf_bool_by_key(text, "/RWPermCopy").unwrap_or(true),
        annotation_permission: parse_pdf_bool_by_key(text, "/RWPermAnnot").unwrap_or(true),
    })
}
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

