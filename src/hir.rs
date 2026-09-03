//! Resolved high-level intermediate representation.
//!
//! The parsed AST keeps the names humans wrote. HIR replaces every nominal,
//! callable, and value reference with a deterministic identity. Backends should
//! consume this layer as the language grows rather than repeating name lookup.
//!
//! This module root owns the analysis entry points (`analyze`, `resolve`,
//! `validate`), the resolver and validator scope state, and the index of
//! submodules. The data model lives in `ids`, `nodes`, and `expr_nodes`; AST
//! lowering is one inherent `Resolver` impl split across `resolve_program`,
//! `resolve_statement`, `resolve_pattern`, `resolve_expr` (with its frames in
//! `resolve_expr_frame`), `resolve_expr_reference`, and `resolve_class`.
//! `monomorphize`, `byte_capacity`, and `byte_slice_provenance` own the
//! derivations layered over resolved HIR.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::ast::{BinaryOp, Program, Span, Type, TypeDeclarationKind, UnaryOp};
use crate::cleanup::CleanupInventory;
use crate::cleanup_plan::CleanupPlan;
use crate::conformance::STATUS_DOMAIN_MAX_BYTES_V1;
use crate::diagnostic::Diagnostic;
use crate::loan_plan::LoanId;
use crate::source_verify;

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

#[cfg(test)]
mod command_io_provenance_hostile_tests {
    use super::*;

    #[test]
    fn command_argument_root_provenance_cannot_be_reclassified_or_recharged() {
        let source = r#"
module test.command_root_hostile;
permit { process.args.read }
@id("command.view")
fn view() -> usize uses { process.args.read } {
    let argument = arg_utf8(0usize);
    let bytes = str_as_bytes(argument);
    byte_len(bytes)
}

@id("app.main") fn main() -> i64 { 0 }
"#;
        let ast = crate::parse(source, "command-root-hostile.spx").unwrap();
        let mut program = resolve(&ast).unwrap();
        let value = program
            .declarations
            .byte_slice_roots
            .iter()
            .find_map(|(value, provenance)| {
                (provenance.root_kind == ByteSliceRootKind::CommandArguments)
                    .then_some(value.clone())
            })
            .expect("arg_utf8/str_as_bytes records its invocation root");
        let provenance = program
            .declarations
            .byte_slice_roots
            .get_mut(&value)
            .unwrap();
        provenance.root_kind = ByteSliceRootKind::BorrowedStr;
        provenance.root = value;
        let error = validate(&program).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(
            error.message.contains("byte-slice symbolic provenance"),
            "{}",
            error.message
        );
    }
}

mod byte_capacity;
mod byte_slice_provenance;
#[cfg(test)]
mod capacity_probe;
mod expr_nodes;
mod ids;
mod inspection;
mod monomorphize;
mod nodes;
#[cfg(test)]
mod private_capacity_contract_tests;
#[cfg(test)]
mod projected_byte_field_provenance_tests;
mod record_evolution;
mod resolve_class;
mod resolve_expr;
mod resolve_expr_frame;
#[cfg(test)]
mod resolve_expr_reference;
mod resolve_pattern;
mod resolve_program;
mod resolve_statement;
mod type_reachability;
mod validation;
mod workspace_link;
pub(crate) use workspace_link::compiler_prelude_declarations;

/// Validate resolved HIR and independently replay its canonical shared-loan
/// proof attachment before any semantic consumer may trust it.
pub fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    inspection::validate(program)?;
    crate::loan_plan::validate_program(program)
}
pub(crate) use inspection::visit_resolved_calls;
use inspection::{
    path_is_prefix, reject_nul_identity, resolved_lifecycle_effects, validate_nul_free_identities,
};
pub(crate) use inspection::{
    validate_attached_identity_references, workspace_call_sites, workspace_expression_identity,
};

pub(crate) use type_reachability::reachable_authored_types;
pub(crate) use validation::validate_core;
#[cfg(test)]
use validation::HirValidator;
pub(crate) use workspace_link::{
    copy_scalar_type, link_language_command_io_workspace, link_line_command_io_workspace,
    link_owned_data_api_workspace, link_package_scalar_workspace,
    link_scalar_native_rust_workspace, link_scalar_workspace, link_useful_data_command_workspace,
    link_useful_data_workspace, link_useful_text_workspace,
    owned_data_api_workspace_return_admitted, useful_data_workspace_parameter_admitted,
    useful_data_workspace_return_admitted, LinkedScalarNatives, COPY_SCALAR_NAMES,
};

#[allow(dead_code, reason = "private Workspace Semantic Graph Phase-A seam")]
pub(crate) fn workspace_call_edges(
    program: &ResolvedProgram,
) -> BTreeSet<(DeclarationId, DeclarationId)> {
    inspection::workspace_call_edges(program)
}

#[cfg(test)]
thread_local! {
    static ITERATIVE_PHASE_CAPACITY_HIGH_WATER: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
    static TYPE_FACTS_OUTER_BASELINE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(any(
    test,
    all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )
))]
mod cache_codec;
mod declaration_index;
pub use declaration_index::{dispose_declaration_index_for_private_contract, DeclarationIndex};

// Resolved data model and shared probes, re-exported so this module and its
// submodules keep reaching every HIR item through `hir::` as before.
pub(crate) use byte_capacity::{
    analyze_byte_data_capacity, push_resolved_expression_children_in_authored_order,
};
use byte_slice_provenance::derive_byte_slice_provenance;
#[cfg(test)]
pub(crate) use capacity_probe::{
    iterative_phase_capacity_high_water, reset_iterative_phase_capacity_high_water,
};
#[cfg(test)]
use capacity_probe::{
    note_iterative_phase_capacity, place_projection_owned_capacity,
    resolved_field_declaration_owned_capacity, resolved_type_owned_capacity,
    resolved_variant_case_owned_capacity, type_facts_outer_baseline,
    validation_scope_owned_capacity,
};
pub use expr_nodes::{
    PatternValue, ResolvedExpr, ResolvedExprKind, ResolvedFieldInitializer, ResolvedMatchArm,
    ResolvedMatchPattern, ResolvedMatchPatternField, ResolvedRecordMatchFieldPattern,
    ResolvedRecordMatchPatternField, ResolvedStatement,
};
pub use ids::{DeclarationId, ExpressionId, FunctionExecutionId, FunctionInstanceId, ValueId};
pub(crate) use monomorphize::substitute_type;
use monomorphize::{
    materialize_function_template, resolved_scalar_substitutions, same_function_meaning,
};
pub(crate) use nodes::{
    admitted_owned_byte_prelude_instance, is_scalar_resolved_type, LinkedDeclarationFact,
    LinkedOwnedDataParts, LinkedScalarFunction,
};
pub use nodes::{
    ByteSliceExtent, ByteSliceProvenance, ByteSliceRangeStep, ByteSliceRootKind, Declaration,
    DeclarationKind, IdentityOrigin, OwnershipMode, ResolvedBinding, ResolvedFieldDeclaration,
    ResolvedFunction, ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedHostCommandCall,
    ResolvedHostCommandOperation, ResolvedImport, ResolvedImportFailure, ResolvedImportParameter,
    ResolvedImportResult, ResolvedImportResultKind, ResolvedInterface, ResolvedMatchMode,
    ResolvedNativeRustImportCall, ResolvedParam, ResolvedProgram, ResolvedResourceDrop,
    ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind,
    ResolvedTypeParameterDeclaration, ResolvedVariantCaseDeclaration, TypeFacts,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub root: ValueId,
    pub projections: Vec<PlaceProjection>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlaceProjection {
    Field(DeclarationId),
    VariantField {
        case: DeclarationId,
        field: DeclarationId,
    },
}

#[derive(Clone)]
struct Binding {
    id: ValueId,
    ty: ResolvedType,
    ownership: OwnershipMode,
    /// Explicit Mutation v1: only local `let mut` bindings are mutable.
    mutable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Available,
    Moved,
    MaybeMoved,
}

impl Availability {
    fn join(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            Self::MaybeMoved
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidationBinding {
    ty: ResolvedType,
    ownership: OwnershipMode,
    availability: Availability,
    moved_places: BTreeMap<Vec<PlaceProjection>, Availability>,
    definitely_partial: BTreeSet<Vec<PlaceProjection>>,
    /// Exact shared loans currently protecting this storage root.
    active_loans: BTreeSet<LoanId>,
}

/// Restores the most recently published ownership scope on every early
/// validation return. Iterative continuations update the publication boundary
/// before entering a direct child; isolated Block/branch/arm children leave it
/// at their outer baseline.
struct ValidationScopePublication<'a> {
    target: &'a mut BTreeMap<ValueId, ValidationBinding>,
    published: BTreeMap<ValueId, ValidationBinding>,
    enabled: bool,
}

impl ValidationScopePublication<'_> {
    fn publish(&mut self, scope: &BTreeMap<ValueId, ValidationBinding>) {
        if self.enabled {
            self.published.clone_from(scope);
        }
    }
}

impl Drop for ValidationScopePublication<'_> {
    fn drop(&mut self) {
        std::mem::swap(self.target, &mut self.published);
    }
}

/// Verify and resolve a parsed program into deterministic HIR.
///
/// Verification errors are returned unchanged. This makes the HIR boundary
/// fail closed: no backend can accidentally resolve and execute an invalid AST.
pub fn resolve(program: &Program) -> Result<ResolvedProgram, Vec<Diagnostic>> {
    let Analysis {
        diagnostics,
        resolved,
    } = analyze(program);
    resolved.ok_or(diagnostics)
}

/// Private checked-module seam for exact monomorphic function reuse.
///
/// Source verification and declaration-index construction always run first.
/// A prior function is eligible only when the complete non-body environment,
/// every function signature, and that function's full AST are exactly equal.
/// The resolver still rebuilds and validates the complete `ResolvedProgram`.
pub(crate) fn resolve_with_function_reuse(
    program: &Program,
    previous_program: Option<&Program>,
    previous_resolved: Option<&ResolvedProgram>,
    previous_costs: Option<&BTreeMap<String, usize>>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>> {
    let diagnostics = source_verify::verify(program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(diagnostics);
    }
    let declarations = DeclarationIndex::from_verified(program).map_err(|error| vec![error])?;
    let reuse = previous_program
        .zip(previous_resolved)
        .zip(previous_costs)
        .filter(|((previous, _), _)| function_reuse_environment_is_exact(program, previous))
        .map(|((program, resolved), costs)| FunctionReuse {
            program,
            resolved,
            costs,
        });
    (Resolver {
        program,
        declarations,
        reuse,
        function_work: FunctionResolutionWork::default(),
    })
    .resolve()
    .map_err(|error| vec![error])
}

#[derive(Default)]
pub(crate) struct FunctionResolutionWork {
    costs: BTreeMap<String, usize>,
    reused: usize,
}

impl FunctionResolutionWork {
    pub(crate) fn costs(self) -> BTreeMap<String, usize> {
        self.costs
    }

    pub(crate) fn reused(&self) -> usize {
        self.reused
    }
}

/// The source diagnostics and optional resolved meaning from one analysis.
///
/// Warnings do not prevent resolution. Any source error fails closed before the
/// resolver runs, so invalid source cannot leak internal HIR diagnostics.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub diagnostics: Vec<Diagnostic>,
    pub resolved: Option<ResolvedProgram>,
}

/// Verify source once and resolve it when only warnings remain.
pub fn analyze(program: &Program) -> Analysis {
    let diagnostics = source_verify::verify(program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Analysis {
            diagnostics,
            resolved: None,
        };
    }

    let declarations = match DeclarationIndex::from_verified(program) {
        Ok(declarations) => declarations,
        Err(diagnostic) => {
            return Analysis {
                diagnostics: vec![diagnostic],
                resolved: None,
            };
        }
    };
    match (Resolver {
        program,
        declarations,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
    })
    .resolve()
    {
        Ok((resolved, _)) => Analysis {
            diagnostics,
            resolved: Some(resolved),
        },
        Err(diagnostic) => Analysis {
            // Preserve `resolve`'s established invariant-failure behavior: an
            // internal HIR diagnostic replaces otherwise non-fatal warnings.
            diagnostics: vec![diagnostic],
            resolved: None,
        },
    }
}

fn hir_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

struct FunctionReuse<'a> {
    program: &'a Program,
    resolved: &'a ResolvedProgram,
    costs: &'a BTreeMap<String, usize>,
}

struct Resolver<'a> {
    program: &'a Program,
    declarations: DeclarationIndex,
    reuse: Option<FunctionReuse<'a>>,
    function_work: FunctionResolutionWork,
}

fn function_reuse_environment_is_exact(current: &Program, previous: &Program) -> bool {
    current.path == previous.path
        && current.module == previous.module
        && current.module_uses == previous.module_uses
        && current.permits == previous.permits
        && current.types == previous.types
        && current.interfaces == previous.interfaces
        && current.protocols == previous.protocols
        && current.implementations == previous.implementations
        && current.functions.len() == previous.functions.len()
        && current
            .functions
            .iter()
            .all(|function| function.type_parameters.is_empty())
        && current
            .functions
            .iter()
            .zip(&previous.functions)
            .all(|(current, previous)| function_signature_is_exact(current, previous))
}

fn function_signature_is_exact(
    current: &crate::ast::Function,
    previous: &crate::ast::Function,
) -> bool {
    current.stable_id == previous.stable_id
        && current.explicit_id == previous.explicit_id
        && current.name == previous.name
        && current.name_span == previous.name_span
        && current.type_parameters == previous.type_parameters
        && current.params == previous.params
        && current.return_type == previous.return_type
        && current.effects == previous.effects
        && current.requires == previous.requires
        && current.ensures == previous.ensures
        && current.span == previous.span
}

#[cfg(test)]
mod function_reuse_environment_tests {
    use super::*;

    fn program(body: &str, requirement: &str) -> Program {
        crate::parse(
            &format!(
                "module reuse; @id(\"reuse.value\") fn value(input: i64) -> i64 requires {requirement} {{ {body} }} @id(\"reuse.main\") fn main() -> i64 {{ value(1) }}"
            ),
            "reuse.spx",
        )
        .unwrap()
    }

    #[test]
    fn only_an_exact_length_body_change_can_preserve_the_function_environment() {
        let before = program("input + 1", "input >= 0");
        let body_only = program("input + 2", "input >= 0");
        assert!(function_reuse_environment_is_exact(&body_only, &before));
        assert_ne!(body_only.functions[0], before.functions[0]);

        let contract = program("input + 2", "input >= 1");
        assert!(!function_reuse_environment_is_exact(&contract, &before));
    }
}

#[cfg(test)]
#[path = "hir/iterative_validator_tests.rs"]
mod iterative_validator_tests;

#[cfg(test)]
#[path = "hir/record_tests.rs"]
mod record_tests;
