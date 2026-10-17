use super::undo;
#[test]
fn complete_original_sources_and_compiled_owning_receivers_remain_exact() {
    {
        let mut current = crate::source_reflows_merge::current344::project(
            "src/hir/function_value/funded.rs",
            include_str!("../../../../hir/function_value/funded.rs"),
        );
        undo(
            &mut current,
            r#"        }
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        }
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__function_value__funded.rs.before"
            ),
            "src/hir/function_value/funded.rs"
        );
    }
    {
        let mut current = super::current_view::project("src/hir/funded.rs", include_str!("../../../../hir/funded.rs"));
        undo(
            &mut current,
            r#"        assert!(fresh.is_empty(), "a refused charge publishes nothing");
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        assert!(fresh.is_empty(), "a refused charge publishes nothing");
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__funded.rs.before"
            ),
            "src/hir/funded.rs"
        );
    }
    {
        let mut current = crate::source_reflows_merge::current344::project("src/hir/ids.rs", include_str!("../../../../hir/ids.rs"));
        undo(
            &mut current,
            r#"mod metered_owner;
#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;

#[cfg(test)]
pub(crate) use metered_owner::general_matches as oracle_owner_matches_with_work;
"#,
            r#"mod metered_owner;
#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;
"#,
        );
        // Exact receiving edits; the complete original source remains the final oracle.
        undo(&mut current, r###"#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;

#[cfg(test)]
pub(in crate::hir) mod fixed_operation_tests;
"###, r###"#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;
"###);
        assert_eq!(
            current,
            include_str!("../../../../../docs/evidence/opt810-live-oracle/src__hir__ids.rs.before"),
            "src/hir/ids.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/ids/metered_owner.rs").to_owned();
        undo(
            &mut current,
            r#"        }
    }
}

mod oracle;
pub(crate) use oracle::matches as general_matches;
"#,
            r#"        }
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__ids__metered_owner.rs.before"
            ),
            "src/hir/ids/metered_owner.rs"
        );
    }
    {
        let mut current = super::current_view::before_production_promotion("src/hir/lexical_exit.rs", include_str!("../../../../hir/lexical_exit.rs"));
        undo(&mut current,"pub(crate) use selected_child::{requires_inline,validate_owner_inline};","pub(crate) use selected_child::requires_inline;");
        undo(&mut current,"mod latch;\nmod latch_inline;\npub(crate) use latch_inline::validate_latch_inline;","mod latch;");
        undo(&mut current,"mod walk;\nmod selected_child;\npub(crate) use selected_child::requires_inline;","mod walk;");
        undo(
            &mut current,
            r#"    }
    visit(expr, None, 0) == RETURN
}

#[cfg(test)]
pub(crate) use owner_funded::{owner_funded_with_work, requires_funded_with_work};
"#,
            r#"    }
    visit(expr, None, 0) == RETURN
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__lexical_exit.rs.before"
            ),
            "src/hir/lexical_exit.rs"
        );
    }
    {
        let mut current = super::current_view::project("src/hir/lexical_exit/owner_funded.rs", include_str!("../../../../hir/lexical_exit/owner_funded.rs"));
        // Exact #766 test-only extension: retain the whole original plain
        // receiver and all previous source inverses; no production bytes change.
        undo(&mut current, r#####"

        // Keep the complete plain-owner control above. Exercise the actual
        // terminated guard and its already-admitted unterminated receiver with
        // the separate existing work callback as well as the byte payer.
        #[derive(Debug)]
        struct Limit {
            remaining: usize,
            used: usize,
            refusal: Option<&'static str>,
            label: &'static str,
        }
        impl Limit {
            fn new(remaining: usize, label: &'static str) -> Self {
                Self {
                    remaining,
                    used: 0,
                    refusal: None,
                    label,
                }
            }
            fn charge(&mut self, amount: usize) -> Result<(), &'static str> {
                if let Some(first) = self.refusal {
                    return Err(first);
                }
                let Some(remaining) = self.remaining.checked_sub(amount) else {
                    self.refusal = Some(self.label);
                    return Err(self.label);
                };
                self.remaining = remaining;
                self.used = self.used.checked_add(amount).expect("bounded test debit");
                Ok(())
            }
        }
        let mut debit_pairs = Vec::new();
        for terminator in ["", ";"] {
            let source = format!("module guard.bounds; @id(\"main\") fn main()->i64 {{ let mut i=0; while i<3 {{ i=i+1; if i==2 {{continue;}} else {{0}}{terminator} i=i+0; 0 }} i }}");
            let checked = crate::check(&source, "guard-bounds.spx").unwrap();
            let program = crate::hir::resolve(&checked).unwrap();
            let function = &program.functions[0];
            let original_owner = super::super::owner(&program, function).unwrap();
            let original_requires = super::super::requires(function).unwrap();
            assert!(original_requires, "real lexical guard required");
            let run = |bytes: &mut Limit, work: &mut Limit| {
                let mut reserve = |amount| bytes.charge(amount);
                let mut visit = |amount| work.charge(amount);
                let owner = owner_funded_with_work(&program, function, &mut reserve, &mut visit)?;
                let required = requires_funded_with_work(function, &mut reserve, &mut visit)?;
                Ok::<_, FundedError<&'static str>>((owner, required))
            };
            // Derive exact cumulative charges from the real complete receiver;
            // no guessed quota or production allowance is introduced.
            let mut bytes = Limit::new(usize::MAX, "byte refusal");
            let mut work = Limit::new(usize::MAX, "work refusal");
            assert_eq!(
                run(&mut bytes, &mut work),
                Ok((Ok(original_owner.clone()), original_requires))
            );
            assert!(bytes.used > 0 && work.used > 0);
            let exact = (bytes.used, work.used);
            debit_pairs.push(exact);
            let mut bytes = Limit::new(exact.0, "byte refusal");
            let mut work = Limit::new(exact.1, "work refusal");
            let ((), observed) = crate::test_allocation_observer::measure(|| {
                let result = run(&mut bytes, &mut work);
                assert_eq!(result, Ok((Ok(original_owner.clone()), original_requires)));
                // The receiving execution ID drops inside the observation.
                drop(result);
            });
            assert!(!observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
            assert!(observed.allocated <= bytes.used && observed.highwater <= bytes.used);
            assert_eq!((bytes.remaining, work.remaining), (0, 0));
            assert_eq!((bytes.used, work.used), exact);
            assert!(bytes.refusal.is_none() && work.refusal.is_none());
            for (byte_limit, work_limit, selected) in [
                (0, exact.1, "byte refusal"),
                (exact.0 - 1, exact.1, "byte refusal"),
                (exact.0, 0, "work refusal"),
                (exact.0, exact.1 - 1, "work refusal"),
            ] {
                let mut bytes = Limit::new(byte_limit, "byte refusal");
                let mut work = Limit::new(work_limit, "work refusal");
                let ((), observed) = crate::test_allocation_observer::measure(|| {
                    let result = run(&mut bytes, &mut work);
                    assert_eq!(result, Err(FundedError::Refused(selected)));
                    drop(result);
                });
                assert!(!observed.invalid && observed.live == 0);
                assert_eq!(observed.allocated, observed.freed);
                assert!(observed.allocated <= bytes.used && observed.highwater <= bytes.used);
                let used = (bytes.used, work.used);
                if selected == "byte refusal" {
                    assert_eq!(bytes.charge(0), Err(selected));
                } else {
                    assert_eq!(work.charge(0), Err(selected));
                }
                assert_eq!(
                    (bytes.used, work.used),
                    used,
                    "sticky refusal never resets or refunds"
                );
                assert_eq!(
                    super::super::owner(&program, function).unwrap(),
                    original_owner
                );
                assert_eq!(super::super::requires(function).unwrap(), original_requires);
            }
        }
        assert_eq!(
            debit_pairs[0], debit_pairs[1],
            "same canonical guard has identical charged work and storage"
        );"#####, "");
        undo(
            &mut current,
            r#"        );
    }
}

mod work;
pub(crate) use work::{owner_funded_with_work, requires_funded_with_work};
"#,
            r#"        );
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__lexical_exit__owner_funded.rs.before"
            ),
            "src/hir/lexical_exit/owner_funded.rs"
        );
    }
    {
        let mut current = super::current_view::project("src/hir/lexical_exit/walk.rs", include_str!("../../../../hir/lexical_exit/walk.rs"));
        undo(
            &mut current,
            r#"
#[cfg(test)]
mod tests;

impl<'a> ChildCursor<'a> {
    #[cfg(test)]
    pub(crate) fn next_with_work<E>(
        &mut self,
        work: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<&'a ResolvedExpr>, E> {
        work(1)?;
        let first = !self.started;
        self.started = true;
        Ok(match (&self.expression.kind, &mut self.position) {
            (
                K::Block { statements, tail }
                | K::VecConstructor {
                    statements, tail, ..
                },
                Position::Block {
                    statement,
                    child: index,
                },
            ) => {
                loop {
                    work(1)?;
                    let Some(item) = statements.get(*statement) else {
                        break;
                    };
                    if *index < item.child_count() {
                        let result = item.child(*index);
                        *index += 1;
                        return Ok(result);
                    }
                    *statement += 1;
                    *index = 0;
                }
                if *index == 0 {
                    *index = 1;
                    Some(tail)
                } else {
                    None
                }
            }
            (
                K::Match {
                    scrutinee, arms, ..
                },
                Position::Match { arm, guard },
            ) => {
                if first {
                    return Ok(Some(scrutinee));
                }
                if let Some(item) = arms.get(*arm) {
                    if *guard {
                        *guard = false;
                        if let Some(value) = &item.guard {
                            return Ok(Some(value));
                        }
                    }
                    *arm += 1;
                    *guard = true;
                    return Ok(Some(&item.value));
                }
                None
            }
            (_, Position::Indexed(index)) => {
                let result = child(self.expression, *index);
                if result.is_some() {
                    *index += 1;
                }
                result
            }
            _ => unreachable!("child cursor position matches its immutable expression"),
        })
    }
}
"#,
            r#"
#[cfg(test)]
mod tests;
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__lexical_exit__walk.rs.before"
            ),
            "src/hir/lexical_exit/walk.rs"
        );
    }
    {
        let mut current = super::current_view::project("src/literal_format.rs", include_str!("../../../../literal_format.rs"));
        undo(
            &mut current,
            r#"        }
    }
}

#[cfg(test)]
pub(crate) use funded::work::checked_moving_string_operands_funded_with_work;
"#,
            r#"        }
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__literal_format.rs.before"
            ),
            "src/literal_format.rs"
        );
    }
    {
        let mut current = include_str!("../../../../literal_format/funded.rs").to_owned();
        undo(
            &mut current,
            r#"        assert_eq!(active, 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        assert_eq!(active, 0);
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__literal_format__funded.rs.before"
            ),
            "src/literal_format/funded.rs"
        );
    }
    {
        let mut current =
            include_str!("../../../../project/incremental/cold_source_tests.rs").to_owned();
        undo(&mut current, "#[path = \"cold_source_tests/oracle_owner_before.rs\"]\n", "");
        undo(
            &mut current,
            r#"    );
    same(&changed_warm, &old_changed);
}

mod oracle_owner_before;
"#,
            r#"    );
    same(&changed_warm, &old_changed);
}
"#,
        );
        undo(
            &mut current,
            r#"
#[test]
fn explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse() {
    crate::cleanup_plan::with_financed_test_oracles(
        oracle_owner_before::explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse,
    );
}

#[test]
"#,
            &format!(
                "{}{}{}",
                r#"
#[test]
"#,
                include_str!(
                    "../../../../project/incremental/cold_source_tests/oracle_owner_before.rs"
                )
                .strip_prefix("use super::*;\n\npub(super) ")
                .unwrap()
                .trim_end_matches('\n'),
                r#"

#[test]
"#
            ),
        );
        // Restore the complete original child before applying every prior parent hook.
        undo(&mut current, r####"fn explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse() {
    use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
    use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
    let source = "module cold; @id(\"helper\") fn helper(value:i64)->i64 {value+1} @id(\"main\") fn main()->i64 {helper(1)}";
    let program = crate::check(source, "cold.spx").unwrap();
    let mut pass = pass(source, &program);
    let view = SyntheticProgramView::new(&program, Vec::new()).unwrap();
    let old = pass
        .resolve_functions(&program.path, &program, None)
        .unwrap()
        .unwrap();
    // Each full resolution is one module route: it acquires its own owner on
    // its own equivalent real workspace builder ledger. Sharing one arbitrary
    // 1 MiB ledger across both exhausted the independent-replay cursor.
    let (owned, failed, owned_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            pass.resolve_functions_paid(&program.path, &program, None, &mut owner)
                .unwrap()
                .unwrap()
        });
    assert!(!failed);
    assert!(owned_used > 0);
    let (borrowed, failed, borrowed_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            pass.resolve_borrowed_cold_source_paid(&program.path, &view, None, &mut owner)
                .unwrap()
                .unwrap()
        });
    assert!(!failed);
    assert!(borrowed_used > 0);
    // Paid costs are measured on an active ledger, so the legacy route is
    // compared under its own equivalent active ledger. The no-active run keeps
    // its zero-cost compatibility inventory, which is not normalized.
    let (old_active, old_failed, _) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            pass.resolve_functions(&program.path, &program, None)
                .unwrap()
                .unwrap()
        });
    assert!(!old_failed);
    assert_eq!(old_active.resolved, old.resolved);
    assert_eq!(
        old.function_costs.keys().collect::<Vec<_>>(),
        old_active.function_costs.keys().collect::<Vec<_>>()
    );
    assert!(old.function_costs.values().all(|cost| *cost == 0));
    for new in [&owned, &borrowed] {
        assert_eq!(new.resolved, old.resolved);
        assert_eq!(new.function_costs, old_active.function_costs);
        assert_eq!(
            (new.reused_functions, new.static_text),
            (old.reused_functions, old.static_text)
        );
    }
    // Retain the independent pre-nominal identity receiver and its original
    // first identity-backing request on an equivalent real owner.
    let word = std::mem::size_of::<usize>();
    let report =
        std::mem::size_of::<IdentityReservation>() + std::mem::size_of::<Diagnostic>() + 48;
    let (legacy_refused, legacy_failed, _, legacy_receipt) =
        with_limit_usage_refusal(report, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            crate::hir::identity_resolution::resolve_checked(
                &program,
                None,
                None,
                None,
                None,
                |bytes| owner.reserve(bytes),
            )
            .err()
            .unwrap()
        });
    assert!(legacy_failed);
    let legacy_receipt = legacy_receipt.unwrap();
    assert_eq!(
        legacy_receipt.requested,
        (2 * word + 11).div_ceil(word) * word
    );
    assert_eq!(legacy_receipt.remaining, 0);
    assert_eq!(legacy_receipt.floor, 0);
    assert_eq!(legacy_receipt.stage, None);
    assert!(matches!(legacy_refused,
        crate::hir::identity_resolution::Failure::Identity(Refused::Budget(first))
        if first == legacy_receipt));
    // The current receiver first funds its actual enlarged nominal map node,
    // before constructing identities. Refusal still returns the same exact
    // caller-owned Budget receipt and moves no HIR; it invents no allowance.
    let (refused, failed, _, receipt) = with_limit_usage_refusal(report, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        pass.resolve_functions_paid(&program.path, &program, None, &mut owner)
            .unwrap()
            .err()
            .unwrap()
    });
    assert!(failed);
    let receipt = receipt.unwrap();
    assert_eq!(
        receipt.requested,
        crate::cleanup_plan::replay_btree_node_upper::<
            crate::hir::DeclarationId,
            crate::hir::NominalPayload<crate::hir::ResolvedTypeParameterDeclaration>,
        >()
        .expect("actual nominal parameter-map node layout")
    );
    assert_eq!(receipt.remaining, 0);
    assert_eq!(receipt.floor, 0);
    assert_eq!(receipt.stage, None);
    assert!(matches!(refused,
        paid_resolution::SelectiveFailure::Resolve(
            crate::hir::identity_resolution::Failure::Identity(Refused::Budget(first)))
        if first == receipt));
    // Checked body reuse and the first-checked-only borrowed gate are unchanged.
    let saved_helper_cost = owned.function_costs["helper"];
    let _owner = pass.resolved_module(
        &program.path,
        program.clone(),
        owned.resolved,
        0,
        0,
        owned.function_costs,
        owned.reused_functions,
    );
    pass.checked = std::mem::take(&mut pass.next_checked);
    let changed = crate::check(&source.replace("helper(1)", "helper(2)"), "cold.spx").unwrap();
    let legacy = pass
        .resolve_functions(&program.path, &changed, None)
        .unwrap()
        .unwrap();
    let (paid, failed, _) = with_limit_usage(1 << 20, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        let paid = pass
            .resolve_functions_paid(&program.path, &changed, None, &mut owner)
            .unwrap()
            .unwrap();
        assert!(pass
            .resolve_borrowed_cold_source_paid(&program.path, &view, None, &mut owner)
            .is_none());
        paid
    });
    assert!(!failed);
    assert_eq!(paid.reused_functions, 1);
    assert_eq!(paid.reused_functions, legacy.reused_functions);
    assert_eq!(paid.resolved, legacy.resolved);
    // Same separate, equivalent 1 MiB active ledger for the legacy route; the
    // no-active `legacy` run above keeps its zero-cost compatibility inventory.
    let (legacy_active, legacy_failed, _) = with_limit_usage(1 << 20, || {
        pass.resolve_functions(&program.path, &changed, None)
            .unwrap()
            .unwrap()
    });
    assert!(!legacy_failed);
    assert_eq!(legacy_active.resolved, legacy.resolved);
    assert_eq!(legacy_active.reused_functions, legacy.reused_functions);
    assert_eq!(paid.function_costs, legacy_active.function_costs);
    assert_eq!(
        legacy.function_costs.keys().collect::<Vec<_>>(),
        legacy_active.function_costs.keys().collect::<Vec<_>>()
    );
    assert_eq!(legacy.function_costs["main"], 0);
    assert_eq!(legacy.function_costs["helper"], saved_helper_cost);
}"####, r####"fn explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse() {
    use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
    use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
    let source = "module cold; @id(\"helper\") fn helper(value:i64)->i64 {value+1} @id(\"main\") fn main()->i64 {helper(1)}";
    let program = crate::check(source, "cold.spx").unwrap();
    let mut pass = pass(source, &program);
    let view = SyntheticProgramView::new(&program, Vec::new()).unwrap();
    let old = pass
        .resolve_functions(&program.path, &program, None)
        .unwrap()
        .unwrap();
    // Each full resolution is one module route: it acquires its own owner on
    // its own equivalent real workspace builder ledger. Sharing one arbitrary
    // 1 MiB ledger across both exhausted the independent-replay cursor.
    let (owned, failed, owned_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            pass.resolve_functions_paid(&program.path, &program, None, &mut owner)
                .unwrap()
                .unwrap()
        });
    assert!(!failed);
    assert!(owned_used > 0);
    let (borrowed, failed, borrowed_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            pass.resolve_borrowed_cold_source_paid(&program.path, &view, None, &mut owner)
                .unwrap()
                .unwrap()
        });
    assert!(!failed);
    assert!(borrowed_used > 0);
    // Paid costs are measured on an active ledger, so the legacy route is
    // compared under its own equivalent active ledger. The no-active run keeps
    // its zero-cost compatibility inventory, which is not normalized.
    let (old_active, old_failed, _) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            pass.resolve_functions(&program.path, &program, None)
                .unwrap()
                .unwrap()
        });
    assert!(!old_failed);
    assert_eq!(old_active.resolved, old.resolved);
    assert_eq!(
        old.function_costs.keys().collect::<Vec<_>>(),
        old_active.function_costs.keys().collect::<Vec<_>>()
    );
    assert!(old.function_costs.values().all(|cost| *cost == 0));
    for new in [&owned, &borrowed] {
        assert_eq!(new.resolved, old.resolved);
        assert_eq!(new.function_costs, old_active.function_costs);
        assert_eq!(
            (new.reused_functions, new.static_text),
            (old.reused_functions, old.static_text)
        );
    }
    // Initial identity refusal keeps the exact budget error and moves no HIR.
    let word = std::mem::size_of::<usize>();
    let report =
        std::mem::size_of::<IdentityReservation>() + std::mem::size_of::<Diagnostic>() + 48;
    let (refused, failed, _, receipt) = with_limit_usage_refusal(report, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        pass.resolve_functions_paid(&program.path, &program, None, &mut owner)
            .unwrap()
            .err()
            .unwrap()
    });
    assert!(failed);
    assert!(matches!(
        refused,
        paid_resolution::SelectiveFailure::Resolve(
            crate::hir::identity_resolution::Failure::Identity(Refused::Budget(_))
        )
    ));
    assert_eq!(
        receipt.unwrap().requested,
        (2 * word + 11).div_ceil(word) * word
    );
    // Checked body reuse and the first-checked-only borrowed gate are unchanged.
    let saved_helper_cost = owned.function_costs["helper"];
    let _owner = pass.resolved_module(
        &program.path,
        program.clone(),
        owned.resolved,
        0,
        0,
        owned.function_costs,
        owned.reused_functions,
    );
    pass.checked = std::mem::take(&mut pass.next_checked);
    let changed = crate::check(&source.replace("helper(1)", "helper(2)"), "cold.spx").unwrap();
    let legacy = pass
        .resolve_functions(&program.path, &changed, None)
        .unwrap()
        .unwrap();
    let (paid, failed, _) = with_limit_usage(1 << 20, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        let paid = pass
            .resolve_functions_paid(&program.path, &changed, None, &mut owner)
            .unwrap()
            .unwrap();
        assert!(pass
            .resolve_borrowed_cold_source_paid(&program.path, &view, None, &mut owner)
            .is_none());
        paid
    });
    assert!(!failed);
    assert_eq!(paid.reused_functions, 1);
    assert_eq!(paid.reused_functions, legacy.reused_functions);
    assert_eq!(paid.resolved, legacy.resolved);
    // Same separate, equivalent 1 MiB active ledger for the legacy route; the
    // no-active `legacy` run above keeps its zero-cost compatibility inventory.
    let (legacy_active, legacy_failed, _) = with_limit_usage(1 << 20, || {
        pass.resolve_functions(&program.path, &changed, None)
            .unwrap()
            .unwrap()
    });
    assert!(!legacy_failed);
    assert_eq!(legacy_active.resolved, legacy.resolved);
    assert_eq!(legacy_active.reused_functions, legacy.reused_functions);
    assert_eq!(paid.function_costs, legacy_active.function_costs);
    assert_eq!(
        legacy.function_costs.keys().collect::<Vec<_>>(),
        legacy_active.function_costs.keys().collect::<Vec<_>>()
    );
    assert_eq!(legacy.function_costs["main"], 0);
    assert_eq!(legacy.function_costs["helper"], saved_helper_cost);
}"####);
        // Exact composed receiving hooks; reconstruct every original byte below.
        undo(&mut current, r###"        let mut owner =
            crate::bounded_output::identity_reservation::IdentityReservation::acquire().unwrap();
        let before = crate::bounded_output::active_remaining();
        assert!(pass
            .resolve_borrowed_cold_source_paid(&program.path, &changed_view, None, &mut owner)
            .is_none());
        assert_eq!(before, crate::bounded_output::active_remaining());
        pass.resolve_borrowed_source_paid(&program.path, &changed_view, None, &mut owner)
            .unwrap()
            .unwrap()
"###, r###"        let mut owner =
            crate::bounded_output::identity_reservation::IdentityReservation::acquire().unwrap();
        pass.resolve_borrowed_cold_source_paid(&program.path, &changed_view, None, &mut owner)
            .unwrap()
            .unwrap()
"###);
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__project__incremental__cold_source_tests.rs.before"
            ),
            "src/project/incremental/cold_source_tests.rs"
        );
    }
    {
        let mut current = super::current_view::project("src/string_ops.rs", include_str!("../../../../string_ops.rs"));
        undo(
            &mut current,
            r#"    }
    false
}

#[cfg(test)]
pub(crate) use concat_funded::work::{
    is_same_owner_concat_hir_funded_with_work, same_owner_concat_appends_funded_with_work,
    same_owner_concat_operands_funded_with_work,
};

#[cfg(test)]
pub(crate) fn by_id_with_work<E>(
    id: &str,
    work: crate::hir::funded::work::Work<'_, E>,
) -> Result<Option<StringOp>, crate::hir::funded::FundedError<E>> {
    use crate::hir::funded::work as paid;
    const KEYS: &[(&str, StringOp)] = &[
        (take::ID, StringOp::Take),
        (LEN_ID, StringOp::Len),
        (CONCAT_ID, StringOp::Concat),
        (IS_EMPTY_ID, StringOp::IsEmpty),
        (STARTS_WITH_ID, StringOp::StartsWith),
        (CONTAINS_ID, StringOp::Contains),
        (LEN_CHARS_ID, StringOp::LenChars),
        (FROM_CHAR_ID, StringOp::FromChar),
        (FROM_I64_ID, StringOp::FromI64),
        (FROM_USIZE_ID, StringOp::FromUsize),
        (SLICE_ID, StringOp::Slice),
        (FIND_ID, StringOp::Find),
        (TO_I64_ID, StringOp::ToI64),
        (TRIM_ID, StringOp::Trim),
        (BYTE_AT_ID, StringOp::ByteAt),
        (FILE_READ_TEXT_ID, StringOp::FileReadText),
        (COMPARE_ID, StringOp::Compare),
        (MAP_NEW_ID, StringOp::MapNew),
        (MAP_ADD_ID, StringOp::MapAdd),
        (MAP_SET_ID, StringOp::MapSet),
        (MAP_GET_OR_ID, StringOp::MapGetOr),
        (MAP_HAS_ID, StringOp::MapHas),
        (MAP_LEN_ID, StringOp::MapLen),
        (MAP_KEY_AT_ID, StringOp::MapKeyAt),
        (MAP_VALUE_AT_ID, StringOp::MapValueAt),
        (MAP_REMOVE_ID, StringOp::MapRemove),
        (FROM_STR_ID, StringOp::FromStr),
        (FROM_UTF8_ID, StringOp::FromUtf8),
        (F64_FROM_I64_ID, StringOp::F64FromI64),
        (I64_FROM_F64_ID, StringOp::I64FromF64),
        (USIZE_FROM_I64_ID, StringOp::UsizeFromI64),
        (I64_FROM_U8_ID, StringOp::I64FromU8),
        (I64_FROM_I32_ID, StringOp::I64FromI32),
        (USIZE_FROM_U8_ID, StringOp::UsizeFromU8),
        (U8_FROM_I64_ID, StringOp::U8FromI64),
        (CHAR_FROM_U8_ID, StringOp::CharFromU8),
        (CHAR_FROM_I64_ID, StringOp::CharFromI64),
        (I64_FROM_USIZE_ID, StringOp::I64FromUsize),
    ];
    for (key, op) in KEYS {
        paid::tick(work, paid::str_units(key, id))?;
        if *key == id {
            return Ok(Some(*op));
        }
    }
    Ok(None)
}
"#,
            r#"    }
    false
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops.rs.before"
            ),
            "src/string_ops.rs"
        );
    }
    {
        let mut current = super::current_view::project("src/string_ops/concat_funded.rs", include_str!("../../../../string_ops/concat_funded.rs"));
        undo(
            &mut current,
            r#"        assert!(value && calls > 1 && charged > 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        assert!(value && calls > 1 && charged > 0);
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops__concat_funded.rs.before"
            ),
            "src/string_ops/concat_funded.rs"
        );
    }
    {
        let mut current = super::current_view::project("src/string_ops/conditions.rs", include_str!("../../../../string_ops/conditions.rs"));
        undo(
            &mut current,
            r#"    }
    false
}

#[cfg(test)]
pub(crate) use funded::work::{
    condition_reads_funded_with_work, function_reads_funded_with_work,
    needs_cleanup_funded_with_work,
};

#[cfg(test)]
pub(crate) fn resolved_operands_with_work<'a, E>(
    expression: &'a ResolvedExpr,
    work: crate::hir::funded::work::Work<'_, E>,
) -> Result<Option<&'a [ResolvedExpr]>, crate::hir::funded::FundedError<E>> {
    use crate::hir::funded::work as paid;
    paid::tick(work, 1)?;
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &expression.kind
    else {
        return Ok(None);
    };
    let Some(op) = super::by_id_with_work(callee.as_str(), work)? else {
        return Ok(None);
    };
    paid::tick(work, 7)?;
    if !is_condition_read(op)
        || !type_arguments.is_empty()
        || instance.is_some()
        || expression.ty != op.return_type()
        || expression.ownership != OwnershipMode::Value
        || args.len() != op.arity()
    {
        return Ok(None);
    }
    for (index, argument) in args.iter().enumerate() {
        paid::tick(work, 5)?;
        if op.param_ownership(index) != OwnershipMode::Borrow
            || argument.ty != ResolvedType::String
            || argument.ownership != OwnershipMode::Own
            || !matches!(&argument.kind,ResolvedExprKind::Place(place) if place.projections.is_empty())
        {
            return Ok(None);
        }
    }
    Ok(Some(args))
}
"#,
            r#"    }
    false
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops__conditions.rs.before"
            ),
            "src/string_ops/conditions.rs"
        );
    }
    {
        let mut current = include_str!("../../../../string_ops/conditions/funded.rs").to_owned();
        undo(
            &mut current,
            r#"        assert_eq!(active, 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        assert_eq!(active, 0);
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops__conditions__funded.rs.before"
            ),
            "src/string_ops/conditions/funded.rs"
        );
    }
    {
        let mut current = include_str!("../../../../string_ops/take.rs").to_owned();
        undo(
            &mut current,
            r#"
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn operand_with_work<'a, E>(
    expression: &'a ResolvedExpr,
    work: crate::hir::funded::work::Work<'_, E>,
) -> Result<Option<&'a ResolvedExpr>, crate::hir::funded::FundedError<E>> {
    use crate::hir::funded::work as paid;
    paid::tick(work, 1)?;
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return Ok(None);
    };
    paid::tick(work, paid::str_units(callee.as_str(), ID))?;
    if callee.as_str() != ID {
        return Ok(None);
    }
    // The complete original predicate inspects only these bounded scalar
    // discriminants/lengths. It performs no hidden lookup or tree walk.
    paid::tick(work, 10)?;
    if instance.is_some()
        || !type_arguments.is_empty()
        || expression.ty != ResolvedType::String
        || expression.ownership != OwnershipMode::Own
    {
        return Ok(None);
    }
    let [argument] = args.as_slice() else {
        return Ok(None);
    };
    Ok((argument.ty == ResolvedType::String
        && argument.ownership == OwnershipMode::Own
        && matches!(&argument.kind,ResolvedExprKind::Place(place) if place.projections.is_empty()))
    .then_some(argument))
}
"#,
            r#"
#[cfg(test)]
mod tests;
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops__take.rs.before"
            ),
            "src/string_ops/take.rs"
        );
    }
    {
        let mut current = super::current_view::project("src/workspace_graph/identity_resolution/tests.rs", include_str!("../../../../workspace_graph/identity_resolution/tests.rs"));
        undo(
            &mut current,
            r#"        assert_eq!(actual.1.costs(), expected_costs);
    }
}

mod oracle_owner_before;
"#,
            r#"        assert_eq!(actual.1.costs(), expected_costs);
    }
}
"#,
        );
        undo(
            &mut current,
            r#"/// Both results remain live until Drop inside the allocation observation.
#[test]
fn one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries() {
    crate::cleanup_plan::with_financed_test_oracles(
        oracle_owner_before::one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries,
    );
}

#[test]
"#,
            &format!(
                "{}{}{}",
                r#"/// Both results remain live until Drop inside the allocation observation.
#[test]
"#,
                include_str!(
                    "../../../../workspace_graph/identity_resolution/tests/oracle_owner_before.rs"
                )
                .strip_prefix("use super::*;\n\npub(super) ")
                .unwrap()
                .trim_end_matches('\n'),
                r#"

#[test]
"#
            ),
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__workspace_graph__identity_resolution__tests.rs.before"
            ),
            "src/workspace_graph/identity_resolution/tests.rs"
        );
    }
    {
        let mut current = super::current_view::project("tests/hir_module_boundaries.rs", include_str!("../../../../../tests/hir_module_boundaries.rs"));
        // 832: complete old/current ordinary lexical inventory source union.
        undo(&mut current,"        include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/return_operands.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/return_operands/exits.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/return_operands/tests.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/lexical_replay.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/lexical_replay/before_operands.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/lexical_replay/operand_tests.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/lexical_replay/ordinary.rs\"),\n","");
        // 781: retain every complete old/new selected-call source in the audit.
        undo(&mut current,"        include_str!(\"../src/cleanup_plan/replay/structural_control.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/structural_control/blocks.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/structural_control/call_receiving.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/structural_control/calls.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/structural_control/calls/custody_tests.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/structural_control/calls_before_receipt.rs\"),\n","");
        undo(&mut current, "        include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_entry/tests/fact_errors.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/semantic_refusal.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/semantic_refusal/tests.rs\"),\n", "");
        // 834: join complete old and new private-entry receivers and controls.
        undo(&mut current,"        include_str!(\"../src/workspace_graph/expected_projection/private_layout.rs\"),\n        include_str!(\"../src/workspace_graph/expected_projection/private_layout/capacity_before.rs\"),\n        include_str!(\"../src/workspace_graph/expected_projection/private_layout/capacity_tests.rs\"),\n        include_str!(\"../src/workspace_graph/expected_projection/private_layout/capacity_tests/owner_controls.rs\"),\n        include_str!(\"../src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs\"),\n","");
        undo(&mut current,"        include_str!(\"../src/cleanup/inventory_builder/selected_inline.rs\"),\n        include_str!(\"../src/cleanup_plan/renewal_profile/profile_refusal.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_owner/paid_before_children.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid_before_children.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid_children.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid_children/collections.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid_children/flat_records.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid_children/row_loan.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid_children/string_transfer.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid_children/tests.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid_children/tests/fixtures.rs\"),\n        include_str!(\"../src/hir/lexical_exit/latch_inline.rs\"),\n","");
        undo(&mut current,"        include_str!(\"../src/cleanup_plan/renewal_profile/child_refusal.rs\"),\n        include_str!(\"../src/cleanup_plan/renewal_profile/selected_inline.rs\"),\n        include_str!(\"../src/cleanup_plan/renewal_profile/selected_inline/append.rs\"),\n        include_str!(\"../src/cleanup_plan/renewal_profile/selected_inline/cells.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_context/child.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_context/child/tests.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_context/child/tests/source.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_context/paid_before_children.rs\"),\n        include_str!(\"../src/hir/lexical_exit/selected_child.rs\"),\n        include_str!(\"../src/loop_calls/byte_writer/resolved/selected_inline.rs\"),\n","");
        undo(&mut current, r########"        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_before.rs"),
        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs"),
"########, ""); undo(&mut current, "        include_str!(\"../src/loan_plan/nested_candidate_before.rs\"),\n", ""); undo(&mut current, "        include_str!(\"../src/loan_plan/paid_candidate/retained/nested_tests.rs\"),\n", ""); undo(&mut current, "        include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs\"),\n", ""); undo(&mut current, "        include_str!(\"../src/cleanup_plan/replay/factored/canonical_shared/app_census.rs\"),\n", "");
        undo(&mut current, "        include_str!(\"../src/hir/nominal_payload/legacy.rs\"),\n        include_str!(\"../src/hir/nominal_payload/legacy/clone.rs\"),\n        include_str!(\"../src/hir/nominal_payload/legacy/tests.rs\"),\n        include_str!(\"../src/hir/declaration_index/nominal/legacy.rs\"),\n        include_str!(\"../src/hir/identity_resolution/nominal_tests/legacy.rs\"),\n        include_str!(\"../src/hir/nominal_payload/legacy/source_tests.rs\"),\n", "");
        undo(&mut current, "        include_str!(\"../src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs\"),\n", "");
        undo(&mut current, "        include_str!(\"../src/cleanup_plan/replay/selected_context/paid.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_owner/paid.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_profile/paid.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/selected_entry/tests/session_errors.rs\"),\n", "");
        undo(&mut current, "            include_str!(\"../src/hir/validation/byte_traversal.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/before_nested.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested/slot.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested/check.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested/index.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested/intervals.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested/shape.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested/storage.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested/tests.rs\"),\n            include_str!(\"../src/hir/validation/byte_traversal/nested/walk.rs\"),\n", "            include_str!(\"../src/hir/validation/byte_traversal.rs\"),\n");
        undo(&mut current, "            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/helpers.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/helpers/regions.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/helpers/before_remaining.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/leaf_index.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/leaf_index/paid.rs\"),\n", "");
        undo(&mut current, "        include_str!(\"../src/loan_plan/paid_plan/retained_before_dense.rs\"),\n        include_str!(\"../src/loan_plan/funded_flat/before_dense.rs\"),\n", "");
        undo(&mut current, r###"        include_str!("../src/loan_plan/paid_live.rs"),
        include_str!("../src/loan_plan/paid_live/dense.rs"),
        include_str!("../src/loan_plan/paid_live/dense/tests.rs"),
"###, "");
        undo(&mut current, r###"        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs"),
        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs"),
        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/status_before_prefix.rs"),
        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/component_before_prefix.rs"),
"###, "");
        undo(&mut current, "            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/remaining.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs\"),\n            include_str!(\"../src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs\"),\n", "");
        undo(&mut current, "        include_str!(\"../src/hir/prelude_bundle/invocation.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/continuation_storage/paid_receiving.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/factored/continuation_receiver.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/factored/continuation_receiver/tests.rs\"),", "        include_str!(\"../src/hir/prelude_bundle/invocation.rs\"),");
        undo(&mut current, r####"                    include_str!("../src/bounded_output/canonical_stage_tests.rs"),
                    include_str!("../src/bounded_output/loan_inner_stage_tests.rs"),
"####, r####"                    include_str!("../src/bounded_output/canonical_stage_tests.rs"),
"####);
      undo(&mut current, r#####"        include_str!("../src/hir/prelude_bundle/invocation.rs"),
        include_str!("../src/hir/prelude_bundle/extension_work.rs"),
        include_str!("../src/hir/prelude_bundle/extension_model.rs"),
        include_str!("../src/hir/prelude_bundle/extension_producer.rs"),
        include_str!("../src/hir/prelude_bundle/extension_staging.rs"),
        include_str!("../src/hir/prelude_bundle/vector_extension.rs"),
        include_str!("../src/hir/prelude_bundle/vector_extension/tests.rs"),
        include_str!("../src/workspace_graph/identity_resolution/base/vector_tests.rs"),"#####, r#####"        include_str!("../src/hir/prelude_bundle/invocation.rs"),"#####);
        // Full exit prefix proof, complete prior receiver and owning controls.
        undo(&mut current, r########"concat!(include_str!("../src/cleanup_plan/replay/expression_facts/receiving.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/status_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/status_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/exit_prefix.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/exit_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/exit_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs"))"########, r########"concat!(include_str!("../src/cleanup_plan/replay/expression_facts/receiving.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/status_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/status_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs"))"########);
        // Exact additive full status-fact receiving and complete old/control join.
        undo(&mut current, r#######"concat!(include_str!("../src/cleanup_plan/replay/expression_facts/receiving.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/status_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/status_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs"))"#######, r#######"concat!(include_str!("../src/cleanup_plan/replay/expression_facts/receiving.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs"))"#######);
        // Remove only the exact new full receiver/oracle/control join before prior hooks.
        undo(&mut current, r######"concat!(include_str!("../src/cleanup_plan/replay/expression_facts/receiving.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs"))"######, r######"include_str!("../src/cleanup_plan/replay/expression_facts/receiving.rs")"######);
        // Exact new legacy iterative Drop producer, complete prior receiver and controls.
        undo(&mut current, r########"        include_str!("../src/hir/declaration_index/legacy_drop.rs"),
        include_str!("../src/hir/declaration_index/legacy_drop/before.rs"),
        include_str!("../src/hir/declaration_index/legacy_drop/tests.rs"),
"########, "");
        // Exact additive Legacy finalizer oracle and complete controls.
        undo(&mut current, r########"        include_str!("../src/hir/declaration_index/linked_owned_data/legacy_before.rs"),
"########, "");
        undo(&mut current, r########"        include_str!("../src/hir/declaration_index/linked_owned_data/tests/legacy.rs"),
"########, "");
        undo(&mut current, r########"        include_str!("../src/hir/workspace_link/legacy_nominal_before.rs"),
"########, "");
        undo(&mut current, r####"        include_str!("../src/hir/declaration_index/nominal_paid_facts.rs"),
        include_str!("../src/hir/declaration_index/linked_owned_data.rs"),
        include_str!("../src/hir/declaration_index/linked_owned_data/original.rs"),
        include_str!("../src/hir/declaration_index/linked_owned_data/tests.rs"),
"####, r####"        include_str!("../src/hir/declaration_index/nominal_paid_facts.rs"),
"####);
        undo(&mut current, r####"        include_str!("../src/hir/identity_resolution/nominal_tests.rs"),
        include_str!("../src/hir/identity_resolution/nominal_invalid_layout_tests.rs"),
"####, r####"        include_str!("../src/hir/identity_resolution/nominal_tests.rs"),
"####);
        undo(&mut current, r#####"        include_str!("../src/loan_plan/paid_candidate_before.rs"),
        include_str!("../src/loan_plan/paid_candidate.rs"),
        include_str!("../src/loan_plan/paid_candidate/retained.rs"),
        include_str!("../src/loan_plan/paid_candidate/retained/inventory.rs"),
        include_str!("../src/loan_plan/paid_candidate/retained/tests.rs"),
        include_str!("../src/loan_plan/paid_plan.rs"),
        include_str!("../src/loan_plan/paid_plan/retained.rs"),
        include_str!("../src/loan_plan/funded_flat.rs"),
        include_str!("../src/loan_plan/funded_flat/inventory_before.rs"),"#####, r#####"        include_str!("../src/loan_plan/paid_candidate_before.rs"),"#####);
        undo(&mut current, r####"        include_str!("../src/hir/declaration_index/nominal_paid_inheritance.rs"),
        include_str!("../src/hir/declaration_index/nominal_paid_facts.rs"),"####, r####"        include_str!("../src/hir/declaration_index/nominal_paid_inheritance.rs"),"####);
        undo(&mut current, r####"        include_str!("../src/hir/declaration_index/nominal_construction/types.rs"),
        include_str!("../src/hir/declaration_index/nominal_paid_values.rs"),
        include_str!("../src/hir/declaration_index/nominal_paid_inheritance.rs"),"####, r####"        include_str!("../src/hir/declaration_index/nominal_construction/types.rs"),"####);
        // Additive current child join, before every unchanged original inventory hook.
        undo(&mut current, r####"        include_str!("../src/hir/declaration_index/nominal_rollback.rs"),
        include_str!("../src/hir/declaration_index/nominal_construction.rs"),
        include_str!("../src/hir/declaration_index/nominal_construction/types.rs"),"####, r####"        include_str!("../src/hir/declaration_index/nominal_rollback.rs"),"####);
        undo(&mut current, r#####"        concat!(include_str!("../src/cleanup_plan/replay/expression_facts/arena.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/arena/prefix.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs")),"#####, r#####"        include_str!("../src/cleanup_plan/replay/expression_facts/arena.rs"),"#####);
        // Exact full pre-stage shared-sealing receiver join.
        undo(&mut current, r###"                    include_str!("../src/hir/canonical_loan_storage/caller/sealed_before_stage.rs"),
"###, "");
        // Exact additive canonical-stage controls remain in the full source scan.
        undo(&mut current, r###"                    include_str!("../src/bounded_output/canonical_stage_tests.rs"),
                    include_str!("../src/hir/canonical_loan_storage/caller/stage_tests.rs"),
"###, "");
        undo(&mut current, r###"                include_str!("../src/hir/validation/function_scope/lifecycle/completed/inline_path.rs"),
                include_str!("../src/hir/validation/function_scope/lifecycle/completed/inline_path/tests.rs"),
                include_str!("../src/hir/validation/function_scope/lifecycle/completed/before_inline.rs"),
"###, "");
        // Undo only the two additive complete checkpoint merge receiver joins.
        undo(&mut current, r###"                include_str!("../src/hir/validation/scope_checkpoint/merge.rs"),
                include_str!("../src/hir/validation/scope_checkpoint/merge/saved.rs"),
                include_str!("../src/hir/validation/scope_checkpoint/merge/tests.rs"),
"###, r###"                include_str!("../src/hir/validation/scope_checkpoint/merge.rs"),
"###);
        // Exact formatting hook for the complete additive generic call-graph union.
        undo(&mut current, r###"concat!(
            include_str!("../src/hir/validation/generic_template.rs"),
            include_str!("../src/hir/validation/generic_template/call_graph.rs"),
            include_str!("../src/hir/validation/generic_template/call_graph/ordered.rs"),
            include_str!("../src/hir/validation/generic_template/call_graph/original.rs"),
            include_str!("../src/hir/validation/generic_template/call_graph/tests.rs")
        )"###, r###"concat!(include_str!("../src/hir/validation/generic_template.rs"), include_str!("../src/hir/validation/generic_template/call_graph.rs"), include_str!("../src/hir/validation/generic_template/call_graph/ordered.rs"), include_str!("../src/hir/validation/generic_template/call_graph/original.rs"), include_str!("../src/hir/validation/generic_template/call_graph/tests.rs"))"###);
        undo(&mut current, r###"concat!(include_str!("../src/hir/validation/generic_template.rs"), include_str!("../src/hir/validation/generic_template/call_graph.rs"), include_str!("../src/hir/validation/generic_template/call_graph/ordered.rs"), include_str!("../src/hir/validation/generic_template/call_graph/original.rs"), include_str!("../src/hir/validation/generic_template/call_graph/tests.rs"))"###, r###"include_str!("../src/hir/validation/generic_template.rs")"###);
        // Undo only the additive complete intrinsic source union.
        // Exact formatting hook; all intrinsic union members remain covered.
        undo(&mut current, r###"concat!(
                        include_str!("../src/hir/validation/constructor_seed/original.rs"),
                        include_str!("../src/list_ops/original_constructor.rs"),
                        include_str!("../src/iterator_ops/original_constructor.rs"),
                        include_str!(
                            "../src/hir/validation/constructor_seed/original_intrinsics.rs"
                        ),
                        include_str!("../src/hir/validation/constructor_seed/intrinsic_tests.rs")
                    )"###, r###"concat!(include_str!("../src/hir/validation/constructor_seed/original.rs"), include_str!("../src/list_ops/original_constructor.rs"), include_str!("../src/iterator_ops/original_constructor.rs"), include_str!("../src/hir/validation/constructor_seed/original_intrinsics.rs"), include_str!("../src/hir/validation/constructor_seed/intrinsic_tests.rs"))"###);
        undo(&mut current, r###"concat!(include_str!("../src/hir/validation/constructor_seed/original.rs"), include_str!("../src/list_ops/original_constructor.rs"), include_str!("../src/iterator_ops/original_constructor.rs"), include_str!("../src/hir/validation/constructor_seed/original_intrinsics.rs"), include_str!("../src/hir/validation/constructor_seed/intrinsic_tests.rs"))"###, r###"include_str!("../src/hir/validation/constructor_seed/original.rs")"###);
        // Exact 824 child joins retain full live coverage; the final comparison
        // still reconstructs the complete unchanged original HIR inventory.
        undo(&mut current, r###"                concat!(
                    include_str!("../src/hir/validation/function_lookup.rs"),
                    include_str!("../src/hir/validation/function_lookup/original.rs"),
                    include_str!("../src/hir/validation/function_lookup/paid_custody.rs")
                ),
"###, r###"                include_str!("../src/hir/validation/function_lookup.rs"),
"###);
        undo(&mut current, r###"                concat!(
                    include_str!("../src/hir/validation/constructor_seed.rs"),
                    include_str!("../src/hir/validation/constructor_seed/original.rs")
                ),
"###, r###"                include_str!("../src/hir/validation/constructor_seed.rs"),
"###);
        undo(&mut current, r###"        concat!(
            include_str!("../src/hir/validation/stdin_stream.rs"),
            include_str!("../src/hir/validation/stdin_stream/paid.rs"),
            include_str!("../src/hir/validation/stdin_stream/paid/analysis.rs"),
            include_str!("../src/hir/validation/stdin_stream/paid/tests.rs")
        ),
"###, r###"        include_str!("../src/hir/validation/stdin_stream.rs"),
"###);
        undo(&mut current, r###"
#[cfg(test)]
#[path = "hir_module_boundaries/oracle_source_inventory.rs"]
mod oracle_source_inventory;
"###, "");
        undo(&mut current, r###"        include_str!("../src/cleanup_plan/replay/paid_type_facts/inventory_profile.rs"),
"###, r###"        include_str!("../src/cleanup_plan/replay/paid_type_facts/inventory_profile.rs"),
        include_str!("../src/cleanup_plan/replay/paid_type_facts/inventory_profile/tests.rs"),
"###);
        undo(&mut current, r###"        include_str!("../src/cleanup_plan/replay/paid_type_facts/inventory_summary/collector.rs"),
"###, r###"        include_str!("../src/cleanup_plan/replay/paid_type_facts/inventory_summary/collector.rs"),
        include_str!("../src/cleanup_plan/replay/paid_type_facts/inventory_summary/tests.rs"),
"###);
        undo(&mut current, r###"                include_str!("../src/hir/validation/held_profiles.rs"),
                include_str!("../src/hir/validation/held_profiles/tests.rs"),
                include_str!("../src/hir/validation/held_profiles/source_audit.rs"),
                include_str!("../src/hir/validation/executable_profiles/held.rs"),
                include_str!("../src/hir/validation/executable_profiles/before_812.rs"),
                include_str!("../src/hir/validation/call_parameters/before_812.rs"),
                include_str!("../src/hir/validation/call_parameters/held_tests.rs"),
                include_str!("../src/hir/ids/fixed_operation_tests.rs"),
                include_str!("../src/hir/resolve_box_call/map/identity_tests.rs"),
                include_str!("../src/hir/resolve_box_call/list/identity_tests.rs"),
                include_str!("../src/hir/resolve_box_call/iterator/identity_tests.rs"),
"###, "");
        undo(&mut current, "        include_str!(\"../src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/factored/canonical_shared/before_alias.rs\"),\n", "");
        undo(&mut current, "        include_str!(\"../src/cleanup_plan/replay/factored/canonical_shared/memo.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/factored/canonical_shared/memo/tests.rs\"),\n        include_str!(\"../src/cleanup_plan/replay/factored/canonical_shared/before_index.rs\"),\n", "");
        // Exact singleton target producer, complete predecessor and controls.
        undo(&mut current, r########"        include_str!("../src/cleanup_plan/replay/factored/canonical_shared/before_receiving_layers.rs"),
        include_str!("../src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs"),
"########, "");
        undo(&mut current, r########"        include_str!("../src/cleanup_plan/replay/factored/canonical_shared/successor_targets.rs"),
        include_str!("../src/cleanup_plan/replay/factored/canonical_shared/before_targets.rs"),
        include_str!("../src/cleanup_plan/replay/factored/canonical_shared/singleton_target_tests.rs"),
"########, "");
        undo(&mut current, r###"        include_str!("../src/cleanup_plan/replay/factored/canonical_shared.rs"),
        include_str!("../src/cleanup_plan/replay/factored/canonical_shared/tests.rs"),
        include_str!("../src/cleanup_plan/replay/factored/canonical_before.rs"),
"###, "");
        undo(&mut current, r###"        include_str!("../src/cleanup_plan/replay/factored/oracle_hook/closure_controls.rs"),
"###, "");
        undo(
            &mut current,
            r#"        include_str!("../src/hir/iterator_loop/renewal/paid_scan.rs"),
        include_str!("../src/hir/iterator_loop/renewal/funded.rs"),
        include_str!("../src/hir/funded.rs"),
        include_str!("../src/bounded_output/classifier_string/oracle_work.rs"),
        include_str!("../src/cleanup_plan/replay/allocation/oracle/diagnostic/metered.rs"),
        include_str!("../src/cleanup_plan/replay/factored/oracle_order.rs"),
        include_str!("../src/cleanup_plan/replay/factored/oracle_order/tests.rs"),
        include_str!("../src/cleanup_plan/replay/live_oracle.rs"),
        include_str!("../src/cleanup_plan/replay/nested_shape/paid/oracle.rs"),
        include_str!("../src/cleanup_plan/replay/paid_type_facts/oracle_inventory.rs"),
        include_str!("../src/cleanup_plan/replay/paid_type_facts/oracle_inventory/tests.rs"),
        include_str!("../src/hir/function_value/funded/work.rs"),
        include_str!("../src/hir/funded/work.rs"),
        include_str!("../src/hir/ids/metered_owner/oracle.rs"),
        include_str!("../src/hir/lexical_exit/owner_funded/work.rs"),
        include_str!("../src/literal_format/funded/work.rs"),
        include_str!("../src/project/incremental/cold_source_tests/oracle_owner_before.rs"),
        include_str!("../src/string_ops/concat_funded/work.rs"),
        include_str!("../src/string_ops/conditions/funded/work.rs"),
        include_str!("../src/workspace_graph/identity_resolution/tests/oracle_owner_before.rs"),
        include_str!("../src/hir/ordinary_scan.rs"),
        include_str!("../src/hir/ordinary_scan/tests.rs"),
        include_str!("../src/hir/function_value/funded.rs"),
"#,
            r#"        include_str!("../src/hir/iterator_loop/renewal/paid_scan.rs"),
        include_str!("../src/hir/iterator_loop/renewal/funded.rs"),
        include_str!("../src/hir/funded.rs"),
        include_str!("../src/hir/ordinary_scan.rs"),
        include_str!("../src/hir/ordinary_scan/tests.rs"),
        include_str!("../src/hir/function_value/funded.rs"),
"#,
        );
        undo(
            &mut current,
            r#"        include_str!(
            "../src/cleanup_plan/replay/paid_type_facts/inventory_summary/shape_original.rs"
        ),
"#,
            "",
        );
        undo(
            &mut current,
            r#"        include_str!(
            "../src/cleanup_plan/replay/paid_type_facts/inventory_summary/frontier_tests.rs"
        ),
"#,
            "",
        );
        undo(
            &mut current,
            r####"                include_str!("../src/hir/validation/function_scope.rs"),
                include_str!("../src/hir/validation/function_scope/empty_ensures.rs"),
                include_str!("../src/hir/validation/function_scope/empty_ensures/before.rs"),
                include_str!("../src/hir/validation/function_scope/empty_ensures/tests.rs"),
                include_str!("../src/hir/ordinary_scan.rs"),
"####,
            r####"                include_str!("../src/hir/validation/function_scope.rs"),
                include_str!("../src/hir/ordinary_scan.rs"),
"####,
        );
            undo(&mut current, "        include_str!(\"../src/hir.rs\"),\n        include_str!(\"../src/bounded_output/identity_reservation/base_prelude.rs\"),\n        include_str!(\"../src/bounded_output/identity_reservation/base_prelude/tests.rs\"),\n        include_str!(\"../src/bounded_output/identity_reservation/producer_failure.rs\"),\n        include_str!(\"../src/hir/declaration_index/nominal.rs\"),\n        include_str!(\"../src/hir/declaration_index/nominal_base.rs\"),\n        include_str!(\"../src/hir/declaration_index/nominal_original.rs\"),\n        include_str!(\"../src/hir/declaration_index/nominal_rollback.rs\"),\n        include_str!(\"../src/hir/identity_resolution/nominal.rs\"),\n        include_str!(\"../src/hir/identity_resolution/nominal_original.rs\"),\n        include_str!(\"../src/hir/identity_resolution/nominal_tests.rs\"),\n        include_str!(\"../src/hir/nominal_payload.rs\"),\n        include_str!(\"../src/hir/nominal_payload/codec.rs\"),\n        include_str!(\"../src/hir/nominal_payload/elements.rs\"),\n        include_str!(\"../src/hir/nominal_payload/map.rs\"),\n        include_str!(\"../src/hir/nominal_payload/tests.rs\"),\n        include_str!(\"../src/hir/prelude_bundle.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/borrowed.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/fixed_work.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/invocation.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/materialize.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/merge.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/original.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/overlay.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/paid.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/producer.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/tests.rs\"),\n        include_str!(\"../src/hir/prelude_bundle/work_model.rs\"),\n        include_str!(\"../src/project/incremental/paid_resolution/base.rs\"),\n        include_str!(\"../src/workspace_graph/identity_resolution/base.rs\"),\n        include_str!(\"../src/workspace_graph/identity_resolution/base/tests.rs\"),\n        include_str!(\"../src/workspace_graph/identity_resolution/base/affinity_tests.rs\"),\n\n        include_str!(\"../src/hir/function_reuse_source.rs\"),\n", "        include_str!(\"../src/hir.rs\"),\n        include_str!(\"../src/hir/function_reuse_source.rs\"),\n");
assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/tests__hir_module_boundaries.rs.before"
            ),
            "tests/hir_module_boundaries.rs"
        );
    }
}
