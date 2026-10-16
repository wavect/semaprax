//! Additive, self-contained C/C++ package projection over C++ Shim v1.

use std::path::Path;

use crate::ast::Type;
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};

use super::{
    consistency_error, domain_digest, generate_from_source, generate_internal_bounded,
    CxxShimOptions,
};

pub const PACKAGE_SCHEMA: &str = "semaprax.cxx-package.v1";
const PAYLOAD_DOMAIN: &[u8] = b"semaprax.cxx-package.payload.v1\0";
const HEADER_DOMAIN: &[u8] = b"semaprax.cxx-package.header.v1\0";
const PROVIDER_DOMAIN: &[u8] = b"semaprax.cxx-package.provider.v1\0";
const GUARD_DOMAIN: &[u8] = b"semaprax.cxx-package.guard.v1\0";
const MAX_CONSTRUCTION_BYTES: usize = crate::graph::MAX_AGENT_CONTEXT_BYTES;
const NONCLAIMS: [&str; 8] = [
    "no_aggregate_resource_or_string_mapping",
    "no_build_or_tool_authority",
    "no_cxx_exception_translation",
    "no_filesystem_write",
    "no_general_native_abi_stability",
    "no_package_publication",
    "no_reusable_runtime_context",
    "scalar_copy_values_only",
];

/// Independently verified, caller-materializable package bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CxxPackage {
    pub header: String,
    pub provider_c: String,
    pub shim_envelope: String,
}

/// Generate a canonical C++17 header and C11 provider without writing files or
/// selecting a compiler. Every requested function must pass Shim v1 admission.
pub fn generate_package(
    source_path: &Path,
    options: &CxxShimOptions,
) -> Result<String, Vec<Diagnostic>> {
    let (initial, initial_overflowed) = with_limit(MAX_CONSTRUCTION_BYTES, || {
        generate_internal_bounded(source_path, options, MAX_CONSTRUCTION_BYTES)
    });
    let initial = initial?;
    if initial_overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-X103",
            "cxx-package derivation exceeds its hard construction budget".to_owned(),
        )]);
    }
    if initial.excluded != 0 || initial.emitted.len() != options.functions.len() {
        return Err(vec![Diagnostic::io(
            "SPX-X106",
            "cxx-package requires every selected function to pass C++ Shim v1 admission".to_owned(),
        )]);
    }
    let canonical_options = CxxShimOptions::new(
        initial
            .emitted
            .iter()
            .map(|function| function.stable_id.clone())
            .collect(),
        options.max_bytes,
    )
    .map_err(|error| vec![error])?;
    let canonical_source = initial.canonical_source.as_deref().ok_or_else(|| {
        vec![consistency_error(
            "cxx-package did not retain its bounded canonical source".to_owned(),
        )]
    })?;
    let (seed, seed_overflowed) = with_limit(MAX_CONSTRUCTION_BYTES, || {
        generate_from_source(
            canonical_source,
            Path::new(&initial.source_path),
            &canonical_options,
            true,
        )
    });
    let seed = seed?;
    if seed_overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-X103",
            "cxx-package canonical replay exceeds its hard construction budget".to_owned(),
        )]);
    }
    if seed.excluded != 0 || seed.emitted.len() != options.functions.len() {
        return Err(vec![Diagnostic::io(
            "SPX-X106",
            "cxx-package requires every selected function to pass C++ Shim v1 admission".to_owned(),
        )]);
    }
    let native = seed.native_text.as_deref().ok_or_else(|| {
        vec![consistency_error(
            "cxx-package has no admitted native projection".to_owned(),
        )]
    })?;
    let canonical_source = seed.canonical_source.as_deref().ok_or_else(|| {
        vec![consistency_error(
            "cxx-package replay did not retain canonical source".to_owned(),
        )]
    })?;
    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        let guard = package_guard(&seed.revision, &seed.emitted);
        let header = render_header(&guard, &seed.emitted);
        let provider = render_provider(native, &seed.emitted);
        let nonclaims = NONCLAIMS
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .budgeted_join(",");
        let selection = seed
            .emitted
            .iter()
            .map(|function| quote_json(&function.stable_id))
            .collect::<Vec<_>>()
            .budgeted_join(",");
        let payload = crate::bounded_output::budgeted_format(format_args!(
            "{{\"header\":{{\"bytes\":{},\"sha256\":{},\"text\":{}}},\"nonclaims\":[{}],\"provider_c\":{{\"bytes\":{},\"sha256\":{},\"text\":{}}},\"revision\":{},\"schema\":\"{}\",\"selection\":[{}],\"shim_envelope\":{},\"source\":{{\"path\":{},\"text\":{}}}}}",
            header.len(),
            quote_json(&domain_digest(HEADER_DOMAIN, header.as_bytes())),
            quote_json(&header),
            nonclaims,
            provider.len(),
            quote_json(&domain_digest(PROVIDER_DOMAIN, provider.as_bytes())),
            quote_json(&provider),
            quote_json(&seed.revision),
            PACKAGE_SCHEMA,
            selection,
            quote_json(&seed.envelope),
            quote_json(&seed.source_path),
            quote_json(canonical_source),
        ));
        crate::bounded_output::budgeted_format(format_args!(
            "{{\"bytes\":{},\"digest\":{},\"payload\":{},\"schema\":\"{}\"}}",
            payload.len(),
            quote_json(&domain_digest(PAYLOAD_DOMAIN, payload.as_bytes())),
            payload,
            PACKAGE_SCHEMA
        ))
    });
    if overflowed {
        Err(vec![Diagnostic::io(
            "SPX-X103",
            "cxx-package output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )])
    } else {
        Ok(envelope)
    }
}

/// Verify a package against the caller's expected source and exact selection,
/// then return its materializable bytes. Embedded source is proof data, not
/// authority to substitute a different subject.
pub fn verify_package_envelope(
    envelope: &str,
    expected_source_path: &Path,
    expected_options: &CxxShimOptions,
) -> Result<CxxPackage, Diagnostic> {
    if envelope.len() > MAX_CONSTRUCTION_BYTES {
        return Err(consistency_error(
            "C++ package exceeds its hard verification byte bound".to_owned(),
        ));
    }
    let expected = generate_package(expected_source_path, expected_options).map_err(|_| {
        consistency_error(
            "expected C++ package source or selection does not generate exactly".to_owned(),
        )
    })?;
    if envelope != expected {
        return Err(consistency_error(
            "C++ package disagrees with the caller-authorized source or selection".to_owned(),
        ));
    }
    inspect_package_envelope(envelope)
}

fn inspect_package_envelope(envelope: &str) -> Result<CxxPackage, Diagnostic> {
    let (result, overflowed) = with_limit(MAX_CONSTRUCTION_BYTES, || {
        inspect_package_envelope_inner(envelope)
    });
    if overflowed {
        Err(consistency_error(
            "C++ package verification exceeds its hard construction budget".to_owned(),
        ))
    } else {
        result
    }
}

fn inspect_package_envelope_inner(envelope: &str) -> Result<CxxPackage, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("C++ package is not valid JSON: {error}")))?;
    if serde_json::to_string(&value).ok().as_deref() != Some(envelope) {
        return Err(consistency_error(
            "C++ package is not canonical compact JSON".to_owned(),
        ));
    }
    let object = exact_object(
        &value,
        &["bytes", "digest", "payload", "schema"],
        "package envelope",
    )?;
    if object["schema"] != PACKAGE_SCHEMA {
        return Err(consistency_error("C++ package schema disagrees".to_owned()));
    }
    let payload = &object["payload"];
    let payload_bytes = serde_json::to_string(payload)
        .map_err(|_| consistency_error("C++ package payload cannot be rendered".to_owned()))?;
    if object["bytes"].as_u64() != Some(payload_bytes.len() as u64)
        || object["digest"].as_str()
            != Some(domain_digest(PAYLOAD_DOMAIN, payload_bytes.as_bytes()).as_str())
    {
        return Err(consistency_error(
            "C++ package payload binding disagrees".to_owned(),
        ));
    }
    let payload = exact_object(
        payload,
        &[
            "header",
            "nonclaims",
            "provider_c",
            "revision",
            "schema",
            "selection",
            "shim_envelope",
            "source",
        ],
        "package payload",
    )?;
    if payload["schema"] != PACKAGE_SCHEMA || payload["nonclaims"] != serde_json::json!(NONCLAIMS) {
        return Err(consistency_error(
            "C++ package payload policy disagrees".to_owned(),
        ));
    }
    let header = artifact(payload.get("header"), HEADER_DOMAIN, "header")?;
    let provider_c = artifact(payload.get("provider_c"), PROVIDER_DOMAIN, "provider")?;
    let shim = payload["shim_envelope"]
        .as_str()
        .ok_or_else(|| consistency_error("shim envelope must be text".to_owned()))?;
    let fragment = super::verify_envelope(shim)?;
    let source = exact_object(&payload["source"], &["path", "text"], "package source")?;
    let source_path = source["path"]
        .as_str()
        .ok_or_else(|| consistency_error("package source path must be text".to_owned()))?;
    let source_text = source["text"]
        .as_str()
        .ok_or_else(|| consistency_error("package source must be text".to_owned()))?;
    let selection = payload["selection"]
        .as_array()
        .ok_or_else(|| consistency_error("package selection must be an array".to_owned()))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| consistency_error("package selection must contain text".to_owned()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shim_value: serde_json::Value = serde_json::from_str(shim)
        .map_err(|_| consistency_error("verified shim cannot be decoded".to_owned()))?;
    let max_bytes = shim_value["payload"]["limits"]["max_bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| consistency_error("shim limit is invalid".to_owned()))?;
    let options = CxxShimOptions::new(selection.clone(), max_bytes).map_err(|_| {
        consistency_error("package selection or construction limit is invalid".to_owned())
    })?;
    let (replayed, overflowed) = with_limit(max_bytes, || {
        generate_from_source(source_text, Path::new(source_path), &options, true)
    });
    let replayed = replayed.map_err(|_| {
        consistency_error("package source does not replay through C++ admission".to_owned())
    })?;
    if overflowed
        || replayed.excluded != 0
        || payload["revision"].as_str() != Some(&replayed.revision)
        || replayed.canonical_source.as_deref() != Some(source_text)
        || replayed.source_path != source_path
        || replayed.envelope != shim
    {
        return Err(consistency_error(
            "package source, selection, revision, or shim replay disagrees".to_owned(),
        ));
    }
    let replayed_selection = replayed
        .emitted
        .iter()
        .map(|function| function.stable_id.as_str())
        .collect::<Vec<_>>();
    if replayed_selection != selection.iter().map(String::as_str).collect::<Vec<_>>() {
        return Err(consistency_error(
            "package stable-ID inventory disagrees".to_owned(),
        ));
    }
    let native = replayed
        .native_text
        .as_deref()
        .ok_or_else(|| consistency_error("replayed package has no native projection".to_owned()))?;
    let expected_guard = package_guard(&replayed.revision, &replayed.emitted);
    if header != render_header(&expected_guard, &replayed.emitted)
        || provider_c != render_provider(native, &replayed.emitted)
    {
        return Err(consistency_error(
            "package generated artifacts do not match authoritative replay".to_owned(),
        ));
    }
    verify_wrapper_inventory(shim, &fragment, &header, &provider_c)?;
    Ok(CxxPackage {
        header,
        provider_c,
        shim_envelope: shim.to_owned(),
    })
}

fn artifact(
    value: Option<&serde_json::Value>,
    domain: &[u8],
    label: &str,
) -> Result<String, Diagnostic> {
    let value = value.ok_or_else(|| consistency_error(format!("missing {label} artifact")))?;
    let object = exact_object(value, &["bytes", "sha256", "text"], label)?;
    let text = object["text"]
        .as_str()
        .ok_or_else(|| consistency_error(format!("{label} text is not a string")))?;
    if object["bytes"].as_u64() != Some(text.len() as u64)
        || object["sha256"].as_str() != Some(domain_digest(domain, text.as_bytes()).as_str())
    {
        return Err(consistency_error(format!(
            "{label} byte or digest binding disagrees"
        )));
    }
    Ok(text.to_owned())
}

fn exact_object<'a>(
    value: &'a serde_json::Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Diagnostic> {
    let object = value
        .as_object()
        .ok_or_else(|| consistency_error(format!("{label} is not an object")))?;
    if object.len() != keys.len() || !keys.iter().all(|key| object.contains_key(*key)) {
        return Err(consistency_error(format!("{label} keys are not closed")));
    }
    Ok(object)
}

fn package_guard(revision: &str, functions: &[super::EmittedFunction]) -> String {
    let mut bytes = crate::bounded_output::CappedVec::new();
    bytes.extend_from_slice(revision.as_bytes());
    for function in functions {
        bytes.push(0);
        bytes.extend_from_slice(function.stable_id.as_bytes());
    }
    domain_digest(GUARD_DOMAIN, &bytes)
        .trim_start_matches("sha256:")
        .to_ascii_uppercase()
}

fn render_header(guard_digest: &str, functions: &[super::EmittedFunction]) -> String {
    let guard =
        crate::bounded_output::budgeted_format(format_args!("SPX_CXX_PACKAGE_{guard_digest}"));
    let mut out = crate::bounded_output::CappedString::new();
    out.push_str(&crate::bounded_output::budgeted_format(format_args!(
        "/* Generated by SEMAPRAX C++ Package v1; do not edit. */\n#ifndef {guard}\n#define {guard}\n\n#include <stdbool.h>\n#include <stdint.h>\n\n#ifdef __cplusplus\nextern \"C\" {{\n#endif\n\ntypedef uint32_t spx_cxx_status_v1;\n#define SPX_CXX_SUCCESS_V1 UINT32_C(0)\n#define SPX_CXX_SEMANTIC_FAILURE_V1 UINT32_C(1)\n#define SPX_CXX_ADAPTER_FAILURE_V1 UINT32_C(2)\n"
    )));
    for function in functions {
        out.push('\n');
        out.push_str(&wrapper_declaration(function));
        out.push('\n');
    }
    out.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n#endif\n");
    out.into_string()
}

fn render_provider(native: &str, functions: &[super::EmittedFunction]) -> String {
    let mut out = crate::bounded_output::CappedString::new();
    out.push_str("#define SPX_NO_ENTRY_WRAPPER 1\n");
    out.push_str(native);
    out.push_str("\ntypedef uint32_t spx_cxx_status_v1;\n#define SPX_CXX_SUCCESS_V1 UINT32_C(0)\n#define SPX_CXX_SEMANTIC_FAILURE_V1 UINT32_C(1)\n#define SPX_CXX_ADAPTER_FAILURE_V1 UINT32_C(2)\n");
    for function in functions {
        out.push('\n');
        out.push_str(&wrapper_definition(function));
        out.push('\n');
    }
    out.into_string()
}

fn wrapper_declaration(function: &super::EmittedFunction) -> String {
    crate::bounded_output::budgeted_format(format_args!(
        "spx_cxx_status_v1 {}({});",
        wrapper_symbol(&function.stable_id),
        wrapper_parameters(function)
    ))
}

fn wrapper_definition(function: &super::EmittedFunction) -> String {
    let arguments = (0..function.params.len())
        .map(|index| crate::bounded_output::budgeted_format(format_args!("spx_arg_{index}")))
        .collect::<Vec<_>>()
        .budgeted_join(", ");
    let mut scalar_validation = crate::bounded_output::CappedString::new();
    for (index, ty) in function.params.iter().enumerate() {
        if matches!(ty, Type::Char) {
            scalar_validation.push_str(&crate::bounded_output::budgeted_format(format_args!(
                "    if (spx_arg_{index} > UINT32_C(0x10ffff) || (spx_arg_{index} >= UINT32_C(0xd800) && spx_arg_{index} <= UINT32_C(0xdfff))) return SPX_CXX_ADAPTER_FAILURE_V1;\n"
            )));
        }
    }
    let scalar_validation = scalar_validation.into_string();
    let comma = if arguments.is_empty() { "" } else { ", " };
    crate::bounded_output::budgeted_format(format_args!(
        "spx_cxx_status_v1 {}({}) {{\n    if (spx_result_out == NULL) return SPX_CXX_ADAPTER_FAILURE_V1;\n{}    struct spx_status_entry spx_status_entries[UINT32_C(1)];\n    struct spx_context spx_context = {{0}};\n    if (!spx_context_init(&spx_context, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, NULL)) return SPX_CXX_ADAPTER_FAILURE_V1;\n    spx_status_token spx_status = {}(&spx_context, {}{}spx_result_out);\n    return spx_status == SPX_STATUS_SUCCESS ? SPX_CXX_SUCCESS_V1 : SPX_CXX_SEMANTIC_FAILURE_V1;\n}}",
        wrapper_symbol(&function.stable_id),
        wrapper_parameters(function),
        scalar_validation,
        function.symbol,
        arguments,
        comma,
    ))
}

fn wrapper_parameters(function: &super::EmittedFunction) -> String {
    let mut parameters = function
        .params
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            crate::bounded_output::budgeted_format(format_args!("{} spx_arg_{index}", c_type(ty)))
        })
        .collect::<Vec<_>>();
    parameters.push(crate::bounded_output::budgeted_format(format_args!(
        "{} *spx_result_out",
        c_type(&function.result)
    )));
    parameters.budgeted_join(", ")
}

fn c_type(ty: &Type) -> &'static str {
    match ty {
        Type::I64 => "int64_t",
        Type::I32 => "int32_t",
        Type::U8 => "uint8_t",
        Type::Char => "uint32_t",
        Type::F32 => "float",
        Type::F64 => "double",
        Type::Bool => "bool",
        _ => unreachable!("C++ package receives only Shim-v1 admitted scalar types"),
    }
}

fn wrapper_symbol(stable_id: &str) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("spx_cxx_call_");
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output.into_string()
}

fn verify_wrapper_inventory(
    shim: &str,
    fragment: &str,
    header: &str,
    provider: &str,
) -> Result<(), Diagnostic> {
    let shim: serde_json::Value = serde_json::from_str(shim)
        .map_err(|_| consistency_error("verified shim cannot be decoded".to_owned()))?;
    let functions = shim["payload"]["functions"].as_array().ok_or_else(|| {
        consistency_error("verified shim function inventory is invalid".to_owned())
    })?;
    if functions.is_empty() {
        return Err(consistency_error(
            "C++ package cannot have an empty function inventory".to_owned(),
        ));
    }
    for function in functions {
        let stable_id = function["stable_id"]
            .as_str()
            .ok_or_else(|| consistency_error("shim stable ID is invalid".to_owned()))?;
        let symbol = function["symbol"]
            .as_str()
            .ok_or_else(|| consistency_error("shim symbol is invalid".to_owned()))?;
        let wrapper = wrapper_symbol(stable_id);
        if fragment.matches(symbol).count() != 1
            || header.matches(&wrapper).count() != 1
            || provider
                .matches(&format!("spx_cxx_status_v1 {wrapper}("))
                .count()
                != 1
            || provider.matches(&format!("{symbol}(&spx_context,")).count() != 1
        {
            return Err(consistency_error(format!(
                "C++ wrapper inventory disagrees for `{stable_id}`"
            )));
        }
    }
    Ok(())
}
