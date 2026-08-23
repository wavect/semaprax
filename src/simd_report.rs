//! Deterministic, read-only Portable SIMD Eligibility Report v1.
//!
//! [`generate`] projects one verified single-file SEMAPRAX module into one
//! canonical compact JSON envelope (`semaprax.simd-report.v1`): a static
//! vectorization-eligibility analysis per admitted explicit-ID monomorphic
//! effect-free scalar function of the module, derived exclusively from real
//! resolved HIR nodes (`hir::resolve` over the verified program). Per
//! function the report lists every maximal pure straight-line arithmetic
//! sub-expression over `i64`/`i32`/`u8`/`f32`/`f64` whose leaves are plain
//! literals or places, the proposed portable lane width for that expression
//! (2, 4, or 8 by element type ceiling and operation count under a documented
//! deterministic largest-feasible-first rule), the closed portable lane
//! operation sequence in post-order evaluation order, effect-freedom
//! justification facts, and an explicit closed ineligibility reason for every
//! non-covered expression: calls, contracts, division/remainder, boolean
//! mixing, char operations, mutation targets (assignment stores), computed
//! operands, control flow, aggregate operations, and trivial scalar leaves.
//!
//! [`verify_envelope`] independently replays one envelope: exact envelope and
//! payload shape, declared byte count, domain-separated payload digest,
//! module counts, both closed vocabularies, the fixed lane model and
//! portable-operation table, canonical ordering, per-region digests, lane
//! width feasibility against the declared model, effect-freedom facts, and
//! the fixed nonclaims.
//!
//! Diagnostics use the previously unused `SPX-V1xx` family:
//! - `SPX-V101`: invalid options (bounds, malformed values).
//! - `SPX-V102`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-V103`: envelope or HIR-consistency failure.
//!
//! This tranche emits no SIMD codegen or intrinsics, emits no SPIR-V/WebGPU/
//! GPU kernels, makes no autovectorization claim about any backend, executes
//! no target, and changes no source.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ast::{BinaryOp, Function, ParamMode, Type, UnaryOp};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    self, PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType, ValueId,
};
use crate::{format, graph, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.simd-report.v1";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.simd-report.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.simd-report.payload.v1\0";
const REGION_DIGEST_DOMAIN: &[u8] = b"semaprax.simd-report.region.v1\0";

/// Closed function-admission exclusion vocabulary, in canonical bytewise
/// order. The profile admits explicit-ID monomorphic effect-free functions
/// whose by-value parameters and result are all primitive scalars; unlike
/// the ABI/package profile this deliberately includes `i32`, `u8`, `f32`,
/// `f64`, `bool`, and `char` signatures so their bodies can be analyzed and
/// their ineligible expressions reported honestly.
const REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_NON_SCALAR_SIGNATURE: &str = "non_scalar_signature";
const FUNCTION_EXCLUSION_REASONS: [&str; 5] = [
    REASON_AUTOMATIC_IDENTITY,
    REASON_DECLARED_EFFECTS,
    REASON_GENERIC_FUNCTION,
    REASON_NON_SCALAR_SIGNATURE,
    REASON_UNSUPPORTED_PARAMETER_MODE,
];

/// Closed per-expression ineligibility vocabulary, in canonical bytewise
/// order. Every expression node that is neither part of an eligible region
/// nor an operand leaf inside one receives exactly one entry with exactly one
/// reason from this list.
const REASON_AGGREGATE_OPERATION: &str = "aggregate_operation";
const REASON_BOOL_MIXING: &str = "bool_mixing";
const REASON_CALL: &str = "call";
const REASON_CHAR_OPERATION: &str = "char_operation";
const REASON_COMPUTED_OPERAND: &str = "computed_operand";
const REASON_CONTROL_FLOW: &str = "control_flow";
const REASON_CONTRACT: &str = "contract";
const REASON_DIVISION_REMAINDER: &str = "division_remainder";
const REASON_MUTATION_TARGET: &str = "mutation_target";
const REASON_SCALAR_LEAF: &str = "scalar_leaf";
const EXPRESSION_INELIGIBILITY_REASONS: [&str; 10] = [
    REASON_AGGREGATE_OPERATION,
    REASON_BOOL_MIXING,
    REASON_CALL,
    REASON_CHAR_OPERATION,
    REASON_COMPUTED_OPERAND,
    REASON_CONTRACT,
    REASON_CONTROL_FLOW,
    REASON_DIVISION_REMAINDER,
    REASON_MUTATION_TARGET,
    REASON_SCALAR_LEAF,
];

/// Closed effect-freedom justification tokens, in canonical bytewise order.
const JUSTIFICATION_TOKENS: [&str; 3] = [
    "calls_recorded_as_ineligible",
    "declared_effects_empty",
    "no_call_expressions_in_body",
];

/// The fixed portable lane model: a 128-bit lane budget, the only proposed
/// widths {2, 4, 8}, and the per-element-type width ceilings within one
/// 128-bit register (capped at the largest admitted width).
const LANE_MODEL_JSON: &str = "{\"register_bits\":128,\
\"widths\":[2,4,8],\
\"type_ceilings\":[\
{\"element_type\":\"f32\",\"ceiling\":4},\
{\"element_type\":\"f64\",\"ceiling\":2},\
{\"element_type\":\"i32\",\"ceiling\":4},\
{\"element_type\":\"i64\",\"ceiling\":2},\
{\"element_type\":\"u8\",\"ceiling\":8}]}";

/// The complete closed portable lane-operation table, in canonical bytewise
/// order by class then operation. These are portable lane-operation names
/// only; they name no ISA intrinsic and imply no emitted code.
const OPERATION_TABLE_JSON: &str = "[\
{\"class\":\"float\",\"operation\":\"add\",\"portable_op\":\"fp_lane.add\"},\
{\"class\":\"float\",\"operation\":\"mul\",\"portable_op\":\"fp_lane.mul\"},\
{\"class\":\"float\",\"operation\":\"neg\",\"portable_op\":\"fp_lane.neg\"},\
{\"class\":\"float\",\"operation\":\"sub\",\"portable_op\":\"fp_lane.sub\"},\
{\"class\":\"integer\",\"operation\":\"add\",\"portable_op\":\"int_lane.add\"},\
{\"class\":\"integer\",\"operation\":\"mul\",\"portable_op\":\"int_lane.mul\"},\
{\"class\":\"integer\",\"operation\":\"neg\",\"portable_op\":\"int_lane.neg\"},\
{\"class\":\"integer\",\"operation\":\"sub\",\"portable_op\":\"int_lane.sub\"}]";

/// The fixed honest-boundary statement, in canonical bytewise order.
const NONCLAIMS_JSON: &str = "[\"no_autovectorization_claims\",\
\"no_simd_codegen_or_intrinsics_emitted\",\
\"no_spirv_webgpu_or_gpu_kernels\",\
\"no_target_execution\",\
\"read_only_no_source_changes\",\
\"static_eligibility_descriptor_only\"]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimdReportOptions {
    pub max_bytes: usize,
}

impl SimdReportOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "simd-report max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for SimdReportOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-V101", message)
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-V103", message)
}

/// One eligible straight-line arithmetic region.
struct RegionEntry {
    root: String,
    element_type: &'static str,
    operators: usize,
    leaves: usize,
    proposed_width: u8,
    operations: Vec<&'static str>,
}

/// One non-covered expression with its closed reason.
struct IneligibleEntry {
    expr: String,
    reason: &'static str,
}

struct FunctionScan {
    regions: Vec<RegionEntry>,
    ineligible: Vec<IneligibleEntry>,
    call_count: usize,
    assignment_count: usize,
}

struct FunctionEntry {
    stable_id: String,
    name: String,
    signature_element_types: Vec<&'static str>,
    scan: FunctionScan,
}

/// One independently replayed admitted function returned by
/// [`verify_envelope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRegion {
    pub root: String,
    pub proposed_width: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFunction {
    pub stable_id: String,
    pub name: String,
    pub regions: Vec<VerifiedRegion>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifiedSimdReport {
    pub functions: Vec<VerifiedFunction>,
}

/// Generate the canonical `semaprax.simd-report.v1` envelope JSON for one
/// verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(
    source_path: &Path,
    options: &SimdReportOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);
    let resolved = hir::resolve(&program).map_err(|errors| -> Vec<Diagnostic> {
        errors
            .into_iter()
            .map(|error| consistency_error(format!("HIR resolution failed: {error}")))
            .collect()
    })?;

    let mut sorted = program.functions.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    let functions_total = sorted.len();

    let mut excluded: Vec<(&Function, &'static str)> = Vec::new();
    let mut admitted: Vec<&Function> = Vec::new();
    for function in sorted {
        match admission(function) {
            Some(reason) => excluded.push((function, reason)),
            None => admitted.push(function),
        }
    }

    let mut functions: Vec<FunctionEntry> = Vec::with_capacity(admitted.len());
    for function in admitted {
        let resolved_function = resolved
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == function.stable_id)
            .ok_or_else(|| {
                vec![consistency_error(format!(
                    "resolved HIR has no function for stable id `{}`",
                    function.stable_id
                ))]
            })?;
        let mut scan = FunctionScan {
            regions: Vec::new(),
            ineligible: Vec::new(),
            call_count: 0,
            assignment_count: 0,
        };
        // Contracts precede the body in source order; each clause is recorded
        // as one non-covered expression with its canonically rendered text.
        for clause in function.requires.iter().chain(function.ensures.iter()) {
            scan.ineligible.push(IneligibleEntry {
                expr: format::expr(clause, 0),
                reason: REASON_CONTRACT,
            });
        }
        let names = resolved_function
            .params
            .iter()
            .map(|param| (param.id.clone(), param.name.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut walker = Walker {
            declarations: &resolved.declarations,
            names,
            scan: &mut scan,
        };
        walker.scan_expr(&resolved_function.body);
        functions.push(FunctionEntry {
            stable_id: function.stable_id.clone(),
            name: function.name.clone(),
            signature_element_types: function
                .params
                .iter()
                .map(|param| scalar_type_name(&param.ty))
                .chain(std::iter::once(scalar_type_name(&function.return_type)))
                .collect::<Option<Vec<_>>>()
                .unwrap_or_default(),
            scan,
        });
    }

    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();
    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(
            &path_text,
            &revision,
            &digest,
            &program.module,
            options.max_bytes,
            functions_total,
            &functions,
            &excluded,
        )
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-V102",
            "simd-report output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Closed AST-level function admission gate.
fn admission(function: &Function) -> Option<&'static str> {
    if !function.explicit_id {
        return Some(REASON_AUTOMATIC_IDENTITY);
    }
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
        if scalar_type_name(&param.ty).is_none() {
            return Some(REASON_NON_SCALAR_SIGNATURE);
        }
    }
    if scalar_type_name(&function.return_type).is_none() {
        return Some(REASON_NON_SCALAR_SIGNATURE);
    }
    None
}

/// The report's primitive-scalar surface: exactly these element types are
/// analyzed; `bool` and `char` signatures are admitted but their operations
/// can never join a numeric lane region.
fn scalar_type_name(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::I64 => Some("i64"),
        Type::I32 => Some("i32"),
        Type::U8 => Some("u8"),
        Type::F32 => Some("f32"),
        Type::F64 => Some("f64"),
        Type::Bool => Some("bool"),
        Type::Char => Some("char"),
        Type::Named { .. } => None,
    }
}

fn element_type_name(ty: &ResolvedType) -> Option<&'static str> {
    match ty {
        ResolvedType::I64 => Some("i64"),
        ResolvedType::I32 => Some("i32"),
        ResolvedType::U8 => Some("u8"),
        ResolvedType::F32 => Some("f32"),
        ResolvedType::F64 => Some("f64"),
        _ => None,
    }
}

fn is_bool_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Bool)
}

fn is_char_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Char)
}

/// The width ceiling of one lane-eligible element type inside the fixed
/// 128-bit lane model, capped at the largest admitted width.
fn type_ceiling(element_type: &str) -> u8 {
    match element_type {
        "i64" | "f64" => 2,
        "i32" | "f32" => 4,
        "u8" => 8,
        other => unreachable!("non-lane element type `{other}` reached width selection"),
    }
}

/// The portable lane-operation table row for one operator and element class.
fn portable_operation(op: RegionOperator, float: bool) -> &'static str {
    match (op, float) {
        (RegionOperator::Add, false) => "int_lane.add",
        (RegionOperator::Sub, false) => "int_lane.sub",
        (RegionOperator::Mul, false) => "int_lane.mul",
        (RegionOperator::Neg, false) => "int_lane.neg",
        (RegionOperator::Add, true) => "fp_lane.add",
        (RegionOperator::Sub, true) => "fp_lane.sub",
        (RegionOperator::Mul, true) => "fp_lane.mul",
        (RegionOperator::Neg, true) => "fp_lane.neg",
    }
}

#[derive(Clone, Copy)]
enum RegionOperator {
    Add,
    Sub,
    Mul,
    Neg,
}

impl RegionOperator {
    fn from_binary(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Add => Some(Self::Add),
            BinaryOp::Sub => Some(Self::Sub),
            BinaryOp::Mul => Some(Self::Mul),
            _ => None,
        }
    }
}

/// Proposed lane width: scan the closed candidate widths from largest to
/// smallest and take the first feasible one, where feasibility requires
/// `w <= ceiling(element_type)` and `w <= operators + leaves`. Every region
/// has at least one operator and one leaf, so the fallback width 2 is always
/// feasible; the rule is therefore total and deterministic.
fn proposed_width(element_type: &str, operators: usize, leaves: usize) -> u8 {
    let element_count = operators.saturating_add(leaves);
    let ceiling = type_ceiling(element_type);
    [8u8, 4, 2]
        .into_iter()
        .find(|width| *width <= ceiling && usize::from(*width) <= element_count)
        .unwrap_or(2)
}

struct Walker<'a> {
    declarations: &'a hir::DeclarationIndex,
    names: BTreeMap<ValueId, String>,
    scan: &'a mut FunctionScan,
}

impl Walker<'_> {
    fn place_name(&self, root: &ValueId, projections: &[PlaceProjection]) -> String {
        let mut text = self
            .names
            .get(root)
            .cloned()
            .unwrap_or_else(|| "_".to_owned());
        for projection in projections {
            let field = match projection {
                PlaceProjection::Field(field) => field,
                PlaceProjection::VariantField { field, .. } => field,
            };
            let name = self
                .declarations
                .declaration(field)
                .map(|declaration| declaration.name.as_str())
                .unwrap_or("_");
            text.push('.');
            text.push_str(name);
        }
        text
    }

    fn declaration_name(&self, id: &hir::DeclarationId) -> String {
        self.declarations
            .declaration(id)
            .map(|declaration| declaration.name.clone())
            .unwrap_or_else(|| "_".to_owned())
    }

    fn record_call(&mut self) {
        self.scan.call_count += 1;
    }
}

/// Post-order collection of the portable operation sequence of one region.
fn collect_operations(
    expr: &ResolvedExpr,
    float: bool,
    operations: &mut Vec<&'static str>,
    operators: &mut usize,
    leaves: &mut usize,
) {
    match &expr.kind {
        ResolvedExprKind::Binary { op, left, right } => {
            collect_operations(left, float, operations, operators, leaves);
            collect_operations(right, float, operations, operators, leaves);
            let tag = RegionOperator::from_binary(*op).expect("region walk admits only + - *");
            operations.push(portable_operation(tag, float));
            *operators += 1;
        }
        ResolvedExprKind::Unary { op, value } => {
            collect_operations(value, float, operations, operators, leaves);
            let tag = match op {
                UnaryOp::Neg => RegionOperator::Neg,
                UnaryOp::Not => unreachable!("region walk admits only arithmetic negation"),
            };
            operations.push(portable_operation(tag, float));
            *operators += 1;
        }
        _ => *leaves += 1,
    }
}

/// `true` when the subtree consists only of numeric arithmetic operators
/// (`+`/`-`/`*`, unary `-`) over plain literals or projection-free places of
/// the same lane-eligible element type — i.e. it is one pure straight-line
/// region candidate.
fn is_pure_region(expr: &ResolvedExpr, element_type: &str) -> bool {
    match &expr.kind {
        ResolvedExprKind::Binary { op, left, right } => {
            RegionOperator::from_binary(*op).is_some()
                && element_type_name(&expr.ty) == Some(element_type)
                && is_pure_region(left, element_type)
                && is_pure_region(right, element_type)
        }
        ResolvedExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => {
            element_type_name(&expr.ty) == Some(element_type) && is_pure_region(value, element_type)
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_) => true,
        ResolvedExprKind::Place(place) => {
            place.projections.is_empty() && element_type_name(&expr.ty) == Some(element_type)
        }
        _ => false,
    }
}

/// Canonical deterministic rendering of one HIR expression for report text.
/// Parenthesization mirrors the canonical source formatter (`(` around a
/// binary operator whose precedence is below the parent context); literals
/// carry the same explicit suffixes as the canonical formatter.
fn render_expr(
    walker: &Walker<'_>,
    expr: &ResolvedExpr,
    parent_precedence: u8,
    output: &mut String,
) {
    match &expr.kind {
        ResolvedExprKind::Int(number) => {
            output.push_str(&number.to_string());
        }
        ResolvedExprKind::Int32(value) => {
            output.push_str(&value.to_string());
            output.push_str("i32");
        }
        ResolvedExprKind::Uint8(value) => {
            output.push_str(&value.to_string());
            output.push_str("u8");
        }
        ResolvedExprKind::Float32(bits) => {
            output.push_str(&format::canonical_f32_bits(*bits));
            output.push_str("f32");
        }
        ResolvedExprKind::Float64(bits) => {
            output.push_str(&format::canonical_f64_bits(*bits));
        }
        ResolvedExprKind::Bool(value) => {
            output.push_str(if *value { "true" } else { "false" });
        }
        ResolvedExprKind::Char(value) => {
            output.push_str(&format!("char({value})"));
        }
        ResolvedExprKind::Place(place) => {
            output.push_str(&walker.place_name(&place.root, &place.projections));
        }
        ResolvedExprKind::Call { callee, args, .. } => {
            let name = walker.declaration_name(callee);
            output.push_str(&name);
            render_args(walker, args, output);
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let name = walker.declaration_name(&call.import);
            output.push_str(&name);
            output.push_str("(<native-rust-import>)");
        }
        ResolvedExprKind::Unary { op, value } => {
            let precedence = 7u8;
            output.push_str(match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            });
            let inner = render_child(walker, value, precedence);
            output.push_str(&inner);
        }
        ResolvedExprKind::Binary { op, left, right } => {
            let precedence = op.precedence();
            let delimited = precedence < parent_precedence;
            if delimited {
                output.push('(');
            }
            output.push_str(&render_child(walker, left, precedence));
            output.push(' ');
            output.push_str(op.text());
            output.push(' ');
            output.push_str(&render_child(walker, right, precedence));
            if delimited {
                output.push(')');
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            output.push_str("{ ");
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        output.push_str("let ");
                        output.push_str(&binding.name);
                        output.push_str(" = ");
                        output.push_str(&render_child(walker, value, 0));
                        output.push_str("; ");
                    }
                    ResolvedStatement::Assign { binding, value, .. } => {
                        output.push_str(&binding.name);
                        output.push_str(" = ");
                        output.push_str(&render_child(walker, value, 0));
                        output.push_str("; ");
                    }
                    ResolvedStatement::Unsafe { body, .. } => {
                        output.push_str("unsafe ");
                        output.push_str(&render_child(walker, body, 0));
                        output.push_str("; ");
                    }
                }
            }
            output.push_str(&render_child(walker, tail, 0));
            output.push_str(" }");
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            output.push_str("if(");
            output.push_str(&render_child(walker, condition, 0));
            output.push(',');
            output.push_str(&render_child(walker, then_branch, 0));
            output.push(',');
            output.push_str(&render_child(walker, else_branch, 0));
            output.push(')');
        }
        ResolvedExprKind::ConstructRecord { record, fields } => {
            output.push_str(&walker.declaration_name(record));
            render_fields(walker, fields, output);
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => {
            output.push_str(&walker.declaration_name(variant));
            output.push('.');
            output.push_str(&walker.declaration_name(case));
            render_fields(walker, fields, output);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            output.push_str("update(");
            output.push_str(&render_child(walker, base, 0));
            output.push(',');
            render_fields(walker, fields, output);
            output.push(')');
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            output.push_str("match(");
            output.push_str(&render_child(walker, scrutinee, 0));
            output.push_str("){");
            for (index, arm) in arms.iter().enumerate() {
                if index != 0 {
                    output.push('|');
                }
                output.push_str(pattern_tag(walker, &arm.pattern).as_str());
                output.push_str("=>");
                output.push_str(&render_child(walker, &arm.value, 0));
            }
            output.push('}');
        }
        ResolvedExprKind::Try { operand, .. } => {
            output.push_str("try(");
            output.push_str(&render_child(walker, operand, 0));
            output.push(')');
        }
        ResolvedExprKind::TryOption { operand, .. } => {
            output.push_str("try_option(");
            output.push_str(&render_child(walker, operand, 0));
            output.push(')');
        }
        ResolvedExprKind::Project { base, field } => {
            output.push_str(&render_child(walker, base, 7));
            output.push('.');
            output.push_str(&walker.declaration_name(field));
        }
    }
}

fn render_child(walker: &Walker<'_>, expr: &ResolvedExpr, parent_precedence: u8) -> String {
    let mut text = String::new();
    render_expr(walker, expr, parent_precedence, &mut text);
    text
}

fn render_args(walker: &Walker<'_>, args: &[ResolvedExpr], output: &mut String) {
    output.push('(');
    for (index, argument) in args.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str(&render_child(walker, argument, 0));
    }
    output.push(')');
}

fn render_fields(
    walker: &Walker<'_>,
    fields: &[hir::ResolvedFieldInitializer],
    output: &mut String,
) {
    output.push('{');
    for (index, field) in fields.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&walker.declaration_name(&field.field));
        output.push(':');
        output.push_str(&render_child(walker, &field.value, 0));
    }
    output.push('}');
}

fn pattern_tag(walker: &Walker<'_>, pattern: &hir::ResolvedMatchPattern) -> String {
    match pattern {
        hir::ResolvedMatchPattern::Variant { variant, case, .. } => format!(
            "{}.{}",
            walker.declaration_name(variant),
            walker.declaration_name(case)
        ),
        hir::ResolvedMatchPattern::Record { record, .. } => walker.declaration_name(record),
        hir::ResolvedMatchPattern::Wildcard => "_".to_owned(),
    }
}

impl Walker<'_> {
    /// Classify one HIR expression node and everything below it. Regions are
    /// discovered top-down at the highest node whose whole subtree is a pure
    /// straight-line region; every other node receives exactly one
    /// ineligibility entry (or is a covered operand inside a region). Entries
    /// appear in pre-order traversal order.
    fn scan_expr(&mut self, expr: &ResolvedExpr) {
        // A numeric arithmetic subtree over plain leaves is one region; take
        // the maximal such root and never descend into it again.
        if let Some(element_type) = element_type_name(&expr.ty) {
            let rooted = match &expr.kind {
                ResolvedExprKind::Binary { op, .. } => RegionOperator::from_binary(*op).is_some(),
                ResolvedExprKind::Unary {
                    op: UnaryOp::Neg, ..
                } => true,
                _ => false,
            };
            if rooted && is_pure_region(expr, element_type) {
                let float = matches!(expr.ty, ResolvedType::F32 | ResolvedType::F64);
                let mut operations = Vec::new();
                let mut operators = 0usize;
                let mut leaves = 0usize;
                collect_operations(expr, float, &mut operations, &mut operators, &mut leaves);
                let proposed_width = proposed_width(element_type, operators, leaves);
                self.scan.regions.push(RegionEntry {
                    root: render_child(self, expr, 0),
                    element_type,
                    operators,
                    leaves,
                    proposed_width,
                    operations,
                });
                return;
            }
        }
        match &expr.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_) => {
                self.push_ineligible(expr, REASON_SCALAR_LEAF);
            }
            ResolvedExprKind::Bool(_) => {
                self.push_ineligible(expr, REASON_BOOL_MIXING);
            }
            ResolvedExprKind::Char(_) => {
                self.push_ineligible(expr, REASON_CHAR_OPERATION);
            }
            ResolvedExprKind::Place(place) => {
                let reason = match &expr.ty {
                    ty if is_bool_type(ty) => REASON_BOOL_MIXING,
                    ty if is_char_type(ty) => REASON_CHAR_OPERATION,
                    _ => {
                        // Numeric or aggregate places outside regions:
                        // projection-free numeric places are trivial leaves;
                        // projected or aggregate places carry no lane work.
                        if place.projections.is_empty() && element_type_name(&expr.ty).is_some() {
                            REASON_SCALAR_LEAF
                        } else {
                            REASON_AGGREGATE_OPERATION
                        }
                    }
                };
                self.push_ineligible(expr, reason);
            }
            ResolvedExprKind::Call { args, .. } => {
                self.record_call();
                self.push_ineligible(expr, REASON_CALL);
                for argument in args {
                    self.scan_expr(argument);
                }
            }
            ResolvedExprKind::NativeRustImportCall(_) => {
                // The HIR node carries no argument payload to descend into;
                // the import call itself is recorded as one ineligible call.
                self.record_call();
                self.push_ineligible(expr, REASON_CALL);
            }
            ResolvedExprKind::Unary { op, value } => {
                let reason = match op {
                    UnaryOp::Neg => REASON_COMPUTED_OPERAND,
                    UnaryOp::Not => REASON_BOOL_MIXING,
                };
                self.push_ineligible(expr, reason);
                self.scan_expr(value);
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                let reason = match expr_kind_operator_category(expr) {
                    BinaryCategory::ComparisonOrLogical | BinaryCategory::BooleanArithmetic => {
                        REASON_BOOL_MIXING
                    }
                    BinaryCategory::DivisionRemainder => REASON_DIVISION_REMAINDER,
                    BinaryCategory::ComputedNumeric => REASON_COMPUTED_OPERAND,
                };
                self.push_ineligible(expr, reason);
                self.scan_expr(left);
                self.scan_expr(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    self.scan_statement(statement);
                }
                self.scan_expr(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.push_ineligible(expr, REASON_CONTROL_FLOW);
                self.scan_expr(condition);
                self.scan_expr(then_branch);
                self.scan_expr(else_branch);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                self.push_ineligible(expr, REASON_AGGREGATE_OPERATION);
                for field in fields {
                    self.scan_expr(&field.value);
                }
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                self.push_ineligible(expr, REASON_AGGREGATE_OPERATION);
                self.scan_expr(base);
                for field in fields {
                    self.scan_expr(&field.value);
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                self.push_ineligible(expr, REASON_CONTROL_FLOW);
                self.scan_expr(scrutinee);
                for arm in arms {
                    self.scan_expr(&arm.value);
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                self.push_ineligible(expr, REASON_CONTROL_FLOW);
                self.scan_expr(operand);
            }
            ResolvedExprKind::Project { base, .. } => {
                self.push_ineligible(expr, REASON_AGGREGATE_OPERATION);
                self.scan_expr(base);
            }
        }
    }

    fn scan_statement(&mut self, statement: &hir::ResolvedStatement) {
        match statement {
            ResolvedStatement::Let { binding, value, .. } => {
                self.scan_expr(value);
                self.names.insert(binding.id.clone(), binding.name.clone());
            }
            ResolvedStatement::Assign { value, .. } => {
                // The stored expression is a mutation target: recorded once as
                // ineligible and never descended into for lane packing.
                self.scan.assignment_count += 1;
                self.push_ineligible(value, REASON_MUTATION_TARGET);
            }
            ResolvedStatement::Unsafe { body, .. } => {
                self.scan_expr(body);
            }
        }
    }

    fn push_ineligible(&mut self, expr: &ResolvedExpr, reason: &'static str) {
        self.scan.ineligible.push(IneligibleEntry {
            expr: render_child(self, expr, 0),
            reason,
        });
    }
}

enum BinaryCategory {
    ComparisonOrLogical,
    BooleanArithmetic,
    DivisionRemainder,
    ComputedNumeric,
}

/// Why one non-region binary operator is not lane-covered.
fn expr_kind_operator_category(expr: &ResolvedExpr) -> BinaryCategory {
    let Some(kind) = binary_operator(expr) else {
        return BinaryCategory::ComputedNumeric;
    };
    match kind {
        BinaryOp::Div | BinaryOp::Rem => BinaryCategory::DivisionRemainder,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            BinaryCategory::ComparisonOrLogical
        }
        BinaryOp::And | BinaryOp::Or => {
            if matches!(expr.ty, ResolvedType::Bool) {
                BinaryCategory::BooleanArithmetic
            } else {
                BinaryCategory::ComparisonOrLogical
            }
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => BinaryCategory::ComputedNumeric,
    }
}

fn binary_operator(expr: &ResolvedExpr) -> Option<BinaryOp> {
    match &expr.kind {
        ResolvedExprKind::Binary { op, .. } => Some(*op),
        _ => None,
    }
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count and payload key order, replays
/// module counts, both closed vocabularies, the fixed lane model, the closed
/// portable-operation table, canonical ordering, per-region digests, lane
/// width feasibility, effect-freedom facts, and the fixed nonclaims before
/// returning the admitted function summaries.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedSimdReport, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(object) = value.as_object() else {
        return Err(consistency_error(
            "envelope must be a JSON object".to_owned(),
        ));
    };
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    if keys != ["bytes", "digest", "payload", "schema"] {
        return Err(consistency_error(format!(
            "envelope keys must be exactly [bytes, digest, payload, schema], found {keys:?}"
        )));
    }
    if object["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "envelope schema must be {SCHEMA}"
        )));
    }
    let Some(envelope_digest) = object["digest"].as_str() else {
        return Err(consistency_error(
            "envelope digest must be a string".to_owned(),
        ));
    };
    let Some(declared_bytes) = object["bytes"].as_u64() else {
        return Err(consistency_error(
            "envelope bytes must be an unsigned integer".to_owned(),
        ));
    };
    const PAYLOAD_KEY: &str = "\"payload\":";
    let Some(offset) = envelope.find(PAYLOAD_KEY) else {
        return Err(consistency_error(
            "envelope is missing its payload member".to_owned(),
        ));
    };
    if !envelope.ends_with('}') {
        return Err(consistency_error("envelope must end with `}`".to_owned()));
    }
    let payload = &envelope[offset + PAYLOAD_KEY.len()..envelope.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(consistency_error(
            "envelope payload must be a JSON object".to_owned(),
        ));
    }
    if declared_bytes != payload.len() as u64 {
        return Err(consistency_error(format!(
            "envelope declares {declared_bytes} payload bytes but {} are present",
            payload.len()
        )));
    }
    let recomputed = domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes());
    if envelope_digest != recomputed {
        return Err(consistency_error(
            "envelope digest does not match the exact payload bytes".to_owned(),
        ));
    }
    let payload_value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    if payload_value["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "payload schema must be {SCHEMA}"
        )));
    }
    const PAYLOAD_KEYS: [&str; 10] = [
        "analysis_scope",
        "exclusions",
        "functions",
        "lane_model",
        "limits",
        "module",
        "nonclaims",
        "operation_table",
        "schema",
        "source",
    ];
    let payload_keys: Vec<&str> = payload_value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if payload_keys != PAYLOAD_KEYS {
        return Err(consistency_error(format!(
            "payload keys must be exactly {PAYLOAD_KEYS:?}, found {payload_keys:?}"
        )));
    }
    if payload_value["analysis_scope"] != "pure_straight_line_arithmetic_only" {
        return Err(consistency_error(
            "payload analysis_scope must be fixed".to_owned(),
        ));
    }

    // Closed sections.
    let lane_model: serde_json::Value =
        serde_json::from_str(LANE_MODEL_JSON).expect("lane model constant is valid JSON");
    if payload_value["lane_model"] != lane_model {
        return Err(consistency_error(
            "lane_model must be exactly the fixed portable lane model".to_owned(),
        ));
    }
    let operation_table: serde_json::Value =
        serde_json::from_str(OPERATION_TABLE_JSON).expect("operation table constant is valid JSON");
    if payload_value["operation_table"] != operation_table {
        return Err(consistency_error(
            "operation_table must be exactly the closed portable operation table".to_owned(),
        ));
    }
    let nonclaims: serde_json::Value =
        serde_json::from_str(NONCLAIMS_JSON).expect("nonclaims constant is valid JSON");
    if payload_value["nonclaims"] != nonclaims {
        return Err(consistency_error(
            "nonclaims must be exactly the fixed honest-boundary statement".to_owned(),
        ));
    }

    // Module counts agree with the listings.
    let Some(module) = payload_value["module"].as_object() else {
        return Err(consistency_error(
            "payload module must be an object".to_owned(),
        ));
    };
    let functions_total = module["functions_total"].as_u64().ok_or_else(|| {
        consistency_error("module functions_total must be an unsigned integer".to_owned())
    })?;
    let functions_admitted = module["functions_admitted"].as_u64().ok_or_else(|| {
        consistency_error("module functions_admitted must be an unsigned integer".to_owned())
    })?;
    let functions_excluded = module["functions_excluded"].as_u64().ok_or_else(|| {
        consistency_error("module functions_excluded must be an unsigned integer".to_owned())
    })?;
    let listed_functions = payload_value["functions"]
        .as_array()
        .ok_or_else(|| consistency_error("payload functions must be an array".to_owned()))?;
    let listed_exclusions = payload_value["exclusions"]
        .as_array()
        .ok_or_else(|| consistency_error("payload exclusions must be an array".to_owned()))?;
    if functions_total != (listed_functions.len() + listed_exclusions.len()) as u64
        || functions_admitted != listed_functions.len() as u64
        || functions_excluded != listed_exclusions.len() as u64
    {
        return Err(consistency_error(
            "module counts disagree with the listed functions and exclusions".to_owned(),
        ));
    }

    // Closed vocabularies.
    for exclusion in listed_exclusions {
        let Some(reason) = exclusion["reason"].as_str() else {
            return Err(consistency_error(
                "function exclusion reason must be a string".to_owned(),
            ));
        };
        if !FUNCTION_EXCLUSION_REASONS.contains(&reason) {
            return Err(consistency_error(format!(
                "function exclusion reason `{reason}` is outside the closed vocabulary"
            )));
        }
    }

    // Canonical ordering across both listings.
    let mut combined = Vec::<&str>::with_capacity(listed_functions.len() + listed_exclusions.len());
    for listing in [listed_functions, listed_exclusions] {
        let mut previous: Option<&str> = None;
        for entry in listing {
            let Some(stable_id) = entry["stable_id"].as_str() else {
                return Err(consistency_error(
                    "function stable_id must be a string".to_owned(),
                ));
            };
            if let Some(previous) = previous {
                if previous.as_bytes() >= stable_id.as_bytes() {
                    return Err(consistency_error(format!(
                        "function `{stable_id}` breaks the strict stable-id ordering"
                    )));
                }
            }
            previous = Some(stable_id);
            combined.push(stable_id);
        }
    }
    combined.sort_unstable();
    for pair in combined.windows(2) {
        if pair[0] == pair[1] {
            return Err(consistency_error(format!(
                "stable id `{}` appears in both listings",
                pair[0]
            )));
        }
    }

    // Per-function replay.
    let mut verified = Vec::<VerifiedFunction>::with_capacity(listed_functions.len());
    for function in listed_functions {
        let stable_id = function["stable_id"]
            .as_str()
            .ok_or_else(|| consistency_error("function stable_id must be a string".to_owned()))?;
        let name = function["name"]
            .as_str()
            .ok_or_else(|| consistency_error("function name must be a string".to_owned()))?;
        let effect_freedom = &function["effect_freedom"];
        if effect_freedom["declared_effects"]
            .as_array()
            .is_none_or(|effects| !effects.is_empty())
        {
            return Err(consistency_error(format!(
                "function `{stable_id}` must declare no effects"
            )));
        }
        let justification = effect_freedom["justification"].as_array().ok_or_else(|| {
            consistency_error("effect_freedom justification must be an array".to_owned())
        })?;
        let tokens: Vec<&str> = justification
            .iter()
            .map(|token| token.as_str())
            .collect::<Option<_>>()
            .ok_or_else(|| {
                consistency_error("effect_freedom justification tokens must be strings".to_owned())
            })?;
        for token in &tokens {
            if !JUSTIFICATION_TOKENS.contains(token) {
                return Err(consistency_error(format!(
                    "effect_freedom token `{token}` is outside the closed vocabulary"
                )));
            }
        }
        let call_count = effect_freedom["call_count"].as_u64().ok_or_else(|| {
            consistency_error("effect_freedom call_count must be an unsigned integer".to_owned())
        })?;
        let expected_call_token = if call_count == 0 {
            "no_call_expressions_in_body"
        } else {
            "calls_recorded_as_ineligible"
        };
        if !tokens.contains(&expected_call_token)
            || tokens.contains(&"no_call_expressions_in_body")
                != (expected_call_token == "no_call_expressions_in_body")
        {
            return Err(consistency_error(format!(
                "effect_freedom justification of `{stable_id}` contradicts its call count"
            )));
        }
        if effect_freedom["assignment_count"].as_u64().is_none() {
            return Err(consistency_error(
                "effect_freedom assignment_count must be an unsigned integer".to_owned(),
            ));
        }

        let regions = function["regions"]
            .as_array()
            .ok_or_else(|| consistency_error("function regions must be an array".to_owned()))?;
        let mut verified_regions = Vec::<VerifiedRegion>::with_capacity(regions.len());
        for (index, region) in regions.iter().enumerate() {
            if region["index"].as_u64() != Some(index as u64) {
                return Err(consistency_error(
                    "region indices must ascend from zero without gaps".to_owned(),
                ));
            }
            let root = region["root"]
                .as_str()
                .ok_or_else(|| consistency_error("region root must be a string".to_owned()))?;
            let Some(root_digest) = region["root_sha256"].as_str() else {
                return Err(consistency_error(
                    "region root_sha256 must be a string".to_owned(),
                ));
            };
            if root_digest != domain_digest(REGION_DIGEST_DOMAIN, root.as_bytes()) {
                return Err(consistency_error(format!(
                    "region digest of `{stable_id}` does not match the exact root text"
                )));
            }
            let Some(element_type) = region["element_type"].as_str() else {
                return Err(consistency_error(
                    "region element_type must be a string".to_owned(),
                ));
            };
            let operators = region["operators"].as_u64().ok_or_else(|| {
                consistency_error("region operators must be an unsigned integer".to_owned())
            })?;
            let leaves = region["leaves"].as_u64().ok_or_else(|| {
                consistency_error("region leaves must be an unsigned integer".to_owned())
            })?;
            let proposed_width = region["proposed_width"].as_u64().ok_or_else(|| {
                consistency_error("region proposed_width must be an unsigned integer".to_owned())
            })?;
            if ![2u64, 4, 8].contains(&proposed_width)
                || proposed_width > type_ceiling(element_type) as u64
                || proposed_width > operators.saturating_add(leaves).max(2)
            {
                return Err(consistency_error(format!(
                    "region width {proposed_width} of `{stable_id}` is infeasible under the lane model"
                )));
            }
            let operations = region["operations"].as_array().ok_or_else(|| {
                consistency_error("region operations must be an array".to_owned())
            })?;
            if operations.len() as u64 != operators {
                return Err(consistency_error(
                    "region operations length must equal the operator count".to_owned(),
                ));
            }
            let expected_class = match element_type {
                "f32" | "f64" => "float",
                "i64" | "i32" | "u8" => "integer",
                other => {
                    return Err(consistency_error(format!(
                        "region element type `{other}` is not lane-eligible"
                    )))
                }
            };
            for operation in operations {
                let Some(operation) = operation.as_str() else {
                    return Err(consistency_error(
                        "region operations entries must be strings".to_owned(),
                    ));
                };
                if !OPERATION_ROWS
                    .iter()
                    .any(|row| row.class == expected_class && row.portable_op == operation)
                {
                    return Err(consistency_error(format!(
                        "portable operation `{operation}` is outside the closed table"
                    )));
                }
            }
            verified_regions.push(VerifiedRegion {
                root: root.to_owned(),
                proposed_width: proposed_width as u8,
            });
        }

        let ineligible = function["ineligible"]
            .as_array()
            .ok_or_else(|| consistency_error("function ineligible must be an array".to_owned()))?;
        for (index, entry) in ineligible.iter().enumerate() {
            if entry["index"].as_u64() != Some(index as u64) {
                return Err(consistency_error(
                    "ineligibility indices must ascend from zero without gaps".to_owned(),
                ));
            }
            let Some(reason) = entry["reason"].as_str() else {
                return Err(consistency_error(
                    "ineligibility reason must be a string".to_owned(),
                ));
            };
            if !EXPRESSION_INELIGIBILITY_REASONS.contains(&reason) {
                return Err(consistency_error(format!(
                    "ineligibility reason `{reason}` is outside the closed vocabulary"
                )));
            }
            if entry["expr"].as_str().is_none() {
                return Err(consistency_error(
                    "ineligibility expr must be a string".to_owned(),
                ));
            }
        }
        verified.push(VerifiedFunction {
            stable_id: stable_id.to_owned(),
            name: name.to_owned(),
            regions: verified_regions,
        });
    }
    Ok(VerifiedSimdReport {
        functions: verified,
    })
}

struct OperationRow {
    class: &'static str,
    portable_op: &'static str,
}

const OPERATION_ROWS: [OperationRow; 8] = [
    OperationRow {
        class: "float",
        portable_op: "fp_lane.add",
    },
    OperationRow {
        class: "float",
        portable_op: "fp_lane.mul",
    },
    OperationRow {
        class: "float",
        portable_op: "fp_lane.neg",
    },
    OperationRow {
        class: "float",
        portable_op: "fp_lane.sub",
    },
    OperationRow {
        class: "integer",
        portable_op: "int_lane.add",
    },
    OperationRow {
        class: "integer",
        portable_op: "int_lane.mul",
    },
    OperationRow {
        class: "integer",
        portable_op: "int_lane.neg",
    },
    OperationRow {
        class: "integer",
        portable_op: "int_lane.sub",
    },
];

fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
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

#[allow(clippy::too_many_arguments)]
fn render(
    path_text: &str,
    revision: &str,
    digest: &str,
    module_name: &str,
    max_bytes: usize,
    functions_total: usize,
    functions: &[FunctionEntry],
    excluded: &[(&Function, &'static str)],
) -> String {
    let function_entries = functions
        .iter()
        .map(|entry| {
            let signature_element_types = signature_element_types_of(entry);
            let regions = entry
                .scan
                .regions
                .iter()
                .enumerate()
                .map(|(index, region)| {
                    let operations = region
                        .operations
                        .iter()
                        .map(|operation| quote_json(operation))
                        .collect::<Vec<_>>();
                    bformat!(
                        "{{\"index\":{},\"root_sha256\":{},\"root\":{},\
\"element_type\":\"{}\",\"operators\":{},\"leaves\":{},\"proposed_width\":{},\
\"operations\":[{}]}}",
                        index,
                        quote_json(&domain_digest(REGION_DIGEST_DOMAIN, region.root.as_bytes())),
                        quote_json(&region.root),
                        region.element_type,
                        region.operators,
                        region.leaves,
                        region.proposed_width,
                        operations.budgeted_join(","),
                    )
                })
                .collect::<Vec<_>>();
            let ineligible = entry
                .scan
                .ineligible
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    bformat!(
                        "{{\"index\":{},\"reason\":\"{}\",\"expr\":{}}}",
                        index,
                        item.reason,
                        quote_json(&item.expr),
                    )
                })
                .collect::<Vec<_>>();
            let call_token = if entry.scan.call_count == 0 {
                "no_call_expressions_in_body"
            } else {
                "calls_recorded_as_ineligible"
            };
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"signature_element_types\":[{}],\
\"regions\":[{}],\"ineligible\":[{}],\
\"effect_freedom\":{{\"declared_effects\":[],\
\"justification\":[\"{call_token}\",\"declared_effects_empty\"],\
\"call_count\":{},\"assignment_count\":{}}}}}",
                quote_json(&entry.stable_id),
                quote_json(&entry.name),
                signature_element_types.budgeted_join(","),
                regions.budgeted_join(","),
                ineligible.budgeted_join(","),
                entry.scan.call_count,
                entry.scan.assignment_count,
            )
        })
        .collect::<Vec<_>>();
    let exclusion_entries = excluded
        .iter()
        .map(|(function, reason)| {
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"reason\":\"{}\"}}",
                quote_json(&function.stable_id),
                quote_json(&function.name),
                reason,
            )
        })
        .collect::<Vec<_>>();

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"module\":{{\"name\":{},\"functions_total\":{},\"functions_admitted\":{},\"functions_excluded\":{}}},\
\"analysis_scope\":\"pure_straight_line_arithmetic_only\",\
\"lane_model\":{},\"operation_table\":{},\
\"functions\":[{}],\"exclusions\":[{}],\"nonclaims\":{}}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        quote_json(module_name),
        functions_total,
        functions.len(),
        excluded.len(),
        LANE_MODEL_JSON,
        OPERATION_TABLE_JSON,
        function_entries.budgeted_join(","),
        exclusion_entries.budgeted_join(","),
        NONCLAIMS_JSON,
    );
    bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload,
    )
}

fn signature_element_types_of(entry: &FunctionEntry) -> Vec<String> {
    entry
        .signature_element_types
        .iter()
        .map(|element| quote_json(element))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(SimdReportOptions::new(512).is_err());
        assert!(SimdReportOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
        assert!(SimdReportOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
        assert_eq!(SimdReportOptions::default().max_bytes, DEFAULT_MAX_BYTES);
    }

    #[test]
    fn constants_are_canonical_sorted_and_agree() {
        let mut sorted = FUNCTION_EXCLUSION_REASONS;
        sorted.sort_unstable();
        assert_eq!(sorted, FUNCTION_EXCLUSION_REASONS);
        let mut sorted = EXPRESSION_INELIGIBILITY_REASONS;
        sorted.sort_unstable();
        assert_eq!(sorted, EXPRESSION_INELIGIBILITY_REASONS);
        let mut sorted = JUSTIFICATION_TOKENS;
        sorted.sort_unstable();
        assert_eq!(sorted, JUSTIFICATION_TOKENS);

        let lane_model: serde_json::Value =
            serde_json::from_str(LANE_MODEL_JSON).expect("lane model constant");
        assert_eq!(
            lane_model,
            serde_json::json!({
                "register_bits": 128,
                "widths": [2, 4, 8],
                "type_ceilings": [
                    {"element_type": "f32", "ceiling": 4},
                    {"element_type": "f64", "ceiling": 2},
                    {"element_type": "i32", "ceiling": 4},
                    {"element_type": "i64", "ceiling": 2},
                    {"element_type": "u8", "ceiling": 8}
                ]
            })
        );
        let table: serde_json::Value =
            serde_json::from_str(OPERATION_TABLE_JSON).expect("operation table constant");
        let rows = table.as_array().expect("array");
        assert_eq!(rows.len(), OPERATION_ROWS.len());
        for (value, row) in rows.iter().zip(OPERATION_ROWS.iter()) {
            assert_eq!(value["class"], row.class);
            assert_eq!(value["portable_op"], row.portable_op);
        }
        let nonclaims: serde_json::Value =
            serde_json::from_str(NONCLAIMS_JSON).expect("nonclaims constant");
        let listed = nonclaims.as_array().expect("array");
        assert!(listed.iter().all(|token| token.is_string()));
        assert!(NONCLAIMS_JSON.contains("no_simd_codegen_or_intrinsics_emitted"));
        assert!(NONCLAIMS_JSON.contains("no_spirv_webgpu_or_gpu_kernels"));
        assert!(NONCLAIMS_JSON.contains("no_autovectorization_claims"));
        assert!(NONCLAIMS_JSON.contains("no_target_execution"));
    }

    #[test]
    fn domain_digest_is_domain_separated() {
        let first = domain_digest(SOURCE_DIGEST_DOMAIN, b"abc");
        let second = domain_digest(REGION_DIGEST_DOMAIN, b"abc");
        assert_ne!(first, second);
        assert_eq!(first, domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"));
    }

    #[test]
    fn width_selection_follows_the_documented_rule() {
        // Ceilings dominate: i64/f64 never exceed 2.
        assert_eq!(proposed_width("i64", 7, 8), 2);
        assert_eq!(proposed_width("f64", 3, 4), 2);
        // i32/f32 reach 4 when the element count allows it.
        assert_eq!(proposed_width("i32", 6, 7), 4);
        assert_eq!(proposed_width("f32", 3, 1), 4);
        // Element counts below the next width fall back deterministically.
        assert_eq!(proposed_width("i32", 2, 1), 2);
        assert_eq!(proposed_width("u8", 7, 8), 8);
        assert_eq!(proposed_width("u8", 3, 4), 4);
        // Minimum regions always propose 2.
        assert_eq!(proposed_width("i64", 1, 1), 2);
    }

    #[test]
    fn admission_mirrors_the_scalar_profile() {
        let source = r#"
module test.probe;

@id("probe.ok")
fn ok(value: i64) -> f64 { 1.0 }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("probe.borrowed")
fn borrowed(target: borrow Buffer, amount: i64) -> i64 { amount }

@id("probe.record")
fn wrapped(value: i64) -> Pair { Pair { left: value, right: value } }

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

record Pair {
    @id("pair.left") left: i64,
    @id("pair.right") right: i64,
}
"#;
        let path = std::env::temp_dir().join(format!(
            "semaprax-simd-report-unit-{}.spx",
            std::process::id()
        ));
        std::fs::write(&path, source).unwrap();
        let program = parse(&std::fs::read_to_string(&path).unwrap(), &path).expect("parses");
        let mut functions = program.functions.iter().collect::<Vec<_>>();
        functions.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
        let reasons: Vec<Option<&'static str>> = functions
            .iter()
            .map(|function| admission(function))
            .collect();
        assert!(reasons.contains(&None));
        assert!(reasons.contains(&Some(REASON_GENERIC_FUNCTION)));
        assert!(reasons.contains(&Some(REASON_DECLARED_EFFECTS)));
        assert!(reasons.contains(&Some(REASON_UNSUPPORTED_PARAMETER_MODE)));
        assert!(reasons.contains(&Some(REASON_NON_SCALAR_SIGNATURE)));
        let _ = std::fs::remove_file(&path);
    }
}
