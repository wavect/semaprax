use super::*;

pub(super) fn workspace_analysis_target_kind(
    command: &str,
    value: &str,
) -> Result<workspace_analysis::WorkspaceAnalysisTargetKind, u8> {
    match value {
        "declaration" => Ok(workspace_analysis::WorkspaceAnalysisTargetKind::Declaration),
        "capability" => Ok(workspace_analysis::WorkspaceAnalysisTargetKind::Capability),
        _ => {
            eprintln!("{command} target kind must be `declaration` or `capability`");
            Err(2)
        }
    }
}

pub(super) fn workspace_context_options(
    args: &[String],
) -> Result<workspace_analysis::WorkspaceContextOptions, u8> {
    let mut direction = workspace_analysis::WorkspaceAnalysisDirection::Both;
    let mut depth = 4usize;
    let mut max_bytes = 1024 * 1024usize;
    let mut max_nodes = 1024usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 5usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--direction" | "--depth" | "--max-bytes" | "--max-nodes"
        ) {
            eprintln!("unknown workspace-context option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate workspace-context option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("workspace-context option `{option}` requires a value");
            2
        })?;
        match option {
            "--direction" => {
                direction = match value.as_str() {
                    "forward" => workspace_analysis::WorkspaceAnalysisDirection::Forward,
                    "reverse" => workspace_analysis::WorkspaceAnalysisDirection::Reverse,
                    "both" => workspace_analysis::WorkspaceAnalysisDirection::Both,
                    _ => {
                        eprintln!("unknown workspace-context direction `{value}`");
                        return Err(2);
                    }
                };
            }
            "--depth" => depth = workspace_analysis_number("workspace-context", option, value)?,
            "--max-bytes" => {
                max_bytes = workspace_analysis_number("workspace-context", option, value)?;
            }
            "--max-nodes" => {
                max_nodes = workspace_analysis_number("workspace-context", option, value)?;
            }
            _ => unreachable!("closed workspace-context option table"),
        }
        index += 2;
    }
    workspace_analysis::WorkspaceContextOptions::new(direction, depth, max_bytes, max_nodes)
        .map_err(|error| {
            eprintln!("{error}");
            2
        })
}

pub(super) fn workspace_impact_options(
    args: &[String],
) -> Result<workspace_analysis::WorkspaceImpactOptions, u8> {
    let mut depth = 16usize;
    let mut max_bytes = 1024 * 1024usize;
    let mut max_nodes = 1024usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 5usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--depth" | "--max-bytes" | "--max-nodes") {
            eprintln!("unknown workspace-impact option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate workspace-impact option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("workspace-impact option `{option}` requires a value");
            2
        })?;
        let value = workspace_analysis_number("workspace-impact", option, value)?;
        match option {
            "--depth" => depth = value,
            "--max-bytes" => max_bytes = value,
            "--max-nodes" => max_nodes = value,
            _ => unreachable!("closed workspace-impact option table"),
        }
        index += 2;
    }
    workspace_analysis::WorkspaceImpactOptions::new(depth, max_bytes, max_nodes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn workspace_analysis_number(
    command: &str,
    option: &str,
    value: &str,
) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("{command} option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("{command} option `{option}` requires a canonical nonnegative integer");
        2
    })
}

pub(super) fn impact_options(args: &[String]) -> Result<impact::SemanticImpactOptions, u8> {
    let mut depth = 1usize;
    let mut max_bytes = 64 * 1024;
    let mut max_nodes = 256usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 3usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--depth" | "--max-bytes" | "--max-nodes") {
            eprintln!("unknown impact option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate impact option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("impact option `{option}` requires a value");
            2
        })?;
        let value = impact_number(option, value)?;
        match option {
            "--depth" => depth = value,
            "--max-bytes" => max_bytes = value,
            "--max-nodes" => max_nodes = value,
            _ => unreachable!("closed impact option table"),
        }
        index += 2;
    }
    impact::SemanticImpactOptions::new(depth, max_bytes, max_nodes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn impact_number(option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("impact option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("impact option `{option}` requires a canonical nonnegative integer");
        2
    })
}

pub(super) fn openapi_options(
    args: &[String],
) -> Result<(Vec<String>, openapi::OpenApiOptions), u8> {
    let mut functions = Vec::new();
    let mut max_bytes = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--function" | "--max-bytes") {
            eprintln!("unknown openapi option `{option}`");
            return Err(2);
        }
        if option == "--max-bytes" && !seen.insert(option.to_owned()) {
            eprintln!("duplicate openapi option `{option}`");
            return Err(2);
        }
        // A selection is free text, so only the leading-dash filter keeps a
        // value-less `--function` from adopting the following flag and silently
        // discarding the byte budget behind it. `--max-bytes` stays unfiltered:
        // its canonical integer grammar already refuses a `-` value, and does so
        // with the more precise malformed-number diagnostic.
        let value = args
            .get(index + 1)
            .filter(|value| option != "--function" || !value.starts_with('-'))
            .ok_or_else(|| {
                eprintln!("openapi option `{option}` requires a value");
                2
            })?;
        match option {
            "--function" => {
                if value.is_empty() {
                    eprintln!("openapi option `--function` requires a function name or stable id");
                    return Err(2);
                }
                functions.push(value.clone());
            }
            _ => max_bytes = Some(openapi_number(option, value)?),
        }
        index += 2;
    }
    if functions.is_empty() {
        eprintln!("openapi requires at least one --function <name|stable-id> selection");
        return Err(2);
    }
    let options = openapi::OpenApiOptions::new(
        max_bytes.unwrap_or_else(|| openapi::OpenApiOptions::default().max_bytes),
    )
    .map_err(|error| {
        eprintln!("{error}");
        2
    })?;
    Ok((functions, options))
}

pub(super) fn openapi_compat_options(args: &[String]) -> Result<openapi::OpenApiOptions, u8> {
    let mut max_bytes = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 3usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown openapi-compat option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate openapi-compat option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("openapi-compat option `{option}` requires a value");
            2
        })?;
        max_bytes = Some(openapi_number(option, value)?);
        index += 2;
    }
    openapi::OpenApiOptions::new(
        max_bytes.unwrap_or_else(|| openapi::OpenApiOptions::default().max_bytes),
    )
    .map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn openapi_number(option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("openapi option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("openapi option `{option}` requires a canonical nonnegative integer");
        2
    })
}

pub(super) fn property_options(args: &[String]) -> Result<properties::PropertyTestOptions, u8> {
    let mut max_cases = properties::PropertyTestOptions::default().max_cases;
    let mut max_functions = properties::PropertyTestOptions::default().max_functions;
    let mut max_bytes = properties::PropertyTestOptions::default().max_bytes;
    let mut seed = properties::PropertyTestOptions::default().seed;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--max-cases" | "--max-functions" | "--max-bytes" | "--seed"
        ) {
            eprintln!("unknown properties option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate properties option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("properties option `{option}` requires a value");
            2
        })?;
        match option {
            "--seed" => seed = property_seed(option, value)?,
            _ => {
                let number = property_number(option, value)?;
                match option {
                    "--max-cases" => max_cases = number,
                    "--max-functions" => max_functions = number,
                    "--max-bytes" => max_bytes = number,
                    _ => unreachable!("closed properties option table"),
                }
            }
        }
        index += 2;
    }
    properties::PropertyTestOptions::new(max_cases, max_functions, max_bytes, seed).map_err(
        |error| {
            eprintln!("{error}");
            2
        },
    )
}

pub(super) fn hygienic_options(args: &[String]) -> Result<hygienic::HygienicGenOptions, u8> {
    let mut templates: Vec<hygienic::Template> = Vec::new();
    let mut max_bytes = hygienic::HygienicGenOptions::default().max_bytes();
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--templates" | "--max-bytes") {
            eprintln!("unknown hygienic-gen option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate hygienic-gen option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("hygienic-gen option `{option}` requires a value");
            2
        })?;
        match option {
            "--templates" => templates = hygienic_templates(option, value)?,
            "--max-bytes" => max_bytes = property_number(option, value)?,
            _ => unreachable!("closed hygienic-gen option table"),
        }
        index += 2;
    }
    let selection = if templates.is_empty() {
        hygienic::Template::REGISTRY.to_vec()
    } else {
        templates
    };
    hygienic::HygienicGenOptions::new(&selection, max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn hygienic_templates(option: &str, value: &str) -> Result<Vec<hygienic::Template>, u8> {
    let mut templates = Vec::new();
    for token in value.split(',') {
        let Some(template) = hygienic::Template::from_id(token) else {
            eprintln!(
                "hygienic-gen option `{option}` only accepts registry template ids; \
                 unknown `{token}`"
            );
            return Err(2);
        };
        if templates.contains(&template) {
            eprintln!("hygienic-gen option `{option}` repeats template `{token}`");
            return Err(2);
        }
        templates.push(template);
    }
    Ok(templates)
}

pub(super) fn abi_report_options(args: &[String]) -> Result<abi_report::AbiReportOptions, u8> {
    let mut functions: Vec<String> = Vec::new();
    let mut max_bytes = abi_report::AbiReportOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--function" => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| {
                        eprintln!("abi-report option `{option}` requires a value");
                        2
                    })?;
                for token in value.split(',') {
                    if token.is_empty() {
                        eprintln!("abi-report option `{option}` requires nonempty selections");
                        return Err(2);
                    }
                    functions.push(token.to_owned());
                }
                index += 2;
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate abi-report option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("abi-report option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            other => {
                eprintln!("unknown abi-report option `{other}`");
                return Err(2);
            }
        }
    }
    abi_report::AbiReportOptions::new(functions, max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn c_header_options(args: &[String]) -> Result<(c_header::CHeaderOptions, bool), u8> {
    let mut functions: Vec<String> = Vec::new();
    let mut max_bytes = c_header::CHeaderOptions::default().max_bytes;
    let mut emit_header = false;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--function" => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| {
                        eprintln!("c-header option `{option}` requires a value");
                        2
                    })?;
                for token in value.split(',') {
                    if token.is_empty() {
                        eprintln!("c-header option `{option}` requires nonempty selections");
                        return Err(2);
                    }
                    functions.push(token.to_owned());
                }
                index += 2;
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate c-header option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("c-header option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            "--emit-header" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate c-header option `{option}`");
                    return Err(2);
                }
                emit_header = true;
                index += 1;
            }
            other => {
                eprintln!("unknown c-header option `{other}`");
                return Err(2);
            }
        }
    }
    let options = c_header::CHeaderOptions::new(functions, max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })?;
    Ok((options, emit_header))
}

pub(super) fn freestanding_object_options(
    args: &[String],
) -> Result<freestanding_object::FreestandingObjectOptions, u8> {
    let mut max_bytes = freestanding_object::FreestandingObjectOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate freestanding-object option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("freestanding-object option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            other => {
                eprintln!("unknown freestanding-object option `{other}`");
                return Err(2);
            }
        }
    }
    freestanding_object::FreestandingObjectOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn capability_manifest_options(
    args: &[String],
) -> Result<capability_manifest::CapabilityManifestOptions, u8> {
    let mut max_bytes = capability_manifest::CapabilityManifestOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown capability-manifest option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate capability-manifest option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("capability-manifest option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    capability_manifest::CapabilityManifestOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn package_report_options(
    args: &[String],
) -> Result<package_report::PackageReportOptions, u8> {
    let mut max_bytes = package_report::PackageReportOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown package-report option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate package-report option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("package-report option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    package_report::PackageReportOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn region_report_options(
    args: &[String],
) -> Result<region_report::RegionReportOptions, u8> {
    let mut max_bytes = region_report::RegionReportOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown region-report option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate region-report option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("region-report option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    region_report::RegionReportOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn simd_report_options(args: &[String]) -> Result<simd_report::SimdReportOptions, u8> {
    let mut max_bytes = simd_report::SimdReportOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown simd-report option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate simd-report option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("simd-report option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    simd_report::SimdReportOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

pub(super) fn protocol_check_options(
    args: &[String],
) -> Result<protocol_check::ProtocolCheckOptions, u8> {
    let mut max_bytes = protocol_check::ProtocolCheckOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown protocol-check option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate protocol-check option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("protocol-check option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    protocol_check::ProtocolCheckOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

#[cfg(test)]
#[path = "report_options/tests.rs"]
mod tests;
