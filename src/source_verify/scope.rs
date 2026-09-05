//! Verifier scope and control state: lexical binding scopes, the explicit
//! frame stack that drives iterative expression checking, and the match state
//! machines for record, variant, and scalar matches.

use super::binding::{Binding, CheckedValue, SourceLoanId};
#[cfg(test)]
use super::high_water::{ast_type_owned_capacity, binding_owned_capacity};
use crate::ast::{
    BinaryOp, Expr, FieldDeclaration, Function, ImportDeclaration, MatchMode, Param, Span,
    Statement, Type, TypeDeclaration, UnaryOp, VariantCaseDeclaration,
};
#[cfg(test)]
use crate::diagnostic::Diagnostic;
use std::collections::{HashMap, HashSet};

pub(super) struct VerifierScope {
    pub(super) bindings: HashMap<String, Binding>,
    pub(super) local_borrow_count: usize,
}

pub(super) enum VerifierFrame<'a> {
    Enter {
        expression: &'a Expr,
        scope: usize,
    },
    ResumeUnary {
        expression: &'a Expr,
        operand: &'a Expr,
        op: UnaryOp,
    },
    ResumeBinaryLeft {
        expression: &'a Expr,
        op: BinaryOp,
        right: &'a Expr,
        scope: usize,
    },
    ResumeBinaryRight {
        expression: &'a Expr,
        op: BinaryOp,
        left: &'a Expr,
        left_value: Option<CheckedValue>,
        scope: usize,
        evaluated_scope: usize,
        baseline_names: Vec<String>,
    },
    ResumeIfCondition {
        expression: &'a Expr,
        then_branch: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
    },
    ResumeIfThen {
        expression: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
        then_scope: usize,
        baseline_names: Vec<String>,
    },
    ResumeIfElse {
        expression: &'a Expr,
        then_branch: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
        else_scope: usize,
        baseline_names: Vec<String>,
        then_value: Option<CheckedValue>,
        then_bindings: HashMap<String, Binding>,
    },
    ResumeBlockStatement {
        expression: &'a Expr,
        statements: &'a [Statement],
        tail: &'a Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
    },
    ResumeWhileCondition {
        condition: &'a Expr,
    },
    ResumeWhileBody {
        expression: &'a Expr,
        statements: &'a [Statement],
        tail: &'a Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
        statement_span: Span,
        baseline_names: Vec<String>,
        baseline_bindings: HashMap<String, Binding>,
    },
    ResumeBlockTail {
        parent_scope: usize,
        block_scope: usize,
        outer_names: Vec<String>,
    },
    ResumeCallArgument {
        expression: &'a Expr,
        name: &'a str,
        args: &'a [Expr],
        scope: usize,
        index: usize,
        target: VerifierCallTarget<'a>,
        borrowed_bytes_loans: Vec<(String, SourceLoanId)>,
    },
    ResumeMethodReceiver {
        expression: &'a Expr,
        receiver: &'a Expr,
        method: &'a str,
        args: &'a [Expr],
        scope: usize,
    },
    ResumeMethodArgument {
        expression: &'a Expr,
        method: &'a Function,
        args: &'a [Expr],
        scope: usize,
        index: usize,
    },
    ResumeTry {
        expression: &'a Expr,
        operand: &'a Expr,
        scope: usize,
    },
    ResumeProject {
        expression: &'a Expr,
        base: &'a Expr,
        field: &'a str,
    },
    ResumeRecordField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: Option<&'a [FieldDeclaration]>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareRecordField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: Option<&'a [FieldDeclaration]>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeVariantField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        case_name: &'a str,
        fields: &'a [crate::ast::FieldInitializer],
        declaration: Option<&'a TypeDeclaration>,
        case: Option<&'a VariantCaseDeclaration>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareVariantField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        case_name: &'a str,
        fields: &'a [crate::ast::FieldInitializer],
        declaration: Option<&'a TypeDeclaration>,
        case: Option<&'a VariantCaseDeclaration>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeUpdateBase {
        expression: &'a Expr,
        base: &'a Expr,
        fields: &'a [crate::ast::FieldInitializer],
        scope: usize,
    },
    ResumeUpdateField {
        expression: &'a Expr,
        base_type: Type,
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: &'a [FieldDeclaration],
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareUpdateField {
        expression: &'a Expr,
        base_type: Type,
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: &'a [FieldDeclaration],
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeMatchScrutinee {
        expression: &'a Expr,
        scrutinee: &'a Expr,
        arms: &'a [crate::ast::MatchArm],
        scope: usize,
    },
    ResumeRecordMatchArm {
        arm: &'a crate::ast::MatchArm,
        parent_scope: usize,
        arm_scope: usize,
        outer_names: Vec<String>,
    },
    PrepareVariantMatchArm(VariantMatchState<'a>),
    ResumeVariantMatchArm {
        state: VariantMatchState<'a>,
        arm_scope: usize,
    },
    /// Refutable Match v1: one decision-chain state machine per scalar
    /// match, mirroring `VariantMatchState` without case bookkeeping.
    PrepareScalarMatchArm(ScalarMatchState<'a>),
    ResumeScalarMatchGuard {
        state: ScalarMatchState<'a>,
        arm_scope: usize,
    },
    ResumeScalarMatchArm {
        state: ScalarMatchState<'a>,
        arm_scope: usize,
    },
}

#[allow(dead_code)]
pub(super) struct ScalarMatchState<'a> {
    pub(super) expression: &'a Expr,
    pub(super) arms: &'a [crate::ast::MatchArm],
    pub(super) parent_scope: usize,
    pub(super) index: usize,
    pub(super) scrutinee_ty: Type,
    pub(super) outer_names: Vec<String>,
    pub(super) baseline: HashMap<String, Binding>,
    pub(super) arm_states: Vec<HashMap<String, Binding>>,
    pub(super) result: Option<CheckedValue>,
}

/// Refutable Match v1: the declared scalar type of an AST literal pattern.
pub(super) fn pattern_literal_type(value: crate::ast::PatternLiteral) -> Type {
    match value {
        crate::ast::PatternLiteral::Int(_) => Type::I64,
        crate::ast::PatternLiteral::Int32(_) => Type::I32,
        crate::ast::PatternLiteral::Uint8(_) => Type::U8,
        crate::ast::PatternLiteral::Usize(_) => Type::Usize,
        crate::ast::PatternLiteral::Char(_) => Type::Char,
        crate::ast::PatternLiteral::Bool(_) => Type::Bool,
    }
}

pub(super) struct VariantMatchState<'a> {
    pub(super) expression: &'a Expr,
    pub(super) arms: &'a [crate::ast::MatchArm],
    pub(super) parent_scope: usize,
    pub(super) index: usize,
    pub(super) outer_names: Vec<String>,
    pub(super) baseline: HashMap<String, Binding>,
    pub(super) arm_states: Vec<HashMap<String, Binding>>,
    pub(super) covered: HashSet<String>,
    pub(super) wildcard_seen: bool,
    pub(super) result: Option<CheckedValue>,
    pub(super) variant_name: Option<String>,
    pub(super) variant_arguments: Vec<Type>,
    pub(super) declared_cases: Option<&'a [VariantCaseDeclaration]>,
    pub(super) mode: MatchMode,
    pub(super) needs_drop: bool,
}

pub(super) enum VerifierCallTarget<'a> {
    Native(&'a ImportDeclaration),
    Byte(crate::byte_ops::ByteOp),
    HostIo(crate::host_io_ops::HostIoOp),
    CommandIo(crate::hir::ResolvedHostCommandOperation),
    Ordinary(Option<VerifierFunctionSignature<'a>>),
}

pub(super) enum VerifierFunctionSignature<'a> {
    Borrowed(&'a Function),
    Specialized {
        params: Vec<Param>,
        return_type: Type,
        implicit_unique_ownership: bool,
    },
}

#[cfg(test)]
pub(super) fn verifier_signature_owned_capacity(
    signature: &VerifierFunctionSignature<'_>,
) -> usize {
    match signature {
        VerifierFunctionSignature::Borrowed(_) => 0,
        VerifierFunctionSignature::Specialized {
            params,
            return_type,
            ..
        } => params
            .capacity()
            .saturating_mul(std::mem::size_of::<Param>())
            .saturating_add(
                params
                    .iter()
                    .map(|param| {
                        param
                            .name
                            .capacity()
                            .saturating_add(ast_type_owned_capacity(&param.ty))
                    })
                    .sum::<usize>(),
            )
            .saturating_add(ast_type_owned_capacity(return_type)),
    }
}

#[cfg(test)]
pub(super) fn variant_match_state_owned_capacity(state: &VariantMatchState<'_>) -> usize {
    state
        .outer_names
        .capacity()
        .saturating_mul(std::mem::size_of::<String>())
        .saturating_add(
            state
                .outer_names
                .iter()
                .map(String::capacity)
                .sum::<usize>(),
        )
        .saturating_add(
            state
                .baseline
                .capacity()
                .saturating_mul(std::mem::size_of::<(String, Binding)>()),
        )
        .saturating_add(
            state
                .baseline
                .iter()
                .map(|(name, binding)| name.capacity() + binding_owned_capacity(binding))
                .sum::<usize>(),
        )
        .saturating_add(
            state
                .arm_states
                .capacity()
                .saturating_mul(std::mem::size_of::<HashMap<String, Binding>>()),
        )
        .saturating_add(
            state
                .arm_states
                .iter()
                .map(|bindings| {
                    bindings
                        .capacity()
                        .saturating_mul(std::mem::size_of::<(String, Binding)>())
                        .saturating_add(
                            bindings
                                .iter()
                                .map(|(name, binding)| {
                                    name.capacity() + binding_owned_capacity(binding)
                                })
                                .sum::<usize>(),
                        )
                })
                .sum::<usize>(),
        )
        .saturating_add(
            state
                .covered
                .capacity()
                .saturating_mul(std::mem::size_of::<String>()),
        )
        .saturating_add(state.covered.iter().map(String::capacity).sum::<usize>())
        .saturating_add(state.variant_name.as_ref().map_or(0, String::capacity))
        .saturating_add(
            state
                .variant_arguments
                .capacity()
                .saturating_mul(std::mem::size_of::<Type>()),
        )
        .saturating_add(
            state
                .variant_arguments
                .iter()
                .map(ast_type_owned_capacity)
                .sum::<usize>(),
        )
        .saturating_add(
            state
                .result
                .as_ref()
                .map_or(0, |value| ast_type_owned_capacity(&value.ty)),
        )
}

#[cfg(test)]
pub(super) fn diagnostics_owned_capacity(diagnostics: &Vec<Diagnostic>) -> usize {
    diagnostics.capacity() * std::mem::size_of::<Diagnostic>()
        + diagnostics
            .iter()
            .map(|diagnostic| {
                diagnostic.message.capacity()
                    + diagnostic.path.as_ref().map_or(0, String::capacity)
                    + diagnostic.help.as_ref().map_or(0, String::capacity)
            })
            .sum::<usize>()
}

#[cfg(test)]
pub(super) fn verifier_frame_owned_capacity(frame: &VerifierFrame<'_>) -> usize {
    let strings = |values: &Vec<String>| {
        values
            .capacity()
            .saturating_mul(std::mem::size_of::<String>())
            .saturating_add(values.iter().map(String::capacity).sum::<usize>())
    };
    match frame {
        VerifierFrame::ResumeBinaryRight { baseline_names, .. }
        | VerifierFrame::ResumeIfThen { baseline_names, .. } => strings(baseline_names),
        VerifierFrame::ResumeIfElse {
            baseline_names,
            then_bindings,
            ..
        } => strings(baseline_names).saturating_add(
            then_bindings
                .capacity()
                .saturating_mul(std::mem::size_of::<(String, Binding)>())
                .saturating_add(
                    then_bindings
                        .iter()
                        .map(|(name, binding)| name.capacity() + binding_owned_capacity(binding))
                        .sum::<usize>(),
                ),
        ),
        VerifierFrame::ResumeBlockStatement { outer_names, .. }
        | VerifierFrame::ResumeBlockTail { outer_names, .. }
        | VerifierFrame::ResumeRecordMatchArm { outer_names, .. } => strings(outer_names),
        VerifierFrame::ResumeWhileBody {
            outer_names,
            baseline_names,
            baseline_bindings,
            ..
        } => strings(outer_names)
            .saturating_add(strings(baseline_names))
            .saturating_add(
                baseline_bindings
                    .capacity()
                    .saturating_mul(std::mem::size_of::<(String, Binding)>())
                    .saturating_add(
                        baseline_bindings
                            .iter()
                            .map(|(name, binding)| {
                                name.capacity() + binding_owned_capacity(binding)
                            })
                            .sum::<usize>(),
                    ),
            ),
        VerifierFrame::ResumeRecordField { supplied, .. }
        | VerifierFrame::PrepareRecordField { supplied, .. }
        | VerifierFrame::ResumeVariantField { supplied, .. }
        | VerifierFrame::PrepareVariantField { supplied, .. } => supplied
            .capacity()
            .saturating_mul(std::mem::size_of::<&str>()),
        VerifierFrame::ResumeUpdateField {
            base_type,
            supplied,
            ..
        }
        | VerifierFrame::PrepareUpdateField {
            base_type,
            supplied,
            ..
        } => ast_type_owned_capacity(base_type).saturating_add(
            supplied
                .capacity()
                .saturating_mul(std::mem::size_of::<&str>()),
        ),
        VerifierFrame::ResumeCallArgument { target, .. } => match target {
            VerifierCallTarget::Native(_) => 0,
            VerifierCallTarget::Byte(_)
            | VerifierCallTarget::HostIo(_)
            | VerifierCallTarget::CommandIo(_) => 0,
            VerifierCallTarget::Ordinary(Some(signature)) => {
                verifier_signature_owned_capacity(signature)
            }
            VerifierCallTarget::Ordinary(None) => 0,
        },
        VerifierFrame::PrepareVariantMatchArm(state)
        | VerifierFrame::ResumeVariantMatchArm { state, .. } => {
            variant_match_state_owned_capacity(state)
        }
        _ => 0,
    }
}

impl VerifierFunctionSignature<'_> {
    pub(super) fn params(&self) -> &[Param] {
        match self {
            Self::Borrowed(function) => &function.params,
            Self::Specialized { params, .. } => params,
        }
    }

    pub(super) fn return_type(&self) -> &Type {
        match self {
            Self::Borrowed(function) => &function.return_type,
            Self::Specialized { return_type, .. } => return_type,
        }
    }

    pub(super) fn implicit_unique_ownership(&self) -> bool {
        match self {
            Self::Borrowed(_) => true,
            Self::Specialized {
                implicit_unique_ownership,
                ..
            } => *implicit_unique_ownership,
        }
    }
}
