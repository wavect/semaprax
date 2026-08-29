//! Explicit-input, stdout-only Offline Deterministic Package Resolver v1 CLI.

use std::collections::BTreeSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use semaprax::diagnostic::Diagnostic;
use semaprax::package_resolver::{
    self, Requirement, ResolutionInput, ResolutionOptions,
    MAX_ALLOWED_CAPABILITIES as MAX_CAPABILITIES, MAX_OUTPUT_BYTES, MAX_REQUIREMENTS, MAX_SUBJECTS,
    MAX_SUBJECT_BYTES, MAX_TOTAL_SUBJECT_BYTES,
};

const MAX_NAME_BYTES: usize = 255;
const MAX_RANGE_BYTES: usize = 33;
const MAX_REQUIREMENT_BYTES: usize = MAX_NAME_BYTES + 1 + MAX_RANGE_BYTES;
const MAX_VERSION_COMPONENT_BYTES: usize = 10;
const MAX_OUTPUT_DECIMAL_BYTES: usize = decimal_width(MAX_OUTPUT_BYTES);

const fn decimal_width(mut value: usize) -> usize {
    let mut width = 1;
    while value >= 10 {
        value /= 10;
        width += 1;
    }
    width
}

pub(crate) enum PackageResolverCliError {
    Usage(String),
    Domain(Vec<Diagnostic>),
}

pub(crate) fn run(arguments: &[String]) -> Result<String, PackageResolverCliError> {
    let parsed = parse(arguments)?;
    let current_dir = std::env::current_dir().map_err(|_| {
        PackageResolverCliError::Domain(vec![io_error(
            "cannot capture the package-resolve current directory",
        )])
    })?;
    let paths = parsed
        .paths
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                current_dir.join(path)
            }
        })
        .collect::<Vec<_>>();
    let subjects =
        read_subjects(&paths).map_err(|error| PackageResolverCliError::Domain(vec![error]))?;
    package_resolver::generate(
        &ResolutionInput {
            requirements: parsed.requirements,
            subjects,
            target: parsed.target,
            allowed_capabilities: parsed.allowed_capabilities,
        },
        &parsed.options,
    )
    .map_err(PackageResolverCliError::Domain)
}

struct Parsed {
    paths: Vec<PathBuf>,
    requirements: Vec<Requirement>,
    target: String,
    allowed_capabilities: Vec<String>,
    options: ResolutionOptions,
}

fn parse(arguments: &[String]) -> Result<Parsed, PackageResolverCliError> {
    if arguments.iter().any(|argument| argument.is_empty()) {
        return Err(usage("package-resolve arguments must not be empty"));
    }
    let mut index = 0usize;
    let mut paths = Vec::new();
    while let Some(value) = arguments.get(index) {
        if value.starts_with('-') {
            break;
        }
        if paths.len() == MAX_SUBJECTS {
            return Err(usage(format!(
                "package-resolve requires 1..{MAX_SUBJECTS} explicit subject files"
            )));
        }
        if value.starts_with('@') {
            return Err(usage(
                "package-resolve subject files must not use `@` response-file syntax",
            ));
        }
        paths.push(PathBuf::from(value));
        index += 1;
    }
    if !(1..=MAX_SUBJECTS).contains(&paths.len()) {
        return Err(usage(format!(
            "package-resolve requires 1..{MAX_SUBJECTS} explicit subject files"
        )));
    }

    let mut requirements = Vec::new();
    while arguments.get(index).map(String::as_str) == Some("--require") {
        if requirements.len() == MAX_REQUIREMENTS {
            return Err(usage(format!(
                "package-resolve requires 1..{MAX_REQUIREMENTS} contiguous `--require` values"
            )));
        }
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| usage("package-resolve option `--require` requires a value"))?;
        requirements.push(parse_requirement(value)?);
        index += 2;
    }
    if !(1..=MAX_REQUIREMENTS).contains(&requirements.len()) {
        return Err(usage(format!(
            "package-resolve requires 1..{MAX_REQUIREMENTS} contiguous `--require` values"
        )));
    }
    for pair in requirements.windows(2) {
        if pair[0].package >= pair[1].package {
            return Err(usage(
                "package-resolve requirements must be strictly package-sorted and unique",
            ));
        }
    }

    if arguments.get(index).map(String::as_str) != Some("--target") {
        return Err(usage(
            "package-resolve requires one `--target` after its requirements",
        ));
    }
    let target = arguments
        .get(index + 1)
        .ok_or_else(|| usage("package-resolve option `--target` requires a value"))?
        .to_owned();
    if !matches!(target.as_str(), "native64" | "wasm32") {
        return Err(usage(
            "package-resolve target must be exactly `native64` or `wasm32`",
        ));
    }
    index += 2;

    let mut allowed_capabilities = Vec::new();
    while arguments.get(index).map(String::as_str) == Some("--allow-capability") {
        if allowed_capabilities.len() == MAX_CAPABILITIES {
            return Err(usage(format!(
                "package-resolve accepts at most {MAX_CAPABILITIES} capabilities"
            )));
        }
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| usage("package-resolve option `--allow-capability` requires a value"))?;
        validate_name("capability", value)?;
        allowed_capabilities.push(value.to_owned());
        index += 2;
    }
    if allowed_capabilities.len() > MAX_CAPABILITIES {
        return Err(usage(format!(
            "package-resolve accepts at most {MAX_CAPABILITIES} capabilities"
        )));
    }
    if allowed_capabilities
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(usage(
            "package-resolve capabilities must be strictly byte-sorted and unique",
        ));
    }

    let mut max_bytes = ResolutionOptions::default().max_bytes;
    if arguments.get(index).map(String::as_str) == Some("--max-bytes") {
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| usage("package-resolve option `--max-bytes` requires a value"))?;
        max_bytes = canonical_number("--max-bytes", value)?;
        index += 2;
    }
    if index != arguments.len() {
        let value = &arguments[index];
        return Err(usage(format!(
            "unknown or out-of-order package-resolve argument `{value}`"
        )));
    }
    let options = ResolutionOptions::new(max_bytes).map_err(|error| usage(error.to_string()))?;
    Ok(Parsed {
        paths,
        requirements,
        target,
        allowed_capabilities,
        options,
    })
}

fn parse_requirement(value: &str) -> Result<Requirement, PackageResolverCliError> {
    if value.len() > MAX_REQUIREMENT_BYTES {
        return Err(usage("package-resolve requirement exceeds its byte bound"));
    }
    let (package, range) = value
        .split_once(':')
        .ok_or_else(|| usage("package-resolve requirement must be `<package>:<range>`"))?;
    validate_name("package", package)?;
    validate_range(range)?;
    Ok(Requirement {
        package: package.to_owned(),
        range: range.to_owned(),
    })
}

fn validate_name(kind: &str, value: &str) -> Result<(), PackageResolverCliError> {
    if value.is_empty()
        || value.len() > MAX_NAME_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(usage(format!(
            "package-resolve {kind} must use 1..{MAX_NAME_BYTES} ASCII `[A-Za-z0-9._-]` bytes"
        )));
    }
    Ok(())
}

fn validate_range(value: &str) -> Result<(), PackageResolverCliError> {
    let bytes = value.as_bytes();
    if bytes.len() < 6 || bytes.len() > MAX_RANGE_BYTES || !matches!(bytes[0], b'=' | b'^' | b'~') {
        return Err(usage("package-resolve range grammar is invalid"));
    }
    let components = value[1..].split('.').collect::<Vec<_>>();
    if components.len() != 3 {
        return Err(usage("package-resolve range grammar is invalid"));
    }
    let mut parsed = [0u32; 3];
    for (index, component) in components.iter().enumerate() {
        if component.is_empty()
            || component.len() > MAX_VERSION_COMPONENT_BYTES
            || !component.bytes().all(|byte| byte.is_ascii_digit())
            || (component.len() > 1 && component.starts_with('0'))
        {
            return Err(usage("package-resolve range grammar is invalid"));
        }
        parsed[index] = component
            .parse::<u32>()
            .map_err(|_| usage("package-resolve range component exceeds u32"))?;
    }
    let upper_overflows = match bytes[0] {
        b'~' => parsed[1] == u32::MAX,
        b'^' if parsed[0] != 0 => parsed[0] == u32::MAX,
        b'^' if parsed[1] != 0 => parsed[1] == u32::MAX,
        b'^' => parsed[2] == u32::MAX,
        _ => false,
    };
    if upper_overflows {
        return Err(usage("package-resolve range upper bound overflows u32"));
    }
    Ok(())
}

fn canonical_number(option: &str, value: &str) -> Result<usize, PackageResolverCliError> {
    if value.is_empty()
        || value.len() > MAX_OUTPUT_DECIMAL_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(usage(format!(
            "package-resolve option `{option}` requires a canonical positive integer"
        )));
    }
    value.parse::<usize>().map_err(|_| {
        usage(format!(
            "package-resolve option `{option}` requires a canonical positive integer"
        ))
    })
}

struct HeldInput {
    file: std::fs::File,
    bytes: usize,
    identity: (u64, u64),
}

fn read_subjects(paths: &[PathBuf]) -> Result<Vec<String>, Diagnostic> {
    read_subjects_with_hook(paths, &mut NoopSubjectReadHook)
}

trait SubjectReadHook {
    fn before_read(&mut self, _index: usize, _file: &std::fs::File) {}
    fn after_read(&mut self, _index: usize, _file: &std::fs::File) {}
}

struct NoopSubjectReadHook;

impl SubjectReadHook for NoopSubjectReadHook {}

fn read_subjects_with_hook(
    paths: &[PathBuf],
    hook: &mut impl SubjectReadHook,
) -> Result<Vec<String>, Diagnostic> {
    let mut held = Vec::with_capacity(paths.len());
    let mut identities = BTreeSet::new();
    let mut declared_total = 0usize;
    for path in paths {
        let file = open_leaf_no_follow(path)?;
        let metadata = inspect(&file)?;
        let identity = held_input_identity(&file, &metadata)?;
        if !identities.insert(identity) {
            return Err(io_error(
                "package-resolve subject inputs must have distinct held file identities",
            ));
        }
        let bytes = usize::try_from(metadata.len())
            .map_err(|_| limit_error("subject byte count does not fit the host size"))?;
        if bytes > MAX_SUBJECT_BYTES {
            return Err(limit_error("subject_bytes exceeds 17825792"));
        }
        declared_total = declared_total
            .checked_add(bytes)
            .ok_or_else(|| limit_error("total_subject_bytes overflow"))?;
        if declared_total > MAX_TOTAL_SUBJECT_BYTES {
            return Err(limit_error("total_subject_bytes exceeds 134217728"));
        }
        held.push(HeldInput {
            file,
            bytes,
            identity,
        });
    }

    let mut subjects = Vec::with_capacity(held.len());
    let mut actual_total = 0usize;
    for (
        index,
        HeldInput {
            file,
            bytes,
            identity,
        },
    ) in held.into_iter().enumerate()
    {
        hook.before_read(index, &file);
        let mut content = Vec::with_capacity(bytes);
        let remaining = MAX_TOTAL_SUBJECT_BYTES
            .checked_sub(actual_total)
            .ok_or_else(|| limit_error("actual total_subject_bytes exceeds its bound"))?;
        let read_limit = MAX_SUBJECT_BYTES
            .min(remaining)
            .checked_add(1)
            .ok_or_else(|| limit_error("subject read limit overflow"))?;
        (&file)
            .take(read_limit as u64)
            .read_to_end(&mut content)
            .map_err(|_| io_error("cannot read held package-resolve subject input"))?;
        hook.after_read(index, &file);
        actual_total = actual_total
            .checked_add(content.len())
            .ok_or_else(|| limit_error("actual total_subject_bytes overflow"))?;
        if content.len() > MAX_SUBJECT_BYTES || actual_total > MAX_TOTAL_SUBJECT_BYTES {
            return Err(limit_error("actual subject byte bound exceeded"));
        }
        let after = inspect(&file)?;
        if held_input_identity(&file, &after)? != identity
            || usize::try_from(after.len()).ok() != Some(bytes)
            || content.len() != bytes
        {
            return Err(io_error(
                "held package-resolve subject input changed during its single read",
            ));
        }
        subjects.push(
            String::from_utf8(content)
                .map_err(|_| input_error("package-resolve subject input must be valid UTF-8"))?,
        );
    }
    Ok(subjects)
}

fn inspect(file: &std::fs::File) -> Result<std::fs::Metadata, Diagnostic> {
    let metadata = file
        .metadata()
        .map_err(|_| io_error("cannot inspect held package-resolve subject input"))?;
    if !metadata.is_file() {
        return Err(io_error(
            "package-resolve subject input must be a regular file",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if !windows_file_attributes_are_admitted(metadata.file_attributes()) {
            return Err(io_error(
                "package-resolve subject input must not be a reparse point",
            ));
        }
    }
    Ok(metadata)
}

#[cfg(any(windows, test))]
const WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

#[cfg(any(windows, test))]
fn windows_file_attributes_are_admitted(attributes: u32) -> bool {
    attributes & WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT == 0
}

#[cfg(unix)]
fn open_leaf_no_follow(path: &Path) -> Result<std::fs::File, Diagnostic> {
    use rustix::fs::{open, Mode, OFlags};
    open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(|_| io_error("cannot open package-resolve subject input without following links"))
}

#[cfg(windows)]
fn open_leaf_no_follow(path: &Path) -> Result<std::fs::File, Diagnostic> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| io_error("cannot open package-resolve subject input reparse-safely"))
}

#[cfg(not(any(unix, windows)))]
fn open_leaf_no_follow(_path: &Path) -> Result<std::fs::File, Diagnostic> {
    Err(io_error(
        "package-resolve held input authority is unsupported on this host",
    ))
}

#[cfg(unix)]
fn held_input_identity(
    _file: &std::fs::File,
    metadata: &std::fs::Metadata,
) -> Result<(u64, u64), Diagnostic> {
    use std::os::unix::fs::MetadataExt as _;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn held_input_identity(
    file: &std::fs::File,
    _metadata: &std::fs::Metadata,
) -> Result<(u64, u64), Diagnostic> {
    let information = winapi_util::file::information(file)
        .map_err(|_| io_error("held package-resolve subject file identity is unavailable"))?;
    Ok((information.volume_serial_number(), information.file_index()))
}

#[cfg(not(any(unix, windows)))]
fn held_input_identity(
    _file: &std::fs::File,
    _metadata: &std::fs::Metadata,
) -> Result<(u64, u64), Diagnostic> {
    Err(io_error(
        "held package-resolve subject identity is unsupported on this host",
    ))
}

fn io_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-I215", message.into())
}

fn input_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR501", message.into())
}

fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR505", message.into())
}

fn usage(message: impl Into<String>) -> PackageResolverCliError {
    PackageResolverCliError::Usage(message.into())
}

#[cfg(test)]
mod held_read_tests;

#[cfg(test)]
mod bounded_parser_tests;
