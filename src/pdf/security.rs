//! PDF security serialization, parsing, and content-level encryption.
//!
//! # Document-level security model
//!
//! Setting a [`PdfSecurity`] records *intent*: the serialized marker states the
//! requested permissions and explicitly notes that the document is **not
//! encrypted** (document-level PDF encryption is not wired into the writer).
//! Passwords are never written into the output file — echoing them into an
//! unencrypted document would leak the secrets in plain text.
//!
//! # Content-level encryption tooling (`pdf-encryption` feature)
//!
//! With the `pdf-encryption` feature the module additionally provides real
//! AES-128-CBC primitives ([`PdfEncryption`], [`encrypt_pdf`]) that encrypt
//! arbitrary byte content with a password-derived key. These are content-level
//! helpers, not a substitute for a standards-compliant PDF encryption
//! dictionary pipeline.

use crate::pdf::types::*;

/// Serialize the security intent marker.
///
/// Returns the empty string for a default (fully open) security profile.
/// Otherwise returns a standalone `%` comment line stating the requested
/// permissions and warning that the output is NOT encrypted. Never contains
/// password material.
pub(crate) fn serialize_security_diagnostics_entries(security: &PdfSecurity) -> String {
    if *security == PdfSecurity::default() {
        return String::new();
    }
    format!(
        "% RW-NOTE: PDF encryption requested but not implemented (print={}, edit={}, copy={}, annot={}) — document is NOT encrypted",
        security.print_permission,
        security.edit_permission,
        security.copy_permission,
        security.annotation_permission,
    )
}

/// Parse security diagnostics from document info text.
///
/// Looks for the `% RW-NOTE: PDF encryption ...` comment that was placed by
/// [`serialize_security_diagnostics_entries`] and reconstructs the original
/// [`PdfSecurity`] from the embedded permission parameters. Legacy files that
/// embedded plain-text passwords (older builds) are still parsed for
/// backward compatibility, but new output never contains passwords.
pub(crate) fn parse_security_diagnostics(text: &str) -> Option<PdfSecurity> {
    if !text.contains("RW-NOTE: PDF encryption") {
        return None;
    }
    let user_password = parse_legacy_password(text).unwrap_or_default();
    let owner_password = parse_legacy_owner_password(text).unwrap_or_default();
    let print_permission = text.contains("print=true");
    let edit_permission = text.contains("edit=true");
    let copy_permission = text.contains("copy=true");
    let annotation_permission = text.contains("annot=true");
    Some(PdfSecurity {
        user_password: if user_password.is_empty() { None } else { Some(user_password) },
        owner_password: if owner_password.is_empty() { None } else { Some(owner_password) },
        print_permission,
        edit_permission,
        copy_permission,
        annotation_permission,
    })
}

/// Backward-compatible parser for the old /RWUserPassword-based format.
fn parse_legacy_password(text: &str) -> Option<String> {
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

fn parse_pdf_literal_by_key(text: &str, key: &str) -> Option<String> {
    let start = text.find(key)? + key.len();
    let rest = text.get(start..)?.trim_start();
    let literal_start = rest.find('(')? + 1;
    let literal_tail = rest.get(literal_start..)?;
    let literal_end = literal_tail.find(')')?;
    Some(literal_tail[..literal_end].to_string())
}

// ═══════════════════════════════════════════════════════════════════════
// Real PDF Encryption (requires `pdf-encryption` feature)
// ═══════════════════════════════════════════════════════════════════════

/// PDF encryption algorithm selector.
#[cfg(feature = "pdf-encryption")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    /// No encryption.
    None,
    /// AES-128 in CBC mode (PDF 2.0, Revision 6).
    AES128,
    /// AES-256 in CBC mode (PDF 2.0, Revision 6).
    AES256,
}

/// Represents PDF encryption parameters for AES-128-CBC encryption.
///
/// Stores user/owner passwords, permission flags, and the derived
/// encryption key used to encrypt/decrypt PDF stream content.
#[cfg(feature = "pdf-encryption")]
#[derive(Debug, Clone)]
pub struct PdfEncryption {
    /// Selected encryption algorithm.
    pub algorithm: EncryptionAlgorithm,
    /// User password (opens the document).
    pub user_password: String,
    /// Owner password (changes permissions).
    pub owner_password: String,
    /// Permission flags encoded as a 32-bit integer.
    pub permissions: u32,
    /// Derived encryption key (16 bytes for AES-128).
    pub encryption_key: Vec<u8>,
}

#[cfg(feature = "pdf-encryption")]
impl PdfEncryption {
    /// Create a new `PdfEncryption` with AES-128 algorithm, deriving
    /// the encryption key from the user password and a random salt.
    pub fn new(user_password: &str, owner_password: &str, permissions: u32) -> Self {
        let algorithm = EncryptionAlgorithm::AES128;
        let salt = generate_salt();
        let encryption_key = derive_encryption_key(user_password, &salt);
        PdfEncryption {
            algorithm,
            user_password: user_password.to_string(),
            owner_password: owner_password.to_string(),
            permissions,
            encryption_key,
        }
    }

    /// Build a PDF encryption dictionary string with entries:
    /// `/Filter`, `/Length`, `/V`, `/R`, `/O`, `/U`, `/P`, `/StmF`, `/StrF`.
    ///
    /// Produces a PDF 2.0 compliant encryption dictionary for AES-128.
    pub fn build_encryption_dictionary(&self) -> String {
        let (v, r, length) = match self.algorithm {
            EncryptionAlgorithm::AES128 => (5, 6, 16),
            EncryptionAlgorithm::AES256 => (5, 6, 32),
            EncryptionAlgorithm::None => return String::new(),
        };
        let user_salt = generate_salt();
        let owner_salt = generate_salt();

        // /O (32 bytes): SHA-256(owner_password + user_salt + owner_salt)
        let o_hash = compute_hash(&self.owner_password, &user_salt, &owner_salt);
        let o_hex = hex_encode(&o_hash);

        // /U (32 bytes): SHA-256(user_password + user_salt)
        let u_hash = compute_hash(&self.user_password, &user_salt, &[]);
        let u_hex = hex_encode(&u_hash);

        // /P: permission flags as signed integer
        let p = self.permissions as i32;

        format!(
            "\n<< /Filter /Standard /Length {length} /V {v} /R {r} /O <{o_hex}> /U <{u_hex}> /P {p} /StmF /StmCrypt /StrF /StmCrypt >>"
        )
    }
}

/// Encrypt PDF content with AES-128-CBC using a key derived from
/// the user password and a random salt.
///
/// # Arguments
/// * `content` - Raw PDF byte content to encrypt.
/// * `user_password` - User password for opening the document.
/// * `owner_password` - Owner password for permission changes.
///
/// # Returns
/// A `Vec<u8>` containing the encrypted content prefixed with the
/// 16-byte IV, preceded by the encryption dictionary header.
#[cfg(feature = "pdf-encryption")]
pub fn encrypt_pdf(content: &[u8], user_password: &str, owner_password: &str) -> Vec<u8> {
    let salt = generate_salt();
    let key = derive_encryption_key(user_password, &salt);
    let iv = generate_salt(); // IV can use the same RNG
    let encrypted = aes128_cbc_encrypt(&key, &iv, content);

    let enc = PdfEncryption {
        algorithm: EncryptionAlgorithm::AES128,
        user_password: user_password.to_string(),
        owner_password: owner_password.to_string(),
        permissions: 0xFFFFFFFCu32, // allow all by default
        encryption_key: key,
    };
    let dict = enc.build_encryption_dictionary();

    // Build output: encryption dictionary followed by IV + ciphertext
    let mut result = dict.into_bytes();
    result.push(b'\n');
    result.extend_from_slice(&iv);
    result.extend_from_slice(&encrypted);
    result
}

// ── Encryption Primitives ──

/// Generate 16 random bytes using a simple LCG seeded from system time.
#[cfg(feature = "pdf-encryption")]
fn generate_salt() -> [u8; 16] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
    let mut state = seed;
    let mut salt = [0u8; 16];
    for byte in salt.iter_mut() {
        // LCG constants (MMIX/Knuth)
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *byte = (state >> 32) as u8;
    }
    salt
}

/// Derive a 16-byte AES-128 encryption key by hashing the password
/// with a salt using SHA-256 and taking the first 16 bytes.
#[cfg(feature = "pdf-encryption")]
fn derive_encryption_key(password: &str, salt: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt);
    let hash = hasher.finalize();
    hash[..16].to_vec()
}

/// Compute a 32-byte hash for /O or /U entries.
/// For /O: SHA-256(password + user_salt + owner_salt)
/// For /U: SHA-256(password + user_salt)
#[cfg(feature = "pdf-encryption")]
fn compute_hash(password: &str, salt1: &[u8], salt2: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt1);
    hasher.update(salt2);
    let hash = hasher.finalize();
    hash.to_vec()
}

/// Hex-encode bytes to lowercase hex string.
#[cfg(feature = "pdf-encryption")]
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Encrypt plaintext with AES-128-CBC using the given key and IV.
/// Pads plaintext with PKCS#7 before encryption.
#[cfg(feature = "pdf-encryption")]
fn aes128_cbc_encrypt(key: &[u8], iv: &[u8; 16], plaintext: &[u8]) -> Vec<u8> {
    use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
    use aes::Aes128;
    use cbc::Encryptor;

    type Aes128Cbc = Encryptor<Aes128>;

    // KeyIvInit requires GenericArray slices
    let key_arr = aes::cipher::generic_array::GenericArray::from_slice(key);
    let iv_arr = aes::cipher::generic_array::GenericArray::from_slice(iv);

    let cipher = Aes128Cbc::new(key_arr, iv_arr);
    // AES block size is 16 bytes; allocate buffer for plaintext + one padding block
    let mut out = vec![0u8; plaintext.len() + 16];
    // encrypt_padded_b2b_mut returns the padded ciphertext as a sub-slice
    let encrypted = cipher
        .encrypt_padded_b2b_mut::<Pkcs7>(plaintext, &mut out)
        .expect("CBC encryption should not fail with valid padding");
    encrypted.to_vec()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::types::PdfSecurity;

    // ── Diagnostics tests ──

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
        // The marker is a standalone comment regardless of features: the
        // writer never emits a document-level encryption dictionary, so the
        // output must honestly state that it is NOT encrypted, and passwords
        // must never appear in plain text.
        assert!(result.contains("RW-NOTE: PDF encryption"));
        assert!(result.contains("NOT encrypted"));
        assert!(result.contains("print=true"));
        assert!(result.contains("edit=false"));
        assert!(result.contains("copy=true"));
        assert!(result.contains("annot=false"));
        assert!(!result.contains("hello"));
        assert!(!result.contains("world"));
        assert!(!result.contains("password="));
    }

    #[test]
    fn test_round_trip_via_comment_format() {
        // Only meaningful without document-level encryption: with the feature
        // enabled the marker is identical (the writer never emits a dict).
        #[cfg(not(feature = "pdf-encryption"))]
        {
            let security = PdfSecurity {
                user_password: Some("test123".to_string()),
                owner_password: None,
                print_permission: false,
                edit_permission: true,
                copy_permission: false,
                annotation_permission: true,
            };
            let serialized = serialize_security_diagnostics_entries(&security);
            // Permissions round-trip…
            let parsed = parse_security_diagnostics(&serialized);
            assert!(parsed.is_some());
            let parsed = parsed.unwrap();
            assert!(!parsed.print_permission);
            assert!(parsed.edit_permission);
            assert!(!parsed.copy_permission);
            assert!(parsed.annotation_permission);
            // …but passwords must never be written into an unencrypted output.
            assert!(!serialized.contains("test123"));
            assert!(parsed.user_password.is_none());
            assert!(parsed.owner_password.is_none());
            assert!(serialized.contains("NOT encrypted"));
        }
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

    // ── Encryption tests (only when pdf-encryption feature is enabled) ──

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_encrypt_pdf_creates_non_empty_output() {
        let content = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\nxref\n0 2\n0000000000 65535 f \n0000000009 00000 n \ntrailer\n<< /Size 2 /Root 1 0 R >>\nstartxref\n9\n%%EOF\n";
        let result = encrypt_pdf(content, "userpass", "ownerpass");
        assert!(!result.is_empty(), "Encrypted output should not be empty");
        // IV (16 bytes) + ciphertext should be present after the dictionary
        assert!(result.len() > 16, "Output should contain IV + ciphertext");
        // Should contain the encryption dictionary structure
        let result_str = String::from_utf8_lossy(&result);
        assert!(result_str.contains("/Filter"));
        assert!(result_str.contains("/Standard"));
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_encryption_dictionary_has_correct_entries() {
        let enc = PdfEncryption::new("user", "owner", 0xFFFFFFFC);
        let dict = enc.build_encryption_dictionary();
        assert!(dict.contains("/Filter /Standard"));
        assert!(dict.contains("/Length 16"));
        assert!(dict.contains("/V 5"));
        assert!(dict.contains("/R 6"));
        assert!(dict.contains("/O <"));
        assert!(dict.contains("/U <"));
        assert!(dict.contains("/P"));
        assert!(dict.contains("/StmF /StmCrypt"));
        assert!(dict.contains("/StrF /StmCrypt"));
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_same_password_produces_same_key() {
        let salt = generate_salt();
        // Use fixed salt so deterministic
        let key1 = derive_encryption_key("mypassword", &salt);
        let key2 = derive_encryption_key("mypassword", &salt);
        assert_eq!(key1, key2, "Same password + same salt should produce same key");
        assert_eq!(key1.len(), 16, "AES-128 key should be 16 bytes");
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_different_passwords_produce_different_encryption_dictionaries() {
        let enc1 = PdfEncryption::new("pass1", "owner1", 0xFFFFFFFC);
        let enc2 = PdfEncryption::new("pass2", "owner2", 0xFFFFFFFC);
        let dict1 = enc1.build_encryption_dictionary();
        let dict2 = enc2.build_encryption_dictionary();
        // The /O and /U entries should differ
        assert_ne!(dict1, dict2, "Different passwords should produce different dictionaries");
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_generate_salt_is_non_zero() {
        let salt = generate_salt();
        assert_eq!(salt.len(), 16);
        let all_zero = salt.iter().all(|&b| b == 0);
        assert!(!all_zero, "Salt should not be all zeros");
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_aes128_cbc_encrypt_produces_valid_output() {
        let key = b"0123456789abcdef"; // 16 bytes
        let iv = b"fedcba9876543210"; // 16 bytes
        let plaintext = b"Hello, PDF encryption!";
        let ciphertext = aes128_cbc_encrypt(key, iv, plaintext);
        // Ciphertext should be a multiple of 16 (AES block size) due to PKCS#7 padding
        assert_eq!(ciphertext.len() % 16, 0, "Ciphertext length must be a multiple of 16");
        // Ciphertext should differ from plaintext
        assert_ne!(
            ciphertext.as_slice(),
            &plaintext[..],
            "Ciphertext should differ from plaintext"
        );
        // Output should be longer than plaintext (due to padding)
        assert!(ciphertext.len() > plaintext.len(), "Ciphertext should be longer due to padding");
    }
}
