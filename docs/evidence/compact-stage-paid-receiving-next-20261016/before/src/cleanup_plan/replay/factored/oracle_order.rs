//! Fallible exact derived order for the Oracle lease, over every
//! `SkeletonObservation`, `CleanupPlace`, `StagedCopyResultSource` and
//! `ResolvedType` variant. Every evaluated field, identity byte or stack step
//! is charged before it runs; scratch growth is reserved before it happens.
//! The ordinary derived `Ord` and the borrowed `Key` stay untouched.
use super::*;
use std::borrow::Borrow;
use std::cmp::Ordering;

macro_rules! decide {
    ($order:expr) => {{
        let order = $order;
        if order != Ordering::Equal {
            return Ok(order);
        }
    }};
}

fn scalar<T: Ord>(
    a: &T,
    b: &T,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<Ordering, Diagnostic> {
    budget.charge(function, 1, phase)?;
    Ok(a.cmp(b))
}

/// Byte-exact UTF-8 order (NUL included): each compared byte, then length.
fn text(
    a: &str,
    b: &str,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    let mut a = a.bytes();
    let mut b = b.bytes();
    loop {
        budget.charge(function, 1, "oracle identity byte or end")?;
        match (a.next(), b.next()) {
            (None, None) => return Ok(Ordering::Equal),
            (None, Some(_)) => return Ok(Ordering::Less),
            (Some(_), None) => return Ok(Ordering::Greater),
            (Some(x), Some(y)) => decide!(x.cmp(&y)),
        }
    }
}

fn place(
    a: &CleanupPlace,
    b: &CleanupPlace,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    decide!(scalar(
        &a.storage,
        &b.storage,
        function,
        budget,
        "oracle place storage"
    )?);
    for (x, y) in a.projections.iter().zip(&b.projections) {
        budget.charge(function, 1, "oracle place projection member")?;
        decide!(text(x.as_str(), y.as_str(), function, budget)?);
    }
    budget.charge(function, 1, "oracle place projection length")?;
    Ok(a.projections.len().cmp(&b.projections.len()))
}

fn arguments(
    a: &[(u32, CleanupPlace)],
    b: &[(u32, CleanupPlace)],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    for (x, y) in a.iter().zip(b) {
        budget.charge(function, 1, "oracle call argument member")?;
        decide!(scalar(
            &x.0,
            &y.0,
            function,
            budget,
            "oracle call argument index"
        )?);
        decide!(place(&x.1, &y.1, function, budget)?);
    }
    budget.charge(function, 1, "oracle call argument length")?;
    Ok(a.len().cmp(&b.len()))
}

fn execution(
    a: &crate::hir::FunctionExecutionId,
    b: &crate::hir::FunctionExecutionId,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    use crate::hir::FunctionExecutionId::{Generic, Monomorphic};
    budget.charge(function, 1, "oracle execution identity kind")?;
    match (a, b) {
        (Monomorphic(x), Monomorphic(y)) => text(x.as_str(), y.as_str(), function, budget),
        (Generic(x), Generic(y)) => text(x.as_str(), y.as_str(), function, budget),
        (Monomorphic(_), Generic(_)) => Ok(Ordering::Less),
        (Generic(_), Monomorphic(_)) => Ok(Ordering::Greater),
    }
}

fn target(
    a: &crate::hir::ControlTarget,
    b: &crate::hir::ControlTarget,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    use crate::hir::ControlTarget::{Function, Loop};
    budget.charge(function, 1, "oracle control target kind")?;
    match (a, b) {
        (Function(x), Function(y)) => execution(x, y, function, budget),
        (Function(_), Loop { .. }) => Ok(Ordering::Less),
        (Loop { .. }, Function(_)) => Ok(Ordering::Greater),
        (
            Loop {
                owner: xo,
                site: xs,
            },
            Loop {
                owner: yo,
                site: ys,
            },
        ) => {
            decide!(execution(xo, yo, function, budget)?);
            text(xs.as_str(), ys.as_str(), function, budget)
        }
    }
}

fn mismatch(function: &ResolvedFunction) -> Diagnostic {
    replay_error(function, "oracle order ordinal disagrees with variant")
}

// ---------------------------------------------------------------- types ----

fn type_ordinal(ty: &ResolvedType) -> u8 {
    match ty {
        ResolvedType::Never => 0,
        ResolvedType::OnceFunction => 1,
        ResolvedType::OnceFunctionI64 => 2,
        ResolvedType::OnceFunctionI64Pair => 3,
        ResolvedType::MutFunctionI64 => 4,
        ResolvedType::Function { .. } => 5,
        ResolvedType::Unit => 6,
        ResolvedType::I64 => 7,
        ResolvedType::I32 => 8,
        ResolvedType::Char => 9,
        ResolvedType::U8 => 10,
        ResolvedType::Usize => 11,
        ResolvedType::ArrayU8(_) => 12,
        ResolvedType::F32 => 13,
        ResolvedType::F64 => 14,
        ResolvedType::Bool => 15,
        ResolvedType::String => 16,
        ResolvedType::Bytes => 17,
        ResolvedType::Str => 18,
        ResolvedType::SliceU8 => 19,
        ResolvedType::StringMap => 20,
        ResolvedType::TypeParameter { .. } => 21,
        ResolvedType::Nominal { .. } => 22,
    }
}

/// Pending work on the paid explicit stack. Depth follows type nesting only;
/// sequences are walked lazily by cursor, so an early difference never pays
/// for unevaluated members.
enum Frame<'a> {
    Pair(&'a ResolvedType, &'a ResolvedType),
    Seq {
        a: &'a [ResolvedType],
        b: &'a [ResolvedType],
        next: usize,
    },
}

fn grow<T>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    values: &mut Vec<T>,
    additional: usize,
    phase: &str,
) -> Result<(), Diagnostic> {
    let required = values
        .len()
        .checked_add(additional)
        .ok_or_else(|| replay_error(function, "oracle type stack length overflowed"))?;
    if required > values.capacity() {
        budget.charge(function, values.len(), "oracle type stack relocation")?;
        if values.capacity() != 0 {
            budget.charge(function, 1, "oracle type old backing release")?;
        }
    }
    budget.reserve_vec(function, values, additional, phase)
}

pub(super) fn resolved_type(
    a: &ResolvedType,
    b: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    // Frame holds only borrowed references/scalars, so its slots have no
    // destructor walk. The sole receiving buffer release is prepaid here.
    budget.charge(
        function,
        2,
        "oracle type stack acquisition and final release",
    )?;
    let mut stack: Vec<Frame<'_>> = Vec::new();
    grow(function, budget, &mut stack, 1, "oracle type initial frame")?;
    stack.push(Frame::Pair(a, b));
    while let Some(top) = stack.last_mut() {
        budget.charge(function, 1, "oracle type frame")?;
        match top {
            Frame::Seq { a, b, next } => {
                let (xs, ys, index) = (*a, *b, *next);
                if index < xs.len() && index < ys.len() {
                    *next = index + 1;
                    grow(function, budget, &mut stack, 1, "oracle type member frame")?;
                    stack.push(Frame::Pair(&xs[index], &ys[index]));
                } else {
                    stack.pop();
                    budget.charge(function, 1, "oracle type sequence length")?;
                    decide!(xs.len().cmp(&ys.len()));
                }
            }
            Frame::Pair(x, y) => {
                let (x, y) = (*x, *y);
                stack.pop();
                decide!(type_ordinal(x).cmp(&type_ordinal(y)));
                match (x, y) {
                    (
                        ResolvedType::Function {
                            parameters: xp,
                            result: xr,
                        },
                        ResolvedType::Function {
                            parameters: yp,
                            result: yr,
                        },
                    ) => {
                        budget.charge(function, 2, "oracle type function frames")?;
                        grow(
                            function,
                            budget,
                            &mut stack,
                            2,
                            "oracle type function frames",
                        )?;
                        // Result runs last; parameters run first.
                        stack.push(Frame::Pair(xr, yr));
                        stack.push(Frame::Seq {
                            a: xp,
                            b: yp,
                            next: 0,
                        });
                    }
                    (
                        ResolvedType::Nominal {
                            declaration: xd,
                            arguments: xa,
                        },
                        ResolvedType::Nominal {
                            declaration: yd,
                            arguments: ya,
                        },
                    ) => {
                        decide!(text(xd.as_str(), yd.as_str(), function, budget)?);
                        budget.charge(function, 1, "oracle type argument frame")?;
                        grow(
                            function,
                            budget,
                            &mut stack,
                            1,
                            "oracle type argument frame",
                        )?;
                        stack.push(Frame::Seq {
                            a: xa,
                            b: ya,
                            next: 0,
                        });
                    }
                    (
                        ResolvedType::TypeParameter {
                            owner: xo,
                            index: xi,
                        },
                        ResolvedType::TypeParameter {
                            owner: yo,
                            index: yi,
                        },
                    ) => {
                        decide!(text(xo.as_str(), yo.as_str(), function, budget)?);
                        decide!(scalar(
                            xi,
                            yi,
                            function,
                            budget,
                            "oracle type parameter index"
                        )?);
                    }
                    (ResolvedType::ArrayU8(xn), ResolvedType::ArrayU8(yn)) => {
                        decide!(scalar(
                            xn,
                            yn,
                            function,
                            budget,
                            "oracle type array length"
                        )?);
                    }
                    (ResolvedType::Never, ResolvedType::Never)
                    | (ResolvedType::OnceFunction, ResolvedType::OnceFunction)
                    | (ResolvedType::OnceFunctionI64, ResolvedType::OnceFunctionI64)
                    | (ResolvedType::OnceFunctionI64Pair, ResolvedType::OnceFunctionI64Pair)
                    | (ResolvedType::MutFunctionI64, ResolvedType::MutFunctionI64)
                    | (ResolvedType::Unit, ResolvedType::Unit)
                    | (ResolvedType::I64, ResolvedType::I64)
                    | (ResolvedType::I32, ResolvedType::I32)
                    | (ResolvedType::Char, ResolvedType::Char)
                    | (ResolvedType::U8, ResolvedType::U8)
                    | (ResolvedType::Usize, ResolvedType::Usize)
                    | (ResolvedType::F32, ResolvedType::F32)
                    | (ResolvedType::F64, ResolvedType::F64)
                    | (ResolvedType::Bool, ResolvedType::Bool)
                    | (ResolvedType::String, ResolvedType::String)
                    | (ResolvedType::Bytes, ResolvedType::Bytes)
                    | (ResolvedType::Str, ResolvedType::Str)
                    | (ResolvedType::SliceU8, ResolvedType::SliceU8)
                    | (ResolvedType::StringMap, ResolvedType::StringMap) => {}
                    _ => return Err(mismatch(function)),
                }
            }
        }
    }
    Ok(Ordering::Equal)
}

// --------------------------------------------------------- staged source ----

fn compact_staged(
    a: &super::super::compact_stage::CopySource,
    b: &super::super::compact_stage::CopySource,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    use super::super::compact_stage::CopySource;
    match (a, b) {
        (CopySource::Residual(a), CopySource::Residual(b)) => {
            staged(a.canonical(), b.canonical(), function, budget)
        }
        (
            CopySource::Body {
                expression: xe,
                instance: xi,
            },
            CopySource::Body {
                expression: ye,
                instance: yi,
            },
        ) => {
            budget.charge(function, 1, "oracle staged source kind")?;
            decide!(text(xe.as_str(), ye.as_str(), function, budget)?);
            resolved_type(xi, yi, function, budget)
        }
        (CopySource::Body { .. }, CopySource::Residual(_)) => {
            budget.charge(function, 1, "oracle staged source kind")?;
            Ok(Ordering::Less)
        }
        (CopySource::Residual(_), CopySource::Body { .. }) => {
            budget.charge(function, 1, "oracle staged source kind")?;
            Ok(Ordering::Greater)
        }
    }
}

fn staged_ordinal(source: &StagedCopyResultSource) -> u8 {
    match source {
        StagedCopyResultSource::Body { .. } => 0,
        StagedCopyResultSource::TryResidual { .. } => 1,
        StagedCopyResultSource::TryOptionNone { .. } => 2,
    }
}

pub(super) fn staged(
    a: &StagedCopyResultSource,
    b: &StagedCopyResultSource,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    budget.charge(function, 1, "oracle staged source kind")?;
    decide!(staged_ordinal(a).cmp(&staged_ordinal(b)));
    macro_rules! ids {
        ($($x:expr, $y:expr);+ $(;)?) => {$(
            decide!(text($x.as_str(), $y.as_str(), function, budget)?);
        )+};
    }
    match (a, b) {
        (
            StagedCopyResultSource::Body {
                expression: xe,
                instance: xi,
            },
            StagedCopyResultSource::Body {
                expression: ye,
                instance: yi,
            },
        ) => {
            ids!(xe, ye);
            resolved_type(xi, yi, function, budget)
        }
        (
            StagedCopyResultSource::TryResidual {
                expression: xe,
                operand: xo,
                source_instance: xs,
                target_instance: xt,
                result: xr,
                ok_case: xoc,
                ok_field: xof,
                err_case: xec,
                err_field: xef,
            },
            StagedCopyResultSource::TryResidual {
                expression: ye,
                operand: yo,
                source_instance: ys,
                target_instance: yt,
                result: yr,
                ok_case: yoc,
                ok_field: yof,
                err_case: yec,
                err_field: yef,
            },
        ) => {
            ids!(xe, ye; xo, yo);
            decide!(resolved_type(xs, ys, function, budget)?);
            decide!(resolved_type(xt, yt, function, budget)?);
            ids!(xr, yr; xoc, yoc; xof, yof; xec, yec; xef, yef);
            Ok(Ordering::Equal)
        }
        (
            StagedCopyResultSource::TryOptionNone {
                expression: xe,
                operand: xo,
                source_instance: xs,
                target_instance: xt,
                option: xp,
                some_case: xsc,
                some_field: xsf,
                none_case: xnc,
            },
            StagedCopyResultSource::TryOptionNone {
                expression: ye,
                operand: yo,
                source_instance: ys,
                target_instance: yt,
                option: yp,
                some_case: ysc,
                some_field: ysf,
                none_case: ync,
            },
        ) => {
            ids!(xe, ye; xo, yo);
            decide!(resolved_type(xs, ys, function, budget)?);
            decide!(resolved_type(xt, yt, function, budget)?);
            ids!(xp, yp; xsc, ysc; xsf, ysf; xnc, ync);
            Ok(Ordering::Equal)
        }
        _ => Err(mismatch(function)),
    }
}

// ----------------------------------------------------------- observation ----

fn ordinal(observation: &SkeletonObservation) -> u8 {
    match observation {
        SkeletonObservation::ReserveRenewal { .. } => 0,
        SkeletonObservation::Initialize { .. } => 1,
        SkeletonObservation::Transfer { .. } => 2,
        SkeletonObservation::CallCommit { .. } => 3,
        SkeletonObservation::Boolean { .. } => 4,
        SkeletonObservation::VariantCase { .. } => 5,
        SkeletonObservation::ArmSelected { .. } => 6,
        SkeletonObservation::Status { .. } => 7,
        SkeletonObservation::StageCopyResult(_) => 8,
        SkeletonObservation::LexicalExit { .. } => 9,
        SkeletonObservation::LoopLatch { .. } => 10,
    }
}

pub(super) fn observation(
    a: &SkeletonObservation,
    b: &SkeletonObservation,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    use SkeletonObservation as O;
    budget.charge(function, 1, "oracle observation kind")?;
    decide!(ordinal(a).cmp(&ordinal(b)));
    match (a, b) {
        (
            O::ReserveRenewal {
                at: xa,
                binding: xb,
            },
            O::ReserveRenewal {
                at: ya,
                binding: yb,
            },
        ) => {
            decide!(text(xa.as_str(), ya.as_str(), function, budget)?);
            place(xb, yb, function, budget)
        }
        (
            O::Initialize {
                at: xa,
                destination: xd,
            },
            O::Initialize {
                at: ya,
                destination: yd,
            },
        ) => {
            decide!(text(xa.as_str(), ya.as_str(), function, budget)?);
            place(xd, yd, function, budget)
        }
        (
            O::Transfer {
                at: xa,
                source: xs,
                destination: xd,
            },
            O::Transfer {
                at: ya,
                source: ys,
                destination: yd,
            },
        ) => {
            decide!(text(xa.as_str(), ya.as_str(), function, budget)?);
            decide!(place(xs, ys, function, budget)?);
            place(xd, yd, function, budget)
        }
        (
            O::CallCommit {
                call: xc,
                arguments: xa,
            },
            O::CallCommit {
                call: yc,
                arguments: ya,
            },
        ) => {
            decide!(text(xc.as_str(), yc.as_str(), function, budget)?);
            arguments(xa, ya, function, budget)
        }
        (
            O::Boolean {
                expression: xe,
                value: xv,
            },
            O::Boolean {
                expression: ye,
                value: yv,
            },
        ) => {
            decide!(text(xe.as_str(), ye.as_str(), function, budget)?);
            scalar(xv, yv, function, budget, "oracle boolean value")
        }
        (
            O::VariantCase {
                scrutinee: xs,
                case: xc,
                matches: xm,
            },
            O::VariantCase {
                scrutinee: ys,
                case: yc,
                matches: ym,
            },
        ) => {
            decide!(text(xs.as_str(), ys.as_str(), function, budget)?);
            decide!(text(xc.as_str(), yc.as_str(), function, budget)?);
            scalar(xm, ym, function, budget, "oracle variant match flag")
        }
        (
            O::ArmSelected {
                scrutinee: xs,
                arm: xa,
                selected: xv,
            },
            O::ArmSelected {
                scrutinee: ys,
                arm: ya,
                selected: yv,
            },
        ) => {
            decide!(text(xs.as_str(), ys.as_str(), function, budget)?);
            decide!(scalar(xa, ya, function, budget, "oracle arm ordinal")?);
            scalar(xv, yv, function, budget, "oracle arm selected flag")
        }
        (
            O::Status {
                source: xs,
                success: xv,
            },
            O::Status {
                source: ys,
                success: yv,
            },
        ) => {
            decide!(text(
                xs.expression.as_str(),
                ys.expression.as_str(),
                function,
                budget
            )?);
            decide!(scalar(
                &xs.lane,
                &ys.lane,
                function,
                budget,
                "oracle status lane"
            )?);
            scalar(xv, yv, function, budget, "oracle status success flag")
        }
        (O::StageCopyResult(x), O::StageCopyResult(y)) => compact_staged(x, y, function, budget),
        (
            O::LexicalExit {
                site: xs,
                kind: xk,
                target: xt,
            },
            O::LexicalExit {
                site: ys,
                kind: yk,
                target: yt,
            },
        ) => {
            decide!(text(xs.as_str(), ys.as_str(), function, budget)?);
            decide!(scalar(
                xk,
                yk,
                function,
                budget,
                "oracle lexical exit kind"
            )?);
            target(xt, yt, function, budget)
        }
        (O::LoopLatch { target: x }, O::LoopLatch { target: y }) => target(x, y, function, budget),
        _ => Err(mismatch(function)),
    }
}

// ------------------------------------------------------- label and path ----

pub(super) fn label(
    a: &Label,
    b: &Label,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    budget.charge(function, 1, "oracle label kind")?;
    match (a, b) {
        (Label::Observation(x), Label::Observation(y)) => {
            observation(x.as_ref(), y.as_ref(), function, budget)
        }
        (Label::Observation(_), Label::End(_)) => Ok(Ordering::Less),
        (Label::End(_), Label::Observation(_)) => Ok(Ordering::Greater),
        (Label::End(x), Label::End(y)) => scalar(x, y, function, budget, "oracle label terminal"),
    }
}

fn items<I: Borrow<SkeletonObservation>>(
    a: &[I],
    b: &[I],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    for (x, y) in a.iter().zip(b) {
        budget.charge(function, 1, "oracle flat observation member")?;
        decide!(observation(x.borrow(), y.borrow(), function, budget)?);
    }
    budget.charge(function, 1, "oracle flat observation length")?;
    Ok(a.len().cmp(&b.len()))
}

/// Exact existing prefix pointer identity, then every flat observation.
/// A pointer here identifies shared proof storage; it never grants admission.
fn observations(
    a: &Observations,
    b: &Observations,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    let (ap, ai) = a.parts();
    let (bp, bi) = b.parts();
    let ap = ap.map(|node| Rc::as_ptr(node) as usize);
    let bp = bp.map(|node| Rc::as_ptr(node) as usize);
    decide!(scalar(
        &ap,
        &bp,
        function,
        budget,
        "oracle observation prefix identity"
    )?);
    items(ai, bi, function, budget)
}

pub(super) fn path(
    a: &SkeletonPath,
    b: &SkeletonPath,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    decide!(observations(
        &a.observations,
        &b.observations,
        function,
        budget
    )?);
    scalar(
        &a.terminal,
        &b.terminal,
        function,
        budget,
        "oracle path terminal",
    )
}

/// Fallible heapsort over the already owned paths. Every probe and swap is
/// paid before execution; no standard-library sort scratch is acquired and
/// no provisional ordering can escape a refused comparison.
pub(super) fn sort(
    paths: &mut [SkeletonPath],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    budget.charge(function, 1, "oracle sort header")?;
    let length = paths.len();
    for root in (0..length / 2).rev() {
        sift(paths, root, length, function, budget)?;
    }
    for end in (1..length).rev() {
        budget.charge(function, 1, "oracle sort root swap")?;
        paths.swap(0, end);
        sift(paths, 0, end, function, budget)?;
    }
    Ok(())
}
fn sift(
    paths: &mut [SkeletonPath],
    mut root: usize,
    end: usize,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    loop {
        budget.charge(function, 1, "oracle sort child index")?;
        let Some(left) = root.checked_mul(2).and_then(|i| i.checked_add(1)) else {
            return Ok(());
        };
        if left >= end {
            return Ok(());
        }
        let mut child = left;
        let right = left + 1;
        budget.charge(function, 1, "oracle sort sibling probe")?;
        if right < end && path(&paths[left], &paths[right], function, budget)? == Ordering::Less {
            child = right;
        }
        budget.charge(function, 1, "oracle sort parent probe")?;
        if path(&paths[root], &paths[child], function, budget)? != Ordering::Less {
            return Ok(());
        }
        budget.charge(function, 1, "oracle sort sift swap")?;
        paths.swap(root, child);
        root = child;
    }
}

#[cfg(test)]
mod tests;
