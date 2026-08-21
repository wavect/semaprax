//! Public, build-only packaging for the admitted callable-v2 ownership slice.
//!
//! This module emits and compiles one exact provider, but deliberately exposes
//! no loader, invocation, adoption, capability, raw symbol lookup, or pointer
//! surface. Symbol names remain authenticated build metadata.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::ast::Program;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::graph;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedResourceDropKind, ResolvedType,
    ResolvedTypeDeclarationKind,
};

use super::{emit_native_callable_admission_core, first_backend_diagnostic};

const PREFLIGHT_SCHEMA: &str = "semaprax.native-callable-preflight.v1";
const BUNDLE_SCHEMA: &str = "semaprax.native-callable-bundle.v1";
const ABI_SCHEMA: &str = "semaprax.native-callable.v2";
const PROVIDER_FILE: &str = "provider.c";
const DESCRIPTOR_FILE: &str = "descriptor.bin";
const EVENT_DICTIONARY_FILE: &str = "semantic-event-dictionary.json";
const TRACE_CERTIFICATE_FILE: &str = "trace-path-certificate.json";
const MANIFEST_FILE: &str = "semaprax.native-callable.json";
const MANIFEST_CHECKSUM_FILE: &str = "semaprax.native-callable.sha256";

static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(1);

/// Deterministic, authority-free preflight for one exact callable-v2 function.
///
/// Preflight validates source/HIR, ownership, cleanup, target guards, codecs,
/// dictionary, and trace certificate. It performs no file writes, compilation,
/// loading, invocation, resource adoption, or capability creation.
pub struct NativeCallableBundlePreflight {
    module: String,
    graph_revision: String,
    function_id: String,
    descriptor: Vec<u8>,
    getter_symbol: String,
    callable_symbol: String,
    call_contract: [u8; 32],
    max_request_bytes: u32,
    max_response_bytes: u32,
    provider_source: String,
    event_dictionary: String,
    trace_path_certificate: String,
    preflight_sha256: String,
}

impl NativeCallableBundlePreflight {
    #[must_use]
    pub fn module(&self) -> &str {
        &self.module
    }

    #[must_use]
    pub fn graph_revision(&self) -> &str {
        &self.graph_revision
    }

    #[must_use]
    pub fn function_id(&self) -> &str {
        &self.function_id
    }

    #[must_use]
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    #[must_use]
    pub fn getter_symbol(&self) -> &str {
        &self.getter_symbol
    }

    #[must_use]
    pub fn callable_symbol(&self) -> &str {
        &self.callable_symbol
    }

    #[must_use]
    pub fn call_contract(&self) -> [u8; 32] {
        self.call_contract
    }

    #[must_use]
    pub fn max_request_bytes(&self) -> u32 {
        self.max_request_bytes
    }

    #[must_use]
    pub fn max_response_bytes(&self) -> u32 {
        self.max_response_bytes
    }

    #[must_use]
    pub fn provider_source(&self) -> &str {
        &self.provider_source
    }

    #[must_use]
    pub fn event_dictionary(&self) -> &str {
        &self.event_dictionary
    }

    #[must_use]
    pub fn trace_path_certificate(&self) -> &str {
        &self.trace_path_certificate
    }

    /// SHA-256 of the canonical preflight projection and all nonbinary inputs.
    #[must_use]
    pub fn preflight_sha256(&self) -> &str {
        &self.preflight_sha256
    }
}

/// One successfully committed native-callable bundle.
pub struct NativeCallableBundle {
    output_directory: PathBuf,
    library_path: PathBuf,
    manifest_path: PathBuf,
    manifest_sha256: String,
}

impl NativeCallableBundle {
    #[must_use]
    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    #[must_use]
    pub fn library_path(&self) -> &Path {
        &self.library_path
    }

    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    #[must_use]
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
}

/// Validate and deterministically derive one public build-only callable bundle.
pub fn preflight_native_callable_bundle(
    program: &Program,
    function_id: &str,
) -> Result<NativeCallableBundlePreflight, Diagnostic> {
    if function_id.is_empty() || function_id.contains('\0') {
        return Err(bundle_error(
            "native-callable bundle requires a nonempty NUL-free function stable ID",
        ));
    }
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    if !resolved.function_templates.is_empty() || !resolved.function_instances.is_empty() {
        return Err(bundle_error(
            "native-callable bundles do not admit generic function templates or instances",
        ));
    }
    let function_id = DeclarationId::new(function_id);
    let declaration = resolved
        .declarations
        .declaration(&function_id)
        .ok_or_else(|| bundle_error(format!("function `{function_id}` is not in the program")))?;
    if !declaration.identity_origin.is_persistent() {
        return Err(bundle_error(format!(
            "native-callable function `{}` requires an explicit persistent @id",
            declaration.name
        )));
    }
    let function = resolved
        .functions
        .iter()
        .find(|candidate| candidate.id == function_id)
        .ok_or_else(|| bundle_error(format!("function `{function_id}` is not in the program")))?;
    let has_direct_owned_trivial_resource = function.params.iter().any(|parameter| {
        if parameter.ownership != OwnershipMode::Own {
            return false;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &parameter.ty
        else {
            return false;
        };
        arguments.is_empty()
            && resolved.types.iter().any(|candidate| {
                candidate.id == *declaration
                    && matches!(
                        &candidate.kind,
                        ResolvedTypeDeclarationKind::Resource { drop }
                            if drop.kind == ResolvedResourceDropKind::Trivial
                    )
            })
    });
    if !has_direct_owned_trivial_resource {
        return Err(bundle_error(format!(
            "native-callable function `{function_id}` requires at least one direct `own` trivial-resource parameter"
        )));
    }
    let artifact = emit_native_callable_admission_core(&resolved, &function_id)?;
    let trace_path_certificate = artifact.trace_path_certificate().canonical_json();
    let module = program.module.clone();
    let graph_revision = graph::revision(program);
    let function_id = function_id.as_str().to_owned();
    let descriptor = artifact.descriptor().to_vec();
    let getter_symbol = artifact.getter_symbol().to_owned();
    let callable_symbol = artifact.callable_symbol().to_owned();
    let call_contract = artifact.call_contract();
    let max_request_bytes = artifact.max_request_bytes();
    let max_response_bytes = artifact.max_response_bytes();
    let provider_source = artifact.provider_source().to_owned();
    let event_dictionary = artifact.event_dictionary().to_owned();
    let projection = preflight_projection(
        &module,
        &graph_revision,
        &function_id,
        &descriptor,
        &getter_symbol,
        &callable_symbol,
        &call_contract,
        max_request_bytes,
        max_response_bytes,
        provider_source.as_bytes(),
        event_dictionary.as_bytes(),
        trace_path_certificate.as_bytes(),
    );
    let preflight_sha256 = digest_hex(projection.as_bytes());
    Ok(NativeCallableBundlePreflight {
        module,
        graph_revision,
        function_id,
        descriptor,
        getter_symbol,
        callable_symbol,
        call_contract,
        max_request_bytes,
        max_response_bytes,
        provider_source,
        event_dictionary,
        trace_path_certificate,
        preflight_sha256,
    })
}

/// Build one host-platform shared-library bundle without loading or executing it.
///
/// `output` must not already exist, including as a dangling symlink. All files
/// are first materialized in a private sibling staging directory and the
/// completed directory is renamed into place only after strict compilation and
/// manifest hashing succeed. The canonical parent directory is a trusted local
/// build location and must not be modified concurrently: portable `std` has no
/// atomic directory rename-without-replacement primitive, so this API does not
/// claim adversarial race-safe no-clobber after its final absence check.
pub fn build_native_callable_bundle(
    program: &Program,
    function_id: &str,
    output: &Path,
) -> Result<NativeCallableBundle, Diagnostic> {
    let preflight = preflight_native_callable_bundle(program, function_id)?;
    build_preflight(preflight, output)
}

fn build_preflight(
    preflight: NativeCallableBundlePreflight,
    output: &Path,
) -> Result<NativeCallableBundle, Diagnostic> {
    let library_name = host_library_name()?;
    let output_name = output.file_name().ok_or_else(|| {
        bundle_io_error(format!(
            "native-callable output `{}` must name a new directory",
            output.display()
        ))
    })?;
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|error| {
        bundle_io_error(format!(
            "cannot canonicalize native-callable output parent {}: {error}",
            parent.display()
        ))
    })?;
    let final_output = parent.join(output_name);
    require_absent_output(&final_output)?;

    let staging_id = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
    let staging_path = parent.join(format!(
        ".semaprax-native-callable-staging-{}-{staging_id}",
        std::process::id()
    ));
    fs::create_dir(&staging_path).map_err(|error| {
        bundle_io_error(format!(
            "cannot create native-callable staging directory {}: {error}",
            staging_path.display()
        ))
    })?;
    let mut staging = StagingDirectory::new(staging_path);

    write_new(
        &staging.path.join(PROVIDER_FILE),
        preflight.provider_source.as_bytes(),
    )?;
    write_new(&staging.path.join(DESCRIPTOR_FILE), &preflight.descriptor)?;
    write_new(
        &staging.path.join(EVENT_DICTIONARY_FILE),
        preflight.event_dictionary.as_bytes(),
    )?;
    write_new(
        &staging.path.join(TRACE_CERTIFICATE_FILE),
        preflight.trace_path_certificate.as_bytes(),
    )?;
    compile_provider(&staging.path, library_name)?;
    remove_windows_export_artifact(&staging.path)?;
    require_exact_inventory(
        &staging.path,
        &[
            PROVIDER_FILE,
            DESCRIPTOR_FILE,
            EVENT_DICTIONARY_FILE,
            TRACE_CERTIFICATE_FILE,
            library_name,
        ],
        "compiler output",
    )?;

    let library_path = staging.path.join(library_name);
    let library_metadata = fs::symlink_metadata(&library_path).map_err(|error| {
        bundle_io_error(format!(
            "cannot inspect compiled native-callable library: {error}"
        ))
    })?;
    if library_metadata.file_type().is_symlink() || !library_metadata.is_file() {
        return Err(bundle_io_error(
            "native-callable compiler output must be one regular file",
        ));
    }

    let mut files = vec![
        bundle_file(&staging.path, DESCRIPTOR_FILE)?,
        bundle_file(&staging.path, EVENT_DICTIONARY_FILE)?,
        bundle_file(&staging.path, PROVIDER_FILE)?,
        bundle_file(&staging.path, TRACE_CERTIFICATE_FILE)?,
        bundle_file(&staging.path, library_name)?,
    ];
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = bundle_manifest(&preflight, library_name, &files);
    let manifest_sha256 = digest_hex(manifest.as_bytes());
    write_new(&staging.path.join(MANIFEST_FILE), manifest.as_bytes())?;
    let checksum = format!("{manifest_sha256}  {MANIFEST_FILE}\n");
    write_new(
        &staging.path.join(MANIFEST_CHECKSUM_FILE),
        checksum.as_bytes(),
    )?;
    require_exact_inventory(
        &staging.path,
        &[
            PROVIDER_FILE,
            DESCRIPTOR_FILE,
            EVENT_DICTIONARY_FILE,
            TRACE_CERTIFICATE_FILE,
            library_name,
            MANIFEST_FILE,
            MANIFEST_CHECKSUM_FILE,
        ],
        "completed bundle",
    )?;

    // Recheck immediately before commit. This prevents ordinary overwrite and
    // dangling-leaf symlink substitution; the containing canonical directory
    // keeps staging and the final rename on one filesystem.
    require_absent_output(&final_output)?;
    fs::rename(&staging.path, &final_output).map_err(|error| {
        bundle_io_error(format!(
            "cannot commit native-callable bundle to {}: {error}",
            final_output.display()
        ))
    })?;
    staging.committed = true;

    Ok(NativeCallableBundle {
        library_path: final_output.join(library_name),
        manifest_path: final_output.join(MANIFEST_FILE),
        output_directory: final_output,
        manifest_sha256,
    })
}

struct StagingDirectory {
    path: PathBuf,
    committed: bool,
}

impl StagingDirectory {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

struct BundleFile {
    path: String,
    bytes: usize,
    sha256: String,
}

fn bundle_file(directory: &Path, name: &str) -> Result<BundleFile, Diagnostic> {
    let bytes = fs::read(directory.join(name)).map_err(|error| {
        bundle_io_error(format!(
            "cannot read staged native-callable file `{name}`: {error}"
        ))
    })?;
    Ok(BundleFile {
        path: name.to_owned(),
        bytes: bytes.len(),
        sha256: digest_hex(&bytes),
    })
}

fn bundle_manifest(
    preflight: &NativeCallableBundlePreflight,
    library_name: &str,
    files: &[BundleFile],
) -> String {
    let files = files
        .iter()
        .map(|file| {
            format!(
                "{{\"path\":{},\"bytes\":{},\"sha256\":{}}}",
                quote_json(&file.path),
                file.bytes,
                quote_json(&file.sha256)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"abi\":{},\"module\":{},\"graph_revision\":{},\"function_id\":{},\"getter_symbol\":{},\"callable_symbol\":{},\"call_contract\":{},\"max_request_bytes\":{},\"max_response_bytes\":{},\"preflight_sha256\":{},\"library\":{},\"files\":[{}]}}\n",
        quote_json(BUNDLE_SCHEMA),
        quote_json(ABI_SCHEMA),
        quote_json(&preflight.module),
        quote_json(&preflight.graph_revision),
        quote_json(&preflight.function_id),
        quote_json(&preflight.getter_symbol),
        quote_json(&preflight.callable_symbol),
        quote_json(&hex(&preflight.call_contract)),
        preflight.max_request_bytes,
        preflight.max_response_bytes,
        quote_json(&preflight.preflight_sha256),
        quote_json(library_name),
        files,
    )
}

#[allow(clippy::too_many_arguments)]
fn preflight_projection(
    module: &str,
    graph_revision: &str,
    function_id: &str,
    descriptor: &[u8],
    getter_symbol: &str,
    callable_symbol: &str,
    call_contract: &[u8; 32],
    max_request_bytes: u32,
    max_response_bytes: u32,
    provider: &[u8],
    dictionary: &[u8],
    certificate: &[u8],
) -> String {
    format!(
        "{{\"schema\":{},\"abi\":{},\"module\":{},\"graph_revision\":{},\"function_id\":{},\"getter_symbol\":{},\"callable_symbol\":{},\"call_contract\":{},\"max_request_bytes\":{},\"max_response_bytes\":{},\"inputs\":[{{\"path\":{},\"bytes\":{},\"sha256\":{}}},{{\"path\":{},\"bytes\":{},\"sha256\":{}}},{{\"path\":{},\"bytes\":{},\"sha256\":{}}},{{\"path\":{},\"bytes\":{},\"sha256\":{}}}]}}",
        quote_json(PREFLIGHT_SCHEMA),
        quote_json(ABI_SCHEMA),
        quote_json(module),
        quote_json(graph_revision),
        quote_json(function_id),
        quote_json(getter_symbol),
        quote_json(callable_symbol),
        quote_json(&hex(call_contract)),
        max_request_bytes,
        max_response_bytes,
        quote_json(DESCRIPTOR_FILE),
        descriptor.len(),
        quote_json(&digest_hex(descriptor)),
        quote_json(EVENT_DICTIONARY_FILE),
        dictionary.len(),
        quote_json(&digest_hex(dictionary)),
        quote_json(PROVIDER_FILE),
        provider.len(),
        quote_json(&digest_hex(provider)),
        quote_json(TRACE_CERTIFICATE_FILE),
        certificate.len(),
        quote_json(&digest_hex(certificate)),
    )
}

fn compile_provider(directory: &Path, library_name: &str) -> Result<(), Diagnostic> {
    let mut compiler = Command::new("clang");
    compiler.current_dir(directory);
    if cfg!(target_os = "macos") {
        compiler.args([
            "-dynamiclib",
            "-fPIC",
            "-fvisibility=hidden",
            "-Wl,-no_uuid",
        ]);
    } else if cfg!(target_os = "linux") {
        compiler.args([
            "-shared",
            "-fPIC",
            "-fvisibility=hidden",
            "-Wl,--build-id=sha1",
        ]);
    } else if cfg!(target_os = "windows") {
        compiler.args(["-shared", "-Wl,/Brepro", "-Wl,/NOIMPLIB"]);
    } else {
        return Err(bundle_error(
            "native-callable bundles support only host Linux, macOS, and Windows",
        ));
    }
    let output = compiler
        .args(["-O2", "-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(PROVIDER_FILE)
        .arg("-o")
        .arg(library_name)
        .output()
        .map_err(|error| {
            bundle_error(format!(
                "failed to start clang for native-callable bundle: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(bundle_error(format!(
            "native-callable shared-library compilation failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn require_exact_inventory(
    directory: &Path,
    expected: &[&str],
    stage: &str,
) -> Result<(), Diagnostic> {
    let mut observed = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| {
        bundle_io_error(format!(
            "cannot inspect native-callable {stage} directory: {error}"
        ))
    })? {
        let entry = entry.map_err(|error| {
            bundle_io_error(format!(
                "cannot inspect native-callable {stage} entry: {error}"
            ))
        })?;
        let name = entry.file_name().into_string().map_err(|_| {
            bundle_io_error(format!(
                "native-callable {stage} contains a non-Unicode file name"
            ))
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
            bundle_io_error(format!(
                "cannot inspect native-callable {stage} entry `{name}`: {error}"
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(bundle_io_error(format!(
                "native-callable {stage} entry `{name}` must be one regular file"
            )));
        }
        observed.push(name);
    }
    observed.sort();
    let mut expected = expected
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    if observed != expected {
        return Err(bundle_io_error(format!(
            "native-callable {stage} inventory mismatch: expected {expected:?}, observed {observed:?}"
        )));
    }
    Ok(())
}

fn remove_windows_export_artifact(directory: &Path) -> Result<(), Diagnostic> {
    if !cfg!(target_os = "windows") {
        return Ok(());
    }
    let export_path = directory.join("semaprax-native-callable.exp");
    match fs::symlink_metadata(&export_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(bundle_io_error(
                "native-callable Windows export side artifact must be one regular file",
            ))
        }
        Ok(_) => fs::remove_file(&export_path).map_err(|error| {
            bundle_io_error(format!(
                "cannot remove native-callable Windows export side artifact: {error}"
            ))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(bundle_io_error(format!(
            "cannot inspect native-callable Windows export side artifact: {error}"
        ))),
    }
}

fn host_library_name() -> Result<&'static str, Diagnostic> {
    if cfg!(target_os = "macos") {
        Ok("libsemaprax-native-callable.dylib")
    } else if cfg!(target_os = "linux") {
        Ok("libsemaprax-native-callable.so")
    } else if cfg!(target_os = "windows") {
        Ok("semaprax-native-callable.dll")
    } else {
        Err(bundle_error(
            "native-callable bundles support only host Linux, macOS, and Windows",
        ))
    }
}

fn require_absent_output(path: &Path) -> Result<(), Diagnostic> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let kind = if metadata.file_type().is_symlink() {
                "symlink"
            } else if metadata.is_dir() {
                "directory"
            } else {
                "file"
            };
            Err(bundle_io_error(format!(
                "native-callable output {} already exists as a {kind}; refusing to overwrite",
                path.display()
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(bundle_io_error(format!(
            "cannot inspect native-callable output {}: {error}",
            path.display()
        ))),
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Diagnostic> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            bundle_io_error(format!(
                "cannot create staged native-callable file {}: {error}",
                path.display()
            ))
        })?;
    file.write_all(bytes).map_err(|error| {
        bundle_io_error(format!(
            "cannot write staged native-callable file {}: {error}",
            path.display()
        ))
    })
}

fn digest_hex(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

fn bundle_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B105", message)
}

fn bundle_io_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-I107", message)
}
