//! Additive authenticated facts for one durable HTTP attempt.
//!
//! This is a storage primitive, not a wired webhook policy or a v2 event
//! encoder. The host supplies the signing timestamp and key; this module
//! authenticates those facts and commits the actual request body's digest.
//! It does not prove that the body itself contains that timestamp. Reading
//! an intent never authorizes dispatch, proves remote receipt, or proves
//! freshness against deletion/rollback of the caller-owned store.

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

use super::*;

pub(super) const MAX_AUTHENTICATED_INTENT_BYTES: usize = 512;
const SCHEMA: &str = "semaprax.outbound.authenticated-http-intent.v1";
const MAC_DOMAIN: &[u8] = b"semaprax.outbound.authenticated-http-intent.mac.v1\0";
const BODY_DOMAIN: &[u8] = b"semaprax.outbound.authenticated-http-intent.body.v1\0";
type HmacSha256 = Hmac<Sha256>;

/// Authenticated data only. An intent can exist before the physical adapter
/// was entered; `attempt()` identifies the reserved first attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpIntentFacts {
    body_digest: String,
    signed_at: i64,
}

impl HttpIntentFacts {
    /// Domain-separated commitment to the exact HTTP body, not a complete
    /// request commitment (endpoint, headers and policy remain separate).
    pub fn body_digest(&self) -> &str {
        &self.body_digest
    }

    pub fn signed_at(&self) -> i64 {
        self.signed_at
    }

    pub fn attempt(&self) -> i64 {
        1
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpIntentLookup {
    /// Absent at this observation only. A concurrent create may still win.
    Absent,
    /// The original marker has no authenticated descriptor or timestamp.
    /// Its presence still prevents every fresh-session dispatch.
    LegacyBlocked,
    Authenticated(HttpIntentFacts),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpIntentReadRefusal {
    InvalidIdentity,
    StorageUnavailable,
    InvalidRecord,
}

/// Read one exact identity under a held directory, without scanning or
/// selecting a latest checkpoint. Obtain `identity_key` from
/// `PreparedHttpDelivery::pending_identity_key()`. The name carries no
/// authority; the store's held directory and independently held key do.
///
/// Only an explicit no-follow absence observation yields `Absent`. Every
/// other filesystem error refuses. A legacy marker yields no invented facts;
/// an authenticated marker binds its identity, timestamp and body digest.
pub fn read_http_intent(
    store: &OutboundDeliveryStore<'_>,
    identity_key: &str,
    authentication_key: &[u8; 32],
) -> Result<HttpIntentLookup, HttpIntentReadRefusal> {
    let name = pending_intent_filename(OutboundCheckpointKind::HttpSession, identity_key)
        .ok_or(HttpIntentReadRefusal::InvalidIdentity)?;
    let directory = store.directory();
    let child = platform::prepare_child_name(OsStr::new(&name))
        .map_err(|_| HttpIntentReadRefusal::InvalidIdentity)?;
    platform::recheck_directory(directory)
        .map_err(|_| HttpIntentReadRefusal::StorageUnavailable)?;
    let absent = platform::child_absent_prepared(directory, &child)
        .map_err(|_| HttpIntentReadRefusal::StorageUnavailable)?;
    if absent {
        platform::recheck_directory(directory)
            .map_err(|_| HttpIntentReadRefusal::StorageUnavailable)?;
        return Ok(HttpIntentLookup::Absent);
    }
    let held = platform::hold_regular_file_bounded(
        directory,
        OsStr::new(&name),
        MAX_AUTHENTICATED_INTENT_BYTES,
    )
    .map_err(|_| HttpIntentReadRefusal::StorageUnavailable)?;
    let bytes = platform::read_exact(&held, MAX_AUTHENTICATED_INTENT_BYTES)
        .map_err(|_| HttpIntentReadRefusal::StorageUnavailable)?;
    platform::recheck_regular_file_named_bounded(
        directory,
        OsStr::new(&name),
        &held,
        MAX_AUTHENTICATED_INTENT_BYTES,
    )
    .map_err(|_| HttpIntentReadRefusal::StorageUnavailable)?;
    if bytes == PENDING_INTENT_BYTES {
        return Ok(HttpIntentLookup::LegacyBlocked);
    }
    decode(&bytes, identity_key, authentication_key)
        .map(HttpIntentLookup::Authenticated)
        .ok_or(HttpIntentReadRefusal::InvalidRecord)
}

/// Reserve exactly one attempt using an authenticated marker and then the
/// ordinary typed session checkpoint/adapter pipeline. All identities share
/// the legacy marker namespace, so either API blocks the other on restart.
///
/// A prior record never permits another physical attempt. The caller may read
/// it before source admission; final create-new still arbitrates races. This
/// API does not restore a terminal session or infer a delivered disposition.
/// Negative timestamps refuse before creating any file or entering an adapter.
pub fn deliver_http_durable_authenticated(
    store: &mut OutboundDeliveryStore<'_>,
    capacity: usize,
    capability: OutboundCapability,
    request: HttpRequest,
    signed_at: i64,
    authentication_key: &[u8; 32],
    adapter: &mut impl OutboundAdapter,
) -> Result<ServiceHttpDeliveryOutcome, ServiceHttpDeliveryRefusal> {
    if signed_at < 0 {
        return Err(ServiceHttpDeliveryRefusal::InvalidRequest);
    }
    let facts = HttpIntentFacts {
        body_digest: http_intent_body_digest(&request.body),
        signed_at,
    };
    let prepared = prepare_http_delivery(capability, request)
        .map_err(|_| ServiceHttpDeliveryRefusal::InvalidRequest)?;
    let marker = encode(prepared.pending_identity_key(), &facts, authentication_key);
    let session =
        HttpDeliverySession::new(capacity).map_err(ServiceHttpDeliveryRefusal::Session)?;
    reconcile_prepared(store, session, prepared, Some(marker), adapter)
}

/// Compute the same body commitment used by authenticated prior-intent facts.
/// Callers must bound input before constructing a request; this pure helper
/// neither authenticates the bytes nor grants export authority.
pub fn http_intent_body_digest(body: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(BODY_DOMAIN);
    hash.update((body.len() as u64).to_le_bytes());
    hash.update(body);
    format!("sha256:{}", hex(&hash.finalize()))
}

fn prefix(identity_key: &str, facts: &HttpIntentFacts) -> String {
    format!(
        "{SCHEMA}\n{identity_key}\n{}\n{}\n1\n",
        facts.body_digest, facts.signed_at
    )
}

fn encode(identity_key: &str, facts: &HttpIntentFacts, key: &[u8; 32]) -> Vec<u8> {
    let mut wire = prefix(identity_key, facts);
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts 32-byte keys");
    mac.update(MAC_DOMAIN);
    mac.update(wire.as_bytes());
    wire.push_str(&hex(&mac.finalize().into_bytes()));
    wire.push('\n');
    wire.into_bytes()
}

fn decode(bytes: &[u8], identity_key: &str, key: &[u8; 32]) -> Option<HttpIntentFacts> {
    if bytes.len() > MAX_AUTHENTICATED_INTENT_BYTES {
        return None;
    }
    let wire = std::str::from_utf8(bytes).ok()?;
    let mut fields = wire.strip_suffix('\n')?.split('\n');
    if fields.next()? != SCHEMA || fields.next()? != identity_key {
        return None;
    }
    let body_digest = fields.next()?;
    // Reuse the exact lowercase SHA-256 grammar; do not construct a path.
    pending_intent_filename(OutboundCheckpointKind::HttpSession, body_digest)?;
    let signed_at_text = fields.next()?;
    let signed_at: i64 = signed_at_text.parse().ok()?;
    if signed_at < 0 || signed_at.to_string() != signed_at_text || fields.next()? != "1" {
        return None;
    }
    let tag = decode_tag(fields.next()?)?;
    if fields.next().is_some() {
        return None;
    }
    let facts = HttpIntentFacts {
        body_digest: body_digest.to_owned(),
        signed_at,
    };
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts 32-byte keys");
    mac.update(MAC_DOMAIN);
    mac.update(prefix(identity_key, &facts).as_bytes());
    mac.verify_slice(&tag).ok()?;
    Some(facts)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[usize::from(byte >> 4)] as char);
        result.push(DIGITS[usize::from(byte & 15)] as char);
    }
    result
}

fn decode_tag(text: &str) -> Option<[u8; 32]> {
    fn digit(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }
    let mut result = [0; 32];
    if text.len() != result.len() * 2 {
        return None;
    }
    for (output, pair) in result.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *output = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Some(result)
}
