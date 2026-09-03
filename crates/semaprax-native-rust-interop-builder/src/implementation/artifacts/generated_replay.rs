//! Independent replay of the generated header and Rust artifacts.

use super::*;

pub(in crate::implementation) fn replay_generated(
    header: &str,
    c: &str,
    rust: &str,
    ffi: &str,
) -> Result<(), Diagnostic> {
    if !header.starts_with("#ifndef ")
        || !header.ends_with("#endif\n")
        || !c.starts_with("#include \"semaprax_native_rust_interop.h\"")
        || !rust.starts_with("mod api{#![forbid(unsafe_code)]\n")
        || rust.contains("unsafe {")
        || !ffi.starts_with("#![allow(unsafe_code)]\n")
    {
        return Err(b111());
    }
    Ok(())
}

pub(super) fn replay_header_exact(
    source: &str,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("#ifndef SEMAPRAX_NATIVE_RUST_INTEROP_H\n#define SEMAPRAX_NATIVE_RUST_INTEROP_H\n#include <stdint.h>\n#include <stddef.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\ntypedef uint64_t spxnr_status_v1;\ntypedef struct spxnr_imports_v1 spxnr_imports_v1;\ntypedef struct { uint32_t abi_version; uint32_t size; void *userdata; const spxnr_imports_v1 *imports; uint8_t capabilities_digest[32]; uint32_t call_depth; uint32_t reserved; } spxnr_context_v1;\nstruct spxnr_imports_v1 { uint32_t abi_version; uint32_t size;");
    for import in imports {
        replay.text(" spxnr_status_v1 (*");
        replay.text(&import.c_field);
        replay.text(")(void *userdata");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(", ");
            replay.text(c_type(parameter.ty));
            replay.text(" arg_");
            replay.number(index);
        }
        if import.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(c_type(import.result));
            replay.text(" *result_out");
        }
        replay.text(");");
    }
    replay.text(" };\n");
    for export in exports {
        replay.text("spxnr_status_v1 ");
        replay.text(&export.c_symbol);
        replay.text("(const spxnr_context_v1 *ctx");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(", ");
            replay.text(c_type(parameter.ty));
            replay.text(" arg_");
            replay.number(index);
        }
        if export.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(c_type(export.result));
            replay.text(" *result_out");
        }
        replay.text(");\n");
    }
    replay.text("#ifdef __cplusplus\n}\n#endif\n#endif\n");
    replay.finish()
}

fn replay_rust_scalar(replay: &mut ExactReplay<'_>, ty: ScalarType) {
    replay.text(match ty {
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
        ScalarType::Unit => "()",
    });
}

fn replay_rust_parameters(replay: &mut ExactReplay<'_>, parameters: &[ParameterFact]) {
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            replay.text(", ");
        }
        replay.text("arg_");
        replay.number(index);
        replay.text(": ");
        replay_rust_scalar(replay, parameter.ty);
    }
}

pub(super) fn replay_safe_rust_exact(
    source: &str,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("mod api{#![forbid(unsafe_code)]\nuse core::num::NonZeroU32;\n#[repr(u8)] #[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum NativeRustStatusClass{Semantic=1,Contract=2,Import=3,Adapter=4}\n");
    if !imports.is_empty() {
        replay.text("pub enum NativeRustImportResult<T>{Success(T),Status{code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailure}\n");
    }
    replay.text("pub enum NativeRustCallError{Semantic{domain_id:&'static str,code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailed,HostPanicked,AdapterRejected}\npub struct NativeRustAdmissionError;\n");
    replay.text("pub trait NativeRustImports{");
    for import in imports {
        replay.text("fn ");
        replay.text(&import.rust_method);
        replay.text("(&mut self");
        if !import.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &import.parameters);
        }
        replay.text(")->NativeRustImportResult<");
        replay_rust_scalar(&mut replay, import.result);
        replay.text(">;");
    }
    replay.text("}\nconst EXPECTED_CAPABILITIES:&[&str]=&[");
    for (index, capability) in spec.capabilities.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(capability);
    }
    replay.text("];\n");
    replay.text("pub struct NativeRustCapabilities{digest:[u8;32]} impl NativeRustCapabilities{pub fn new(values:&[&str])->Result<Self,NativeRustAdmissionError>{if values!=EXPECTED_CAPABILITIES{return Err(NativeRustAdmissionError)}Ok(Self{digest:super::ffi::capabilities_digest()})}}\n");
    replay.text("struct ActiveGuard<'a>{active:&'a mut bool}impl Drop for ActiveGuard<'_>{fn drop(&mut self){*self.active=false;}}\npub struct NativeRustBridge<H:NativeRustImports>{host:H,capabilities:NativeRustCapabilities,owner:std::thread::ThreadId,active:bool,calls:u32,_not_send_sync:core::marker::PhantomData<*mut ()>} impl<H:NativeRustImports> NativeRustBridge<H>{pub fn new(host:H,capabilities:NativeRustCapabilities)->Self{Self{host,capabilities,owner:std::thread::current().id(),active:false,calls:0,_not_send_sync:core::marker::PhantomData}}\n");
    for export in exports {
        replay.text("pub fn ");
        replay.text(&export.rust_method);
        replay.text("(&mut self");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &export.parameters);
        }
        replay.text(")->Result<");
        replay_rust_scalar(&mut replay, export.result);
        replay.text(",NativeRustCallError>{if self.owner!=std::thread::current().id()||core::mem::replace(&mut self.active,true){return Err(NativeRustCallError::AdapterRejected)}let _active_guard=ActiveGuard{active:&mut self.active};super::ffi::");
        replay.text(&export.rust_method);
        replay.text("(&mut self.host,&mut self.calls,self.capabilities.digest");
        for index in 0..export.parameters.len() {
            replay.text(", arg_");
            replay.number(index);
        }
        replay.text(")}\n");
    }
    replay.text("}\n}\n#[path=\"semaprax_native_rust_interop_ffi.rs\"]mod ffi;\npub use api::*;\n");
    replay.finish()
}

fn replay_ffi_wire_scalar(replay: &mut ExactReplay<'_>, ty: ScalarType) {
    replay.text(match ty {
        ScalarType::I64 => "i64",
        ScalarType::Bool => "u8",
        ScalarType::Unit => "()",
    });
}

pub(super) fn replay_private_ffi_exact(
    source: &str,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("#![allow(unsafe_code)]\nuse super::api::*;\nuse core::ffi::c_void;\n#[repr(C)]struct Imports{abi_version:u32,size:u32,");
    for import in imports {
        replay.text(&import.c_field);
        replay.text(":unsafe extern \"C\" fn(*mut c_void");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", " } else { "," });
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if import.result != ScalarType::Unit {
            replay.text(", *mut ");
            replay_ffi_wire_scalar(&mut replay, import.result);
        }
        replay.text(")->u64,");
    }
    replay.text(" }\n#[repr(C)]struct Context{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}\n");
    if !imports.is_empty() {
        replay.text("struct Frame<H>{host:*mut H,calls:*mut u32}\n");
    }
    replay.text("pub(super) fn capabilities_digest()->[u8;32]{[");
    let digest = replay_capabilities_digest(&spec.capabilities);
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    if hex.len() != 64 {
        return false;
    }
    for index in (0..64).step_by(2) {
        if index != 0 {
            replay.text(",");
        }
        replay.text("0x");
        replay.text(&hex[index..index + 2]);
    }
    replay.text("]}\n");
    if !imports.is_empty() {
        replay.text("fn adapter(code:u32)->u64{((65535u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|u64::from(code)}\n");
    }
    replay.text("fn decode_status(status:u64)->NativeRustCallError{let code=(status&0xffff_ffff)as u32;let class=((status>>32)&0xff)as u8;let retryable=((status>>40)&1)!=0;let reserved=(status>>41)&0x7f;let domain=(status>>48)as u16;let Some(code)=core::num::NonZeroU32::new(code)else{return NativeRustCallError::AdapterRejected};let class=match class{1=>NativeRustStatusClass::Semantic,2=>NativeRustStatusClass::Contract,3=>NativeRustStatusClass::Import,4=>NativeRustStatusClass::Adapter,_=>return NativeRustCallError::AdapterRejected};if reserved!=0||domain==0{return NativeRustCallError::AdapterRejected}match domain{65533=>{let valid=!retryable&&match class{NativeRustStatusClass::Semantic=>(1..=6).contains(&code.get()),NativeRustStatusClass::Contract=>(1..=2).contains(&code.get()),_=>false};if !valid{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:\"semaprax.native-rust-semantics.v1\",code,class,retryable}},");
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .cloned()
        .collect::<BTreeSet<_>>();
    for (index, domain) in domains.iter().enumerate() {
        replay.number(index + 1);
        replay.text("=>{if class!=NativeRustStatusClass::Import{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:");
        replay.json(domain);
        replay.text(",code,class,retryable}},");
    }
    replay.text("65534=>if class==NativeRustStatusClass::Adapter&&!retryable{match code.get(){1=>NativeRustCallError::HostPanicked,2=>NativeRustCallError::HostFailed,_=>NativeRustCallError::AdapterRejected}}else{NativeRustCallError::AdapterRejected},65535=>if class==NativeRustStatusClass::Adapter&&!retryable&&(1..=8).contains(&code.get()){NativeRustCallError::AdapterRejected}else{NativeRustCallError::AdapterRejected},_=>NativeRustCallError::AdapterRejected}}\n");
    for import in imports {
        replay.text("unsafe extern \"C\" fn cb_");
        replay.text(&import.rust_method);
        replay.text("<H:NativeRustImports>(userdata:*mut c_void");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", arg_" } else { ",arg_" });
            replay.number(index);
            replay.text(":");
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if import.result != ScalarType::Unit {
            replay.text(", result_out:*mut ");
            replay_ffi_wire_scalar(&mut replay, import.result);
        }
        replay.text(") -> u64{if userdata.is_null(){return adapter(1);}");
        for (index, parameter) in import.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                replay.text("if arg_");
                replay.number(index);
                replay.text(">1{return adapter(4);}");
            }
        }
        if import.result != ScalarType::Unit {
            replay.text("if result_out.is_null()||(result_out as usize)%core::mem::align_of::<");
            replay_rust_scalar(&mut replay, import.result);
            replay.text(">()!=0{return adapter(5);}");
        }
        replay.text("if (userdata as usize)%core::mem::align_of::<Frame<H>>()!=0{return adapter(1);}let frame=&mut*(userdata as *mut Frame<H>);if frame.host.is_null()||frame.calls.is_null()||*frame.calls>=4096{return adapter(7);}*frame.calls+=1;let run=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{let host=&mut *frame.host;host.");
        replay.text(&import.rust_method);
        replay.text("(");
        for (index, parameter) in import.parameters.iter().enumerate() {
            if index != 0 {
                replay.text(",");
            }
            replay.text("arg_");
            replay.number(index);
            if parameter.ty == ScalarType::Bool {
                replay.text("!=0");
            }
        }
        replay.text(")}));match run{Err(payload)=>{core::mem::forget(payload);((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|1},Ok(NativeRustImportResult::HostFailure)=>((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|2,");
        if let Some(domain) = &import.failure {
            let Some(ordinal) = domains.iter().position(|value| value == domain) else {
                return false;
            };
            replay.text("Ok(NativeRustImportResult::Status{code,class,retryable})=>if class==NativeRustStatusClass::Import{((");
            replay.number(ordinal + 1);
            replay.text("u64)<<48)|((class as u64)<<32)|((retryable as u64)<<40)|u64::from(code.get())}else{adapter(3)},");
        } else {
            replay.text("Ok(NativeRustImportResult::Status{code,class,retryable})=>{let _=(code,class,retryable);adapter(3)},");
        }
        if import.result == ScalarType::Unit {
            replay.text("Ok(NativeRustImportResult::Success(()))=>0}}}\n");
        } else {
            replay.text("Ok(NativeRustImportResult::Success(value))=>{*result_out=");
            if import.result == ScalarType::Bool {
                replay.text("u8::from(value)");
            } else {
                replay.text("value");
            }
            replay.text(";0}}}\n");
        }
    }
    for export in exports {
        replay.text("extern \"C\"{fn ");
        replay.text(&export.c_symbol);
        replay.text("(ctx:*const Context");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", arg_" } else { ",arg_" });
            replay.number(index);
            replay.text(":");
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if export.result != ScalarType::Unit {
            replay.text(", result_out:*mut ");
            replay_ffi_wire_scalar(&mut replay, export.result);
        }
        replay.text(")->u64;}\n");
    }
    for export in exports {
        replay.text("pub(super) fn ");
        replay.text(&export.rust_method);
        replay.text("<H:NativeRustImports>(host:&mut H,calls:&mut u32,digest:[u8;32]");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &export.parameters);
        }
        replay.text(")->Result<");
        replay_rust_scalar(&mut replay, export.result);
        replay.text(",NativeRustCallError>{unsafe{if *calls>=4096{return Err(NativeRustCallError::AdapterRejected)}*calls+=1;let table=Imports{abi_version:1,size:core::mem::size_of::<Imports>() as u32,");
        for import in imports {
            replay.text(&import.c_field);
            replay.text(":cb_");
            replay.text(&import.rust_method);
            replay.text("::<H>,");
        }
        replay.text("};");
        if imports.is_empty() {
            replay.text("let _=host;let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:core::ptr::null_mut(),imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};");
        } else {
            replay.text("let mut frame=Frame{host:host as *mut H,calls:calls as *mut u32};let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:&mut frame as *mut Frame<H> as *mut c_void,imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};");
        }
        match export.result {
            ScalarType::Unit => {}
            ScalarType::I64 => {
                replay.text("let mut result=core::mem::MaybeUninit::<i64>::uninit();")
            }
            ScalarType::Bool => {
                replay.text("let mut result=core::mem::MaybeUninit::<u8>::uninit();")
            }
        }
        replay.text("let status=");
        replay.text(&export.c_symbol);
        replay.text("(&ctx");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", " } else { "," });
            if parameter.ty == ScalarType::Bool {
                replay.text("u8::from(");
            }
            replay.text("arg_");
            replay.number(index);
            if parameter.ty == ScalarType::Bool {
                replay.text(")");
            }
        }
        if export.result != ScalarType::Unit {
            replay.text(", result.as_mut_ptr()");
        }
        replay.text(");if status!=0{return Err(decode_status(status))}");
        match export.result {
            ScalarType::Unit => replay.text("Ok(())"),
            ScalarType::I64 => replay.text("Ok(result.assume_init())"),
            ScalarType::Bool => replay.text("let value=result.assume_init();if value>1{return Err(NativeRustCallError::AdapterRejected)}Ok(value!=0)"),
        }
        replay.text(" }}\n");
    }
    replay.finish()
}

pub(super) fn replay_c_scalar(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::I64 => "int64_t",
        ScalarType::Bool => "uint8_t",
        ScalarType::Unit => "void",
    }
}

pub(in crate::implementation) fn replay_symbol_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(encoded, "{byte:02x}").unwrap();
    }
    #[cfg(test)]
    note_post_hir_replay_capacity(encoded.capacity());
    encoded
}

pub(in crate::implementation) fn replay_capabilities_digest(capabilities: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CAPABILITIES_DOMAIN);
    for capability in capabilities {
        hasher.update(
            u64::try_from(capability.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hasher.update(capability.as_bytes());
    }
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    #[cfg(test)]
    note_post_hir_replay_capacity(digest.capacity());
    digest
}

pub(super) fn replay_resolved_scalar(ty: &ResolvedType) -> Option<ScalarType> {
    match ty {
        ResolvedType::Unit => Some(ScalarType::Unit),
        ResolvedType::I64 => Some(ScalarType::I64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        _ => None,
    }
}

pub(super) fn replay_parameter_facts(
    function: &ResolvedFunction,
) -> Result<Vec<ParameterFact>, Diagnostic> {
    if function.params.len() > MAX_PARAMETERS {
        return Err(b109("max_parameters", MAX_PARAMETERS));
    }
    function
        .params
        .iter()
        .map(|parameter| {
            if parameter.ownership != OwnershipMode::Value
                || parameter.name.len() > MAX_IDENTIFIER_BYTES
            {
                return Err(b107("scalar value signature required"));
            }
            Ok(ParameterFact {
                name: parameter.name.clone(),
                ty: replay_resolved_scalar(&parameter.ty)
                    .filter(|ty| *ty != ScalarType::Unit)
                    .ok_or_else(|| b107("scalar value signature required"))?,
            })
        })
        .collect()
}

pub(super) fn replay_c_parameters(parameters: &[ParameterFact]) -> String {
    let values = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("{} arg_{index}", replay_c_scalar(parameter.ty)))
        .collect::<Vec<_>>();
    let joined = values.join(", ");
    #[cfg(test)]
    note_post_hir_replay_capacity(
        string_slice_owned_capacity(&values).saturating_add(joined.capacity()),
    );
    joined
}
