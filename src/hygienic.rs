//! Deterministic, read-only Typed Hygienic Generation v1.
//!
//! This tranche synthesizes typed [`crate::ast`] declarations for verified,
//! single-file programs and proves them through the crate's real pipeline:
//! the combined program (original plus generated functions) is re-verified
//! through [`crate::verify`] and projected as facts through the unchanged
//! Graph module ([`crate::graph`]). Generated identifiers derive
//! deterministically from persistent declaration stable IDs, live under the
//! reserved `__gen_` prefix, and any collision with an existing program
//! symbol fails closed. There is no textual rewriting channel: generated
//! source text exists only as the output of [`crate::format::canonical`]
//! applied to the synthesized AST, and the report binds its digest.
//!
//! Sandbox contract: the template registry is a closed set of enumerated
//! constants. Generation reads exactly one source snapshot and consults no
//! environment, filesystem state beyond that snapshot, clock, randomness, or
//! network. Unknown template IDs are rejected before any synthesis runs.
//!
//! Diagnostics allocated by this module:
//!
//! - `SPX-Y100`: invalid command options (unknown or duplicate template,
//!   missing selection, out-of-bounds byte budget).
//! - `SPX-Y101`: reserved for a wrapped base-program parse or verification
//!   rejection; this route surfaces the underlying parser and verifier
//!   diagnostics unchanged instead of wrapping them, so the code currently
//!   has no emitter.
//! - `SPX-Y102`: a derived generated name collides with an existing program
//!   symbol or with another derived name; generation fails closed.
//! - `SPX-Y103`: an existing program symbol already uses the reserved
//!   `__gen_` prefix; generation fails closed.
//! - `SPX-Y104`: the combined program was rejected by the real verifier;
//!   generation fails closed (internal invariant).
//! - `SPX-Y105`: the mandatory report envelope cannot fit the requested
//!   byte budget; generation fails closed instead of emitting a partial
//!   inventory.
//! - `SPX-Y106`: the combined Graph projection did not resolve every
//!   generated function identity; generation fails closed.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::ast::{
    Expr, ExprKind, FieldInitializer, Function, Param, ParamMode, Program, Span, Type,
    TypeDeclaration, TypeDeclarationKind,
};
use crate::bounded_output::{with_limit_usage, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{format, graph, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.hygienic-gen.v1";

/// Reserved identifier prefix owned by this generator. Existing program
/// symbols may not use it; every generated declaration lives under it.
pub const RESERVED_PREFIX: &str = "__gen_";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;
/// Exact bytes of the `,"outer_sha256":"sha256:<64 hex>"}` suffix appended
/// after the payload's final `}` is trimmed (net envelope delta: +89).
const RESERVE_OUTER_BYTES: usize = 90;
const MAX_SCAN_STEPS: usize = 200_000;

const OUTER_DIGEST_DOMAIN: &[u8] = b"semaprax.hygienic-gen.v1:outer-digest.v1\0";
const NAME_DIGEST_DOMAIN: &[u8] = b"semaprax.hygienic-gen.v1:name-digest.v1\0";
const FORMATTED_DIGEST_DOMAIN: &[u8] = b"semaprax.hygienic-gen.v1:formatted.v1\0";
const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.hygienic-gen.v1:source.v1\0";

const REASON_RESOURCE_DECLARATION: &str = "resource_declaration";
const REASON_VARIANT_DECLARATION: &str = "variant_declaration";
const REASON_INTERFACE_DECLARATION: &str = "interface_declaration";
const REASON_GENERIC_RECORD: &str = "generic_record";
const REASON_NON_SCALAR_FIELD: &str = "non_scalar_field";
const REASON_EMPTY_RECORD: &str = "empty_record";

const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";
const REASON_SCAN_STEP_BUDGET_EXHAUSTED: &str = "scan_step_budget_exhausted";
const REASON_FLOAT_LITERAL: &str = "float_literal";
const REASON_CHAR_LITERAL: &str = "char_literal";
const REASON_RECORD_CONSTRUCTION: &str = "record_construction";
const REASON_VARIANT_CONSTRUCTION: &str = "variant_construction";
const REASON_RECORD_UPDATE: &str = "record_update";
const REASON_RECORD_PROJECTION: &str = "record_projection";
const REASON_MATCH_EXPRESSION: &str = "match_expression";
const REASON_TRY_EXPRESSION: &str = "try_expression";
const REASON_GENERIC_CALL: &str = "generic_call";
const REASON_UNSUPPORTED_CALLEE: &str = "unsupported_callee";
const REASON_CLASS_DECLARATION: &str = "class_declaration";
const REASON_METHOD_CALL: &str = "method_call";

const TRUNCATION_BYTE_BUDGET: &str = "byte_budget";

const REGISTRY_JSON: &str = "[\"default-constructor\",\"field-accessors\"]";

/// The closed nonclaim surface of this feature, in canonical report order.
pub const NONCLAIMS: [&str; 6] = [
    "no_unrestricted_textual_rewriting",
    "no_macro_system",
    "no_cross_file_scope",
    "read_only_no_source_mutation",
    "no_persistent_artifacts",
    "no_target_execution",
];

fn nonclaims_json() -> String {
    NONCLAIMS
        .iter()
        .map(|claim| bformat!("\"{claim}\""))
        .collect::<Vec<_>>()
        .budgeted_join(",")
        .to_string()
}

/// One member of the closed generation template registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Template {
    /// `<prefix><Record>_<identity>_default() -> Record`, zero-literal body.
    DefaultConstructor,
    /// `<prefix><Record>_<identity>_get_<field>(value: Record) -> <scalar>`.
    FieldAccessors,
}

impl Template {
    /// The complete closed registry, in canonical order.
    pub const REGISTRY: [Template; 2] = [Self::DefaultConstructor, Self::FieldAccessors];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::DefaultConstructor => "default-constructor",
            Self::FieldAccessors => "field-accessors",
        }
    }

    /// Parse one exact registry ID. Unknown or case-folded IDs are rejected;
    /// this is the only string-to-template boundary in the tranche.
    #[must_use]
    pub fn from_id(name: &str) -> Option<Self> {
        Self::REGISTRY
            .into_iter()
            .find(|template| template.id() == name)
    }
}

/// Validated deterministic limits for one generation run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HygienicGenOptions {
    templates: Vec<Template>,
    max_bytes: usize,
}

impl HygienicGenOptions {
    /// Validate a template subset and byte budget. The selection is stored in
    /// canonical registry order regardless of input order.
    pub fn new(templates: &[Template], max_bytes: usize) -> Result<Self, Diagnostic> {
        if templates.is_empty() {
            return Err(option_error(
                "hygienic generation requires at least one registry template",
            ));
        }
        let mut selected = BTreeSet::new();
        for template in templates {
            if !selected.insert(*template) {
                return Err(option_error(format!(
                    "duplicate hygienic generation template `{}`",
                    template.id()
                )));
            }
        }
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "hygienic generation max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self {
            templates: selected.into_iter().collect(),
            max_bytes,
        })
    }

    /// The validated template selection, in canonical registry order.
    #[must_use]
    pub fn templates(&self) -> &[Template] {
        &self.templates
    }

    /// The validated whole-report byte budget.
    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }
}

impl Default for HygienicGenOptions {
    fn default() -> Self {
        Self {
            templates: Template::REGISTRY.to_vec(),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-Y100", message.into())
}

fn bound_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-Y105", message.into())
}

fn invariant_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-Y106", message.into())
}

fn hygiene_error(code: &'static str, message: String, span: Span) -> Diagnostic {
    Diagnostic::error(code, message, span)
}

/// The reserved-prefix identity digest embedded in every derived name: the
/// first four bytes of the domain-separated SHA-256 over the persistent
/// stable ID, as eight lowercase hex characters.
#[must_use]
pub fn identity_digest_hex(stable_id: &str) -> String {
    use std::fmt::Write as _;
    let mut hasher = Sha256::new();
    hasher.update(NAME_DIGEST_DOMAIN);
    hasher.update(stable_id.as_bytes());
    let digest = hasher.finalize();
    let mut output = String::with_capacity(8);
    for byte in &digest[..4] {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// Derive the reserved-prefix default-constructor name for one record.
///
/// The name is a pure function of the record's persistent stable ID, so
/// renaming the record while keeping its `@id` keeps every derived name, and
/// moving code changes nothing.
#[must_use]
pub fn default_constructor_name(stable_id: &str) -> String {
    format!(
        "{RESERVED_PREFIX}{}_default",
        identity_digest_hex(stable_id)
    )
}

/// Derive the reserved-prefix field-accessor name for one record field.
///
/// Like [`default_constructor_name`], the identity component derives solely
/// from the owning record's persistent stable ID; the accessor suffix binds
/// the field identifier it projects.
#[must_use]
pub fn accessor_name(stable_id: &str, field_name: &str) -> String {
    format!(
        "{RESERVED_PREFIX}{}_get_{}",
        identity_digest_hex(stable_id),
        field_name
    )
}

fn source_digest(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_DIGEST_DOMAIN);
    hasher.update((source.len() as u64).to_le_bytes());
    hasher.update(source.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Generate the canonical `semaprax.hygienic-gen.v1` report for one verified
/// source file. Read-only: the source must remain unchanged for the whole
/// invocation or the run fails closed.
pub fn generate(
    source_path: &Path,
    options: &HygienicGenOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = crate::parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    let inventory = collect_inventory(&program)?;
    enforce_hygiene(&program, &inventory)?;
    let artifacts = synthesize(&inventory, options.templates(), &program.module);
    let combined = combine(&program, &artifacts);
    let rejected = verify::verify(&combined)
        .into_iter()
        .filter(|item| item.severity.is_error())
        .count();
    if rejected > 0 {
        return Err(vec![Diagnostic::io(
            "SPX-Y104",
            format!(
                "combined program rejected by verification with {rejected} error(s); \
                 typed generation fails closed"
            ),
        )
        .at_path(program.path.clone())]);
    }

    let base_facts = graph_facts(&program)?;
    let combined_facts = graph_facts(&combined)?;
    let identities = generated_identities(&combined_facts.nodes, &artifacts)?;
    let digests = formatted_digests(&combined.module, &artifacts);

    let context = RenderContext {
        options,
        path_json: &quote_json(&source_path.display().to_string()),
        base_revision_json: &quote_json(&revision),
        source_json: &quote_json(&source_digest(snapshot.source())),
        inventory: &inventory,
        identities: &identities,
        digests: &digests,
        base_facts: &base_facts,
        combined_facts: &combined_facts,
    };
    let report = render_bounded(&context, &artifacts)?;
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(report)
}

struct AdmittedRecord {
    name: String,
    stable_id: String,
    fields: Vec<(String, Type)>,
}

struct Inventory {
    types_total: usize,
    records: Vec<AdmittedRecord>,
    excluded_types_json: Vec<String>,
    functions_total: usize,
    functions_admitted: usize,
    excluded_functions_json: Vec<String>,
}

fn is_scalar(ty: &Type) -> bool {
    matches!(ty, Type::I64 | Type::Bool)
}

fn scalar_type_text(ty: &Type) -> &'static str {
    match ty {
        Type::Bool => "bool",
        _ => "i64",
    }
}

fn excluded_type_json(declaration: &TypeDeclaration, kind: &str, reason: &str) -> String {
    bformat!(
        "{{\"name\":{},\"kind\":\"{kind}\",\"reason\":\"{reason}\"}}",
        quote_json(&declaration.name),
    )
}

fn excluded_function_json(function: &Function, reason: &str) -> String {
    bformat!(
        "{{\"stable_id\":{},\"name\":{},\"reason\":\"{reason}\"}}",
        quote_json(&function.stable_id),
        quote_json(&function.name),
    )
}

struct ScanState<'a> {
    steps: usize,
    admitted_callees: &'a BTreeSet<&'a str>,
}

impl ScanState<'_> {
    fn scan(&mut self, expression: &Expr) -> Option<&'static str> {
        self.steps += 1;
        if self.steps >= MAX_SCAN_STEPS {
            return Some(REASON_SCAN_STEP_BUDGET_EXHAUSTED);
        }
        match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Uint8(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => None,
            ExprKind::Float32(_) | ExprKind::Float64(_) => Some(REASON_FLOAT_LITERAL),
            ExprKind::Char(_) => Some(REASON_CHAR_LITERAL),
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } => {
                if !type_arguments.is_empty() {
                    return Some(REASON_GENERIC_CALL);
                }
                // Verified programs resolve every call to a local function or
                // an interface import; only calls into signature-admitted
                // local scalar functions stay inside the stable core.
                if !self.admitted_callees.contains(name.as_str()) {
                    return Some(REASON_UNSUPPORTED_CALLEE);
                }
                args.iter().find_map(|argument| self.scan(argument))
            }
            ExprKind::Unary { value, .. } => self.scan(value),
            ExprKind::Binary { left, right, .. } => self.scan(left).or_else(|| self.scan(right)),
            ExprKind::Block { statements, tail } => statements
                .iter()
                .find_map(|statement| self.scan(statement.value()))
                .or_else(|| self.scan(tail)),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self
                .scan(condition)
                .or_else(|| self.scan(then_branch))
                .or_else(|| self.scan(else_branch)),
            ExprKind::ConstructRecord { .. } => Some(REASON_RECORD_CONSTRUCTION),
            ExprKind::ConstructVariant { .. } => Some(REASON_VARIANT_CONSTRUCTION),
            ExprKind::UpdateRecord { .. } => Some(REASON_RECORD_UPDATE),
            ExprKind::Project { .. } => Some(REASON_RECORD_PROJECTION),
            ExprKind::MethodCall { .. } | ExprKind::SuperMethod { .. } => Some(REASON_METHOD_CALL),
            ExprKind::Match { .. } => Some(REASON_MATCH_EXPRESSION),
            ExprKind::Try { .. } => Some(REASON_TRY_EXPRESSION),
        }
    }

    fn scan_function(&mut self, function: &Function) -> Option<&'static str> {
        for clause in function.requires.iter().chain(function.ensures.iter()) {
            if let Some(reason) = self.scan(clause) {
                return Some(reason);
            }
        }
        self.scan(&function.body)
    }
}

fn function_signature_exclusion(function: &Function) -> Option<&'static str> {
    if !function.type_parameters.is_empty() {
        return Some(REASON_GENERIC_FUNCTION);
    }
    if !function.effects.is_empty() {
        return Some(REASON_DECLARED_EFFECTS);
    }
    for param in &function.params {
        if param.mode != ParamMode::Value {
            return Some(REASON_UNSUPPORTED_PARAMETER_MODE);
        }
        if !is_scalar(&param.ty) {
            return Some(REASON_UNSUPPORTED_PARAMETER_TYPE);
        }
    }
    if !is_scalar(&function.return_type) {
        return Some(REASON_UNSUPPORTED_RESULT_TYPE);
    }
    None
}

fn collect_inventory(program: &Program) -> Result<Inventory, Vec<Diagnostic>> {
    let types_total = program.types.len() + program.interfaces.len();
    let mut records = Vec::new();
    let mut excluded_types_json = Vec::new();
    for declaration in &program.types {
        match &declaration.kind {
            TypeDeclarationKind::Resource { .. } => {
                excluded_types_json.push(excluded_type_json(
                    declaration,
                    "resource",
                    REASON_RESOURCE_DECLARATION,
                ));
            }
            TypeDeclarationKind::Variant { .. } => {
                excluded_types_json.push(excluded_type_json(
                    declaration,
                    "variant",
                    REASON_VARIANT_DECLARATION,
                ));
            }
            TypeDeclarationKind::Class { .. } => {
                excluded_types_json.push(excluded_type_json(
                    declaration,
                    "class",
                    REASON_CLASS_DECLARATION,
                ));
            }
            TypeDeclarationKind::Record { fields } => {
                if !declaration.type_parameters.is_empty() {
                    excluded_types_json.push(excluded_type_json(
                        declaration,
                        "record",
                        REASON_GENERIC_RECORD,
                    ));
                } else if fields.is_empty() {
                    excluded_types_json.push(excluded_type_json(
                        declaration,
                        "record",
                        REASON_EMPTY_RECORD,
                    ));
                } else if !fields.iter().all(|field| is_scalar(&field.ty)) {
                    excluded_types_json.push(excluded_type_json(
                        declaration,
                        "record",
                        REASON_NON_SCALAR_FIELD,
                    ));
                } else {
                    records.push(AdmittedRecord {
                        name: declaration.name.clone(),
                        stable_id: declaration.stable_id.clone(),
                        fields: fields
                            .iter()
                            .map(|field| (field.name.clone(), field.ty.clone()))
                            .collect(),
                    });
                }
            }
        }
    }
    for interface in &program.interfaces {
        excluded_types_json.push(bformat!(
            "{{\"name\":{},\"kind\":\"interface\",\"reason\":\"{REASON_INTERFACE_DECLARATION}\"}}",
            quote_json(&interface.name),
        ));
    }

    // Signature admission is computed once up front so callee classification
    // is independent of authored declaration order.
    let admitted_callees: BTreeSet<&str> = program
        .functions
        .iter()
        .filter(|function| function_signature_exclusion(function).is_none())
        .map(|function| function.name.as_str())
        .collect();
    let mut excluded_functions_json = Vec::new();
    let mut scanner = ScanState {
        steps: 0,
        admitted_callees: &admitted_callees,
    };
    for function in &program.functions {
        if let Some(reason) =
            function_signature_exclusion(function).or_else(|| scanner.scan_function(function))
        {
            excluded_functions_json.push(excluded_function_json(function, reason));
        }
    }
    let functions_admitted = program.functions.len() - excluded_functions_json.len();
    Ok(Inventory {
        types_total,
        records,
        excluded_types_json,
        functions_total: program.functions.len(),
        functions_admitted,
        excluded_functions_json,
    })
}

fn enforce_hygiene(program: &Program, inventory: &Inventory) -> Result<(), Vec<Diagnostic>> {
    // Exact derived-name collisions are diagnosed first so a source that
    // defines precisely one derived name reports SPX-Y102 rather than the
    // broader reserved-prefix rule below.
    let existing: BTreeSet<&str> = program
        .types
        .iter()
        .map(|declaration| declaration.name.as_str())
        .chain(
            program
                .interfaces
                .iter()
                .map(|interface| interface.name.as_str()),
        )
        .chain(
            program
                .functions
                .iter()
                .map(|function| function.name.as_str()),
        )
        .collect();
    let mut derived: BTreeSet<String> = BTreeSet::new();
    for record in &inventory.records {
        let constructors = std::iter::once(default_constructor_name(&record.stable_id));
        let accessors = record
            .fields
            .iter()
            .map(|(field, _)| accessor_name(&record.stable_id, field));
        for candidate in constructors.chain(accessors) {
            if existing.contains(candidate.as_str()) || !derived.insert(candidate.clone()) {
                return Err(vec![hygiene_error(
                    "SPX-Y102",
                    format!(
                        "generated name `{candidate}` collides with an existing program symbol"
                    ),
                    Span::default(),
                )
                .at_path(program.path.clone())]);
            }
        }
    }
    for symbol in program
        .types
        .iter()
        .map(|declaration| (&declaration.name, declaration.name_span))
        .chain(
            program
                .interfaces
                .iter()
                .map(|interface| (&interface.name, interface.name_span)),
        )
        .chain(
            program
                .functions
                .iter()
                .map(|function| (&function.name, function.name_span)),
        )
    {
        if symbol.0.starts_with(RESERVED_PREFIX) {
            return Err(vec![hygiene_error(
                "SPX-Y103",
                format!(
                    "symbol `{}` uses the reserved generation prefix `{RESERVED_PREFIX}`",
                    symbol.0
                ),
                symbol.1,
            )
            .at_path(program.path.clone())]);
        }
    }
    Ok(())
}

struct GeneratedArtifact {
    template: Template,
    record_name: String,
    record_stable_id: String,
    field_name: Option<String>,
    function: Function,
}

fn zero_literal(ty: &Type, span: Span) -> Expr {
    Expr {
        kind: match ty {
            Type::Bool => ExprKind::Bool(false),
            _ => ExprKind::Int(0),
        },
        span,
    }
}

fn record_type(record: &AdmittedRecord) -> Type {
    Type::Named {
        name: record.name.clone(),
        arguments: vec![],
    }
}

fn block(tail: Expr, span: Span) -> Expr {
    Expr {
        kind: ExprKind::Block {
            statements: vec![],
            tail: Box::new(tail),
        },
        span,
    }
}

#[must_use]
fn build_function(
    module: &str,
    name: String,
    params: Vec<Param>,
    return_type: Type,
    tail: Expr,
) -> Function {
    let span = Span::default();
    Function {
        // Mirrors the parser's automatic identity convention so the combined
        // program keeps unique declaration identities without granting the
        // generated functions persistent public identities.
        stable_id: format!("auto:{module}.{name}"),
        explicit_id: false,
        name,
        name_span: span,
        type_parameters: vec![],
        params,
        return_type,
        effects: vec![],
        requires: vec![],
        ensures: vec![],
        body: block(tail, span),
        span,
    }
}

fn synthesize(
    inventory: &Inventory,
    templates: &[Template],
    module: &str,
) -> Vec<GeneratedArtifact> {
    let mut artifacts = Vec::new();
    for record in &inventory.records {
        for template in templates {
            match template {
                Template::DefaultConstructor => {
                    let name = default_constructor_name(&record.stable_id);
                    let tail = Expr {
                        kind: ExprKind::ConstructRecord {
                            type_name: record.name.clone(),
                            type_span: Span::default(),
                            type_arguments: vec![],
                            fields: record
                                .fields
                                .iter()
                                .map(|(field, ty)| FieldInitializer {
                                    name: field.clone(),
                                    name_span: Span::default(),
                                    value: zero_literal(ty, Span::default()),
                                    span: Span::default(),
                                })
                                .collect(),
                        },
                        span: Span::default(),
                    };
                    artifacts.push(GeneratedArtifact {
                        template: *template,
                        record_name: record.name.clone(),
                        record_stable_id: record.stable_id.clone(),
                        field_name: None,
                        function: build_function(module, name, vec![], record_type(record), tail),
                    });
                }
                Template::FieldAccessors => {
                    for (field, ty) in &record.fields {
                        let name = accessor_name(&record.stable_id, field);
                        let tail = Expr {
                            kind: ExprKind::Project {
                                base: Box::new(Expr {
                                    kind: ExprKind::Var("value".to_owned()),
                                    span: Span::default(),
                                }),
                                field: field.clone(),
                                field_span: Span::default(),
                            },
                            span: Span::default(),
                        };
                        artifacts.push(GeneratedArtifact {
                            template: *template,
                            record_name: record.name.clone(),
                            record_stable_id: record.stable_id.clone(),
                            field_name: Some(field.clone()),
                            function: build_function(
                                module,
                                name,
                                vec![Param {
                                    name: "value".to_owned(),
                                    mode: ParamMode::Value,
                                    ty: record_type(record),
                                    span: Span::default(),
                                }],
                                ty.clone(),
                                tail,
                            ),
                        });
                    }
                }
            }
        }
    }
    artifacts
}

fn combine(program: &Program, artifacts: &[GeneratedArtifact]) -> Program {
    let mut combined = program.clone();
    combined
        .functions
        .extend(artifacts.iter().map(|artifact| artifact.function.clone()));
    combined
}

struct GraphFacts {
    schema: String,
    revision: String,
    nodes: Value,
    function_nodes: usize,
}

fn graph_facts(program: &Program) -> Result<GraphFacts, Vec<Diagnostic>> {
    let json = graph::to_json(program)?;
    let value: Value = serde_json::from_str(&json).map_err(|error| {
        vec![invariant_error(format!(
            "graph projection was not valid JSON: {error}"
        ))]
    })?;
    let schema = value["schema"]
        .as_str()
        .ok_or_else(|| vec![invariant_error("graph projection lacked a schema string")])?
        .to_owned();
    let revision = value["revision"]
        .as_str()
        .ok_or_else(|| vec![invariant_error("graph projection lacked a revision digest")])?
        .to_owned();
    let nodes = value["nodes"].clone();
    let function_nodes = nodes
        .as_array()
        .map(|nodes| {
            nodes
                .iter()
                .filter(|node| node["kind"] == "function")
                .count()
        })
        .unwrap_or_default();
    Ok(GraphFacts {
        schema,
        revision,
        nodes,
        function_nodes,
    })
}

fn generated_identities(
    combined_nodes: &Value,
    artifacts: &[GeneratedArtifact],
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let mut resolved: HashMap<&str, &str> = HashMap::new();
    if let Some(nodes) = combined_nodes.as_array() {
        for node in nodes {
            if node["kind"] == "function" {
                if let (Some(name), Some(id)) = (node["name"].as_str(), node["id"].as_str()) {
                    if name.starts_with(RESERVED_PREFIX) {
                        resolved.insert(name, id);
                    }
                }
            }
        }
    }
    artifacts
        .iter()
        .map(|artifact| {
            resolved
                .get(artifact.function.name.as_str())
                .copied()
                .map(str::to_owned)
                .ok_or_else(|| {
                    vec![invariant_error(format!(
                        "combined graph projection lacked resolved identity for `{}`",
                        artifact.function.name
                    ))]
                })
        })
        .collect()
}

fn formatted_digest(module: &str, artifact: &GeneratedArtifact) -> String {
    // The single-declaration program exists only so the crate's own canonical
    // formatter produces the text; its output is digested, never reparsed.
    let single = Program {
        path: artifact.function.name.clone(),
        module: module.to_owned(),
        module_uses: vec![],
        permits: vec![],
        types: vec![],
        interfaces: vec![],
        protocols: vec![],
        functions: vec![artifact.function.clone()],
    };
    let text = format::canonical(&single);
    let mut hasher = Sha256::new();
    hasher.update(FORMATTED_DIGEST_DOMAIN);
    hasher.update((text.len() as u64).to_le_bytes());
    hasher.update(text.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn formatted_digests(module: &str, artifacts: &[GeneratedArtifact]) -> Vec<String> {
    artifacts
        .iter()
        .map(|artifact| formatted_digest(module, artifact))
        .collect()
}

struct RenderContext<'a> {
    options: &'a HygienicGenOptions,
    path_json: &'a str,
    base_revision_json: &'a str,
    source_json: &'a str,
    inventory: &'a Inventory,
    identities: &'a [String],
    digests: &'a [String],
    base_facts: &'a GraphFacts,
    combined_facts: &'a GraphFacts,
}

fn render_bounded(
    context: &RenderContext<'_>,
    artifacts: &[GeneratedArtifact],
) -> Result<String, Vec<Diagnostic>> {
    let entries: Vec<String> = artifacts
        .iter()
        .zip(context.identities)
        .zip(context.digests)
        .map(|((artifact, identity), digest)| entry_json(artifact, identity, digest))
        .collect();

    let total_entries = entries.len();
    let byte_budget = context
        .options
        .max_bytes()
        .saturating_sub(RESERVE_OUTER_BYTES);
    let render = |count: usize, dropped: usize, reasons: &[&'static str]| -> (String, bool) {
        let (payload, overflowed, _) = with_limit_usage(byte_budget, || {
            render_payload(context, &entries[..count], dropped, reasons, total_entries)
        });
        (payload, overflowed)
    };

    let (payload, overflowed) = render(total_entries, 0, &[]);
    let payload = if overflowed {
        let reasons = [TRUNCATION_BYTE_BUDGET];
        let mut low = 0usize;
        let mut high = total_entries;
        let mut best: Option<String> = None;
        while low <= high {
            let middle = (low + high) / 2;
            let (candidate, still_over) = render(middle, total_entries - middle, &reasons);
            if still_over {
                if middle == 0 {
                    break;
                }
                high = middle - 1;
            } else {
                best = Some(candidate);
                if middle == total_entries {
                    break;
                }
                low = middle + 1;
            }
        }
        best.ok_or_else(|| {
            vec![bound_error(format!(
                "hygienic generation report envelope exceeds the {} byte budget; \
                 failing closed",
                context.options.max_bytes()
            ))]
        })?
    } else {
        payload
    };

    let outer_digest = domain_digest(OUTER_DIGEST_DOMAIN, payload.as_bytes());
    let trimmed = &payload[..payload.len() - 1];
    Ok(bformat!("{trimmed},\"outer_sha256\":\"{outer_digest}\"}}"))
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn entry_json(artifact: &GeneratedArtifact, identity: &str, digest: &str) -> String {
    let field_json = artifact
        .field_name
        .as_deref()
        .map_or_else(|| "null".to_owned(), quote_json);
    let (params, result, tail) = match artifact.template {
        Template::DefaultConstructor => (0usize, artifact.record_name.clone(), "construct_record"),
        Template::FieldAccessors => (1usize, artifact.function.return_type.to_string(), "project"),
    };
    bformat!(
        "{{\"template\":\"{}\",\"record\":{},\"record_stable_id\":{},\"field\":{field_json},\
\"name\":{},\"resolved_id\":{},\"formatted_sha256\":{},\
\"ast\":{{\"params\":{params},\"result\":{},\"tail\":\"{tail}\"}}}}",
        artifact.template.id(),
        quote_json(&artifact.record_name),
        quote_json(&artifact.record_stable_id),
        quote_json(&artifact.function.name),
        quote_json(identity),
        quote_json(digest),
        quote_json(&result),
    )
}

fn render_payload(
    context: &RenderContext<'_>,
    entries: &[String],
    byte_dropped: usize,
    reasons: &[&'static str],
    total_entries: usize,
) -> String {
    let truncated = byte_dropped > 0 || !reasons.is_empty();
    let reasons_json = reasons
        .iter()
        .map(|reason| bformat!("\"{reason}\""))
        .collect::<Vec<_>>();
    let templates_json = context
        .options
        .templates()
        .iter()
        .map(|template| bformat!("\"{}\"", template.id()))
        .collect::<Vec<_>>();
    let admitted_records_json = context
        .inventory
        .records
        .iter()
        .map(admitted_record_json)
        .collect::<Vec<_>>();
    bformat!(
        "{{\"schema\":\"{SCHEMA}\",\"source\":{{\"path\":{0},\"revision\":{1},\
\"sha256\":{2}}},\
\"registry\":{REGISTRY_JSON},\
\"templates\":[{3}],\
\"limits\":{{\"max_bytes\":{4}}},\
\"types\":{{\"total\":{5},\"admitted\":[{6}],\"excluded\":[{7}]}},\
\"functions\":{{\"total\":{8},\"admitted\":{9},\"excluded\":[{10}]}},\
\"generated\":[{11}],\
\"budget\":{{\"generated_total\":{12},\"generated_emitted\":{13}}},\
\"combined\":{{\"base_graph_schema\":{14},\"graph_schema\":{15},\"base_function_nodes\":{16},\
\"function_nodes\":{17},\"base_revision\":{18},\"revision\":{19}}},\
\"truncation\":{{\"truncated\":{20},\"reasons\":[{21}],\"omitted_generated\":{22}}},\
\"nonclaims\":[{23}]}}",
        context.path_json,
        context.base_revision_json,
        context.source_json,
        templates_json.budgeted_join(","),
        context.options.max_bytes(),
        context.inventory.types_total,
        admitted_records_json.budgeted_join(","),
        context.inventory.excluded_types_json.budgeted_join(","),
        context.inventory.functions_total,
        context.inventory.functions_admitted,
        context.inventory.excluded_functions_json.budgeted_join(","),
        entries.budgeted_join(","),
        total_entries,
        entries.len(),
        quote_json(&context.base_facts.schema),
        quote_json(&context.combined_facts.schema),
        context.base_facts.function_nodes,
        context.combined_facts.function_nodes,
        quote_json(&context.base_facts.revision),
        quote_json(&context.combined_facts.revision),
        truncated,
        reasons_json.budgeted_join(","),
        byte_dropped,
        nonclaims_json(),
    )
}

fn admitted_record_json(record: &AdmittedRecord) -> String {
    let fields = record
        .fields
        .iter()
        .map(|(field, ty)| {
            bformat!(
                "{{\"name\":{},\"type\":\"{}\"}}",
                quote_json(field),
                scalar_type_text(ty)
            )
        })
        .collect::<Vec<_>>();
    bformat!(
        "{{\"name\":{},\"stable_id\":{},\"fields\":[{}]}}",
        quote_json(&record.name),
        quote_json(&record.stable_id),
        fields.budgeted_join(","),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(source: &str) -> Program {
        crate::parse(source, "test.spx").unwrap_or_else(|error| {
            panic!("test source must parse: {} ({})", error.code, error.message)
        })
    }

    #[test]
    fn template_registry_is_closed_and_ordered() {
        let ids: Vec<&str> = Template::REGISTRY.iter().map(|t| t.id()).collect();
        assert_eq!(ids, ["default-constructor", "field-accessors"]);
        assert_eq!(
            Template::from_id("default-constructor"),
            Some(Template::DefaultConstructor)
        );
        assert_eq!(
            Template::from_id("field-accessors"),
            Some(Template::FieldAccessors)
        );
        assert_eq!(Template::from_id(""), None);
        assert_eq!(Template::from_id("__gen_"), None);
        assert_eq!(Template::from_id("default-constructor "), None);
    }

    #[test]
    fn options_reject_unknown_duplicate_templates_and_bounds() {
        let err = HygienicGenOptions::new(
            &[Template::DefaultConstructor, Template::DefaultConstructor],
            1024,
        )
        .unwrap_err();
        assert_eq!(err.code, "SPX-Y100");
        let unknown = Template::from_id("nope");
        assert!(unknown.is_none());
        for bad_bytes in [0usize, 1, 99] {
            let err = HygienicGenOptions::new(&[Template::FieldAccessors], bad_bytes).unwrap_err();
            assert_eq!(err.code, "SPX-Y100", "{bad_bytes}");
        }
        let ok =
            HygienicGenOptions::new(&[Template::FieldAccessors], graph::MIN_AGENT_CONTEXT_BYTES)
                .unwrap();
        assert_eq!(ok.max_bytes(), graph::MIN_AGENT_CONTEXT_BYTES);
    }

    #[test]
    fn derived_names_bind_identity_not_display_name() {
        let constructor = default_constructor_name("gen.point");
        assert!(constructor.starts_with(RESERVED_PREFIX));
        assert!(constructor.ends_with("_default"));
        assert_eq!(
            constructor.len(),
            RESERVED_PREFIX.len() + 8 + "_default".len()
        );
        assert_eq!(
            accessor_name("gen.point", "x"),
            format!("{}_get_x", constructor.trim_end_matches("_default")),
        );
        assert_ne!(
            default_constructor_name("gen.point"),
            default_constructor_name("other.point")
        );
        assert_ne!(
            accessor_name("gen.point", "x"),
            accessor_name("gen.point", "flag")
        );
    }

    #[test]
    fn hygiene_scan_flags_user_symbols_matching_derived_names() {
        let derived = default_constructor_name("gen.point");
        let source = format!(
            r#"
module test.gen;

@id("gen.point")
record Point {{
    @id("gen.point.x")
    x: i64,
}}

@id("user.clash")
fn {derived}() -> i64 {{ 0 }}

@id("app.main")
fn main() -> i64 {{ 0 }}
"#
        );
        let program = parse_ok(&source);
        let diagnostics = verify::verify(&program);
        assert!(diagnostics.is_empty(), "test source must verify");
        let inventory = collect_inventory(&program).unwrap();
        let errors = enforce_hygiene(&program, &inventory).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-Y102");
        assert!(errors[0].message.contains(&derived));
    }

    #[test]
    fn envelope_reserve_is_exact_and_digest_is_domain_separated() {
        let payload = "{\"schema\":\"semaprax.hygienic-gen.v1\"}";
        let digest = domain_digest(OUTER_DIGEST_DOMAIN, payload.as_bytes());
        let trimmed = &payload[..payload.len() - 1];
        let envelope = bformat!("{trimmed},\"outer_sha256\":\"{digest}\"}}").to_string();
        assert_eq!(envelope.len(), payload.len() - 1 + RESERVE_OUTER_BYTES);
        assert_eq!(
            &envelope[..payload.len() - 1],
            &payload[..payload.len() - 1]
        );
        assert!(envelope.ends_with(&format!("\",\"outer_sha256\":\"{digest}\"}}")));
        // A different domain over the same bytes must not reproduce the digest.
        assert_ne!(
            domain_digest(NAME_DIGEST_DOMAIN, payload.as_bytes()),
            digest
        );
    }
}
