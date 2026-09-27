//! Resumable Effects v1 (issue #204): resolving and admitting a function's
//! `yields Request -> Response` clause and its body's `yield` expression.
//!
//! `parser::yields` already guarantees, purely syntactically, that an
//! admitted function contains one or more `yield` expressions, only as direct
//! top-level `let`/assignment values or a tail expression of its own body
//! block, never nested. This module adds the type-level half:
//!
//! - The declared request and response types must be admitted Copy scalars
//!   (`hir::nodes::is_scalar_resolved_type`) or a bounded, flat,
//!   non-recursive record/variant of Copy scalars (issue #296 R20; see
//!   [`bounded_aggregate_refusal`]), deferring a generic aggregate, a
//!   nested aggregate, and an owned `Bytes` leaf inside one to future work
//!   so a suspend/resume never has to reason about ownership crossing the
//!   suspension.
//! - The whole function -- every parameter and every intermediate value --
//!   must stay within that same scalar profile, so cleanup-plan
//!   construction never needs a genuinely new exit path for `yield`
//!   (`cleanup_plan::build` treats it as an ordinary single-child node).
//! - The function may declare no `uses` effects: this slice's interpreter
//!   resumes by re-executing the function's prefix with the resume value
//!   substituted at the yield site, which would redispatch a host effect
//!   that already ran during the original, suspended call.
//! - The function may declare no generics (deferred: generics over the
//!   effect type). `source_verify::declared_type` already refuses any
//!   generic function whose body reaches a `Yield` node before `hir::resolve`
//!   ever runs its own checks, so this module adds no second, unreachable
//!   check for that case.
//!
//! See `docs/RESUMABLE-EFFECTS-V1.md`.

use crate::ast;
use crate::diagnostic::Diagnostic;
use std::collections::BTreeMap;

use super::expr_nodes::{
    ResolvedExpr, ResolvedExprKind, ResolvedFieldInitializer, ResolvedMatchArm,
};
use super::nodes::{is_scalar_resolved_type, ResolvedParam, ResolvedType, ResolvedYieldsClause};
use super::yield_aggregate::{self, MAX_YIELD_AGGREGATE_FIELDS};
use super::Resolver;

/// A `yields`-declaring function's own request or response type is not an
/// admitted Copy scalar.
const NON_SCALAR_SIGNATURE: &str = "SPX-T301";
/// `yield`'s operand does not have the declared request type.
const ILL_TYPED_YIELD: &str = "SPX-T299";
/// A `yields`-declaring function also declares `uses` effects.
const EFFECTFUL_YIELDS: &str = "SPX-T302";
/// A `yields`-declaring function's body -- a parameter or an intermediate
/// value -- leaves the admitted Copy-scalar profile.
const NON_SCALAR_BODY: &str = "SPX-T303";
/// A borrow (`borrow T`, `str`, `Slice<u8>`) in a `yields`-declaring function:
/// it could be live across a suspension, and replay cannot re-establish it.
const BORROW_ACROSS_YIELD: &str = "SPX-T305";
/// A resource or handle in a `yields`-declaring function: a suspension would
/// either leak it or run its drop before the resumed suffix.
const RESOURCE_ACROSS_YIELD: &str = "SPX-T306";
/// A record or variant used as a `yields` request/response type, or as an
/// intermediate value in a `yields`-declaring function's body, that is not
/// an admitted bounded aggregate (issue #296 R20; see
/// `yield_aggregate::bounded_aggregate_refusal`): generic, too many
/// fields/cases, or a field that is not itself a Copy scalar.
const AGGREGATE_BOUND_EXCEEDED: &str = "SPX-T307";

/// The stable refusal for a type outside the Copy-scalar profile.
///
/// A syntactic `borrow`/`share` ownership, or a type that is itself an
/// unowned view (`str`, `Slice<u8>`), gets `SPX-T305`: the referent is only
/// as live as its original owner's frame, which a suspension does not keep
/// around. This is checked before the scalar-type short-circuit in
/// [`check_scalar`], so a borrowed or shared *scalar* (a `borrow i64`
/// parameter, say) is refused too, not just a borrowed aggregate.
///
/// A type whose recursive [`super::TypeFacts::contains_resource`] is `true`
/// gets `SPX-T306`: a resource or handle, live directly or nested inside a
/// plain (non-generic) record's fields, would either leak across the
/// suspension or run its drop before the resumed suffix. `type_facts` gives
/// up (`None`) on a generic record or variant instantiated with a
/// non-scalar type argument -- for example `Option<Token>` -- since the
/// declaration index does not compute recursive facts for that shape; such
/// a value keeps the original `SPX-T303` rather than a confident `SPX-T306`
/// until that gap closes. Every other owned or aggregate value also keeps
/// `SPX-T303`.
fn profile_refusal(
    resolver: &Resolver<'_>,
    ty: &ResolvedType,
    ownership: super::OwnershipMode,
) -> (&'static str, &'static str) {
    use super::OwnershipMode;
    if matches!(ty, ResolvedType::Str | ResolvedType::SliceU8)
        || matches!(ownership, OwnershipMode::Borrow | OwnershipMode::Shared)
    {
        return (
            BORROW_ACROSS_YIELD,
            "a borrowed or unowned view could outlive its owner's frame across a suspension",
        );
    }
    if resolver
        .declarations
        .type_facts(ty)
        .is_some_and(|facts| facts.contains_resource)
    {
        return (
            RESOURCE_ACROSS_YIELD,
            "a resource or handle, directly or nested inside a record, could be live across a \
             suspension",
        );
    }
    (NON_SCALAR_BODY, "a value is not an admitted Copy scalar")
}

impl Resolver<'_> {
    /// The type a `yield` node carries while its function body resolves: the
    /// enclosing function's declared response type, so a yielded binding in
    /// a nested block (issue #296) is typed correctly for later statements.
    /// Falls back to the request type when no clause resolves;
    /// [`Self::finish_yields_admission`] still checks and retags every site.
    pub(super) fn yield_answer_type(
        &self,
        function: &super::FunctionExecutionId,
        request: &ResolvedType,
    ) -> ResolvedType {
        let super::FunctionExecutionId::Monomorphic(id) = function else {
            return request.clone();
        };
        self.program
            .functions
            .iter()
            .find(|candidate| candidate.stable_id == id.as_str())
            .and_then(|candidate| candidate.yields.as_ref())
            .and_then(|clause| self.resolve_type(&clause.response_type, clause.span).ok())
            .unwrap_or_else(|| request.clone())
    }

    /// Resolves `function.yields`, if present, and checks the admission
    /// rules that depend only on the signature: no `uses` effects,
    /// request/response types and every parameter type scalar. A generic
    /// function declaring `yields` is refused before this ever runs:
    /// `parser::yields` guarantees a `yields`-declaring function's body
    /// contains one or more direct sequential yields, and `source_verify::declared_type`
    /// already refuses any generic function whose body reaches a `Yield`
    /// node (`SPX-T226`, "outside the direct-scalar slice") as part of
    /// `hir::resolve`'s existing source-verification gate -- generic
    /// functions never reach `resolve_function_in_scope`, so a second,
    /// unreachable check here would be dead code.
    pub(super) fn resolve_yields_clause(
        &self,
        function: &ast::Function,
        params: &[ResolvedParam],
    ) -> Result<Option<ResolvedYieldsClause>, Diagnostic> {
        let Some(yields) = &function.yields else {
            return Ok(None);
        };
        if !function.effects.is_empty() {
            return Err(self.error(
                EFFECTFUL_YIELDS,
                format!(
                    "function `{}` declares both `uses` and `yields`; resuming would \
                     redispatch its host effects a second time, which is not yet admitted",
                    function.name
                ),
                yields.span,
            ));
        }
        let request_type = self.resolve_type(&yields.request_type, yields.span)?;
        let response_type = self.resolve_type(&yields.response_type, yields.span)?;
        for (role, ty) in [("request", &request_type), ("response", &response_type)] {
            if is_scalar_resolved_type(ty) {
                continue;
            }
            // Issue #296 R20: a record or variant is no longer refused
            // outright here. It is admitted exactly when it is a bounded,
            // flat, non-recursive aggregate of Copy scalars (see
            // `bounded_aggregate_refusal`); every other record/variant, and
            // every other non-scalar type, keeps its refusal.
            if matches!(ty, ResolvedType::Nominal { .. }) {
                if let Err(reason) =
                    yield_aggregate::bounded_aggregate_refusal(&self.declarations, ty)
                {
                    return Err(self.error(
                        AGGREGATE_BOUND_EXCEEDED,
                        format!(
                            "function `{}` declares a `yields` {role} type that is a record or \
                             variant but is not an admitted bounded aggregate: {reason}",
                            function.name
                        ),
                        yields.span,
                    ));
                }
                continue;
            }
            return Err(self.error(
                NON_SCALAR_SIGNATURE,
                format!(
                    "function `{}` declares a `yields` {role} type that is not an admitted \
                     Copy scalar or bounded aggregate; owned types other than a bounded \
                     record/variant of Copy scalars are not yet admitted here",
                    function.name
                ),
                yields.span,
            ));
        }
        if let Some(offender) = params.iter().find(|param| {
            !is_scalar_resolved_type(&param.ty) || param.ownership != super::OwnershipMode::Value
        }) {
            let (code, reason) = profile_refusal(self, &offender.ty, offender.ownership);
            return Err(self.error(
                code,
                format!(
                    "function `{}` declares `yields` but parameter `{}` is outside the \
                     resumable profile: {reason}",
                    function.name, offender.name
                ),
                offender.span,
            ));
        }
        Ok(Some(ResolvedYieldsClause {
            request_type,
            response_type,
            span: yields.span,
        }))
    }

    /// Checks every direct top-level `yield` (the parser guarantees at least
    /// one), verifies each operand against the declared request type, and
    /// rewrites each node's
    /// placeholder `ty` (the operand's own type, set when `resolve_expr`
    /// first built the node with no signature context available) to the
    /// declared response type. Also verifies every other resolved value in
    /// the body stays within the admitted Copy-scalar profile.
    pub(super) fn finish_yields_admission(
        &self,
        function_name: &str,
        yields: &ResolvedYieldsClause,
        body: &mut ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let mut found = 0usize;
        let mut yielded_bindings = BTreeMap::new();
        scan_expr(
            self,
            function_name,
            yields,
            body,
            true,
            true,
            &mut found,
            &mut yielded_bindings,
        )?;
        if found == 0 {
            // Unreachable given the parser-level guarantee; kept as a
            // defensive check rather than trusted silently.
            return Err(self.error(
                "SPX-T298",
                format!("function `{function_name}` declares `yields` but its body never yields"),
                yields.span,
            ));
        }
        Ok(())
    }
}

fn check_scalar(
    resolver: &Resolver<'_>,
    function_name: &str,
    expr: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    // A scalar *type* is Copy, but a `borrow`/`share` *ownership* of one --
    // `borrow i64`, say -- is still a reference to a frame a suspension does
    // not keep live; check ownership before the scalar-type short-circuit
    // so it is not skipped for an otherwise-admitted type.
    let borrowed = matches!(
        expr.ownership,
        super::OwnershipMode::Borrow | super::OwnershipMode::Shared
    );
    // Issue #296, spec section 11.6, second increment: an unborrowed value of
    // type `Bytes` is deferred rather than refused here. Whether it is truly
    // admitted -- a whole `let`-bound local live across a real suspension
    // site -- is decided once `function.cleanup_plan` exists, by
    // `cleanup_plan::admit_owned_bytes_profile`; this function runs before
    // that plan is built and would otherwise refuse every owned value
    // unconditionally. A borrowed `Bytes` (an unusual but syntactically
    // possible `borrow Bytes`/`share Bytes`) still falls through to
    // `profile_refusal` below and keeps `SPX-T305`.
    if !borrowed && (is_scalar_resolved_type(&expr.ty) || expr.ty == ResolvedType::Unit) {
        return Ok(());
    }
    if !borrowed && expr.ty == ResolvedType::Bytes {
        return Ok(());
    }
    let (code, reason) = profile_refusal(resolver, &expr.ty, expr.ownership);
    // Issue #296 R20: `profile_refusal` only ever returns the generic
    // catch-all (`NON_SCALAR_BODY`) for a record/variant that is not a
    // borrow and does not itself carry a resource; a borrowed or
    // resource-carrying value keeps its existing, more precise code
    // (`BORROW_ACROSS_YIELD`/`RESOURCE_ACROSS_YIELD`) untouched below. Only
    // that generic catch-all is narrowed to admit a bounded aggregate.
    if !borrowed && code == NON_SCALAR_BODY {
        if let ResolvedType::Nominal { .. } = &expr.ty {
            return match yield_aggregate::bounded_aggregate_refusal(
                &resolver.declarations,
                &expr.ty,
            ) {
                Ok(()) => Ok(()),
                Err(aggregate_reason) => Err(resolver.error(
                    AGGREGATE_BOUND_EXCEEDED,
                    format!(
                        "function `{function_name}` declares `yields` but an intermediate \
                         record or variant value is not an admitted bounded aggregate: \
                         {aggregate_reason}"
                    ),
                    expr.span,
                )),
            };
        }
    }
    Err(resolver.error(
        code,
        format!(
            "function `{function_name}` declares `yields` but an intermediate value is \
             outside the resumable profile: {reason}"
        ),
        expr.span,
    ))
}

/// Exhaustive descent over every resolved expression shape, mirroring
/// `parser::yields`'s two-part placement rule.
///
/// `top_level` is `true` only while walking positions the parser already
/// admits a direct `yield` value in: the function's own top-level
/// statement/tail slots, and -- recursively -- the statement values of any
/// block reached from there through `if`/`else` branches, `while` bodies, or
/// a block-valued slot (issue #296). It is `false`, and stays `false` for
/// every descendant, once a child closes those positions off: call
/// arguments, operator operands, conditions, closures, `match` arms, and a
/// nested block's own tail.
///
/// `is_root` is `true` only for the function's own outermost body block --
/// the single position the parser's `Level::Top` names. It lets that one
/// block's tail admit a direct `yield` the same way its statement values do.
/// Every other block the parser scans at `Level::Nested` (an `if`/`else`
/// branch, a `while` body, or a block-valued statement value) still scans
/// its statements with `top_level` unchanged, but scans its own tail with
/// `is_root` forced `false`, so a nested block's tail never admits a
/// `yield`.
fn scan_expr(
    resolver: &Resolver<'_>,
    function_name: &str,
    yields: &ResolvedYieldsClause,
    expr: &mut ResolvedExpr,
    top_level: bool,
    is_root: bool,
    found: &mut usize,
    yielded_bindings: &mut BTreeMap<super::ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    // The general expression resolver initially gives `yield <request>` its
    // request type because that API does not receive the already-resolved
    // enclosing `yields` clause. A direct yielded
    // `let` is subsequently retagged to the declared response type below;
    // rewrite every later use of that exact value identity before validation
    // sees the binding and its places disagree.
    if let ResolvedExprKind::Place(place) = &expr.kind {
        if place.projections.is_empty() {
            if let Some(ty) = yielded_bindings.get(&place.root) {
                expr.ty = ty.clone();
            }
        }
    }
    match &mut expr.kind {
        ResolvedExprKind::Yield { request } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                request,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            if !top_level {
                // Unreachable: `parser::yields` already refuses a nested
                // `yield` before resolution ever runs. Defensive.
                return Err(resolver.error(
                    "SPX-T297",
                    format!(
                        "function `{function_name}` yields in a position the parser should \
                         already have refused"
                    ),
                    expr.span,
                ));
            }
            if request.ty != yields.request_type {
                return Err(resolver.error(
                    ILL_TYPED_YIELD,
                    format!(
                        "function `{function_name}`'s `yield` operand has type `{:?}` but the \
                         declared request type is `{:?}`",
                        request.ty, yields.request_type
                    ),
                    request.span,
                ));
            }
            expr.ty = yields.response_type.clone();
            *found += 1;
        }
        ResolvedExprKind::Closure { captures, body, .. } => {
            for capture in captures.iter_mut() {
                scan_expr(
                    resolver,
                    function_name,
                    yields,
                    &mut capture.value,
                    false,
                    false,
                    found,
                    yielded_bindings,
                )?;
            }
            scan_expr(
                resolver,
                function_name,
                yields,
                body,
                false,
                false,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                source,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            scan_expr(
                resolver,
                function_name,
                yields,
                start,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            scan_expr(
                resolver,
                function_name,
                yields,
                end,
                false,
                false,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::Invoke { callable, args } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                callable,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            scan_children(
                resolver,
                function_name,
                yields,
                args,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::Call { args, .. } => {
            scan_children(
                resolver,
                function_name,
                yields,
                args,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            scan_children(
                resolver,
                function_name,
                yields,
                &mut call.args,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::HostCommandCall(call) => {
            scan_children(
                resolver,
                function_name,
                yields,
                &mut call.args,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::Unary { value, .. } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                value,
                false,
                false,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                left,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            scan_expr(
                resolver,
                function_name,
                yields,
                right,
                false,
                false,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements.iter_mut() {
                scan_statement(
                    resolver,
                    function_name,
                    yields,
                    statement,
                    top_level,
                    found,
                    yielded_bindings,
                )?;
            }
            // Only the function's own outermost body block (`is_root`) has a
            // tail the parser admits a direct `yield` in; every other block
            // -- an `if`/`else` branch, a `while` body, or a block-valued
            // statement value -- is `Level::Nested` and its tail stays
            // closed, however its enclosing chain scanned (`top_level`).
            let tail_admits_yield = top_level && is_root;
            scan_expr(
                resolver,
                function_name,
                yields,
                tail,
                tail_admits_yield,
                false,
                found,
                yielded_bindings,
            )?;
            if tail_admits_yield && matches!(tail.kind, ResolvedExprKind::Yield { .. }) {
                expr.ty = tail.ty.clone();
            }
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                condition,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            // Issue #296: branches of an `if` in an admitted slot open nested
            // blocks whose direct statement values may suspend, but a branch
            // is never the outermost body block itself, so its own tail
            // stays closed to `yield` (`is_root` forced `false`).
            scan_expr(
                resolver,
                function_name,
                yields,
                then_branch,
                top_level,
                false,
                found,
                yielded_bindings,
            )?;
            scan_expr(
                resolver,
                function_name,
                yields,
                else_branch,
                top_level,
                false,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            scan_fields(
                resolver,
                function_name,
                yields,
                fields,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                scrutinee,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            for arm in arms.iter_mut() {
                scan_arm(
                    resolver,
                    function_name,
                    yields,
                    arm,
                    found,
                    yielded_bindings,
                )?;
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                operand,
                false,
                false,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                base,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            scan_fields(
                resolver,
                function_name,
                yields,
                fields,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::Project { base, .. } | ResolvedExprKind::Upcast { source: base } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                base,
                false,
                false,
                found,
                yielded_bindings,
            )?;
        }
    }
    check_scalar(resolver, function_name, expr)
}

fn scan_children(
    resolver: &Resolver<'_>,
    function_name: &str,
    yields: &ResolvedYieldsClause,
    children: &mut [ResolvedExpr],
    found: &mut usize,
    yielded_bindings: &mut BTreeMap<super::ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    for child in children.iter_mut() {
        scan_expr(
            resolver,
            function_name,
            yields,
            child,
            false,
            false,
            found,
            yielded_bindings,
        )?;
    }
    Ok(())
}

fn scan_fields(
    resolver: &Resolver<'_>,
    function_name: &str,
    yields: &ResolvedYieldsClause,
    fields: &mut [ResolvedFieldInitializer],
    found: &mut usize,
    yielded_bindings: &mut BTreeMap<super::ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    for field in fields.iter_mut() {
        scan_expr(
            resolver,
            function_name,
            yields,
            &mut field.value,
            false,
            false,
            found,
            yielded_bindings,
        )?;
    }
    Ok(())
}

fn scan_arm(
    resolver: &Resolver<'_>,
    function_name: &str,
    yields: &ResolvedYieldsClause,
    arm: &mut ResolvedMatchArm,
    found: &mut usize,
    yielded_bindings: &mut BTreeMap<super::ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    if let Some(guard) = &mut arm.guard {
        scan_expr(
            resolver,
            function_name,
            yields,
            guard,
            false,
            false,
            found,
            yielded_bindings,
        )?;
    }
    scan_expr(
        resolver,
        function_name,
        yields,
        &mut arm.value,
        false,
        false,
        found,
        yielded_bindings,
    )
}

fn scan_statement(
    resolver: &Resolver<'_>,
    function_name: &str,
    yields: &ResolvedYieldsClause,
    statement: &mut super::expr_nodes::ResolvedStatement,
    top_level: bool,
    found: &mut usize,
    yielded_bindings: &mut BTreeMap<super::ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    use super::expr_nodes::ResolvedStatement;
    match statement {
        ResolvedStatement::Let { binding, value, .. } => {
            // A `let` value is a slot at any depth in the admitted chain
            // (`top_level` carries that), but the value can never itself be
            // the outermost body block, so its own tail stays closed.
            scan_expr(
                resolver,
                function_name,
                yields,
                value,
                top_level,
                false,
                found,
                yielded_bindings,
            )?;
            if top_level && matches!(&value.kind, ResolvedExprKind::Yield { .. }) {
                binding.ty = value.ty.clone();
                yielded_bindings.insert(binding.id.clone(), binding.ty.clone());
            }
            Ok(())
        }
        ResolvedStatement::Assign {
            binding,
            field,
            value,
            ..
        } => {
            if let Some(ty) = yielded_bindings.get(&binding.id) {
                binding.ty = ty.clone();
            }
            scan_expr(
                resolver,
                function_name,
                yields,
                value,
                top_level,
                false,
                found,
                yielded_bindings,
            )?;
            // `resolve_expr` initially gives a `yield` its request type, as
            // it does not carry the enclosing function's resolved signature.
            // A direct whole-binding assignment is therefore deferred until
            // the expression is retagged above.  Check the actual target
            // against the declared response here, rather than requiring the
            // request and response types to happen to agree.
            if top_level
                && field.is_none()
                && matches!(&value.kind, ResolvedExprKind::Yield { .. })
                && binding.ty != yields.response_type
            {
                return Err(resolver.error(
                    ILL_TYPED_YIELD,
                    format!(
                        "function `{function_name}` assigns a yielded response of type `{:?}` to \
                         binding `{}` of type `{:?}`",
                        yields.response_type, binding.name, binding.ty
                    ),
                    value.span,
                ));
            }
            Ok(())
        }
        ResolvedStatement::Unsafe { body, .. } => scan_expr(
            resolver,
            function_name,
            yields,
            body,
            false,
            false,
            found,
            yielded_bindings,
        ),
        ResolvedStatement::While {
            condition, body, ..
        } => {
            scan_expr(
                resolver,
                function_name,
                yields,
                condition,
                false,
                false,
                found,
                yielded_bindings,
            )?;
            // Issue #296: a `while` body opens a nested block whose direct
            // statement values may suspend (the parser owns placement), but
            // the body is never the outermost body block itself, so its own
            // tail stays closed to `yield` (`is_root` forced `false`).
            scan_expr(
                resolver,
                function_name,
                yields,
                body,
                top_level,
                false,
                found,
                yielded_bindings,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        AGGREGATE_BOUND_EXCEEDED, BORROW_ACROSS_YIELD, EFFECTFUL_YIELDS, ILL_TYPED_YIELD,
        MAX_YIELD_AGGREGATE_FIELDS, NON_SCALAR_BODY, NON_SCALAR_SIGNATURE, RESOURCE_ACROSS_YIELD,
    };
    use crate::hir;

    fn resolve(source: &str) -> Result<hir::ResolvedProgram, crate::diagnostic::Diagnostic> {
        let program = crate::parse(source, Path::new("resolve-yield-fixture.spx")).unwrap();
        hir::resolve(&program).map_err(|mut errors| errors.remove(0))
    }

    #[test]
    fn a_well_typed_scalar_yield_resolves() {
        let source = r#"
module test.resolve_yield_ok;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield seed + 1;
    answer * 2
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let resolved = resolve(source).expect("well-typed scalar yield resolves");
        let ask = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        let yields = ask.yields.as_ref().expect("declares yields");
        assert_eq!(yields.request_type, hir::ResolvedType::I64);
        assert_eq!(yields.response_type, hir::ResolvedType::I64);
    }

    #[test]
    fn sequential_yields_share_the_declared_response_type() {
        let source = r#"
module test.resolve_sequential_yields;
@id("app.ask")
fn ask() -> bool
    yields i64 -> bool
{
    let first = yield 1;
    let second = yield 2;
    first && second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let resolved = resolve(source).expect("sequential scalar yields resolve");
        let ask = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &ask.body.kind else {
            panic!("resumable function body is a block")
        };
        assert_eq!(statements.len(), 2);
        for statement in statements {
            let hir::ResolvedStatement::Let { value, .. } = statement else {
                panic!("fixture contains only let statements")
            };
            assert!(matches!(&value.kind, hir::ResolvedExprKind::Yield { .. }));
            assert_eq!(value.ty, hir::ResolvedType::Bool);
        }
    }

    #[test]
    fn a_distinct_response_type_assignment_is_retagged_before_assignment_validation() {
        let source = r#"
module test.resolve_yield_assignment_response;
@id("app.ask")
fn ask(seed: i64) -> bool yields i64 -> bool {
    let mut answer = false;
    answer = yield seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let parsed = crate::parse(source, Path::new("resolve-yield-assignment.spx")).unwrap();
        let source_diagnostics = crate::source_verify::verify(&parsed);
        assert!(
            source_diagnostics.is_empty(),
            "the valid source must pass verification before HIR retagging: {source_diagnostics:?}"
        );
        let resolved = hir::resolve(&parsed)
            .map_err(|mut diagnostics| diagnostics.remove(0))
            .expect("a direct assignment accepts the response type");
        hir::validate(&resolved).expect("the retagged assignment remains valid HIR");
        let ask = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &ask.body.kind else {
            panic!("resumable function body is a block")
        };
        let hir::ResolvedStatement::Assign { binding, value, .. } = &statements[1] else {
            panic!("fixture's yielded response remains a direct assignment")
        };
        assert_eq!(binding.ty, hir::ResolvedType::Bool);
        assert_eq!(value.ty, hir::ResolvedType::Bool);
        assert!(matches!(&value.kind, hir::ResolvedExprKind::Yield { .. }));
    }

    #[test]
    fn a_direct_yield_assignment_still_requires_the_declared_response_type() {
        let source = r#"
module test.resolve_yield_assignment_response_mismatch;
@id("app.ask")
fn ask(seed: i64) -> i64 yields i64 -> bool {
    let mut answer = 0;
    answer = yield seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let parsed =
            crate::parse(source, Path::new("resolve-yield-assignment-mismatch.spx")).unwrap();
        let source_diagnostics = crate::source_verify::verify(&parsed);
        assert!(
            source_diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "SPX-U102"),
            "the placeholder mismatch must be deferred to the response-aware HIR check: {source_diagnostics:?}"
        );
        let error = hir::resolve(&parsed)
            .unwrap_err()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(error.code, ILL_TYPED_YIELD);
        assert!(error.message.contains("yielded response of type"));
    }

    #[test]
    fn a_distinct_response_type_can_be_the_function_tail() {
        let source = r#"
module test.resolve_yield_tail_response;
@id("app.ask")
fn ask(seed: i64) -> bool yields i64 -> bool { yield seed }
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let resolved = resolve(source).expect("yield response types the function tail");
        hir::validate(&resolved).unwrap();
    }

    #[test]
    fn a_yield_operand_of_the_wrong_type_is_refused() {
        let source = r#"
module test.resolve_yield_ill_typed;
@id("app.ask")
fn ask() -> i64
    yields i64 -> i64
{
    let answer = yield true;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = resolve(source).unwrap_err();
        assert_eq!(error.code, ILL_TYPED_YIELD);
    }

    #[test]
    fn a_later_yield_operand_of_the_wrong_type_is_refused() {
        let source = r#"
module test.resolve_sequential_yield_ill_typed;
@id("app.ask")
fn ask() -> i64
    yields i64 -> i64
{
    let first = yield 1;
    let second = yield false;
    first + second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = resolve(source).unwrap_err();
        assert_eq!(error.code, ILL_TYPED_YIELD);
    }

    #[test]
    fn a_generic_function_cannot_declare_yields() {
        let source = r#"
module test.resolve_yield_generic;
@id("app.ask")
fn ask<T>() -> i64
    yields i64 -> i64
{
    let answer = yield 1;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = resolve(source).unwrap_err();
        // `source_verify::declared_type` refuses any generic function whose
        // body reaches a `Yield` node before `hir::resolve` runs its own
        // checks; see the module doc for why this module adds no second,
        // unreachable check for the same case.
        assert_eq!(error.code, "SPX-T226");
    }

    #[test]
    fn a_function_with_uses_effects_cannot_also_declare_yields() {
        let source = r#"
module test.resolve_yield_effectful;
permit { clock.read }
@id("app.ask")
fn ask() -> i64
    uses { clock.read }
    yields i64 -> i64
{
    let answer = yield 1;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = resolve(source).unwrap_err();
        assert_eq!(error.code, EFFECTFUL_YIELDS);
    }

    #[test]
    fn an_owned_yields_signature_that_is_not_a_record_or_variant_is_refused() {
        // `Bytes` itself is not an admitted bounded aggregate (it is not a
        // record or variant at all), so it keeps the original catch-all
        // refusal rather than the new, more specific `SPX-T307`.
        let source = r#"
module test.resolve_yield_non_scalar;
@id("app.ask")
fn ask() -> i64
    yields Bytes -> i64
{
    let answer = yield 1;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = resolve(source).unwrap_err();
        assert_eq!(error.code, NON_SCALAR_SIGNATURE);
    }

    #[test]
    fn a_record_yields_signature_with_a_non_scalar_field_is_refused_as_an_aggregate_bound() {
        // Issue #296 R20: a record used as the `yields` request type is no
        // longer refused outright, but a field that is itself not a Copy
        // scalar (here, an owned `Bytes` leaf, deferred to a future
        // increment) still fails admission -- now with the more precise
        // `SPX-T307` rather than the old blanket `SPX-T301`.
        let source = r#"
module test.resolve_yield_non_scalar;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.note") note: Bytes,
}
@id("app.ask")
fn ask() -> i64
    yields Prompt -> i64
{
    let answer = yield Prompt { seed: 1, note: bytes_zeroed(1usize) };
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = resolve(source).unwrap_err();
        assert_eq!(error.code, AGGREGATE_BOUND_EXCEEDED);
    }

    #[test]
    fn a_bounded_copy_scalar_record_yields_signature_is_admitted() {
        // Issue #296 R20, blocker (1) of the Agent lifecycle migration
        // assessment (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md §12): a
        // flat record of Copy scalars is now an admitted `yields` request
        // and response type.
        let source = r#"
module test.resolve_yield_record_aggregate;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.urgent") urgent: bool,
}
@id("app.ask")
fn ask() -> i64
    yields Prompt -> Prompt
{
    let answer = yield Prompt { seed: 1, urgent: true };
    answer.seed
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let program = resolve(source).expect("a flat Copy-scalar record aggregate is admitted");
        hir::validate(&program).unwrap();
        let ask = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        let yields = ask.yields.as_ref().expect("declares yields");
        assert!(matches!(
            yields.request_type,
            hir::ResolvedType::Nominal { .. }
        ));
        assert_eq!(yields.request_type, yields.response_type);
    }

    #[test]
    fn a_bounded_copy_scalar_variant_yields_signature_is_admitted() {
        let source = r#"
module test.resolve_yield_variant_aggregate;
@id("app.step")
variant Step {
    @id("app.step.continue")
    Continue { @id("app.step.continue.round") round: i64, },
    @id("app.step.done")
    Done { @id("app.step.done.ok") ok: bool, },
}
@id("app.ask")
fn ask() -> i64
    yields i64 -> Step
{
    let answer = yield 1;
    match answer {
        Step::Continue { round: round } => round,
        Step::Done { ok: ok } => if ok { 1 } else { 0 },
    }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let program = resolve(source).expect("a flat Copy-scalar variant aggregate is admitted");
        hir::validate(&program).unwrap();
    }

    #[test]
    fn a_record_yields_signature_past_the_field_bound_is_refused() {
        let declared_fields: String = (0..=MAX_YIELD_AGGREGATE_FIELDS)
            .map(|index| format!("    @id(\"app.wide.f{index}\") f{index}: i64,\n"))
            .collect();
        let constructed_fields: String = (0..=MAX_YIELD_AGGREGATE_FIELDS)
            .map(|index| format!("f{index}: {index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "module test.resolve_yield_wide_record;\n@id(\"app.wide\")\nrecord Wide {{\n{declared_fields}}}\n@id(\"app.ask\")\nfn ask() -> i64 yields Wide -> i64 {{ let answer = yield Wide {{ {constructed_fields} }}; answer }}\n@id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
        );
        let error = resolve(&source).unwrap_err();
        assert_eq!(error.code, AGGREGATE_BOUND_EXCEEDED);
    }

    #[test]
    fn a_generic_record_yields_signature_is_refused() {
        let source = r#"
module test.resolve_yield_generic_record;
@id("app.boxed")
record Boxed<T> { @id("app.boxed.value") value: T, }
@id("app.ask")
fn ask() -> i64
    yields Boxed<i64> -> i64
{
    let answer = yield Boxed<i64> { value: 1 };
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = resolve(source).unwrap_err();
        assert_eq!(error.code, AGGREGATE_BOUND_EXCEEDED);
    }

    #[test]
    fn a_record_nested_inside_a_record_yields_signature_is_refused() {
        let source = r#"
module test.resolve_yield_nested_record;
@id("app.inner")
record Inner { @id("app.inner.seed") seed: i64, }
@id("app.outer")
record Outer { @id("app.outer.inner") inner: Inner, }
@id("app.ask")
fn ask() -> i64
    yields Outer -> i64
{
    let answer = yield Outer { inner: Inner { seed: 1 } };
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = resolve(source).unwrap_err();
        assert_eq!(error.code, AGGREGATE_BOUND_EXCEEDED);
    }

    #[test]
    fn a_bounded_aggregate_intermediate_value_is_admitted_in_a_yields_function_body() {
        // Issue #296 R20: the body-level `check_scalar` walk admits the
        // same bounded flat Copy-scalar aggregate shape for an ordinary
        // intermediate value, not only for the declared `yields` channel
        // type itself.
        let source = r#"
module test.resolve_yield_body_aggregate;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.urgent") urgent: bool,
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let prompt = Prompt { seed: seed, urgent: false };
    let answer = yield prompt.seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let program = resolve(source).expect("a bounded aggregate intermediate value is admitted");
        hir::validate(&program).unwrap();
    }

    #[test]
    fn yields_in_branches_and_loops_resolve_with_the_response_type() {
        let source = r#"
module test.resolve_yield_control;
@id("app.ask")
fn ask(limit: i64) -> bool
    yields i64 -> bool
{
    let mut round = 0;
    let mut accepted = false;
    while round < limit {
        let ok = yield round;
        accepted = ok;
        round = round + 1;
        round > 0
    }
    let last = if accepted {
        let again = yield round;
        again
    } else {
        false
    };
    last
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let program = resolve(source).unwrap();
        hir::validate(&program).unwrap();
    }

    fn refused(parameter: &str, declarations: &str) -> String {
        let source = format!(
            "module test.resolve_yield_refusal;\n{declarations}\n@id(\"app.ask\")\nfn ask({parameter}) -> i64\n    yields i64 -> i64\n{{\n    let answer = yield 1;\n    answer\n}}\n@id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
        );
        resolve(&source).unwrap_err().code.to_string()
    }

    #[test]
    fn borrows_resources_and_owned_values_have_stable_refusals() {
        assert_eq!(refused("text: borrow str", ""), BORROW_ACROSS_YIELD);
        let token = "@id(\"app.token\")\nresource Token {\n    @id(\"app.token.drop\")\n    drop trivial;\n}";
        assert_eq!(refused("token: borrow Token", token), BORROW_ACROSS_YIELD);
        assert_eq!(refused("token: own Token", token), RESOURCE_ACROSS_YIELD);
        let owned = r#"
module test.resolve_yield_owned;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let text = "owned";
    let answer = yield seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        assert_eq!(resolve(owned).unwrap_err().code, NON_SCALAR_BODY);
    }

    #[test]
    fn a_resource_nested_inside_a_plain_record_field_is_still_the_resource_refusal() {
        // Issue #296 review: `profile_refusal` used to classify a resource
        // only when the checked type was itself directly the `resource`
        // declaration; a record that merely *contains* one (no generic
        // arguments, so `TypeFacts` computes recursively) fell through to
        // the generic `SPX-T303`. It must get the more precise `SPX-T306`,
        // the same as a bare resource parameter.
        let declarations = "@id(\"app.token\")\nresource Token {\n    @id(\"app.token.drop\")\n    drop trivial;\n}\n@id(\"app.wrapper\")\nrecord Wrapper {\n    @id(\"app.wrapper.token\")\n    token: Token,\n}";
        assert_eq!(
            refused("wrapper: own Wrapper", declarations),
            RESOURCE_ACROSS_YIELD
        );
    }
}
