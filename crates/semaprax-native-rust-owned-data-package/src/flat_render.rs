use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::Value;

use super::descriptor::ParameterKind;
use super::flat_descriptor::{Descriptor, Export, FieldKind, API_SCHEMA};
use super::{raw_sha256, HostTarget, PackageError, OWNED_CRATE_NAME, OWNED_CRATE_VERSION};

pub(crate) const MANIFEST_SCHEMA: &str = "semaprax.native-rust-flat-owned-record-sdk.v1";

pub(crate) struct Sources {
    pub(crate) cargo_toml: String,
    pub(crate) build_rs: String,
    pub(crate) lib_rs: String,
    pub(crate) ffi_rs: String,
}

pub(crate) fn render_sources(descriptor: &Descriptor, target: HostTarget) -> Sources {
    let archive = target.archive_name();
    let triple = target.triple();
    Sources {
        cargo_toml: format!("[package]\nname = \"{OWNED_CRATE_NAME}\"\nversion = \"{OWNED_CRATE_VERSION}\"\nedition = \"2021\"\nrust-version = \"1.85\"\npublish = false\nbuild = \"build.rs\"\n\n[lib]\npath = \"lib.rs\"\n\n[workspace]\n"),
        build_rs: format!("#![forbid(unsafe_code)]\nfn main(){{if std::env::var(\"TARGET\").unwrap_or_default()!={triple:?}{{panic!(\"generated SEMAPRAX flat-record SDK target mismatch\")}}println!(\"cargo:rerun-if-changed={archive}\");println!(\"cargo:rustc-link-search=native={{}}\",std::env::var(\"CARGO_MANIFEST_DIR\").unwrap());println!(\"cargo:rustc-link-lib=static=semaprax_native_rust_owned_data_sdk\");}}\n"),
        lib_rs: render_lib(descriptor),
        ffi_rs: render_ffi(descriptor),
    }
}

fn render_lib(descriptor: &Descriptor) -> String {
    let mut output = String::from("#[path=\"owned_data_ffi.rs\"]mod ffi;\nmod safe_api{#![forbid(unsafe_code)]\nuse super::ffi;\n#[derive(Clone,Copy,Debug,Eq,PartialEq)]pub enum CallError{SemanticFailure,AdapterRejected,HostFailure}\n");
    let mut records = BTreeMap::new();
    for export in &descriptor.exports {
        records.entry(export.record_id.as_str()).or_insert(export);
    }
    for export in records.values() {
        writeln!(
            output,
            "#[derive(Clone,Debug,Eq,PartialEq)]pub struct {}{{",
            export.record_host_name
        )
        .unwrap();
        for field in &export.fields {
            writeln!(
                output,
                "pub {}:{},",
                field.host_name,
                rust_field(field.kind)
            )
            .unwrap();
        }
        output.push_str("}\n");
    }
    output.push_str("pub struct NativeRustOwnedDataSdk{context:ffi::Context}\nimpl NativeRustOwnedDataSdk{pub fn new()->Result<Self,CallError>{Ok(Self{context:ffi::Context::new().map_err(Self::map_failure)?})}\nfn map_failure(value:ffi::Failure)->CallError{match value{ffi::Failure::Semantic=>CallError::SemanticFailure,ffi::Failure::Adapter=>CallError::AdapterRejected,ffi::Failure::Host=>CallError::HostFailure}}\n");
    for export in &descriptor.exports {
        write!(output, "pub fn {}(&mut self", export.rust_method_name).unwrap();
        for (index, parameter) in export.parameters.iter().enumerate() {
            write!(output, ",arg_{index}:{}", rust_parameter(parameter.kind)).unwrap();
        }
        write!(
            output,
            ")->Result<{},CallError>{{let raw=self.context.call_{}(",
            export.record_host_name, export.rust_method_name
        )
        .unwrap();
        arguments(&mut output, export.parameters.len());
        output.push_str(").map_err(Self::map_failure)?;");
        let owned = export
            .fields
            .iter()
            .find(|field| field.kind == FieldKind::OwnedBytes)
            .expect("descriptor replay proves one owned field");
        write!(output, "let owned=self.context.copy_and_settle(raw.slots[{}]).map_err(Self::map_failure)?;Ok({}{{", owned.ordinal, export.record_host_name).unwrap();
        for field in &export.fields {
            write!(output, "{}:", field.host_name).unwrap();
            match field.kind {
                FieldKind::OwnedBytes => output.push_str("owned"),
                FieldKind::I64 => write!(
                    output,
                    "i64::from_ne_bytes(raw.slots[{}].to_ne_bytes())",
                    field.ordinal
                )
                .unwrap(),
                FieldKind::Usize => write!(
                    output,
                    "usize::try_from(raw.slots[{}]).map_err(|_|CallError::AdapterRejected)?",
                    field.ordinal
                )
                .unwrap(),
                FieldKind::Bool => write!(output, "raw.slots[{}]!=0", field.ordinal).unwrap(),
            }
            output.push(',');
        }
        output.push_str("})}\n");
    }
    output.push_str("}\n}\npub use safe_api::*;\n");
    output
}

fn render_ffi(descriptor: &Descriptor) -> String {
    let mut output = String::from("#![allow(unsafe_code)]\nuse core::marker::PhantomData;use core::ptr::NonNull;use std::rc::Rc;\n#[repr(C)]struct RawContext{_private:[u8;0]}\ntype Handle=u64;type Status=u32;\nextern \"C\"{fn spx_owned_data_context_size_v1()->u64;fn spx_owned_data_context_align_v1()->u64;fn spx_owned_data_context_init_v1(storage:*mut core::ffi::c_void,length:u64)->Status;fn spx_owned_data_context_drop_v1(context:*mut RawContext)->Status;fn spx_owned_bytes_len_v1(context:*mut RawContext,handle:Handle,length:*mut u64)->Status;fn spx_owned_bytes_copy_v1(context:*mut RawContext,handle:Handle,destination:*mut u8,length:u64)->Status;fn spx_owned_bytes_drop_v1(context:*mut RawContext,handle:Handle)->Status;\n");
    for export in &descriptor.exports {
        write!(
            output,
            "#[link_name={:?}]fn raw_{}(context:*mut RawContext",
            provider_symbol(&export.rust_method_name),
            export.rust_method_name
        )
        .unwrap();
        raw_parameters(&mut output, export);
        writeln!(
            output,
            ",record:*mut RawRecord{})->Status;",
            export.fields.len()
        )
        .unwrap();
    }
    output.push_str("}\n#[derive(Clone,Copy)]pub(super)enum Failure{Semantic,Adapter,Host}\n");
    for count in descriptor
        .exports
        .iter()
        .map(|export| export.fields.len())
        .collect::<std::collections::BTreeSet<_>>()
    {
        writeln!(
            output,
            "#[repr(C)]pub(super)struct RawRecord{count}{{pub slots:[u64;{count}]}}"
        )
        .unwrap();
    }
    output.push_str("pub(super)struct Context{storage:Vec<u64>,raw:NonNull<RawContext>,_thread:PhantomData<Rc<()>>}\nimpl Context{pub fn new()->Result<Self,Failure>{unsafe{let size=spx_owned_data_context_size_v1();let align=spx_owned_data_context_align_v1();if size==0||align==0||align>core::mem::align_of::<u64>()as u64{return Err(Failure::Adapter)}let words=usize::try_from(size.checked_add(7).ok_or(Failure::Adapter)?/8).map_err(|_|Failure::Adapter)?;let mut storage=vec![0u64;words];let raw=NonNull::new(storage.as_mut_ptr().cast()).ok_or(Failure::Host)?;if spx_owned_data_context_init_v1(raw.as_ptr().cast(),size)!=0{return Err(Failure::Adapter)}Ok(Self{storage,raw,_thread:PhantomData})}}\n");
    for export in &descriptor.exports {
        write!(output, "pub fn call_{}(&mut self", export.rust_method_name).unwrap();
        for (index, parameter) in export.parameters.iter().enumerate() {
            write!(output, ",arg_{index}:{}", rust_parameter(parameter.kind)).unwrap();
        }
        write!(output, ")->Result<RawRecord{},Failure>{{let mut value=RawRecord{}{{slots:[u64::MAX;{}]}};let status=unsafe{{raw_{}(self.raw.as_ptr()", export.fields.len(), export.fields.len(), export.fields.len(), export.rust_method_name).unwrap();
        raw_arguments(&mut output, export);
        output.push_str(",&mut value)};");
        let owned = export
            .fields
            .iter()
            .find(|field| field.kind == FieldKind::OwnedBytes)
            .expect("descriptor replay proves one owned field");
        write!(output, "if status!=0&&value.slots[{}]!=0&&value.slots[{}]!=u64::MAX{{self.discard(value.slots[{}])?}}", owned.ordinal, owned.ordinal, owned.ordinal).unwrap();
        write!(output, "if status==0&&(value.slots[{}]==0||value.slots[{}]==u64::MAX){{return Err(Failure::Adapter)}}", owned.ordinal, owned.ordinal).unwrap();
        for field in export
            .fields
            .iter()
            .filter(|field| field.kind == FieldKind::Bool)
        {
            write!(output, "if status==0&&value.slots[{}]>1{{self.discard(value.slots[{}])?;return Err(Failure::Adapter)}}", field.ordinal, owned.ordinal).unwrap();
        }
        output.push_str("if status==0{Ok(value)}else{match status{1=>Err(Failure::Semantic),2..=5=>Err(Failure::Adapter),_=>Err(Failure::Host)}}}\n");
    }
    output.push_str("pub fn copy_and_settle(&mut self,handle:Handle)->Result<Vec<u8>,Failure>{let mut guard=Guard{context:self,handle,armed:true};let mut length=0u64;if unsafe{spx_owned_bytes_len_v1(guard.context.raw.as_ptr(),handle,&mut length)}!=0{return Err(Failure::Adapter)}if length>65536{return Err(Failure::Adapter)}let length=usize::try_from(length).map_err(|_|Failure::Adapter)?;let mut bytes=vec![0u8;length];let pointer=if length==0{core::ptr::null_mut()}else{bytes.as_mut_ptr()};if unsafe{spx_owned_bytes_copy_v1(guard.context.raw.as_ptr(),handle,pointer,length as u64)}!=0{return Err(Failure::Adapter)}if unsafe{spx_owned_bytes_drop_v1(guard.context.raw.as_ptr(),handle)}!=0{std::process::abort()}guard.armed=false;Ok(bytes)}pub fn discard(&mut self,handle:Handle)->Result<(),Failure>{if unsafe{spx_owned_bytes_drop_v1(self.raw.as_ptr(),handle)}!=0{std::process::abort()}Ok(())}}\nstruct Guard<'a>{context:&'a mut Context,handle:Handle,armed:bool}impl Drop for Guard<'_>{fn drop(&mut self){if self.armed&&unsafe{spx_owned_bytes_drop_v1(self.context.raw.as_ptr(),self.handle)}!=0{std::process::abort()}}}\nimpl Drop for Context{fn drop(&mut self){let _=self.storage.len();if unsafe{spx_owned_data_context_drop_v1(self.raw.as_ptr())}!=0{std::process::abort()}}}\n");
    output
}

pub(crate) fn render_manifest(
    target: HostTarget,
    descriptor: &[u8],
    descriptor_digest: &str,
    archive_name: &str,
    provider_sha256: &str,
    mut files: [(&str, &[u8]); 6],
) -> String {
    files.sort_by_key(|row| row.0.as_bytes());
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
    format!("{{\"schema\":\"{MANIFEST_SCHEMA}\",\"crate\":{{\"name\":\"{OWNED_CRATE_NAME}\",\"version\":\"{OWNED_CRATE_VERSION}\"}},\"target\":{},\"descriptor\":{{\"schema\":\"{API_SCHEMA}\",\"bytes\":{},\"digest\":{}}},\"provider\":{{\"abi\":\"opaque-handle-plus-scalars.v1\",\"archive\":{},\"descriptor_digest\":{},\"source_sha256\":{},\"operations\":[\"len\",\"copy\",\"drop\"]}},\"files\":[{rows}],\"limits\":{{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}},\"nonclaims\":[\"lower_does_not_authenticate_provider_semantics\",\"no_public_aggregate_abi\",\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\"]}}\n", serde_json::to_string(target.triple()).unwrap(), descriptor.len(), serde_json::to_string(descriptor_digest).unwrap(), serde_json::to_string(archive_name).unwrap(), serde_json::to_string(descriptor_digest).unwrap(), serde_json::to_string(provider_sha256).unwrap())
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

fn arguments(output: &mut String, count: usize) {
    for index in 0..count {
        if index != 0 {
            output.push(',');
        }
        write!(output, "arg_{index}").unwrap();
    }
}
fn raw_parameters(output: &mut String, export: &Export) {
    for (index, parameter) in export.parameters.iter().enumerate() {
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
fn raw_arguments(output: &mut String, export: &Export) {
    for (index, parameter) in export.parameters.iter().enumerate() {
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
fn rust_field(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::I64 => "i64",
        FieldKind::Bool => "bool",
        FieldKind::Usize => "usize",
        FieldKind::OwnedBytes => "Vec<u8>",
    }
}
fn provider_symbol(method: &str) -> String {
    format!("spx_owned_data_call_{method}_v1")
}
