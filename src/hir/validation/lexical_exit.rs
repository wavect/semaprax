//! Bottom joins retain only reachable normal ownership states.
use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_lexical_return_operand(
        &self,
        operand: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
    ) -> Result<(), Diagnostic> {
        if operand.ty == ResolvedType::Never
            || operand.ownership != self.expected_ownership(&operand.ty, OwnershipMode::Own)?
        {
            return Err(hir_error(
                "lexical return operand has invalid result ownership",
            ));
        }
        if operand.ownership == OwnershipMode::Own {
            self.mark_value_sources_moved(operand, scope)?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(super) fn finish_lexical_if(
        &self,
        expression: &ResolvedExpr,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        outer: &mut BTreeMap<ValueId, ValidationBinding>,
        left_scope: &dyn ScopeRead,
        right_scope: &dyn ScopeRead,
        ids: &[ValueId],
    ) -> Result<(), Diagnostic> {
        match (
            super::super::lexical_exit::returns_only(left),
            super::super::lexical_exit::returns_only(right),
        ) {
            (true, false) => Self::merge_availability(outer, right_scope, ids),
            (false, true) => Self::merge_availability(outer, left_scope, ids),
            _ => Self::join_branches(outer, left_scope, right_scope, ids),
        }
        let normal = match (
            left.ty == ResolvedType::Never,
            right.ty == ResolvedType::Never,
        ) {
            (true, false) => right,
            (false, true) => left,
            (a, b) => {
                if !a && !b {
                    self.require_type(&left.ty, &right.ty, "if branches")?;
                    if left.ownership != right.ownership {
                        return Err(hir_error("if branches have inconsistent ownership"));
                    }
                }
                left
            }
        };
        self.finish_expr(expression, &normal.ty, normal.ownership)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish_lexical_if_checkpoint(
        &self,
        expression: &ResolvedExpr,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        outer: &mut dyn ScopeWrite,
        left_scope: &dyn ScopeRead,
        right_scope: &dyn ScopeRead,
        ids: &dyn scope_checkpoint::Ids,
    ) -> Result<(), Diagnostic> {
        match (
            super::super::lexical_exit::returns_only(left),
            super::super::lexical_exit::returns_only(right),
        ) {
            (true, false) => scope_checkpoint::merge::availability(outer, right_scope, ids)?,
            (false, true) => scope_checkpoint::merge::availability(outer, left_scope, ids)?,
            _ => scope_checkpoint::merge::branches(outer, left_scope, right_scope, ids)?,
        }
        let normal = match (
            left.ty == ResolvedType::Never,
            right.ty == ResolvedType::Never,
        ) {
            (true, false) => right,
            (false, true) => left,
            (a, b) => {
                if !a && !b {
                    self.require_type(&left.ty, &right.ty, "if branches")?;
                    if left.ownership != right.ownership {
                        return Err(hir_error("if branches have inconsistent ownership"));
                    }
                }
                left
            }
        };
        self.finish_expr(expression, &normal.ty, normal.ownership)
    }
}

impl HirValidator<'_> {
    #[cfg(test)]
    pub(super) fn join_lexical_match(
        &self,
        result: &mut Option<(ResolvedType, OwnershipMode)>,
        value: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        if let Some((ty, ownership)) = result.as_ref() {
            if *ty != ResolvedType::Never && value.ty != ResolvedType::Never {
                self.require_type(&value.ty, ty, "match arm")?;
                if value.ownership != *ownership {
                    return Err(hir_error("resolved match arms have inconsistent ownership"));
                }
            }
        }
        if result
            .as_ref()
            .is_none_or(|(ty, _)| *ty == ResolvedType::Never)
        {
            *result = Some((value.ty.clone(), value.ownership));
        }
        Ok(())
    }
}
impl HirValidator<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_template_lexical_exit<'value>(
        &mut self,
        template: &'value ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &'value ResolvedExpr,
        value: Option<&'value ResolvedExpr>,
        values: &mut generic_template::values::Values<'value>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if expression.ty != ResolvedType::Never || expression.ownership != OwnershipMode::Value {
            return Err(hir_error("generic lexical exit has a runtime value"));
        }
        if let Some(operand) = value {
            self.validate_template_expr(
                template,
                execution,
                operand,
                values,
                &format!("{path}.value"),
            )?;
            self.require_type(&operand.ty, &template.return_type, "generic lexical return")?;
        }
        // Canonical template replay checks exact target/contour; each materialized
        // instance independently validates its execution-rekeyed typed contour.
        Ok(())
    }
}

// The new witness continuation's explicit transport and local receivers. These
// are additions to the unchanged full-capacity Storage<Frame> backing invoice;
// removed type clones receive no separate byte credit.
type WitnessJoinCall<'a, 'v> = (
    &'a HirValidator<'v>,
    &'a mut Option<&'v ResolvedExpr>,
    &'v ResolvedExpr,
);
const PAYMENT: usize =
    2 * std::mem::size_of::<Option<usize>>() + 2 * std::mem::size_of::<Result<(), Diagnostic>>();
// A fixed complete helper activation also owns its two reachable delegated
// call envelopes; it claims no credit for the independently retained old helper.
type WitnessTypeCall<'a, 'v> = (
    &'a HirValidator<'v>,
    &'a ResolvedType,
    &'a ResolvedType,
    &'a str,
);
const WITNESS_TYPE_CALL: usize = 2 * std::mem::size_of::<WitnessTypeCall<'static, 'static>>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>();
const WITNESS_ERROR_CALL: usize =
    2 * std::mem::size_of::<&str>() + 2 * std::mem::size_of::<Diagnostic>();
pub(super) const WITNESS_JOIN: usize = PAYMENT
    + WITNESS_TYPE_CALL
    + WITNESS_ERROR_CALL
    + 2 * std::mem::size_of::<WitnessJoinCall<'static, 'static>>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
    + std::mem::size_of::<Option<&ResolvedExpr>>() // copied selector value
    + std::mem::size_of::<&ResolvedExpr>() // selected already-validated witness
    + std::mem::size_of::<Option<&ResolvedExpr>>() // replacement initializer
    + 4 * std::mem::size_of::<bool>(); // two Never tests, ownership, replacement test
                                       // Each immutable Vec field produces its pattern binding, then the actual and
                                       // formal Deref receiver and returned/received exact old slice view. Subsequent
                                       // len/index/substitution operations retain their original slice types.
const ARMS_VIEW: usize = 3 * std::mem::size_of::<&Vec<ResolvedMatchArm>>()
    + 2 * std::mem::size_of::<&[ResolvedMatchArm]>();
const ARGUMENTS_VIEW: usize =
    3 * std::mem::size_of::<&Vec<ResolvedType>>() + 2 * std::mem::size_of::<&[ResolvedType]>();
pub(super) const VARIANT_VIEWS: usize = PAYMENT
    + ARMS_VIEW
    + std::mem::size_of::<&DeclarationId>()
    + ARGUMENTS_VIEW
    + 2 * std::mem::size_of::<bool>();
pub(super) const VARIANT_ARM_VIEW: usize = PAYMENT + ARMS_VIEW + std::mem::size_of::<bool>();

// Result selection is now a borrowed std::Option/Result receiver rather than
// the old owning tuple. Keep its whole new envelope, projection and error
// closure roles explicit; no old tuple/payload credit is claimed.
pub(super) const WITNESS_FINISH: usize = PAYMENT
    + 2 * std::mem::size_of::<Option<&ResolvedExpr>>()
    + 2 * std::mem::size_of::<Result<&ResolvedExpr, Diagnostic>>()
    + std::mem::size_of::<&ResolvedExpr>()
    + std::mem::size_of::<&ResolvedType>()
    + std::mem::size_of::<OwnershipMode>()
    + WITNESS_ERROR_CALL;

impl HirValidator<'_> {
    /// Called only by AfterArm, after the complete arm body succeeded and the
    /// original borrow-release and owned-result checks. The immutable expression
    /// itself witnesses the type; a reconstructed/cached type is never authority.
    pub(super) fn join_lexical_match_witness<'value>(
        &self,
        result: &mut Option<&'value ResolvedExpr>,
        value: &'value ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let Some(previous) = *result else {
            *result = Some(value);
            return Ok(());
        };
        if previous.ty != ResolvedType::Never && value.ty != ResolvedType::Never {
            self.require_type(&value.ty, &previous.ty, "match arm")?;
            if value.ownership != previous.ownership {
                return Err(hir_error("resolved match arms have inconsistent ownership"));
            }
        }
        if previous.ty == ResolvedType::Never {
            *result = Some(value);
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
struct Frontier {
    remaining: usize,
    amount: usize,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
struct FrameRequests {
    frame_type: Option<&'static str>,
    size: usize,
    alignment: usize,
    requested_capacity: usize,
    requests: usize,
    acquired_capacity: usize,
    joins: usize,
    variant_views: usize,
    arm_views: usize,
    finishes: usize,
    first_join: Option<Frontier>,
    first_variant: Option<Frontier>,
    first_arm: Option<Frontier>,
    first_finish: Option<Frontier>,
    overflow: bool,
}
#[cfg(test)]
thread_local! {
    static FRAME_REQUESTS: std::cell::Cell<Option<FrameRequests>> = const { std::cell::Cell::new(None) };
    static PANIC_AFTER_WITNESS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(in crate::hir::validation) fn observe_frame_layout<T>(size: usize, alignment: usize) {
    FRAME_REQUESTS.with(|cell| {
        if let Some(mut report) = cell.get() {
            let frame_type = std::any::type_name::<T>();
            if report
                .frame_type
                .is_some_and(|previous| previous != frame_type)
            {
                report.overflow = true;
            }
            report.frame_type = Some(frame_type);
            if report.size != 0 && (report.size != size || report.alignment != alignment) {
                report.overflow = true;
            }
            report.size = size;
            report.alignment = alignment;
            cell.set(Some(report));
        }
    });
}
#[cfg(test)]
pub(in crate::hir::validation) fn observe_frame_request<T>(capacity: usize) {
    // Diagnostic type label only; it neither selects an algorithm nor funds a
    // receiver. The charged Storage<T> request and its allocation remain intact.
    FRAME_REQUESTS.with(|cell| {
        if let Some(mut report) = cell.get() {
            let Some(frame_type) = report.frame_type else {
                report.overflow = true;
                cell.set(Some(report));
                return;
            };
            if frame_type != std::any::type_name::<T>() {
                return;
            }
            match (
                report.requested_capacity.checked_add(capacity),
                report.requests.checked_add(1),
            ) {
                (Some(total), Some(requests)) => {
                    report.requested_capacity = total;
                    report.requests = requests;
                }
                _ => report.overflow = true,
            }
            cell.set(Some(report));
        }
    });
}

#[cfg(test)]
pub(in crate::hir::validation) fn observe_frame_acquired<T>(capacity: usize) {
    FRAME_REQUESTS.with(|cell| {
        if let Some(mut report) = cell.get() {
            let Some(frame_type) = report.frame_type else {
                report.overflow = true;
                cell.set(Some(report));
                return;
            };
            if frame_type != std::any::type_name::<T>() {
                return;
            }
            match report.acquired_capacity.checked_add(capacity) {
                Some(total) => report.acquired_capacity = total,
                None => report.overflow = true,
            }
            cell.set(Some(report));
        }
    });
}
#[cfg(test)]
pub(in crate::hir::validation) fn observe_witness_role(role: &'static str) {
    FRAME_REQUESTS.with(|cell| {
        if let Some(mut report) = cell.get() {
            let count = match role {
                "join" => &mut report.joins,
                "variant" => &mut report.variant_views,
                "arm" => &mut report.arm_views,
                "finish" => &mut report.finishes,
                _ => {
                    report.overflow = true;
                    cell.set(Some(report));
                    return;
                }
            };
            match count.checked_add(1) {
                Some(total) => *count = total,
                None => report.overflow = true,
            }
            cell.set(Some(report));
        }
    });
}

#[cfg(test)]
pub(in crate::hir::validation) fn observe_witness_frontier(role: &'static str, amount: usize) {
    let Some(remaining) = crate::bounded_output::active_remaining() else {
        return;
    };
    FRAME_REQUESTS.with(|cell| {
        if let Some(mut report) = cell.get() {
            let first = match role {
                "join" => &mut report.first_join,
                "variant" => &mut report.first_variant,
                "arm" => &mut report.first_arm,
                "finish" => &mut report.first_finish,
                _ => {
                    report.overflow = true;
                    cell.set(Some(report));
                    return;
                }
            };
            if first.is_none() {
                *first = Some(Frontier { remaining, amount });
            }
            cell.set(Some(report));
        }
    });
}

#[cfg(test)]
pub(in crate::hir::validation) fn witness_unwind_point() {
    if PANIC_AFTER_WITNESS.with(|flag| flag.replace(false)) {
        std::panic::resume_unwind(Box::new(()));
    }
}

#[cfg(test)]
mod witness_tests {
    use super::*;
    const SCALAR: &str = "module witness; @id(\"w.f\") fn f(x:i64)->i64 {match x {0=>1, n=>n,}} @id(\"w.main\") fn main()->i64 {f(0)}";
    const MIXED_GUARD_RETURN: &str =
        "module mixed; @id(\"main\") fn main()->i64 {match 0 {0=>{return 1;}, _=>2,}}";
    fn sources() -> Vec<String> {
        let mut nested = "0".to_owned();
        for _ in 0..16 {
            nested = format!("match 0 {{0=>{nested}, _=>0,}}");
        }
        let calls = std::iter::repeat_n("f(0)", 64)
            .collect::<Vec<_>>()
            .join("+");
        let mut binary = "1".to_owned();
        for _ in 0..64 {
            binary = format!("({binary}+1)");
        }
        vec![
            format!("module deep_binary; @id(\"main\") fn main()->i64 {{{binary}}}"),
            "module tiny; @id(\"t.main\") fn main()->i64 {1}".to_owned(),
            SCALAR.to_owned(),
            "module guarded; @id(\"main\") fn main()->i64 {match 0 {0 if true=>1, n=>n,}}".to_owned(),
            "module never; @id(\"main\") fn main()->i64 {match 0 {0=>{return 1;}, _=>{return 2;},}}".to_owned(),
            "module mixed; @id(\"main\") fn main()->i64 {match 0 {0=>{1}, _=>2,}}".to_owned(),
            "module borrowed; @id(\"consume\") fn consume(value:own Result<Bytes,Bytes>)->i64 {let borrowed=match borrow value {Result::Ok {value:payload}=>if byte_len(bytes_as_slice(payload))==0usize {0}else{1},Result::Err {error:payload}=>if byte_len(bytes_as_slice(payload))==0usize {0}else{2},}; match own value {Result::Ok {value:payload}=>borrowed,Result::Err {error:payload}=>borrowed,}} @id(\"main\") fn main()->i64 {0}".to_owned(),
            format!("module nested; @id(\"main\") fn main()->i64 {{{nested}}}"),
            format!("module calls; @id(\"f\") fn f(x:i64)->i64 {{x}} @id(\"main\") fn main()->i64 {{{calls}}}"),
            "module variant_witness; @id(\"shape.type\") variant Shape {@id(\"shape.dot\") Dot,@id(\"shape.box\") Box {width:i64,},} @id(\"main\") fn main()->i64 {match Shape::Box {width:4} {Shape::Dot {}=>0,Shape::Box {width}=>width,}}".to_owned(),
        ]
    }

    #[test]
    fn borrowed_join_keeps_first_normal_latest_never_and_complete_old_errors() {
        let missing_comma = SCALAR.replacen("n=>n,}", "n=>n}", 1);
        assert_ne!(missing_comma, SCALAR);
        let syntax = crate::check(&missing_comma, "witness-missing-comma.spx").unwrap_err();
        assert_eq!(syntax[0].code, "SPX-P106");
        // Mixed returning/fallthrough arms are a retained negative, not an
        // admitted positive fixture. The positive corpus keeps block/scalar arms.
        let mixed = crate::check(MIXED_GUARD_RETURN, "witness-mixed-return.spx").unwrap_err();
        assert_eq!(mixed[0].code, "SPX-P210");
        let checked = crate::check(SCALAR, "witness.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let validator = HirValidator::new(&program).unwrap();
        let base = &program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "w.f")
            .unwrap()
            .body;
        for types in [
            vec![ResolvedType::Never],
            vec![ResolvedType::Never, ResolvedType::Never],
            vec![ResolvedType::Never, ResolvedType::I64, ResolvedType::Never],
            vec![ResolvedType::I64, ResolvedType::Never, ResolvedType::I64],
            vec![ResolvedType::I64, ResolvedType::Bool],
        ] {
            for ownership_mismatch in [false, true] {
                let values: Vec<_> = types
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, ty)| {
                        let mut value = base.clone();
                        value.ty = ty;
                        value.ownership = if ownership_mismatch && i != 0 {
                            OwnershipMode::Own
                        } else {
                            OwnershipMode::Value
                        };
                        value
                    })
                    .collect();
                let mut old = None;
                let mut witness = None;
                let mut selected = None;
                for (i, value) in values.iter().enumerate() {
                    let before = old.clone();
                    let old_outcome = validator.join_lexical_match(&mut old, value);
                    let new_outcome = validator.join_lexical_match_witness(&mut witness, value);
                    assert_eq!(format!("{old_outcome:?}"), format!("{new_outcome:?}"));
                    if old_outcome.is_err() {
                        break;
                    }
                    if before
                        .as_ref()
                        .is_none_or(|(ty, _)| *ty == ResolvedType::Never)
                    {
                        selected = Some(i);
                    }
                    let picked = witness.unwrap();
                    assert!(std::ptr::eq(picked, &values[selected.unwrap()]));
                    assert_eq!(old.as_ref(), Some(&(picked.ty.clone(), picked.ownership)));
                }
            }
        }
    }

    #[test]
    fn complete_recursive_validator_keeps_forged_cached_types_and_late_arm_failures_independent() {
        fn match_arms(expression: &mut ResolvedExpr) -> Option<&mut Vec<ResolvedMatchArm>> {
            match &mut expression.kind {
                ResolvedExprKind::Match { arms, .. } => Some(arms),
                ResolvedExprKind::Block { tail, .. }
                | ResolvedExprKind::VecConstructor { tail, .. } => match_arms(tail),
                _ => None,
            }
        }
        // Keep the complete saved depth64 oracle. On the owning unoptimized
        // toolchain its recursive body has a 154448-byte frame, so the body
        // frames alone exceed 8 MiB. This bounded test-only stack also retains
        // the wrapper/caller frames and propagates every failed assertion.
        std::thread::Builder::new()
            .name("904-complete-recursive-oracle".to_owned())
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let mut late_attacks = [0; 2];
                for source in sources() {
                    let checked = crate::check(&source, "witness-oracle.spx").unwrap();
                    let program = crate::hir::resolve(&checked).unwrap();
                    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
                    for attack in 0..5 {
                        let mut expression = function.body.clone();
                        match attack {
                            0 => {}
                            1 => expression.ty = ResolvedType::Bool,
                            2 => expression.ownership = OwnershipMode::Own,
                            3 => {
                                let Some(arms) = match_arms(&mut expression) else {
                                    continue;
                                };
                                arms.last_mut().unwrap().value.ty = ResolvedType::Bool;
                                late_attacks[0] += 1;
                            }
                            4 => {
                                let Some(arms) = match_arms(&mut expression) else {
                                    continue;
                                };
                                arms.pop().unwrap();
                                late_attacks[1] += 1;
                            }
                            _ => unreachable!(),
                        }
                        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
                        let mut iterative = HirValidator::new(&program).unwrap();
                        let mut recursive = HirValidator::new(&program).unwrap();
                        let mut iterative_scope = BTreeMap::new();
                        let mut recursive_scope = BTreeMap::new();
                        let actual = iterative.validate_expr(
                            &execution,
                            &expression,
                            &mut iterative_scope,
                            "body",
                            true,
                            None,
                        );
                        let old = recursive.validate_expr_recursive_reference_body(
                            &execution,
                            &expression,
                            &mut recursive_scope,
                            "body",
                            true,
                            None,
                        );
                        HirValidator::assert_validation_oracle(
                            &actual,
                            &old,
                            &iterative,
                            &recursive,
                            &iterative_scope,
                            &recursive_scope,
                            "body",
                        );
                        if attack == 0 {
                            assert!(actual.is_ok());
                        } else {
                            assert!(
                                actual.is_err(),
                                "attack {attack} must be genuinely malformed"
                            );
                        }
                    }
                }
                assert!(late_attacks.into_iter().all(|count| count > 0));
            })
            .expect("spawn complete recursive oracle")
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
    }

    struct Capture;
    impl Capture {
        fn begin() -> Self {
            FRAME_REQUESTS
                .with(|cell| assert!(cell.replace(Some(FrameRequests::default())).is_none()));
            Self
        }
        fn finish(self) -> FrameRequests {
            let report = FRAME_REQUESTS.with(|cell| cell.take().unwrap());
            drop(self);
            report
        }
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            FRAME_REQUESTS.with(|cell| {
                cell.take();
            });
            PANIC_AFTER_WITNESS.with(|flag| flag.set(false));
        }
    }

    fn run_public(
        checked: &crate::ast::Program,
        source: &str,
        limit: usize,
        floor: usize,
    ) -> (
        bool,
        crate::test_allocation_observer::Metrics,
        bool,
        usize,
        Option<crate::bounded_output::ReservationRefusal>,
        FrameRequests,
    ) {
        let capture = Capture::begin();
        // Reporting storage is harness setup, outside compiler observation.
        // Every compiler-owned diagnostic is still dropped inside the window.
        use std::fmt::Write as _;
        let mut diagnostic_report = String::with_capacity(1 << 16);
        let phase = std::cell::Cell::new("before-resolve");
        let ((success, metrics), refused, paid, receipt) =
            crate::bounded_output::with_limit_usage_refusal(limit, || {
                assert!(crate::bounded_output::set_active_floor(floor));
                let measured = crate::test_allocation_observer::measure_with_capacity(
                    1 << 16,
                    || {
                        phase.set("hir::resolve");
                        let resolved = match crate::hir::resolve(checked) {
                            Ok(resolved) => resolved,
                            Err(errors) => {
                                for error in &errors {
                                    writeln!(&mut diagnostic_report, "OPT904_DIAGNOSTIC phase=hir::resolve code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}", error.code, error.severity, error.message, error.path, error.span, error.help).unwrap();
                                }
                                assert!(errors.iter().all(|error| error.code == "SPX-H006"));
                                drop(errors);
                                return false;
                            }
                        };
                        phase.set("second hir::validate");
                        let outcome = crate::hir::validate(&resolved);
                        if let Err(error) = &outcome {
                            writeln!(&mut diagnostic_report, "OPT904_DIAGNOSTIC phase=hir::validate code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}", error.code, error.severity, error.message, error.path, error.span, error.help).unwrap();
                            assert_eq!(error.code, "SPX-H006");
                        }
                        let success = outcome.is_ok();
                        if success {
                            phase.set("both full passes completed");
                        }
                        drop(outcome);
                        drop(resolved);
                        success
                    },
                );
                let ledger = crate::bounded_output::classifier_string::ClassifierLedger::capture();
                let first = ledger.first_refusal();
                let remaining = crate::bounded_output::active_remaining();
                if first.is_some() {
                    assert!(scope_checkpoint::charge::reserve(Some(0)).is_err());
                    assert_eq!(ledger.first_refusal(), first);
                    assert_eq!(crate::bounded_output::active_remaining(), remaining);
                }
                crate::bounded_output::clear_active_floor();
                measured
            });
        let frame = capture.finish();
        // Darwin stderr's process-lifetime lock belongs to reporting, never
        // to the compiler's allocation/Drop interval.
        if !diagnostic_report.is_empty() {
            eprint!("{diagnostic_report}");
        }
        // Observation follows the complete allocation window, both ledger
        // passes, original sticky-refusal checks and all owned compiler Drop.
        if !success || refused || receipt.is_some() || metrics.invalid || metrics.live != 0 {
            use sha2::{Digest, Sha256};
            eprintln!(
                "OPT904_REFUSAL source_sha={:x} source_bytes={} module={} phase={} success={} refused={} paid={} first={:?} frame={:?} allocations={:?}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes())),
                source.len(),
                checked.module,
                phase.get(),
                success,
                refused,
                paid,
                receipt,
                frame,
                metrics,
            );
        }
        (success, metrics, refused, paid, receipt, frame)
    }

    #[test]
    fn real_new_witness_frontiers_exact_short_zero_floor_sticky_and_drop_are_not_layout_guesses() {
        let mut seen = [false; 4];
        for source in sources() {
            let checked = crate::check(&source, "witness-frontiers.spx").unwrap();
            let (success, metrics, refused, paid, receipt, full) =
                run_public(&checked, &source, 64 << 20, 0);
            assert!(
                success && !refused && receipt.is_none() && !metrics.invalid && metrics.live == 0,
                "source_bytes={} success={success} refused={refused} paid={paid} first={receipt:?} frame={full:?} allocations={metrics:?}",
                source.len(),
            );
            for (i, frontier) in [
                full.first_join,
                full.first_variant,
                full.first_arm,
                full.first_finish,
            ]
            .into_iter()
            .enumerate()
            {
                let Some(frontier) = frontier else {
                    continue;
                };
                seen[i] = true;
                let before = (64usize << 20).checked_sub(frontier.remaining).unwrap();
                let exact = before.checked_add(frontier.amount).unwrap();
                for (limit, floor, target_acquired) in
                    [(exact, 0, true), (exact - 1, 0, false), (exact, 1, false)]
                {
                    let (success, metrics, refused, _, receipt, prefix) =
                        run_public(&checked, &source, limit, floor);
                    assert!(
                        !metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed
                    );
                    let acquired = [
                        prefix.joins,
                        prefix.variant_views,
                        prefix.arm_views,
                        prefix.finishes,
                    ][i];
                    assert_eq!(acquired != 0, target_acquired);
                    if !target_acquired {
                        assert!(refused && !success);
                        assert_eq!(receipt.unwrap().requested, frontier.amount);
                    } else if success {
                        assert!(!refused && receipt.is_none());
                    } else {
                        assert!(refused && receipt.is_some());
                    }
                }
            }
        }
        assert!(
            seen.into_iter().all(|seen| seen),
            "every actual new role must be reached"
        );
        let checked = crate::check(SCALAR, "witness-zero.spx").unwrap();
        let (success, metrics, refused, _, receipt, _) = run_public(&checked, SCALAR, 0, 0);
        assert!(!success && refused && receipt.is_some() && !metrics.invalid && metrics.live == 0);
    }

    #[test]
    fn complete_public_caller_reports_real_backing_replacement_bill_and_drop_for_matched_sources() {
        for source in sources() {
            let checked = crate::check(&source, "matched-frame.spx").unwrap();
            let (outcome, metrics, refused, paid, receipt, report) =
                run_public(&checked, &source, 64 << 20, 0);
            assert!(
                outcome && !refused && receipt.is_none(),
                "source_bytes={} success={outcome} refused={refused} paid={paid} first={receipt:?} frame={report:?} allocations={metrics:?}",
                source.len(),
            );
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
            assert!(!report.overflow && report.size > 0 && report.requests > 0);
            assert_eq!(report.requested_capacity, report.acquired_capacity);
            let backing = report.acquired_capacity.checked_mul(report.size).unwrap();
            let replacement = report
                .joins
                .checked_mul(WITNESS_JOIN)
                .unwrap()
                .checked_add(report.variant_views.checked_mul(VARIANT_VIEWS).unwrap())
                .unwrap()
                .checked_add(report.arm_views.checked_mul(VARIANT_ARM_VIEW).unwrap())
                .unwrap()
                .checked_add(report.finishes.checked_mul(WITNESS_FINISH).unwrap())
                .unwrap();
            eprintln!(
                "OPT904 full-public-caller source_sha={} source_bytes={} complete=true layout={report:?} full_frame_backing={backing} actual_replacement_bill={replacement} paid={paid} allocations={metrics:?}; no inferred old layout, no clone payload credit, no shipping/app-fit claim",
                {
                    use sha2::Digest;
                    format!(
                        "{:x}",
                        crate::digest_hex::LowerHex(sha2::Sha256::digest(source.as_bytes()))
                    )
                },
                source.len()
            );
            let source_sha256 = {
                use sha2::Digest;
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(sha2::Sha256::digest(source.as_bytes()))
                )
            };
            eprintln!(
                "OPT904_JSON {}",
                serde_json::json!({
                    "source_sha256": source_sha256,
                    "source_bytes": source.len(), "complete": true,
                    "frame_size": report.size, "frame_align": report.alignment,
                    "requested_capacity": report.requested_capacity,
                    "acquired_capacity": report.acquired_capacity, "requests": report.requests,
                    "replacement_bill": replacement, "paid": paid,
                    "joins": report.joins, "variant_views": report.variant_views,
                    "arm_views": report.arm_views, "finishes": report.finishes,
                    "live": metrics.live, "allocated": metrics.allocated, "freed": metrics.freed,
                    "invalid": metrics.invalid, "overflow": report.overflow
                })
            );
        }
    }

    #[test]
    fn real_complete_frame_layout_and_every_capacity_request_are_reported_without_248_assumption() {
        // Missing registration is explicitly incomplete; a foreign Storage<T>
        // cannot contribute capacities to the actual registered frame type.
        let capture = Capture::begin();
        observe_frame_request::<usize>(1);
        assert!(capture.finish().overflow);
        let capture = Capture::begin();
        observe_frame_layout::<(usize, usize)>(
            std::mem::size_of::<(usize, usize)>(),
            std::mem::align_of::<(usize, usize)>(),
        );
        observe_frame_request::<usize>(7);
        observe_frame_acquired::<usize>(7);
        observe_frame_request::<(usize, usize)>(3);
        observe_frame_acquired::<(usize, usize)>(3);
        let registration = capture.finish();
        assert!(!registration.overflow);
        assert_eq!(registration.requests, 1);
        assert_eq!(registration.requested_capacity, 3);
        assert_eq!(registration.acquired_capacity, 3);
        for source in sources() {
            let checked = crate::check(&source, "frame-invoice.spx").unwrap();
            let capture = Capture::begin();
            let program = crate::hir::resolve(&checked).unwrap();
            crate::hir::validate(&program).unwrap();
            let report = capture.finish();
            assert!(!report.overflow);
            assert!(report.size > 0 && report.alignment.is_power_of_two());
            assert_eq!(report.size % report.alignment, 0);
            assert!(report.requests > 0 && report.requested_capacity > 0);
            let backing = report.requested_capacity.checked_mul(report.size).unwrap();
            eprintln!(
                "OPT904 actual_complete_frame size={} alignment={} ALL_requested_capacities={} requests={} full_backing={} includes_failed_requests=true no_248_or_clone_payload_credit",
                report.size, report.alignment, report.requested_capacity, report.requests, backing
            );
        }
    }
    #[test]
    fn actual_live_witness_unwind_and_counter_overflow_preserve_full_owner_drop() {
        let checked = crate::check(SCALAR, "witness-unwind.spx").unwrap();
        let capture = Capture::begin();
        PANIC_AFTER_WITNESS.with(|flag| flag.set(true));
        let (((), metrics), refused, _, receipt) =
            crate::bounded_output::with_limit_usage_refusal(64 << 20, || {
                crate::test_allocation_observer::measure_with_capacity(1 << 16, || {
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let resolved = crate::hir::resolve(&checked).unwrap();
                        drop(resolved);
                    }));
                    assert!(caught.is_err());
                    drop(caught);
                })
            });
        let report = capture.finish();
        assert!(!refused && receipt.is_none() && report.joins > 0);
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        let ordinary = crate::hir::resolve(&checked).unwrap();
        let capture = Capture::begin();
        FRAME_REQUESTS.with(|cell| {
            let mut report = cell.get().unwrap();
            report.requested_capacity = usize::MAX;
            cell.set(Some(report));
        });
        let observed = crate::hir::resolve(&checked).unwrap();
        let overflow = capture.finish();
        assert!(overflow.overflow);
        assert_eq!(
            format!("{ordinary:?}"),
            format!("{observed:?}"),
            "overflow is diagnostic, never authority"
        );
    }

    #[test]
    fn complete_public_cold_sequential_and_nested_match_callers_report_actual_frame_and_replacement_bills(
    ) {
        let mut cases = vec![
            ("literal", 0usize, "module cold; @id(\"cold.main\") fn main()->i64 {1}".to_owned()),
            ("call", 0usize, "module cold; @id(\"cold.f\") fn f(x:i64)->i64 {x} @id(\"cold.main\") fn main()->i64 {f(1)}".to_owned()),
        ];
        for count in [1usize, 8, 32] {
            let mut body = String::new();
            for index in 0..count {
                body.push_str(&format!("let n{index}=match 0 {{_=>1,}};"));
            }
            body.push_str("1");
            cases.push((
                "sequential",
                count,
                format!("module sequential; @id(\"sequence.main\") fn main()->i64 {{{body}}}"),
            ));
            let mut body = "1".to_owned();
            for _ in 0..count {
                body = format!("match 0 {{_=>{body},}}");
            }
            cases.push((
                "nested",
                count,
                format!("module nested; @id(\"nested.main\") fn main()->i64 {{{body}}}"),
            ));
        }
        // A single exhaustive arm keeps one PLAN path. Sequential matches grow
        // witness joins without introducing an exponential path-product fixture.
        for (shape, count, source) in cases {
            let checked = crate::check(&source, "matched-frame-shape.spx").unwrap();
            let (success, metrics, refused, paid, receipt, report) =
                run_public(&checked, &source, 64 << 20, 0);
            let replacement = report
                .joins
                .checked_mul(WITNESS_JOIN)
                .unwrap()
                .checked_add(report.variant_views.checked_mul(VARIANT_VIEWS).unwrap())
                .unwrap()
                .checked_add(report.arm_views.checked_mul(VARIANT_ARM_VIEW).unwrap())
                .unwrap()
                .checked_add(report.finishes.checked_mul(WITNESS_FINISH).unwrap())
                .unwrap();
            let source_sha256 = {
                use sha2::Digest;
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(sha2::Sha256::digest(source.as_bytes()))
                )
            };
            eprintln!(
                "OPT904_SHAPE_JSON {}",
                serde_json::json!({
                    "source_sha256": source_sha256,
                    "source_bytes": source.len(), "shape": shape, "match_count": count,
                    "success": success, "refused": refused, "first_refusal": format!("{receipt:?}"),
                    "limit": 64usize << 20, "floor": 0, "paid": paid,
                    "frame_size": report.size, "frame_align": report.alignment,
                    "requested_capacity": report.requested_capacity, "acquired_capacity": report.acquired_capacity,
                    "requests": report.requests, "joins": report.joins, "variant_views": report.variant_views,
                    "arm_views": report.arm_views, "finishes": report.finishes,
                    "replacement_bill": replacement, "overflow": report.overflow,
                    "allocated": metrics.allocated, "freed": metrics.freed, "live": metrics.live, "invalid": metrics.invalid,
                    "matched_old_image_required": true, "no_clone_payload_credit": true
                })
            );
            assert!(
                success && !refused && receipt.is_none(),
                "complete unchanged-capacity caller {shape}/{count}"
            );
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
            assert!(!report.overflow && report.size > 0 && report.alignment.is_power_of_two());
            assert_eq!(report.requested_capacity, report.acquired_capacity);
        }
    }

    #[test]
    fn whole_immediate_production_frame_dispatch_helper_and_invoice_restore_and_refuse_drift() {
        use sha2::{Digest, Sha256};
        fn hash(bytes: &[u8]) -> String {
            format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
        }
        fn restore(row: &serde_json::Value, current: &[u8]) -> Option<Vec<u8>> {
            if current.len() as u64 != row["current_length"].as_u64()?
                || hash(current) != row["current_sha256"].as_str()?
            {
                return None;
            }
            let mut bytes = current.to_vec();
            for edit in row["edits"].as_array()?.iter().rev() {
                let start = usize::try_from(edit["current_start"].as_u64()?).ok()?;
                let end = usize::try_from(edit["current_end"].as_u64()?).ok()?;
                if bytes.get(start..end)? != edit["current"].as_str()?.as_bytes() {
                    return None;
                }
                bytes.splice(
                    start..end,
                    edit["before"].as_str()?.as_bytes().iter().copied(),
                );
            }
            Some(bytes)
        }
        let inverse: serde_json::Value =
            serde_json::from_str(include_str!("match_witness_inverse.json")).unwrap();
        let original_root = frame_reuse::source_inverse::whole_root(
            "src/hir/validation.rs",
            include_str!("../validation.rs"),
        );
        let original_lexical = frame_reuse::source_inverse::whole_root(
            "src/hir/validation/lexical_exit.rs",
            include_str!("lexical_exit.rs"),
        );
        let sources = [
            original_root.as_str(),
            original_lexical.as_str(),
            include_str!("scope_checkpoint/storage.rs"),
        ];
        let before = [
            (
                399157,
                "85a730c701db6b3031fecd84499aa1f2dff5f5c07d0498612bdcd1284dd0b271",
            ),
            (
                5536,
                "5b1655ec915ba15347dcaaea9914a4f53ee3e922df4e32e424fc3d53775ea869",
            ),
            (
                4073,
                "b6bdc77ba0bbacf17d7481a0cf5028ed17c795e5f44b361079fc4d1d31e6d64f",
            ),
        ];
        assert_eq!(inverse["rows"].as_array().unwrap().len(), sources.len());
        for ((row, current), (length, expected)) in inverse["rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(sources)
            .zip(before)
        {
            let old = restore(row, current.as_bytes()).unwrap();
            assert_eq!((old.len(), hash(&old)), (length, expected.to_owned()));
            assert!(
                restore(row, &old).is_none(),
                "already-old is not current authority"
            );
            let mut drift = current.as_bytes().to_vec();
            drift[0] ^= 1;
            assert!(restore(row, &drift).is_none());
        }
        let frame = include_str!("continuation_frame.rs")
            .split_once("pub(super) enum Frame<'e> {")
            .unwrap()
            .1;
        assert!(
            !frame.contains("#[cfg"),
            "instrumentation cannot change shipping frame layout"
        );
        assert_eq!(frame.matches("result: Option<&'e ResolvedExpr>").count(), 5);
        let storage = sources[2]
            .split_once("struct Storage<T> {")
            .unwrap()
            .1
            .split_once('}')
            .unwrap()
            .0;
        assert_eq!(storage, "\n    values: Vec<T>,\n");
    }
    #[test]
    fn invocation_reuse_unwinds_live_scopes_and_witness_before_publication_drop() {
        let checked = crate::check(SCALAR, "reuse-witness-unwind.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "w.f")
            .unwrap();
        let validator = HirValidator::new(&program).unwrap();
        let capture = Capture::begin();
        let (((), metrics), refused, _, receipt) =
            crate::bounded_output::with_limit_usage_refusal(64 << 20, || {
                crate::test_allocation_observer::measure_with_capacity(1 << 16, || {
                    PANIC_AFTER_WITNESS.with(|flag| flag.set(true));
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut validator = validator;
                        let mut scratch = frame_reuse::Scratch::new().unwrap();
                        frame_reuse::argument().unwrap();
                        validator
                            .validate_function_with_frames(
                                function,
                                &FunctionExecutionId::Monomorphic(function.id.clone()),
                                Some(&mut scratch),
                            )
                            .unwrap();
                    }));
                    assert!(caught.is_err());
                    drop(caught);
                    PANIC_AFTER_WITNESS.with(|flag| flag.set(false));
                })
            });
        let report = capture.finish();
        assert!(!refused && receipt.is_none() && report.joins > 0 && !report.overflow);
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        crate::hir::validate(&program).unwrap();
    }
}

#[cfg(test)]
#[path = "lexical_exit/arm_context_tests.rs"]
pub(super) mod arm_context_tests;
