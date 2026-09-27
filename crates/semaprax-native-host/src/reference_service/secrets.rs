//! Host-held secret resolution for the reference service.
//!
//! The decoded adapter request names four secret *references*; this module
//! resolves them against a directory the host operator explicitly holds.
//! Each reference names one exact regular file; directory traversal is
//! impossible because the decoder's reference grammar admits no separator,
//! and every read follows the store's hold/read/recheck discipline. A
//! missing, empty, over-bound, or wrong-sized secret refuses startup before
//! any listener or store is touched.

use std::ffi::OsStr;
use std::fmt;

use semaprax::project::service_host_adapter_request::ServiceSecretResolutionRequirement;
use semaprax_native_rust_interop_platform as platform;
use semaprax_native_rust_interop_platform::HeldDirectory;

/// The maximum secret file size admitted. Secrets are keys, not documents.
pub const MAX_SECRET_BYTES: usize = 4_096;
/// The exact HMAC key size for session and webhook keys.
pub const HMAC_KEY_BYTES: usize = 32;
const MIN_PEPPER_BYTES: usize = 16;
const MAX_PEPPER_BYTES: usize = 64;
/// The maximum bytes for a held TLS leaf certificate (DER, one certificate,
/// no intermediate chain -- this reference host serves loopback
/// local/test deployments, not a publicly chained one).
pub const MAX_TLS_CERTIFICATE_BYTES: usize = 8_192;
/// The maximum bytes for a held TLS private key (PKCS#8 DER).
pub const MAX_TLS_PRIVATE_KEY_BYTES: usize = 4_096;

/// Stable refusal categories for secret resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretRefusal {
    /// A named secret file is absent, unreadable, or failed recheck.
    Unavailable,
    /// A secret is empty, over-bound, or the wrong size for its use.
    Invalid,
}

/// The four host-held secrets. Debug is redacted; these bytes live only in
/// this process and are never logged, rendered, or embedded in snapshots.
pub struct HeldServiceSecrets {
    password_pepper: Vec<u8>,
    session_key: [u8; HMAC_KEY_BYTES],
    webhook_key: [u8; HMAC_KEY_BYTES],
    /// The database DSN the decoded host-mode intent names. The reference
    /// snapshot store takes no DSN, so this value is held but never
    /// connected to; resolving it proves the host holds every named secret
    /// before serving, and no silent downgrade to "no DSN" is possible.
    database_dsn: Vec<u8>,
}

impl fmt::Debug for HeldServiceSecrets {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HeldServiceSecrets")
            .field("password_pepper", &"[REDACTED]")
            .field("session_key", &"[REDACTED]")
            .field("webhook_key", &"[REDACTED]")
            .field("database_dsn", &"[REDACTED]")
            .finish()
    }
}

impl HeldServiceSecrets {
    pub fn password_pepper(&self) -> &[u8] {
        &self.password_pepper
    }

    pub fn session_key(&self) -> &[u8; HMAC_KEY_BYTES] {
        &self.session_key
    }

    pub fn webhook_key(&self) -> &[u8; HMAC_KEY_BYTES] {
        &self.webhook_key
    }

    /// The held DSN's length only: this proves the host holds the named
    /// value without exposing or connecting it.
    pub fn database_dsn_len(&self) -> usize {
        self.database_dsn.len()
    }
}

/// Resolve every secret the decoded host-mode intent names against the
/// operator-held directory. References are decoder-validated
/// (`[a-z][a-z0-9._-]{0,127}`), so each names exactly one file.
pub fn resolve(
    directory: &HeldDirectory,
    secrets: &ServiceSecretResolutionRequirement,
    dsn_secret_reference: &str,
) -> Result<HeldServiceSecrets, SecretRefusal> {
    let password_pepper = read_secret(directory, secrets.password_pepper_reference())?;
    if password_pepper.len() < MIN_PEPPER_BYTES || password_pepper.len() > MAX_PEPPER_BYTES {
        return Err(SecretRefusal::Invalid);
    }
    let session_key = read_key(directory, secrets.session_signing_key_reference())?;
    let webhook_key = read_key(directory, secrets.webhook_signing_key_reference())?;
    let database_dsn = read_secret(directory, dsn_secret_reference)?;
    Ok(HeldServiceSecrets {
        password_pepper,
        session_key,
        webhook_key,
        database_dsn,
    })
}

fn read_secret(directory: &HeldDirectory, reference: &str) -> Result<Vec<u8>, SecretRefusal> {
    read_bounded(directory, reference, MAX_SECRET_BYTES)
}

/// Resolve one held regular file, bounded by `maximum`. Shared by every
/// exact-file secret this module resolves, including TLS certificate/key
/// material, whose bound differs from the password/session/webhook secrets.
fn read_bounded(
    directory: &HeldDirectory,
    reference: &str,
    maximum: usize,
) -> Result<Vec<u8>, SecretRefusal> {
    platform::recheck_directory(directory).map_err(|_| SecretRefusal::Unavailable)?;
    let file = platform::hold_regular_file_bounded(directory, OsStr::new(reference), maximum)
        .map_err(|_| SecretRefusal::Unavailable)?;
    let bytes = platform::read_exact(&file, maximum).map_err(|_| SecretRefusal::Unavailable)?;
    platform::recheck_regular_file(&file).map_err(|_| SecretRefusal::Unavailable)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(SecretRefusal::Invalid);
    }
    Ok(bytes)
}

fn read_key(
    directory: &HeldDirectory,
    reference: &str,
) -> Result<[u8; HMAC_KEY_BYTES], SecretRefusal> {
    let bytes = read_secret(directory, reference)?;
    if bytes.len() != HMAC_KEY_BYTES {
        return Err(SecretRefusal::Invalid);
    }
    let mut key = [0_u8; HMAC_KEY_BYTES];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// One held TLS certificate/private-key pair, both raw DER. Debug is
/// redacted like [`HeldServiceSecrets`].
pub struct HeldTlsMaterial {
    certificate_der: Vec<u8>,
    private_key_der: Vec<u8>,
}

impl fmt::Debug for HeldTlsMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HeldTlsMaterial")
            .field("certificate_der", &"[REDACTED]")
            .field("private_key_der", &"[REDACTED]")
            .finish()
    }
}

impl HeldTlsMaterial {
    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate_der
    }

    pub fn private_key_der(&self) -> &[u8] {
        &self.private_key_der
    }
}

/// Resolve one operator-held TLS certificate and private key against the
/// held secrets directory. TLS serving is entirely optional and configured
/// only by naming these two references on the command line; naming a
/// reference with nothing held under it refuses before any listener binds,
/// so configuration intent alone can never stand in for held authority.
///
/// Unlike the three password/session/webhook references (named by the
/// checked service-host-adapter request itself, so the decoder already
/// validated their grammar), these two references come straight from the
/// command line, so this validates their shape itself before ever touching
/// the filesystem.
pub fn resolve_tls(
    directory: &HeldDirectory,
    certificate_reference: &str,
    private_key_reference: &str,
) -> Result<HeldTlsMaterial, SecretRefusal> {
    if !valid_reference(certificate_reference) || !valid_reference(private_key_reference) {
        return Err(SecretRefusal::Invalid);
    }
    let certificate_der =
        read_bounded(directory, certificate_reference, MAX_TLS_CERTIFICATE_BYTES)?;
    let private_key_der =
        read_bounded(directory, private_key_reference, MAX_TLS_PRIVATE_KEY_BYTES)?;
    Ok(HeldTlsMaterial {
        certificate_der,
        private_key_der,
    })
}

/// The same closed reference grammar the decoded service host-adapter
/// request enforces on its own secret references: one exact file per
/// reference, lowercase-starting, no path separator admitted.
fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hold_temp() -> (super::super::test_support::TempDir, HeldDirectory) {
        super::super::test_support::TempDir::hold("reference-secrets")
    }

    #[test]
    fn redacted_debug_carries_no_secret_bytes() {
        let secrets = HeldServiceSecrets {
            password_pepper: b"pepper-marker-7f3a00112233".to_vec(),
            session_key: [7_u8; HMAC_KEY_BYTES],
            webhook_key: [9_u8; HMAC_KEY_BYTES],
            database_dsn: b"dsn-marker".to_vec(),
        };
        let rendered = format!("{secrets:?}");
        assert!(!rendered.contains("pepper-marker"));
        assert!(!rendered.contains("dsn-marker"));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn missing_and_missized_secrets_refuse() {
        let (_guard, directory) = hold_temp();
        let requirement = requirement();
        assert!(matches!(
            resolve(&directory, &requirement, "db.primary"),
            Err(SecretRefusal::Unavailable)
        ));
    }

    #[test]
    fn exact_keys_resolve_and_wrong_sizes_refuse() {
        let (_guard, directory) = hold_temp();
        write_secret(&directory, "auth.pepper", &[1_u8; 32]);
        write_secret(&directory, "auth.session", &[2_u8; 31]);
        write_secret(&directory, "webhook.signing", &[3_u8; 32]);
        write_secret(&directory, "db.primary", b"dsn");
        let requirement = requirement();
        assert!(matches!(
            resolve(&directory, &requirement, "db.primary"),
            Err(SecretRefusal::Invalid)
        ));
        std::fs::remove_file(_guard.join("auth.session")).unwrap();
        write_secret(&directory, "auth.session", &[2_u8; 32]);
        let resolved = resolve(&directory, &requirement, "db.primary").unwrap();
        assert_eq!(resolved.password_pepper(), &[1_u8; 32]);
        assert_eq!(resolved.session_key(), &[2_u8; 32]);
        assert_eq!(resolved.webhook_key(), &[3_u8; 32]);
        assert_eq!(format!("{resolved:?}").matches("[REDACTED]").count(), 4);
    }

    fn requirement() -> ServiceSecretResolutionRequirement {
        let bytes = host_request();
        let decoded = semaprax::project::service_host_adapter_request::decode(&bytes).unwrap();
        decoded.secrets().unwrap().clone()
    }

    fn host_request() -> Vec<u8> {
        let text = r#"{"capabilities":["semaprax.service.database.connect.v1","semaprax.service.http.serve-tls.v1","semaprax.service.secrets.resolve.v1","semaprax.service.telemetry.emit.v1"],"database":{"adapter":"sqlite","dsn_secret_ref":"db.primary","migration_table":"semaprax_migrations"},"http":{"adapter":"native","listen_origin":"https://service.example","tls_profile":"modern"},"mode":"host","schema":"semaprax.service-host-adapter-request.v1","secrets":{"password_pepper_ref":"auth.pepper","session_signing_key_ref":"auth.session","webhook_signing_key_ref":"webhook.signing"},"telemetry":{"adapter":"otlp","endpoint_origin":"https://telemetry.example"}}"#;
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(b'\n');
        bytes
    }

    fn write_secret(directory: &HeldDirectory, name: &str, bytes: &[u8]) {
        let _ = platform::write_file_new(directory, OsStr::new(name), bytes, 0o600).unwrap();
    }

    #[test]
    fn tls_material_resolves_and_redacts() {
        let (_guard, directory) = hold_temp();
        write_secret(&directory, "tls.certificate", b"certificate-marker-bytes");
        write_secret(&directory, "tls.private-key", b"private-key-marker-bytes");
        let resolved = resolve_tls(&directory, "tls.certificate", "tls.private-key").unwrap();
        assert_eq!(resolved.certificate_der(), b"certificate-marker-bytes");
        assert_eq!(resolved.private_key_der(), b"private-key-marker-bytes");
        let rendered = format!("{resolved:?}");
        assert!(!rendered.contains("certificate-marker"));
        assert!(!rendered.contains("private-key-marker"));
        assert_eq!(rendered.matches("[REDACTED]").count(), 2);
    }

    #[test]
    fn tls_material_without_held_files_refuses() {
        let (_guard, directory) = hold_temp();
        // Naming a TLS reference the operator never granted a file for must
        // refuse: configuration intent alone never mints authority.
        assert!(matches!(
            resolve_tls(&directory, "tls.certificate", "tls.private-key"),
            Err(SecretRefusal::Unavailable)
        ));
    }

    #[test]
    fn tls_certificate_and_key_bounds_are_enforced() {
        let (_guard, directory) = hold_temp();
        write_secret(
            &directory,
            "tls.certificate",
            &vec![7_u8; MAX_TLS_CERTIFICATE_BYTES + 1],
        );
        write_secret(&directory, "tls.private-key", b"short-key");
        assert!(matches!(
            resolve_tls(&directory, "tls.certificate", "tls.private-key"),
            Err(SecretRefusal::Unavailable)
        ));
    }

    #[test]
    fn tls_reference_grammar_is_validated_before_any_filesystem_access() {
        let (_guard, directory) = hold_temp();
        // No file named "../escape" or "UPPER" could ever exist under the
        // held directory's own grammar; these must refuse as `Invalid`
        // (grammar), never `Unavailable` (filesystem lookup), which proves
        // the check runs before `hold_regular_file_bounded` is reached.
        for bad in ["../escape", "UPPER", "", "has space"] {
            assert!(
                matches!(
                    resolve_tls(&directory, bad, "tls.private-key"),
                    Err(SecretRefusal::Invalid)
                ),
                "certificate reference {bad:?}"
            );
            assert!(
                matches!(
                    resolve_tls(&directory, "tls.certificate", bad),
                    Err(SecretRefusal::Invalid)
                ),
                "private key reference {bad:?}"
            );
        }
    }
}
