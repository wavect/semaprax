//! Closed no-binding roots omit only the first Names traversal machinery.
use super::{Expr, ExprKind, Proof, Statement};
use std::mem::size_of;
// Two helper argument/formal+boolean-return tuples; both match-place loans;
// bounded Vec::is_empty actual/formals and length/boolean receivers; Box tail
// pattern/Deref loans and returned expression; all selected/branch booleans.
// This bank is ADDITIONAL on every attempted root, including fallback.
pub(in crate::workspace_graph::expected_projection) const SELECTOR: usize =
    2 * size_of::<(&Expr, &mut Proof, bool)>()
        + size_of::<(
            [&ExprKind; 2],
            [&Vec<Statement>; 3],
            [&Vec<Expr>; 4],
            [&Box<Expr>; 3],
            [&Expr; 3],
            [usize; 8],
            [bool; 12],
        )>();

pub(in crate::workspace_graph::expected_projection) fn accounted(
    root: &Expr,
    proof: &mut Proof,
) -> bool {
    #[cfg(test)]
    note(0);
    // The original callback's charge(&expression) is retained once per node,
    // in original root-before-tail order. No body/parameter/callee result is
    // reused; every other root still runs the entire original bounded walker.
    let closed = match &root.kind {
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::BorrowedUtf8Literal(_) => {
            proof.charge(&root);
            true
        }
        ExprKind::Call { args, .. } if args.is_empty() => {
            proof.charge(&root);
            true
        }
        ExprKind::Block { statements, tail } if statements.is_empty() => {
            let child: &Expr = tail;
            match &child.kind {
                ExprKind::Int(_)
                | ExprKind::Int32(_)
                | ExprKind::Char(_)
                | ExprKind::Uint8(_)
                | ExprKind::Usize(_)
                | ExprKind::ArrayU8(_)
                | ExprKind::RepeatArrayU8 { .. }
                | ExprKind::Float32(_)
                | ExprKind::Float64(_)
                | ExprKind::Bool(_)
                | ExprKind::String(_)
                | ExprKind::BorrowedUtf8Literal(_) => {
                    proof.charge(&root);
                    proof.charge(&child);
                    true
                }
                ExprKind::Call { args, .. } if args.is_empty() => {
                    proof.charge(&root);
                    proof.charge(&child);
                    true
                }
                _ => false,
            }
        }
        _ => false,
    };
    #[cfg(test)]
    if closed {
        note(1);
    }
    closed
}

#[cfg(test)]
thread_local! {
    static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static COUNTS: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) };
}
#[cfg(test)]
pub(super) fn before_selected() -> bool {
    BEFORE.with(|flag| flag.get())
}
#[cfg(test)]
pub(super) struct Before(bool);
#[cfg(test)]
impl Before {
    pub(super) fn hold() -> Self {
        Self(BEFORE.with(|flag| flag.replace(true)))
    }
}
#[cfg(test)]
impl Drop for Before {
    fn drop(&mut self) {
        BEFORE.with(|flag| flag.set(self.0));
    }
}
#[cfg(test)]
fn note(index: usize) {
    COUNTS.with(|count| {
        let mut n = count.get();
        n[index] += 1;
        count.set(n);
    });
}

#[cfg(test)]
#[path = "../../identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
#[cfg(test)]
mod tests {
    use super::super::{call_identity, count, count_joint, count_linear, count_modular};
    use super::*;
    use sha2::{Digest, Sha256};
    const ROOT: &str = "src/workspace_graph/expected_projection/call_carriers.rs";
    fn parse(text: &str) -> crate::ast::Program {
        crate::parse(text, "closed-roots.spx").unwrap()
    }
    fn take() -> [usize; 2] {
        COUNTS.with(|n| n.replace([0; 2]))
    }
    fn fixture(count: usize, complex: bool) -> String {
        let mut text = String::from("module closed_roots;");
        for i in 0..count {
            let body = if complex {
                format!("let n={i}; n")
            } else {
                format!("{i}")
            };
            text.push_str(&format!("@id(\"f{i}\") fn f{i}()->i64{{{body}}}"));
        }
        text
    }
    #[test]
    fn exact_closed_roots_keep_all_original_node_charges_and_binding_shapes_fall_back() {
        for (body, expected) in [
            ("1", true),
            ("f()", true),
            ("let n=1;n", false),
            ("f(1)", false),
            ("if true {1} else {2}", false),
            ("match 0 {_=>1,}", false),
        ] {
            let program = parse(&format!("module root; @id(\"f\") fn f()->i64{{{body}}}"));
            let root = &program.functions[0].body;
            let mut old = Proof::default();
            let mut visited = 0usize;
            assert!(call_identity::walk(root, &mut |expression| {
                visited += 1;
                old.charge(&expression);
                true
            }));
            let mut current = Proof {
                bytes: SELECTOR,
                ..Default::default()
            };
            let selected = accounted(root, &mut current);
            assert_eq!(selected, expected);
            if selected {
                assert!(visited == 1 || visited == 2);
                assert_eq!(current.bytes, SELECTOR + old.bytes);
                assert!(SELECTOR < size_of::<[Option<(&Expr, usize)>; 129]>());
            } else {
                assert_eq!(current.bytes, SELECTOR);
            }
        }
    }
    #[test]
    fn all_four_complete_callers_keep_proofs_and_exact_actual_scratch_delta() {
        for (n, complex) in [(0, false), (1, false), (64, false), (64, true)] {
            let programs = vec![parse(&fixture(n, complex))];
            let authored = crate::workspace_graph::index_authored(&programs).unwrap();
            let context =
                crate::workspace_graph::expected_projection::shared_local_callees::Context {
                    program: &programs[0],
                    authored: &authored,
                    programs: &programs,
                };
            for mode in 0..4 {
                let run = |old| {
                    let _before = if old { Some(Before::hold()) } else { None };
                    take();
                    let proof = match mode {
                        0 => count_linear(&programs[0], &authored, true),
                        1 => count(&programs[0], &authored, true),
                        2 => count_joint(context, true).unwrap().0,
                        _ => count_modular(context, true).unwrap().0,
                    };
                    let counts = take();
                    (proof, counts)
                };
                let (old, old_counts) = run(true);
                let (current, counts) = run(false);
                assert_eq!(old_counts, [0; 2]);
                assert_eq!(old.roots, current.roots);
                assert_eq!(old.retained, current.retained);
                assert_eq!(
                    (old.current_retained, old.retention_closed),
                    (current.current_retained, current.retention_closed)
                );
                let with_new = old
                    .bytes
                    .checked_add(counts[0].checked_mul(SELECTOR).unwrap())
                    .unwrap();
                let absent = counts[1]
                    .checked_mul(size_of::<[Option<(&Expr, usize)>; 129]>())
                    .unwrap();
                assert_eq!(with_new.checked_sub(absent), Some(current.bytes));
                if n > 0 && !complex {
                    assert!(counts[1] > 0 && current.bytes < old.bytes);
                }
                if complex {
                    assert_eq!(counts[1], 0);
                    assert!(current.bytes > old.bytes);
                }
                eprintln!("CLOSED_NAMES mode={mode} functions={n} complex={complex} selector_calls={} omitted_actual_walkers={} old_fee={} current_fee={}; source-only control until executed",counts[0],counts[1],old.bytes,current.bytes);
            }
        }
    }
    #[test]
    fn full_mode6_forecasts_preserve_all_storage_semantics_and_complete_drop() {
        for (n, complex) in [(0, false), (1, false), (64, false), (64, true)] {
            let programs = vec![parse(&fixture(n, complex))];
            let authored = crate::workspace_graph::index_authored(&programs).unwrap();
            let maximum = crate::workspace_graph::expected_projection::dependency_identity_max(
                &programs[0],
                &authored,
                &programs,
            )
            .unwrap();
            let run = |old| {
                let _before = if old { Some(Before::hold()) } else { None };
                crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                    crate::test_allocation_observer::measure(|| {
                        let (full,retained)=crate::workspace_graph::expected_projection::shared_local_callees::modular_pair::selected_estimates(&programs[0],&authored,&programs,Some(maximum),6,true,None).unwrap();
                        let result = (
                            [
                                full.retained_hir,
                                retained.retained_hir,
                                full.synthetic_ast,
                                retained.synthetic_ast,
                                full.runtime,
                                retained.runtime,
                            ],
                            retained.retained_scratch,
                        );
                        drop((full, retained));
                        result
                    })
                })
            };
            let old = run(true);
            let current = run(false);
            assert_eq!(&old.0 .0 .0[2..], &current.0 .0 .0[2..]);
            assert_eq!(old.0 .0 .1, current.0 .0 .1);
            if n > 0 && !complex {
                assert!(current.0 .0 .0[0] < old.0 .0 .0[0] && current.0 .0 .0[1] < old.0 .0 .0[1]);
            }
            if complex {
                assert!(
                    current.0 .0 .0[0] >= old.0 .0 .0[0] && current.0 .0 .0[1] >= old.0 .0 .0[1]
                );
            }
            for row in [&old, &current] {
                assert!(!row.1 && row.3.is_none());
                assert!(!row.0 .1.invalid && row.0 .1.live == 0);
                assert_eq!(row.0 .1.allocated, row.0 .1.freed);
                assert!(row.0 .1.allocated <= row.2);
            }
            eprintln!("CLOSED_NAMES_FULL functions={n} fallback={complex} current={current:?} old={old:?}; no application fit or measured result until owning execution");
        }
    }
    #[test]
    fn complete_forecast_exact_short_zero_keep_six_field_diagnostics_and_old_caps() {
        struct Limit(usize);
        impl Drop for Limit {
            fn drop(&mut self) {
                crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|n| n.set(self.0));
            }
        }
        let programs = vec![parse(&fixture(64, false))];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        for old in [true, false] {
            let _stage = if old { Some(Before::hold()) } else { None };
            for retention in [
                crate::workspace_graph::expected_projection::cached_exact::Retention::Parsed,
                crate::workspace_graph::expected_projection::cached_exact::Retention::Semantic,
            ] {
                let expected = crate::workspace_graph::expected_projection::cached_exact::summed(
                    &programs, &authored, retention,
                )
                .unwrap();
                let _limit = Limit(
                    crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|n| n.replace(expected.1)),
                );
                assert_eq!(
                    crate::workspace_graph::expected_projection::cached_exact::summed(
                        &programs, &authored, retention
                    )
                    .unwrap(),
                    expected
                );
                for bound in [expected.1 - 1, 0] {
                    crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|n| n.set(bound));
                    let first = crate::workspace_graph::expected_projection::cached_exact::summed(
                        &programs, &authored, retention,
                    )
                    .unwrap_err();
                    assert!(first.iter().all(|e| e.code == "SPX-G171"));
                    let retry = crate::workspace_graph::expected_projection::cached_exact::summed(
                        &programs, &authored, retention,
                    )
                    .unwrap_err();
                    assert_eq!(first, retry);
                }
            }
        }
    }
    #[test]
    fn complete_sources_old_fields_and_new_child_remain_physically_bound() {
        let metadata = include_str!("closed_roots_inverse.json");
        let hash = |b: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(b)));
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = root.join(
            "src/workspace_graph/expected_projection/call_carriers/closed_roots_inverse.json",
        );
        assert_eq!(
            reader::read(&path, metadata.len(), &hash(metadata.as_bytes()), 2 << 20).unwrap(),
            metadata
        );
        let data: serde_json::Value = serde_json::from_str(metadata).unwrap();
        for row in data["owners"].as_array().unwrap() {
            let source = row["source"].as_str().unwrap();
            let current = reader::read(
                &root.join(source),
                row["current"]["length"]
                    .as_u64()
                    .unwrap()
                    .try_into()
                    .unwrap(),
                row["current"]["sha256"].as_str().unwrap(),
                2 << 20,
            )
            .unwrap();
            let mut restored = current.clone();
            for edit in row["edits"].as_array().unwrap().iter().rev() {
                let start: usize = edit["start"].as_u64().unwrap().try_into().unwrap();
                let end: usize = edit["end"].as_u64().unwrap().try_into().unwrap();
                assert_eq!(&restored[start..end], edit["after"].as_str().unwrap());
                restored.replace_range(start..end, edit["before"].as_str().unwrap());
            }
            assert_eq!(
                restored.len() as u64,
                row["before"]["length"].as_u64().unwrap()
            );
            assert_eq!(
                hash(restored.as_bytes()),
                row["before"]["sha256"].as_str().unwrap()
            );
            assert_eq!(
                reader::read(
                    &root.join(source),
                    current.len(),
                    &hash(current.as_bytes()),
                    2 << 20
                )
                .unwrap(),
                current
            );
        }
        let current = include_str!("closed_roots.rs");
        assert_eq!(
            hash(current.as_bytes()),
            data["new_child"]["sha256"].as_str().unwrap()
        );
        assert_eq!(
            reader::read(
                &root.join("src/workspace_graph/expected_projection/call_carriers/closed_roots.rs"),
                current.len(),
                &hash(current.as_bytes()),
                2 << 20
            )
            .unwrap(),
            current
        );
        assert_eq!(data["owners"][0]["source"], ROOT);
    }
}
