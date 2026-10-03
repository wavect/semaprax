//! `semaprax release verify <release-dir>`: issue #168's one documented
//! command over an already-downloaded release directory.
//!
//! This front adds no release-policy decisions of its own. It locates the
//! documents `docs/RELEASE-SIGNING-POLICY-V1.md` names, then hands their
//! exact bytes to `semaprax::release_provenance`, the independent decoder
//! and binding verifier that owns every rule. Everything is re-derived from
//! held, no-follow reads: the manifest digest is recomputed from the
//! manifest's real bytes, each named archive is re-hashed from its real bytes, and a
//! signature claim's subject digest is recomputed from the provenance
//! statement's real bytes. Nothing the documents say about themselves is
//! trusted.
//!
//! The command carries no authority. It opens only the exact paths the
//! manifest names (no directory listing, no traversal), never touches the
//! network, never spawns a process, and never executes a release artifact --
//! `release_provenance`'s own
//! `verification_touches_no_network_and_executes_no_artifact` states that
//! property for the verifier, and this adapter preserves it. It publishes
//! nothing, signs nothing, and creates no key or identity material.
//!
//! **No published SEMAPRAX release is signed today.** For a directory that
//! carries the complete offline Sigstore material, the standalone CLI uses
//! [`SigstoreOfflineVerifier`] to verify signatures, certificate identity and
//! issuer, certificate chain and SCT, transparency-log proofs and promises,
//! and bundle consistency against the exact trusted-root snapshot in that
//! directory. An embedding host may inject another explicit
//! [`OfflineBundleVerificationCapability`]. A directory without the complete
//! material stays on the binding-only `VERIFIED UNSIGNED RELEASE` path.

use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use semaprax::diagnostic::Diagnostic;
use semaprax::release_provenance::{
    parse_manifest, parse_sigstore_trusted_root_jsonl, verify_offline_release_with_capability,
    verify_provenance_binds_manifest, verify_release_binding,
    verify_signature_claim_binds_provenance, verify_signature_claim_consumes_sigstore_bundle,
    OfflineBundleVerificationCapability, OfflineReleaseArchive, SigstoreOfflineVerifier,
};

const USAGE: &str = "release accepts exactly `verify <release-dir>`; see `semaprax help release`";

/// The three document names a published release directory carries, exactly
/// as `docs/RELEASE-SIGNING-POLICY-V1.md` spells them.
pub(crate) const MANIFEST_FILE: &str = "release-manifest.json";
pub(crate) const PROVENANCE_FILE: &str = "release-provenance.json";
pub(crate) const SIGNATURE_CLAIM_FILE: &str = "release-signature-claim.json";
pub(crate) const MESSAGE_BUNDLE_FILE: &str = "release-provenance.bundle";
pub(crate) const TRUSTED_ROOT_FILE: &str = "trusted_root.jsonl";
const ATTESTATION_PREFIX: &str = "release-attestation-";
const ATTESTATION_SUFFIX: &str = ".json";

/// Largest release *document* this front reads. The real documents are a few
/// kilobytes; this bound only keeps a hostile directory from being read into
/// memory before the owning verifier ever sees it. Archive bytes use their
/// separate held-reader bounds below.
const MAX_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;

/// The aggregate verifier needs the exact archive bytes alive through its
/// caller-supplied capability. Bound both each allocation and their combined
/// footprint before opening an archive, rather than letting a hostile
/// manifest turn this CLI adapter into an unbounded in-memory reader.
const MAX_OFFLINE_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_OFFLINE_ARCHIVE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

/// This front's own code, for "the release directory does not present a
/// readable document at all". Every *verification* failure keeps the owning
/// module's code instead (SPX-Z701 shape, SPX-Z702 binding, SPX-Z703
/// identity, SPX-Z704 artifact), because this front decides none of them.
fn document_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z705", message)
}

/// `release verify <dir>` and nothing else. An unknown subcommand, a missing
/// operand, an extra operand, or an option-shaped operand fails closed with
/// the usage line and exit code 2 before any path is opened.
pub(crate) fn parse(args: &[String]) -> Result<PathBuf, u8> {
    let rejected = match args {
        [subcommand, directory]
            if subcommand == "verify" && !directory.is_empty() && !directory.starts_with('-') =>
        {
            return Ok(PathBuf::from(directory));
        }
        [subcommand, ..] if subcommand != "verify" => {
            format!("unknown release subcommand `{subcommand}`; {USAGE}")
        }
        _ => USAGE.to_owned(),
    };
    eprintln!("{rejected}");
    Err(2)
}

/// The explicit doctor release check is separate from offline tool profiles.
/// Complete syntax admission precedes all release-directory reads.
fn parse_doctor_release(args: &[String]) -> Result<(PathBuf, [u8; 32]), u8> {
    match args {
        [command, directory, option, digest]
            if command == "verify-release"
                && !directory.is_empty()
                && !directory.starts_with('-')
                && option == "--trusted-root-sha256"
                && digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
        {
            let mut commitment = [0; 32];
            for (index, byte) in commitment.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&digest[index * 2..index * 2 + 2], 16)
                    .expect("validated lowercase hexadecimal commitment");
            }
            Ok((PathBuf::from(directory), commitment))
        }
        _ => {
            eprintln!(
                "doctor accepts exactly `verify-release <release-dir> --trusted-root-sha256 <64-lowercase-hex>` for release verification"
            );
            Err(2)
        }
    }
}

pub(crate) fn doctor_release_command(
    args: &[String],
    capability: Option<&(dyn OfflineBundleVerificationCapability + Sync)>,
    report: impl FnOnce(&[Diagnostic], bool) -> u8,
) -> Result<(), u8> {
    let (directory, commitment) = parse_doctor_release(args)?;
    let receipt = run_doctor_release(&directory, &commitment, capability)
        .map_err(|error| report(&[error], false))?;
    print!("{receipt}");
    Ok(())
}

#[cfg(unix)]
fn open_regular_no_follow(path: &Path) -> Result<std::fs::File, ()> {
    use rustix::fs::{open, Mode, OFlags};
    open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(|_| ())
}

#[cfg(windows)]
fn open_regular_no_follow(path: &Path) -> Result<std::fs::File, ()> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| ())
}

#[cfg(not(any(unix, windows)))]
fn open_regular_no_follow(_path: &Path) -> Result<std::fs::File, ()> {
    Err(())
}

#[cfg(unix)]
fn regular_identity(file: &std::fs::File, metadata: &std::fs::Metadata) -> Result<(u64, u64), ()> {
    use std::os::unix::fs::MetadataExt as _;
    let _ = file;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn regular_identity(file: &std::fs::File, _metadata: &std::fs::Metadata) -> Result<(u64, u64), ()> {
    let information = winapi_util::file::information(file).map_err(|_| ())?;
    Ok((information.volume_serial_number(), information.file_index()))
}

#[cfg(not(any(unix, windows)))]
fn regular_identity(
    _file: &std::fs::File,
    _metadata: &std::fs::Metadata,
) -> Result<(u64, u64), ()> {
    Err(())
}

fn held_regular_metadata(
    file: &std::fs::File,
    error: &impl Fn(String) -> Diagnostic,
    path: &Path,
) -> Result<std::fs::Metadata, Diagnostic> {
    let metadata = file
        .metadata()
        .map_err(|source| error(format!("cannot inspect held {}: {source}", path.display())))?;
    if !metadata.file_type().is_file() {
        return Err(error(format!(
            "{} is not a held regular file",
            path.display()
        )));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(error(format!("{} is a held reparse point", path.display())));
        }
    }
    Ok(metadata)
}

fn recheck_path_identity(
    path: &Path,
    expected: (u64, u64),
    error: &impl Fn(String) -> Diagnostic,
) -> Result<(), Diagnostic> {
    let path_metadata = std::fs::symlink_metadata(path)
        .map_err(|source| error(format!("cannot re-inspect {}: {source}", path.display())))?;
    if !path_metadata.file_type().is_file() || path_metadata.file_type().is_symlink() {
        return Err(error(format!(
            "{} is not a regular non-link path",
            path.display()
        )));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if path_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(error(format!("{} is a reparse-point path", path.display())));
        }
    }
    let rebound = open_regular_no_follow(path).map_err(|_| {
        error(format!(
            "cannot re-open {} without following links or preserving identity",
            path.display()
        ))
    })?;
    let metadata = held_regular_metadata(&rebound, error, path)?;
    if regular_identity(&rebound, &metadata).map_err(|_| {
        error(format!(
            "held identity is unsupported for {} on this host",
            path.display()
        ))
    })? != expected
    {
        return Err(error(format!(
            "{} changed identity while being read",
            path.display()
        )));
    }
    Ok(())
}

/// Read one no-follow held regular file into a bounded stable snapshot. The
/// held file is read twice without duplicating the bounded buffer; those reads
/// must agree. Both before and after, the pathname is rebound and compared to
/// the held file. A path replacement, reparse point, link, content mutation
/// observed across the two reads, or unsupported identity guarantee fails
/// closed rather than changing the held bytes this adapter authenticates.
fn read_bounded_regular_with_hook(
    path: &Path,
    name: &str,
    limit: u64,
    error: impl Fn(String) -> Diagnostic,
    before_read: impl FnOnce(),
    after_initial_read: impl FnOnce(),
) -> Result<Vec<u8>, Diagnostic> {
    let file = open_regular_no_follow(path).map_err(|_| {
        error(format!(
            "cannot open {} without following links or preserving identity",
            path.display()
        ))
    })?;
    let metadata = held_regular_metadata(&file, &error, path)?;
    if metadata.len() > limit {
        return Err(error(format!(
            "{name} is {} bytes, over the {limit}-byte bound",
            metadata.len()
        )));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| error(format!("{name} does not fit this platform's address space")))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| error(format!("cannot reserve {capacity} bytes to read {name}")))?;
    let identity = regular_identity(&file, &metadata).map_err(|_| {
        error(format!(
            "held identity is unsupported for {} on this host",
            path.display()
        ))
    })?;
    recheck_path_identity(path, identity, &error)?;
    before_read();
    let read_limit = limit
        .checked_add(1)
        .ok_or_else(|| error(format!("{name} read bound is unsupported")))?;
    (&file)
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|source| error(format!("cannot read {}: {source}", path.display())))?;
    if bytes.len() as u64 > limit {
        return Err(error(format!(
            "{name} grew over the {limit}-byte bound while reading"
        )));
    }
    after_initial_read();
    (&file)
        .seek(SeekFrom::Start(0))
        .map_err(|source| error(format!("cannot rewind {}: {source}", path.display())))?;
    let mut reread = (&file).take(read_limit);
    let mut compared = 0usize;
    let mut chunk = [0u8; 8192];
    loop {
        let read = reread
            .read(&mut chunk)
            .map_err(|source| error(format!("cannot re-read {}: {source}", path.display())))?;
        if read == 0 {
            break;
        }
        let end = compared
            .checked_add(read)
            .ok_or_else(|| error(format!("{name} is too large to compare")))?;
        if end > bytes.len() || bytes[compared..end] != chunk[..read] {
            return Err(error(format!("{name} changed while reading")));
        }
        compared = end;
    }
    if compared != bytes.len() {
        return Err(error(format!("{name} changed while reading")));
    }
    let after = held_regular_metadata(&file, &error, path)?;
    if regular_identity(&file, &after).map_err(|_| {
        error(format!(
            "held identity is unsupported for {} on this host",
            path.display()
        ))
    })? != identity
        || after.len() != metadata.len()
    {
        return Err(error(format!("{name} changed while reading")));
    }
    recheck_path_identity(path, identity, &error)?;
    Ok(bytes)
}

fn read_bounded_regular(
    path: &Path,
    name: &str,
    limit: u64,
    error: impl Fn(String) -> Diagnostic,
) -> Result<Vec<u8>, Diagnostic> {
    read_bounded_regular_with_hook(path, name, limit, error, || {}, || {})
}

/// Read one release document from the directory, bounded, failing closed if
/// it is absent, unreadable, not a regular file, or larger than the document
/// bound. The names are fixed by this adapter rather than derived from input.
fn read_document(directory: &Path, name: &str) -> Result<Vec<u8>, Diagnostic> {
    let path = directory.join(name);
    read_bounded_regular(&path, name, MAX_DOCUMENT_BYTES, |message| {
        document_error(format!(
            "release directory {} does not contain a readable {name}: {message}",
            directory.display()
        ))
    })
}

fn attestation_name(platform: &str) -> String {
    format!("{ATTESTATION_PREFIX}{platform}{ATTESTATION_SUFFIX}")
}

/// Manifest artifact names are untrusted JSON. The aggregate reader supports
/// only one file immediately below the caller-supplied release directory;
/// neither a parent traversal nor an absolute path can select another file.
fn require_leaf_name(name: &str) -> Result<(), Diagnostic> {
    if name.is_empty() || Path::new(name).file_name().and_then(|leaf| leaf.to_str()) != Some(name) {
        return Err(Diagnostic::io(
            "SPX-Z704",
            format!("manifest artifact name {name:?} is not one plain file name"),
        ));
    }
    Ok(())
}

fn read_archive(directory: &Path, name: &str, expected_size: u64) -> Result<Vec<u8>, Diagnostic> {
    require_leaf_name(name)?;
    if expected_size > MAX_OFFLINE_ARCHIVE_BYTES {
        return Err(Diagnostic::io(
            "SPX-Z704",
            format!(
                "manifest artifact {name:?} is {expected_size} bytes, over the {MAX_OFFLINE_ARCHIVE_BYTES}-byte offline verification bound"
            ),
        ));
    }
    let path = directory.join(name);
    let bytes = read_bounded_regular(&path, name, expected_size, |message| {
        Diagnostic::io(
            "SPX-Z704",
            format!("cannot read manifest artifact {name:?}: {message}"),
        )
    })?;
    if bytes.len() as u64 != expected_size {
        return Err(Diagnostic::io(
            "SPX-Z704",
            format!(
                "manifest artifact {name:?} is {} bytes from its held read but the manifest records {expected_size}",
                bytes.len()
            ),
        ));
    }
    Ok(bytes)
}

/// Re-hash exactly the files the manifest names, through the held no-follow
/// reader. It deliberately does not enumerate the directory or reject files
/// not named by the manifest; that wider inventory claim belongs to a release
/// package contract, not this read-only verifier.
fn verify_manifest_archives_from_held(
    manifest: &semaprax::release_provenance::ParsedManifest,
    directory: &Path,
) -> Result<(), Diagnostic> {
    let mut total = 0u64;
    for artifact in &manifest.artifacts {
        total = total.checked_add(artifact.size).ok_or_else(|| {
            Diagnostic::io(
                "SPX-Z704",
                "manifest artifact sizes overflow the held-read aggregate bound".to_owned(),
            )
        })?;
        if total > MAX_OFFLINE_ARCHIVE_TOTAL_BYTES {
            return Err(Diagnostic::io(
                "SPX-Z704",
                format!(
                    "manifest artifacts total {total} bytes, over the {MAX_OFFLINE_ARCHIVE_TOTAL_BYTES}-byte held-read aggregate bound"
                ),
            ));
        }
        let bytes = read_archive(directory, &artifact.name, artifact.size)?;
        let digest = format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))
        );
        if digest != artifact.digest {
            return Err(Diagnostic::io(
                "SPX-Z704",
                format!(
                    "manifest artifact {:?} digest disagrees with bytes read from its held file",
                    artifact.name
                ),
            ));
        }
    }
    Ok(())
}

fn material_is_present(directory: &Path) -> Result<bool, Diagnostic> {
    let fixed = [MESSAGE_BUNDLE_FILE, TRUSTED_ROOT_FILE];
    for name in fixed {
        let path = directory.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(document_error(format!(
                    "cannot inspect {}: {error}",
                    path.display()
                )));
            }
        }
    }
    for platform in semaprax::release_provenance::ARCHIVE_PLATFORMS {
        let path = directory.join(attestation_name(platform));
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(document_error(format!(
                    "cannot inspect {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Ok(false)
}

struct OfflineArchive {
    name: String,
    bytes: Vec<u8>,
    attestation: Vec<u8>,
}

struct OfflineRelease {
    manifest: Vec<u8>,
    provenance: Vec<u8>,
    claim: Vec<u8>,
    message_bundle: Vec<u8>,
    trusted_root: Vec<u8>,
    archives: Vec<OfflineArchive>,
}

/// Load the complete closed release inventory for the aggregate verifier.
/// The non-archive binding/root checks run before any archive is allocated,
/// and archive bytes have an explicit combined memory bound.
fn load_offline_release(
    directory: &Path,
    root_commitment: Option<&[u8; 32]>,
) -> Result<OfflineRelease, Diagnostic> {
    // Doctor authenticates root bytes before any other release member is read.
    // None preserves the existing release route's read/diagnostic ordering.
    let committed_root = root_commitment.map(|expected| {
        let root = read_document(directory, TRUSTED_ROOT_FILE)?;
        let actual: [u8; 32] = Sha256::digest(&root).into();
        if &actual != expected {
            return Err(Diagnostic::io("SPX-Z707",
                "held trusted-root bytes disagree with the independently supplied SHA-256 commitment"));
        }
        Ok(root)
    }).transpose()?;
    let manifest = read_document(directory, MANIFEST_FILE)?;
    let provenance = read_document(directory, PROVENANCE_FILE)?;
    let claim = read_document(directory, SIGNATURE_CLAIM_FILE)?;
    let message_bundle = read_document(directory, MESSAGE_BUNDLE_FILE)?;
    let trusted_root = match committed_root {
        Some(root) => root,
        None => read_document(directory, TRUSTED_ROOT_FILE)?,
    };

    // These cheap checks are deliberately repeated by the aggregate API. They
    // prevent an invalid document/root/bundle from causing archive allocation
    // and retain the aggregate API as the sole gate before capability use.
    verify_release_binding(&manifest, &provenance, &claim)?;
    verify_signature_claim_consumes_sigstore_bundle(&claim, &provenance, &message_bundle)?;
    parse_sigstore_trusted_root_jsonl(&trusted_root)?;
    let parsed_manifest = parse_manifest(&manifest)?;

    let mut total = 0u64;
    let mut archives = Vec::with_capacity(parsed_manifest.artifacts.len());
    for artifact in &parsed_manifest.artifacts {
        total = total.checked_add(artifact.size).ok_or_else(|| {
            Diagnostic::io(
                "SPX-Z704",
                "manifest artifact sizes overflow the offline verification bound".to_owned(),
            )
        })?;
        if total > MAX_OFFLINE_ARCHIVE_TOTAL_BYTES {
            return Err(Diagnostic::io(
                "SPX-Z704",
                format!(
                    "manifest artifacts total {total} bytes, over the {MAX_OFFLINE_ARCHIVE_TOTAL_BYTES}-byte offline verification bound"
                ),
            ));
        }
        let name = artifact.name.clone();
        let bytes = read_archive(directory, &name, artifact.size)?;
        let attestation_name = attestation_name(&artifact.platform);
        let attestation = read_document(directory, &attestation_name)?;
        archives.push(OfflineArchive {
            name,
            bytes,
            attestation,
        });
    }
    Ok(OfflineRelease {
        manifest,
        provenance,
        claim,
        message_bundle,
        trusted_root,
        archives,
    })
}

#[derive(Clone, Copy)]
enum OfflineVerifierKind {
    BuiltInSigstore,
    CallerSupplied,
}

/// Verify signed offline material through the selected pure capability. This
/// adapter never opens a network connection or launches `cosign`; the
/// capability receives only bounded bytes after the aggregate structural
/// gates have succeeded.
fn run_with_offline_capability_kind(
    directory: &Path,
    capability: &dyn OfflineBundleVerificationCapability,
    kind: OfflineVerifierKind,
    root_commitment: Option<&[u8; 32]>,
) -> Result<String, Diagnostic> {
    let release = load_offline_release(directory, root_commitment)?;
    let OfflineRelease {
        manifest,
        provenance,
        claim,
        message_bundle,
        trusted_root,
        archives,
    } = release;
    let borrowed_archives = archives
        .iter()
        .map(|archive| OfflineReleaseArchive {
            name: &archive.name,
            bytes: &archive.bytes,
            attestation_bundle_bytes: &archive.attestation,
        })
        .collect::<Vec<_>>();
    verify_offline_release_with_capability(
        &manifest,
        &provenance,
        &claim,
        &message_bundle,
        &trusted_root,
        &borrowed_archives,
        capability,
    )?;
    let parsed_manifest = parse_manifest(&manifest)?;
    let verification = match kind {
        OfflineVerifierKind::BuiltInSigstore => {
            "status: CRYPTOGRAPHICALLY VERIFIED OFFLINE\n\
             Verification: the built-in Sigstore verifier accepted each exact subject and v0.3 bundle,\n\
             its signing certificate and pinned identity/issuer, certificate chain and SCT,\n\
             transparency-log inclusion proof, signed checkpoint and promise, and bundle consistency\n\
             against the exact trusted-root snapshot supplied in this directory.\n\
             Boundary: this is historical verification against supplied root bytes, not a network\n\
             freshness or current-revocation check and not proof that these files were published.\n"
        }
        OfflineVerifierKind::CallerSupplied => {
            "status: OFFLINE RELEASE ACCEPTED BY CALLER-SUPPLIED VERIFICATION CAPABILITY\n\
             This command made no independent cryptographic claim: the embedding host supplied\n\
             the verifier and accepted the exact bounded subjects, bundles, and trusted root.\n"
        }
    };
    Ok(format!(
        "release verify: {directory}\n\
         manifest: {MANIFEST_FILE} (version {version}, tag {tag}, commit {commit})\n\
         offline material: {MESSAGE_BUNDLE_FILE}, {TRUSTED_ROOT_FILE}, and {artifacts} archive attestations\n\
         {verification}\
         Nothing was published, signed, executed, installed, or fetched.\n",
        directory = directory.display(),
        version = parsed_manifest.version,
        tag = parsed_manifest.tag,
        commit = parsed_manifest.commit,
        artifacts = parsed_manifest.artifacts.len(),
    ))
}

pub(crate) fn run_with_offline_capability(
    directory: &Path,
    capability: &dyn OfflineBundleVerificationCapability,
) -> Result<String, Diagnostic> {
    run_with_offline_capability_kind(
        directory,
        capability,
        OfflineVerifierKind::CallerSupplied,
        None,
    )
}

fn run_with_builtin_offline_verifier(directory: &Path) -> Result<String, Diagnostic> {
    run_with_offline_capability_kind(
        directory,
        &SigstoreOfflineVerifier,
        OfflineVerifierKind::BuiltInSigstore,
        None,
    )
}

/// Unlike `release verify`, doctor never falls back to digest-only success.
/// The existing held loader requires every signed-material file before the
/// aggregate verifier runs. Ordinary doctor tool-profile admission is untouched.
pub(crate) fn run_doctor_release(
    directory: &Path,
    root_commitment: &[u8; 32],
    capability: Option<&(dyn OfflineBundleVerificationCapability + Sync)>,
) -> Result<String, Diagnostic> {
    let receipt = match capability {
        Some(capability) => run_with_offline_capability_kind(
            directory,
            capability,
            OfflineVerifierKind::CallerSupplied,
            Some(root_commitment),
        ),
        None => run_with_offline_capability_kind(
            directory,
            &SigstoreOfflineVerifier,
            OfflineVerifierKind::BuiltInSigstore,
            Some(root_commitment),
        ),
    }?;
    Ok(format!(
        "doctor verify-release: offline release check\n\
         trusted-root commitment: sha256:{:x}\n\
         Trust boundary: the operator must authenticate this commitment independently;\n\
         a digest copied from the release directory does not establish trust.\n{receipt}",
        semaprax::digest_hex::LowerHex(root_commitment)
    ))
}

/// What the directory says about signing. A missing claim document is the
/// ordinary case today and is *not* an error; any other read failure is.
fn signature_lines(directory: &Path, provenance_bytes: &[u8]) -> Result<String, Diagnostic> {
    let path = directory.join(SIGNATURE_CLAIM_FILE);
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(format!(
            "signature: absent; this directory carries no {SIGNATURE_CLAIM_FILE}\n"
        )),
        Err(error) => Err(document_error(format!(
            "cannot inspect {}: {error}",
            path.display()
        ))),
        Ok(_) => {
            let claim_bytes = read_document(directory, SIGNATURE_CLAIM_FILE)?;
            verify_signature_claim_binds_provenance(&claim_bytes, provenance_bytes)?;
            Ok(format!(
                "signature: {SIGNATURE_CLAIM_FILE} binds these exact provenance bytes and names \
                 the pinned trusted identity\n\
                 signature: binding only; its signature and certificate bytes were NOT decoded or \
                 cryptographically verified\n"
            ))
        }
    }
}

/// The nonclaims every run prints, signed or not. Kept one constant so the
/// two status paths cannot drift on what was and was not established.
const NONCLAIMS: &str = "\
Verified: integrity (the held bytes read during this verification match the manifest and\n\
provenance statement's recorded values) and provenance binding (version, tag, commit,\n\
builder workflow identity, and the manifest-declared artifact inventory agree).\n\
The release directory was not listed; unrelated files are outside this check.\n\
Not verified and not claimed: authenticity. No cryptographic signature is\n\
checked by this command, this repository holds no signing key or keyless\n\
identity, and no SEMAPRAX release is signed today. Reproducibility,\n\
notarization, and production support are separate claims this command does\n\
not make. Nothing was published, signed, executed, or installed.\n";

/// Verify one release directory and render its deterministic report. Signed
/// material uses an explicit embedding-host verifier when one was supplied,
/// otherwise the standalone built-in Sigstore verifier.
pub(crate) fn run(
    directory: &Path,
    capability: Option<&(dyn OfflineBundleVerificationCapability + Sync)>,
) -> Result<String, Diagnostic> {
    if material_is_present(directory)? {
        return match capability {
            Some(capability) => run_with_offline_capability(directory, capability),
            None => run_with_builtin_offline_verifier(directory),
        };
    }
    let manifest_bytes = read_document(directory, MANIFEST_FILE)?;
    let provenance_bytes = read_document(directory, PROVENANCE_FILE)?;
    let manifest = parse_manifest(&manifest_bytes)?;
    for artifact in &manifest.artifacts {
        require_leaf_name(&artifact.name)?;
    }

    // The owning module decides every rule below; this front only orders the
    // checks and stops at the first failure.
    verify_provenance_binds_manifest(&provenance_bytes, &manifest_bytes)?;
    verify_manifest_archives_from_held(&manifest, directory)?;
    let signature = signature_lines(directory, &provenance_bytes)?;

    let artifacts = manifest.artifacts.len();
    Ok(format!(
        "release verify: {directory}\n\
         manifest: {MANIFEST_FILE} (version {version}, tag {tag}, commit {commit})\n\
         provenance: {PROVENANCE_FILE} binds this manifest byte for byte\n\
         artifacts: {artifacts} of {artifacts} re-hashed from held reads; every size and digest matches \
         the manifest\n\
         {signature}\
         status: VERIFIED UNSIGNED RELEASE\n\
         {NONCLAIMS}",
        directory = directory.display(),
        version = manifest.version,
        tag = manifest.tag,
        commit = manifest.commit,
    ))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use sha2::Sha256;

    use super::*;

    const TAG: &str = "v9.9.9";
    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
    const CLAIM_SIGNATURE: &str = "RklYVFVSRS1TSUdTVE9SRS1TSUdOQVRVUkU=";
    const CLAIM_CERTIFICATE: &str = "RklYVFVSRS1TSUdTVE9SRS1DRVJUSUZJQ0FURQ==";

    fn sha256(bytes: &[u8]) -> String {
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
        )
    }

    fn base64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let first = chunk[0];
            let second = *chunk.get(1).unwrap_or(&0);
            let third = *chunk.get(2).unwrap_or(&0);
            output.push(ALPHABET[(first >> 2) as usize] as char);
            output.push(ALPHABET[((first & 3) << 4 | second >> 4) as usize] as char);
            output.push(if chunk.len() > 1 {
                ALPHABET[((second & 15) << 2 | third >> 6) as usize] as char
            } else {
                '='
            });
            output.push(if chunk.len() > 2 {
                ALPHABET[(third & 63) as usize] as char
            } else {
                '='
            });
        }
        output
    }

    fn verification_material(kind: &str, certificate: &str) -> String {
        format!(
            r#"{{"certificate":{{"rawBytes":"{certificate}"}},"tlogEntries":[{{"logIndex":"1","logId":{{"keyId":"RklYVFVSRS1SRUtPUi1LRVk="}},"kindVersion":{{"kind":"{kind}","version":"0.0.1"}},"integratedTime":"1","inclusionPromise":{{"signedEntryTimestamp":"RklYVFVSRS1TRVQ="}},"inclusionProof":{{"logIndex":"1","rootHash":"RklYVFVSRS1ST09U","treeSize":"1","hashes":["RklYVFVSRS1IQVNI"],"checkpoint":{{"envelope":"fixture checkpoint"}}}},"canonicalizedBody":"RklYVFVSRS1SRUtPUi1CT0RZ"}}],"timestampVerificationData":{{"rfc3161Timestamps":[{{"signedTimestamp":"RklYVFVSRS1SRkMzMTYx"}}]}}}}"#
        )
    }

    fn message_bundle(provenance: &[u8]) -> String {
        let digest = sha256(provenance);
        let raw = digest.strip_prefix("sha256:").unwrap();
        let bytes = (0..raw.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&raw[offset..offset + 2], 16).unwrap())
            .collect::<Vec<_>>();
        format!(
            r#"{{"mediaType":"application/vnd.dev.sigstore.bundle.v0.3+json","verificationMaterial":{},"messageSignature":{{"messageDigest":{{"algorithm":"SHA2_256","digest":"{}"}},"signature":"{CLAIM_SIGNATURE}"}}}}"#,
            verification_material("hashedrekord", CLAIM_CERTIFICATE),
            base64(&bytes),
        )
    }

    fn archive_bundle(name: &str, bytes: &[u8]) -> String {
        let digest = sha256(bytes);
        let predicate = format!(
            r#"{{"buildDefinition":{{"buildType":"https://actions.github.io/buildtypes/workflow/v1","externalParameters":{{"workflow":{{"path":".github/workflows/ci.yml","ref":"refs/tags/{TAG}","repository":"https://github.com/wavect/semaprax"}}}},"internalParameters":{{"github":{{"event_name":"push","repository_id":"1","repository_owner_id":"1","runner_environment":"github-hosted"}}}},"resolvedDependencies":[{{"digest":{{"gitCommit":"{COMMIT}"}},"uri":"git+https://github.com/wavect/semaprax@refs/tags/{TAG}"}}]}},"runDetails":{{"builder":{{"id":"https://github.com/actions/runner/github-hosted"}},"metadata":{{"invocationId":"https://github.com/wavect/semaprax/actions/runs/1/attempts/1"}}}}}}"#
        );
        let statement = format!(
            r#"{{"_type":"https://in-toto.io/Statement/v1","subject":[{{"name":"{name}","digest":{{"sha256":"{}"}}}}],"predicateType":"https://slsa.dev/provenance/v1","predicate":{predicate}}}"#,
            digest.strip_prefix("sha256:").unwrap(),
        );
        format!(
            r#"{{"mediaType":"application/vnd.dev.sigstore.bundle.v0.3+json","verificationMaterial":{},"dsseEnvelope":{{"payload":"{}","payloadType":"application/vnd.in-toto+json","signatures":[{{"sig":"RklYVFVSRS1EU1NFLVNJR05BVFVSRQ=="}}]}}}}"#,
            verification_material("dsse", "RklYVFVSRS1BVFRFU1RBVElPTi1DRVJUSUZJQ0FURQ=="),
            base64(statement.as_bytes()),
        )
    }

    fn signed_directory(label: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-release-offline-{label}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let mut artifacts = Vec::new();
        for platform in semaprax::release_provenance::ARCHIVE_PLATFORMS {
            let extension = if platform.contains("windows") {
                "zip"
            } else {
                "tar.gz"
            };
            let name = format!("semaprax-{TAG}-{platform}.{extension}");
            let bytes = format!("fixture archive for {platform}\n").into_bytes();
            std::fs::write(directory.join(&name), &bytes).unwrap();
            std::fs::write(
                directory.join(attestation_name(platform)),
                archive_bundle(&name, &bytes),
            )
            .unwrap();
            artifacts.push(format!(
                r#"{{"name":"{name}","platform":"{platform}","size":{},"digest":"{}"}}"#,
                bytes.len(),
                sha256(&bytes)
            ));
        }
        let artifacts = artifacts.join(",");
        let manifest = format!(
            r#"{{"schema":"semaprax.release-manifest.v1","version":"9.9.9","tag":"{TAG}","commit":"{COMMIT}","prerelease":true,"required_checks":["alpha"],"changelog_section_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","artifacts":[{artifacts}]}}"#
        );
        let provenance = format!(
            r#"{{"schema":"semaprax.release-provenance.v1","version":"9.9.9","tag":"{TAG}","commit":"{COMMIT}","prerelease":true,"required_checks":["alpha"],"artifacts":[{artifacts}],"manifest_digest":"{}","source":{{"repository":"wavect/semaprax","commit":"{COMMIT}","tag":"{TAG}"}},"builder":{{"workflow_identity":"wavect/semaprax/.github/workflows/ci.yml@refs/tags/{TAG}","run_id":"1","run_attempt":"1"}},"toolchain":{{"rustc_version":"1.88.0","cargo_locked":true}},"build_host_class":"github-hosted-ubuntu-24.04","nonclaims":["unsigned_without_a_paired_signature_claim"]}}"#,
            sha256(manifest.as_bytes())
        );
        let claim = format!(
            r#"{{"schema":"semaprax.release-signature-claim.v1","subject_digest":"{}","subject_name":"release-provenance.json","identity":{{"issuer":"https://token.actions.githubusercontent.com","subject":"repo:wavect@47505194/semaprax@1326961553:ref:refs/tags/{TAG}","workflow_ref":"wavect/semaprax/.github/workflows/ci.yml@refs/tags/{TAG}"}},"algorithm":"sigstore-cosign-bundle-v0.3","signature":"{CLAIM_SIGNATURE}","certificate":"{CLAIM_CERTIFICATE}"}}"#,
            sha256(provenance.as_bytes())
        );
        std::fs::write(directory.join(MANIFEST_FILE), &manifest).unwrap();
        std::fs::write(directory.join(PROVENANCE_FILE), &provenance).unwrap();
        std::fs::write(directory.join(SIGNATURE_CLAIM_FILE), &claim).unwrap();
        std::fs::write(
            directory.join(MESSAGE_BUNDLE_FILE),
            message_bundle(provenance.as_bytes()),
        )
        .unwrap();
        std::fs::write(
            directory.join(TRUSTED_ROOT_FILE),
            b"{\"trustedRoot\":\"fixture\"}\n",
        )
        .unwrap();
        directory
    }

    /// This is a transport probe, not a cryptographic implementation. Its
    /// acceptance proves the CLI passes exact bounded inputs to an explicitly
    /// supplied capability; it must never be described as Sigstore success.
    struct RecordingCapability(Cell<usize>);

    impl OfflineBundleVerificationCapability for RecordingCapability {
        fn verify_offline_bundle(
            &self,
            _identity: &semaprax::release_provenance::ExpectedReleaseIdentity,
            _subject: &[u8],
            _bundle: &[u8],
            _root: &[u8],
        ) -> Result<(), Diagnostic> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parse_admits_only_verify_with_exactly_one_directory() {
        assert_eq!(
            parse(&strings(&["verify", "dist"])).unwrap(),
            PathBuf::from("dist")
        );
        for malformed in [
            &[][..],
            &["verify"][..],
            &["verify", "dist", "extra"][..],
            &["verify", ""][..],
            &["verify", "--json"][..],
            &["sign", "dist"][..],
            &["publish", "dist"][..],
        ] {
            assert_eq!(parse(&strings(malformed)), Err(2), "{malformed:?}");
        }
    }

    /// A missing directory fails closed with this front's own code, never
    /// with a verifier code -- nothing was verified.
    #[test]
    fn a_directory_without_a_manifest_fails_closed_before_any_verification() {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-release-verify-absent-{}",
            std::process::id()
        ));
        let error =
            run(&directory, None).expect_err("an absent release directory must fail closed");
        assert_eq!(error.code, "SPX-Z705");
        assert!(error.message.contains(MANIFEST_FILE), "{}", error.message);
    }

    /// The nonclaims can never read as a signed release.
    #[test]
    fn the_report_template_never_claims_a_signature() {
        assert!(NONCLAIMS.contains("no SEMAPRAX release is signed today"));
        assert!(NONCLAIMS.contains("Not verified and not claimed: authenticity."));
    }

    #[test]
    fn partial_signed_material_refuses_before_builtin_verification_or_unsigned_reporting() {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-release-capability-required-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(MESSAGE_BUNDLE_FILE), b"present").unwrap();
        let error = run(&directory, None).expect_err("partial signed material must fail closed");
        assert_eq!(error.code, "SPX-Z705");
        assert!(error.message.contains(MANIFEST_FILE));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn explicit_capability_receives_complete_bounded_offline_release() {
        let directory = signed_directory("capability");
        let capability = RecordingCapability(Cell::new(0));
        let report = run_with_offline_capability(&directory, &capability)
            .expect("only the explicit transport probe may accept this fixture");
        assert_eq!(capability.0.get(), 4, "provenance plus three archives");
        assert!(report.contains("CALLER-SUPPLIED VERIFICATION CAPABILITY"));
        assert!(report.contains("no independent cryptographic claim"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    // This capability observes transport only: it cannot establish signing.
    #[derive(Default)]
    struct DoctorRecordingCapability(std::sync::Mutex<Vec<Vec<u8>>>);

    fn doctor_fixture_commitment() -> [u8; 32] {
        // Independent test expectation, never discovered from the directory.
        Sha256::digest(b"{\"trustedRoot\":\"fixture\"}\n").into()
    }

    impl OfflineBundleVerificationCapability for DoctorRecordingCapability {
        fn verify_offline_bundle(
            &self,
            identity: &semaprax::release_provenance::ExpectedReleaseIdentity,
            subject: &[u8],
            _bundle: &[u8],
            root: &[u8],
        ) -> Result<(), Diagnostic> {
            assert_eq!(identity.tag, TAG);
            assert_eq!(
                identity.issuer,
                "https://token.actions.githubusercontent.com"
            );
            assert_eq!(root, b"{\"trustedRoot\":\"fixture\"}\n");
            self.0.lock().unwrap().push(subject.to_vec());
            Ok(())
        }
    }

    #[test]
    fn doctor_release_valid_transport_and_builtin_crypto_are_distinct() {
        let directory = signed_directory("doctor-valid");
        let held = load_offline_release(&directory, None).unwrap();
        let expected: Vec<_> = std::iter::once(held.provenance)
            .chain(held.archives.into_iter().map(|archive| archive.bytes))
            .collect();
        let capability = DoctorRecordingCapability::default();
        let report =
            run_doctor_release(&directory, &doctor_fixture_commitment(), Some(&capability))
                .unwrap();
        assert_eq!(*capability.0.lock().unwrap(), expected);
        assert!(report.starts_with("doctor verify-release: offline release check\n"));
        assert!(report.contains("CALLER-SUPPLIED VERIFICATION CAPABILITY"));
        assert!(report.contains("no independent cryptographic claim"));
        assert!(!report.contains("CRYPTOGRAPHICALLY VERIFIED OFFLINE"));
        assert!(!report.contains("VERIFIED UNSIGNED RELEASE"));
        // Identical valid framing must reach the real engine, which refuses
        // fabricated certificate/signature material instead of digest fallback.
        assert_eq!(
            run_doctor_release(&directory, &doctor_fixture_commitment(), None)
                .unwrap_err()
                .code,
            "SPX-Z707"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn doctor_release_swapped_stale_tampered_untrusted_refuse_before_capability() {
        for (case, code) in [
            ("swapped", "SPX-Z702"),
            ("stale", "SPX-Z702"),
            ("tampered", "SPX-Z704"),
            ("untrusted", "SPX-Z703"),
        ] {
            let directory = signed_directory(&format!("doctor-{case}"));
            match case {
                "swapped" => {
                    let platforms = semaprax::release_provenance::ARCHIVE_PLATFORMS;
                    let first = directory.join(attestation_name(platforms[0]));
                    let second = directory.join(attestation_name(platforms[1]));
                    let first_bytes = std::fs::read(&first).unwrap();
                    let second_bytes = std::fs::read(&second).unwrap();
                    std::fs::write(first, second_bytes).unwrap();
                    std::fs::write(second, first_bytes).unwrap();
                }
                "stale" => {
                    let path = directory.join(SIGNATURE_CLAIM_FILE);
                    let claim = std::fs::read_to_string(&path).unwrap();
                    let provenance = std::fs::read(directory.join(PROVENANCE_FILE)).unwrap();
                    std::fs::write(
                        path,
                        claim.replace(&sha256(&provenance), &sha256(b"older release")),
                    )
                    .unwrap();
                }
                "tampered" => {
                    let manifest =
                        parse_manifest(&std::fs::read(directory.join(MANIFEST_FILE)).unwrap())
                            .unwrap();
                    let path = directory.join(&manifest.artifacts[0].name);
                    let mut bytes = std::fs::read(&path).unwrap();
                    bytes[0] ^= 1;
                    std::fs::write(path, bytes).unwrap();
                }
                "untrusted" => {
                    let path = directory.join(SIGNATURE_CLAIM_FILE);
                    let claim = std::fs::read_to_string(&path).unwrap();
                    std::fs::write(
                        path,
                        claim.replace(
                            "https://token.actions.githubusercontent.com",
                            "https://untrusted.example",
                        ),
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }
            let capability = DoctorRecordingCapability::default();
            let error =
                run_doctor_release(&directory, &doctor_fixture_commitment(), Some(&capability))
                    .unwrap_err();
            assert_eq!(error.code, code, "{case}: {}", error.message);
            assert!(capability.0.lock().unwrap().is_empty(), "{case}");
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn doctor_release_never_downgrades_missing_signed_material() {
        for name in [SIGNATURE_CLAIM_FILE, MESSAGE_BUNDLE_FILE, TRUSTED_ROOT_FILE] {
            let directory = signed_directory(&format!("doctor-missing-{name}"));
            std::fs::remove_file(directory.join(name)).unwrap();
            let capability = DoctorRecordingCapability::default();
            assert_eq!(
                run_doctor_release(&directory, &doctor_fixture_commitment(), Some(&capability))
                    .unwrap_err()
                    .code,
                "SPX-Z705"
            );
            assert!(capability.0.lock().unwrap().is_empty());
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn malformed_root_refuses_before_capability_or_archive_reading() {
        let directory = signed_directory("bad-root");
        std::fs::write(directory.join(TRUSTED_ROOT_FILE), b"{\"root\":true}").unwrap();
        let capability = RecordingCapability(Cell::new(0));
        let error = run_with_offline_capability(&directory, &capability)
            .expect_err("unframed trusted root must fail before authority use");
        assert_eq!(error.code, "SPX-Z701");
        assert_eq!(capability.0.get(), 0);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn doctor_release_valid_uncommitted_root_refuses_before_other_reads() {
        // Calibrate the substituted root with a genuine external-signer bundle:
        // it is functional trust material, not merely syntactically valid JSON.
        // This signer is NOT the pinned SEMAPRAX release identity.
        let root = include_str!("../../tests/fixtures/release_sigstore/public-good.json");
        let root = format!(
            "{}\n",
            serde_json::to_string(&serde_json::from_str::<serde_json::Value>(root).unwrap())
                .unwrap()
        );
        let trusted = sigstore_verify::trust_root::TrustedRoot::from_json(&root).unwrap();
        let bundle = sigstore_verify::types::Bundle::from_json(include_str!(
            "../../tests/fixtures/release_sigstore/cosign-v3-blob.sigstore.json"
        ))
        .unwrap();
        sigstore_verify::Verifier::new(&trusted)
            .expect("fixture trusted root must construct a verifier")
            .verify(
                include_bytes!("../../tests/fixtures/release_sigstore/cosign-v3-blob.txt"),
                &bundle,
                &sigstore_verify::VerificationPolicy::new(
                    "w.vollprecht@gmail.com",
                    "https://github.com/login/oauth",
                ),
            )
            .expect("substituted root must be a working cryptographic trust root");
        let directory = signed_directory("doctor-valid-root-substitution");
        std::fs::write(directory.join(TRUSTED_ROOT_FILE), &root).unwrap();
        // Root mismatch must win before attempting any other release read.
        std::fs::remove_file(directory.join(MANIFEST_FILE)).unwrap();
        let capability = DoctorRecordingCapability::default();
        for verifier in [
            Some(&capability as &(dyn OfflineBundleVerificationCapability + Sync)),
            None,
        ] {
            let error =
                run_doctor_release(&directory, &doctor_fixture_commitment(), verifier).unwrap_err();
            assert_eq!(error.code, "SPX-Z707");
            assert!(error
                .message
                .contains("independently supplied SHA-256 commitment"));
        }
        assert!(capability.0.lock().unwrap().is_empty());
        // Matching the bytes passes only this binding step, not verification.
        let matching: [u8; 32] = Sha256::digest(root.as_bytes()).into();
        let error = run_doctor_release(&directory, &matching, None).unwrap_err();
        assert_eq!(error.code, "SPX-Z705");
        assert!(error.message.contains(MANIFEST_FILE));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn aggregate_adapter_rejects_path_traversal_before_opening_an_archive() {
        for name in ["../outside", "/outside", "nested/archive", ""] {
            let error = require_leaf_name(name).expect_err("not a one-file release inventory");
            assert_eq!(error.code, "SPX-Z704");
        }
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reader_refuses_a_symlink_without_reading_its_target() {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-release-no-follow-symlink-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let target = directory.join("target");
        let link = directory.join("document");
        std::fs::write(&target, b"outside bytes").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let error = read_bounded_regular(&link, "document", 1024, document_error)
            .expect_err("a symlink must not become a release input");
        assert_eq!(error.code, "SPX-Z705");
        assert!(error.message.contains("without following links"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reader_refuses_a_path_replaced_after_the_held_identity_check() {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-release-held-identity-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("document");
        let replacement = directory.join("replacement");
        std::fs::write(&path, b"held original").unwrap();
        std::fs::write(&replacement, b"replacement").unwrap();
        let path_for_hook = path.clone();
        let replacement_for_hook = replacement.clone();
        let error = read_bounded_regular_with_hook(
            &path,
            "document",
            1024,
            document_error,
            move || {
                std::fs::rename(&replacement_for_hook, &path_for_hook).unwrap();
            },
            || {},
        )
        .expect_err("a path rebound away from the held file must fail closed");
        assert_eq!(error.code, "SPX-Z705");
        assert!(error.message.contains("changed identity"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reader_refuses_same_inode_same_length_mutation_after_first_read() {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-release-held-content-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("document");
        let original = b"held bytes";
        let replacement = b"mutatebyte";
        assert_eq!(original.len(), replacement.len());
        std::fs::write(&path, original).unwrap();
        let path_for_hook = path.clone();
        let error = read_bounded_regular_with_hook(
            &path,
            "document",
            1024,
            document_error,
            || {},
            move || std::fs::write(&path_for_hook, replacement).unwrap(),
        )
        .expect_err("a same-inode same-length mutation must fail closed");
        assert_eq!(error.code, "SPX-Z705");
        assert!(error.message.contains("changed while reading"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
