//! Parsing and validation for bounded command-specific CLI options.

use super::*;

pub(super) fn write_package_resolver_stdout(evidence: &str) -> Result<(), Diagnostic> {
    #[cfg(unix)]
    let mut stdout = std::fs::File::from(
        rustix::io::dup(rustix::stdio::stdout()).map_err(|_| package_resolver_stdout_error())?,
    );
    #[cfg(not(unix))]
    let stdout = std::io::stdout();
    #[cfg(not(unix))]
    let mut stdout = stdout.lock();
    stdout
        .write_all(evidence.as_bytes())
        .and_then(|()| stdout.write_all(b"\n"))
        .and_then(|()| stdout.flush())
        .map_err(|_| package_resolver_stdout_error())
}

pub(super) fn package_resolver_stdout_error() -> Diagnostic {
    Diagnostic::io(
        "SPX-I215",
        "cannot write package-resolve evidence to standard output",
    )
}

pub(super) fn serve_options(args: &[String]) -> Result<agent_transport::TransportLimits, u8> {
    let mut max_request_bytes = agent_transport::DEFAULT_MAX_REQUEST_BYTES;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-request-bytes") {
            eprintln!("unknown serve option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate serve option `{option}`");
            return Err(2);
        }
        let Some(value) = args.get(index + 1) else {
            eprintln!("serve option `{option}` requires a value");
            return Err(2);
        };
        let parsed = context_number(option, value)?;
        max_request_bytes = parsed;
        index += 2;
    }
    agent_transport::TransportLimits::new(max_request_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn with_native_executable_suffix(path: PathBuf) -> PathBuf {
    let extension = std::env::consts::EXE_EXTENSION;
    if extension.is_empty() || path.extension().is_some() {
        return path;
    }
    path.with_extension(extension)
}

/// Exit status of a child that was terminated by a signal. Shell convention
/// reports `128 + signal`; platforms without signal exit statuses fall back
/// to the generic failure code.
pub(super) fn child_exit_code(status: &std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map_or(1, |signal| 128 + signal)
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        1
    }
}

/// The `run` command reports child failures as its own `u8` exit code. Raw
/// platform codes can exceed that range (Windows NTSTATUS crash codes such
/// as `0xC0000005`), so out-of-range values fall back to the generic failure
/// code after printing the exact code for diagnosis instead of silently
/// truncating a hard crash into an ordinary small failure.
pub(super) fn child_result_code(status: &std::process::ExitStatus) -> u8 {
    let raw = status.code().unwrap_or_else(|| child_exit_code(status));
    u8::try_from(raw).unwrap_or_else(|_| {
        eprintln!("child process exited with code {raw}");
        1
    })
}

pub(super) fn interpret_options(
    args: &[String],
) -> Result<(String, Vec<String>, interpreter::InterpreterOptions), u8> {
    let mut function = None;
    let mut arguments = Vec::new();
    let mut options = interpreter::InterpreterOptions::default();
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--function" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate interpret option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("interpret option `{option}` requires a value");
                    2
                })?;
                if value.is_empty() {
                    eprintln!("interpret option `{option}` requires a function name or stable id");
                    return Err(2);
                }
                function = Some(value.clone());
            }
            "--arg" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("interpret option `{option}` requires a value");
                    2
                })?;
                if value.is_empty() {
                    eprintln!("interpret option `{option}` requires a scalar literal");
                    return Err(2);
                }
                arguments.push(value.clone());
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate interpret option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("interpret option `{option}` requires a value");
                    2
                })?;
                options.max_bytes = property_number(option, value)?;
            }
            other => {
                eprintln!("unknown interpret option `{other}`");
                return Err(2);
            }
        }
        index += 2;
    }
    let Some(function) = function else {
        eprintln!("interpret requires --function <name|stable-id>");
        return Err(2);
    };
    let options = interpreter::InterpreterOptions::new(options.max_bytes, options.max_steps)
        .map_err(|error| {
            eprintln!("{error}");
            2
        })?;
    Ok((function, arguments, options))
}

pub(super) fn plugin_manifest_options(
    args: &[String],
) -> Result<plugin_manifest::PluginManifestOptions, u8> {
    let mut max_bytes = plugin_manifest::PluginManifestOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown plugin-manifest option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate plugin-manifest option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("plugin-manifest option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    plugin_manifest::PluginManifestOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn ui_schema_options(args: &[String]) -> Result<ui_schema::UiSchemaOptions, u8> {
    let mut max_bytes = ui_schema::UiSchemaOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown ui-schema option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate ui-schema option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("ui-schema option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    ui_schema::UiSchemaOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn cxx_shim_options(args: &[String]) -> Result<(cxx_shim::CxxShimOptions, bool), u8> {
    cxx_selection_options(args, "cxx-shim", true)
}

pub(super) fn cxx_package_options(args: &[String]) -> Result<cxx_shim::CxxShimOptions, u8> {
    let (options, emit_fragment) = cxx_selection_options(args, "cxx-package", false)?;
    debug_assert!(!emit_fragment);
    Ok(options)
}

pub(super) fn cxx_selection_options(
    args: &[String],
    command: &str,
    allow_fragment: bool,
) -> Result<(cxx_shim::CxxShimOptions, bool), u8> {
    let mut functions: Vec<String> = Vec::new();
    let mut max_bytes = cxx_shim::CxxShimOptions::default().max_bytes;
    let mut emit_fragment = false;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--function" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("{command} option `{option}` requires a value");
                    2
                })?;
                for token in value.split(',') {
                    if token.is_empty() {
                        eprintln!("{command} option `{option}` requires nonempty selections");
                        return Err(2);
                    }
                    functions.push(token.to_owned());
                }
                index += 2;
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate {command} option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("{command} option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            "--emit-fragment" if allow_fragment => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate {command} option `{option}`");
                    return Err(2);
                }
                emit_fragment = true;
                index += 1;
            }
            other => {
                eprintln!("unknown {command} option `{other}`");
                return Err(2);
            }
        }
    }
    let options = cxx_shim::CxxShimOptions::new(functions, max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })?;
    Ok((options, emit_fragment))
}

pub(super) fn property_number(option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("properties option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("properties option `{option}` requires a canonical nonnegative integer");
        2
    })
}

pub(super) fn property_seed(option: &str, value: &str) -> Result<u64, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("properties option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<u64>().map_err(|_| {
        eprintln!("properties option `{option}` requires a canonical nonnegative integer");
        2
    })
}

pub(super) enum ParsedContextOptions {
    V1(graph::AgentContextOptions),
    V2(graph::AgentContextV2Options),
}

impl ParsedContextOptions {
    pub(super) const fn max_bytes(&self) -> usize {
        match self {
            Self::V1(options) => options.max_bytes(),
            Self::V2(options) => options.max_bytes(),
        }
    }
}

pub(super) fn project_context_options(
    options: &ParsedContextOptions,
) -> Result<workspace_analysis::WorkspaceContextOptions, u8> {
    let (direction, depth, max_nodes) = match options {
        ParsedContextOptions::V1(options) => (
            workspace_analysis::WorkspaceAnalysisDirection::Forward,
            options.depth(),
            options.max_nodes(),
        ),
        ParsedContextOptions::V2(options) => {
            let direction = match options.direction() {
                graph::AgentContextDirection::Forward => {
                    workspace_analysis::WorkspaceAnalysisDirection::Forward
                }
                graph::AgentContextDirection::Reverse => {
                    workspace_analysis::WorkspaceAnalysisDirection::Reverse
                }
                graph::AgentContextDirection::Both => {
                    workspace_analysis::WorkspaceAnalysisDirection::Both
                }
            };
            (direction, options.depth(), options.max_nodes())
        }
    };
    // The public limit applies to the compact CLI projection. Its authenticated
    // compiler-owned input can be larger without transferring those bytes.
    let internal_bytes = 16 * 1024 * 1024;
    workspace_analysis::WorkspaceContextOptions::new(direction, depth, internal_bytes, max_nodes)
        .map_err(|error| {
            eprintln!("{error}");
            2
        })
}

pub(super) fn context_options(args: &[String]) -> Result<ParsedContextOptions, u8> {
    let defaults = graph::AgentContextOptions::default();
    let mut depth = defaults.depth();
    let mut max_bytes = defaults.max_bytes();
    let mut max_nodes = defaults.max_nodes();
    let mut filters = None;
    let mut direction = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 3;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--depth" | "--max-bytes" | "--max-nodes" | "--filters" | "--direction"
        ) {
            eprintln!("unknown context option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate context option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("context option `{option}` requires a value");
            2
        })?;
        match option {
            "--depth" => depth = context_number(option, value)?,
            "--max-bytes" => max_bytes = context_number(option, value)?,
            "--max-nodes" => max_nodes = context_number(option, value)?,
            "--filters" => {
                if value.is_empty() {
                    eprintln!("context --filters requires a comma-separated nonempty list");
                    return Err(2);
                }
                let mut parsed = std::collections::BTreeSet::new();
                for name in value.split(',') {
                    let Some(filter) = graph::AgentContextFilter::from_name(name) else {
                        eprintln!("unknown context filter `{name}`");
                        return Err(2);
                    };
                    if !parsed.insert(filter) {
                        eprintln!("duplicate context filter `{name}`");
                        return Err(2);
                    }
                }
                filters = Some(parsed);
            }
            "--direction" => {
                let Some(parsed) = graph::AgentContextDirection::from_name(value) else {
                    eprintln!("unknown context direction `{value}`");
                    return Err(2);
                };
                direction = Some(parsed);
            }
            _ => unreachable!("closed context option table"),
        }
        index += 2;
    }
    let filters = filters.unwrap_or_else(|| {
        [
            graph::AgentContextFilter::Contracts,
            graph::AgentContextFilter::Ownership,
            graph::AgentContextFilter::Effects,
            graph::AgentContextFilter::Types,
        ]
        .into_iter()
        .collect()
    });
    match direction {
        Some(direction) => {
            graph::AgentContextV2Options::new(depth, max_bytes, max_nodes, filters, direction)
                .map(ParsedContextOptions::V2)
                .map_err(|error| {
                    eprintln!("{error}");
                    2
                })
        }
        None => graph::AgentContextOptions::new(depth, max_bytes, max_nodes, filters)
            .map(ParsedContextOptions::V1)
            .map_err(|error| {
                eprintln!("{error}");
                2
            }),
    }
}

pub(super) fn context_number(option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("context option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("context option `{option}` requires a canonical nonnegative integer");
        2
    })
}

#[cfg(test)]
#[path = "options/tests.rs"]
mod tests;
