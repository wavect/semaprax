//! Private Windows materialization of an already authenticated compiler plan.
//! Namespace observations require controlled parent ancestry; this is not a
//! same-principal sandbox, crash recovery, or post-publication rollback.
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use same_file::Handle;
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{ProjectNpmPublication, MAX_PROJECT_NPM_BUILD_BYTES};
use semaprax_native_rust_interop_platform as platform;

#[cfg(test)]
mod tests;

const NAMES: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.api.json",
    "package.json",
];
const ATTEMPTS: usize = 32;
static SERIAL: AtomicU64 = AtomicU64::new(0);
type Failure = Vec<Diagnostic>;

pub(super) fn publish(plan: &ProjectNpmPublication, output: &Path) -> Result<(), Failure> {
    if !matches!(
        plan.project_schema(),
        "semaprax.project.v8" | "semaprax.project.v9" | "semaprax.project.v10"
    ) {
        return Err(failure("unsupported owned npm Project schema"));
    }
    let mut artifacts = plan.artifacts();
    if artifacts.len() != NAMES.len() {
        return Err(failure("owned npm artifact inventory is not exact"));
    }
    let mut files: [(&str, &[u8]); 6] = [("", &[]); 6];
    for slot in &mut files {
        *slot = artifacts
            .next()
            .ok_or_else(|| failure("owned npm artifact inventory is not exact"))?;
    }
    publish_files(
        output,
        files,
        || SERIAL.fetch_add(1, Ordering::Relaxed),
        #[cfg(test)]
        |_, _| Ok(()),
    )
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Point {
    Created,
    BeforeWrite(usize),
    BeforeSettlement,
    AfterSettlement,
    AfterRename,
    BeforeFinalBinding,
}

struct Parent {
    requested: PathBuf,
    original: Handle,
    canonical: PathBuf,
    held: platform::HeldDirectory,
    output_name: OsString,
    output: PathBuf,
}

impl Parent {
    fn prepare(output: &Path) -> Result<Self, Failure> {
        require_requested_output(output)?;
        if output
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(failure(
                "npm package output may not contain parent traversal",
            ));
        }
        // Validate before Win32 absolute-path normalization can erase an
        // otherwise rejected trailing-dot/space leaf spelling.
        let requested_leaf = output
            .file_name()
            .ok_or_else(|| failure("npm package output must name one directory"))?;
        require_output_leaf(requested_leaf)?;
        platform::prepare_child_name(requested_leaf).map_err(map_error)?;
        let absolute = std::path::absolute(output)
            .map_err(|_| failure("cannot resolve npm output directory"))?;
        let output_name = absolute
            .file_name()
            .ok_or_else(|| failure("npm package output must name one directory"))?
            .to_os_string();
        // The existing Windows child grammar also rejects device names, ADS,
        // trailing dots/spaces and non-ASCII output leaves, before effects.
        platform::prepare_child_name(&output_name).map_err(map_error)?;
        let requested = absolute
            .parent()
            .ok_or_else(|| failure("npm package output has no parent directory"))?
            .to_path_buf();
        require_plain_directory(&requested)?;
        let original =
            Handle::from_path(&requested).map_err(|_| failure("cannot identify npm parent"))?;
        let canonical = requested
            .canonicalize()
            .map_err(|_| failure("cannot resolve npm parent"))?;
        let held = platform::hold_directory(&canonical).map_err(map_error)?;
        let output = canonical.join(&output_name);
        let parent = Self {
            requested,
            original,
            canonical,
            held,
            output_name,
            output,
        };
        parent.recheck()?;
        let child = platform::prepare_child_name(&parent.output_name).map_err(map_error)?;
        if !platform::child_absent_prepared(&parent.held, &child).map_err(map_error)? {
            return Err(failure("npm package destination already exists"));
        }
        Ok(parent)
    }

    fn recheck(&self) -> Result<(), Failure> {
        require_plain_directory(&self.requested)?;
        let original = Handle::from_path(&self.requested)
            .map_err(|_| failure("npm package parent identity changed"))?;
        let canonical = Handle::from_path(&self.canonical)
            .map_err(|_| failure("npm package parent identity changed"))?;
        if original != self.original
            || canonical != self.original
            || !platform::same_directory_path(&self.held, &self.canonical).map_err(map_error)?
        {
            return Err(failure("npm package parent identity changed"));
        }
        platform::recheck_directory(&self.held).map_err(map_error)
    }
}

fn require_requested_output(output: &Path) -> Result<(), Failure> {
    let encoded = output.as_os_str().as_encoded_bytes();
    if encoded
        .split(|byte| matches!(byte, b'/' | b'\\'))
        .any(|part| part == b"..")
    {
        return Err(failure(
            "npm package output may not contain parent traversal",
        ));
    }
    let leaf = encoded
        .rsplit(|byte| matches!(byte, b'/' | b'\\'))
        .next()
        .and_then(|leaf| std::str::from_utf8(leaf).ok())
        .ok_or_else(|| failure("npm package output leaf is invalid"))?;
    require_output_leaf_text(leaf)
}

fn require_output_leaf(name: &OsStr) -> Result<(), Failure> {
    let text = name
        .to_str()
        .filter(|text| !text.is_empty() && text.is_ascii())
        .ok_or_else(|| failure("npm package output leaf is invalid"))?;
    require_output_leaf_text(text)
}

fn require_output_leaf_text(text: &str) -> Result<(), Failure> {
    if text.is_empty() || !text.is_ascii() {
        return Err(failure("npm package output leaf is invalid"));
    }
    if matches!(text, "." | "..")
        || text.contains(['/', '\\', '\0', ':'])
        || text.ends_with([' ', '.'])
    {
        return Err(failure("npm package output leaf is invalid"));
    }
    let stem = text.split('.').next().unwrap_or("");
    if ["CON", "PRN", "AUX", "NUL", "CLOCK$"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case("COM") || stem[..3].eq_ignore_ascii_case("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(failure("npm package output leaf is invalid"));
    }
    Ok(())
}

fn require_plain_directory(path: &Path) -> Result<(), Failure> {
    use std::os::windows::fs::MetadataExt as _;
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| failure("npm package requires an existing parent"))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.file_attributes() & 0x400 != 0
    {
        return Err(failure(
            "npm package parent must be a non-reparse directory",
        ));
    }
    Ok(())
}

fn publish_files(
    output: &Path,
    files: [(&str, &[u8]); 6],
    mut serial: impl FnMut() -> u64,
    #[cfg(test)] mut observe: impl FnMut(Point, &Path) -> Result<(), Failure>,
) -> Result<(), Failure> {
    if files.map(|(name, _)| name) != NAMES {
        return Err(failure("owned npm artifact inventory is not exact"));
    }
    let total = files
        .iter()
        .try_fold(0usize, |total, (_, bytes)| total.checked_add(bytes.len()))
        .ok_or_else(|| failure("owned npm artifacts exceed the package bound"))?;
    if total > MAX_PROJECT_NPM_BUILD_BYTES {
        return Err(failure("owned npm artifacts exceed the package bound"));
    }
    let parent = Parent::prepare(output)?;
    let mut inventory =
        platform::prepare_discard_inventory(NAMES.map(OsStr::new)).map_err(map_error)?;
    let mut scan = platform::prepare_inventory_exact(&inventory).map_err(map_error)?;
    let mut final_scan =
        platform::prepare_inventory_entries_exact(NAMES.map(OsStr::new), 6).map_err(map_error)?;
    let mut rename = platform::prepare_publish_directory(&parent.output_name).map_err(map_error)?;
    let mut selected = None;
    for _ in 0..ATTEMPTS {
        let text = format!(".semaprax-owned-npm-{}-{}", std::process::id(), serial());
        if text
            .as_bytes()
            .eq_ignore_ascii_case(parent.output_name.as_encoded_bytes())
        {
            continue;
        }
        let name = platform::prepare_stage_name(OsStr::new(&text)).map_err(map_error)?;
        let path = parent.canonical.join(&text);
        match platform::create_directory_new_prepared_settled(&parent.held, &name, 0o700) {
            Ok(stage) => {
                selected = Some((name, path, stage));
                break;
            }
            Err(error) if error.namespace_created => {
                // No authenticated ownership was returned. Do not retry, infer
                // cleanup authority, or continue another publication phase.
                std::process::abort();
            }
            Err(error) if error.error == platform::Error::Exists => continue,
            Err(error) => return Err(map_error(error.error)),
        }
    }
    let (stage_name, stage_path, stage) =
        selected.ok_or_else(|| failure("cannot create fresh npm staging directory"))?;
    let preparation = (|| {
        #[cfg(test)]
        observe(Point::Created, &stage_path)?;
        parent.recheck()?;
        require_binding(&stage, &stage_path)?;
        for (index, (name, bytes)) in files.into_iter().enumerate() {
            #[cfg(test)]
            observe(Point::BeforeWrite(index), &stage_path)?;
            #[cfg(not(test))]
            let _ = index;
            platform::write_file_new_prepared(&stage, &mut inventory, name, bytes, 0o600)
                .map_err(map_error)?;
            compare(inventory.file(name).map_err(map_error)?, bytes)?;
        }
        platform::inventory_exact_prepared(&mut scan, &stage, &inventory).map_err(map_error)?;
        parent.recheck()?;
        require_binding(&stage, &stage_path)?;
        #[cfg(test)]
        observe(Point::BeforeSettlement, &stage_path)?;
        Ok(())
    })();
    if let Err(error) = preparation {
        // Only exact attached identities can be discarded. Untracked/foreign
        // partial files make this fail closed and leave inert residue.
        let _ =
            platform::discard_owned_stage_prepared(&parent.held, &stage, &stage_name, &inventory);
        return Err(error);
    }
    // Consuming boundary: there is no cleanup path or deleting Drop below it,
    // including settlement/rename errors or a published tree moved back.
    inventory.settle_for_publish().map_err(map_error)?;
    #[cfg(test)]
    observe(Point::AfterSettlement, &stage_path)?;
    platform::publish_directory_new_prepared(
        &mut rename,
        &parent.held,
        &stage,
        &stage_name,
        &parent.output_name,
    )
    .map_err(map_error)?;
    #[cfg(test)]
    observe(Point::AfterRename, &parent.output)?;
    let held = files
        .iter()
        .map(|(name, bytes)| {
            let file = platform::hold_regular_file(&stage, OsStr::new(name)).map_err(map_error)?;
            compare(&file, bytes)?;
            Ok(file)
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    platform::inventory_entries_exact_prepared(
        &mut final_scan,
        &stage,
        [&held[0], &held[1], &held[2], &held[3], &held[4], &held[5]],
        [],
    )
    .map_err(map_error)?;
    #[cfg(test)]
    observe(Point::BeforeFinalBinding, &parent.output)?;
    parent.recheck()?;
    require_binding(&stage, &parent.output)?;
    platform::recheck_directory(&stage).map_err(map_error)
}

fn compare(file: &platform::HeldRegularFile, expected: &[u8]) -> Result<(), Failure> {
    let mut scratch = [0u8; platform::FILE_COMPARE_SCRATCH_BYTES];
    if !platform::compare_exact(file, expected, &mut scratch).map_err(map_error)? {
        return Err(failure("npm artifact bytes changed during publication"));
    }
    platform::recheck_regular_file(file).map_err(map_error)
}

fn require_binding(directory: &platform::HeldDirectory, path: &Path) -> Result<(), Failure> {
    if !platform::same_directory_path(directory, path).map_err(map_error)? {
        return Err(failure("npm package directory identity changed"));
    }
    Ok(())
}

fn map_error(error: platform::Error) -> Failure {
    failure(if error == platform::Error::Exists {
        "npm package destination already exists"
    } else {
        "held Windows npm package publication failed"
    })
}

fn failure(message: &str) -> Failure {
    vec![Diagnostic::io("SPX-W120", message)]
}
