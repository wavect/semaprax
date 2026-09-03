//! Host-selected authenticated persistence of compiler-created semantic caches.
//! The host protects the key and keeps its currently executing static compiler
//! installation immutable from exec through each operation. A file hash is not
//! attestation of already-loaded code, dynamic libraries, or a hostile host.

#![cfg_attr(
    not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )),
    allow(
        dead_code,
        reason = "unsupported hosts expose only fail-closed store APIs; private codec helpers are not invoked"
    )
)]

use crate::diagnostic::Diagnostic;
use crate::project::ProjectFrontendCache;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use std::path::Path;

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
mod unix;

pub const MAX_SEMANTIC_CACHE_PAYLOAD_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_SEMANTIC_CACHE_STORE_ENTRIES: usize = 32;
pub const MAX_SEMANTIC_CACHE_COMPILER_BYTES: usize = 256 * 1024 * 1024;
const MAX_ENVELOPE_BYTES: usize = MAX_SEMANTIC_CACHE_PAYLOAD_BYTES + 4096;
const MAGIC: &[u8; 8] = b"SPXSHC01";
const MAC_DOMAIN: &[u8] = b"semaprax.semantic-cache-store.authenticated-envelope.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
type HmacSha256 = Hmac<Sha256>;

/// Content identity only; it carries no key, root, decoder, or authority.
#[derive(Debug)]
pub struct SemanticCacheReceipt {
    entry: String,
    compiler: String,
    payload_bytes: usize,
}

/// Receipt for removal of one exact derived cache entry. It proves only the
/// selected store mutation; it carries no source, compiler, or publication
/// authority.
#[derive(Debug)]
pub struct SemanticCacheEvictionReceipt {
    entry: String,
    envelope_bytes: usize,
    entries_remaining: usize,
}
impl SemanticCacheEvictionReceipt {
    pub fn entry_digest(&self) -> &str {
        &self.entry
    }
    pub fn envelope_bytes(&self) -> usize {
        self.envelope_bytes
    }
    pub fn entries_remaining(&self) -> usize {
        self.entries_remaining
    }
}
impl SemanticCacheReceipt {
    pub fn entry_digest(&self) -> &str {
        &self.entry
    }
    pub fn compiler_digest(&self) -> &str {
        &self.compiler
    }
    pub fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
}

/// Initialize one EMPTY dedicated host root with an OS-generated private key.
/// No adoption, root creation, key import, or implicit key rotation is supported.
pub fn initialize(root: &Path) -> Result<()> {
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        unix::initialize(root)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = root;
        Err(io(
            "semantic cache store requires supported Unix filesystem authority",
        ))
    }
}

/// Encode only opaque compiler-produced state. No public arbitrary-byte signer
/// or raw-HIR constructor exists. Cache preparation itself grants no disk access.
pub fn persist(root: &Path, cache: &ProjectFrontendCache) -> Result<SemanticCacheReceipt> {
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        if !cache.is_semantic_cache_enabled() {
            return Err(invalid(
                "only checked-module semantic caches can be persisted",
            ));
        }
        let payload = crate::project::incremental::encode_snapshot(cache)?;
        if payload.len() > MAX_SEMANTIC_CACHE_PAYLOAD_BYTES {
            return Err(capacity("semantic cache payload exceeds128MiB"));
        }
        unix::persist(root, &payload)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = (root, cache);
        Err(io(
            "semantic cache store requires supported Unix filesystem authority",
        ))
    }
}

/// Authenticate the complete selected envelope BEFORE private HIR decoding.
/// Fresh source/context checks still govern whether decoded entries can be reused.
pub fn load(root: &Path, expected_digest: &str) -> Result<ProjectFrontendCache> {
    digest_hex(expected_digest)?;
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        unix::load(root, expected_digest)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = root;
        Err(io(
            "semantic cache store requires supported Unix filesystem authority",
        ))
    }
}

/// Remove one exact derived entry under the store's ordinary exclusive lock.
/// The key, other entries, canonical source, and host policy are unchanged.
/// A failure after the namespace pivot is reported as uncertainty and must not
/// be retried blindly.
pub fn evict(root: &Path, expected_digest: &str) -> Result<SemanticCacheEvictionReceipt> {
    digest_hex(expected_digest)?;
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        unix::evict(root, expected_digest)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = root;
        Err(io(
            "semantic cache store requires supported Unix filesystem authority",
        ))
    }
}

fn compatibility() -> Vec<u8> {
    let mut bytes = Vec::new();
    for text in [
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(target_endian = "little") {
            "little"
        } else {
            "big"
        },
        crate::project::incremental::PROJECT_SEMANTIC_CACHE_COMPATIBILITY,
    ] {
        bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
        bytes.extend_from_slice(text.as_bytes());
    }
    bytes.extend_from_slice(&usize::BITS.to_le_bytes());
    bytes
}
fn seal(
    payload: &[u8],
    key: &[u8; 32],
    compiler: &[u8; 32],
) -> Result<(Vec<u8>, SemanticCacheReceipt)> {
    if payload.len() > MAX_SEMANTIC_CACHE_PAYLOAD_BYTES {
        return Err(capacity("semantic cache payload exceeds128MiB"));
    }
    let context = compatibility();
    if context.len() > 2048 {
        return Err(capacity(
            "compiler cache compatibility header exceeds its limit",
        ));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(8 + 32 + 4 + context.len() + 8 + payload.len() + 32)
        .map_err(|_| capacity("cannot reserve bounded semantic cache envelope"))?;
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(compiler);
    bytes.extend_from_slice(&(context.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&context);
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(payload);
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| invalid("cannot initialize cache authentication"))?;
    mac.update(MAC_DOMAIN);
    mac.update(&bytes);
    bytes.extend_from_slice(&mac.finalize().into_bytes());
    let receipt = SemanticCacheReceipt {
        entry: hash(&bytes),
        compiler: format!("sha256:{:x}", crate::digest_hex::LowerHex(compiler)),
        payload_bytes: payload.len(),
    };
    Ok((bytes, receipt))
}
fn authenticate<'a>(
    bytes: &'a [u8],
    expected: &str,
    key: &[u8; 32],
    compiler: &[u8; 32],
) -> Result<&'a [u8]> {
    if bytes.len() > MAX_ENVELOPE_BYTES || bytes.len() < 8 + 32 + 4 + 8 + 32 {
        return Err(capacity("semantic cache envelope length is outside bounds"));
    }
    if hash(bytes) != expected {
        return Err(binding(
            "semantic cache envelope digest does not match selection",
        ));
    }
    let body_end = bytes.len() - 32;
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| invalid("cannot initialize cache authentication"))?;
    mac.update(MAC_DOMAIN);
    mac.update(&bytes[..body_end]);
    mac.verify_slice(&bytes[body_end..])
        .map_err(|_| authentication("semantic cache authentication tag does not match"))?;
    // No payload/header-driven allocation or HIR decoding precedes MAC verification.
    if &bytes[..8] != MAGIC || &bytes[8..40] != compiler {
        return Err(binding(
            "semantic cache version or exact compiler file identity differs",
        ));
    }
    let context_len =
        u32::from_le_bytes(bytes[40..44].try_into().expect("bounded header")) as usize;
    if context_len > 2048 || context_len > body_end - 52 {
        return Err(capacity(
            "semantic cache compatibility header length is invalid",
        ));
    }
    let payload_start = 52 + context_len;
    if bytes[44..44 + context_len] != compatibility() {
        return Err(binding(
            "semantic cache compiler compatibility context differs",
        ));
    }
    let payload_len = u64::from_le_bytes(
        bytes[44 + context_len..payload_start]
            .try_into()
            .expect("bounded length"),
    );
    if payload_len > MAX_SEMANTIC_CACHE_PAYLOAD_BYTES as u64
        || payload_len != (body_end - payload_start) as u64
    {
        return Err(capacity(
            "semantic cache authenticated payload length disagrees",
        ));
    }
    Ok(&bytes[payload_start..body_end])
}
fn hash(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
fn digest_hex(value: &str) -> Result<&str> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or_else(|| invalid("semantic cache selector requires SHA256 syntax"))?;
    if !canonical_hex(hex) {
        return Err(invalid(
            "semantic cache selector is not canonical lowercase SHA256",
        ));
    }
    Ok(hex)
}
fn canonical_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G306", message)]
}
fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G307", message)]
}
fn binding(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G308", message)]
}
fn authentication(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G309", message)]
}
fn io(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I362", message)]
}
fn post_pivot(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I363", message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn code<T>(result: Result<T>, expected: &str) {
        match result {
            Ok(_) => panic!("expected {expected}"),
            Err(errors) => assert!(errors.iter().any(|e| e.code == expected), "{errors:?}"),
        }
    }
    #[test]
    fn complete_payload_is_authenticated_and_rehashed_tamper_does_not_reach_decode() {
        let (bytes, receipt) = seal(b"private codec payload", &[7; 32], &[3; 32]).unwrap();
        assert_eq!(
            authenticate(&bytes, receipt.entry_digest(), &[7; 32], &[3; 32]).unwrap(),
            b"private codec payload"
        );
        let mut altered = bytes.clone();
        let index = altered.len() - 33;
        altered[index] ^= 1;
        // A caller may recompute the public content address but cannot repair MAC.
        let attempt = authenticate(&altered, &hash(&altered), &[7; 32], &[3; 32]);
        assert!(attempt.is_err());
        code(attempt, "SPX-G309");
        code(
            authenticate(&bytes, receipt.entry_digest(), &[8; 32], &[3; 32]),
            "SPX-G309",
        );
        code(
            authenticate(&bytes, receipt.entry_digest(), &[7; 32], &[4; 32]),
            "SPX-G308",
        );
        code(
            authenticate(
                &bytes,
                &format!("sha256:{}", "0".repeat(64)),
                &[7; 32],
                &[3; 32],
            ),
            "SPX-G308",
        );
    }
    #[test]
    fn authenticated_incompatible_or_oversized_headers_reject_before_private_decoder() {
        let (bytes, _) = seal(
            b"not decoded by envelope authentication",
            &[7; 32],
            &[3; 32],
        )
        .unwrap();
        for oversized in [false, true] {
            let mut altered = bytes.clone();
            if oversized {
                altered[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
            } else {
                altered[48] ^= 1;
            }
            // Only a private test seam can reseal arbitrary bytes. No public API can.
            let body = altered.len() - 32;
            let mut mac = HmacSha256::new_from_slice(&[7; 32]).unwrap();
            mac.update(MAC_DOMAIN);
            mac.update(&altered[..body]);
            altered[body..].copy_from_slice(&mac.finalize().into_bytes());
            code(
                authenticate(&altered, &hash(&altered), &[7; 32], &[3; 32]),
                if oversized { "SPX-G307" } else { "SPX-G308" },
            );
        }
    }
}
