//! `semaprax resolve <manifest> --target <native64|wasm32> --cache <dir>`:
//! deterministically resolve a project's `[dependencies]` against a local
//! content-addressed cache of Semantic Package Subject-v3 envelopes, using the
//! offline resolver. Reading the cache is the explicit effect of this command;
//! it is never an implicit action of `check` or `build`, and no registry is
//! contacted. The cache is caller-populated: a registry, `fetch`, or vendoring
//! step places subjects into it, each file named by its own digest.

use std::path::{Path, PathBuf};

use semaprax::diagnostic::Diagnostic;
use semaprax::package_resolver_v2::{
    self, Requirement, ResolutionInput, ResolutionOptions, MAX_REQUIREMENTS, MAX_SUBJECTS,
    MAX_SUBJECT_BYTES,
};
use semaprax::project::{
    ProjectManifest, MAX_MANIFEST_BYTES, PACKAGE_TARGET_NATIVE64, PACKAGE_TARGET_WASM32,
};

pub(crate) enum ResolveCliError {
    Usage(String),
    Domain(Vec<Diagnostic>),
}

const CODE_CACHE: &str = "SPX-J126";

/// Run `resolve` and return the resolution evidence to print on stdout.
pub(crate) fn run(arguments: &[String]) -> Result<String, ResolveCliError> {
    let parsed = parse(arguments)?;
    let manifest = read_manifest(&parsed.manifest)?;
    let requirements = requirements(&manifest)?;
    let target = admit_target(&manifest, &parsed.target)?;
    let subjects = read_cache(&parsed.cache)?;
    let options = ResolutionOptions::new(parsed.max_bytes)
        .map_err(|error| ResolveCliError::Domain(vec![error]))?;
    let input = ResolutionInput {
        requirements,
        subjects,
        target,
        allowed_capabilities: manifest.capabilities().to_vec(),
    };
    package_resolver_v2::generate(&input, &options).map_err(ResolveCliError::Domain)
}

struct Parsed {
    manifest: PathBuf,
    target: String,
    cache: PathBuf,
    max_bytes: usize,
}

fn parse(arguments: &[String]) -> Result<Parsed, ResolveCliError> {
    let mut manifest = None;
    let mut target = None;
    let mut cache = None;
    let mut max_bytes = ResolutionOptions::default().max_bytes;
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--target" => target = Some(value(arguments, &mut index, "--target")?),
            "--cache" => cache = Some(PathBuf::from(value(arguments, &mut index, "--cache")?)),
            "--max-bytes" => {
                let raw = value(arguments, &mut index, "--max-bytes")?;
                max_bytes = raw
                    .parse::<usize>()
                    .ok()
                    .filter(|parsed| parsed.to_string() == raw)
                    .ok_or_else(|| {
                        usage("resolve option `--max-bytes` requires a canonical decimal")
                    })?;
            }
            option if option.starts_with('-') => {
                return Err(usage(format!("unknown resolve option `{option}`")))
            }
            path if manifest.is_none() => {
                manifest = Some(PathBuf::from(path));
                index += 1;
            }
            _ => return Err(usage("resolve accepts exactly one manifest path")),
        }
    }
    Ok(Parsed {
        manifest: manifest.ok_or_else(|| usage("resolve requires a manifest path"))?,
        target: target.ok_or_else(|| usage("resolve requires `--target native64|wasm32`"))?,
        cache: cache.ok_or_else(|| usage("resolve requires `--cache <dir>`"))?,
        max_bytes,
    })
}

fn value(arguments: &[String], index: &mut usize, option: &str) -> Result<String, ResolveCliError> {
    let next = arguments
        .get(*index + 1)
        .ok_or_else(|| usage(format!("resolve option `{option}` requires a value")))?
        .clone();
    *index += 2;
    Ok(next)
}

fn read_manifest(path: &Path) -> Result<ProjectManifest, ResolveCliError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        ResolveCliError::Domain(vec![Diagnostic::io(
            CODE_CACHE,
            format!("resolve cannot open the manifest: {error}"),
        )])
    })?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES as u64 {
        return Err(ResolveCliError::Domain(vec![Diagnostic::io(
            CODE_CACHE,
            format!("resolve manifest must be a plain file of at most {MAX_MANIFEST_BYTES} bytes"),
        )]));
    }
    let text = std::fs::read_to_string(path).map_err(|error| {
        ResolveCliError::Domain(vec![Diagnostic::io(
            CODE_CACHE,
            format!("resolve cannot read the manifest: {error}"),
        )])
    })?;
    ProjectManifest::parse(&text).map_err(ResolveCliError::Domain)
}

fn requirements(manifest: &ProjectManifest) -> Result<Vec<Requirement>, ResolveCliError> {
    let dependencies = manifest.dependencies();
    if dependencies.is_empty() {
        return Err(ResolveCliError::Domain(vec![Diagnostic::io(
            CODE_CACHE,
            "resolve needs a `[dependencies]` table; this manifest declares none",
        )]));
    }
    if dependencies.len() > MAX_REQUIREMENTS {
        return Err(ResolveCliError::Domain(vec![Diagnostic::io(
            CODE_CACHE,
            format!("resolve admits at most {MAX_REQUIREMENTS} root dependencies"),
        )]));
    }
    Ok(dependencies
        .iter()
        .map(|dependency| Requirement {
            package: dependency.name().to_owned(),
            range: dependency.range().to_owned(),
        })
        .collect())
}

fn admit_target(manifest: &ProjectManifest, target: &str) -> Result<String, ResolveCliError> {
    if target != PACKAGE_TARGET_NATIVE64 && target != PACKAGE_TARGET_WASM32 {
        return Err(usage(format!(
            "resolve `--target` admits only `{PACKAGE_TARGET_NATIVE64}` and `{PACKAGE_TARGET_WASM32}`; found `{target}`"
        )));
    }
    if let Some(matrix) = manifest.target_matrix() {
        if !matrix.iter().any(|declared| declared == target) {
            return Err(ResolveCliError::Domain(vec![Diagnostic::io(
                CODE_CACHE,
                format!("resolve target `{target}` is outside the manifest `[targets] matrix`"),
            )]));
        }
    }
    Ok(target.to_owned())
}

/// Load every Subject-v3 envelope from the content-addressed cache. Each cache
/// file is named `<hex>.json`, and its `digest` field must be `sha256:<hex>`;
/// a mismatch is a content-address integrity failure.
fn read_cache(cache: &Path) -> Result<Vec<String>, ResolveCliError> {
    let metadata = std::fs::symlink_metadata(cache)
        .map_err(|error| domain(format!("resolve cannot open the cache directory: {error}")))?;
    if !metadata.is_dir() {
        return Err(domain("resolve `--cache` must name a directory".to_owned()));
    }
    let mut entries = std::fs::read_dir(cache)
        .map_err(|error| domain(format!("resolve cannot read the cache directory: {error}")))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| domain(format!("resolve cannot enumerate the cache: {error}")))?;
    entries
        .retain(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"));
    entries.sort();
    if entries.len() > MAX_SUBJECTS {
        return Err(domain(format!(
            "resolve cache holds more than {MAX_SUBJECTS} subject files"
        )));
    }
    let mut subjects = Vec::with_capacity(entries.len());
    for path in entries {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| domain("resolve cache file name is not UTF-8".to_owned()))?
            .to_owned();
        let file = std::fs::symlink_metadata(&path)
            .map_err(|error| domain(format!("resolve cannot open a cache subject: {error}")))?;
        if !file.is_file() || file.len() > MAX_SUBJECT_BYTES as u64 {
            return Err(domain(format!(
                "resolve cache subject `{stem}` must be a plain file of at most {MAX_SUBJECT_BYTES} bytes"
            )));
        }
        let bytes = std::fs::read_to_string(&path).map_err(|error| {
            domain(format!(
                "resolve cannot read cache subject `{stem}`: {error}"
            ))
        })?;
        let envelope: serde_json::Value = serde_json::from_str(&bytes).map_err(|_| {
            domain(format!(
                "resolve cache subject `{stem}` is not a JSON object"
            ))
        })?;
        let digest = envelope
            .get("digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| domain(format!("resolve cache subject `{stem}` has no digest")))?;
        let expected = digest.strip_prefix("sha256:").unwrap_or(digest);
        if expected != stem {
            return Err(domain(format!(
                "resolve cache subject `{stem}` is not content-addressed: its digest is `{digest}`"
            )));
        }
        subjects.push(bytes);
    }
    Ok(subjects)
}

fn domain(message: String) -> ResolveCliError {
    ResolveCliError::Domain(vec![Diagnostic::io(CODE_CACHE, message)])
}

fn usage(message: impl Into<String>) -> ResolveCliError {
    ResolveCliError::Usage(format!(
        "{}\nhint: run `semaprax resolve --help` for usage",
        message.into()
    ))
}
