//! Bounded, host-owned signed session tokens. This is deliberately not JWT.

use std::collections::BTreeMap;
use std::fmt;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use super::{AuthEntropy, AuthError, SecretBytes};

type HmacSha256 = Hmac<Sha256>;

const TOKEN_VERSION: u8 = 1;
const TOKEN_MAGIC: &[u8; 4] = b"SPXS";
const MAC_DOMAIN: &[u8] = b"semaprax.session-token.v1\0";
const MAC_BYTES: usize = 32;
const SESSION_ID_BYTES: usize = 32;
const MAX_TOKEN_BYTES: usize = 768;
const MAX_POLICY_ID_BYTES: usize = 64;
const MAX_ISSUER_BYTES: usize = 128;
const MAX_AUDIENCE_BYTES: usize = 128;
const MAX_SUBJECT_BYTES: usize = 128;
const MAX_TTL: u64 = 2_592_000;
pub const MAX_IN_MEMORY_SESSIONS: usize = 1_024;

/// Server-selected policy bound into every token. A token does not select an
/// algorithm, signing key, issuer, audience, or policy identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPolicy {
    policy_id: String,
    issuer: String,
    audience: String,
    min_ttl: u64,
    max_ttl: u64,
}

impl SessionPolicy {
    pub fn new(
        policy_id: &str,
        issuer: &str,
        audience: &str,
        min_ttl: u64,
        max_ttl: u64,
    ) -> Result<Self, AuthError> {
        valid_text(policy_id, MAX_POLICY_ID_BYTES)?;
        valid_text(issuer, MAX_ISSUER_BYTES)?;
        valid_text(audience, MAX_AUDIENCE_BYTES)?;
        if min_ttl == 0 || min_ttl > max_ttl || max_ttl > MAX_TTL {
            return Err(AuthError::InvalidPolicy);
        }
        Ok(Self {
            policy_id: policy_id.to_owned(),
            issuer: issuer.to_owned(),
            audience: audience.to_owned(),
            min_ttl,
            max_ttl,
        })
    }

    pub fn policy_id(&self) -> &str {
        &self.policy_id
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    pub fn audience(&self) -> &str {
        &self.audience
    }

    fn ttl_is_valid(&self, ttl: u64) -> bool {
        ttl >= self.min_ttl && ttl <= self.max_ttl
    }
}

/// Opaque bearer-token bytes. It has no `Clone`, serialization, or debug
/// implementation. `bearer` is the one deliberate transport boundary.
pub struct SessionToken(SecretBytes);

impl SessionToken {
    /// Returns the ASCII bearer value for an explicitly selected transport.
    pub fn bearer(&self) -> &str {
        // Constructed only by `encode_token`, which emits ASCII base64url and
        // a dot separator.
        std::str::from_utf8(self.0.as_bytes()).expect("session token is ASCII")
    }
}

/// Public authentication result. It deliberately exposes no bearer token or
/// signing key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedSession {
    subject: String,
    issued_at: u64,
    expires_at: u64,
    generation: u64,
}

impl AuthenticatedSession {
    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub const fn issued_at(&self) -> u64 {
        self.issued_at
    }

    pub const fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct SessionId([u8; SESSION_ID_BYTES]);

impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionId([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Active,
    Rotated,
    Revoked,
}

/// The non-secret data a session store retains. Store implementations must not
/// treat values reconstructed from a bearer token as an authority grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRecord {
    id: SessionId,
    subject: String,
    issued_at: u64,
    expires_at: u64,
    generation: u64,
    state: SessionState,
}

impl SessionRecord {
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub const fn issued_at(&self) -> u64 {
        self.issued_at
    }

    pub const fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn state(&self) -> SessionState {
        self.state
    }
}

/// Store mutation operations must perform the named generation comparison and
/// state transition atomically. This trait does not claim durable storage.
pub trait SessionStore {
    fn lookup(&self, id: &SessionId) -> Result<Option<SessionRecord>, AuthError>;
    fn issue_if_absent(&mut self, record: SessionRecord) -> Result<(), AuthError>;
    fn compare_and_rotate(
        &mut self,
        id: &SessionId,
        generation: u64,
        replacement: SessionRecord,
    ) -> Result<(), AuthError>;
    fn compare_and_revoke(&mut self, id: &SessionId, generation: u64) -> Result<(), AuthError>;
}

/// Capped process-local reference store. It is useful for deterministic host
/// integration and tests; it makes no durability or distributed-atomicity
/// claim.
pub struct InMemorySessionStore {
    capacity: usize,
    records: BTreeMap<SessionId, SessionRecord>,
    /// Host-supplied time through which expired records were reclaimed.
    purged_through: u64,
}

impl InMemorySessionStore {
    pub fn new(capacity: usize) -> Result<Self, AuthError> {
        if capacity == 0 || capacity > MAX_IN_MEMORY_SESSIONS {
            return Err(AuthError::InvalidPolicy);
        }
        Ok(Self {
            capacity,
            records: BTreeMap::new(),
            purged_through: 0,
        })
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Remove records expired at or before `now`, including rotated and
    /// revoked records. The host must supply a nondecreasing trusted tick.
    /// Refusing clock regression also prevents an expired bearer from being
    /// recreated with the same random id and original issue time.
    pub fn purge_expired(&mut self, now: u64) -> Result<usize, AuthError> {
        if now < self.purged_through {
            return Err(AuthError::InvalidInput);
        }
        let before = self.records.len();
        self.records.retain(|_, record| now < record.expires_at);
        self.purged_through = now;
        Ok(before - self.records.len())
    }
}

impl SessionStore for InMemorySessionStore {
    fn lookup(&self, id: &SessionId) -> Result<Option<SessionRecord>, AuthError> {
        Ok(self.records.get(id).cloned())
    }

    fn issue_if_absent(&mut self, record: SessionRecord) -> Result<(), AuthError> {
        if record.issued_at < self.purged_through {
            return Err(AuthError::InvalidInput);
        }
        if self.records.contains_key(&record.id) {
            return Err(AuthError::Conflict);
        }
        if self.records.len() >= self.capacity {
            return Err(AuthError::Capacity);
        }
        self.records.insert(record.id.clone(), record);
        Ok(())
    }

    fn compare_and_rotate(
        &mut self,
        id: &SessionId,
        generation: u64,
        replacement: SessionRecord,
    ) -> Result<(), AuthError> {
        if replacement.issued_at < self.purged_through {
            return Err(AuthError::InvalidInput);
        }
        if replacement.id == *id || self.records.contains_key(&replacement.id) {
            return Err(AuthError::Conflict);
        }
        if self.records.len() >= self.capacity {
            return Err(AuthError::Capacity);
        }
        let current = self.records.get_mut(id).ok_or(AuthError::Revoked)?;
        if current.generation != generation || current.state != SessionState::Active {
            return Err(AuthError::Conflict);
        }
        current.state = SessionState::Rotated;
        self.records.insert(replacement.id.clone(), replacement);
        Ok(())
    }

    fn compare_and_revoke(&mut self, id: &SessionId, generation: u64) -> Result<(), AuthError> {
        let current = self.records.get_mut(id).ok_or(AuthError::Revoked)?;
        if current.generation != generation || current.state != SessionState::Active {
            return Err(AuthError::Conflict);
        }
        current.state = SessionState::Revoked;
        Ok(())
    }
}

/// A previous signing key retained only until `grace_until`, so rotating the
/// server signing key does not immediately invalidate every live session.
/// New tokens are never signed with a previous key; it is accepted only for
/// verifying tokens issued before the rotation, and only while the caller's
/// `now` has not yet passed `grace_until`.
pub struct SigningKeyRotation {
    key: SecretBytes,
    grace_until: u64,
}

impl SigningKeyRotation {
    pub fn new(key: SecretBytes, grace_until: u64) -> Result<Self, AuthError> {
        if key.as_bytes().len() < MAC_BYTES {
            return Err(AuthError::InvalidPolicy);
        }
        Ok(Self { key, grace_until })
    }
}

/// Host service using fixed HS256 HMAC verification. The signing key is
/// selected by the server at construction and is never chosen by token data.
pub struct SessionService {
    policy: SessionPolicy,
    signing_key: SecretBytes,
    previous_key: Option<SigningKeyRotation>,
}

impl SessionService {
    pub fn new(policy: SessionPolicy, signing_key: SecretBytes) -> Result<Self, AuthError> {
        Self::with_key_rotation(policy, signing_key, None)
    }

    /// Creates a service that signs new tokens with `signing_key` and, while
    /// `previous` names a still-in-grace key, also accepts tokens signed with
    /// that retired key. `previous`'s key must differ from `signing_key`: a
    /// rotation names a change, not the same key twice.
    pub fn with_key_rotation(
        policy: SessionPolicy,
        signing_key: SecretBytes,
        previous: Option<SigningKeyRotation>,
    ) -> Result<Self, AuthError> {
        if signing_key.as_bytes().len() < MAC_BYTES {
            return Err(AuthError::InvalidPolicy);
        }
        if let Some(previous) = &previous {
            if previous.key.as_bytes() == signing_key.as_bytes() {
                return Err(AuthError::InvalidPolicy);
            }
        }
        Ok(Self {
            policy,
            signing_key,
            previous_key: previous,
        })
    }

    pub fn policy(&self) -> &SessionPolicy {
        &self.policy
    }

    pub fn issue(
        &self,
        store: &mut dyn SessionStore,
        entropy: &mut dyn AuthEntropy,
        subject: &str,
        now: u64,
        ttl: u64,
    ) -> Result<SessionToken, AuthError> {
        let record = self.new_record(entropy, subject, now, ttl, 0)?;
        let claims = Claims::from_record(&record, &self.policy);
        let token = self.encode_token(&claims)?;
        store.issue_if_absent(record)?;
        Ok(token)
    }

    pub fn verify(
        &self,
        store: &dyn SessionStore,
        bearer: &str,
        now: u64,
    ) -> Result<AuthenticatedSession, AuthError> {
        Ok(self.verify_claims(store, bearer, now)?.public())
    }

    pub fn rotate(
        &self,
        store: &mut dyn SessionStore,
        entropy: &mut dyn AuthEntropy,
        bearer: &str,
        now: u64,
        ttl: u64,
    ) -> Result<SessionToken, AuthError> {
        let current = self.verify_claims(store, bearer, now)?;
        let generation = current
            .generation
            .checked_add(1)
            .ok_or(AuthError::Capacity)?;
        let replacement = self.new_record(entropy, &current.subject, now, ttl, generation)?;
        let replacement_claims = Claims::from_record(&replacement, &self.policy);
        let token = self.encode_token(&replacement_claims)?;
        store.compare_and_rotate(&current.id, current.generation, replacement)?;
        Ok(token)
    }

    pub fn revoke(
        &self,
        store: &mut dyn SessionStore,
        bearer: &str,
        now: u64,
    ) -> Result<(), AuthError> {
        let current = self.verify_claims(store, bearer, now)?;
        store.compare_and_revoke(&current.id, current.generation)
    }

    fn new_record(
        &self,
        entropy: &mut dyn AuthEntropy,
        subject: &str,
        now: u64,
        ttl: u64,
        generation: u64,
    ) -> Result<SessionRecord, AuthError> {
        valid_text(subject, MAX_SUBJECT_BYTES)?;
        if !self.policy.ttl_is_valid(ttl) {
            return Err(AuthError::InvalidPolicy);
        }
        let expires_at = now.checked_add(ttl).ok_or(AuthError::InvalidInput)?;
        let mut bytes = [0_u8; SESSION_ID_BYTES];
        entropy.fill(&mut bytes).map_err(|_| AuthError::Entropy)?;
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(AuthError::Entropy);
        }
        Ok(SessionRecord {
            id: SessionId(bytes),
            subject: subject.to_owned(),
            issued_at: now,
            expires_at,
            generation,
            state: SessionState::Active,
        })
    }

    fn verify_claims(
        &self,
        store: &dyn SessionStore,
        bearer: &str,
        now: u64,
    ) -> Result<Claims, AuthError> {
        let claims = self.decode_token(bearer, now)?;
        if claims.policy_id != self.policy.policy_id
            || claims.issuer != self.policy.issuer
            || claims.audience != self.policy.audience
        {
            return Err(AuthError::InvalidCredential);
        }
        let ttl = claims
            .expires_at
            .checked_sub(claims.issued_at)
            .ok_or(AuthError::InvalidCredential)?;
        if now < claims.issued_at || !self.policy.ttl_is_valid(ttl) {
            return Err(AuthError::InvalidCredential);
        }
        if now >= claims.expires_at {
            return Err(AuthError::Expired);
        }
        let record = store.lookup(&claims.id)?.ok_or(AuthError::Revoked)?;
        if record.state != SessionState::Active {
            return Err(AuthError::Revoked);
        }
        if now >= record.expires_at {
            return Err(AuthError::Expired);
        }
        if record.id != claims.id
            || record.subject != claims.subject
            || record.issued_at != claims.issued_at
            || record.expires_at != claims.expires_at
            || record.generation != claims.generation
        {
            return Err(AuthError::InvalidCredential);
        }
        Ok(claims)
    }

    fn encode_token(&self, claims: &Claims) -> Result<SessionToken, AuthError> {
        let payload = claims.encode()?;
        let mut mac = Self::mac_for(&self.signing_key)?;
        mac.update(MAC_DOMAIN);
        mac.update(&payload);
        let tag = mac.finalize().into_bytes();
        let mut bearer = Zeroizing::new(String::new());
        base64url_encode(&payload, &mut bearer)?;
        bearer.push('.');
        base64url_encode(&tag, &mut bearer)?;
        if bearer.len() > MAX_TOKEN_BYTES {
            return Err(AuthError::Capacity);
        }
        Ok(SessionToken(SecretBytes::try_from_bytes(
            bearer.as_bytes(),
        )?))
    }

    fn decode_token(&self, bearer: &str, now: u64) -> Result<Claims, AuthError> {
        if bearer.is_empty() || bearer.len() > MAX_TOKEN_BYTES {
            return Err(AuthError::InvalidCredential);
        }
        let mut fields = bearer.split('.');
        let payload_text = fields.next().ok_or(AuthError::InvalidCredential)?;
        let tag_text = fields.next().ok_or(AuthError::InvalidCredential)?;
        if fields.next().is_some() || payload_text.is_empty() || tag_text.is_empty() {
            return Err(AuthError::InvalidCredential);
        }
        let payload = base64url_decode(payload_text)?;
        let tag = base64url_decode(tag_text)?;
        if tag.len() != MAC_BYTES {
            return Err(AuthError::InvalidCredential);
        }
        if !self.tag_matches(&self.signing_key, &payload, &tag)?
            && !self.tag_matches_previous_key_in_grace(&payload, &tag, now)?
        {
            return Err(AuthError::InvalidCredential);
        }
        Claims::decode(&payload)
    }

    /// Checks the tag under one candidate key. A malformed key (wrong byte
    /// length for HMAC-SHA-256's key-init step) is a policy error, not a
    /// tag mismatch; both keys admitted by this service's constructors
    /// already satisfy HMAC's key-length requirement, so this only guards
    /// against a future caller of `mac_for` outside that contract.
    fn tag_matches(
        &self,
        key: &SecretBytes,
        payload: &[u8],
        tag: &[u8],
    ) -> Result<bool, AuthError> {
        let mut mac = Self::mac_for(key)?;
        mac.update(MAC_DOMAIN);
        mac.update(payload);
        Ok(mac.verify_slice(tag).is_ok())
    }

    /// A retired key verifies a tag only while it is still within its
    /// declared grace window; past `grace_until` it is refused identically
    /// to an unknown key, exactly like `token_key_is_current_or_in_grace`'s
    /// pure policy counterpart in `std.auth`.
    fn tag_matches_previous_key_in_grace(
        &self,
        payload: &[u8],
        tag: &[u8],
        now: u64,
    ) -> Result<bool, AuthError> {
        let Some(previous) = &self.previous_key else {
            return Ok(false);
        };
        if now > previous.grace_until {
            return Ok(false);
        }
        self.tag_matches(&previous.key, payload, tag)
    }

    fn mac_for(key: &SecretBytes) -> Result<HmacSha256, AuthError> {
        HmacSha256::new_from_slice(key.as_bytes()).map_err(|_| AuthError::InvalidPolicy)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Claims {
    id: SessionId,
    generation: u64,
    issued_at: u64,
    expires_at: u64,
    policy_id: String,
    issuer: String,
    audience: String,
    subject: String,
}

impl Claims {
    fn from_record(record: &SessionRecord, policy: &SessionPolicy) -> Self {
        Self {
            id: record.id.clone(),
            generation: record.generation,
            issued_at: record.issued_at,
            expires_at: record.expires_at,
            policy_id: policy.policy_id.clone(),
            issuer: policy.issuer.clone(),
            audience: policy.audience.clone(),
            subject: record.subject.clone(),
        }
    }

    fn public(&self) -> AuthenticatedSession {
        AuthenticatedSession {
            subject: self.subject.clone(),
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            generation: self.generation,
        }
    }

    fn encode(&self) -> Result<Vec<u8>, AuthError> {
        let mut payload = Vec::with_capacity(MAX_TOKEN_BYTES);
        payload.extend_from_slice(TOKEN_MAGIC);
        payload.push(TOKEN_VERSION);
        payload.extend_from_slice(&self.id.0);
        payload.extend_from_slice(&self.generation.to_be_bytes());
        payload.extend_from_slice(&self.issued_at.to_be_bytes());
        payload.extend_from_slice(&self.expires_at.to_be_bytes());
        push_text(&mut payload, &self.policy_id, MAX_POLICY_ID_BYTES)?;
        push_text(&mut payload, &self.issuer, MAX_ISSUER_BYTES)?;
        push_text(&mut payload, &self.audience, MAX_AUDIENCE_BYTES)?;
        push_text(&mut payload, &self.subject, MAX_SUBJECT_BYTES)?;
        Ok(payload)
    }

    fn decode(payload: &[u8]) -> Result<Self, AuthError> {
        if payload.len() < TOKEN_MAGIC.len() + 1 + SESSION_ID_BYTES + 24
            || payload.len() > MAX_TOKEN_BYTES
            || &payload[..4] != TOKEN_MAGIC
            || payload[4] != TOKEN_VERSION
        {
            return Err(AuthError::InvalidCredential);
        }
        let mut cursor = 5;
        let mut id = [0_u8; SESSION_ID_BYTES];
        id.copy_from_slice(take(payload, &mut cursor, SESSION_ID_BYTES)?);
        let generation = take_u64(payload, &mut cursor)?;
        let issued_at = take_u64(payload, &mut cursor)?;
        let expires_at = take_u64(payload, &mut cursor)?;
        if issued_at >= expires_at || id.iter().all(|byte| *byte == 0) {
            return Err(AuthError::InvalidCredential);
        }
        let policy_id = take_text(payload, &mut cursor, MAX_POLICY_ID_BYTES)?;
        let issuer = take_text(payload, &mut cursor, MAX_ISSUER_BYTES)?;
        let audience = take_text(payload, &mut cursor, MAX_AUDIENCE_BYTES)?;
        let subject = take_text(payload, &mut cursor, MAX_SUBJECT_BYTES)?;
        if cursor != payload.len() {
            return Err(AuthError::InvalidCredential);
        }
        Ok(Self {
            id: SessionId(id),
            generation,
            issued_at,
            expires_at,
            policy_id,
            issuer,
            audience,
            subject,
        })
    }
}

fn valid_text(value: &str, maximum: usize) -> Result<(), AuthError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > maximum
        || bytes
            .iter()
            .any(|byte| *byte == 0 || byte.is_ascii_control())
    {
        return Err(AuthError::InvalidInput);
    }
    Ok(())
}

fn push_text(output: &mut Vec<u8>, value: &str, maximum: usize) -> Result<(), AuthError> {
    valid_text(value, maximum)?;
    output.push(u8::try_from(value.len()).map_err(|_| AuthError::Capacity)?);
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], AuthError> {
    let end = cursor
        .checked_add(length)
        .ok_or(AuthError::InvalidCredential)?;
    let value = input
        .get(*cursor..end)
        .ok_or(AuthError::InvalidCredential)?;
    *cursor = end;
    Ok(value)
}

fn take_u64(input: &[u8], cursor: &mut usize) -> Result<u64, AuthError> {
    let bytes: [u8; 8] = take(input, cursor, 8)?
        .try_into()
        .map_err(|_| AuthError::InvalidCredential)?;
    Ok(u64::from_be_bytes(bytes))
}

fn take_text(input: &[u8], cursor: &mut usize, maximum: usize) -> Result<String, AuthError> {
    let length = usize::from(
        *take(input, cursor, 1)?
            .first()
            .ok_or(AuthError::InvalidCredential)?,
    );
    if length == 0 || length > maximum {
        return Err(AuthError::InvalidCredential);
    }
    let value = std::str::from_utf8(take(input, cursor, length)?)
        .map_err(|_| AuthError::InvalidCredential)?;
    valid_text(value, maximum).map_err(|_| AuthError::InvalidCredential)?;
    Ok(value.to_owned())
}

fn base64url_encode(input: &[u8], output: &mut String) -> Result<(), AuthError> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let encoded = input
        .len()
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(2))
        .map(|bytes| bytes / 3)
        .ok_or(AuthError::Capacity)?;
    output
        .try_reserve(encoded)
        .map_err(|_| AuthError::Capacity)?;
    for chunk in input.chunks(3) {
        let first = u32::from(chunk[0]) << 16;
        let second = chunk.get(1).map_or(0, |byte| u32::from(*byte)) << 8;
        let third = chunk.get(2).map_or(0, |byte| u32::from(*byte));
        let word = first | second | third;
        output.push(char::from(ALPHABET[((word >> 18) & 63) as usize]));
        output.push(char::from(ALPHABET[((word >> 12) & 63) as usize]));
        if chunk.len() > 1 {
            output.push(char::from(ALPHABET[((word >> 6) & 63) as usize]));
        }
        if chunk.len() > 2 {
            output.push(char::from(ALPHABET[(word & 63) as usize]));
        }
    }
    Ok(())
}

fn base64url_decode(input: &str) -> Result<Vec<u8>, AuthError> {
    if input.is_empty() || input.len() > MAX_TOKEN_BYTES || input.len() % 4 == 1 {
        return Err(AuthError::InvalidCredential);
    }
    let capacity = input
        .len()
        .checked_mul(3)
        .map(|bytes| bytes / 4 + 2)
        .ok_or(AuthError::InvalidCredential)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| AuthError::Capacity)?;
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let remaining = bytes.len() - index;
        let count = remaining.min(4);
        let a = base64url_value(bytes[index])?;
        let b = base64url_value(*bytes.get(index + 1).ok_or(AuthError::InvalidCredential)?)?;
        let c = if count > 2 {
            base64url_value(bytes[index + 2])?
        } else {
            0
        };
        let d = if count > 3 {
            base64url_value(bytes[index + 3])?
        } else {
            0
        };
        if count != 4 && remaining != count {
            return Err(AuthError::InvalidCredential);
        }
        if (count == 2 && b & 0x0f != 0) || (count == 3 && c & 0x03 != 0) {
            return Err(AuthError::InvalidCredential);
        }
        let word = (u32::from(a) << 18) | (u32::from(b) << 12) | (u32::from(c) << 6) | u32::from(d);
        output.push((word >> 16) as u8);
        if count > 2 {
            output.push((word >> 8) as u8);
        }
        if count > 3 {
            output.push(word as u8);
        }
        index += count;
    }
    Ok(output)
}

fn base64url_value(byte: u8) -> Result<u8, AuthError> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'-' => Ok(62),
        b'_' => Ok(63),
        _ => Err(AuthError::InvalidCredential),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Entropy(Vec<[u8; SESSION_ID_BYTES]>);

    impl AuthEntropy for Entropy {
        fn fill(&mut self, out: &mut [u8]) -> Result<(), AuthError> {
            let value = self.0.pop().ok_or(AuthError::Entropy)?;
            out.copy_from_slice(&value);
            Ok(())
        }
    }

    fn service() -> SessionService {
        SessionService::new(
            SessionPolicy::new("policy-1", "issuer", "audience", 1, 60).unwrap(),
            SecretBytes::try_from_bytes(&[7; 32]).unwrap(),
        )
        .unwrap()
    }

    fn entropy() -> Entropy {
        Entropy(vec![[2; SESSION_ID_BYTES], [1; SESSION_ID_BYTES]])
    }

    #[test]
    fn expiry_maintenance_reuses_one_capped_store_without_reviving_bearers() {
        let service = service();
        let mut store = InMemorySessionStore::new(2).unwrap();
        let mut entropy = Entropy(
            (1..=10)
                .rev()
                .map(|byte| [byte; SESSION_ID_BYTES])
                .collect(),
        );
        let first = service
            .issue(&mut store, &mut entropy, "alice", 10, 1)
            .unwrap();
        let second = service
            .issue(&mut store, &mut entropy, "bob", 10, 1)
            .unwrap();
        assert!(matches!(
            service.issue(&mut store, &mut entropy, "full", 10, 1),
            Err(AuthError::Capacity)
        ));
        assert_eq!(
            service.verify(&store, first.bearer(), 11),
            Err(AuthError::Expired)
        );
        assert_eq!(store.purge_expired(10), Ok(0));
        assert_eq!(store.len(), 2);
        assert_eq!(store.purge_expired(11), Ok(2));
        for tick in 11..16 {
            let current = service
                .issue(&mut store, &mut entropy, "carol", tick, 1)
                .unwrap();
            assert!(service.verify(&store, current.bearer(), tick).is_ok());
            assert_eq!(store.purge_expired(tick + 1), Ok(1));
            assert_eq!(
                service.verify(&store, current.bearer(), tick + 1),
                Err(AuthError::Expired)
            );
        }
        assert_eq!(
            service.verify(&store, second.bearer(), 16),
            Err(AuthError::Expired)
        );
        assert_eq!(store.purge_expired(15), Err(AuthError::InvalidInput));
        assert!(matches!(
            service.issue(&mut store, &mut entropy, "regressed", 10, 1),
            Err(AuthError::InvalidInput)
        ));
    }

    #[test]
    fn cleanup_never_revives_an_old_bearer_when_entropy_reuses_its_id() {
        let service = service();
        let mut store = InMemorySessionStore::new(1).unwrap();
        let reused = [9; SESSION_ID_BYTES];
        let mut entropy = Entropy(vec![reused, reused]);
        let old = service
            .issue(&mut store, &mut entropy, "alice", 10, 1)
            .unwrap();
        assert_eq!(store.purge_expired(11), Ok(1));
        let current = service
            .issue(&mut store, &mut entropy, "alice", 11, 10)
            .unwrap();
        // A host clock regression cannot make the original bearer match the
        // replacement record: record issue time is part of the authenticated
        // comparison, while the replacement bearer remains usable at its
        // admitted tick.
        assert_eq!(
            service.verify(&store, old.bearer(), 10),
            Err(AuthError::InvalidCredential)
        );
        assert!(service.verify(&store, current.bearer(), 11).is_ok());
    }

    #[test]
    fn expiry_maintenance_preserves_live_and_unexpired_retired_records() {
        let service = service();
        let mut store = InMemorySessionStore::new(4).unwrap();
        let mut entropy = Entropy((1..=5).rev().map(|byte| [byte; SESSION_ID_BYTES]).collect());
        let expiring = service
            .issue(&mut store, &mut entropy, "short", 10, 2)
            .unwrap();
        let live = service
            .issue(&mut store, &mut entropy, "live", 10, 30)
            .unwrap();
        let rotated = service
            .issue(&mut store, &mut entropy, "rotated", 10, 3)
            .unwrap();
        let successor = service
            .rotate(&mut store, &mut entropy, rotated.bearer(), 11, 5)
            .unwrap();
        assert_eq!(store.purge_expired(11), Ok(0));
        assert_eq!(store.purge_expired(12), Ok(1));
        assert_eq!(
            service.verify(&store, expiring.bearer(), 12),
            Err(AuthError::Expired)
        );
        assert!(service.verify(&store, live.bearer(), 12).is_ok());
        assert_eq!(
            service.verify(&store, rotated.bearer(), 12),
            Err(AuthError::Revoked)
        );
        assert_eq!(store.purge_expired(13), Ok(1));
        assert_eq!(
            service.verify(&store, rotated.bearer(), 13),
            Err(AuthError::Expired)
        );
        service.revoke(&mut store, successor.bearer(), 13).unwrap();
        assert_eq!(
            service.verify(&store, successor.bearer(), 13),
            Err(AuthError::Revoked)
        );
        assert_eq!(store.purge_expired(16), Ok(1));
        assert_eq!(
            service.verify(&store, successor.bearer(), 16),
            Err(AuthError::Expired)
        );
        assert!(service.verify(&store, live.bearer(), 16).is_ok());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn issue_verify_rotate_and_revoke_are_generation_bound() {
        let service = service();
        let mut store = InMemorySessionStore::new(4).unwrap();
        let mut entropy = entropy();
        let token = service
            .issue(&mut store, &mut entropy, "user-1", 10, 20)
            .unwrap();
        assert_eq!(
            service
                .verify(&store, token.bearer(), 11)
                .unwrap()
                .subject(),
            "user-1"
        );
        let replacement = service
            .rotate(&mut store, &mut entropy, token.bearer(), 12, 20)
            .unwrap();
        assert_eq!(
            service.verify(&store, token.bearer(), 12),
            Err(AuthError::Revoked)
        );
        assert_eq!(
            service
                .verify(&store, replacement.bearer(), 12)
                .unwrap()
                .generation(),
            1
        );
        service
            .revoke(&mut store, replacement.bearer(), 13)
            .unwrap();
        assert_eq!(
            service.verify(&store, replacement.bearer(), 13),
            Err(AuthError::Revoked)
        );
    }

    #[test]
    fn tampering_key_confusion_algorithm_confusion_and_expiry_fail_closed() {
        let service = service();
        let mut store = InMemorySessionStore::new(4).unwrap();
        let mut entropy = entropy();
        let token = service
            .issue(&mut store, &mut entropy, "user-1", 10, 1)
            .unwrap();
        let mut tampered = token.bearer().as_bytes().to_vec();
        tampered[0] = if tampered[0] == b'A' { b'B' } else { b'A' };
        assert_eq!(
            service.verify(&store, std::str::from_utf8(&tampered).unwrap(), 10),
            Err(AuthError::InvalidCredential)
        );
        assert_eq!(
            service.verify(&store, "eyJhbGciOiJub25lIn0.e30", 10),
            Err(AuthError::InvalidCredential)
        );
        let other_key = SessionService::new(
            service.policy().clone(),
            SecretBytes::try_from_bytes(&[9; 32]).unwrap(),
        )
        .unwrap();
        assert_eq!(
            other_key.verify(&store, token.bearer(), 10),
            Err(AuthError::InvalidCredential)
        );
        assert_eq!(
            service.verify(&store, token.bearer(), 11),
            Err(AuthError::Expired)
        );
    }

    #[test]
    fn policy_bounds_store_capacity_and_invalid_entropy_are_explicit() {
        assert_eq!(
            SessionPolicy::new("p", "i", "a", 0, 1),
            Err(AuthError::InvalidPolicy)
        );
        assert!(matches!(
            InMemorySessionStore::new(0),
            Err(AuthError::InvalidPolicy)
        ));
        let service = service();
        let mut store = InMemorySessionStore::new(1).unwrap();
        let mut entropy = Entropy(vec![[0; SESSION_ID_BYTES]]);
        assert!(matches!(
            service.issue(&mut store, &mut entropy, "user", 1, 1),
            Err(AuthError::Entropy)
        ));
    }

    #[test]
    fn future_issue_and_tightened_policy_fail_closed() {
        let service = service();
        let mut store = InMemorySessionStore::new(4).unwrap();
        let mut entropy = entropy();
        let token = service
            .issue(&mut store, &mut entropy, "user-1", 10, 20)
            .unwrap();
        assert_eq!(
            service.verify(&store, token.bearer(), 9),
            Err(AuthError::InvalidCredential)
        );

        let tightened = SessionService::new(
            SessionPolicy::new("policy-1", "issuer", "audience", 21, 60).unwrap(),
            SecretBytes::try_from_bytes(&[7; 32]).unwrap(),
        )
        .unwrap();
        assert_eq!(
            tightened.verify(&store, token.bearer(), 10),
            Err(AuthError::InvalidCredential)
        );
    }

    #[test]
    fn signing_key_rotation_accepts_the_old_key_only_within_its_grace_window() {
        let policy = SessionPolicy::new("policy-1", "issuer", "audience", 1, 60).unwrap();
        let old_key = SecretBytes::try_from_bytes(&[1; 32]).unwrap();
        let issuing_service = SessionService::new(policy.clone(), old_key).unwrap();
        let mut store = InMemorySessionStore::new(4).unwrap();
        let mut entropy = entropy();
        let token = issuing_service
            .issue(&mut store, &mut entropy, "user-1", 10, 40)
            .unwrap();

        // A service that knows only the new key cannot verify a pre-rotation
        // token at all.
        let new_key_only = SessionService::new(
            policy.clone(),
            SecretBytes::try_from_bytes(&[2; 32]).unwrap(),
        )
        .unwrap();
        assert_eq!(
            new_key_only.verify(&store, token.bearer(), 11),
            Err(AuthError::InvalidCredential)
        );

        // A rotated service names the retired key with an explicit grace
        // deadline. Within the window the old token verifies; new tokens it
        // issues are signed with the new key, never the retired one.
        let rotated = SessionService::with_key_rotation(
            policy.clone(),
            SecretBytes::try_from_bytes(&[2; 32]).unwrap(),
            Some(
                SigningKeyRotation::new(SecretBytes::try_from_bytes(&[1; 32]).unwrap(), 20)
                    .unwrap(),
            ),
        )
        .unwrap();
        assert_eq!(
            rotated
                .verify(&store, token.bearer(), 15)
                .unwrap()
                .subject(),
            "user-1"
        );
        let fresh = rotated
            .issue(&mut store, &mut entropy, "user-2", 15, 20)
            .unwrap();
        // The new token is signed with the new key: a party holding only the
        // new key verifies it directly.
        assert_eq!(
            new_key_only
                .verify(&store, fresh.bearer(), 16)
                .unwrap()
                .subject(),
            "user-2"
        );
        // A party holding only the retired key (no configured previous-key
        // grace) cannot verify a post-rotation token.
        assert_eq!(
            issuing_service.verify(&store, fresh.bearer(), 16),
            Err(AuthError::InvalidCredential)
        );
        assert_eq!(
            rotated
                .verify(&store, fresh.bearer(), 16)
                .unwrap()
                .subject(),
            "user-2"
        );

        // Past the grace deadline the retired key is refused identically to
        // an unknown key, even though the session record itself is still
        // otherwise active and unexpired.
        assert_eq!(
            rotated.verify(&store, token.bearer(), 21),
            Err(AuthError::InvalidCredential)
        );

        // A rotation cannot name the same key twice.
        assert_eq!(
            SessionService::with_key_rotation(
                policy,
                SecretBytes::try_from_bytes(&[2; 32]).unwrap(),
                Some(
                    SigningKeyRotation::new(SecretBytes::try_from_bytes(&[2; 32]).unwrap(), 20)
                        .unwrap()
                ),
            )
            .err(),
            Some(AuthError::InvalidPolicy)
        );
    }

    #[test]
    fn oversized_truncated_and_control_byte_bearer_tokens_are_refused() {
        let service = service();
        let mut store = InMemorySessionStore::new(4).unwrap();
        let mut entropy = entropy();
        let token = service
            .issue(&mut store, &mut entropy, "user-1", 10, 20)
            .unwrap();

        // Oversized: well past MAX_TOKEN_BYTES.
        let oversized = "A".repeat(MAX_TOKEN_BYTES + 1);
        assert_eq!(
            service.verify(&store, &oversized, 11),
            Err(AuthError::InvalidCredential)
        );

        // Truncated: payload with no tag, and an empty bearer.
        let mut truncated = token.bearer().split('.');
        let payload_only = truncated.next().unwrap();
        assert_eq!(
            service.verify(&store, payload_only, 11),
            Err(AuthError::InvalidCredential)
        );
        assert_eq!(
            service.verify(&store, "", 11),
            Err(AuthError::InvalidCredential)
        );

        // Control bytes and interior NUL are not valid base64url and cannot
        // decode to a real payload or tag.
        assert_eq!(
            service.verify(&store, "A\0B.CDEF", 11),
            Err(AuthError::InvalidCredential)
        );
        assert_eq!(
            service.verify(&store, "\n\t.\r", 11),
            Err(AuthError::InvalidCredential)
        );

        // Multi-byte UTF-8 (unicode-confusable content) is never valid
        // base64url either.
        assert_eq!(
            service.verify(&store, "café.token", 11),
            Err(AuthError::InvalidCredential)
        );
    }

    #[test]
    fn control_bytes_and_unicode_confusable_subjects_are_refused_at_issue() {
        let service = service();
        let mut store = InMemorySessionStore::new(4).unwrap();
        let mut rejected_entropy = entropy();
        // Invalid subjects are refused before any entropy is consumed, so
        // one fixture with unused fills is enough for both cases.
        assert_eq!(
            service
                .issue(&mut store, &mut rejected_entropy, "user\u{0}one", 10, 20)
                .err(),
            Some(AuthError::InvalidInput)
        );
        assert_eq!(
            service
                .issue(&mut store, &mut rejected_entropy, "user\ncontrol", 10, 20)
                .err(),
            Some(AuthError::InvalidInput)
        );
        // A confusable subject (Cyrillic "а" instead of Latin "a") is
        // distinct bytes, not silently normalized or conflated.
        let mut latin_entropy = entropy();
        let latin = service
            .issue(&mut store, &mut latin_entropy, "alice", 10, 20)
            .unwrap();
        let mut confusable_entropy = Entropy(vec![[3; SESSION_ID_BYTES]]);
        let confusable = service
            .issue(&mut store, &mut confusable_entropy, "\u{0430}lice", 10, 20)
            .unwrap();
        assert_ne!(
            service
                .verify(&store, latin.bearer(), 11)
                .unwrap()
                .subject(),
            service
                .verify(&store, confusable.bearer(), 11)
                .unwrap()
                .subject()
        );
    }

    #[test]
    fn concurrent_sessions_for_the_same_subject_are_independent() {
        let service = service();
        let mut store = InMemorySessionStore::new(4).unwrap();
        let mut entropy = Entropy(vec![[5; SESSION_ID_BYTES], [4; SESSION_ID_BYTES]]);
        let first = service
            .issue(&mut store, &mut entropy, "user-1", 10, 20)
            .unwrap();
        let second = service
            .issue(&mut store, &mut entropy, "user-1", 10, 20)
            .unwrap();
        assert_ne!(first.bearer(), second.bearer());
        // Both sessions for the same subject are simultaneously valid.
        assert_eq!(
            service
                .verify(&store, first.bearer(), 11)
                .unwrap()
                .subject(),
            "user-1"
        );
        assert_eq!(
            service
                .verify(&store, second.bearer(), 11)
                .unwrap()
                .subject(),
            "user-1"
        );
        // Revoking one leaves the other active: revocation is per-session,
        // not per-subject.
        service.revoke(&mut store, first.bearer(), 12).unwrap();
        assert_eq!(
            service.verify(&store, first.bearer(), 12),
            Err(AuthError::Revoked)
        );
        assert_eq!(
            service
                .verify(&store, second.bearer(), 12)
                .unwrap()
                .subject(),
            "user-1"
        );
    }
}
