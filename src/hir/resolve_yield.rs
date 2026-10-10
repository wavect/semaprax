//! Resumable Effects v1 (issue #204): resolving and admitting a function's
//! `yields Request -> Response` clause and its body's `yield` expression.
//!
//! `parser::yields` already guarantees, purely syntactically, that an
//! admitted function contains one or more `yield` expressions, only as direct
//! top-level `let`/assignment values or a tail expression of its own body
//! block, never nested. This module adds the type-level half:
//!
//! - The declared request and response types must be admitted Copy scalars
//!   (`hir::nodes::is_scalar_resolved_type`), or (issue #296 R20) a bounded,
//!   flat, non-recursive record or variant of Copy scalars --
//!   `yield_aggregate::bounded_aggregate_refusal`'s exact shape
//!   (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md §12.1) -- admitted only for
//!   the direct top-level (sequential) `yield` placement this doc comment's
//!   own first paragraph describes, never a control-dependent one
//!   (`finish_yields_admission`'s own placement check keeps `SPX-T307` for
//!   that combination). Every runtime layer this shape needs now exists end
//!   to end: `resumable_effects::lowering`'s `ResumableScalar::Record`/
//!   `Variant`, the interpreter's `ResumableChannelValue` boundary
//!   (`crate::interpreter::resumable::channel`), the `v5` `source_checkpoint`
//!   envelope, and the durable journal's `_channel` entry points. A shape
//!   that does not fit the bound at all -- too many fields/cases, a nested
//!   aggregate, an owned `Bytes` leaf, or a generic instantiation -- keeps
//!   `SPX-T307` regardless of placement, since lowering still cannot execute
//!   it; a checked program must either run or be refused with a stable
//!   diagnostic before it ever reaches lowering, so this module never admits
//!   a shape lowering cannot yet execute.
//! - The whole function -- every parameter and every intermediate value --
//!   must stay within that same profile: a parameter stays a bare Copy
//!   scalar always (this widening never touches parameters), and an
//!   ordinary intermediate value stays a bare Copy scalar unless it is
//!   *exactly* the declared request or response type (`check_scalar`), so
//!   cleanup-plan construction never needs a genuinely new exit path for
//!   `yield` (`cleanup_plan::build` treats it as an ordinary single-child
//!   node) and an unrelated aggregate used only as body scratch keeps its
//!   ordinary `SPX-T303`.
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
/// A record or variant used as a `yields` request/response type: aggregate
/// yield channels are not yet admitted (issue #296 R20). This is a stable,
/// dedicated refusal distinct from the generic `SPX-T301` precisely because
/// `yield_aggregate::bounded_aggregate_refusal` already states the exact
/// bounded, flat, non-recursive Copy-scalar shape a future increment would
/// admit (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md §12.1): fitting that
/// shape today still is not enough, since no lowering, envelope, journal, or
/// driver support for it exists yet.
const AGGREGATE_NOT_YET_ADMITTED: &str = "SPX-T307";

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
        // v6 can authenticate a declared Bytes response without ambiguity, but
        // the direct sequential interpreter cannot yet expose that owned answer
        // to a suffix. Refuse here, before lowering, rather than leaking H006.
        if yield_aggregate::has_bytes_leaf(&self.declarations, &response_type) {
            return Err(self.error(
                AGGREGATE_NOT_YET_ADMITTED,
                format!(
                    "function `{}` declares a `yields` response with a bounded `Bytes` leaf; \
                     the current direct sequential profile admits `Bytes` leaves on requests only",
                    function.name
                ),
                yields.span,
            ));
        }
        for (role, ty) in [("request", &request_type), ("response", &response_type)] {
            if is_scalar_resolved_type(ty) {
                continue;
            }
            // Issue #296 R20: a record or variant that fits the bounded,
            // flat, non-recursive Copy-scalar aggregate shape
            // `yield_aggregate::bounded_aggregate_refusal` states
            // (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md §12.1) is admitted
            // here -- `resumable_effects::lowering`, the interpreter's
            // `ResumableChannelValue` boundary, the `v5` checkpoint
            // envelope, and the durable journal's `_channel` entry points
            // all now run it end to end for the direct top-level
            // (sequential) `yield` placement.
            // `finish_yields_admission` refuses a control-dependent
            // placement of this same aggregate shape afterward, once the
            // resolved body's placement is known; this signature-only check
            // cannot see placement yet. A shape that does not fit the bound
            // at all -- too many fields/cases, a nested aggregate, an owned
            // `Bytes` leaf, or a generic instantiation -- keeps the
            // dedicated `SPX-T307` refusal regardless of placement.
            if matches!(ty, ResolvedType::Nominal { .. }) {
                match yield_aggregate::bounded_aggregate_refusal(&self.declarations, ty) {
                    Ok(()) => continue,
                    Err(reason) => {
                        return Err(self.error(
                            AGGREGATE_NOT_YET_ADMITTED,
                            format!(
                                "function `{}` declares a `yields` {role} type that is a record \
                                 or variant but does not fit the admitted bounded Copy-scalar \
                                 aggregate shape (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md \
                                 §12.1): {reason}",
                                function.name
                            ),
                            yields.span,
                        ));
                    }
                }
            }
            return Err(self.error(
                NON_SCALAR_SIGNATURE,
                format!(
                    "function `{}` declares a `yields` {role} type that is not an admitted \
                     Copy scalar; only a Copy scalar, or a bounded flat record/variant of Copy \
                     scalars, is admitted here",
                    function.name
                ),
                yields.span,
            ));
        }
        if !crate::cleanup_plan::owned_frame_parameter(&self.declarations, params)
            && !crate::cleanup_plan::owned_frame_v2_parameter(&self.declarations, params)
        {
            if let Some(offender) = params.iter().find(|param| {
                param.ownership != super::OwnershipMode::Value
                    || (!is_scalar_resolved_type(&param.ty)
                        && (!matches!(param.ty, ResolvedType::Nominal { .. })
                            || yield_aggregate::has_bytes_leaf(&self.declarations, &param.ty)
                            || yield_aggregate::bounded_aggregate_refusal(
                                &self.declarations,
                                &param.ty,
                            )
                            .is_err()))
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
        params: &[ResolvedParam],
        return_type: &ResolvedType,
        body: &mut ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        // Issue #296 R20: an admitted bounded aggregate channel
        // (`resolve_yields_clause` already checked the shape; only the
        // signature's own type, not yet the body's placement, was known
        // there) is wired end to end only for the direct top-level
        // (sequential) `yield` placement. A control-dependent placement --
        // `yield` reachable only through `if`/`else` or `while` -- keeps the
        // dedicated `SPX-T307` refusal here, before this ever reaches
        // lowering, rather than admitting a shape
        // `resumable_effects::lowering::control` cannot run.
        if (matches!(yields.request_type, ResolvedType::Nominal { .. })
            || matches!(yields.response_type, ResolvedType::Nominal { .. }))
            && body_is_control_dependent(body)
        {
            return Err(self.error(
                AGGREGATE_NOT_YET_ADMITTED,
                format!(
                    "function `{function_name}` declares a bounded record/variant `yields` \
                     channel but its `yield` is reachable only through `if`/`else` or `while`; \
                     an aggregate channel is admitted only for the direct top-level (sequential) \
                     placement"
                ),
                yields.span,
            ));
        }
        if crate::cleanup_plan::owned_frame_parameter(&self.declarations, params)
            && (!is_scalar_resolved_type(&yields.request_type)
                || !is_scalar_resolved_type(&yields.response_type)
                || !crate::cleanup_plan::owned_frame_body(params, return_type, body))
        {
            return Err(self.error(
                NON_SCALAR_BODY,
                "owned frame requires one direct Copy yield and whole identity return",
                body.span,
            ));
        }
        if crate::cleanup_plan::owned_frame_v2_parameter(&self.declarations, params)
            && !crate::cleanup_plan::owned_frame_v2_body(
                &self.declarations,
                params,
                return_type,
                yields,
                body,
            )
        {
            return Err(self.error(
                NON_SCALAR_BODY,
                "owned frame v2 requires one identity Copy record yield and whole State return",
                body.span,
            ));
        }
        let mut found = 0usize;
        let mut yielded_bindings = BTreeMap::new();
        scan_expr(
            self,
            function_name,
            yields,
            params,
            return_type,
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
        if yield_aggregate::has_bytes_leaf(&self.declarations, &yields.request_type) && found > 1 {
            return Err(self.error(
                AGGREGATE_NOT_YET_ADMITTED,
                format!(
                    "function `{function_name}` has more than one bounded `Bytes` request site; \
                     the v6 checkpoint profile admits one direct suspension only"
                ),
                yields.span,
            ));
        }
        Ok(())
    }
}

fn check_scalar(
    resolver: &Resolver<'_>,
    function_name: &str,
    yields: &ResolvedYieldsClause,
    params: &[ResolvedParam],
    return_type: &ResolvedType,
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
    // Issue #296 R20: an admitted bounded record/variant `yields` channel
    // (`resolve_yields_clause` already checked its shape) may appear as any
    // intermediate value of *exactly* the declared request or response
    // type -- the request built at a `yield` site, the answer bound from
    // one, and any later expression over that same bound value (a field
    // read's own receiver, say). This is never widened to any other
    // record/variant: a value of some *other* aggregate type used only as
    // body scratch still keeps the ordinary `SPX-T303` below, exactly as it
    // did before this channel type was admitted.
    if !borrowed
        && (expr.ty == yields.request_type
            || expr.ty == yields.response_type
            || expr.ty == *return_type
            || params.iter().any(|param| expr.ty == param.ty))
    {
        return Ok(());
    }
    let (code, reason) = profile_refusal(resolver, &expr.ty, expr.ownership);
    Err(resolver.error(
        code,
        format!(
            "function `{function_name}` declares `yields` but an intermediate value is \
             outside the resumable profile: {reason}"
        ),
        expr.span,
    ))
}

/// True when some `yield` in `body` is reachable only through an `if`/`else`
/// branch or a `while` body rather than being a direct top-level statement
/// value or tail of the function's own outermost block. Mirrors
/// `resumable_effects::lowering::control::is_control_dependent`
/// independently, in the shape hir's own resolved nodes already give it
/// (`hir` does not, and must not, depend on `resumable_effects`), so a
/// bounded aggregate `yields` channel can be refused for this placement
/// before it ever reaches lowering.
fn body_is_control_dependent(body: &ResolvedExpr) -> bool {
    fn count_yields(expr: &ResolvedExpr) -> usize {
        let mut count = 0usize;
        let mut pending = vec![expr];
        while let Some(expr) = pending.pop() {
            if matches!(expr.kind, ResolvedExprKind::Yield { .. }) {
                count += 1;
            }
            super::push_resolved_expression_children_in_authored_order(expr, &mut pending);
        }
        count
    }
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    let direct = statements
        .iter()
        .filter(|statement| {
            matches!(
                statement,
                super::expr_nodes::ResolvedStatement::Let { value, .. }
                    | super::expr_nodes::ResolvedStatement::Assign { value, .. }
                    if matches!(value.kind, ResolvedExprKind::Yield { .. })
            )
        })
        .count()
        + usize::from(matches!(tail.kind, ResolvedExprKind::Yield { .. }));
    count_yields(body) != direct
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
    params: &[ResolvedParam],
    return_type: &ResolvedType,
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
                params,
                return_type,
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
                    params,
                    return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
                args,
                found,
                yielded_bindings,
            )?;
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. } => {
            scan_children(
                resolver,
                function_name,
                yields,
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                    params,
                    return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                    params,
                    return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
                params,
                return_type,
                base,
                false,
                false,
                found,
                yielded_bindings,
            )?;
        }
    }
    check_scalar(resolver, function_name, yields, params, return_type, expr)
}

fn scan_children(
    resolver: &Resolver<'_>,
    function_name: &str,
    yields: &ResolvedYieldsClause,
    params: &[ResolvedParam],
    return_type: &ResolvedType,
    children: &mut [ResolvedExpr],
    found: &mut usize,
    yielded_bindings: &mut BTreeMap<super::ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    for child in children.iter_mut() {
        scan_expr(
            resolver,
            function_name,
            yields,
            params,
            return_type,
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
    params: &[ResolvedParam],
    return_type: &ResolvedType,
    fields: &mut [ResolvedFieldInitializer],
    found: &mut usize,
    yielded_bindings: &mut BTreeMap<super::ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    for field in fields.iter_mut() {
        scan_expr(
            resolver,
            function_name,
            yields,
            params,
            return_type,
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
    params: &[ResolvedParam],
    return_type: &ResolvedType,
    arm: &mut ResolvedMatchArm,
    found: &mut usize,
    yielded_bindings: &mut BTreeMap<super::ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    if let Some(guard) = &mut arm.guard {
        scan_expr(
            resolver,
            function_name,
            yields,
            params,
            return_type,
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
        params,
        return_type,
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
    params: &[ResolvedParam],
    return_type: &ResolvedType,
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
                params,
                return_type,
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
                params,
                return_type,
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
            params,
            return_type,
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
                params,
                return_type,
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
                params,
                return_type,
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
mod tests;

#[cfg(test)]
mod owned_frame_tests;
