//! Explicit-payer funded twin of `checked_bindings`. The census walk, admission
//! verdict and ordering are the ordinary ones (same private seam); the census
//! cursor, proof cursor and every entry-vector growth are debited on the
//! caller's payer first. The payer's refusal is returned unchanged and nothing
//! reads the production ACTIVE ledger.
//!
//! Two independent callbacks: `reserve` pays physical BYTES; `work` observes
//! admission WORK UNITS (census loop/statement/insertion units and every proof
//! unit, in the order the seam debits them). Units are never converted to bytes
//! and neither callback replaces the ordinary ceilings: `WORK`, `DEPTH`,
//! `CENSUS_DEPTH` and each proof call's own budget run unchanged inside the
//! shared seam, so a `work` that always succeeds cannot buy extra admission.
//! A refusal from either callback is returned as `Refused` and is never read
//! as a census verdict.
use super::*;
#[cfg(test)]
use crate::hir::funded;
use crate::hir::funded::{FundedError, Reserve};

struct Funding<'r, 'w, E> {
    reserve: Reserve<'r, E>,
    work: &'w mut dyn FnMut(usize) -> Result<(), E>,
    refused: Option<E>,
    /// The entry vector's own layout/allocation failed (seam notification).
    physical: bool,
}

impl<E> Funding<'_, '_, E> {
    fn pay(&mut self, bytes: usize) -> bool {
        match (self.reserve)(bytes) {
            Ok(()) => true,
            Err(error) => {
                self.refused.get_or_insert(error);
                false
            }
        }
    }

    /// Report work units; the seam then fails with an allocation-free census
    /// carrier, which `refusal` discards in favour of the recorded refusal.
    fn units(&mut self, units: usize) -> Result<(), crate::diagnostic::Diagnostic> {
        match (self.work)(units) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.refused.get_or_insert(error);
                Err(census_static())
            }
        }
    }
}

/// Inline control carrier with the six fields of the owning `census_error`.
/// Every funded diagnostic is discarded by `refusal`, so nothing owned is ever
/// built on this route and no unpaid backing exists; the ordinary and selected
/// routes keep the original owning constructor.
fn census_static() -> crate::diagnostic::Diagnostic {
    crate::diagnostic::Diagnostic::io_static(CENSUS_CODE, CENSUS_MESSAGE)
}

#[cfg(test)]
pub(crate) fn checked_bindings_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<HelperBindings<'a>, FundedError<E>> {
    checked_bindings_funded_with_work(program, function, reserve, &mut |_| Ok(()))
}

pub(crate) fn checked_bindings_funded_with_work<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    reserve: Reserve<'_, E>,
    work: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<HelperBindings<'a>, FundedError<E>> {
    let mut funding = Funding {
        reserve,
        work,
        refused: None,
        physical: false,
    };
    if !funding.pay(0) {
        return Err(refusal(funding));
    }
    if !potential(program) {
        return Ok(HelperBindings::default());
    }
    if !funding.pay(std::mem::size_of::<CensusCursor<'_>>()) {
        return Err(refusal(funding));
    }
    let mut cursor = CensusCursor {
        frames: [(&function.body, 0, 0); CENSUS_DEPTH],
    };
    let mut proof: Option<ProofCursor<'a>> = None;
    let outcome = checked_bindings_with_storage(
        function,
        &mut cursor,
        &mut funding,
        |binding, value, funding| {
            let ResolvedExprKind::Call {
                callee,
                instance: None,
                ..
            } = &value.kind
            else {
                return Ok(false);
            };
            let Some(target) = program.resolve_call_target(callee, None) else {
                return Ok(false);
            };
            if target.return_type != ResolvedType::Bytes
                || target
                    .params
                    .iter()
                    .filter(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
                    .count()
                    != 1
                || !target
                    .params
                    .iter()
                    .all(|p| resolved_param(program, p.ownership, &p.ty))
            {
                return Ok(false);
            }
            if proof.is_none() {
                if !funding.pay(std::mem::size_of::<Option<ProofCursor<'_>>>()) {
                    return Err(census_static());
                }
                proof = Some(ProofCursor {
                    frames: [None; DEPTH],
                });
            }
            let proved = nonallocating_with_cursor_and_work(
                target,
                |id, instance| program.resolve_call_target(id, instance),
                |_, mode, ty| resolved_param(program, mode, ty),
                |_, ty| {
                    crate::hir::is_scalar_resolved_type(ty)
                        || crate::hir::copy_record_collection::admitted(program.declarations(), ty)
                        || crate::loop_calls::resolved_match_scrutinee_admitted(
                            program.declarations(),
                            ty,
                        )
                },
                program
                    .functions()
                    .len()
                    .saturating_add(program.function_instances().len()),
                proof.as_mut().ok_or_else(census_static)?,
                &mut |units| funding.units(units),
            )?;
            Ok(assignment_with_target(
                program, binding, value, target, proved,
            ))
        },
        |bytes, funding| funding.pay(bytes),
        |units, funding| funding.units(units),
        CensusRefusal {
            physical: |funding: &mut Funding<'_, '_, E>| funding.physical = true,
            error: census_static,
        },
    );
    match outcome {
        Ok(found) => Ok(found),
        Err(_) => Err(refusal(funding)),
    }
}

/// A recorded payer/work refusal wins, then a physical failure of the entry
/// vector; only the remainder is the original work/depth census verdict.
fn refusal<E>(funding: Funding<'_, '_, E>) -> FundedError<E> {
    match (funding.refused, funding.physical) {
        (Some(error), _) => FundedError::Refused(error),
        (None, true) => FundedError::Layout("byte helper census entry vector failed physically"),
        (None, false) => {
            FundedError::Census("byte helper census exceeds admitted scratch, work or depth")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::funded::payer::assert_every_boundary;

    const SOURCE: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");

    fn normalize(sites: HelperBindings<'_>) -> Vec<(String, String)> {
        sites
            .into_iter()
            .map(|(at, binding)| (at.as_str().to_owned(), binding.id.as_str().to_owned()))
            .collect()
    }

    #[test]
    fn funded_helper_census_matches_the_original_and_refuses_at_every_boundary() {
        let checked = crate::check(SOURCE, "funded-helper-census.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut sites = 0;
        for function in &program.functions {
            let old = normalize(checked_bindings(&program, function).unwrap());
            sites += old.len();
            let (value, calls, charged) = assert_every_boundary(|reserve| {
                Ok(normalize(checked_bindings_funded(
                    &program, function, reserve,
                )?))
            });
            assert_eq!(value, old, "{}", function.id.as_str());
            assert!(calls > 0 && (old.is_empty() || charged > 0));
        }
        assert!(sites >= 2, "the helper corpus must publish census sites");
    }

    /// `main` of the helper corpus with its tail wrapped deeper than the census
    /// admits; the program stays the helper corpus so `potential` holds.
    fn too_deep(program: &crate::hir::ResolvedProgram) -> ResolvedFunction {
        let mut deep = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
            .clone();
        let ResolvedExprKind::Block { tail, .. } = &mut deep.body.kind else {
            panic!("main block")
        };
        for _ in 0..CENSUS_DEPTH + 2 {
            let inner = tail.clone();
            tail.kind = ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: inner,
            };
        }
        deep
    }

    #[test]
    fn census_depth_verdict_is_semantic_while_every_payer_and_work_refusal_propagates() {
        let checked = crate::check(SOURCE, "funded-census-depth.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let deep = too_deep(&program);
        // Old normal receiver: the verdict is the census diagnostic.
        match checked_bindings(&program, &deep) {
            Err(error) => assert_eq!(error.code, census_error().code),
            Ok(_) => panic!("the original census must refuse the too-deep body"),
        }
        let mut payer = funded::payer::Payer::unlimited();
        assert!(matches!(
            checked_bindings_funded(&program, &deep, &mut |bytes| payer.pay(bytes)),
            Err(FundedError::Census(_))
        ));
        let calls = payer.calls;
        assert!(calls > 0);
        for ordinal in 0..calls {
            let mut scripted = funded::payer::Payer::refusing_at(ordinal);
            assert!(matches!(
                checked_bindings_funded(&program, &deep, &mut |bytes| scripted.pay(bytes)),
                Err(FundedError::Refused(funded::payer::PayerRefusal::Ordinal(at))) if at == ordinal
            ));
        }
        // The work callback counts units apart from bytes; its refusal at any
        // unit comes back unchanged and ends the walk at that unit.
        let mut units = Vec::new();
        let mut bytes = funded::payer::Payer::unlimited();
        assert!(matches!(
            checked_bindings_funded_with_work(&program, &deep, &mut |b| bytes.pay(b), &mut |u| {
                units.push(u);
                Ok(())
            }),
            Err(FundedError::Census(_))
        ));
        assert!(units.len() > CENSUS_DEPTH, "one census unit per node");
        assert!(bytes.charged >= std::mem::size_of::<CensusCursor<'_>>());
        for at in [0, 1, units.len() / 2, units.len() - 1] {
            let mut seen = 0usize;
            let mut payer = funded::payer::Payer::unlimited();
            let refused = checked_bindings_funded_with_work(
                &program,
                &deep,
                &mut |b| payer.pay(b),
                &mut |_| {
                    seen += 1;
                    if seen == at + 1 {
                        Err(funded::payer::PayerRefusal::Ordinal(at))
                    } else {
                        Ok(())
                    }
                },
            );
            assert!(
                matches!(refused, Err(FundedError::Refused(funded::payer::PayerRefusal::Ordinal(n))) if n == at)
            );
            assert_eq!(seen, at + 1);
        }
    }

    #[test]
    fn work_callback_sees_every_proof_and_census_unit_without_changing_the_verdict() {
        let checked = crate::check(SOURCE, "funded-census-work.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut proofs_seen = 0usize;
        for function in &program.functions {
            let old = normalize(checked_bindings(&program, function).unwrap());
            let mut total = 0usize;
            let mut payer = funded::payer::Payer::unlimited();
            let funded_sites = checked_bindings_funded_with_work(
                &program,
                function,
                &mut |b| payer.pay(b),
                &mut |u| {
                    total += u;
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(normalize(funded_sites), old, "{}", function.id.as_str());
            assert!(
                total <= WORK,
                "census work stays under the ordinary ceiling"
            );
            proofs_seen += usize::from(total > 0);
        }
        assert!(proofs_seen > 0);
    }

    #[test]
    fn static_census_carrier_has_the_six_original_fields_and_stays_borrowed() {
        let owning = census_error();
        let carrier = census_static();
        assert_eq!(carrier, owning);
        assert_eq!(carrier.code, owning.code);
        assert_eq!(carrier.severity, owning.severity);
        assert_eq!(carrier.message, owning.message);
        assert_eq!(carrier.path, owning.path);
        assert_eq!(carrier.span, owning.span);
        assert_eq!(carrier.help, owning.help);
        assert!(matches!(carrier.message, std::borrow::Cow::Borrowed(_)));
        assert!(matches!(owning.message, std::borrow::Cow::Owned(_)));
    }

    #[test]
    fn work_refusal_first_wins_over_the_census_verdict_it_triggers() {
        let checked = crate::check(SOURCE, "funded-census-order.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let main = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        // First unit refused: the helper walk stops there and reports that
        // refusal, not the carrier the seam returned to unwind.
        let mut payer = funded::payer::Payer::unlimited();
        let mut calls = 0usize;
        let refused =
            checked_bindings_funded_with_work(&program, main, &mut |b| payer.pay(b), &mut |_| {
                calls += 1;
                Err(funded::payer::PayerRefusal::Ordinal(calls))
            });
        assert!(matches!(
            refused,
            Err(FundedError::Refused(funded::payer::PayerRefusal::Ordinal(
                1
            )))
        ));
        assert_eq!(calls, 1, "the seam stops at the first refused unit");
        // A payer refusal at every ordinal is returned unchanged and ahead of
        // any census classification, including ordinals past the proof cursor.
        let mut counting = funded::payer::Payer::unlimited();
        checked_bindings_funded(&program, main, &mut |b| counting.pay(b)).unwrap();
        for ordinal in 0..counting.calls {
            let mut scripted = funded::payer::Payer::refusing_at(ordinal);
            assert!(matches!(
                checked_bindings_funded(&program, main, &mut |b| scripted.pay(b)),
                Err(FundedError::Refused(funded::payer::PayerRefusal::Ordinal(at))) if at == ordinal
            ));
        }
    }

    #[test]
    fn verdict_classifier_keeps_refusal_physical_and_census_distinct() {
        let mut pay = |_: usize| -> Result<(), u8> { Ok(()) };
        let mut work = |_: usize| -> Result<(), u8> { Ok(()) };
        for (refused, physical) in [
            (Some(7u8), false),
            (Some(7), true),
            (None, true),
            (None, false),
        ] {
            let funding = Funding {
                reserve: &mut pay,
                work: &mut work,
                refused,
                physical,
            };
            match (refused, physical) {
                (Some(error), _) => assert_eq!(refusal(funding), FundedError::Refused(error)),
                (None, true) => assert!(matches!(refusal(funding), FundedError::Layout(_))),
                (None, false) => assert!(matches!(refusal(funding), FundedError::Census(_))),
            }
        }
    }

    #[test]
    fn funded_helper_census_without_a_writer_signature_charges_only_authentication() {
        let source = "module old; @id(\"old.main\") fn main()->i64 {42}";
        let checked = crate::check(source, "funded-no-helper.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let (value, calls, charged) = assert_every_boundary(|reserve| {
            Ok(normalize(checked_bindings_funded(
                &program,
                &program.functions[0],
                reserve,
            )?))
        });
        assert!(value.is_empty());
        assert_eq!((calls, charged), (1, 0));
    }

    #[test]
    fn funded_helper_census_never_touches_the_production_ledger() {
        let checked = crate::check(SOURCE, "funded-helper-active.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let main = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let (_, refused, active) = crate::bounded_output::with_limit_usage(0, || {
            let mut payer = funded::payer::Payer::unlimited();
            checked_bindings_funded(&program, main, &mut |bytes| payer.pay(bytes))
                .map(normalize)
                .unwrap()
        });
        assert!(!refused);
        assert_eq!(active, 0);
    }
}
