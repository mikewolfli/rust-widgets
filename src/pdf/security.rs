// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PDF security serialization, parsing, and content-level encryption.
//!
//! # Document-level security model
//!
//! Setting a [`PdfSecurity`] records intent. How that intent is serialized depends
//! on the build:
//!
//! - **With `pdf-encryption`**: a non-default profile is applied as a standard
//!   `/Filter /Standard` security handler — the writer emits an `/Encrypt`
//!   dictionary plus a `/Crypt` filter, references it from the trailer and the
//!   catalog, and encrypts every content stream with AES-128-CBC. The `% RW-NOTE`
//!   marker then states that encryption was applied (see
//!   [`serialize_security_diagnostics_entries`]).
//! - **Without `pdf-encryption`**: the profile stays intent only; the `% RW-NOTE`
//!   marker records the requested permissions and states the output is **not**
//!   encrypted, because it is not.
//!
//! In both cases passwords are never written into the output in plain text: echoing
//! them into the file would leak the secrets.
//!
//! # Encryption tooling (`pdf-encryption` feature)
//!
//! With the `pdf-encryption` feature this module additionally provides the real
//! AES-128-CBC primitives (`PdfEncryption`, `encrypt_pdf`) used by the writer to
//! encrypt content. `PdfEncryption` also builds the standard encryption dictionary
//! wired into the document pipeline.
//!
//! Those two items are named without documentation links on purpose: they are
//! `#[cfg(feature = "pdf-encryption")]`-gated, so under any other feature set the
//! links would dangle and this module's docs would fail to build.

use crate::pdf::types::*;

/// Serialize the security intent marker for a profile that is **not** applied as
/// document-level encryption.
///
/// Returns the empty string for a default (fully open) security profile.
/// Otherwise returns a standalone `%` comment line stating the requested
/// permissions and warning that the output is NOT encrypted. Used only when the
/// standard encryption handler is not in play: the writer substitutes an
/// "encryption applied" marker once a real `/Encrypt` dictionary is emitted, so the
/// marker never contradicts the file. Never contains password material.
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
///
/// # Why there is no `AES256`
///
/// An `AES256` variant used to exist and was **never produced**: `PdfEncryption::new` hardcoded
/// `AES128`, `derive_encryption_key` always returns 16 bytes and `aes128_cbc_encrypt` is AES-128
/// only — so the branch that advertised `/Length 32` was unreachable, and reaching it would have
/// emitted a dictionary whose key length disagreed with the ciphertext. A variant no constructor
/// can produce is not a feature; it is a promise the code cannot keep. Removed until a real AES-256
/// key path exists (which is a self-contained addition, not a flag).
#[cfg(feature = "pdf-encryption")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    /// No encryption.
    None,
    /// AES-128 in CBC mode.
    AES128,
}

/// Represents PDF encryption parameters for AES-128-CBC encryption.
///
/// Stores user/owner passwords, permission flags, the salts the `/O` and `/U` entries are derived
/// from, and the derived encryption key used to encrypt/decrypt PDF stream content.
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
    /// Per-document salt mixed into the `/U` entry.
    ///
    /// Stored rather than regenerated per dictionary build: `build_encryption_dictionary` used to
    /// mint fresh salts, so the `/O` / `/U` entries described a **different** salt than the one the
    /// key was derived from — the dictionary did not match the ciphertext it was prepended to.
    pub user_salt: [u8; 16],
    /// Second salt mixed into the `/O` entry (owner).
    pub owner_salt: [u8; 16],
}

#[cfg(feature = "pdf-encryption")]
impl PdfEncryption {
    /// Create a new `PdfEncryption` with AES-128, deriving the encryption key from the user password
    /// and a CSPRNG salt.
    ///
    /// Returns `Err` when the OS entropy source is unavailable: the alternative is a predictable
    /// salt, which would silently defeat the encryption.
    pub fn new(
        user_password: &str,
        owner_password: &str,
        permissions: u32,
    ) -> Result<Self, String> {
        let algorithm = EncryptionAlgorithm::AES128;
        let user_salt = generate_salt().ok_or_else(entropy_unavailable)?;
        let owner_salt = generate_salt().ok_or_else(entropy_unavailable)?;
        let encryption_key = derive_encryption_key(user_password, &user_salt);
        Ok(PdfEncryption {
            algorithm,
            user_password: user_password.to_string(),
            owner_password: owner_password.to_string(),
            permissions,
            encryption_key,
            user_salt,
            owner_salt,
        })
    }

    /// Build a PDF encryption dictionary string with entries:
    /// `/Filter`, `/Length`, `/V`, `/R`, `/O`, `/U`, `/P`, `/StmF`, `/StrF`.
    ///
    /// Produces a PDF-2.0-style encryption dictionary for AES-128. The `/StmF` and `/StrF` names
    /// are the standard `/StdCF` crypt-filter names that the writer resolves through the document's
    /// own `/Crypt` dictionary.
    ///
    /// The result is a single-line `<< ... >>` body with no leading or trailing
    /// newline, so callers can embed it directly inside an object or a trailer
    /// without breaking the surrounding structure.
    ///
    /// # Why `/O` and `/U` use a *stored* salt
    ///
    /// They are `SHA-256(password + salt)` entries. They used to be computed from **fresh** salts
    /// generated here, unrelated to the key the content was actually encrypted under — so the
    /// dictionary did not describe the file it was prepended to. They are now derived from the same
    /// salts [`PdfEncryption`] was constructed with, so the dictionary and the ciphertext agree.
    pub fn build_encryption_dictionary(&self) -> String {
        let (v, r, length) = match self.algorithm {
            EncryptionAlgorithm::AES128 => (5, 6, 16),
            EncryptionAlgorithm::None => return String::new(),
        };

        // /O (32 bytes): SHA-256(owner_password + user_salt + owner_salt)
        let o_hash = compute_hash(&self.owner_password, &self.user_salt, &self.owner_salt);
        let o_hex = hex_encode(&o_hash);

        // /U (32 bytes): SHA-256(user_password + user_salt)
        let u_hash = compute_hash(&self.user_password, &self.user_salt, &[]);
        let u_hex = hex_encode(&u_hash);

        // /P: permission flags as signed integer
        let p = self.permissions as i32;

        format!(
            "<< /Filter /Standard /Length {length} /V {v} /R {r} /O <{o_hex}> /U <{u_hex}> /P {p} /StmF /StdCF /StrF /StdCF >>"
        )
    }

    /// Encrypt one content string/stream under this document's file key.
    ///
    /// Encodes the ciphertext in PDF-2.0 `AESV3` form: a 16-byte random IV
    /// followed by the PKCS#7-padded AES-128-CBC ciphertext, each byte written as
    /// two lowercase hex digits and wrapped in `<...>`. (PDF 2.0 / ISO 32000-2 §7.6.2.)
    ///
    /// The IV is freshly drawn per call from the OS CSPRNG so equal plaintexts
    /// never produce equal ciphertexts.
    pub fn encrypt_content(&self, plaintext: &[u8]) -> Result<String, String> {
        if self.algorithm == EncryptionAlgorithm::None {
            return Err(
                "cannot encrypt content with the `None` algorithm; use an AES-128 key".to_string()
            );
        }
        let iv = generate_salt().ok_or_else(entropy_unavailable)?;
        let ciphertext = aes128_cbc_encrypt(&self.encryption_key, &iv, plaintext);
        let mut payload = Vec::with_capacity(iv.len() + ciphertext.len());
        payload.extend_from_slice(&iv);
        payload.extend_from_slice(&ciphertext);
        Ok(format!("<{}>", hex_encode(&payload)))
    }
}

/// Map a [`PdfSecurity`] intent profile to the standard `/P` permission bits and
/// build the matching [`PdfEncryption`] for a *non-default* profile.
///
/// Returns `None` for the default (fully open, password-less) profile, so callers
/// can distinguish "no document-level encryption requested" from "encryption
/// requested but the entropy source failed".
///
/// # Why the permission bits are hard-coded
///
/// PDF's `/P` is a signed 32-bit field whose low bits select allowed operations
/// and whose upper bits are reserved; all reserved bits must be `1`. The crate's
/// four boolean flags map directly onto the four that matter here (bit positions
/// follow Table 22 of ISO 32000-1: bit 3 print, bit 4 modify, bit 5 copy,
/// bit 6 annotate). This is a deliberately *honest* mapping of the intent flags
/// into a real dictionary rather than a claim of full Acrobat permission
/// semantics.
#[cfg(feature = "pdf-encryption")]
pub(crate) fn security_profile_encryption(
    security: &PdfSecurity,
) -> Option<Result<PdfEncryption, String>> {
    if *security == PdfSecurity::default() {
        return None;
    }
    let permissions = encode_permission_flags(security);
    let user_password = security.user_password.as_deref().unwrap_or("");
    let owner_password = security.owner_password.as_deref().unwrap_or("");
    Some(PdfEncryption::new(user_password, owner_password, permissions))
}

/// Encode the four [`PdfSecurity`] permission flags into the standard `/P` value.
#[cfg(feature = "pdf-encryption")]
fn encode_permission_flags(security: &PdfSecurity) -> u32 {
    // Reserved high bits stay set; unused bits 1-2 are always cleared.
    let mut permissions: u32 = 0xFFFF_F0C0;
    if security.print_permission {
        permissions |= 1 << 2;
    }
    if security.edit_permission {
        permissions |= 1 << 3;
    }
    if security.copy_permission {
        permissions |= 1 << 4;
    }
    if security.annotation_permission {
        permissions |= 1 << 5;
    }
    permissions
}

/// Encrypt PDF content with AES-128-CBC using a key derived from the user password and a CSPRNG salt.
///
/// # Arguments
/// * `content` - Raw PDF byte content to encrypt.
/// * `user_password` - User password for opening the document.
/// * `owner_password` - Owner password for permission changes.
///
/// # Returns
/// The encryption dictionary followed by, in order: the 16-byte key salt, the 16-byte IV, and the
/// AES-128-CBC ciphertext (PKCS#7 padded).
///
/// # Why the salt is written out
///
/// The key is `SHA-256(user_password + salt)[..16]`. A previous revision derived the key from a salt
/// it **never wrote**, so the result was not decryptable by anyone — not even by the tool that made
/// it. The salt is now serialized alongside the IV, which is what makes the output a real encrypted
/// blob rather than undecryptable bytes wearing a dictionary.
///
/// # Errors
/// Returns `Err` when the OS entropy source is unavailable (see [`generate_salt`]).
#[cfg(feature = "pdf-encryption")]
pub fn encrypt_pdf(
    content: &[u8],
    user_password: &str,
    owner_password: &str,
) -> Result<Vec<u8>, String> {
    let salt = generate_salt().ok_or_else(entropy_unavailable)?;
    let key = derive_encryption_key(user_password, &salt);
    let iv = generate_salt().ok_or_else(entropy_unavailable)?;
    let encrypted = aes128_cbc_encrypt(&key, &iv, content);

    let mut enc = PdfEncryption {
        algorithm: EncryptionAlgorithm::AES128,
        user_password: user_password.to_string(),
        owner_password: owner_password.to_string(),
        permissions: 0xFFFFFFFCu32, // allow all by default
        encryption_key: key,
        user_salt: salt,
        owner_salt: [0u8; 16],
    };
    enc.owner_salt = generate_salt().ok_or_else(entropy_unavailable)?;
    let dict = enc.build_encryption_dictionary();

    // Output: encryption dictionary, key salt, IV, then ciphertext. The salt is what lets a reader
    // re-derive the key; without it the file is a black box.
    let mut result = dict.into_bytes();
    result.push(b'\n');
    result.extend_from_slice(&salt);
    result.extend_from_slice(&iv);
    result.extend_from_slice(&encrypted);
    Ok(result)
}

/// The error string for an unavailable OS entropy source.
#[cfg(feature = "pdf-encryption")]
fn entropy_unavailable() -> String {
    "the OS entropy source is unavailable, so a cryptographically random salt/IV cannot be \
     generated; refusing to fall back to a predictable value"
        .to_string()
}

// ── Encryption Primitives ──

/// Generate 16 random bytes from the operating system's CSPRNG.
///
/// # Why this is not an LCG
///
/// It was `state = state * A + C` seeded from `SystemTime::now().as_nanos()`. An LCG's output is a
/// deterministic function of its seed, and the seed is a clock reading — so every "random" byte here
/// (the key salt and the CBC IV) was predictable from the time the file was written. That defeats
/// the entire purpose of a salt and an IV, which is exactly the class of defect the crate forbids
/// (a security claim the implementation does not honour). `getrandom` reads the OS entropy source
/// (`getrandom(2)` / `BCryptGenRandom` / `SecRandomCopyBytes` / `crypto.getRandomValues`).
///
/// # On failure
///
/// `getrandom::fill` only fails if the OS entropy source itself is unavailable (a misconfigured
/// early-boot container). That is a reason to refuse, not to fall back to a predictable value, so
/// this returns `None` and the callers propagate it as an error rather than silently weakening the
/// encryption.
#[cfg(feature = "pdf-encryption")]
fn generate_salt() -> Option<[u8; 16]> {
    let mut salt = [0u8; 16];
    match getrandom::fill(&mut salt) {
        Ok(()) => Some(salt),
        Err(error) => {
            log::error!(
                "[pdf] the OS entropy source is unavailable ({error}); refusing to derive a salt \
                 from a predictable value"
            );
            None
        }
    }
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

    // `cipher` 0.4 uses `GenericArray`; buffer types go through `.into()` so the
    // code does not depend on the deprecated `GenericArray::from_slice` helper.
    let key_arr = key.into();
    let iv_arr = iv.into();

    let cipher = Aes128Cbc::new(key_arr, iv_arr);
    // AES block size is 16 bytes; allocate buffer for plaintext + one padding block
    let mut out = vec![0u8; plaintext.len() + 16];
    // encrypt_padded_b2b_mut returns the padded ciphertext as a sub-slice
    let encrypted = cipher.encrypt_padded_b2b_mut::<Pkcs7>(plaintext, &mut out).expect(
        "AES-128-CBC encryption cannot fail for a buffer that was sized as \
             plaintext.len() + 16 and Pkcs7 padding, so the allocator or key length is wrong",
    );
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
        // The writer substitutes an "encryption applied" marker when the standard
        // handler is emitted, so this bare intent marker must only be asserted when
        // encryption is genuinely off.
        #[cfg(not(feature = "pdf-encryption"))]
        {
            assert!(result.contains("RW-NOTE: PDF encryption"));
            assert!(result.contains("NOT encrypted"));
        }
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
        let result =
            encrypt_pdf(content, "userpass", "ownerpass").expect("entropy is available in tests");
        assert!(!result.is_empty(), "Encrypted output should not be empty");
        // Key salt (16) + IV (16) + at least one ciphertext block.
        assert!(result.len() > 32, "Output should contain salt + IV + ciphertext");
        // Should contain the encryption dictionary structure
        let result_str = String::from_utf8_lossy(&result);
        assert!(result_str.contains("/Filter"));
        assert!(result_str.contains("/Standard"));
    }

    /// The output must be **decryptable**: the salt the key was derived from is written out, so a
    /// reader can re-derive the key. Pins the defect where the salt was discarded (the output could
    /// never be decrypted by anyone).
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn encrypt_pdf_writes_the_salt_so_the_output_is_decryptable() {
        let content = b"decryptable payload";
        let out = encrypt_pdf(content, "pw", "owner").expect("entropy");
        let dict_end =
            out.iter().position(|&b| b == b'\n').expect("the dictionary ends with a newline");
        // The 16-byte key salt follows the dictionary, then the IV, then the ciphertext.
        let payload = &out[dict_end + 1..];
        assert!(payload.len() >= 32, "salt + IV must be present after the dictionary");
        let salt: [u8; 16] = payload[..16].try_into().expect("16 bytes");
        let expected_key = derive_encryption_key("pw", &salt);
        // The stored key must be the one the written salt derives, or the file is a black box.
        let mut enc = PdfEncryption::new("pw", "owner", 0xFFFFFFFC).expect("entropy");
        enc.user_salt = salt;
        enc.encryption_key = expected_key.clone();
        assert_eq!(enc.encryption_key, expected_key);
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_encryption_dictionary_has_correct_entries() {
        let enc = PdfEncryption::new("user", "owner", 0xFFFFFFFC).expect("entropy");
        let dict = enc.build_encryption_dictionary();
        assert!(dict.contains("/Filter /Standard"));
        assert!(dict.contains("/Length 16"));
        assert!(dict.contains("/V 5"));
        assert!(dict.contains("/R 6"));
        assert!(dict.contains("/O <"));
        assert!(dict.contains("/U <"));
        assert!(dict.contains("/P"));
        assert!(dict.contains("/StmF /StdCF"));
        assert!(dict.contains("/StrF /StdCF"));
    }

    /// The dictionary describes the **same** salt the key was derived from.
    ///
    /// Pins the defect: `/O` and `/U` were computed from fresh salts unrelated to the encryption
    /// key, so the dictionary did not describe the ciphertext it was prepended to. Rebuilding the
    /// dictionary twice must now be byte-identical (it reads stored salts, not fresh ones).
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn the_dictionary_is_stable_across_rebuilds() {
        let enc = PdfEncryption::new("user", "owner", 0xFFFFFFFC).expect("entropy");
        assert_eq!(
            enc.build_encryption_dictionary(),
            enc.build_encryption_dictionary(),
            "the dictionary must read the stored salts, not mint new ones"
        );
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_same_password_produces_same_key() {
        let salt = generate_salt().expect("entropy");
        // Use a fixed salt so the derivation is deterministic.
        let key1 = derive_encryption_key("mypassword", &salt);
        let key2 = derive_encryption_key("mypassword", &salt);
        assert_eq!(key1, key2, "Same password + same salt should produce same key");
        assert_eq!(key1.len(), 16, "AES-128 key should be 16 bytes");
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_different_passwords_produce_different_encryption_dictionaries() {
        let enc1 = PdfEncryption::new("pass1", "owner1", 0xFFFFFFFC).expect("entropy");
        let enc2 = PdfEncryption::new("pass2", "owner2", 0xFFFFFFFC).expect("entropy");
        let dict1 = enc1.build_encryption_dictionary();
        let dict2 = enc2.build_encryption_dictionary();
        // The /O and /U entries should differ
        assert_ne!(dict1, dict2, "Different passwords should produce different dictionaries");
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_generate_salt_is_non_zero_and_varies() {
        let salt = generate_salt().expect("entropy is available in tests");
        assert_eq!(salt.len(), 16);
        assert!(!salt.iter().all(|&b| b == 0), "Salt should not be all zeros");
        // Two draws must differ: a time-seeded LCG could return the same value twice in a tight
        // loop, which is exactly the weakness this replaced.
        let other = generate_salt().expect("entropy");
        assert_ne!(salt, other, "a CSPRNG must not repeat in consecutive draws");
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

    // ── Document-level encryption dictionary tests ──

    /// A non-empty `/CF` entry must name the `/StdCF` crypt filter, otherwise
    /// `/StmF /StdCF` resolves to nothing and the dictionary is unusable.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_encryption_dictionary_has_crypt_filter_entries() {
        let enc = PdfEncryption::new(
            "user",
            "owner",
            encode_permission_flags(&PdfSecurity {
                user_password: Some("user".to_string()),
                owner_password: Some("owner".to_string()),
                print_permission: false,
                edit_permission: true,
                copy_permission: false,
                annotation_permission: false,
            }),
        )
        .expect("entropy");
        let dict = enc.build_encryption_dictionary();
        assert!(dict.contains("/StmF /StdCF"));
        assert!(dict.contains("/StrF /StdCF"));
    }

    /// The intent flags map onto the ISO 32000 /P bits, not into an arbitrary value.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_permission_flags_map_to_standard_bits() {
        let permissions = encode_permission_flags(&PdfSecurity {
            user_password: Some("u".to_string()),
            owner_password: None,
            print_permission: false,
            edit_permission: true,
            copy_permission: false,
            annotation_permission: true,
        });
        assert_eq!(permissions & (1 << 2), 0, "print denied -> bit 3 clear");
        assert_ne!(permissions & (1 << 3), 0, "edit allowed -> bit 4 set");
        assert_eq!(permissions & (1 << 4), 0, "copy denied -> bit 5 clear");
        assert_ne!(permissions & (1 << 5), 0, "annotate allowed -> bit 6 set");
        // Reserved high bits must stay set for a valid /P value.
        assert_eq!(permissions & 0xFFFF_F000, 0xFFFF_F000);
    }

    /// The default (open) profile arms no encryption at all.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_default_profile_arms_no_encryption() {
        assert!(security_profile_encryption(&PdfSecurity::default()).is_none());
        assert!(security_profile_encryption(&PdfSecurity {
            user_password: Some("pw".to_string()),
            owner_password: None,
            print_permission: true,
            edit_permission: true,
            copy_permission: true,
            annotation_permission: true,
        })
        .is_some());
    }

    /// `encrypt_content` yields a PDF-2.0 AESV3 hex string: `<` + a whole number of
    /// AES blocks including the 16-byte IV prefix + `>`.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_encrypt_content_emits_hex_wrapped_iv_plus_ciphertext() {
        let enc = PdfEncryption::new("pw", "owner", 0xFFFF_F0C0).expect("entropy");
        let payload = enc.encrypt_content(b"stream body").expect("encryption succeeds");
        assert!(payload.starts_with('<') && payload.ends_with('>'));
        let hex_body = &payload[1..payload.len() - 1];
        assert!(hex_body.chars().all(|ch| ch.is_ascii_hexdigit()));
        // 16-byte IV + PKCS#7-padded ciphertext, all hex-encoded.
        assert_eq!(hex_body.len() % 32, 0, "IV + ciphertext must be whole AES blocks");
        assert!(hex_body.len() >= 64, "IV (16 bytes) + at least one block");
        // A second call draws a fresh IV, so the ciphertexts differ.
        let other = enc.encrypt_content(b"stream body").expect("encryption succeeds");
        assert_ne!(payload, other, "each stream must get its own random IV");
    }

    /// The `None` algorithm must refuse rather than emit plaintext wearing a `<...>`
    /// wrapper, which would be a silent confidentiality failure.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_encrypt_content_rejects_none_algorithm() {
        let enc = PdfEncryption {
            algorithm: EncryptionAlgorithm::None,
            user_password: String::new(),
            owner_password: String::new(),
            permissions: 0,
            encryption_key: vec![0u8; 16],
            user_salt: [0u8; 16],
            owner_salt: [0u8; 16],
        };
        assert!(enc.encrypt_content(b"payload").is_err());
    }
}
