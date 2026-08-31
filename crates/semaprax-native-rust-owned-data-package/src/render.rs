use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_json::Value;

use super::descriptor::{json_string, Descriptor, ParameterKind, ResultKind};
use super::{
    descriptor_digest_for_schema, raw_sha256, HostTarget, PackageError, PackageMode,
    NATIVE_RUST_OWNED_DATA_SDK_SCHEMA, NATIVE_RUST_OWNED_UTF8_SDK_SCHEMA, OWNED_CRATE_NAME,
    OWNED_CRATE_VERSION,
};

pub(crate) struct Sources {
    pub(crate) cargo_toml: String,
    pub(crate) build_rs: String,
    pub(crate) lib_rs: String,
    pub(crate) ffi_rs: String,
}

pub(crate) fn render_sources(
    descriptor: &Descriptor,
    target: HostTarget,
    mode: PackageMode,
) -> Sources {
    let cargo_toml = format!(
        "[package]\nname = \"{OWNED_CRATE_NAME}\"\nversion = \"{OWNED_CRATE_VERSION}\"\nedition = \"2021\"\nrust-version = \"1.85\"\npublish = false\nbuild = \"build.rs\"\n\n[lib]\npath = \"lib.rs\"\n\n[workspace]\n"
    );
    let build_rs = super::build_script::render(target, false);
    Sources {
        cargo_toml,
        build_rs,
        lib_rs: render_lib(descriptor),
        ffi_rs: render_ffi(descriptor, mode),
    }
}

fn render_lib(descriptor: &Descriptor) -> String {
    let mut output = String::from(
        "#[path=\"owned_data_ffi.rs\"]mod ffi;\nmod safe_api{#![forbid(unsafe_code)]\nuse super::ffi;\n#[derive(Clone,Copy,Debug,Eq,PartialEq)]pub enum CallError{SemanticFailure,AdapterRejected,HostFailure}\npub struct NativeRustOwnedDataSdk{context:ffi::Context}\nimpl NativeRustOwnedDataSdk{pub fn new()->Result<Self,CallError>{Ok(Self{context:ffi::Context::new().map_err(Self::map_failure)?})}\n",
    );
    output.push_str("fn map_failure(value:ffi::Failure)->CallError{match value{ffi::Failure::Semantic=>CallError::SemanticFailure,ffi::Failure::Adapter=>CallError::AdapterRejected,ffi::Failure::Host=>CallError::HostFailure}}\n");
    for export in &descriptor.exports {
        write!(output, "pub fn {}(&mut self", export.rust_method_name).unwrap();
        for (index, parameter) in export.parameters.iter().enumerate() {
            write!(output, ",arg_{index}:{}", rust_parameter(parameter.kind)).unwrap();
        }
        write!(
            output,
            ")->Result<{},CallError>{{self.context.invoke(|context|{{",
            rust_result(export.result)
        )
        .unwrap();
        match export.result {
            ResultKind::I64 | ResultKind::Bool | ResultKind::Usize => {
                write!(output, "context.call_{}(", export.rust_method_name).unwrap();
                render_arguments(&mut output, export.parameters.len());
                output.push_str(").map_err(Self::map_failure)");
            }
            ResultKind::OwnedBytes
            | ResultKind::OptionOwnedBytes
            | ResultKind::ResultOwnedBytesI64
            | ResultKind::OwnedUtf8 => {
                write!(output, "let raw=context.call_{}(", export.rust_method_name).unwrap();
                render_arguments(&mut output, export.parameters.len());
                output.push_str(").map_err(Self::map_failure)?;");
                match export.result {
                    ResultKind::OwnedBytes => output.push_str("context.copy_and_settle(raw.handle).map_err(map_failure)"),
                    ResultKind::OptionOwnedBytes => output.push_str("if raw.tag==0{Ok(None)}else{context.copy_and_settle(raw.handle).map(Some).map_err(map_failure)}"),
                    ResultKind::ResultOwnedBytesI64 => output.push_str("if raw.tag==1{Ok(Err(raw.error))}else{context.copy_and_settle(raw.handle).map(Ok).map_err(map_failure)}"),
                    ResultKind::OwnedUtf8 => output.push_str("let bytes=context.copy_and_settle(raw.handle).map_err(map_failure)?;String::from_utf8(bytes).map_err(|_|CallError::AdapterRejected)"),
                    _ => unreachable!("owned result branch"),
                }
            }
        }
        output.push_str("}).map_err(Self::map_failure)?}\n");
    }
    output.push_str("}\n}\npub use safe_api::*;\n");
    output.replace("map_err(map_failure)", "map_err(Self::map_failure)")
}

fn render_arguments(output: &mut String, count: usize) {
    for index in 0..count {
        if index != 0 {
            output.push(',');
        }
        write!(output, "arg_{index}").unwrap();
    }
}

fn render_ffi(descriptor: &Descriptor, mode: PackageMode) -> String {
    let has_owned_result = descriptor.exports.iter().any(|export| {
        matches!(
            export.result,
            ResultKind::OwnedBytes
                | ResultKind::OptionOwnedBytes
                | ResultKind::ResultOwnedBytesI64
                | ResultKind::OwnedUtf8
        )
    });
    let mut output = String::from(
        "#![allow(unsafe_code)]\nuse core::marker::PhantomData;use core::ptr::NonNull;use std::rc::Rc;\n#[repr(C)]struct RawContext{_private:[u8;0]}\n",
    );
    if has_owned_result {
        output.push_str("type Handle=u64;");
    }
    output.push_str("type Status=u32;\nextern \"C\"{fn spx_owned_data_context_size_v1()->u64;fn spx_owned_data_context_align_v1()->u64;fn spx_owned_data_context_init_v1(storage:*mut core::ffi::c_void,length:u64)->Status;fn spx_owned_data_context_drop_v1(context:*mut RawContext)->Status;");
    if has_owned_result {
        output.push_str("fn spx_owned_bytes_len_v1(context:*mut RawContext,handle:Handle,length:*mut u64)->Status;fn spx_owned_bytes_copy_v1(context:*mut RawContext,handle:Handle,destination:*mut u8,length:u64)->Status;fn spx_owned_bytes_drop_v1(context:*mut RawContext,handle:Handle)->Status;");
    }
    output.push('\n');
    for export in &descriptor.exports {
        write!(
            output,
            "#[link_name={:?}]fn raw_{}(context:*mut RawContext",
            provider_symbol(&export.rust_method_name),
            export.rust_method_name
        )
        .unwrap();
        render_raw_parameters(&mut output, &export.parameters);
        match export.result {
            ResultKind::I64 => output.push_str(",value:*mut i64)->Status;\n"),
            ResultKind::Bool => output.push_str(",value:*mut u8)->Status;\n"),
            ResultKind::Usize => output.push_str(",value:*mut u64)->Status;\n"),
            ResultKind::OwnedBytes
            | ResultKind::OptionOwnedBytes
            | ResultKind::ResultOwnedBytesI64
            | ResultKind::OwnedUtf8 => {
                output.push_str(",tag:*mut u32,handle:*mut Handle,error:*mut i64)->Status;\n")
            }
        }
    }
    output.push_str("}\n#[derive(Clone,Copy)]pub(super)enum Failure{Semantic,Adapter,Host}\n");
    if has_owned_result {
        output.push_str("pub(super)struct RawCall{pub tag:u32,pub handle:Handle,pub error:i64}\n");
    }
    output.push_str(super::owned_ffi_runtime::CONTEXT);
    for export in &descriptor.exports {
        write!(output, "pub fn call_{}(&mut self", export.rust_method_name).unwrap();
        for (index, parameter) in export.parameters.iter().enumerate() {
            write!(output, ",arg_{index}:{}", rust_parameter(parameter.kind)).unwrap();
        }
        match export.result {
            ResultKind::I64 => output.push_str(")->Result<i64,Failure>{let mut value=0i64;let status=unsafe{raw_"),
            ResultKind::Bool => output.push_str(")->Result<bool,Failure>{let mut value=u8::MAX;let status=unsafe{raw_"),
            ResultKind::Usize => output.push_str(")->Result<u64,Failure>{let mut value=0u64;let status=unsafe{raw_"),
            ResultKind::OwnedBytes | ResultKind::OptionOwnedBytes | ResultKind::ResultOwnedBytesI64 | ResultKind::OwnedUtf8 => output.push_str(")->Result<RawCall,Failure>{let mut value=RawCall{tag:u32::MAX,handle:0,error:0};let status=unsafe{raw_"),
        }
        output.push_str(&export.rust_method_name);
        output.push_str("(self.raw.as_ptr()");
        render_raw_arguments(&mut output, &export.parameters);
        match export.result {
            ResultKind::I64 | ResultKind::Bool | ResultKind::Usize => {
                output.push_str(",&mut value)};match status{0=>");
                match export.result {
                    ResultKind::I64 => output.push_str("Ok(value)"),
                    ResultKind::Bool => {
                        output.push_str("if value<=1{Ok(value!=0)}else{Err(Failure::Adapter)}")
                    }
                    ResultKind::Usize => output.push_str("Ok(value)"),
                    _ => unreachable!("scalar result branch"),
                }
                output.push_str(",1=>Err(Failure::Semantic),2..=5=>Err(Failure::Adapter),_=>Err(Failure::Host)}}\n");
            }
            ResultKind::OwnedBytes
            | ResultKind::OptionOwnedBytes
            | ResultKind::ResultOwnedBytesI64
            | ResultKind::OwnedUtf8 => {
                output.push_str(",&mut value.tag,&mut value.handle,&mut value.error)};if status!=0{if value.tag!=u32::MAX||value.handle!=0||value.error!=0{std::process::abort()}return match status{1=>Err(Failure::Semantic),2..=5=>Err(Failure::Adapter),_=>Err(Failure::Host)}}");
                match export.result {
                    ResultKind::OwnedBytes | ResultKind::OwnedUtf8 => output.push_str("if value.tag!=0{std::process::abort()}if value.handle==0{return Err(Failure::Adapter)}if value.error!=0{self.discard(value.handle)?;return Err(Failure::Adapter)}"),
                    ResultKind::OptionOwnedBytes => output.push_str("if value.tag>1||(value.tag==0&&value.handle!=0){std::process::abort()}if value.tag==1&&value.handle==0{return Err(Failure::Adapter)}if value.error!=0{if value.tag==1{self.discard(value.handle)?}return Err(Failure::Adapter)}"),
                    ResultKind::ResultOwnedBytesI64 => output.push_str("if value.tag>1||(value.tag==1&&value.handle!=0){std::process::abort()}if value.tag==0{if value.handle==0{return Err(Failure::Adapter)}if value.error!=0{self.discard(value.handle)?;return Err(Failure::Adapter)}}"),
                    _ => unreachable!("owned result branch"),
                }
                output.push_str("Ok(value)}\n");
            }
        }
    }
    if has_owned_result {
        // Every owned carrier has a malformed-success path that discards its
        // live result; scalar-only selections have no such owner operations.
        super::owned_ffi_runtime::append_owner_operations(
            &mut output,
            mode == PackageMode::StandaloneEvidence,
            true,
        );
    } else {
        output.push_str("}\n");
    }
    output.push_str(super::owned_ffi_runtime::INVOCATION);
    output
}

fn render_raw_parameters(output: &mut String, parameters: &[super::descriptor::Parameter]) {
    for (index, parameter) in parameters.iter().enumerate() {
        match parameter.kind {
            ParameterKind::I64 => write!(output, ",arg_{index}:i64"),
            ParameterKind::Bool => write!(output, ",arg_{index}:u8"),
            ParameterKind::BorrowStr | ParameterKind::BorrowSliceU8 => {
                write!(output, ",arg_{index}:*const u8,arg_{index}_len:u64")
            }
        }
        .unwrap();
    }
}

fn render_raw_arguments(output: &mut String, parameters: &[super::descriptor::Parameter]) {
    for (index, parameter) in parameters.iter().enumerate() {
        match parameter.kind {
            ParameterKind::I64 => write!(output, ",arg_{index}"),
            ParameterKind::Bool => write!(output, ",u8::from(arg_{index})"),
            ParameterKind::BorrowStr | ParameterKind::BorrowSliceU8 => {
                write!(output, ",arg_{index}.as_ptr(),arg_{index}.len()as u64")
            }
        }
        .unwrap();
    }
}

fn rust_parameter(kind: ParameterKind) -> &'static str {
    match kind {
        ParameterKind::I64 => "i64",
        ParameterKind::Bool => "bool",
        ParameterKind::BorrowStr => "&str",
        ParameterKind::BorrowSliceU8 => "&[u8]",
    }
}

fn rust_result(kind: ResultKind) -> &'static str {
    match kind {
        ResultKind::I64 => "i64",
        ResultKind::Bool => "bool",
        ResultKind::Usize => "u64",
        ResultKind::OwnedBytes => "Vec<u8>",
        ResultKind::OptionOwnedBytes => "Option<Vec<u8>>",
        ResultKind::ResultOwnedBytesI64 => "Result<Vec<u8>,i64>",
        ResultKind::OwnedUtf8 => "String",
    }
}

fn provider_symbol(method: &str) -> String {
    format!("spx_owned_data_call_{method}_v1")
}

pub(crate) fn render_manifest(
    target: HostTarget,
    descriptor: &[u8],
    descriptor_digest: &str,
    archive_name: &str,
    mode: PackageMode,
    provider_sha256: &str,
    mut files: [(&str, &[u8]); 6],
) -> String {
    files.sort_by_key(|row| row.0.as_bytes());
    let mut output = String::new();
    output.push_str("{\"schema\":");
    json_string(&mut output, manifest_schema(mode));
    output.push_str(",\"crate\":{\"name\":");
    json_string(&mut output, OWNED_CRATE_NAME);
    output.push_str(",\"version\":");
    json_string(&mut output, OWNED_CRATE_VERSION);
    output.push('}');
    output.push_str(",\"target\":");
    json_string(&mut output, target.triple());
    output.push_str(",\"descriptor\":{\"schema\":");
    json_string(&mut output, descriptor_schema(mode));
    write!(output, ",\"bytes\":{},\"digest\":", descriptor.len()).unwrap();
    json_string(&mut output, descriptor_digest);
    output.push('}');
    output.push_str(",\"provider\":{\"abi\":\"opaque-handle.v1\",\"archive\":");
    json_string(&mut output, archive_name);
    if descriptor_bound(mode) {
        output.push_str(",\"descriptor_digest\":");
        json_string(&mut output, descriptor_digest);
        output.push_str(",\"source_sha256\":");
        json_string(&mut output, provider_sha256);
    }
    output.push_str(",\"operations\":[\"len\",\"copy\",\"drop\"]}");
    output.push_str(",\"files\":[");
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"path\":");
        json_string(&mut output, path);
        write!(output, ",\"bytes\":{},\"sha256\":", bytes.len()).unwrap();
        json_string(&mut output, &raw_sha256(bytes));
        output.push('}');
    }
    output.push(']');
    output.push_str(",\"limits\":{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}");
    output.push_str(",\"nonclaims\":[\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\"");
    if mode == PackageMode::StandaloneEvidence {
        output.push_str(",\"no_project_v8_activation\"");
    }
    output.push_str("]}\n");
    output
}

#[allow(clippy::too_many_arguments, reason = "manifest inputs remain explicit")]
pub(crate) fn verify_manifest(
    bytes: &[u8],
    target: HostTarget,
    expected_descriptor: &[u8],
    expected_descriptor_digest: &str,
    archive_name: &str,
    mode: PackageMode,
    provider_sha256: &str,
    files: [(&str, &[u8]); 6],
) -> Result<(), PackageError> {
    if !bytes.ends_with(b"\n")
        || descriptor_digest_for_schema(descriptor_schema(mode), expected_descriptor).as_deref()
            != Some(expected_descriptor_digest)
    {
        return Err(PackageError::descriptor());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PackageError::descriptor())?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 8)
        .ok_or_else(PackageError::descriptor)?;
    if root.get("schema").and_then(Value::as_str) != Some(manifest_schema(mode))
        || root.get("target").and_then(Value::as_str) != Some(target.triple())
    {
        return Err(PackageError::descriptor());
    }
    let crate_row = closed(root.get("crate"), 2)?;
    let descriptor_row = closed(root.get("descriptor"), 3)?;
    let provider = closed(
        root.get("provider"),
        if descriptor_bound(mode) { 5 } else { 3 },
    )?;
    let limits = closed(root.get("limits"), 4)?;
    if crate_row.get("name").and_then(Value::as_str) != Some(OWNED_CRATE_NAME)
        || crate_row.get("version").and_then(Value::as_str) != Some(OWNED_CRATE_VERSION)
        || descriptor_row.get("schema").and_then(Value::as_str) != Some(descriptor_schema(mode))
        || descriptor_row.get("bytes").and_then(Value::as_u64)
            != u64::try_from(expected_descriptor.len()).ok()
        || descriptor_row.get("digest").and_then(Value::as_str) != Some(expected_descriptor_digest)
        || provider.get("abi").and_then(Value::as_str) != Some("opaque-handle.v1")
        || provider.get("archive").and_then(Value::as_str) != Some(archive_name)
        || !exact_strings(
            provider.get("operations").and_then(Value::as_array),
            &["len", "copy", "drop"],
        )
    {
        return Err(PackageError::descriptor());
    }
    if descriptor_bound(mode)
        && (provider.get("descriptor_digest").and_then(Value::as_str)
            != Some(expected_descriptor_digest)
            || provider.get("source_sha256").and_then(Value::as_str) != Some(provider_sha256))
    {
        return Err(PackageError::descriptor());
    }
    if limits
        .get("max_borrowed_input_bytes")
        .and_then(Value::as_u64)
        != Some(65_536)
        || limits.get("max_owned_output_bytes").and_then(Value::as_u64) != Some(65_536)
        || limits.get("max_handles").and_then(Value::as_u64) != Some(4_096)
        || limits.get("exact_package_files").and_then(Value::as_u64) != Some(7)
    {
        return Err(PackageError::descriptor());
    }
    let expected_nonclaims: &[&str] = match mode {
        PackageMode::StandaloneEvidence => &[
            "no_raw_handle_or_context_public_api",
            "no_allocator_transfer",
            "no_allocator_oom_abort_or_panic_recovery_proof",
            "no_send_sync",
            "no_project_v8_activation",
        ],
        PackageMode::ProjectV8 => &[
            "no_raw_handle_or_context_public_api",
            "no_allocator_transfer",
            "no_allocator_oom_abort_or_panic_recovery_proof",
            "no_send_sync",
        ],
        PackageMode::ProjectV9FlatRecord => &[
            "no_raw_handle_or_context_public_api",
            "no_allocator_transfer",
            "no_allocator_oom_abort_or_panic_recovery_proof",
            "no_send_sync",
        ],
        PackageMode::ProjectV10OwnedUtf8 => &[
            "no_raw_handle_or_context_public_api",
            "no_allocator_transfer",
            "no_allocator_oom_abort_or_panic_recovery_proof",
            "no_send_sync",
        ],
    };
    if !exact_strings(
        root.get("nonclaims").and_then(Value::as_array),
        expected_nonclaims,
    ) {
        return Err(PackageError::descriptor());
    }
    let rows = root
        .get("files")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == files.len())
        .ok_or_else(PackageError::descriptor)?;
    let expected_names = [
        "Cargo.toml",
        "build.rs",
        "lib.rs",
        "owned_data_ffi.rs",
        archive_name,
        "descriptor.json",
    ];
    if expected_names.iter().any(|name| {
        files
            .iter()
            .filter(|(candidate, _)| candidate == name)
            .count()
            != 1
    }) || files
        .iter()
        .find(|(name, _)| *name == "descriptor.json")
        .is_none_or(|(_, bytes)| *bytes != expected_descriptor)
    {
        return Err(PackageError::descriptor());
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        let row = row
            .as_object()
            .filter(|row| row.len() == 3)
            .ok_or_else(PackageError::descriptor)?;
        let path = row
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| seen.insert(*path))
            .ok_or_else(PackageError::descriptor)?;
        let expected = files
            .iter()
            .find(|(candidate, _)| *candidate == path)
            .map(|(_, bytes)| *bytes)
            .ok_or_else(PackageError::descriptor)?;
        if row.get("bytes").and_then(Value::as_u64) != u64::try_from(expected.len()).ok()
            || row.get("sha256").and_then(Value::as_str) != Some(raw_sha256(expected).as_str())
        {
            return Err(PackageError::descriptor());
        }
    }
    Ok(())
}

const fn descriptor_bound(mode: PackageMode) -> bool {
    matches!(
        mode,
        PackageMode::ProjectV8 | PackageMode::ProjectV10OwnedUtf8
    )
}

const fn manifest_schema(mode: PackageMode) -> &'static str {
    match mode {
        PackageMode::ProjectV10OwnedUtf8 => NATIVE_RUST_OWNED_UTF8_SDK_SCHEMA,
        _ => NATIVE_RUST_OWNED_DATA_SDK_SCHEMA,
    }
}

const fn descriptor_schema(mode: PackageMode) -> &'static str {
    match mode {
        PackageMode::ProjectV10OwnedUtf8 => super::PUBLIC_OWNED_UTF8_API_SCHEMA,
        _ => super::PUBLIC_OWNED_DATA_API_SCHEMA,
    }
}

fn closed(
    value: Option<&Value>,
    length: usize,
) -> Result<&serde_json::Map<String, Value>, PackageError> {
    value
        .and_then(Value::as_object)
        .filter(|row| row.len() == length)
        .ok_or_else(PackageError::descriptor)
}

fn exact_strings(values: Option<&Vec<Value>>, expected: &[&str]) -> bool {
    values.is_some_and(|values| {
        values.len() == expected.len()
            && expected.iter().all(|expected| {
                values
                    .iter()
                    .filter(|value| value.as_str() == Some(expected))
                    .count()
                    == 1
            })
    })
}
