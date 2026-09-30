//! Inert exact-function proof. Checked Agent association is a later consumer.
use super::CheckedOwnedFrameHelperV2;
use crate::cleanup_plan::{ExitContinuation, FinalizeAction};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedStatement, ResolvedType,
};

#[derive(Clone)]
pub(crate) struct CheckedOwnedAuthorizeV2 {
    helper: CheckedOwnedFrameHelperV2,
    function: DeclarationId,
    decision: DeclarationId,
    granted: DeclarationId,
    refused: DeclarationId,
    seal: DeclarationId,
    disposal: Vec<FinalizeAction>,
    partial_disposal: Vec<FinalizeAction>,
}
impl CheckedOwnedAuthorizeV2 {
    pub(crate) fn helper(&self) -> &CheckedOwnedFrameHelperV2 {
        &self.helper
    }
    pub(crate) fn decision(&self) -> &DeclarationId {
        &self.decision
    }
    pub(crate) fn granted(&self) -> &DeclarationId {
        &self.granted
    }
    pub(crate) fn refused(&self) -> &DeclarationId {
        &self.refused
    }
    pub(crate) fn seal(&self) -> &DeclarationId {
        &self.seal
    }
    pub(crate) fn disposal(&self) -> &[FinalizeAction] {
        &self.disposal
    }
    pub(crate) fn partial_disposal(&self) -> &[FinalizeAction] {
        &self.partial_disposal
    }
    pub(crate) fn function(&self) -> &ResolvedFunction {
        self.helper
            .program()
            .functions
            .iter()
            .find(|f| f.id == self.function)
            .expect("checked authorize")
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io(
        "SPX-T303",
        "owned authorize outside bounded checked profile",
    )
}

pub(crate) fn compile_owned_authorize_v2(
    helper: &CheckedOwnedFrameHelperV2,
    function: &DeclarationId,
) -> Result<CheckedOwnedAuthorizeV2, Diagnostic> {
    let p = helper.program();
    hir::validate(p)?;
    let f = p
        .functions
        .iter()
        .find(|f| f.id == *function)
        .ok_or_else(refused)?;
    if !p
        .declarations
        .declaration(function)
        .is_some_and(|d| d.identity_origin == hir::IdentityOrigin::Explicit)
        || f.yields.is_some()
        || !f.effects.is_empty()
        || f.params.first().is_none_or(|v| {
            v.ownership != OwnershipMode::Borrow || v.ty != helper.function().params[0].ty
        })
    {
        return Err(refused());
    }
    let ResolvedType::Nominal {
        declaration: proposal,
        ..
    } = &helper
        .function()
        .yields
        .as_ref()
        .expect("checked yields")
        .response_type
    else {
        return Err(refused());
    };
    let fields = p.declarations.record_fields(proposal).ok_or_else(refused)?;
    if f.params.len() != fields.len() + 1
        || f.params[1..]
            .iter()
            .zip(fields)
            .any(|(param, field)| param.ty != field.ty || param.ownership != OwnershipMode::Value)
    {
        return Err(refused());
    }
    let ResolvedType::Nominal {
        declaration: decision,
        arguments,
    } = &f.return_type
    else {
        return Err(refused());
    };
    let cases = p.declarations.variant_cases(decision).ok_or_else(refused)?;
    if !arguments.is_empty() || cases.len() != 2 {
        return Err(refused());
    }
    let granted = cases
        .iter()
        .find(|c| c.name == "Granted")
        .ok_or_else(refused)?;
    let refused_case = cases
        .iter()
        .find(|c| c.name == "Refused")
        .ok_or_else(refused)?;
    if granted.fields.len() != 2
        || granted.fields[0].name != "seal"
        || granted.fields[0].ty != ResolvedType::Bytes
        || granted.fields[1].name != "budget"
        || granted.fields[1].ty != ResolvedType::I64
        || refused_case.fields.len() != 1
        || refused_case.fields[0].name != "code"
        || refused_case.fields[0].ty != ResolvedType::I64
        || ![
            decision,
            &granted.id,
            &refused_case.id,
            &granted.fields[0].id,
            &granted.fields[1].id,
            &refused_case.fields[0].id,
        ]
        .iter()
        .all(|id| {
            p.declarations
                .declaration(id)
                .is_some_and(|d| d.identity_origin == hir::IdentityOrigin::Explicit)
        })
    {
        return Err(refused());
    }
    let ResolvedExprKind::Block { statements, tail } = &f.body.kind else {
        return Err(refused());
    };
    let [ResolvedStatement::Let { binding, value, .. }] = statements.as_slice() else {
        return Err(refused());
    };
    if !matches!(&value.kind, ResolvedExprKind::ArrayU8(v) if v.len() <= 1024) {
        return Err(refused());
    }
    if f.loan_plan.loans.iter().any(|loan| {
        loan.origin.root != binding.id
            || !loan.origin.projections.is_empty()
            || !matches!(
                loan.cause,
                crate::loan_plan::LoanCause::SliceView
                    | crate::loan_plan::LoanCause::BorrowedCall { argument: 0 }
            )
    }) {
        return Err(refused());
    }
    let ResolvedExprKind::If {
        condition,
        then_branch,
        else_branch,
    } = &tail.kind
    else {
        return Err(refused());
    };
    if !scalar(condition, f)
        || !branch(then_branch, f, decision, &granted.id, &binding.id, true)
        || !branch(
            else_branch,
            f,
            decision,
            &refused_case.id,
            &binding.id,
            false,
        )
        || !f.requires.iter().chain(&f.ensures).all(|c| scalar(c, f))
    {
        return Err(refused());
    }
    let mut commits = f
        .cleanup_plan
        .exits
        .iter()
        .filter(|e| matches!(e.continuation, ExitContinuation::CommitResult { .. }));
    if commits
        .next()
        .is_none_or(|e| !e.finalize_in_order.is_empty())
        || commits.next().is_some()
    {
        return Err(refused());
    }
    let disposal = crate::cleanup_plan::owned_authorize_result_disposal(&p.declarations, f)?;
    let mut constructor = then_branch.as_ref();
    while let ResolvedExprKind::Block { tail, .. } = &constructor.kind {
        constructor = tail;
    }
    let partial_disposal =
        crate::cleanup_plan::owned_authorize_partial_disposal(&p.declarations, f, constructor)?;
    Ok(CheckedOwnedAuthorizeV2 {
        helper: helper.clone(),
        function: function.clone(),
        decision: decision.clone(),
        granted: granted.id.clone(),
        refused: refused_case.id.clone(),
        seal: granted.fields[0].id.clone(),
        disposal,
        partial_disposal,
    })
}
fn scalar(e: &ResolvedExpr, f: &ResolvedFunction) -> bool {
    if !hir::is_scalar_resolved_type(&e.ty) {
        return false;
    }
    match &e.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_) => true,
        ResolvedExprKind::Place(p) => {
            (p.root == f.params[0].id && p.projections.len() == 1)
                || (p.projections.is_empty() && f.params[1..].iter().any(|v| v.id == p.root))
        }
        ResolvedExprKind::Unary { value, .. } => scalar(value, f),
        ResolvedExprKind::Binary { left, right, .. } => scalar(left, f) && scalar(right, f),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => scalar(condition, f) && copy_block(then_branch, f) && copy_block(else_branch, f),
        _ => false,
    }
}
fn copy_block(e: &ResolvedExpr, f: &ResolvedFunction) -> bool {
    match &e.kind {
        ResolvedExprKind::Block { statements, tail } => statements.is_empty() && scalar(tail, f),
        _ => scalar(e, f),
    }
}
fn branch(
    e: &ResolvedExpr,
    f: &ResolvedFunction,
    decision: &DeclarationId,
    case: &DeclarationId,
    array: &hir::ValueId,
    granted: bool,
) -> bool {
    if let ResolvedExprKind::Block { statements, tail } = &e.kind {
        return statements.is_empty() && branch(tail, f, decision, case, array, granted);
    }
    let ResolvedExprKind::ConstructVariant {
        variant,
        case: actual,
        fields,
    } = &e.kind
    else {
        return false;
    };
    if variant != decision || actual != case {
        return false;
    }
    if !granted {
        return fields.len() == 1 && scalar(&fields[0].value, f);
    }
    fields.len() == 2
        && scalar(&fields[1].value, f)
        && matches!(&fields[0].value.kind, ResolvedExprKind::Call { callee, args, .. }
            if callee.as_str() == crate::byte_ops::COPY_ID && args.len() == 1
            && matches!(&args[0].kind, ResolvedExprKind::BorrowPlace { operation, place }
                if operation.as_str() == crate::byte_ops::ARRAY_AS_SLICE_ID && place.root == *array && place.projections.is_empty()))
}
