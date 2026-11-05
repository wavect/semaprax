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
        let mut current = super::current_view::project(
            "src/hir/funded.rs",
            include_str!("../../../../hir/funded.rs"),
        );
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
        let mut current = crate::source_reflows_merge::current344::project(
            "src/hir/ids.rs",
            include_str!("../../../../hir/ids.rs"),
        );
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
        undo(
            &mut current,
            r###"#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;

#[cfg(test)]
pub(in crate::hir) mod fixed_operation_tests;
"###,
            r###"#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;
"###,
        );
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
        let mut current = super::current_view::before_production_promotion(
            "src/hir/lexical_exit.rs",
            include_str!("../../../../hir/lexical_exit.rs"),
        );
        undo(
            &mut current,
            "pub(crate) use selected_child::{requires_inline,validate_owner_inline};",
            "pub(crate) use selected_child::requires_inline;",
        );
        undo(
            &mut current,
            "mod latch;\nmod latch_inline;\npub(crate) use latch_inline::validate_latch_inline;",
            "mod latch;",
        );
        undo(
            &mut current,
            "mod walk;\nmod selected_child;\npub(crate) use selected_child::requires_inline;",
            "mod walk;",
        );
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
        let mut current = super::current_view::project(
            "src/hir/lexical_exit/owner_funded.rs",
            include_str!("../../../../hir/lexical_exit/owner_funded.rs"),
        );
        // Exact #766 test-only extension: retain the whole original plain
        // receiver and all previous source inverses; no production bytes change.
        undo(
            &mut current,
            r#####"

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
        );"#####,
            "",
        );
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
        let mut current = super::current_view::project(
            "src/hir/lexical_exit/walk.rs",
            include_str!("../../../../hir/lexical_exit/walk.rs"),
        );
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
        let mut current = super::current_view::project(
            "src/literal_format.rs",
            include_str!("../../../../literal_format.rs"),
        );
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
        let mut current = super::current_view::funded_before_promotion(
            "src/literal_format/funded.rs",
            include_str!("../../../../literal_format/funded.rs"),
        );
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
        undo(
            &mut current,
            "#[path = \"cold_source_tests/oracle_owner_before.rs\"]\n",
            "",
        );
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
        undo(
            &mut current,
            r####"fn explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse() {
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
}"####,
            r####"fn explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse() {
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
}"####,
        );
        // Exact composed receiving hooks; reconstruct every original byte below.
        undo(
            &mut current,
            r###"        let mut owner =
            crate::bounded_output::identity_reservation::IdentityReservation::acquire().unwrap();
        let before = crate::bounded_output::active_remaining();
        assert!(pass
            .resolve_borrowed_cold_source_paid(&program.path, &changed_view, None, &mut owner)
            .is_none());
        assert_eq!(before, crate::bounded_output::active_remaining());
        pass.resolve_borrowed_source_paid(&program.path, &changed_view, None, &mut owner)
            .unwrap()
            .unwrap()
"###,
            r###"        let mut owner =
            crate::bounded_output::identity_reservation::IdentityReservation::acquire().unwrap();
        pass.resolve_borrowed_cold_source_paid(&program.path, &changed_view, None, &mut owner)
            .unwrap()
            .unwrap()
"###,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__project__incremental__cold_source_tests.rs.before"
            ),
            "src/project/incremental/cold_source_tests.rs"
        );
    }
    complete_original_sources_tail_b();
}

include!("b_tail.rs");
