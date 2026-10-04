//! Actual password/session composition for embedding applications.
use super::password::{PasswordHasherHost, StoredPasswordHash};
use super::session::{
    AuthenticatedSession, InMemorySessionStore, SessionService, SessionStore, SessionToken,
};
use super::{AuthEntropy, AuthError, SecretBytes};
use std::collections::BTreeMap;

/// Storage implementations must atomically refuse duplicate account identifiers.
/// Only the password hash, never the plaintext password, enters this interface.
pub trait AccountStore {
    fn lookup(&self, subject: &str) -> Result<Option<&StoredPasswordHash>, AuthError>;
    fn insert_if_absent(
        &mut self,
        subject: &str,
        password: StoredPasswordHash,
    ) -> Result<(), AuthError>;
}

pub struct InMemoryAccountStore {
    capacity: usize,
    accounts: BTreeMap<String, StoredPasswordHash>,
}
impl InMemoryAccountStore {
    pub fn new(capacity: usize) -> Result<Self, AuthError> {
        if capacity == 0 || capacity > 4096 {
            return Err(AuthError::InvalidPolicy);
        }
        Ok(Self {
            capacity,
            accounts: BTreeMap::new(),
        })
    }
}
fn validate_subject(subject: &str) -> Result<(), AuthError> {
    if subject.is_empty()
        || subject.len() > 128
        || !subject
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-@".contains(&b))
    {
        return Err(AuthError::InvalidInput);
    }
    Ok(())
}
impl AccountStore for InMemoryAccountStore {
    fn lookup(&self, subject: &str) -> Result<Option<&StoredPasswordHash>, AuthError> {
        validate_subject(subject)?;
        Ok(self.accounts.get(subject))
    }
    fn insert_if_absent(
        &mut self,
        subject: &str,
        password: StoredPasswordHash,
    ) -> Result<(), AuthError> {
        validate_subject(subject)?;
        if self.accounts.contains_key(subject) {
            return Err(AuthError::Conflict);
        }
        if self.accounts.len() >= self.capacity {
            return Err(AuthError::Capacity);
        }
        self.accounts.insert(subject.to_owned(), password);
        Ok(())
    }
}

/// Each method is synchronous; hosts must bound concurrent hashing and apply
/// request rate limits. Storage and session authority are explicit parameters.
pub struct AuthService {
    passwords: PasswordHasherHost,
    sessions: SessionService,
}
impl AuthService {
    pub fn new(passwords: PasswordHasherHost, sessions: SessionService) -> Self {
        Self {
            passwords,
            sessions,
        }
    }
    /// Signup creates an account only. A storage failure never issues a session.
    pub fn signup(
        &self,
        accounts: &mut dyn AccountStore,
        entropy: &mut impl AuthEntropy,
        subject: &str,
        password: &SecretBytes,
    ) -> Result<(), AuthError> {
        validate_subject(subject)?;
        let hash = self.passwords.hash(password, entropy)?;
        accounts.insert_if_absent(subject, hash)
    }
    /// Unknown accounts and wrong passwords have the same error tag. This does
    /// not promise identical timing; rate limiting remains the host's responsibility.
    #[allow(clippy::too_many_arguments)]
    pub fn login(
        &self,
        accounts: &dyn AccountStore,
        sessions: &mut dyn SessionStore,
        entropy: &mut impl AuthEntropy,
        subject: &str,
        password: &SecretBytes,
        now: u64,
        ttl: u64,
    ) -> Result<SessionToken, AuthError> {
        validate_subject(subject)?;
        let hash = accounts
            .lookup(subject)?
            .ok_or(AuthError::InvalidCredential)?;
        self.passwords.verify(password, hash)?;
        self.sessions.issue(sessions, entropy, subject, now, ttl)
    }
    /// Explicit host-time maintenance for the capped process-local session
    /// store. Call before login when expired records may consume capacity.
    pub fn purge_expired_sessions(
        &self,
        sessions: &mut InMemorySessionStore,
        now: u64,
    ) -> Result<usize, AuthError> {
        sessions.purge_expired(now)
    }
    pub fn protected(
        &self,
        sessions: &dyn SessionStore,
        bearer: &str,
        now: u64,
    ) -> Result<AuthenticatedSession, AuthError> {
        self.sessions.verify(sessions, bearer, now)
    }
    pub fn rotate(
        &self,
        sessions: &mut dyn SessionStore,
        entropy: &mut impl AuthEntropy,
        bearer: &str,
        now: u64,
        ttl: u64,
    ) -> Result<SessionToken, AuthError> {
        self.sessions.rotate(sessions, entropy, bearer, now, ttl)
    }
    pub fn logout(
        &self,
        sessions: &mut dyn SessionStore,
        bearer: &str,
        now: u64,
    ) -> Result<(), AuthError> {
        self.sessions.revoke(sessions, bearer, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authentication::password::PasswordPolicy;
    use crate::authentication::session::{InMemorySessionStore, SessionPolicy};
    struct Entropy(u8);
    impl AuthEntropy for Entropy {
        fn fill(&mut self, out: &mut [u8]) -> Result<(), AuthError> {
            self.0 = self.0.checked_add(1).ok_or(AuthError::Entropy)?;
            out.fill(self.0);
            Ok(())
        }
    }
    #[test]
    fn actual_signup_login_rotation_protected_logout() {
        let sessions = SessionService::new(
            SessionPolicy::new("app", "issuer", "audience", 1, 100).unwrap(),
            SecretBytes::try_from_bytes(&[42; 32]).unwrap(),
        )
        .unwrap();
        let service = AuthService::new(
            PasswordHasherHost::new(PasswordPolicy::default()).unwrap(),
            sessions,
        );
        let mut accounts = InMemoryAccountStore::new(8).unwrap();
        let mut sessions = InMemorySessionStore::new(8).unwrap();
        let mut entropy = Entropy(0);
        let password = SecretBytes::try_from_bytes(b"correct horse battery staple").unwrap();
        service
            .signup(&mut accounts, &mut entropy, "alice", &password)
            .unwrap();
        let wrong = SecretBytes::try_from_bytes(b"wrong").unwrap();
        assert!(matches!(
            service.login(
                &accounts,
                &mut sessions,
                &mut entropy,
                "alice",
                &wrong,
                10,
                20
            ),
            Err(AuthError::InvalidCredential)
        ));
        let token = service
            .login(
                &accounts,
                &mut sessions,
                &mut entropy,
                "alice",
                &password,
                10,
                20,
            )
            .unwrap();
        assert_eq!(
            service
                .protected(&sessions, token.bearer(), 11)
                .unwrap()
                .subject(),
            "alice"
        );
        let rotated = service
            .rotate(&mut sessions, &mut entropy, token.bearer(), 12, 20)
            .unwrap();
        assert!(service.protected(&sessions, token.bearer(), 13).is_err());
        assert_eq!(
            service
                .protected(&sessions, rotated.bearer(), 13)
                .unwrap()
                .subject(),
            "alice"
        );
        service.logout(&mut sessions, rotated.bearer(), 14).unwrap();
        assert!(service.protected(&sessions, rotated.bearer(), 15).is_err());
        assert_eq!(service.purge_expired_sessions(&mut sessions, 32), Ok(2));
        let later = service
            .login(
                &accounts,
                &mut sessions,
                &mut entropy,
                "alice",
                &password,
                32,
                20,
            )
            .unwrap();
        assert!(service.protected(&sessions, later.bearer(), 32).is_ok());
        assert_eq!(format!("{:?}", password), "SecretBytes([REDACTED])");
    }
}
