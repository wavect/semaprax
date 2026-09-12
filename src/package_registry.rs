//! Package Registry Snapshot v1 (issue #195): an authority-free,
//! content-addressed model of a published-package registry, deliberately
//! composed with the existing [`crate::package_resolver_v2`] deterministic
//! resolver rather than reimplementing dependency solving.
//!
//! ## The two halves of issue #195, and why only one lives here
//!
//! Issue #195 asks for both a *signed* registry (publisher authorization,
//! signed immutable uploads, revocation) and a *reproducible resolver*.
//! **Publication -- the signed half -- is `HUMAN_BLOCKED`.** Exactly as
//! [`crate::audit_capsule::SignatureEntry`] and
//! [`crate::release_provenance::ParsedSignatureClaim`] already do for the
//! same reason (issue #168, still open): no signing key, keyless-signing
//! identity, or signature-verification dependency exists in this
//! repository, and generated code and compiler tooling gain no ambient
//! signing authority (`AGENTS.md`). [`RegistrySignature`] therefore carries
//! `algorithm`/`identity`/`signature` as opaque, structurally-checked
//! strings: [`build_snapshot`] bounds their shape and never decodes,
//! verifies, or trusts them. A forged signature naming an approved identity
//! is **not** rejected by this module -- see
//! `signature_is_opaque_and_never_cryptographically_checked` in `tests`.
//! `provenance_digest` is the same kind of opaque, unverified reference.
//!
//! **The reproducible resolver half is fully reachable offline**, and is
//! what this module implements: a deterministic, immutable, content-addressed
//! snapshot of published package coordinates that projects cleanly into the
//! `subjects: Vec<String>` catalog [`crate::package_resolver_v2`] already
//! consumes, so registry data and dependency solving stay two composed
//! layers rather than one reimplemented one.
//!
//! ## What a snapshot is
//!
//! [`build_snapshot`] takes a complete, caller-owned list of
//! [`PublishedEntry`] values (there is no ambient mutable server state: a
//! "publish" is a pure function from the complete prior entry list plus one
//! new entry, exactly as [`crate::package_lock_v3`] and
//! [`crate::package_resolver_v2`] take a complete caller-owned catalog) and
//! renders one canonical, content-addressed
//! `semaprax.package-registry-snapshot.v1` envelope. Every entry binds a
//! package identity, a canonical version, a plain SHA-256 `content_digest`
//! of its embedded Subject-v3 `subject_bytes` (the same digest convention as
//! [`crate::audit_capsule::sha256_digest`] and its `ObjectRef` binding --
//! deliberately reused, not reinvented), an opaque `api_digest`, a license
//! string, an optional opaque `provenance_digest`, an opaque
//! [`RegistrySignature`], and a [`PublicationStatus`].
//!
//! ## Determinism, structurally
//!
//! Entries are keyed and iterated through one `BTreeMap<(String, Version),
//! PublishedEntry>` -- never a `HashMap`/`HashSet` -- so canonical bytes are
//! a pure function of entry *content*, never of call order or hidden
//! iteration order. Nothing in this module reads the clock, an environment
//! variable, or a file; every input arrives as an in-memory value. See
//! `tests::determinism_argument_is_structural_not_just_repeated_runs`, which
//! greps this module's own source for exactly the constructs the docstring
//! above claims are absent, and the reordering test right above it, which
//! proves canonical bytes do not depend on entry-list order.
//!
//! ## Reserved namespace
//!
//! `crate::project::standard_dependencies` is the compiler's existing closed
//! bundled-dependency registry (`std.*`). A real package registry has to
//! relate to that existing answer to "where do packages come from" rather
//! than ignore it, so [`build_snapshot`] refuses to admit any `std` or
//! `std.*` package name into the open registry namespace: that whole prefix
//! is reserved for the compiler-bundled closed registry, which protects
//! against exactly the dependency-confusion/squatting failure mode issue
//! #195 names.
//!
//! ## Revocation
//!
//! [`PublicationStatus::Yanked`] never removes or mutates an entry; it is
//! carried in the same canonical, digest-bound bytes as everything else, so
//! a yank cannot silently break a reproducible build that already resolved
//! the version. [`project_subjects`] then applies one of three closed
//! [`YankPolicy`] choices when deriving a resolver catalog from a snapshot:
//! exclude yanked versions silently, refuse outright if any are present, or
//! include them with an explicit warning diagnostic -- "warning or refusing
//! according to policy", per the issue text, never silent inclusion.
//!
//! ## Nonclaims
//!
//! This module performs no publisher authentication, no cryptographic
//! signature or transparency-log verification, no network access, no
//! filesystem access, no CLI wiring, and no build execution. It does not
//! itself compute `api_digest` or `provenance_digest`; both are caller-owned
//! opaque values it stores and structurally bounds. It does not solve
//! dependency graphs; [`project_subjects`] hands its output to
//! [`crate::package_resolver_v2`] for that.

use std::collections::BTreeMap;

use sha2::{Digest as _, Sha256};

use crate::bounded_output;
use crate::diagnostic::{quote_json, Diagnostic, Severity};
use crate::package_lock_v3;
use crate::package_range::{self, Version};

#[cfg(test)]
mod tests;

pub const SCHEMA: &str = "semaprax.package-registry-snapshot.v1";
pub const MAX_ENTRIES: usize = 256;
pub const MAX_ENTRY_BYTES: usize = 17 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_RENDER_BYTES: usize = 64 * 1024 * 1024;
const MAX_IDENTITY_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 512;
const MAX_REASON_BYTES: usize = 1024;
const DIGEST_DOMAIN: &[u8] = b"semaprax.package-registry-snapshot.v1\0";

/// An opaque, unverified signature claim. See the module docstring: no
/// cryptography runs over these fields anywhere in this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrySignature {
    pub algorithm: String,
    pub identity: String,
    pub signature: String,
}

/// A published version's revocation state. Yanked entries are retained, not
/// removed -- see the module docstring's "Revocation" section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicationStatus {
    Active,
    Yanked { reason: String },
}

/// One caller-supplied candidate registry entry: a single package version's
/// complete public registry facts, bound to its exact Subject-v3 bytes by
/// content digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedEntry {
    pub package: String,
    pub version: String,
    pub content_digest: String,
    pub api_digest: String,
    pub license: String,
    pub provenance_digest: Option<String>,
    pub signature: RegistrySignature,
    pub status: PublicationStatus,
    pub subject_bytes: String,
}

/// One immutable, canonical, content-addressed registry snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrySnapshot {
    entries: BTreeMap<(String, Version), PublishedEntry>,
    envelope: String,
    digest: String,
}

impl RegistrySnapshot {
    #[must_use]
    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    #[must_use]
    pub fn coordinates(&self) -> Vec<(String, String)> {
        self.entries
            .keys()
            .map(|(package, version)| (package.clone(), canonical_version_text(*version)))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSnapshot {
    pub coordinates: Vec<(String, String)>,
    pub digest: String,
}

/// How [`project_subjects`] treats [`PublicationStatus::Yanked`] entries
/// when deriving a resolver catalog. A closed three-way choice: exclude
/// silently, refuse outright, or include with an explicit warning -- never
/// silent inclusion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum YankPolicy {
    ExcludeYanked,
    RefuseIfYanked,
    AllowYankedWithWarning,
}

/// A snapshot projected into the exact `subjects: Vec<String>` shape
/// [`crate::package_resolver_v2::ResolutionInput`] consumes, plus any
/// [`YankPolicy::AllowYankedWithWarning`] warnings.
#[derive(Clone, Debug)]
pub struct ProjectedCatalog {
    pub subjects: Vec<String>,
    pub warnings: Vec<Diagnostic>,
}

/// Builds one canonical registry snapshot from a complete, caller-owned
/// entry list. Pure function of its argument: there is no retained,
/// mutable, cross-call registry state anywhere in this module, so calling
/// this twice with the same entries always yields byte-identical output
/// (see `tests::same_entries_produce_byte_identical_snapshots_every_time`),
/// and calling it with one changed entry always yields a different digest
/// (see `tests::one_changed_byte_changes_the_digest`), never a stale reuse
/// of a prior result.
pub fn build_snapshot(entries: &[PublishedEntry]) -> Result<RegistrySnapshot, Diagnostic> {
    let (result, overflowed) = bounded_output::with_limit(MAX_RENDER_BYTES, || build(entries));
    if overflowed {
        return Err(limit_error(
            "registry snapshot cumulative String budget exceeded",
        ));
    }
    result.map(|built| RegistrySnapshot {
        entries: built.entries,
        envelope: built.envelope,
        digest: built.digest,
    })
}

/// Independently rebuilds a snapshot from `entries` and checks it replays
/// `evidence` byte-for-byte. There is no cache: every call recomputes from
/// the supplied entries, so evidence that does not match the entries it
/// claims to describe -- a substituted digest, a stripped yank, a resurrected
/// signature -- is refused, never silently accepted because a prior call
/// happened to look similar.
pub fn verify_snapshot(
    evidence: &str,
    entries: &[PublishedEntry],
) -> Result<VerifiedSnapshot, Diagnostic> {
    if evidence.len() > MAX_OUTPUT_BYTES {
        return Err(limit_error("registry snapshot evidence exceeds output bound"));
    }
    let (result, overflowed) = bounded_output::with_limit(MAX_RENDER_BYTES, || {
        let rebuilt = build(entries)?;
        if rebuilt.envelope != evidence {
            return Err(replay_error(
                "registry snapshot evidence does not exactly replay the supplied entries",
            ));
        }
        Ok(VerifiedSnapshot {
            coordinates: rebuilt.coordinates(),
            digest: rebuilt.digest.clone(),
        })
    });
    if overflowed {
        return Err(limit_error(
            "registry snapshot cumulative String budget exceeded",
        ));
    }
    result
}

/// Projects a snapshot into the resolver-v2 subject catalog shape, applying
/// `policy` to any yanked entries. Subjects are emitted in the snapshot's
/// canonical `(package, version)` order.
pub fn project_subjects(
    snapshot: &RegistrySnapshot,
    policy: YankPolicy,
) -> Result<ProjectedCatalog, Diagnostic> {
    let mut subjects = Vec::with_capacity(snapshot.entries.len());
    let mut warnings = Vec::new();
    for ((package, version), entry) in &snapshot.entries {
        match (&entry.status, policy) {
            (PublicationStatus::Active, _) => subjects.push(entry.subject_bytes.clone()),
            (PublicationStatus::Yanked { .. }, YankPolicy::ExcludeYanked) => {}
            (PublicationStatus::Yanked { reason }, YankPolicy::RefuseIfYanked) => {
                return Err(yank_refused_error(format!(
                    "`{package}@{}` is yanked ({reason}) and the active policy refuses any yanked package",
                    canonical_version_text(*version)
                )));
            }
            (PublicationStatus::Yanked { reason }, YankPolicy::AllowYankedWithWarning) => {
                subjects.push(entry.subject_bytes.clone());
                warnings.push(Diagnostic {
                    code: "SPX-PKR609",
                    severity: Severity::Warning,
                    message: format!(
                        "including yanked `{package}@{}` ({reason})",
                        canonical_version_text(*version)
                    ),
                    path: None,
                    span: None,
                    help: None,
                });
            }
        }
    }
    Ok(ProjectedCatalog { subjects, warnings })
}

struct BuiltSnapshot {
    entries: BTreeMap<(String, Version), PublishedEntry>,
    envelope: String,
    digest: String,
}

impl BuiltSnapshot {
    fn coordinates(&self) -> Vec<(String, String)> {
        self.entries
            .keys()
            .map(|(package, version)| (package.clone(), canonical_version_text(*version)))
            .collect()
    }
}

fn build(entries: &[PublishedEntry]) -> Result<BuiltSnapshot, Diagnostic> {
    if entries.is_empty() || entries.len() > MAX_ENTRIES {
        return Err(capacity_error(
            "registry snapshot entry count is outside bounds",
        ));
    }
    let mut work = 0usize;
    let mut total_bytes = 0usize;
    let mut ordered: BTreeMap<(String, Version), PublishedEntry> = BTreeMap::new();
    for entry in entries {
        let version = validate_and_bind(entry, &mut work)?;
        total_bytes = entry
            .subject_bytes
            .len()
            .checked_add(total_bytes)
            .filter(|total| *total <= MAX_TOTAL_BYTES)
            .ok_or_else(|| {
                capacity_error("registry snapshot total subject bytes exceed bound")
            })?;
        let key = (entry.package.clone(), version);
        if let Some(existing) = ordered.get(&key) {
            return Err(if existing.content_digest == entry.content_digest {
                duplicate_publish_error(format!(
                    "`{}@{}` was already published with the same content_digest; publication is one-time, not idempotent",
                    entry.package, entry.version
                ))
            } else {
                immutable_conflict_error(format!(
                    "`{}@{}` is already published and cannot be replaced with a different content_digest",
                    entry.package, entry.version
                ))
            });
        }
        ordered.insert(key, entry.clone());
    }
    let payload = render_payload(&ordered);
    let envelope = render_wrapper(&payload);
    if envelope.len() > MAX_OUTPUT_BYTES {
        return Err(limit_error("registry snapshot evidence exceeds output bound"));
    }
    let digest = envelope_digest(&payload);
    Ok(BuiltSnapshot {
        entries: ordered,
        envelope,
        digest,
    })
}

fn validate_and_bind(entry: &PublishedEntry, work: &mut usize) -> Result<Version, Diagnostic> {
    validate_identity(&entry.package)?;
    reject_reserved_namespace(&entry.package)?;
    let version = validate_version(&entry.version)?;
    validate_digest_shape(&entry.content_digest, "content_digest")?;
    validate_digest_shape(&entry.api_digest, "api_digest")?;
    if let Some(provenance) = &entry.provenance_digest {
        validate_digest_shape(provenance, "provenance_digest")?;
    }
    validate_bounded_text(&entry.license, MAX_TEXT_BYTES, "license")?;
    validate_bounded_text(&entry.signature.algorithm, MAX_TEXT_BYTES, "signature.algorithm")?;
    validate_bounded_text(&entry.signature.identity, MAX_TEXT_BYTES, "signature.identity")?;
    validate_bounded_text(&entry.signature.signature, MAX_TEXT_BYTES, "signature.signature")?;
    if let PublicationStatus::Yanked { reason } = &entry.status {
        validate_bounded_text(reason, MAX_REASON_BYTES, "status.reason")?;
    }
    if entry.subject_bytes.len() > MAX_ENTRY_BYTES {
        return Err(capacity_error("subject_bytes exceeds the per-entry bound"));
    }
    let computed_digest = crate::audit_capsule::sha256_digest(entry.subject_bytes.as_bytes());
    if computed_digest != entry.content_digest {
        return Err(binding_error(
            "content_digest does not match the plain SHA-256 of subject_bytes",
        ));
    }
    let subject = package_lock_v3::authenticate_subject_for_resolution(&entry.subject_bytes, work)
        .map_err(|_| binding_error("subject_bytes failed Subject-v3/Report-v2 replay"))?;
    if subject.coordinate.package != entry.package || subject.coordinate.version != entry.version {
        return Err(binding_error(
            "embedded Subject-v3 coordinate differs from the declared package/version",
        ));
    }
    Ok(version)
}

fn validate_identity(value: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.len() > MAX_IDENTITY_BYTES {
        return Err(shape_error(
            "package identity length is outside the admitted bound".to_owned(),
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(shape_error(
            "package identity contains a byte outside [a-z0-9._-]".to_owned(),
        ));
    }
    if !value.as_bytes()[0].is_ascii_lowercase() {
        return Err(shape_error(
            "package identity must start with a lowercase letter".to_owned(),
        ));
    }
    if value.split('.').any(str::is_empty) {
        return Err(shape_error(
            "package identity has an empty dot-separated segment".to_owned(),
        ));
    }
    Ok(())
}

fn reject_reserved_namespace(package: &str) -> Result<(), Diagnostic> {
    // The whole `std.*` top-level segment is reserved for the compiler's
    // existing closed bundled-dependency registry, not merely the names it
    // happens to bundle today (`is_bundled` guards the currently-bundled
    // names too, so a future bundled addition is covered even before this
    // prefix rule alone would have to be trusted).
    if package == "std" || package.starts_with("std.") || crate::project::standard_dependencies::is_bundled(package) {
        return Err(reserved_namespace_error(format!(
            "`{package}` is in the `std.*` namespace reserved for the compiler-bundled closed registry (crate::project::standard_dependencies); publish under a different top-level segment"
        )));
    }
    Ok(())
}

fn validate_version(value: &str) -> Result<Version, Diagnostic> {
    let version = package_range::parse_version(value, shape_error)?;
    if canonical_version_text(version) != value {
        return Err(shape_error("version text is not canonical".to_owned()));
    }
    Ok(version)
}

fn canonical_version_text(version: Version) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

fn validate_digest_shape(value: &str, label: &str) -> Result<(), Diagnostic> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(shape_error(format!("{label} must begin with `sha256:`")));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(shape_error(format!(
            "{label} must be `sha256:` followed by 64 lowercase hex digits"
        )));
    }
    Ok(())
}

fn validate_bounded_text(value: &str, max: usize, label: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.len() > max || value.bytes().any(|byte| byte < 0x20 && byte != b'\t')
    {
        return Err(shape_error(format!(
            "{label} is empty, exceeds the admitted length, or contains a control byte"
        )));
    }
    Ok(())
}

fn render_payload(ordered: &BTreeMap<(String, Version), PublishedEntry>) -> String {
    let rendered = ordered
        .iter()
        .map(|((package, version), entry)| render_entry(package, *version, entry))
        .collect::<Vec<_>>();
    let count = rendered.len();
    let joined = bounded_output::budgeted_join(rendered, ",");
    bounded_output::budgeted_format(format_args!(
        "{{\"schema\":{},\"count\":{count},\"entries\":[{joined}]}}",
        quote_json(SCHEMA),
    ))
}

fn render_entry(package: &str, version: Version, entry: &PublishedEntry) -> String {
    let provenance = entry
        .provenance_digest
        .as_deref()
        .map_or_else(|| "null".to_owned(), quote_json);
    let status = render_status(&entry.status);
    bounded_output::budgeted_format(format_args!(
        "{{\"package\":{},\"version\":{},\"content_digest\":{},\"api_digest\":{},\"license\":{},\"provenance_digest\":{provenance},\"signature\":{{\"algorithm\":{},\"identity\":{},\"signature\":{}}},\"status\":{status}}}",
        quote_json(package),
        quote_json(&canonical_version_text(version)),
        quote_json(&entry.content_digest),
        quote_json(&entry.api_digest),
        quote_json(&entry.license),
        quote_json(&entry.signature.algorithm),
        quote_json(&entry.signature.identity),
        quote_json(&entry.signature.signature),
    ))
}

fn render_status(status: &PublicationStatus) -> String {
    match status {
        PublicationStatus::Active => "{\"state\":\"active\"}".to_owned(),
        PublicationStatus::Yanked { reason } => bounded_output::budgeted_format(format_args!(
            "{{\"state\":\"yanked\",\"reason\":{}}}",
            quote_json(reason)
        )),
    }
}

fn envelope_digest(payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    ))
}

fn render_wrapper(payload: &str) -> String {
    bounded_output::budgeted_format(format_args!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{payload}}}",
        quote_json(SCHEMA),
        quote_json(&envelope_digest(payload)),
        payload.len(),
    ))
}

fn shape_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-PKR601", message)
}
fn reserved_namespace_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PKR602", message.into())
}
fn binding_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PKR603", message.into())
}
fn immutable_conflict_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PKR604", message.into())
}
fn duplicate_publish_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PKR605", message.into())
}
fn capacity_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PKR606", message.into())
}
fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PKR607", message.into())
}
fn replay_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PKR608", message.into())
}
fn yank_refused_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PKR610", message.into())
}
