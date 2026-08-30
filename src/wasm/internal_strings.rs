//! Explicit standalone internal-String compilation and opt-in source Web
//! packaging. Legacy targets never select this profile implicitly. Returned
//! artifacts confer no execution authority.

mod admission;
mod runtime;
mod web;
pub use web::build_web_from_source;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::ast::Program;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{DeclarationId, ResolvedType};
use sha2::{Digest as _, Sha256};

/// Identity of the standalone compiler descriptor.
pub const SCHEMA: &str = "semaprax.wasm-internal-strings.v1";
/// Identity of the bound trusted JavaScript runtime.
pub const RUNTIME_SCHEMA: &str = "semaprax.wasm-internal-strings.runtime.v1";

/// Generation-time String arena policy. Limits cannot be widened by JavaScript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InternalStringOptions {
    pub max_string_bytes: u32,
    pub max_live_bytes: u32,
    pub max_cumulative_bytes: u32,
    /// `None` uses the compiler-derived simultaneous owner bound.
    pub max_live_owners: Option<u32>,
}

impl Default for InternalStringOptions {
    fn default() -> Self {
        Self {
            max_string_bytes: 65_536,
            max_live_bytes: 1_048_576,
            max_cumulative_bytes: 16_777_216,
            max_live_owners: None,
        }
    }
}

/// Immutable compiler output, not an independently replayed authority receipt.
#[derive(Debug)]
pub struct InternalStringModule {
    wasm: Vec<u8>,
    descriptor: String,
    runtime: String,
}

impl InternalStringModule {
    pub fn wasm_bytes(&self) -> &[u8] {
        &self.wasm
    }
    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }
    pub fn runtime_source(&self) -> &str {
        &self.runtime
    }
}

pub(super) struct Export {
    pub(super) id: DeclarationId,
    pub(super) parameters: Vec<ResolvedType>,
    pub(super) result: ResolvedType,
}

pub(super) fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W111", message)
}

/// Resolve and validate source, admit a closed selected profile, then emit its
/// module, canonical descriptor and digest-bound trusted runtime. No execution,
/// files, process, network or publication operations occur here.
pub fn emit_module(
    program: &Program,
    export_ids: &[String],
    options: InternalStringOptions,
) -> Result<InternalStringModule, Diagnostic> {
    let resolved = crate::hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|diagnostic| diagnostic.severity.is_error())
            .unwrap_or_else(|| error("standalone String HIR resolution failed"))
    })?;
    crate::hir::validate(&resolved)?;
    if options.max_string_bytes > 65_536
        || options.max_live_bytes > 16_777_216
        || options.max_cumulative_bytes > 67_108_864
    {
        return Err(error(
            "standalone String byte policy exceeds its hard bounds",
        ));
    }
    let (exports, closure) = admission::prepare(&resolved, export_ids)?;
    let (wasm, stack_bytes, derived_owner_capacity) = super::aggregate::internal_strings::emit(
        &resolved,
        &exports,
        &closure,
        options.max_live_owners,
    )?;
    let owners = options.max_live_owners.unwrap_or(derived_owner_capacity);
    if owners == 0 || owners > derived_owner_capacity {
        return Err(error(
            "standalone String owner policy must be within the derived bound",
        ));
    }
    let wasm_sha256 = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&wasm)));
    let mut descriptor = format!(
        "{{\"schema\":{},\"runtime_schema\":{},\"wasm_sha256\":{},\"wasm_bytes\":{},\"memory_pages\":4,\"result_offset\":65536,\"literal_offset\":196608,\"stack_bytes\":{},\"derived_owner_capacity\":{},\"limits\":{{\"max_string_bytes\":{},\"max_live_bytes\":{},\"max_cumulative_bytes\":{},\"max_live_owners\":{}}},\"exports\":[",
        quote_json(SCHEMA), quote_json(RUNTIME_SCHEMA), quote_json(&wasm_sha256), wasm.len(),
        stack_bytes, derived_owner_capacity, options.max_string_bytes, options.max_live_bytes,
        options.max_cumulative_bytes, owners
    );
    for (ordinal, export) in exports.iter().enumerate() {
        if ordinal != 0 {
            descriptor.push(',');
        }
        write!(
            descriptor,
            "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[",
            quote_json(export.id.as_str()),
            quote_json(&format!("__spx_call_{ordinal}"))
        )
        .expect("writing to String cannot fail");
        for (index, parameter) in export.parameters.iter().enumerate() {
            if index != 0 {
                descriptor.push(',');
            }
            descriptor.push_str(&quote_json(scalar_name(parameter)));
        }
        write!(
            descriptor,
            "],\"result\":{}}}",
            quote_json(scalar_name(&export.result))
        )
        .expect("writing to String cannot fail");
    }
    descriptor.push_str("]}");
    let runtime = runtime::render(&descriptor, &wasm_sha256, wasm.len());
    Ok(InternalStringModule {
        wasm,
        descriptor,
        runtime,
    })
}

fn scalar_name(ty: &ResolvedType) -> &'static str {
    match ty {
        ResolvedType::I64 => "i64",
        ResolvedType::Bool => "bool",
        _ => unreachable!("admitted scalar export"),
    }
}

type PreparedSelection = (Vec<Export>, BTreeSet<DeclarationId>);
