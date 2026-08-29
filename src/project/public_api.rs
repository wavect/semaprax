//! Canonical, authority-free semantic descriptor for the proposed Project v8
//! public owned-data API.
//!
//! This module does not add a Project manifest profile or a build route. It
//! derives one bounded target-neutral description from validated HIR so later
//! target generators do not rediscover source signatures independently.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::call_index::{PersistentCallIndex, PersistentCallableKind};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    DeclarationId, IdentityOrigin, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedType, ValueId,
};

pub const PUBLIC_OWNED_DATA_API_SCHEMA: &str = "semaprax.public-owned-data-api.v1";
pub const PUBLIC_OWNED_DATA_PROJECT_SCHEMA: &str = "semaprax.project.v8";
pub const MAX_PUBLIC_API_DESCRIPTOR_BYTES: usize = 1024 * 1024;
pub const MAX_PUBLIC_API_EXPORTS: usize = 32;
pub const MAX_PUBLIC_API_PARAMETERS: usize = 8;
pub const MAX_PUBLIC_API_CLOSURE_FUNCTIONS: usize = 256;
pub const MAX_PUBLIC_API_BORROWED_INPUT_BYTES: usize = 65_536;
pub const MAX_PUBLIC_API_OWNED_OUTPUT_BYTES: usize = 65_536;

const MAX_STABLE_ID_BYTES: usize = 128;
const MAX_PARAMETER_ID_BYTES: usize = 512;
const MAX_SOURCE_NAME_BYTES: usize = 128;
const DESCRIPTOR_DIGEST_DOMAIN: &[u8] = b"semaprax.public-owned-data-api.digest.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicApiParameterType {
    I64,
    Bool,
    BorrowStr,
    BorrowSliceU8,
}

impl PublicApiParameterType {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::BorrowStr => "borrow-str",
            Self::BorrowSliceU8 => "borrow-slice-u8",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "i64" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            "borrow-str" => Some(Self::BorrowStr),
            "borrow-slice-u8" => Some(Self::BorrowSliceU8),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicApiResultType {
    I64,
    Bool,
    Usize,
    OwnedBytes,
    OptionOwnedBytes,
    ResultOwnedBytesI64,
}

impl PublicApiResultType {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Usize => "usize",
            Self::OwnedBytes => "owned-bytes",
            Self::OptionOwnedBytes => "option-owned-bytes",
            Self::ResultOwnedBytesI64 => "result-owned-bytes-i64",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "i64" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            "usize" => Some(Self::Usize),
            "owned-bytes" => Some(Self::OwnedBytes),
            "option-owned-bytes" => Some(Self::OptionOwnedBytes),
            "result-owned-bytes-i64" => Some(Self::ResultOwnedBytesI64),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicApiParameter {
    stable_id: ValueId,
    source_name: String,
    ty: PublicApiParameterType,
}

impl PublicApiParameter {
    pub fn stable_id(&self) -> &ValueId {
        &self.stable_id
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub const fn ty(&self) -> PublicApiParameterType {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicApiExport {
    stable_id: DeclarationId,
    typescript_name: String,
    rust_method_name: String,
    parameters: Vec<PublicApiParameter>,
    result: PublicApiResultType,
}

impl PublicApiExport {
    pub fn stable_id(&self) -> &DeclarationId {
        &self.stable_id
    }

    /// TypeScript's stable-ID property name. Generators must quote this value.
    pub fn typescript_name(&self) -> &str {
        &self.typescript_name
    }

    /// Injective Rust identifier derived from the persistent stable ID.
    pub fn rust_method_name(&self) -> &str {
        &self.rust_method_name
    }

    pub fn parameters(&self) -> &[PublicApiParameter] {
        &self.parameters
    }

    pub const fn result(&self) -> PublicApiResultType {
        self.result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiLimits {
    pub max_exports: usize,
    pub max_parameters: usize,
    pub max_closure_functions: usize,
    pub max_borrowed_input_bytes: usize,
    pub max_owned_output_bytes: usize,
    pub max_descriptor_bytes: usize,
}

impl PublicApiLimits {
    pub const V1: Self = Self {
        max_exports: MAX_PUBLIC_API_EXPORTS,
        max_parameters: MAX_PUBLIC_API_PARAMETERS,
        max_closure_functions: MAX_PUBLIC_API_CLOSURE_FUNCTIONS,
        max_borrowed_input_bytes: MAX_PUBLIC_API_BORROWED_INPUT_BYTES,
        max_owned_output_bytes: MAX_PUBLIC_API_OWNED_OUTPUT_BYTES,
        max_descriptor_bytes: MAX_PUBLIC_API_DESCRIPTOR_BYTES,
    };
}

/// Invocation-borrowed, authority-free Project identity facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiSubject<'a> {
    pub project_schema: &'a str,
    pub project_revision: &'a str,
    pub workspace_revision: &'a str,
    pub project_graph_digest: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicApiDescriptor {
    schema: &'static str,
    project_schema: &'static str,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    exports: Vec<PublicApiExport>,
    limits: PublicApiLimits,
}

impl PublicApiDescriptor {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn project_schema(&self) -> &'static str {
        self.project_schema
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }

    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn project_graph_digest(&self) -> &str {
        &self.project_graph_digest
    }

    pub fn exports(&self) -> &[PublicApiExport] {
        &self.exports
    }

    pub const fn limits(&self) -> PublicApiLimits {
        self.limits
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        render_descriptor(self).into_bytes()
    }

    pub fn digest(&self) -> String {
        domain_digest(DESCRIPTOR_DIGEST_DOMAIN, &self.canonical_bytes())
    }
}

/// Derive the only semantic API description intended for every future v8
/// target generator. This operation reads validated HIR and has no authority.
pub fn derive_public_api_descriptor(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
) -> Result<PublicApiDescriptor, Diagnostic> {
    validate_subject(subject)?;
    validate_selected(selected)?;
    let call_index = PersistentCallIndex::build(program)?;
    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let closure = selected_closure(program, selected, &functions, &call_index)?;
    for id in &closure {
        let function = functions
            .get(id)
            .ok_or_else(|| api_error("selected closure contains a non-monomorphic callable"))?;
        validate_closure_function(function)?;
    }

    let mut exports = Vec::with_capacity(selected.len());
    let mut rust_names = BTreeSet::new();
    for stable_id in selected {
        let id = DeclarationId::new(stable_id.clone());
        let function = functions.get(&id).ok_or_else(|| {
            api_error(format!(
                "selected public API export `{stable_id}` is absent"
            ))
        })?;
        let declaration = program.declarations.declaration(&id).ok_or_else(|| {
            api_error(format!(
                "selected public API export `{stable_id}` lacks declaration metadata"
            ))
        })?;
        if declaration.identity_origin != IdentityOrigin::Explicit {
            return Err(api_error(format!(
                "selected public API export `{stable_id}` requires an explicit @id"
            )));
        }
        if id == program.entrypoint {
            return Err(api_error(format!(
                "selected public API export `{stable_id}` must not be the entry function"
            )));
        }
        let parameters = function
            .params
            .iter()
            .map(|parameter| {
                if parameter.name.is_empty()
                    || parameter.name.len() > MAX_SOURCE_NAME_BYTES
                    || parameter.name.chars().any(char::is_control)
                {
                    return Err(api_error(format!(
                        "selected public API export `{stable_id}` has an invalid parameter name"
                    )));
                }
                let ty = parameter_type(&parameter.ty, parameter.ownership).ok_or_else(|| {
                    api_error(format!(
                        "selected public API export `{stable_id}` has an unsupported parameter"
                    ))
                })?;
                Ok(PublicApiParameter {
                    stable_id: parameter.id.clone(),
                    source_name: parameter.name.clone(),
                    ty,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if parameters.len() > MAX_PUBLIC_API_PARAMETERS {
            return Err(api_error(format!(
                "selected public API export `{stable_id}` exceeds the {MAX_PUBLIC_API_PARAMETERS}-parameter limit"
            )));
        }
        let result = result_type(&function.return_type).ok_or_else(|| {
            api_error(format!(
                "selected public API export `{stable_id}` has an unsupported result"
            ))
        })?;
        let rust_method_name = rust_method_name(stable_id)?;
        if !rust_names.insert(rust_method_name.clone()) {
            return Err(api_error("public API Rust method identities collide"));
        }
        exports.push(PublicApiExport {
            stable_id: id,
            typescript_name: stable_id.clone(),
            rust_method_name,
            parameters,
            result,
        });
    }
    let descriptor = PublicApiDescriptor {
        schema: PUBLIC_OWNED_DATA_API_SCHEMA,
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: subject.project_revision.to_owned(),
        workspace_revision: subject.workspace_revision.to_owned(),
        project_graph_digest: subject.project_graph_digest.to_owned(),
        exports,
        limits: PublicApiLimits::V1,
    };
    if descriptor.canonical_bytes().len() > MAX_PUBLIC_API_DESCRIPTOR_BYTES {
        return Err(api_error("public API descriptor exceeds its byte limit"));
    }
    Ok(descriptor)
}

/// Independently parse canonical bytes, validate their closed shape and
/// digest, rebuild the descriptor from HIR, and require exact equality.
pub fn replay_public_api_descriptor(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    submitted: &[u8],
    submitted_digest: &str,
) -> Result<PublicApiDescriptor, Diagnostic> {
    let parsed = parse_descriptor(submitted)?;
    if domain_digest(DESCRIPTOR_DIGEST_DOMAIN, submitted) != submitted_digest {
        return Err(api_error("public API descriptor digest does not match"));
    }
    let rebuilt = derive_public_api_descriptor(program, selected, subject)?;
    if parsed != ParsedDescriptor::from_descriptor(&rebuilt)
        || submitted != rebuilt.canonical_bytes().as_slice()
        || submitted_digest != rebuilt.digest()
    {
        return Err(api_error(
            "public API descriptor does not replay against the retained subject",
        ));
    }
    Ok(rebuilt)
}

fn validate_subject(subject: PublicApiSubject<'_>) -> Result<(), Diagnostic> {
    if subject.project_schema != PUBLIC_OWNED_DATA_PROJECT_SCHEMA {
        return Err(api_error(
            "public API descriptor requires the inactive Project v8 schema fact",
        ));
    }
    for (name, value) in [
        ("project revision", subject.project_revision),
        ("workspace revision", subject.workspace_revision),
        ("project graph digest", subject.project_graph_digest),
    ] {
        if !valid_sha256_fact(value) {
            return Err(api_error(format!(
                "public API descriptor {name} is not a canonical SHA-256 fact"
            )));
        }
    }
    Ok(())
}

fn validate_selected(selected: &[String]) -> Result<(), Diagnostic> {
    if !(1..=MAX_PUBLIC_API_EXPORTS).contains(&selected.len()) {
        return Err(api_error(format!(
            "public API descriptor requires 1..={MAX_PUBLIC_API_EXPORTS} exports"
        )));
    }
    let mut previous: Option<&str> = None;
    for stable_id in selected {
        if !valid_stable_id(stable_id) {
            return Err(api_error("public API export stable identity is invalid"));
        }
        if previous.is_some_and(|value| value.as_bytes() >= stable_id.as_bytes()) {
            return Err(api_error(
                "public API export identities must be strictly sorted and unique",
            ));
        }
        previous = Some(stable_id);
    }
    Ok(())
}

fn selected_closure(
    program: &ResolvedProgram,
    selected: &[String],
    functions: &BTreeMap<DeclarationId, &ResolvedFunction>,
    index: &PersistentCallIndex,
) -> Result<BTreeSet<DeclarationId>, Diagnostic> {
    let mut states = BTreeMap::<DeclarationId, u8>::new();
    let mut closure = BTreeSet::new();
    for root in selected.iter().map(|id| DeclarationId::new(id.clone())) {
        if !functions.contains_key(&root) {
            return Err(api_error(format!(
                "selected public API export `{root}` is not a monomorphic function"
            )));
        }
        if states.get(&root) == Some(&2) {
            continue;
        }
        let mut stack = vec![(root, false)];
        while let Some((id, exiting)) = stack.pop() {
            if exiting {
                states.insert(id, 2);
                continue;
            }
            match states.get(&id).copied() {
                Some(1) => return Err(api_error("public API selected closure must be acyclic")),
                Some(2) => continue,
                _ => {}
            }
            if index.kind(&id) != Some(PersistentCallableKind::Function)
                || !functions.contains_key(&id)
            {
                return Err(api_error("public API selected closure must be monomorphic"));
            }
            states.insert(id.clone(), 1);
            closure.insert(id.clone());
            if closure.len() > MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
                return Err(api_error(format!(
                    "public API selected closure exceeds {MAX_PUBLIC_API_CLOSURE_FUNCTIONS} functions"
                )));
            }
            stack.push((id.clone(), true));
            if let Some(callees) = index.calls_by_owner().get(&id) {
                for callee in callees.iter().rev() {
                    match states.get(callee).copied() {
                        Some(1) => {
                            return Err(api_error("public API selected closure must be acyclic"))
                        }
                        Some(2) => {}
                        _ => stack.push((callee.clone(), false)),
                    }
                }
            }
        }
    }
    if closure.is_empty() || program.functions.is_empty() {
        return Err(api_error("public API selected closure is empty"));
    }
    Ok(closure)
}

fn validate_closure_function(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    if !function.effects.is_empty() {
        return Err(api_error(format!(
            "public API closure function `{}` must be effect-free",
            function.id
        )));
    }
    if !function.requires.is_empty() || !function.ensures.is_empty() {
        return Err(api_error(format!(
            "public API closure function `{}` must be contract-free",
            function.id
        )));
    }
    if expression_reaches_import(&function.body) {
        return Err(api_error(format!(
            "public API closure function `{}` must be import-free",
            function.id
        )));
    }
    Ok(())
}

fn expression_reaches_import(root: &ResolvedExpr) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::NativeRustImportCall(_) | ResolvedExprKind::HostCommandCall(_) => {
                return true
            }
            ResolvedExprKind::Call { args, .. } => pending.extend(args),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        } else {
                            return true;
                        }
                    }
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => pending.extend([
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]),
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        pending.push(guard);
                    }
                    pending.push(&arm.value);
                }
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::Place(_) => {}
        }
    }
    false
}

fn parameter_type(ty: &ResolvedType, ownership: OwnershipMode) -> Option<PublicApiParameterType> {
    match (ty, ownership) {
        (ResolvedType::I64, OwnershipMode::Value) => Some(PublicApiParameterType::I64),
        (ResolvedType::Bool, OwnershipMode::Value) => Some(PublicApiParameterType::Bool),
        (ResolvedType::Str, OwnershipMode::Borrow) => Some(PublicApiParameterType::BorrowStr),
        (ResolvedType::SliceU8, OwnershipMode::Borrow) => {
            Some(PublicApiParameterType::BorrowSliceU8)
        }
        _ => None,
    }
}

fn result_type(ty: &ResolvedType) -> Option<PublicApiResultType> {
    match ty {
        ResolvedType::I64 => Some(PublicApiResultType::I64),
        ResolvedType::Bool => Some(PublicApiResultType::Bool),
        ResolvedType::Usize => Some(PublicApiResultType::Usize),
        ResolvedType::Bytes => Some(PublicApiResultType::OwnedBytes),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if declaration.as_str() == crate::prelude::OPTION_ID
            && arguments.as_slice() == [ResolvedType::Bytes] =>
        {
            Some(PublicApiResultType::OptionOwnedBytes)
        }
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if declaration.as_str() == crate::prelude::RESULT_ID
            && arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::I64] =>
        {
            Some(PublicApiResultType::ResultOwnedBytesI64)
        }
        _ => None,
    }
}

fn rust_method_name(stable_id: &str) -> Result<String, Diagnostic> {
    if !valid_stable_id(stable_id) {
        return Err(api_error("public API stable identity is not portable"));
    }
    let mut output = String::with_capacity(stable_id.len().saturating_mul(12).saturating_add(4));
    output.push_str("spx_");
    for byte in stable_id.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => output.push(char::from(byte)),
            b'_' => output.push_str("_underscore_"),
            b'.' => output.push_str("_dot_"),
            b'-' => output.push_str("_hyphen_"),
            _ => return Err(api_error("public API stable identity is not portable")),
        }
    }
    Ok(output)
}

fn render_descriptor(descriptor: &PublicApiDescriptor) -> String {
    let mut output = String::with_capacity(4096);
    output.push_str("{\"schema\":");
    output.push_str(&quote_json(descriptor.schema));
    output.push_str(",\"project_schema\":");
    output.push_str(&quote_json(descriptor.project_schema));
    output.push_str(",\"project_revision\":");
    output.push_str(&quote_json(&descriptor.project_revision));
    output.push_str(",\"workspace_revision\":");
    output.push_str(&quote_json(&descriptor.workspace_revision));
    output.push_str(",\"project_graph_digest\":");
    output.push_str(&quote_json(&descriptor.project_graph_digest));
    output.push_str(",\"exports\":[");
    for (export_index, export) in descriptor.exports.iter().enumerate() {
        if export_index != 0 {
            output.push(',');
        }
        output.push_str("{\"stable_id\":");
        output.push_str(&quote_json(export.stable_id.as_str()));
        output.push_str(",\"typescript_name\":");
        output.push_str(&quote_json(&export.typescript_name));
        output.push_str(",\"rust_method_name\":");
        output.push_str(&quote_json(&export.rust_method_name));
        output.push_str(",\"parameters\":[");
        for (parameter_index, parameter) in export.parameters.iter().enumerate() {
            if parameter_index != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            output.push_str(&quote_json(parameter.stable_id.as_str()));
            output.push_str(",\"source_name\":");
            output.push_str(&quote_json(&parameter.source_name));
            output.push_str(",\"ordinal\":");
            output.push_str(&parameter_index.to_string());
            output.push_str(",\"type\":");
            output.push_str(&quote_json(parameter.ty.wire_name()));
            output.push('}');
        }
        output.push_str("],\"result\":");
        output.push_str(&quote_json(export.result.wire_name()));
        output.push('}');
    }
    output.push_str("],\"limits\":{");
    output.push_str(&format!(
        "\"max_exports\":{},\"max_parameters\":{},\"max_closure_functions\":{},\"max_borrowed_input_bytes\":{},\"max_owned_output_bytes\":{},\"max_descriptor_bytes\":{}",
        descriptor.limits.max_exports,
        descriptor.limits.max_parameters,
        descriptor.limits.max_closure_functions,
        descriptor.limits.max_borrowed_input_bytes,
        descriptor.limits.max_owned_output_bytes,
        descriptor.limits.max_descriptor_bytes,
    ));
    output.push_str("}}\n");
    output
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedParameter {
    stable_id: String,
    source_name: String,
    ty: PublicApiParameterType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedExport {
    stable_id: String,
    typescript_name: String,
    rust_method_name: String,
    parameters: Vec<ParsedParameter>,
    result: PublicApiResultType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedDescriptor {
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    exports: Vec<ParsedExport>,
}

impl ParsedDescriptor {
    fn from_descriptor(descriptor: &PublicApiDescriptor) -> Self {
        Self {
            project_revision: descriptor.project_revision.clone(),
            workspace_revision: descriptor.workspace_revision.clone(),
            project_graph_digest: descriptor.project_graph_digest.clone(),
            exports: descriptor
                .exports
                .iter()
                .map(|export| ParsedExport {
                    stable_id: export.stable_id.as_str().to_owned(),
                    typescript_name: export.typescript_name.clone(),
                    rust_method_name: export.rust_method_name.clone(),
                    parameters: export
                        .parameters
                        .iter()
                        .map(|parameter| ParsedParameter {
                            stable_id: parameter.stable_id.as_str().to_owned(),
                            source_name: parameter.source_name.clone(),
                            ty: parameter.ty,
                        })
                        .collect(),
                    result: export.result,
                })
                .collect(),
        }
    }
}

fn parse_descriptor(bytes: &[u8]) -> Result<ParsedDescriptor, Diagnostic> {
    if bytes.is_empty()
        || bytes.len() > MAX_PUBLIC_API_DESCRIPTOR_BYTES
        || !bytes.ends_with(b"\n")
        || bytes.contains(&0)
    {
        return Err(api_error("public API descriptor bytes are not canonical"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| api_error("public API descriptor JSON is malformed"))?;
    let root = exact_object(&value, 7, "public API descriptor root")?;
    if string(root, "schema")? != PUBLIC_OWNED_DATA_API_SCHEMA
        || string(root, "project_schema")? != PUBLIC_OWNED_DATA_PROJECT_SCHEMA
    {
        return Err(api_error("public API descriptor schema is unsupported"));
    }
    let project_revision = string(root, "project_revision")?.to_owned();
    let workspace_revision = string(root, "workspace_revision")?.to_owned();
    let project_graph_digest = string(root, "project_graph_digest")?.to_owned();
    for value in [
        project_revision.as_str(),
        workspace_revision.as_str(),
        project_graph_digest.as_str(),
    ] {
        if !valid_sha256_fact(value) {
            return Err(api_error("public API descriptor subject fact is invalid"));
        }
    }
    parse_limits(
        root.get("limits")
            .ok_or_else(|| api_error("missing limits"))?,
    )?;
    let rows = root
        .get("exports")
        .and_then(Value::as_array)
        .filter(|rows| (1..=MAX_PUBLIC_API_EXPORTS).contains(&rows.len()))
        .ok_or_else(|| api_error("public API descriptor export count is invalid"))?;
    let mut exports = Vec::with_capacity(rows.len());
    let mut previous: Option<&str> = None;
    let mut rust_names = BTreeSet::new();
    for row in rows {
        let row = exact_object(row, 5, "public API export")?;
        let stable_id = string(row, "stable_id")?;
        if !valid_stable_id(stable_id)
            || previous.is_some_and(|value| value.as_bytes() >= stable_id.as_bytes())
        {
            return Err(api_error(
                "public API descriptor exports are not strictly ordered",
            ));
        }
        previous = Some(stable_id);
        let typescript_name = string(row, "typescript_name")?;
        let rust_name = string(row, "rust_method_name")?;
        if typescript_name != stable_id || rust_name != rust_method_name(stable_id)? {
            return Err(api_error("public API host method identity is invalid"));
        }
        if !rust_names.insert(rust_name.to_owned()) {
            return Err(api_error("public API Rust method identities collide"));
        }
        let parameters = row
            .get("parameters")
            .and_then(Value::as_array)
            .filter(|values| values.len() <= MAX_PUBLIC_API_PARAMETERS)
            .ok_or_else(|| api_error("public API parameter count is invalid"))?
            .iter()
            .enumerate()
            .map(|(ordinal, parameter)| parse_parameter(parameter, ordinal))
            .collect::<Result<Vec<_>, _>>()?;
        if parameters
            .iter()
            .map(|parameter| parameter.stable_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != parameters.len()
        {
            return Err(api_error(
                "public API parameter identities must be unique within an export",
            ));
        }
        let result = PublicApiResultType::parse(string(row, "result")?)
            .ok_or_else(|| api_error("public API result type is invalid"))?;
        exports.push(ParsedExport {
            stable_id: stable_id.to_owned(),
            typescript_name: typescript_name.to_owned(),
            rust_method_name: rust_name.to_owned(),
            parameters,
            result,
        });
    }
    let parsed = ParsedDescriptor {
        project_revision,
        workspace_revision,
        project_graph_digest,
        exports,
    };
    if render_parsed(&parsed).as_bytes() != bytes {
        return Err(api_error("public API descriptor is not canonical"));
    }
    Ok(parsed)
}

fn parse_parameter(value: &Value, ordinal: usize) -> Result<ParsedParameter, Diagnostic> {
    let row = exact_object(value, 4, "public API parameter")?;
    let stable_id = string(row, "stable_id")?;
    let source_name = string(row, "source_name")?;
    if stable_id.is_empty()
        || stable_id.len() > MAX_PARAMETER_ID_BYTES
        || stable_id.chars().any(char::is_control)
        || source_name.is_empty()
        || source_name.len() > MAX_SOURCE_NAME_BYTES
        || source_name.chars().any(char::is_control)
        || row.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
    {
        return Err(api_error("public API parameter identity is invalid"));
    }
    let ty = PublicApiParameterType::parse(string(row, "type")?)
        .ok_or_else(|| api_error("public API parameter type is invalid"))?;
    Ok(ParsedParameter {
        stable_id: stable_id.to_owned(),
        source_name: source_name.to_owned(),
        ty,
    })
}

fn parse_limits(value: &Value) -> Result<(), Diagnostic> {
    let row = exact_object(value, 6, "public API limits")?;
    for (key, expected) in [
        ("max_exports", MAX_PUBLIC_API_EXPORTS),
        ("max_parameters", MAX_PUBLIC_API_PARAMETERS),
        ("max_closure_functions", MAX_PUBLIC_API_CLOSURE_FUNCTIONS),
        (
            "max_borrowed_input_bytes",
            MAX_PUBLIC_API_BORROWED_INPUT_BYTES,
        ),
        ("max_owned_output_bytes", MAX_PUBLIC_API_OWNED_OUTPUT_BYTES),
        ("max_descriptor_bytes", MAX_PUBLIC_API_DESCRIPTOR_BYTES),
    ] {
        if row.get(key).and_then(Value::as_u64) != Some(expected as u64) {
            return Err(api_error("public API descriptor limits are invalid"));
        }
    }
    Ok(())
}

fn render_parsed(parsed: &ParsedDescriptor) -> String {
    let mut output = String::with_capacity(4096);
    output.push_str("{\"schema\":");
    output.push_str(&quote_json(PUBLIC_OWNED_DATA_API_SCHEMA));
    output.push_str(",\"project_schema\":");
    output.push_str(&quote_json(PUBLIC_OWNED_DATA_PROJECT_SCHEMA));
    output.push_str(",\"project_revision\":");
    output.push_str(&quote_json(&parsed.project_revision));
    output.push_str(",\"workspace_revision\":");
    output.push_str(&quote_json(&parsed.workspace_revision));
    output.push_str(",\"project_graph_digest\":");
    output.push_str(&quote_json(&parsed.project_graph_digest));
    output.push_str(",\"exports\":[");
    for (export_index, export) in parsed.exports.iter().enumerate() {
        if export_index != 0 {
            output.push(',');
        }
        output.push_str("{\"stable_id\":");
        output.push_str(&quote_json(&export.stable_id));
        output.push_str(",\"typescript_name\":");
        output.push_str(&quote_json(&export.typescript_name));
        output.push_str(",\"rust_method_name\":");
        output.push_str(&quote_json(&export.rust_method_name));
        output.push_str(",\"parameters\":[");
        for (ordinal, parameter) in export.parameters.iter().enumerate() {
            if ordinal != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            output.push_str(&quote_json(&parameter.stable_id));
            output.push_str(",\"source_name\":");
            output.push_str(&quote_json(&parameter.source_name));
            output.push_str(",\"ordinal\":");
            output.push_str(&ordinal.to_string());
            output.push_str(",\"type\":");
            output.push_str(&quote_json(parameter.ty.wire_name()));
            output.push('}');
        }
        output.push_str("],\"result\":");
        output.push_str(&quote_json(export.result.wire_name()));
        output.push('}');
    }
    output.push_str("],\"limits\":{");
    output.push_str(&format!(
        "\"max_exports\":{},\"max_parameters\":{},\"max_closure_functions\":{},\"max_borrowed_input_bytes\":{},\"max_owned_output_bytes\":{},\"max_descriptor_bytes\":{}",
        MAX_PUBLIC_API_EXPORTS,
        MAX_PUBLIC_API_PARAMETERS,
        MAX_PUBLIC_API_CLOSURE_FUNCTIONS,
        MAX_PUBLIC_API_BORROWED_INPUT_BYTES,
        MAX_PUBLIC_API_OWNED_OUTPUT_BYTES,
        MAX_PUBLIC_API_DESCRIPTOR_BYTES,
    ));
    output.push_str("}}\n");
    output
}

fn exact_object<'a>(
    value: &'a Value,
    fields: usize,
    label: &str,
) -> Result<&'a Map<String, Value>, Diagnostic> {
    value
        .as_object()
        .filter(|object| object.len() == fields)
        .ok_or_else(|| api_error(format!("{label} has an invalid shape")))
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| api_error(format!("public API descriptor field `{key}` is invalid")))
}

fn valid_stable_id(value: &str) -> bool {
    (1..=MAX_STABLE_ID_BYTES).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn valid_sha256_fact(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn api_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-J113", message)
}
