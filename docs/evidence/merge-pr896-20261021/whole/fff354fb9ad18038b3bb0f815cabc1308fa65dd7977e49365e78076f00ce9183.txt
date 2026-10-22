//! Program-level AST lowering.
//!
//! Entry point resolution, record layout validation, function and
//! function-template lowering, instance discovery, and type lowering.

use crate::ast::program_source::FunctionRef;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::ast::{
    ImportFailure, ParamMode, ResourceLifecycleKind, Span, Type, TypeDeclarationKind,
};
use crate::bounded_output::{reservation_stage_guard, staged, ReservationStage};
use crate::cleanup::CleanupInventory;
use crate::cleanup_plan::CleanupPlan;
use crate::diagnostic::Diagnostic;
use crate::loan_plan::LoanPlan;

use self::agent_identity::AgentIdentityFailure;
use super::byte_slice_provenance::derive_byte_slice_provenance;
use super::hir_error;
use super::ids::{DeclarationId, FunctionExecutionId, FunctionInstanceId, ValueId};
use super::monomorphize::materialize_function_template;
use super::nodes::{
    admitted_owned_byte_prelude_instance, DeclarationKind, OwnershipMode, ResolvedFunction,
    ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedImport, ResolvedImportFailure,
    ResolvedImportParameter, ResolvedImportResult, ResolvedImportResultKind, ResolvedInterface,
    ResolvedParam, ResolvedProgram, ResolvedResourceDrop, ResolvedResourceDropKind, ResolvedType,
    ResolvedTypeDeclaration, ResolvedTypeDeclarationKind, ResolvedTypeParameterDeclaration,
};
use super::{validate, Binding, Resolver};

include!("resolve_program_methods.rs");

#[cfg(test)]
mod tests;

pub(in crate::hir) mod agent_identity;
#[cfg(test)]
pub(in crate::hir) mod agent_identity_original;
pub(super) mod initial_loan_plans;
mod reuse_lowered;
