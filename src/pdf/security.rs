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
//!   dictionary with a direct `/CF` crypt filter, references it from the trailer,
//!   and encrypts every stream and string with AES-256-CBC (PDF 2.0, V5/R6,
//!   `/CFM /AESV3`), with an `% RW-NOTE` marker stating that encryption was applied.
//! - **Without `pdf-encryption`**: the writer **refuses** a non-default profile with
//!   an `Unsupported` error and writes nothing. It used to write the document with a
//!   `% RW-NOTE` line saying it was not encrypted, which reported success for a
//!   security request that was not honoured (R-30).
//!
//! In both cases passwords are never written into the output in plain text: echoing
//! them into the file would leak the secrets. `parse_security_diagnostics` still
//! reads the intent markers older builds wrote, so those files keep loading.
//!
//! # Encryption tooling (`pdf-encryption` feature)
//!
//! With the `pdf-encryption` feature this module additionally provides the real
//! AES-256-CBC primitives (`PdfEncryption`, `encrypt_pdf`) used by the writer to
//! encrypt content. `PdfEncryption` also builds the standard encryption dictionary
//! wired into the document pipeline.
//!
//! Those two items are named without documentation links on purpose: they are
//! `#[cfg(feature = "pdf-encryption")]`-gated, so under any other feature set the
//! links would dangle and this module's docs would fail to build.

use crate::pdf::types::*;

/// Parse security diagnostics from document info text.
///
/// Looks for the `% RW-NOTE: PDF encryption ...` comment that **older builds** placed in a
/// document that requested a profile they could not apply, and reconstructs the original
/// [`PdfSecurity`] from the embedded permission parameters. Current builds refuse such a
/// request instead of writing it (R-30), so this is a reader-only compatibility path for files
/// that already exist. Legacy files that embedded plain-text passwords are still parsed for
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
/// The standard R=6 handler (`/V 5`, `/R 6`, `/CFM /AESV3`) is the only applied algorithm: it is
/// what ISO 32000-2 (PDF 2.0) specifies. R=4/AES-128 was removed with the upgrade rather than kept
/// as a second, weaker path — a file this writer emits must be decryptable by a current reader.
#[cfg(feature = "pdf-encryption")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    /// No encryption.
    None,
    /// AES-256 in CBC mode (PDF 2.0, V5/R6, `/CFM /AESV3`).
    AES256,
}

/// Represents PDF encryption parameters for the standard R=6 handler (AES-256).
///
/// The file key is **random**, not derived from a password: `/U`/`/UE` wrap it for the user
/// password and `/O`/`/OE` for the owner password, exactly as ISO 32000-2 Algorithms 2.A/2.B
/// require. The entries are computed once in [`PdfEncryption::new`], so the dictionary always
/// describes the key the ciphertext was produced with.
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
    /// Random 256-bit file key, wrapped by `/UE` and `/OE`.
    pub encryption_key: Vec<u8>,
    /// The first `/ID` element, written to the trailer.
    pub document_id: [u8; 16],
    /// `/U`: 32-byte password hash ‖ validation salt ‖ key salt.
    u_entry: [u8; 48],
    /// `/O`: 32-byte owner hash ‖ validation salt ‖ key salt.
    o_entry: [u8; 48],
    /// `/UE`: the file key encrypted with the user key salt's intermediate key.
    ue_entry: [u8; 32],
    /// `/OE`: the file key encrypted with the owner key salt's intermediate key.
    oe_entry: [u8; 32],
    /// `/Perms`: the permission word plus the `adb` marker, encrypted with the file key.
    perms_entry: [u8; 16],
}

#[cfg(feature = "pdf-encryption")]
impl PdfEncryption {
    /// Create a new `PdfEncryption` under the standard R=6 handler (AES-256).
    ///
    /// Draws a random 256-bit file key, the document ID and four 8-byte salts from the OS CSPRNG,
    /// then builds `/U`/`/UE` and `/O`/`/OE` with ISO 32000-2 Algorithms 2.A/2.B.
    ///
    /// Returns `Err` when the OS entropy source is unavailable: the alternative is a predictable
    /// file key, which would silently defeat the encryption.
    pub fn new(
        user_password: &str,
        owner_password: &str,
        permissions: u32,
    ) -> Result<Self, String> {
        let mut file_key = [0u8; AES256_KEY_LEN];
        let mut document_id = [0u8; 16];
        let mut u_validation_salt = [0u8; 8];
        let mut u_key_salt = [0u8; 8];
        let mut o_validation_salt = [0u8; 8];
        let mut o_key_salt = [0u8; 8];
        let mut perms_tail = [0u8; 4];
        for bytes in [
            file_key.as_mut_slice(),
            document_id.as_mut_slice(),
            u_validation_salt.as_mut_slice(),
            u_key_salt.as_mut_slice(),
            o_validation_salt.as_mut_slice(),
            o_key_salt.as_mut_slice(),
            perms_tail.as_mut_slice(),
        ] {
            getrandom::fill(bytes).map_err(|_| entropy_unavailable())?;
        }

        let u_entry = compute_u_entry(user_password, &u_validation_salt, &u_key_salt);
        let ue_entry = compute_ue_entry(user_password, &u_key_salt, &file_key);
        let o_entry = compute_o_entry(
            owner_password,
            user_password,
            &o_validation_salt,
            &o_key_salt,
            &u_entry,
        );
        let oe_entry =
            compute_oe_entry(owner_password, user_password, &o_key_salt, &u_entry, &file_key);
        let perms_entry = compute_perms_entry(&file_key, permissions, perms_tail);

        Ok(PdfEncryption {
            algorithm: EncryptionAlgorithm::AES256,
            user_password: user_password.to_string(),
            owner_password: owner_password.to_string(),
            permissions,
            encryption_key: file_key.to_vec(),
            document_id,
            u_entry,
            o_entry,
            ue_entry,
            oe_entry,
            perms_entry,
        })
    }

    /// The first `/ID` element, hex-encoded for the trailer.
    pub fn file_id_hex(&self) -> String {
        hex_encode(&self.document_id)
    }

    /// Verifies a user password against the stored `/U` entry (ISO 32000-2 Algorithm 2.A/2.B).
    pub fn verify_user_password(&self, password: &str) -> bool {
        let hash = hash_r6(password, &self.u_entry[32..40], &[]);
        hash[..] == self.u_entry[..32]
    }

    /// Verifies an owner password against the stored `/O` entry (the `/U` entry is its `udata`).
    pub fn verify_owner_password(&self, password: &str) -> bool {
        let hash = hash_r6(password, &self.o_entry[32..40], &self.u_entry);
        hash[..] == self.o_entry[..32]
    }

    /// Build a PDF encryption dictionary string with entries:
    /// `/Filter`, `/Length`, `/V`, `/R`, `/O`, `/U`, `/OE`, `/UE`, `/P`, `/Perms`,
    /// `/StmF`, `/StrF` and `/CF`.
    ///
    /// Produces the ISO 32000-2 V5/R6 dictionary for AES-256. `/CF` is a **direct** dictionary
    /// whose `/StdCF` entry is the crypt filter `/StmF`/`/StrF` name, which readers resolve without
    /// an extra object indirection.
    ///
    /// The result is a single-line `<< ... >>` body with no leading or trailing newline, so
    /// callers can embed it directly inside an object or a trailer without breaking the
    /// surrounding structure.
    pub fn build_encryption_dictionary(&self) -> String {
        if self.algorithm != EncryptionAlgorithm::AES256 {
            return String::new();
        }
        let p = self.permissions as i32;
        format!(
            "<< /Filter /Standard /Length 256 /V 5 /R 6 /O <{}> /U <{}> /OE <{}> /UE <{}> /P {p} \
             /Perms <{}> /StmF /StdCF /StrF /StdCF /CF << /StdCF << /AuthEvent /DocOpen \
             /CFM /AESV3 /Length 32 >> >> >>",
            hex_encode(&self.o_entry),
            hex_encode(&self.u_entry),
            hex_encode(&self.oe_entry),
            hex_encode(&self.ue_entry),
            hex_encode(&self.perms_entry),
        )
    }

    /// Encrypt one string value, returning PDF hex-string syntax.
    ///
    /// The result is `<IV ‖ AES-256-CBC ciphertext>` under the document's file key, which is how a
    /// reader reconstructs the value. Objects that must stay readable (the encryption dictionary
    /// itself and the trailer's `/ID`) must not be passed through here.
    pub fn encrypt_content(&self, plaintext: &[u8]) -> Result<String, String> {
        let payload = self.encrypt_stream(plaintext)?;
        Ok(format!("<{}>", hex_encode(&payload)))
    }

    /// Encrypt one stream body, returning the raw bytes a `stream ... endstream` carries.
    ///
    /// The result is the 16-byte IV followed by the AES-256-CBC ciphertext (PKCS#7 padded), under
    /// the document's file key. ISO 32000-2 applies the security handler to a stream **after** its
    /// filters, so a filtered stream is encrypted in its encoded form and a reader reverses the
    /// order (decrypt, then decode).
    ///
    /// The IV is freshly drawn per call from the OS CSPRNG.
    pub fn encrypt_stream(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        if self.algorithm != EncryptionAlgorithm::AES256 {
            return Err("cannot encrypt a stream with the `None` algorithm; use the AES-256 key"
                .to_string());
        }
        let iv = generate_salt().ok_or_else(entropy_unavailable)?;
        let ciphertext = aes256_cbc_encrypt(&self.encryption_key, &iv, plaintext);
        let mut payload = Vec::with_capacity(iv.len() + ciphertext.len());
        payload.extend_from_slice(&iv);
        payload.extend_from_slice(&ciphertext);
        Ok(payload)
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

/// Encrypt PDF content under the real R=6 standard handler (AES-256).
///
/// # Arguments
/// * `content` - Raw PDF byte content to encrypt.
/// * `user_password` - User password for opening the document.
/// * `owner_password` - Owner password for permission changes.
///
/// # Returns
/// The encryption dictionary followed by, in order: the 16-byte document ID, the
/// 16-byte IV, and the AES-256-CBC ciphertext (PKCS#7 padded). A consumer recovers the
/// random file key from `/U` + `/UE` (user password) or `/O` + `/OE` (owner password)
/// using ISO 32000-2 Algorithm 2.B.
///
/// # Errors
/// Returns `Err` when the OS entropy source is unavailable (see `generate_salt`).
#[cfg(feature = "pdf-encryption")]
pub fn encrypt_pdf(
    content: &[u8],
    user_password: &str,
    owner_password: &str,
) -> Result<Vec<u8>, String> {
    let enc = PdfEncryption::new(user_password, owner_password, 0xFFFF_FFFC)?;
    let payload = enc.encrypt_stream(content)?;
    let dict = enc.build_encryption_dictionary();

    // Output: encryption dictionary, document ID, IV, then ciphertext. The document ID is
    // kept for the caller's trailer; unlike R=4 it is not an input to the key derivation.
    let mut result = dict.into_bytes();
    result.push(b'\n');
    result.extend_from_slice(&enc.document_id);
    result.extend_from_slice(&payload);
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

/// The 256-bit AES key length / R=6 file-key length, in bytes.
///
/// Kept as a named constant because the key length appears in `/Length 256`, the 32-byte
/// `/UE`/`/OE` payloads and the 32-byte `Perms` clear text; one fact, one spelling.
#[cfg(feature = "pdf-encryption")]
const AES256_KEY_LEN: usize = 32;

/// ISO 32000-2 truncates a password to 127 bytes of UTF-8.
#[cfg(feature = "pdf-encryption")]
fn password_bytes(password: &str) -> &[u8] {
    let bytes = password.as_bytes();
    &bytes[..bytes.len().min(127)]
}

/// ISO 32000-2 Algorithm 2.B: the R=6 password hash.
///
/// Starts with `SHA-256(password ‖ salt ‖ udata)`, then repeatedly encrypts
/// `(password ‖ K ‖ udata)` 64 times with AES-128-CBC and re-hashes the result with
/// SHA-256/384/512 until the termination condition holds. `K` grows to 48/64 bytes on the
/// SHA-384/512 rounds; only the first 32 bytes are returned.
#[cfg(feature = "pdf-encryption")]
fn hash_r6(password: &str, salt: &[u8], udata: &[u8]) -> [u8; 32] {
    let password = password_bytes(password);
    let mut key_material = sha_digest::<sha2::Sha256>(password, salt, udata);
    let mut round = 0u32;
    loop {
        round += 1;
        let mut block =
            Vec::with_capacity(64 * (password.len() + key_material.len() + udata.len()));
        for _ in 0..64 {
            block.extend_from_slice(password);
            block.extend_from_slice(&key_material);
            block.extend_from_slice(udata);
        }
        let key: [u8; 16] = key_material[..16].try_into().expect("K always starts with 16 bytes");
        let iv: [u8; 16] =
            key_material[16..32].try_into().expect("K always carries a 16-byte IV after the key");
        let encrypted = aes128_cbc_encrypt_no_padding(&key, &iv, &block);
        let selector: u32 = encrypted[..16].iter().map(|&byte| u32::from(byte)).sum::<u32>() % 3;
        key_material = match selector {
            0 => sha_digest::<sha2::Sha256>(&encrypted, &[], &[]),
            1 => sha_digest::<sha2::Sha384>(&encrypted, &[], &[]),
            _ => sha_digest::<sha2::Sha512>(&encrypted, &[], &[]),
        };
        let last = u32::from(encrypted[encrypted.len() - 1]);
        if round >= 64 && last <= round - 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(&key_material[..32]);
            return out;
        }
    }
}

/// Hash the three components Algorithm 2.B consumes, with the selected SHA-2 variant.
#[cfg(feature = "pdf-encryption")]
fn sha_digest<D: sha2::Digest + Default>(first: &[u8], second: &[u8], third: &[u8]) -> Vec<u8> {
    let mut hasher = D::new();
    hasher.update(first);
    hasher.update(second);
    hasher.update(third);
    hasher.finalize().to_vec()
}

/// `/U` = password hash ‖ validation salt ‖ key salt (ISO 32000-2 Algorithm 2.A).
#[cfg(feature = "pdf-encryption")]
fn compute_u_entry(user_password: &str, validation_salt: &[u8; 8], key_salt: &[u8; 8]) -> [u8; 48] {
    let hash = hash_r6(user_password, validation_salt, &[]);
    let mut entry = [0u8; 48];
    entry[..32].copy_from_slice(&hash);
    entry[32..40].copy_from_slice(validation_salt);
    entry[40..48].copy_from_slice(key_salt);
    entry
}

/// `/UE` = the file key encrypted under `hash(password, key salt)` (AES-256-CBC, zero IV).
#[cfg(feature = "pdf-encryption")]
fn compute_ue_entry(
    user_password: &str,
    key_salt: &[u8; 8],
    file_key: &[u8; AES256_KEY_LEN],
) -> [u8; 32] {
    let intermediate = hash_r6(user_password, key_salt, &[]);
    let mut entry = [0u8; 32];
    entry.copy_from_slice(&aes256_cbc_encrypt_no_padding(&intermediate, &[0u8; 16], file_key));
    entry
}

/// `/O` = owner-password hash ‖ validation salt ‖ key salt (ISO 32000-2 Algorithm 2.A).
#[cfg(feature = "pdf-encryption")]
fn compute_o_entry(
    owner_password: &str,
    user_password: &str,
    validation_salt: &[u8; 8],
    key_salt: &[u8; 8],
    u_entry: &[u8; 48],
) -> [u8; 48] {
    let owner = if owner_password.is_empty() { user_password } else { owner_password };
    let hash = hash_r6(owner, validation_salt, u_entry);
    let mut entry = [0u8; 48];
    entry[..32].copy_from_slice(&hash);
    entry[32..40].copy_from_slice(validation_salt);
    entry[40..48].copy_from_slice(key_salt);
    entry
}

/// `/OE` = the file key encrypted under the owner's key-salt hash (AES-256-CBC, zero IV).
#[cfg(feature = "pdf-encryption")]
fn compute_oe_entry(
    owner_password: &str,
    user_password: &str,
    key_salt: &[u8; 8],
    u_entry: &[u8; 48],
    file_key: &[u8; AES256_KEY_LEN],
) -> [u8; 32] {
    let owner = if owner_password.is_empty() { user_password } else { owner_password };
    let intermediate = hash_r6(owner, key_salt, u_entry);
    let mut entry = [0u8; 32];
    entry.copy_from_slice(&aes256_cbc_encrypt_no_padding(&intermediate, &[0u8; 16], file_key));
    entry
}

/// `/Perms`: the permission word and the `adb` marker, encrypted with the file key.
///
/// `EncryptMetadata` is always true for this writer, so byte 8 is `T`.
#[cfg(feature = "pdf-encryption")]
fn compute_perms_entry(
    file_key: &[u8; AES256_KEY_LEN],
    permissions: u32,
    tail: [u8; 4],
) -> [u8; 16] {
    let mut clear = [0u8; 16];
    let extended = 0xFFFF_FFFF_0000_0000u64 | u64::from(permissions);
    clear[..8].copy_from_slice(&extended.to_le_bytes());
    clear[8] = b'T';
    clear[9] = b'a';
    clear[10] = b'd';
    clear[11] = b'b';
    clear[12..].copy_from_slice(&tail);
    let mut entry = [0u8; 16];
    entry.copy_from_slice(&aes256_cbc_encrypt_no_padding(file_key, &[0u8; 16], &clear));
    entry
}

/// Hex-encode bytes to lowercase hex string.
///
/// Delegates to the PDF reader's table-driven encoder instead of collecting one
/// `format!` temporary per byte.
#[cfg(feature = "pdf-encryption")]
fn hex_encode(bytes: &[u8]) -> String {
    super::reader::hex_encode_lower(bytes)
}

/// Encrypt with AES-128-CBC and no padding (Algorithm 2.B's inner loop).
#[cfg(feature = "pdf-encryption")]
fn aes128_cbc_encrypt_no_padding(key: &[u8; 16], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    use aes::cipher::{block_padding::NoPadding, BlockModeEncrypt, KeyIvInit};
    use aes::Aes128;
    use cbc::Encryptor;

    type Aes128Cbc = Encryptor<Aes128>;
    let cipher = Aes128Cbc::new(&(*key).into(), &(*iv).into());
    let mut out = vec![0u8; data.len()];
    cipher
        .encrypt_padded_b2b::<NoPadding>(data, &mut out)
        .expect("Algorithm 2.B input is block-aligned by construction")
        .to_vec()
}

/// Encrypt with AES-256-CBC and no padding (`/UE`, `/OE`, `/Perms`).
#[cfg(feature = "pdf-encryption")]
fn aes256_cbc_encrypt_no_padding(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    use aes::cipher::{block_padding::NoPadding, BlockModeEncrypt, KeyIvInit};
    use aes::Aes256;
    use cbc::Encryptor;

    type Aes256Cbc = Encryptor<Aes256>;
    let key_arr: [u8; 32] = key.try_into().expect("AES-256 key is exactly 32 bytes");
    let cipher = Aes256Cbc::new(&key_arr.into(), &(*iv).into());
    let mut out = vec![0u8; data.len()];
    cipher
        .encrypt_padded_b2b::<NoPadding>(data, &mut out)
        .expect("the R=6 wrapped entries are block-aligned by construction")
        .to_vec()
}

/// Encrypt with AES-256-CBC and PKCS#7 padding (streams and strings).
#[cfg(feature = "pdf-encryption")]
fn aes256_cbc_encrypt(key: &[u8], iv: &[u8; 16], plaintext: &[u8]) -> Vec<u8> {
    use aes::cipher::{block_padding::Pkcs7, BlockModeEncrypt, KeyIvInit};
    use aes::Aes256;
    use cbc::Encryptor;

    type Aes256Cbc = Encryptor<Aes256>;
    let key_arr: [u8; 32] = key.try_into().expect("AES-256 key is exactly 32 bytes");
    let cipher = Aes256Cbc::new(&key_arr.into(), &(*iv).into());
    let mut out = vec![0u8; plaintext.len() + 16];
    cipher
        .encrypt_padded_b2b::<Pkcs7>(plaintext, &mut out)
        .expect(
            "AES-256-CBC encryption cannot fail for a buffer sized plaintext.len() + 16 with \
             PKCS#7 padding, so the allocator or key length is wrong",
        )
        .to_vec()
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
    fn test_legacy_intent_marker_round_trips_permissions() {
        // A marker written by an older build: parsing must still recover the permissions,
        // while passwords never survive (no build writes them any more).
        let text = "% RW-NOTE: PDF encryption requested but not implemented (print=false, \
                    edit=true, copy=false, annot=true) — document is NOT encrypted";
        let parsed = parse_security_diagnostics(text).expect("a legacy marker must parse");
        assert!(!parsed.print_permission);
        assert!(parsed.edit_permission);
        assert!(!parsed.copy_permission);
        assert!(parsed.annotation_permission);
        assert!(parsed.user_password.is_none());
        assert!(parsed.owner_password.is_none());
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
        // Document ID (16) + IV (16) + at least one ciphertext block.
        assert!(result.len() > 32, "Output should contain document ID + IV + ciphertext");
        // Should contain the encryption dictionary structure
        let result_str = String::from_utf8_lossy(&result);
        assert!(result_str.contains("/Filter"));
        assert!(result_str.contains("/Standard"));
    }

    /// `encrypt_pdf`'s output must be recoverable by a conforming R=6 consumer: `/U` + `/UE`
    /// wrap the random file key, and the AES-256-CBC payload is decrypted with it.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn encrypt_pdf_can_be_opened_with_the_r6_entries() {
        let content = b"decryptable payload";
        let out = encrypt_pdf(content, "pw", "owner").expect("entropy");
        let dict_end =
            out.iter().position(|&b| b == b'\n').expect("the dictionary ends with a newline");
        let dict = String::from_utf8_lossy(&out[..dict_end]).to_string();
        let payload = &out[dict_end + 1..];
        assert!(payload.len() >= 16 + 16 + 16, "document ID + IV + ciphertext");
        let _document_id: [u8; 16] = payload[..16].try_into().expect("16 bytes");
        let iv: [u8; 16] = payload[16..32].try_into().expect("16 bytes");
        let ciphertext = &payload[32..];

        let u_entry = parse_hex_n::<48>(&dict, "/U <").expect("/U entry");
        let ue_entry = parse_hex_n::<32>(&dict, "/UE <").expect("/UE entry");
        // Algorithm 2.A/2.B: the user key salt is U[40..48]; its hash unwraps the file key.
        let intermediate = hash_r6("pw", &u_entry[40..48], &[]);
        let file_key = aes256_cbc_decrypt_no_padding(&intermediate, &[0u8; 16], &ue_entry);
        let decrypted = aes256_cbc_decrypt(&file_key, &iv, ciphertext);
        assert_eq!(decrypted, content, "the R=6 entries must recover the random file key");

        assert_eq!(parse_permission_entry(&dict), Some(0xFFFF_FFFC));
    }

    /// Parse an N-byte hex entry (`/U <...>`, `/UE <...>`, ...) out of the dictionary.
    #[cfg(feature = "pdf-encryption")]
    fn parse_hex_n<const N: usize>(dict: &str, marker: &str) -> Option<[u8; N]> {
        let start = dict.find(marker)? + marker.len();
        let hex = dict.get(start..start + N * 2)?;
        let mut out = [0u8; N];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
        }
        Some(out)
    }

    /// Parse the signed `/P` permission word out of the dictionary.
    #[cfg(feature = "pdf-encryption")]
    fn parse_permission_entry(dict: &str) -> Option<u32> {
        let start = dict.find("/P ")? + "/P ".len();
        let token: String =
            dict[start..].chars().take_while(|ch| ch.is_ascii_digit() || *ch == '-').collect();
        token.parse::<i64>().ok().map(|value| value as u32)
    }

    /// AES-256-CBC decryption, used only to prove the ciphertext matches the recovered key.
    #[cfg(feature = "pdf-encryption")]
    fn aes256_cbc_decrypt(key: &[u8], iv: &[u8; 16], ciphertext: &[u8]) -> Vec<u8> {
        use aes::cipher::{block_padding::Pkcs7, BlockModeDecrypt, KeyIvInit};
        use aes::Aes256;
        use cbc::Decryptor;

        type Aes256CbcDec = Decryptor<Aes256>;
        let key_arr: [u8; 32] = key.try_into().expect("AES-256 key is 32 bytes");
        let cipher = Aes256CbcDec::new(&key_arr.into(), &(*iv).into());
        let mut out = vec![0u8; ciphertext.len()];
        cipher
            .decrypt_padded_b2b::<Pkcs7>(ciphertext, &mut out)
            .expect("valid PKCS#7 ciphertext")
            .to_vec()
    }

    /// AES-256-CBC decryption without padding (`/UE`, `/OE`, `/Perms`).
    #[cfg(feature = "pdf-encryption")]
    fn aes256_cbc_decrypt_no_padding(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
        use aes::cipher::{block_padding::NoPadding, BlockModeDecrypt, KeyIvInit};
        use aes::Aes256;
        use cbc::Decryptor;

        type Aes256CbcDec = Decryptor<Aes256>;
        let key_arr: [u8; 32] = key.try_into().expect("AES-256 key is 32 bytes");
        let cipher = Aes256CbcDec::new(&key_arr.into(), &(*iv).into());
        let mut out = vec![0u8; data.len()];
        cipher
            .decrypt_padded_b2b::<NoPadding>(data, &mut out)
            .expect("the wrapped entries are block-aligned")
            .to_vec()
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn test_encryption_dictionary_has_correct_entries() {
        let enc = PdfEncryption::new("user", "owner", 0xFFFFFFFC).expect("entropy");
        let dict = enc.build_encryption_dictionary();
        assert!(dict.contains("/Filter /Standard"));
        assert!(dict.contains("/Length 256"));
        assert!(dict.contains("/V 5"));
        assert!(dict.contains("/R 6"));
        assert!(dict.contains("/O <"));
        assert!(dict.contains("/U <"));
        assert!(dict.contains("/OE <"));
        assert!(dict.contains("/UE <"));
        assert!(dict.contains("/Perms <"));
        assert!(dict.contains("/P "));
        assert!(dict.contains("/CFM /AESV3"));
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

    /// R=6 draws a random file key; the password only wraps it. Unlike R=4, the same password
    /// and permissions must therefore produce **different** keys on two constructions.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn r6_uses_a_random_file_key() {
        let first = PdfEncryption::new("same", "owner", 0xFFFF_FFFC).expect("entropy");
        let second = PdfEncryption::new("same", "owner", 0xFFFF_FFFC).expect("entropy");
        assert_ne!(first.encryption_key, second.encryption_key);
        assert!(first.verify_user_password("same"));
        assert!(second.verify_user_password("same"));
    }

    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn r6_user_and_owner_passwords_verify() {
        let enc = PdfEncryption::new("correct horse", "owner", 0xFFFF_FFFC).expect("entropy");
        assert!(enc.verify_user_password("correct horse"));
        assert!(!enc.verify_user_password("wrong horse"));
        assert!(enc.verify_owner_password("owner"));
        assert!(!enc.verify_owner_password("wrong owner"));
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
    fn aes256_cbc_encrypt_produces_valid_output() {
        let key = b"0123456789abcdef0123456789abcdef"; // 32 bytes
        let iv = b"fedcba9876543210"; // 16 bytes
        let plaintext = b"Hello, PDF encryption!";
        let ciphertext = aes256_cbc_encrypt(key, iv, plaintext);
        assert_eq!(ciphertext.len() % 16, 0, "Ciphertext length must be a multiple of 16");
        assert_ne!(
            ciphertext.as_slice(),
            &plaintext[..],
            "Ciphertext should differ from plaintext"
        );
        assert!(ciphertext.len() > plaintext.len(), "Ciphertext should be longer due to padding");
        assert_eq!(aes256_cbc_decrypt(key, iv, &ciphertext), plaintext);
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

    /// `encrypt_content` yields a PDF 2.0 AESV3 hex string: `<` + the 16-byte IV + a whole
    /// number of AES blocks + `>`.
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
            encryption_key: vec![0u8; 32],
            document_id: [0u8; 16],
            u_entry: [0u8; 48],
            o_entry: [0u8; 48],
            ue_entry: [0u8; 32],
            oe_entry: [0u8; 32],
            perms_entry: [0u8; 16],
        };
        assert!(enc.encrypt_content(b"payload").is_err());
    }

    /// Algorithm 2.B against an independent implementation: qpdf 11.9 wrote this `/U` for the
    /// password `user`. Its first 32 bytes are `SHA-2` rounds over `user` and the validation salt
    /// at `U[32..40]`; recomputing them pins the whole loop.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn r6_hash_matches_the_qpdf_reference() {
        let u = decode_hex::<48>(
            "36fcc1d9f34915526157c565a4de877239db53d13521e957f5ef1eb42fb0847c\
             8d23e8e905754f65f33b46ff8faa708b",
        );
        assert_eq!(hash_r6("user", &u[32..40], &[]), u[..32]);
    }

    /// `/Perms` decodes to the permission word, `T` (EncryptMetadata) and the `adb` marker.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn r6_perms_entry_decodes_the_permission_word() {
        let enc = PdfEncryption::new("user", "owner", 0xFFFF_F0C0).expect("entropy");
        let clear =
            aes256_cbc_decrypt_no_padding(&enc.encryption_key, &[0u8; 16], &enc.perms_entry);
        assert_eq!(u32::from_le_bytes(clear[..4].try_into().expect("4 bytes")), 0xFFFF_F0C0);
        assert_eq!(&clear[8..12], b"Tadb");
    }

    /// Decode a fixed-size byte array from a hex string, ignoring ASCII whitespace.
    #[cfg(feature = "pdf-encryption")]
    fn decode_hex<const N: usize>(hex: &str) -> [u8; N] {
        let compact: String = hex.chars().filter(|ch| !ch.is_ascii_whitespace()).collect();
        assert_eq!(compact.len(), N * 2, "hex string must contain exactly {N} bytes");
        let mut out = [0u8; N];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&compact[index * 2..index * 2 + 2], 16)
                .expect("valid hex digit");
        }
        out
    }
}
