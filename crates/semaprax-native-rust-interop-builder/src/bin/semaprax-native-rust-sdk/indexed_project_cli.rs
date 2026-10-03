//! Explicit, file-backed CLI admission for a selected indexed Project SDK.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use semaprax_native_rust_interop::indexed_binding::SelectedPackage;
use semaprax_native_rust_interop::{
    build_indexed_project_native_rust_sdk, IndexedProjectScalarSelection, IndexedScalarSelection,
};
use semaprax_rust_api_index::{RustApiIndex, MAX_INDEX_BYTES};
use serde_json::Value;

const SCHEMA: &str = "semaprax.indexed-project-selection.v1";
const RESULT_SCHEMA: &str = "semaprax.indexed-project-native-rust-sdk-result.v1";
const MAX_SELECTION_BYTES: u64 = 65_536;
const MAX_SOURCE_BYTES: u64 = 1_048_576;
const MAX_PACKAGE_BYTES: u64 = 65_536;

struct Command {
    manifest_path: PathBuf,
    selections_path: PathBuf,
    output: PathBuf,
}

struct Selection {
    source_path: String,
    source: String,
    import_id: String,
    index_bytes: Vec<u8>,
    package_source_bytes: Vec<u8>,
    index: RustApiIndex,
}

fn refusal(code: &str, message: &str) -> ExitCode {
    eprintln!("{code}: {message}");
    ExitCode::FAILURE
}

fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Command, ExitCode> {
    let mut manifest_path = None;
    let mut selections_path = None;
    let mut output = None;
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        let slot = if option == OsStr::new("--manifest-path") {
            &mut manifest_path
        } else if option == OsStr::new("--selections") {
            &mut selections_path
        } else if option == OsStr::new("--output") {
            &mut output
        } else {
            return Err(refusal("SPX-B112", "unknown indexed Project SDK option"));
        };
        if slot.is_some() {
            return Err(refusal(
                "SPX-B112",
                "indexed Project SDK option is repeated",
            ));
        }
        let value = arguments
            .next()
            .ok_or_else(|| refusal("SPX-B112", "indexed Project SDK option requires a value"))?;
        if value.is_empty() || value.to_str().is_some_and(|text| text.starts_with('-')) {
            return Err(refusal(
                "SPX-B112",
                "indexed Project SDK option requires a value",
            ));
        }
        *slot = Some(PathBuf::from(value));
    }
    let (Some(manifest_path), Some(selections_path), Some(output)) =
        (manifest_path, selections_path, output)
    else {
        return Err(refusal(
            "SPX-B112",
            "expected `indexed-project --manifest-path <path> --selections <file> --output <fresh-absolute-path>`",
        ));
    };
    if !output.is_absolute() {
        return Err(refusal(
            "SPX-B112",
            "indexed Project SDK output must be absolute",
        ));
    }
    Ok(Command {
        manifest_path,
        selections_path,
        output,
    })
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, ExitCode> {
    let file = std::fs::File::open(path)
        .map_err(|_| refusal("SPX-B112", "indexed Project SDK input cannot be read"))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| refusal("SPX-B112", "indexed Project SDK input cannot be read"))?;
    if bytes.len() as u64 > limit {
        return Err(refusal(
            "SPX-B112",
            "indexed Project SDK input exceeds its bound",
        ));
    }
    Ok(bytes)
}

fn field<'a>(object: &'a serde_json::Map<String, Value>, key: &str) -> Result<&'a str, ExitCode> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .ok_or_else(|| refusal("SPX-B112", "indexed Project selection field is invalid"))
}

fn selections(manifest_path: &Path, selections_path: &Path) -> Result<Vec<Selection>, ExitCode> {
    let bytes = read_bounded(selections_path, MAX_SELECTION_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| refusal("SPX-B112", "indexed Project selection JSON is invalid"))?;
    let root = value
        .as_object()
        .filter(|object| {
            object.len() == 2 && object.contains_key("schema") && object.contains_key("selections")
        })
        .ok_or_else(|| refusal("SPX-B112", "indexed Project selection schema is invalid"))?;
    if field(root, "schema")? != SCHEMA {
        return Err(refusal(
            "SPX-B112",
            "indexed Project selection schema is invalid",
        ));
    }
    let rows = root["selections"]
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= 32)
        .ok_or_else(|| refusal("SPX-B112", "indexed Project selection count is invalid"))?;
    let project_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .filter(|object| object.len() == 4)
            .ok_or_else(|| refusal("SPX-B112", "indexed Project selection row is invalid"))?;
        let source_path = field(row, "source_path")?;
        let source_path_value = Path::new(source_path);
        if !source_path_value
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(refusal(
                "SPX-B112",
                "indexed Project source path is invalid",
            ));
        }
        let import_id = field(row, "import_id")?.to_owned();
        let source_bytes = read_bounded(&project_root.join(source_path_value), MAX_SOURCE_BYTES)?;
        let source = String::from_utf8(source_bytes)
            .map_err(|_| refusal("SPX-B112", "indexed Project source is not UTF-8"))?;
        let index_path = Path::new(field(row, "index_path")?);
        let package_source_path = Path::new(field(row, "package_source_path")?);
        if !index_path.is_absolute() || !package_source_path.is_absolute() {
            return Err(refusal(
                "SPX-B112",
                "indexed Project selection artifacts must be absolute paths",
            ));
        }
        let index_bytes = read_bounded(index_path, MAX_INDEX_BYTES as u64)?;
        let index = RustApiIndex::replay(&index_bytes)
            .map_err(|_| refusal("SPX-B148", "prepared Rust API index cannot be replayed"))?;
        let package_source_bytes = read_bounded(package_source_path, MAX_PACKAGE_BYTES)?;
        result.push(Selection {
            source_path: source_path.to_owned(),
            source,
            import_id,
            index_bytes,
            package_source_bytes,
            index,
        });
    }
    Ok(result)
}

pub(super) fn run(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let command = match parse(arguments) {
        Ok(command) => command,
        Err(code) => return code,
    };
    let rows = match selections(&command.manifest_path, &command.selections_path) {
        Ok(rows) => rows,
        Err(code) => return code,
    };
    let selected = rows
        .iter()
        .map(|row| {
            let package = row.index.package();
            IndexedProjectScalarSelection {
                source_path: &row.source_path,
                source: &row.source,
                selection: IndexedScalarSelection {
                    import_id: &row.import_id,
                    index_bytes: &row.index_bytes,
                    package_source_bytes: &row.package_source_bytes,
                    package: SelectedPackage {
                        cargo_alias: package.renamed_from.as_deref().unwrap_or(&package.name),
                        name: &package.name,
                        version: &package.version,
                        source_sha256: &package.source_sha256,
                        target: row.index.target(),
                        feature_digest: row.index.feature_digest(),
                        stable_rustc_version: row.index.stable_rustc_version(),
                    },
                },
            }
        })
        .collect::<Vec<_>>();
    let bundle = match build_indexed_project_native_rust_sdk(
        &command.manifest_path,
        &selected,
        &command.output,
    ) {
        Ok(bundle) => bundle,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!(
                    "{}: indexed Project Native Rust SDK build failed",
                    diagnostic.code
                );
            }
            return ExitCode::FAILURE;
        }
    };
    let mut output = BTreeMap::new();
    output.insert("crate_name", bundle.crate_name());
    output.insert("manifest_digest", bundle.manifest_digest());
    output.insert("project_revision", bundle.project_revision());
    output.insert("schema", RESULT_SCHEMA);
    output.insert("subject_digest", bundle.subject_digest());
    output.insert("target_triple", bundle.target_triple());
    output.insert("workspace_revision", bundle.workspace_revision());
    match serde_json::to_string(&output) {
        Ok(json) => {
            let mut stdout = std::io::stdout().lock();
            if writeln!(stdout, "{json}").is_err() {
                return refusal("SPX-I233", "indexed Project SDK result publication failed");
            }
            ExitCode::SUCCESS
        }
        Err(_) => refusal("SPX-I233", "indexed Project SDK result publication failed"),
    }
}

/// Interpret explicitly captured rustc JSON without invoking Cargo or rustc.
/// The projection is limited to one selected import so a wrapper diagnostic
/// cannot be attributed to the wrong Semaprax source declaration.
pub(super) fn run_diagnostics(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let mut manifest_path = None;
    let mut selections_path = None;
    let mut rustc_json = None;
    let mut generated_file = None;
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        let slot = if option == OsStr::new("--manifest-path") {
            &mut manifest_path
        } else if option == OsStr::new("--selections") {
            &mut selections_path
        } else if option == OsStr::new("--rustc-json") {
            &mut rustc_json
        } else if option == OsStr::new("--generated-file") {
            &mut generated_file
        } else {
            return refusal("SPX-B149", "unknown indexed rustc diagnostics option");
        };
        if slot.is_some() {
            return refusal("SPX-B149", "indexed rustc diagnostics option is repeated");
        }
        let Some(value) = arguments.next() else {
            return refusal(
                "SPX-B149",
                "indexed rustc diagnostics option requires a value",
            );
        };
        if value.is_empty() || value.to_str().is_some_and(|text| text.starts_with('-')) {
            return refusal(
                "SPX-B149",
                "indexed rustc diagnostics option requires a value",
            );
        }
        *slot = Some(PathBuf::from(value));
    }
    let (Some(manifest_path), Some(selections_path), Some(rustc_json), Some(generated_file)) =
        (manifest_path, selections_path, rustc_json, generated_file)
    else {
        return refusal("SPX-B149", "expected `indexed-diagnostics --manifest-path <path> --selections <file> --rustc-json <file> --generated-file <rustc-span-file>`");
    };
    if !manifest_path.is_absolute()
        || !selections_path.is_absolute()
        || !rustc_json.is_absolute()
        || !generated_file.is_absolute()
    {
        return refusal(
            "SPX-B149",
            "indexed rustc diagnostic input paths must be absolute",
        );
    }
    let rows = match selections(&manifest_path, &selections_path) {
        Ok(rows) if rows.len() == 1 => rows,
        Ok(_) => {
            return refusal(
                "SPX-B149",
                "indexed rustc diagnostic mapping requires exactly one selection",
            )
        }
        Err(code) => return code,
    };
    let captured = match std::fs::File::open(&rustc_json) {
        Ok(file) => {
            let mut bytes = Vec::new();
            if file
                .take(
                    semaprax_native_rust_interop::rustc_diagnostics::MAX_RUSTC_JSON_BYTES as u64
                        + 1,
                )
                .read_to_end(&mut bytes)
                .is_err()
                || bytes.len()
                    > semaprax_native_rust_interop::rustc_diagnostics::MAX_RUSTC_JSON_BYTES
            {
                return refusal(
                    "SPX-B149",
                    "captured rustc diagnostics cannot be read within their bound",
                );
            }
            bytes
        }
        Err(_) => return refusal("SPX-B149", "captured rustc diagnostics cannot be read"),
    };
    let row = &rows[0];
    let generated_file = generated_file.to_string_lossy();
    let report = match semaprax_native_rust_interop::rustc_diagnostics::map_captured_wrapper_errors(
        &row.index,
        &row.source_path,
        &row.source,
        &row.import_id,
        &generated_file,
        &captured,
    ) {
        Ok(report) => report,
        Err(error) => return refusal(error.code, &error.message),
    };
    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "{report}").is_err() {
        return refusal("SPX-I233", "indexed rustc diagnostics publication failed");
    }
    ExitCode::SUCCESS
}

/// Explicitly admit a pinned extractor envelope and save its canonical,
/// replayable prepared index. This never invokes rustdoc, rustc, or Cargo.
pub(super) fn run_prepare(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let mut extractor_output = None;
    let mut output = None;
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        let slot = if option == OsStr::new("--extractor-output") {
            &mut extractor_output
        } else if option == OsStr::new("--output") {
            &mut output
        } else {
            return refusal("SPX-B148", "unknown indexed preparation option");
        };
        if slot.is_some() {
            return refusal("SPX-B148", "indexed preparation option is repeated");
        }
        let Some(value) = arguments.next() else {
            return refusal("SPX-B148", "indexed preparation option requires a value");
        };
        if value.is_empty() || value.to_str().is_some_and(|text| text.starts_with('-')) {
            return refusal("SPX-B148", "indexed preparation option requires a value");
        }
        *slot = Some(PathBuf::from(value));
    }
    let (Some(extractor_output), Some(output)) = (extractor_output, output) else {
        return refusal("SPX-B148", "expected `indexed-prepare --extractor-output <absolute-file> --output <fresh-absolute-file>`");
    };
    if !extractor_output.is_absolute() || !output.is_absolute() {
        return refusal("SPX-B148", "indexed preparation paths must be absolute");
    }
    let input = match std::fs::File::open(&extractor_output) {
        Ok(file) => {
            let mut bytes = Vec::new();
            if file
                .take(MAX_INDEX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .is_err()
                || bytes.len() > MAX_INDEX_BYTES
            {
                return refusal(
                    "SPX-B148",
                    "extractor output cannot be read within its bound",
                );
            }
            bytes
        }
        Err(_) => return refusal("SPX-B148", "extractor output cannot be read"),
    };
    let index = match RustApiIndex::admit_extractor_output(&input) {
        Ok(index) => index,
        Err(_) => {
            return refusal(
                "SPX-B148",
                "extractor output is not an admitted prepared Rust API index",
            )
        }
    };
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
    {
        Ok(file) => file,
        Err(_) => {
            return refusal(
                "SPX-I233",
                "prepared index output must be a fresh writable file",
            )
        }
    };
    if file.write_all(index.canonical_json().as_bytes()).is_err() || file.sync_all().is_err() {
        return refusal("SPX-I233", "prepared index output write failed");
    }
    ExitCode::SUCCESS
}
