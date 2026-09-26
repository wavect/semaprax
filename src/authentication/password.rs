//! Bounded Argon2id password storage for the native authentication host.
//!
//! This module deliberately accepts only the PHC shape it writes.  Parsing a
//! stored record checks its complete, small envelope before an Argon2
//! invocation is possible; verification then admits the current policy or an
//! explicitly configured, bounded migration policy.

use std::fmt;

use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher as _, PasswordVerifier as _},
    Algorithm, Argon2, Params, Version,
};

use super::{AuthEntropy, AuthError, SecretBytes};

const MIN_MEMORY_KIB: u32 = 19_456;
const MAX_MEMORY_KIB: u32 = 65_536;
const MIN_ITERATIONS: u32 = 2;
const MAX_ITERATIONS: u32 = 4;
const PARALLELISM: u32 = 1;
const SALT_BYTES: usize = 16;
const OUTPUT_BYTES: usize = 32;
const MAX_PASSWORD_BYTES: usize = 1_024;
const MAX_PHC_BYTES: usize = 512;
const MAX_MIGRATIONS: usize = 4;

/// The only Argon2id resource shapes admitted by this host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PasswordPolicy {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl PasswordPolicy {
    /// Creates a policy within the fixed host resource envelope.
    pub fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Result<Self, AuthError> {
        if !(MIN_MEMORY_KIB..=MAX_MEMORY_KIB).contains(&memory_kib)
            || !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&iterations)
            || parallelism != PARALLELISM
        {
            return Err(AuthError::InvalidPolicy);
        }
        Ok(Self {
            memory_kib,
            iterations,
            parallelism,
        })
    }

    pub fn memory_kib(self) -> u32 {
        self.memory_kib
    }

    pub fn iterations(self) -> u32 {
        self.iterations
    }

    pub fn parallelism(self) -> u32 {
        self.parallelism
    }
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            memory_kib: MIN_MEMORY_KIB,
            iterations: MIN_ITERATIONS,
            parallelism: PARALLELISM,
        }
    }
}

/// A validated, redacted PHC password record suitable for host storage.
pub struct StoredPasswordHash {
    encoded: String,
}

impl StoredPasswordHash {
    /// Validates a bounded Argon2id v19 PHC record before retaining it.
    pub fn parse_for_storage(encoded: &str) -> Result<Self, AuthError> {
        validate_record(encoded)?;
        PasswordHash::new(encoded).map_err(|_| AuthError::InvalidCredential)?;
        Ok(Self {
            encoded: encoded.to_owned(),
        })
    }

    /// Returns the validated PHC record for the host's storage adapter.
    pub fn expose_for_storage(&self) -> &str {
        &self.encoded
    }
}

impl fmt::Debug for StoredPasswordHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StoredPasswordHash(REDACTED)")
    }
}

/// Password KDF and verification service with an explicit migration set.
pub struct PasswordHasherHost {
    policy: PasswordPolicy,
    approved_migrations: Vec<PasswordPolicy>,
}

impl PasswordHasherHost {
    pub fn new(policy: PasswordPolicy) -> Result<Self, AuthError> {
        Self::with_approved_migrations(policy, &[])
    }

    /// Creates a hasher which will also verify at most four explicit old
    /// policies.  New hashes always use `policy`.
    pub fn with_approved_migrations(
        policy: PasswordPolicy,
        approved_migrations: &[PasswordPolicy],
    ) -> Result<Self, AuthError> {
        if approved_migrations.len() > MAX_MIGRATIONS || approved_migrations.contains(&policy) {
            return Err(AuthError::InvalidPolicy);
        }
        Ok(Self {
            policy,
            approved_migrations: approved_migrations.to_vec(),
        })
    }

    pub fn policy(&self) -> PasswordPolicy {
        self.policy
    }

    /// Hashes a bounded secret with a fresh 16-byte host-supplied salt.
    pub fn hash(
        &self,
        password: &SecretBytes,
        entropy: &mut impl AuthEntropy,
    ) -> Result<StoredPasswordHash, AuthError> {
        ensure_password_size(password)?;
        let mut salt_bytes = [0_u8; SALT_BYTES];
        entropy.fill(&mut salt_bytes)?;
        let encoded = self
            .argon2_for(self.policy)?
            .hash_password_with_salt(password.as_bytes(), &salt_bytes)
            .map_err(|_| AuthError::Storage)?
            .to_string();
        StoredPasswordHash::parse_for_storage(&encoded).map_err(|_| AuthError::Storage)
    }

    /// Verifies a record only after its complete PHC envelope and policy have
    /// been checked.  A post-dispatch mismatch is deliberately indistinct from
    /// an invalid stored credential.
    pub fn verify(
        &self,
        password: &SecretBytes,
        stored: &StoredPasswordHash,
    ) -> Result<(), AuthError> {
        ensure_password_size(password)?;
        let stored_policy = validate_record(stored.expose_for_storage())?;
        if stored_policy != self.policy && !self.approved_migrations.contains(&stored_policy) {
            return Err(AuthError::InvalidPolicy);
        }
        let parsed = PasswordHash::new(stored.expose_for_storage())
            .map_err(|_| AuthError::InvalidCredential)?;
        self.argon2_for(stored_policy)?
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| AuthError::InvalidCredential)
    }

    fn argon2_for(&self, policy: PasswordPolicy) -> Result<Argon2<'static>, AuthError> {
        let params = Params::new(
            policy.memory_kib,
            policy.iterations,
            policy.parallelism,
            Some(OUTPUT_BYTES),
        )
        .map_err(|_| AuthError::InvalidPolicy)?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

fn ensure_password_size(password: &SecretBytes) -> Result<(), AuthError> {
    if password.as_bytes().len() > MAX_PASSWORD_BYTES {
        return Err(AuthError::Capacity);
    }
    Ok(())
}

fn validate_record(encoded: &str) -> Result<PasswordPolicy, AuthError> {
    if encoded.len() > MAX_PHC_BYTES {
        return Err(AuthError::Capacity);
    }
    if !encoded.is_ascii() {
        return Err(AuthError::InvalidCredential);
    }
    let mut fields = encoded.split('$');
    let (Some(prefix), Some(algorithm), Some(version), Some(params), Some(salt), Some(digest)) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err(AuthError::InvalidCredential);
    };
    if fields.next().is_some() || !prefix.is_empty() || algorithm != "argon2id" || version != "v=19"
    {
        return Err(AuthError::InvalidCredential);
    }
    let policy = parse_params(params)?;
    if !canonical_base64(salt, SALT_BYTES) || !canonical_base64(digest, OUTPUT_BYTES) {
        return Err(AuthError::InvalidCredential);
    }
    Ok(policy)
}

fn parse_params(params: &str) -> Result<PasswordPolicy, AuthError> {
    let mut memory_kib = None;
    let mut iterations = None;
    let mut parallelism = None;
    for item in params.split(',') {
        let (key, value) = item.split_once('=').ok_or(AuthError::InvalidPolicy)?;
        let value = parse_decimal(value)?;
        let slot = match key {
            "m" => &mut memory_kib,
            "t" => &mut iterations,
            "p" => &mut parallelism,
            _ => return Err(AuthError::InvalidPolicy),
        };
        if slot.replace(value).is_some() {
            return Err(AuthError::InvalidPolicy);
        }
    }
    PasswordPolicy::new(
        memory_kib.ok_or(AuthError::InvalidPolicy)?,
        iterations.ok_or(AuthError::InvalidPolicy)?,
        parallelism.ok_or(AuthError::InvalidPolicy)?,
    )
}

fn parse_decimal(value: &str) -> Result<u32, AuthError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AuthError::InvalidPolicy);
    }
    value.parse().map_err(|_| AuthError::InvalidPolicy)
}

fn canonical_base64(value: &str, expected_bytes: usize) -> bool {
    let expected_len = match expected_bytes % 3 {
        0 => expected_bytes / 3 * 4,
        1 => expected_bytes / 3 * 4 + 2,
        _ => expected_bytes / 3 * 4 + 3,
    };
    if value.len() != expected_len {
        return false;
    }
    let mut last = 0_u8;
    for byte in value.bytes() {
        let Some(sextet) = base64_sextet(byte) else {
            return false;
        };
        last = sextet;
    }
    match value.len() % 4 {
        0 => true,
        2 => last & 0b0000_1111 == 0,
        3 => last & 0b0000_0011 == 0,
        _ => false,
    }
}

fn base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedEntropy(u8);

    impl AuthEntropy for FixedEntropy {
        fn fill(&mut self, out: &mut [u8]) -> Result<(), AuthError> {
            out.fill(self.0);
            Ok(())
        }
    }

    fn secret(bytes: &[u8]) -> SecretBytes {
        SecretBytes::try_from_bytes(bytes).expect("bounded test secret")
    }

    #[test]
    fn wrong_password_is_rejected_by_argon2id() {
        let hasher = PasswordHasherHost::new(PasswordPolicy::default()).unwrap();
        let mut entropy = FixedEntropy(1);
        let stored = hasher.hash(&secret(b"right"), &mut entropy).unwrap();
        hasher.verify(&secret(b"right"), &stored).unwrap();
        assert_eq!(
            hasher.verify(&secret(b"wrong"), &stored),
            Err(AuthError::InvalidCredential)
        );
    }

    #[test]
    fn fresh_entropy_changes_the_stored_salt() {
        let hasher = PasswordHasherHost::new(PasswordPolicy::default()).unwrap();
        let mut first_entropy = FixedEntropy(1);
        let mut second_entropy = FixedEntropy(2);
        let first = hasher.hash(&secret(b"same"), &mut first_entropy).unwrap();
        let second = hasher.hash(&secret(b"same"), &mut second_entropy).unwrap();
        assert_ne!(first.expose_for_storage(), second.expose_for_storage());
    }

    #[test]
    fn malformed_or_oversized_parameters_are_refused_before_verification() {
        assert!(matches!(
            StoredPasswordHash::parse_for_storage(
                "$argon2id$v=19$m=999999999,t=2,p=1$AQEBAQEBAQEBAQEBAQEBAQ$AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"
            ),
            Err(AuthError::InvalidPolicy)
        ));
        let oversized = format!("${}", "x".repeat(MAX_PHC_BYTES));
        assert!(matches!(
            StoredPasswordHash::parse_for_storage(&oversized),
            Err(AuthError::Capacity)
        ));
    }

    #[test]
    fn policy_bounds_reject_out_of_range_iterations_and_parallelism() {
        // Floor and ceiling on iterations, each individually out of bounds.
        assert_eq!(
            PasswordPolicy::new(MIN_MEMORY_KIB, MIN_ITERATIONS - 1, PARALLELISM),
            Err(AuthError::InvalidPolicy)
        );
        assert_eq!(
            PasswordPolicy::new(MIN_MEMORY_KIB, MAX_ITERATIONS + 1, PARALLELISM),
            Err(AuthError::InvalidPolicy)
        );
        // Only parallelism == 1 is admitted; a resource-exhaustion attempt via
        // a larger parallelism is refused, not silently clamped.
        assert_eq!(
            PasswordPolicy::new(MIN_MEMORY_KIB, MIN_ITERATIONS, PARALLELISM + 1),
            Err(AuthError::InvalidPolicy)
        );
        assert_eq!(
            PasswordPolicy::new(MIN_MEMORY_KIB, MIN_ITERATIONS, 0),
            Err(AuthError::InvalidPolicy)
        );
        // The exact bounds are admitted (paired positive control).
        assert!(PasswordPolicy::new(MIN_MEMORY_KIB, MIN_ITERATIONS, PARALLELISM).is_ok());
        assert!(PasswordPolicy::new(MAX_MEMORY_KIB, MAX_ITERATIONS, PARALLELISM).is_ok());
    }

    #[test]
    fn approved_migration_verifies_old_hash_but_new_hashes_always_use_current_policy() {
        let old_policy = PasswordPolicy::new(MIN_MEMORY_KIB, MIN_ITERATIONS, PARALLELISM).unwrap();
        let current_policy =
            PasswordPolicy::new(MIN_MEMORY_KIB, MAX_ITERATIONS, PARALLELISM).unwrap();
        assert_ne!(old_policy, current_policy);

        // A record hashed under the old policy while it was current.
        let old_hasher = PasswordHasherHost::new(old_policy).unwrap();
        let mut entropy = FixedEntropy(3);
        let legacy = old_hasher
            .hash(&secret(b"legacy password"), &mut entropy)
            .unwrap();

        // Without an explicit migration, a hasher on the new policy refuses
        // to verify a record produced under a different, unapproved policy.
        let strict_hasher = PasswordHasherHost::new(current_policy).unwrap();
        assert_eq!(
            strict_hasher.verify(&secret(b"legacy password"), &legacy),
            Err(AuthError::InvalidPolicy)
        );

        // With the old policy explicitly approved as a migration, the legacy
        // record verifies, a wrong password against it still fails, and a
        // freshly issued hash uses the current policy rather than the
        // migration policy.
        let migrating_hasher =
            PasswordHasherHost::with_approved_migrations(current_policy, &[old_policy]).unwrap();
        migrating_hasher
            .verify(&secret(b"legacy password"), &legacy)
            .unwrap();
        assert_eq!(
            migrating_hasher.verify(&secret(b"wrong password"), &legacy),
            Err(AuthError::InvalidCredential)
        );
        let rehashed = migrating_hasher
            .hash(&secret(b"legacy password"), &mut entropy)
            .unwrap();
        assert!(rehashed.expose_for_storage().contains("m=19456,t=4,p=1"));
        assert_eq!(migrating_hasher.policy(), current_policy);

        // A migration set cannot include the active policy itself, and is
        // bounded at four entries.
        assert_eq!(
            PasswordHasherHost::with_approved_migrations(current_policy, &[current_policy]).err(),
            Some(AuthError::InvalidPolicy)
        );
        let five = [old_policy; 5];
        assert_eq!(
            PasswordHasherHost::with_approved_migrations(current_policy, &five).err(),
            Some(AuthError::InvalidPolicy)
        );
    }
}
