use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::Value;

use super::descriptor::ParameterKind;
use super::nested_descriptor::{Descriptor, Export, FieldType, Record, API_SCHEMA};
use super::{raw_sha256, HostTarget, PackageError, OWNED_CRATE_NAME, OWNED_CRATE_VERSION};

pub(crate) const MANIFEST_SCHEMA: &str = "semaprax.native-rust-nested-owned-record-sdk.v1";

pub(crate) struct Sources {
    pub(crate) cargo_toml: String,
    pub(crate) build_rs: String,
    pub(crate) lib_rs: String,
    pub(crate) ffi_rs: String,
}

pub(crate) fn render_sources(descriptor: &Descriptor, target: HostTarget) -> Sources {
    Sources{cargo_toml:format!("[package]\nname = \"{OWNED_CRATE_NAME}\"\nversion = \"{OWNED_CRATE_VERSION}\"\nedition = \"2021\"\nrust-version = \"1.85\"\npublish = false\nbuild = \"build.rs\"\n\n[lib]\npath = \"lib.rs\"\n\n[workspace]\n"),build_rs:super::build_script::render_nested(target),lib_rs:render_lib(descriptor),ffi_rs:render_ffi(descriptor)}
}

fn render_lib(descriptor: &Descriptor) -> String {
    let mut out=String::from("#[path=\"owned_data_ffi.rs\"]mod ffi;\nmod safe_api{#![forbid(unsafe_code)]\nuse super::ffi;\n#[derive(Clone,Debug,Eq,PartialEq)]pub enum CallError{SemanticFailure,AdapterRejected,HostFailure}\n");
    for record in &descriptor.records {
        writeln!(
            out,
            "#[derive(Clone,Debug,Eq,PartialEq)]pub struct {}{{",
            record.host_name
        )
        .unwrap();
        for field in &record.fields {
            writeln!(
                out,
                "pub {}:{},",
                field.host_name,
                rust_field(&field.ty, descriptor)
            )
            .unwrap();
        }
        out.push_str("}\n");
    }
    out.push_str("pub struct NativeRustOwnedDataSdk{context:ffi::Context}\nimpl NativeRustOwnedDataSdk{pub fn new()->Result<Self,CallError>{Ok(Self{context:ffi::Context::new().map_err(Self::map_failure)?})}\nfn map_failure(value:ffi::Failure)->CallError{match value{ffi::Failure::Semantic=>CallError::SemanticFailure,ffi::Failure::Adapter=>CallError::AdapterRejected,ffi::Failure::Host=>CallError::HostFailure}}\n");
    let records = descriptor
        .records
        .iter()
        .map(|r| (r.stable_id.as_str(), r))
        .collect::<BTreeMap<_, _>>();
    for export in &descriptor.exports {
        write!(out, "pub fn {}(&mut self", export.rust_method_name).unwrap();
        parameters(&mut out, export);
        write!(
            out,
            ")->Result<{},CallError>{{self.context.invoke(|context|{{let raw=context.call_{}(",
            records[export.result_record_id.as_str()].host_name,
            export.rust_method_name
        )
        .unwrap();
        arguments(&mut out, export.parameters.len());
        out.push_str(").map_err(Self::map_failure)?;let handles=[");
        for leaf in export
            .leaves
            .iter()
            .filter(|l| l.ty == FieldType::OwnedBytes)
        {
            write!(out, "raw.slots[{}],", leaf.ordinal).unwrap()
        }
        out.push_str("];let mut owned=context.copy_many_and_settle(&handles).map_err(Self::map_failure)?.into_iter();Ok(");
        render_record_value(
            &mut out,
            &records,
            &export.result_record_id,
            &mut Vec::new(),
            export,
        );
        out.push_str(")}).map_err(Self::map_failure)?}\n");
    }
    out.push_str("}\n}\npub use safe_api::*;\n");
    out
}

fn render_record_value(
    out: &mut String,
    records: &BTreeMap<&str, &Record>,
    record_id: &str,
    path: &mut Vec<String>,
    export: &Export,
) {
    let record = records[record_id];
    write!(out, "{}{{", record.host_name).unwrap();
    for field in &record.fields {
        write!(out, "{}:", field.host_name).unwrap();
        path.push(field.stable_id.clone());
        match &field.ty {
            FieldType::Record(child) => render_record_value(out, records, child, path, export),
            FieldType::OwnedBytes => {
                out.push_str("owned.next().expect(\"descriptor owns exact payload inventory\")")
            }
            FieldType::I64 | FieldType::Usize | FieldType::Bool => {
                let leaf = export
                    .leaves
                    .iter()
                    .find(|leaf| leaf.path == *path)
                    .expect("replay proves leaf path");
                match field.ty {
                    FieldType::I64 => write!(
                        out,
                        "i64::from_ne_bytes(raw.slots[{}].to_ne_bytes())",
                        leaf.ordinal
                    )
                    .unwrap(),
                    FieldType::Usize => write!(
                        out,
                        "usize::try_from(raw.slots[{}]).map_err(|_|CallError::AdapterRejected)?",
                        leaf.ordinal
                    )
                    .unwrap(),
                    FieldType::Bool => write!(out, "raw.slots[{}]!=0", leaf.ordinal).unwrap(),
                    _ => unreachable!(),
                }
            }
        }
        path.pop();
        out.push(',')
    }
    out.push('}')
}

fn render_ffi(descriptor: &Descriptor) -> String {
    let mut out=String::from("#![allow(unsafe_code)]\nuse core::marker::PhantomData;use core::ptr::NonNull;use std::rc::Rc;\n#[repr(C)]struct RawContext{_private:[u8;0]}\ntype Handle=u64;type Status=u32;\nextern \"C\"{fn spx_owned_data_context_size_v1()->u64;fn spx_owned_data_context_align_v1()->u64;fn spx_owned_data_context_init_v1(storage:*mut core::ffi::c_void,length:u64)->Status;fn spx_owned_data_context_drop_v1(context:*mut RawContext)->Status;fn spx_owned_bytes_len_v1(context:*mut RawContext,handle:Handle,length:*mut u64)->Status;fn spx_owned_bytes_copy_v1(context:*mut RawContext,handle:Handle,destination:*mut u8,length:u64)->Status;fn spx_owned_bytes_drop_v1(context:*mut RawContext,handle:Handle)->Status;\n");
    for export in &descriptor.exports {
        write!(
            out,
            "#[link_name={:?}]fn raw_{}(context:*mut RawContext",
            provider_symbol(&export.rust_method_name),
            export.rust_method_name
        )
        .unwrap();
        raw_parameters(&mut out, export);
        writeln!(
            out,
            ",record:*mut RawRecord{})->Status;",
            export.leaves.len()
        )
        .unwrap();
    }
    out.push_str("}\n#[derive(Clone,Copy)]pub(super)enum Failure{Semantic,Adapter,Host}\n");
    for count in descriptor
        .exports
        .iter()
        .map(|e| e.leaves.len())
        .collect::<std::collections::BTreeSet<_>>()
    {
        writeln!(
            out,
            "#[repr(C)]pub(super)struct RawRecord{count}{{pub slots:[u64;{count}]}}"
        )
        .unwrap()
    }
    out.push_str(super::owned_ffi_runtime::CONTEXT);
    for export in &descriptor.exports {
        write!(out, "pub fn call_{}(&mut self", export.rust_method_name).unwrap();
        parameters(&mut out, export);
        write!(out,")->Result<RawRecord{},Failure>{{let mut value=RawRecord{}{{slots:[u64::MAX;{}]}};let status=unsafe{{raw_{}(self.raw.as_ptr()",export.leaves.len(),export.leaves.len(),export.leaves.len(),export.rust_method_name).unwrap();
        raw_arguments(&mut out, export);
        out.push_str(",&mut value)};if status!=0&&value.slots.iter().any(|slot|*slot!=u64::MAX){std::process::abort()}");
        out.push_str("if status==0{");
        for leaf in &export.leaves {
            match leaf.ty {
                FieldType::OwnedBytes => write!(
                    out,
                    "if value.slots[{}]==0||value.slots[{}]==u64::MAX{{std::process::abort()}}",
                    leaf.ordinal, leaf.ordinal
                )
                .unwrap(),
                FieldType::Bool => write!(
                    out,
                    "if value.slots[{}]>1{{std::process::abort()}}",
                    leaf.ordinal
                )
                .unwrap(),
                _ => {}
            }
        }
        let owned = export
            .leaves
            .iter()
            .filter(|l| l.ty == FieldType::OwnedBytes)
            .collect::<Vec<_>>();
        for (i, left) in owned.iter().enumerate() {
            for right in &owned[..i] {
                write!(
                    out,
                    "if value.slots[{}]==value.slots[{}]{{std::process::abort()}}",
                    left.ordinal, right.ordinal
                )
                .unwrap()
            }
        }
        out.push_str("Ok(value)}else{match status{1=>Err(Failure::Semantic),2..=5=>Err(Failure::Adapter),_=>Err(Failure::Host)}}}\n");
    }
    super::owned_ffi_runtime::append_multi_owner_operations(&mut out);
    out.push_str(super::owned_ffi_runtime::INVOCATION);
    out
}

pub(crate) fn render_manifest(
    target: HostTarget,
    descriptor: &[u8],
    digest: &str,
    archive: &str,
    provider_sha: &str,
    mut files: [(&str, &[u8]); 6],
) -> String {
    files.sort_by_key(|r| r.0.as_bytes());
    let rows = files
        .iter()
        .map(|(path, bytes)| {
            format!(
                "{{\"path\":{},\"bytes\":{},\"sha256\":{}}}",
                serde_json::to_string(path).unwrap(),
                bytes.len(),
                serde_json::to_string(&raw_sha256(bytes)).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"schema\":\"{MANIFEST_SCHEMA}\",\"crate\":{{\"name\":\"{OWNED_CRATE_NAME}\",\"version\":\"{OWNED_CRATE_VERSION}\"}},\"target\":{},\"descriptor\":{{\"schema\":\"{API_SCHEMA}\",\"bytes\":{},\"digest\":{}}},\"provider\":{{\"abi\":\"opaque-multi-handle-plus-scalars.v1\",\"archive\":{},\"descriptor_digest\":{},\"source_sha256\":{},\"operations\":[\"len\",\"copy\",\"drop\"]}},\"files\":[{rows}],\"limits\":{{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}},\"nonclaims\":[\"lower_does_not_authenticate_provider_semantics\",\"no_public_aggregate_abi\",\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\"]}}\n",serde_json::to_string(target.triple()).unwrap(),descriptor.len(),serde_json::to_string(digest).unwrap(),serde_json::to_string(archive).unwrap(),serde_json::to_string(digest).unwrap(),serde_json::to_string(provider_sha).unwrap())
}
pub(crate) fn verify_manifest(bytes: &[u8], expected: &str) -> Result<(), PackageError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PackageError::descriptor())?;
    if bytes == expected.as_bytes()
        && value.get("schema").and_then(Value::as_str) == Some(MANIFEST_SCHEMA)
    {
        Ok(())
    } else {
        Err(PackageError::descriptor())
    }
}
fn parameters(out: &mut String, export: &Export) {
    for (index, p) in export.parameters.iter().enumerate() {
        write!(out, ",arg_{index}:{}", rust_parameter(p.kind)).unwrap()
    }
}
fn arguments(out: &mut String, count: usize) {
    for index in 0..count {
        if index > 0 {
            out.push(',')
        }
        write!(out, "arg_{index}").unwrap()
    }
}
fn raw_parameters(out: &mut String, export: &Export) {
    for (index, p) in export.parameters.iter().enumerate() {
        match p.kind {
            ParameterKind::I64 => write!(out, ",arg_{index}:i64"),
            ParameterKind::Bool => write!(out, ",arg_{index}:u8"),
            ParameterKind::BorrowStr | ParameterKind::BorrowSliceU8 => {
                write!(out, ",arg_{index}:*const u8,arg_{index}_len:u64")
            }
        }
        .unwrap()
    }
}
fn raw_arguments(out: &mut String, export: &Export) {
    for (index, p) in export.parameters.iter().enumerate() {
        match p.kind {
            ParameterKind::I64 => write!(out, ",arg_{index}"),
            ParameterKind::Bool => write!(out, ",u8::from(arg_{index})"),
            ParameterKind::BorrowStr | ParameterKind::BorrowSliceU8 => {
                write!(out, ",arg_{index}.as_ptr(),arg_{index}.len()as u64")
            }
        }
        .unwrap()
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
fn rust_field<'a>(ty: &'a FieldType, descriptor: &'a Descriptor) -> &'a str {
    match ty {
        FieldType::I64 => "i64",
        FieldType::Bool => "bool",
        FieldType::Usize => "usize",
        FieldType::OwnedBytes => "Vec<u8>",
        FieldType::Record(id) => descriptor
            .records
            .iter()
            .find(|r| r.stable_id == *id)
            .expect("replay proves record")
            .host_name
            .as_str(),
    }
}
fn provider_symbol(method: &str) -> String {
    format!("spx_owned_data_call_{method}_v1")
}
