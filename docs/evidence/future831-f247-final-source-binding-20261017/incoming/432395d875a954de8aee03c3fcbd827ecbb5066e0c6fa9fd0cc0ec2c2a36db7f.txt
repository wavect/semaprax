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

pub(crate) mod items;
mod nominal_payload;
#[path = "hir/prelude_bundle.rs"]
pub(crate) mod prelude_bundle;
use crate::ast::program_source::ProgramSource;
pub use nominal_payload::{NominalMutationError, NominalPayload};
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

pub(crate) mod owned_collection_record;
pub(crate) mod owned_record_subtree;
pub(crate) mod owned_text_record;
pub(crate) mod projected_string_view;
pub(crate) mod vec_field;

mod agent_nodes;
mod agent_validation;
pub(crate) mod borrowed_scalar;
mod byte_capacity;
mod byte_slice_provenance;
mod ordinary_scan;
pub(crate) mod static_text;
pub use static_text::StaticUtf8;
#[cfg(test)]
mod capacity_probe;
pub mod closure;
mod copy_discard;
#[cfg(test)]
pub(crate) mod funded;
pub(crate) use copy_discard::source as copy_discard_source;
mod expr_nodes;
pub(crate) use copy_discard::{
    function_uses as copy_discard_function_uses, program_uses as copy_discard_program_uses,
};
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "selected only by the pending FusedConstructionV1 capacity and executor entries"
    )
)]
pub(crate) mod fused_construction;
pub mod lexical_exit;
pub use lexical_exit::{
    ControlTarget, FlowCompletion, LexicalExitKind, LoopLatchKind, SourceForKind,
};
pub(crate) mod byte_partition;
mod canonical_loan_storage;
mod function_directory;
pub mod function_value;
mod ids;
mod inspection;
mod monomorphize;
mod nodes;
mod owned_result_try;
#[cfg(test)]
mod private_capacity_contract_tests;
#[cfg(test)]
mod projected_byte_field_provenance_tests;
mod record_evolution;
pub(crate) mod record_invariants;
mod resolve_box_call;
mod resolve_class;
mod resolve_expr;
mod resolver_scope;
use resolver_scope::Scope;
#[cfg(test)]
mod borrowed_root_oracle;
mod resolve_expr_frame;
#[cfg(test)]
mod resolve_expr_reference;
mod resolve_for;
mod resolve_for_own;
mod resolve_native_rust_result;
mod resolve_pattern;
mod resolve_program;
mod resolve_statement;
mod resolve_stdin_stream;
mod resolve_variant_or;
mod resolve_vec_call;
mod resolve_yield;
pub(crate) mod selected_fused_candidate;
mod type_reachability;
pub(crate) mod vec_constructor;
pub(crate) use resolve_expr::path_arena::ExprPath;
pub(crate) use type_reachability::is_admitted_copy_aggregate_variant_field;
pub(crate) use type_reachability::{
    is_admitted_concrete_owned_byte_variant, is_admitted_owned_string_variant,
};
pub(crate) fn nested_record_copy_scalar_is_admitted(ty: &ResolvedType) -> bool {
    type_reachability::nested_record_copy_scalar_is_admitted(ty)
}
mod validation;
mod variant_equality;
mod workspace_link;
pub(crate) mod yield_aggregate;
pub(crate) use agent_nodes::AgentExecutionSourceAssociation;
pub use agent_nodes::{
    ResolvedAgentDeclaration, ResolvedAgentModelWaitBinding, ResolvedAgentOperationKind,
    ResolvedAgentOperationRole, ResolvedAgentOperationRoleKind, ResolvedAgentTypeRole,
    ResolvedAgentTypeRoleKind,
};
pub(crate) use agent_validation::embedded::replay_agent_source_associations;
pub(crate) use workspace_link::compiler_prelude_declarations;
pub(crate) use workspace_link::native_owner::admitted_helper as indexed_owner_helper_signature;
pub(crate) use workspace_link::native_owner::{
    admitted_ri06_regex_resource, admitted_ri06_regex_resource_in, admitted_ri06_regex_result,
};

/// Validate resolved HIR and independently replay its canonical shared-loan
/// proof attachment before any semantic consumer may trust it.
pub fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::HirValidate);
    validate_with_static_text(program).map(|_| ())
}

pub(crate) fn validate_with_static_text(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    validate_selected_with_static_text(program, None)
}

/// The complete validation sequence. Only the private selected factory passes
/// a joined `FusedConstructionV1` selection; it changes that one loop's shape
/// check and capacity strategy, and nothing else.
fn validate_selected_with_static_text(
    program: &ResolvedProgram,
    fused: Option<selected_fused_candidate::Selected<'_>>,
) -> Result<bool, Diagnostic> {
    byte_partition::refuse_default(program)?;
    agent_validation::validate(program)?;
    let (selected, deferred) = inspection::validate_with_loan_comparison(program, fused)?;
    deferred.finish(program)?;
    Ok(selected)
}

/// Classify an attached loan only after the ordinary HIR/LoanPlan validation
/// boundary has authenticated its root and canonical origin. This is a schema
/// selector helper, not an alternative loan validator.
pub(crate) fn is_authenticated_nested_projected_byte_loan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    loan: &crate::loan_plan::Loan,
) -> bool {
    if !program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|candidate| std::ptr::eq(candidate, function))
    {
        return false;
    }
    if !function
        .loan_plan
        .loans
        .iter()
        .any(|candidate| std::ptr::eq(candidate, loan))
        || loan.origin.projections.len() < 2
    {
        return false;
    }
    let Some(root) = byte_capacity::resolved_value_type(program, &loan.origin.root) else {
        return false;
    };
    if !type_reachability::is_admitted_nested_owned_byte_record(&program.declarations, &root)
        || type_reachability::is_flat_owned_byte_record(&program.declarations, &root)
    {
        return false;
    }

    let mut ty = root;
    for projection in &loan.origin.projections {
        let PlaceProjection::Field(field) = projection else {
            return false;
        };
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &ty
        else {
            return false;
        };
        if !arguments.is_empty() {
            return false;
        }
        let Some(next) = program
            .declarations
            .record_fields(declaration)
            .and_then(|fields| fields.iter().find(|candidate| candidate.id == *field))
            .map(|field| field.ty.clone())
        else {
            return false;
        };
        ty = next;
    }
    ty == ResolvedType::Bytes
}
#[cfg(test)]
use inspection::extend_resolved_lifecycle_effects;
#[allow(
    unused_imports,
    reason = "private Workspace Semantic Graph Phase-A inspection seam"
)]
pub(crate) use inspection::workspace_call_sites;
pub(crate) use inspection::{
    budgeted_workspace_expression_identity, budgeted_workspace_expression_identity_text,
    validate_attached_identity_references, workspace_expression_identity,
};
use inspection::{path_is_prefix, reject_nul_identity, validate_nul_free_identities};
pub(crate) use inspection::{visit_resolved_calls, visit_workspace_call_sites};

pub(crate) use inspection::validate_core_with_storage as validate_core;
#[cfg(test)]
pub(crate) use type_reachability::owned_oracle::owned_reachable_types as owned_reachable_types_oracle;
pub(crate) use type_reachability::{
    authored_nominal_declarations, is_admitted_nested_owned_byte_record, is_flat_owned_byte_record,
    reachable_authored_types, reachable_authored_types_with_roots, reachable_scalar_authored_types,
    BorrowedTypeInventory,
};
pub(crate) use validation::resolved_type_contains_owned_bytes;
#[cfg(test)]
use validation::HirValidator;
pub(crate) use workspace_link::{
    copy_scalar_type, link_https_command_io_workspace, link_https_entry_workspace,
    link_language_command_io_workspace, link_line_command_io_workspace,
    link_network_command_io_workspace, link_network_entry_workspace, link_owned_data_api_workspace,
    link_package_scalar_workspace, link_scalar_project_exports, link_scalar_project_workspace,
    link_stdin_stream_command_workspace, link_stdin_stream_data_command_workspace,
    link_stdin_stream_data_entry_workspace, link_stdin_stream_exit_command_workspace,
    link_stdin_stream_text_command_workspace, link_stdin_stream_text_entry_workspace,
    link_useful_data_command_workspace, link_useful_data_workspace, link_useful_text_workspace,
    owned_data_api_workspace_return_admitted, package_scalar_type,
    stream_collection_record_signature_admitted, stream_data_parameter_admitted,
    stream_nested_outcome_signature_admitted, stream_owned_signature_admitted,
    stream_record_signature_admitted, stream_text_parameter_admitted,
    stream_text_parameter_with_index, stream_text_return_admitted, stream_text_return_with_index,
    useful_data_workspace_parameter_admitted, useful_data_workspace_return_admitted,
    validate_stream_collection_record_program, validate_stream_data_program,
    validate_stream_nested_outcome_program, validate_stream_owned_program,
    validate_stream_record_program, validate_stream_text_program, LinkedScalarProjectParts,
    COPY_SCALAR_NAMES, PACKAGE_SCALAR_NAME,
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

mod cache_codec;
mod declaration_index;
pub(crate) mod identity_resolution;
mod ordinary_identity;
pub use declaration_index::{dispose_declaration_index_for_private_contract, DeclarationIndex};
pub(crate) use declaration_index::{inventory_identity, inventory_substitute, InventoryPayer};
pub(crate) use declaration_index::{TypeFactsPayer, TypeFlags};

// Resolved data model and shared probes, re-exported so this module and its
// submodules keep reaching every HIR item through `hir::` as before.
#[cfg(test)]
pub(crate) use byte_capacity::byte_data_capacity_inputs;
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
mod vec_field_origin;
pub use expr_nodes::{
    PatternValue, ResolvedClosureCapture, ResolvedExpr, ResolvedExprKind, ResolvedFieldInitializer,
    ResolvedMatchArm, ResolvedMatchPattern, ResolvedMatchPatternField,
    ResolvedRecordMatchFieldPattern, ResolvedRecordMatchPatternField, ResolvedStatement,
};
pub(crate) use ids::CompilerDeclaration;
pub use ids::{DeclarationId, ExpressionId, FunctionExecutionId, FunctionInstanceId, ValueId};
pub use vec_field_origin::VecFieldReadOrigin;
pub(crate) mod row_loan;
pub(crate) use monomorphize::substitute_type;
use monomorphize::{
    materialize_function_template, resolved_owned_record_substitutions,
    resolved_scalar_substitutions, same_function_meaning,
};
pub use row_loan::{RowLoan, RowLoanVersion, ROW_LOAN_V1};
pub(crate) use validation::byte_traversal::nested::Slot as ByteTraversalSlot;
pub(crate) use validation::byte_traversal::{
    function_uses as byte_traversal_function_uses, partial_receiver as is_partial_byte_receiver,
    program_uses as byte_traversal_program_uses, region as byte_traversal_region,
};

pub(crate) fn is_exact_materialized_function_instance(
    template: &ResolvedFunctionTemplate,
    instance: &ResolvedFunctionInstance,
) -> bool {
    materialize_function_template(template, &instance.type_arguments)
        .is_ok_and(|expected| same_function_meaning(&expected, &instance.function))
}

pub(crate) fn bounded_owned_record_template_for_function<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Option<&'a ResolvedFunctionTemplate> {
    let mut matched = None;
    for template in program.function_templates() {
        let owned = template
            .params
            .iter()
            .filter(|parameter| parameter.ownership == OwnershipMode::Own)
            .collect::<Vec<_>>();
        let exact_relay = !owned.is_empty()
            && type_reachability::is_nested_owned_byte_record_template(
                program.declarations(),
                &template.return_type,
                &template.id,
                template.type_parameters.len(),
            )
            && owned.iter().all(|parameter| {
                type_reachability::is_nested_owned_byte_record_template(
                    program.declarations(),
                    &parameter.ty,
                    &template.id,
                    template.type_parameters.len(),
                )
            });
        if !exact_relay || !(1..=2).contains(&template.type_parameters.len()) {
            continue;
        }
        let exact = program.function_instances().iter().any(|instance| {
            instance.template == template.id
                && same_function_meaning(&instance.function, function)
                && is_exact_materialized_function_instance(template, instance)
        }) || resolved_owned_record_substitutions(template.type_parameters.len())
            .into_iter()
            .filter_map(|arguments| materialize_function_template(template, &arguments).ok())
            .any(|candidate| same_function_meaning(&candidate, function));
        if exact {
            if matched.is_some() {
                return None;
            }
            matched = Some(template);
        }
    }
    matched
}
pub(crate) use nodes::{
    admitted_owned_byte_prelude_instance, is_refutable_match_scalar, is_scalar_resolved_type,
    LinkedDeclarationFact, LinkedOwnedDataParts, LinkedScalarFunction,
};
pub use nodes::{
    ByteSliceExtent, ByteSliceProvenance, ByteSliceRangeStep, ByteSliceRootKind, Declaration,
    DeclarationKind, IdentityOrigin, OwnershipMode, ResolvedBinding, ResolvedFieldDeclaration,
    ResolvedFunction, ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedHostCommandCall,
    ResolvedHostCommandOperation, ResolvedImport, ResolvedImportFailure, ResolvedImportParameter,
    ResolvedImportResult, ResolvedImportResultKind, ResolvedInterface, ResolvedMatchMode,
    ResolvedNativeRustImportCall, ResolvedParam, ResolvedProgram, ResolvedResourceDrop,
    ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind,
    ResolvedTypeParameterDeclaration, ResolvedVariantCaseDeclaration, ResolvedYieldsClause,
    TypeFacts, VectorFieldProvenance,
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
    traversal_row: Option<Box<resolve_for::record::Row>>,
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
    /// Reconstructed by fresh let initialization in one contract clause only.
    contract_local: bool,
    mutable: bool,
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
#[cfg(test)]
struct ValidationScopePublication<'a> {
    target: &'a mut BTreeMap<ValueId, ValidationBinding>,
    published: BTreeMap<ValueId, ValidationBinding>,
    enabled: bool,
}

#[cfg(test)]
impl ValidationScopePublication<'_> {
    fn publish(&mut self, scope: &BTreeMap<ValueId, ValidationBinding>) {
        // Full equality includes nested move/partial/loan state and exact
        // types. Equal snapshots already have precisely the required early
        // error publication; avoid reallocating an unchanged whole map.
        if self.enabled && self.published != *scope {
            self.published.retain(|key, _| scope.contains_key(key));
            for (key, binding) in scope {
                if let Some(published) = self.published.get_mut(key) {
                    if published != binding {
                        published.clone_from(binding);
                    }
                } else {
                    self.published.insert(key.clone(), binding.clone());
                }
            }
        }
    }
}

#[cfg(test)]
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
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Resolve);
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
/// `private` only changes how source diagnostics name `program`'s private
/// layout copies.
///
/// Explicit legacy compatibility: ordinary workspace resolution uses the
/// identity-paid `identity_resolution::resolve_checked` sibling instead.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "legacy unpaid route retained as the saved oracle for the paid sibling"
    )
)]
pub(crate) fn resolve_with_function_reuse<'p>(
    program: &'p Program,
    previous_program: Option<&Program>,
    previous_resolved: Option<&ResolvedProgram>,
    previous_costs: Option<&BTreeMap<String, usize>>,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Resolve);
    let diagnostics = source_verify::verify_presenting(program, private);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(diagnostics);
    }
    let reuse = previous_program
        .zip(previous_resolved)
        .zip(previous_costs)
        .filter(|((previous, _), _)| function_reuse_environment_is_exact(program, previous))
        .map(|((previous, resolved), costs)| FunctionReuse {
            current: program,
            program: previous,
            resolved,
            costs,
        });
    resolve_verified(program, reuse).map_err(|error| vec![error])
}

/// Full ordinary verification of the private borrowed presentation. Recursive
/// record-invariant desugaring owns its AST only at the transformation boundary.
/// Explicit legacy compatibility; the paid sibling is
/// `identity_resolution::resolve_source`.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "legacy unpaid route retained as the saved oracle for the paid sibling"
    )
)]
pub(crate) fn resolve_source<'p>(
    program: &'p dyn ProgramSource,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>> {
    let diagnostics = source_verify::verify_presenting(program, private);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(diagnostics);
    }
    let clause_program = record_invariants::source::clause_program(program);
    let inventory: &dyn ProgramSource = clause_program.as_ref().map_or(program, |owned| owned);
    let resolved = resolve_source_pass(inventory)?;
    let Some(desugared) = record_invariants::source::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    resolve_source_pass(&desugared)
}

fn resolve_source_pass(
    program: &dyn ProgramSource,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>> {
    (Resolver {
        program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(program),
        declarations: DeclarationIndex::from_verified_with_prelude(
            program,
            crate::prelude::declarations_for_program(program),
        )
        .map_err(|error| vec![error])?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve()
    .map_err(|error| vec![error])
}

/// Resolve a verified program. Record Invariants v1: when a record declares
/// invariants, the resolution of the original supplies the record type of
/// every production site, and the program is resolved once more after
/// [`record_invariants::desugar`] routes those sites through their checks.
fn resolve_verified(
    program: &Program,
    reuse: Option<FunctionReuse<'_>>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Diagnostic> {
    let clause_program = record_invariants::clause_program(program);
    let inventory_program = clause_program.as_ref().unwrap_or(program);
    let resolved = (Resolver {
        program: inventory_program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(inventory_program),
        declarations: DeclarationIndex::from_verified(inventory_program)?,
        reuse: if clause_program.is_some() {
            None
        } else {
            reuse
        },
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve()?;
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    (Resolver {
        program: &desugared,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(&desugared),
        declarations: DeclarationIndex::from_verified(&desugared)?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve()
}

#[derive(Default)]
pub(crate) struct FunctionResolutionWork {
    costs: BTreeMap<String, usize>,
    reused: usize,
    static_text: bool,
}

impl FunctionResolutionWork {
    pub(crate) fn static_text(&self) -> bool {
        self.static_text
    }
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

    // The ordinary caller captures only its existing invocation ledger. A
    // setup-short compatibility call retains the original owned diagnostic
    // route; initial report setup is not claimed funded by this activation.
    if ordinary_identity::can_acquire() {
        match ordinary_identity::try_resolve_verified_compatible(program) {
            Ok(Ok((resolved, _))) => {
                return Analysis {
                    diagnostics,
                    resolved: Some(resolved),
                };
            }
            Ok(Err(diagnostics)) => {
                return Analysis {
                    diagnostics,
                    resolved: None,
                };
            }
            Err(_) => {} // Legacy setup-only diagnostic compatibility below.
        }
    }
    match resolve_verified(program, None) {
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

fn hir_error_static(message: &'static str) -> Diagnostic {
    Diagnostic::io_static("SPX-H006", message)
}

struct FunctionReuse<'a> {
    // The environment proof belongs to this exact current source receiver.
    current: &'a dyn ProgramSource,
    program: &'a dyn ProgramSource,
    resolved: &'a ResolvedProgram,
    costs: &'a BTreeMap<String, usize>,
}

struct Resolver<'a> {
    program: &'a dyn ProgramSource,
    byte_helpers: crate::loop_calls::byte_writer::SourceIndex<'a>,
    declarations: DeclarationIndex,
    reuse: Option<FunctionReuse<'a>>,
    function_work: FunctionResolutionWork,
    /// The held source selection's exact producer call, set only by the
    /// private selected factory. `None` on every ordinary resolution.
    fused_producer: Option<&'a crate::ast::Expr>,
    partition_profile: bool,
}

fn function_reuse_environment_is_exact(current: &dyn ProgramSource, previous: &Program) -> bool {
    // Generic declarations participate in the same complete ordered signature
    // proof, including scoped type parameters, contracts and source spans.
    // Only monomorphic HIR is cloned. Current templates and the entire instance
    // closure are rebuilt below from this exact source, never from reuse.resolved.
    current.path() == previous.path
        && current.module() == previous.module
        && current.module_uses() == previous.module_uses
        && current.permits() == previous.permits
        && current.types().iter().eq(&previous.types)
        && current.interfaces() == previous.interfaces
        && current.protocols() == previous.protocols
        && current.implementations() == previous.implementations
        && current.functions().len() == previous.functions.len()
        && current
            .functions()
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
        && current.yields == previous.yields
        && current.follows == previous.follows
        && current.requires == previous.requires
        && current.ensures == previous.ensures
        && current.span == previous.span
}

/// Source byte-helper admission reads current transitive bodies and keeps a
/// phase-owned proof memo/failure. Those call-bearing functions resolve afresh;
/// cloning their HIR would skip the source diagnostics and proof side effects.
/// Ordinary signature-bound calls, including generic calls, remain reusable.
fn function_requires_current_byte_helper_resolution(
    source: &dyn ProgramSource,
    function: &ResolvedFunction,
) -> bool {
    let mut requires = false;
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        inspection::visit_resolved_calls(expression, &mut |callee, _, _| {
            requires |= source.functions().iter().any(|target| {
                target.stable_id == callee.as_str()
                    && crate::loop_calls::byte_writer::SourceIndex::may_require_body_proof(target)
            });
        });
    }
    requires
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
mod generic_environment_reuse_tests {
    use super::*;
    const LIMIT: usize = 67_108_864;
    const SOURCE: &str = r#"module reuse.generic;
@id("reuse.first") fn first<T>(one:T,two:T)->T {one}
@id("reuse.other") fn other<T>(one:T,two:T)->T {two}
@id("reuse.relay") fn relay<T>(one:T,two:T)->T {first<T>(one,two)}
@id("reuse.helper") fn helper()->i64 {relay<i64>(3,4)}
@id("reuse.main") fn main()->i64 {if relay<i64>(3,4)==3 {helper()+1} else {0}}
"#;
    fn parse(source: &str) -> Program {
        crate::parse(source, "generic-reuse.spx").unwrap()
    }
    // Full ordinary current source resolution is the independent nonreuse oracle.
    fn fresh(program: &Program) -> (ResolvedProgram, FunctionResolutionWork) {
        let (result, refused, _) = crate::bounded_output::with_limit_usage(LIMIT, || {
            resolve_with_function_reuse(program, None, None, None, None)
        });
        assert!(!refused);
        result.unwrap()
    }
    fn reuse(
        current: &Program,
        previous: &Program,
        old: &ResolvedProgram,
        costs: &BTreeMap<String, usize>,
    ) -> (ResolvedProgram, FunctionResolutionWork) {
        let (result, refused, _) = crate::bounded_output::with_limit_usage(LIMIT, || {
            resolve_with_function_reuse(current, Some(previous), Some(old), Some(costs), None)
        });
        assert!(!refused);
        result.unwrap()
    }
    fn parity(
        current: &Program,
        previous: &Program,
        old: &ResolvedProgram,
        costs: &BTreeMap<String, usize>,
    ) -> ResolvedProgram {
        let oracle = fresh(current);
        let actual = reuse(current, previous, old, costs);
        assert_eq!(actual.0, oracle.0);
        assert_eq!(actual.1.static_text(), oracle.1.static_text());
        assert!(actual.1.reused() > 0);
        assert!(!actual.0.function_instances.is_empty());
        // Complete Eq above includes ordered current template/instance bodies,
        // identities, substitutions and every rebuilt cleanup/loan attachment.
        actual.0
    }
    #[test]
    fn generic_environment_reuses_call_bearing_helpers_and_rebuilds_current_closure() {
        let previous = parse(SOURCE);
        let (old, work) = fresh(&previous);
        let costs = work.costs();
        for source in [
            SOURCE.replace("helper()+1", "helper()+2"),
            SOURCE.replace("->T {one}", "->T {two}"),
            SOURCE.replace("{first<T>(one,two)}", "{other<T>(one,two)}"),
        ] {
            let current = parse(&source);
            assert!(function_reuse_environment_is_exact(&current, &previous));
            parity(&current, &previous, &old, &costs);
        }
        // Keep the original longer boolean replacement as an explicit drift
        // case: function_signature_is_exact includes the complete function span.
        let drifted = parse(&SOURCE.replace("relay<i64>(3,4)==3", "relay<bool>(true,false)    "));
        assert!(!function_reuse_environment_is_exact(&drifted, &previous));
        let expected = fresh(&drifted);
        let actual = reuse(&drifted, &previous, &old, &costs);
        assert_eq!(actual.0, expected.0);
        assert_eq!(actual.1.static_text(), expected.1.static_text());
        assert_eq!(actual.1.reused(), 0);
        assert!(!actual.0.function_instances.is_empty());

        // A separately resolved padded prior source gives the admitted boolean
        // call the exact original body width, preserving all declaration spans.
        let original_call = "relay<i64>(3,4)==3";
        let boolean_call = "relay<bool>(true,false)";
        let padding = " ".repeat(boolean_call.len() - original_call.len());
        let padded = parse(&SOURCE.replace(original_call, &format!("{original_call}{padding}")));
        let (padded_old, padded_work) = fresh(&padded);
        let changed = parse(&SOURCE.replace(original_call, boolean_call));
        assert!(function_reuse_environment_is_exact(&changed, &padded));
        parity(&changed, &padded, &padded_old, &padded_work.costs());

        // Deliberately corrupt every previous generic payload. None is a reuse
        // source: only complete previous monomorphic functions can be cloned.
        let mut stale = old.clone();
        stale.function_templates.reverse();
        stale.function_templates[0].body.ty = ResolvedType::String;
        stale.function_instances.reverse();
        stale.function_instances[0].type_arguments = vec![ResolvedType::String];
        stale.function_instances[0].function.body.ty = ResolvedType::Bytes;
        let current = parse(&SOURCE.replace("helper()+1", "helper()+2"));
        parity(&current, &previous, &stale, &costs);
        stale.function_instances.clear();
        stale.function_templates.clear();
        parity(&current, &previous, &stale, &costs);
    }
    #[test]
    fn generic_environment_drift_disables_reuse_and_detached_capability_is_not_current() {
        let previous = parse(SOURCE);
        let (old, work) = fresh(&previous);
        let costs = work.costs();
        let mut reordered = previous.clone();
        reordered.functions.swap(0, 1);
        let mut removed = previous.clone();
        removed.functions.remove(1);
        let mut added = previous.clone();
        let extra = parse("module reuse.generic; @id(\"reuse.extra\") fn extra()->i64 {0}");
        added.functions.push(extra.functions[0].clone());
        let mut parameter = previous.clone();
        parameter.functions[0].type_parameters[0].name = "U".into();
        for current in [&reordered, &removed, &added, &parameter] {
            assert!(!function_reuse_environment_is_exact(current, &previous));
        }
        for current in [&reordered, &removed, &added] {
            let oracle = fresh(current);
            let actual = reuse(current, &previous, &old, &costs);
            assert_eq!(actual.0, oracle.0);
            assert_eq!(actual.1.reused(), 0);
        }
        let detached = previous.clone();
        let oracle = fresh(&detached);
        let (actual, refused, _) = crate::bounded_output::with_limit_usage(LIMIT, || {
            resolve_verified(
                &detached,
                Some(FunctionReuse {
                    current: &previous,
                    program: &previous,
                    resolved: &old,
                    costs: &costs,
                }),
            )
        });
        assert!(!refused);
        let actual = actual.unwrap();
        assert_eq!(actual.0, oracle.0);
        assert_eq!(actual.1.reused(), 0);
    }
    #[test]
    fn yields_and_follows_signature_drift_matches_full_nonreuse_verdict() {
        let previous = parse(SOURCE);
        let (old, work) = fresh(&previous);
        let costs = work.costs();
        let mut clauses = previous.clone();
        let span = clauses.functions[0].span;
        clauses.functions[0].yields = Some(crate::ast::YieldsClause {
            request_type: crate::ast::Type::I64,
            response_type: crate::ast::Type::Bool,
            span,
        });
        clauses.functions[0].follows = Some(Box::new(crate::ast::SessionProtocolFollowsClause {
            protocol_id: "reuse.missing.protocol".into(),
            protocol_id_span: span,
            span,
        }));
        // Direct AST mutations also exercise hostile clauses without relying
        // on the source parser admitting an invalid generic/effect profile.
        let mut variants = vec![clauses.clone()];
        for attack in 0..6 {
            let mut changed = clauses.clone();
            let function = &mut changed.functions[0];
            match attack {
                0 => function.yields.as_mut().unwrap().request_type = crate::ast::Type::Bool,
                1 => function.yields.as_mut().unwrap().response_type = crate::ast::Type::I64,
                2 => function.yields.as_mut().unwrap().span = function.name_span,
                3 => function.follows.as_mut().unwrap().protocol_id = "reuse.other.protocol".into(),
                4 => function.follows.as_mut().unwrap().protocol_id_span = function.name_span,
                _ => function.follows.as_mut().unwrap().span = function.name_span,
            }
            assert!(!function_signature_is_exact(
                &changed.functions[0],
                &clauses.functions[0]
            ));
            assert!(!function_reuse_environment_is_exact(&changed, &clauses));
            variants.push(changed);
        }
        for current in &variants {
            let mut pairs = vec![(current, &previous), (&previous, current)];
            if current != &clauses {
                pairs.push((current, &clauses));
                pairs.push((&clauses, current));
            }
            for (current, prior) in pairs {
                assert!(!function_reuse_environment_is_exact(current, prior));
                let run = |reuse: bool| {
                    let (result, refused, _) =
                        crate::bounded_output::with_limit_usage(LIMIT, || {
                            resolve_with_function_reuse(
                                current,
                                reuse.then_some(prior),
                                reuse.then_some(&old),
                                reuse.then_some(&costs),
                                None,
                            )
                        });
                    assert!(!refused);
                    result.map(|(resolved, work)| {
                        assert_eq!(work.reused(), 0);
                        resolved
                    })
                };
                // Complete programs on success, or all diagnostics in exact
                // order including spans/messages on refusal, stay identical.
                assert_eq!(run(true), run(false));
            }
        }
    }
    fn full_verdict(
        current: &Program,
        previous: Option<(&Program, &ResolvedProgram, &BTreeMap<String, usize>)>,
    ) -> Result<(ResolvedProgram, usize), Vec<Diagnostic>> {
        let (result, refused, _) = crate::bounded_output::with_limit_usage(LIMIT, || {
            resolve_with_function_reuse(
                current,
                previous.map(|p| p.0),
                previous.map(|p| p.1),
                previous.map(|p| p.2),
                None,
            )
        });
        assert!(!refused);
        result.map(|(program, work)| (program, work.reused()))
    }
    #[test]
    fn current_byte_helper_body_proofs_keep_full_fresh_verdict_and_scalar_generic_reuse() {
        const BYTES: &str = include_str!("../tests/owned_data/byte_helper_renewal.spx");
        let source = format!(
            "{BYTES}\n@id(\"reuse.scalar\") fn scalar<T>(value:T)->T {{value}}\n@id(\"reuse.extra\") fn extra()->i64 {{scalar<i64>(7)}}\n"
        );
        let previous = parse(&source);
        let (old, work) = fresh(&previous);
        let costs = work.costs();
        for id in ["helper.relay", "app.main"] {
            let function = old.functions.iter().find(|f| f.id.as_str() == id).unwrap();
            assert!(function_requires_current_byte_helper_resolution(
                &previous, function
            ));
        }
        let scalar = old
            .functions
            .iter()
            .find(|f| f.id.as_str() == "reuse.extra")
            .unwrap();
        assert!(!function_requires_current_byte_helper_resolution(
            &previous, scalar
        ));
        assert!(costs["reuse.extra"] > 0);
        let original = "next=bytes_set(next,index,value);";
        let cycle = "next=leaf(next,index,value);";
        let cycle = format!("{cycle}{}", " ".repeat(original.len() - cycle.len()));
        let mut allocating = previous.clone();
        let donor = parse(
            "module donor; @id(\"donor.main\") fn main()->i64 {let ignored=bytes_zeroed(1usize);0}",
        );
        let crate::ast::ExprKind::Block {
            statements: donor, ..
        } = &donor.functions[0].body.kind
        else {
            panic!("donor block")
        };
        let crate::ast::ExprKind::Block { statements, .. } = &mut allocating.functions[0].body.kind
        else {
            panic!("helper block")
        };
        statements.insert(0, donor[0].clone());
        // Keep exact declaration spans/signatures while changing a source-valid
        // callee body into an allocating profile. Its unchanged loop callers
        // must reproduce the ordinary source refusal, before HIR replay.
        assert!(function_reuse_environment_is_exact(&allocating, &previous));
        assert_eq!(
            allocating.functions[1], previous.functions[1],
            "wrapper caller is unchanged"
        );
        let (_, refused, _) = crate::bounded_output::with_limit_usage(LIMIT, || {
            let index = crate::loop_calls::byte_writer::SourceIndex::new(&allocating);
            assert!(!index.target(&allocating.functions[1]));
            assert!(
                index.failure().is_none(),
                "body profile refusal is not resource refusal"
            );
        });
        assert!(!refused);
        let invalid_oracle = full_verdict(&allocating, None);
        assert_eq!(invalid_oracle.as_ref().unwrap_err()[0].code, "SPX-T252");
        assert!(invalid_oracle.as_ref().unwrap_err()[0]
            .message
            .contains("call `leaf` is not admitted in loop bodies"));
        assert_eq!(
            full_verdict(&allocating, Some((&previous, &old, &costs)))
                .as_ref()
                .map(|p| &p.0),
            invalid_oracle.as_ref().map(|p| &p.0),
        );
        let variants = [
            (source.clone(), true),
            (
                source.replace("bytes_set(next,index,value)", "bytes_set(next,index,33u8 )"),
                true,
            ),
            (source.replace("at=at+1usize", "at=at+2usize"), true),
            (source.replace(original, &cycle), false),
        ];
        for (variant, (source, admitted)) in variants.into_iter().enumerate() {
            let current = parse(&source);
            assert!(function_reuse_environment_is_exact(&current, &previous));
            let oracle = full_verdict(&current, None);
            let actual = full_verdict(&current, Some((&previous, &old, &costs)));
            assert_eq!(actual.as_ref().map(|p| &p.0), oracle.as_ref().map(|p| &p.0));
            assert_eq!(actual.is_ok(), admitted);
            if let Ok((_, reused)) = actual {
                assert!(
                    reused > 0,
                    "ordinary generic scalar helper remains genuinely reusable"
                );
                if variant == 1 {
                    // Leaf changed, relay/main require current proofs, and the
                    // generic declaration resolves fresh. Only extra can clone.
                    assert_eq!(
                        reused, 1,
                        "the unchanged generic call-bearing extra was reused"
                    );
                }
            }
        }
        let ((result, observed), refused, required) =
            crate::bounded_output::with_limit_usage(LIMIT, || {
                crate::test_allocation_observer::measure(|| {
                    let actual = resolve_with_function_reuse(
                        &previous,
                        Some(&previous),
                        Some(&old),
                        Some(&costs),
                        None,
                    )?;
                    assert_eq!(actual.0, old);
                    assert!(actual.1.reused() > 0);
                    drop(actual);
                    Ok::<_, Vec<Diagnostic>>(())
                })
            });
        result.unwrap();
        assert!(!refused && !observed.invalid);
        assert!(observed.allocated > 0);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(required > 0);
        let (exact, refused, used) = crate::bounded_output::with_limit_usage(required, || {
            resolve_with_function_reuse(&previous, Some(&previous), Some(&old), Some(&costs), None)
        });
        assert!(!refused);
        assert_eq!(used, required);
        assert_eq!(exact.unwrap().0, old);
        let (short, refused, _) = crate::bounded_output::with_limit_usage(required - 1, || {
            resolve_with_function_reuse(&previous, Some(&previous), Some(&old), Some(&costs), None)
        });
        assert!(refused);
        assert!(short.is_err());
    }
    #[test]
    fn current_agent_metadata_replays_with_no_previous_agent_authority() {
        let source = format!(
            "{}\n@id(\"reuse.scalar\") fn scalar<T>(value:T)->T {{value}}\n@id(\"reuse.extra\") fn extra()->i64 {{scalar<i64>(7)}}\n",
            crate::parser::agent_embedded_tests::source()
        );
        let previous = parse(&source);
        let (old, work) = fresh(&previous);
        let costs = work.costs();
        let mut stale = old.clone();
        stale.agents[0].name = "stale".into();
        stale.agents[0].runtime_v1_json = "stale authority".into();
        stale.agents[0].operations.clear();
        let mut renamed = previous.clone();
        renamed.agents[0].name = "CurrentAgent".into();
        let mut invalid_origin = previous.clone();
        invalid_origin.agents[0].operations[0].embedded_function_index = Some(usize::MAX);
        let mut invalid_target = previous.clone();
        invalid_target.agents[0].operations[0].stable_id = "missing.operation".into();
        let mut invalid_kind = previous.clone();
        invalid_kind.agents[0].operations[0].kind = crate::ast::AgentOperationKind::Model;
        let mut invalid_wait = previous.clone();
        invalid_wait.agents[0].model_wait = Some(Box::new(crate::ast::AgentModelWaitBinding {
            helper_id: "op.initialize".into(),
            span: previous.agents[0].span,
        }));
        for (current, admitted) in [
            (&renamed, true),
            (&invalid_origin, false),
            (&invalid_target, false),
            (&invalid_kind, false),
            (&invalid_wait, false),
        ] {
            assert!(function_reuse_environment_is_exact(current, &previous));
            let oracle = full_verdict(current, None);
            let actual = full_verdict(current, Some((&previous, &stale, &costs)));
            assert_eq!(actual.as_ref().map(|p| &p.0), oracle.as_ref().map(|p| &p.0));
            assert_eq!(actual.is_ok(), admitted);
            if let Ok((resolved, reused)) = actual {
                assert!(reused > 0);
                assert_eq!(resolved.agents[0].name, "CurrentAgent");
                assert_eq!(
                    resolved.agents[0].runtime_v1_json,
                    current.agents[0].runtime_v1_json
                );
                assert!(!resolved.agents[0].operations.is_empty());
            }
        }
    }
    fn helper_call(body: &mut ResolvedExpr) -> &mut ResolvedExpr {
        if matches!(&body.kind, ResolvedExprKind::Block { .. }) {
            let ResolvedExprKind::Block { tail, .. } = &mut body.kind else {
                unreachable!()
            };
            helper_call(tail)
        } else {
            body
        }
    }
    #[test]
    fn stale_reused_generic_call_identity_and_arguments_fail_current_binding() {
        let previous = parse(SOURCE);
        let (old, work) = fresh(&previous);
        let costs = work.costs();
        let current = parse(&SOURCE.replace("helper()+1", "helper()+2"));
        for attack in 0..3 {
            let mut stale = old.clone();
            let helper = stale
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "reuse.helper")
                .unwrap();
            let ResolvedExprKind::Call {
                callee,
                instance,
                type_arguments,
                ..
            } = &mut helper_call(&mut helper.body).kind
            else {
                panic!("call-bearing helper")
            };
            match attack {
                0 => *instance = Some(FunctionInstanceId::derive(callee, &[ResolvedType::Bool])),
                1 => type_arguments[0] = ResolvedType::Bool,
                _ => {
                    *callee = DeclarationId::new("reuse.missing");
                    *instance = Some(FunctionInstanceId::derive(callee, type_arguments));
                }
            }
            let (result, refused, _) = crate::bounded_output::with_limit_usage(LIMIT, || {
                resolve_with_function_reuse(
                    &current,
                    Some(&previous),
                    Some(&stale),
                    Some(&costs),
                    None,
                )
            });
            assert!(!refused);
            let errors = result.err().expect("stale generic binding rejects");
            assert_eq!(errors[0].code, "SPX-H006");
            assert_eq!(
                errors[0].message,
                if attack == 2 {
                    "generic function call has no resolved template"
                } else {
                    "generic function call has an inconsistent instance identity"
                }
            );
        }
    }
    #[test]
    fn generic_reuse_paid_clone_and_current_instances_settle_and_keep_exact_limit() {
        let previous = parse(SOURCE);
        let (old, work) = fresh(&previous);
        let costs = work.costs();
        assert!(costs["reuse.helper"] > 0);
        let current = parse(&SOURCE.replace("helper()+1", "helper()+2"));
        let oracle = fresh(&current);
        let ((result, observed), refused, required) =
            crate::bounded_output::with_limit_usage(LIMIT, || {
                crate::test_allocation_observer::measure(|| {
                    let actual = resolve_with_function_reuse(
                        &current,
                        Some(&previous),
                        Some(&old),
                        Some(&costs),
                        None,
                    )?;
                    assert_eq!(actual.0, oracle.0);
                    assert!(actual.1.reused() > 0);
                    drop(actual);
                    Ok::<_, Vec<Diagnostic>>(())
                })
            });
        result.unwrap();
        assert!(!refused);
        assert!(required > 0);
        assert!(!observed.invalid);
        assert!(observed.allocated > 0);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        let (exact, refused, used) = crate::bounded_output::with_limit_usage(required, || {
            resolve_with_function_reuse(&current, Some(&previous), Some(&old), Some(&costs), None)
        });
        assert!(!refused);
        assert_eq!(used, required);
        assert_eq!(exact.unwrap().0, oracle.0);
        let (short, refused, _) = crate::bounded_output::with_limit_usage(required - 1, || {
            resolve_with_function_reuse(&current, Some(&previous), Some(&old), Some(&costs), None)
        });
        assert!(refused);
        assert!(short.is_err());
    }
}

#[cfg(test)]
#[path = "hir/iterative_validator_tests.rs"]
mod iterative_validator_tests;

#[cfg(test)]
#[path = "hir/scope_publication_tests.rs"]
mod scope_publication_tests;

#[cfg(test)]
#[path = "hir/record_tests.rs"]
mod record_tests;

pub(crate) mod collection_outcome;
pub(crate) mod copy_record_collection;
pub(crate) mod declared_collection;
pub(crate) mod generic_collection;
mod generic_inference;
pub(crate) mod generic_result;
pub(crate) mod iterator_loop;
pub(crate) mod owned_leaf_collection;
pub(crate) mod owned_record_collection;
pub(crate) mod renewal_probes;
pub(crate) mod vec_loop_renewal;

pub(crate) mod generic_mapping;

pub(crate) mod generic_variant;

/// Physical shared resolver frame-slot layout, authenticated by this compiled
/// crate: one frame plus the at most one private path-arena node a frame-stack
/// slot can keep live (the arena never holds more nodes than frame capacity).
#[doc(hidden)]
pub const RESOLVER_FRAME_BYTES: usize = std::mem::size_of::<resolve_expr_frame::Frame<'static>>()
    + resolve_expr::path_arena::PATH_NODE_BYTES;
/// The iterative validator independently asserts this exact physical layout.
#[doc(hidden)]
pub const VALIDATOR_FRAME_BYTES: usize = 248;

/// Physical source verifier frame, shared without exposing its private module.
#[doc(hidden)]
pub const SOURCE_VERIFIER_FRAME_BYTES: usize = crate::source_verify::VERIFIER_FRAME_BYTES;

#[cfg(test)]
#[path = "hir/provisional_result_borrow_tests.rs"]
mod provisional_result_borrow_tests;

mod function_reuse_source;
