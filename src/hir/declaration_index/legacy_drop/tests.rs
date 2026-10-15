//! Actual old queue versus adopted owned children; existing 512 owner unchanged.
use super::super::{DeclarationId, DeclarationIndex, ResolvedFieldDeclaration, ResolvedType};
use crate::ast::Span;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure, measure_with_capacity};

fn chain(depth: usize, functions: bool) -> ResolvedType {
    let mut ty = ResolvedType::I64;
    for level in 0..depth {
        ty = if functions && level % 2 == 0 {
            ResolvedType::Function {
                parameters: Vec::new(),
                result: Box::new(ty),
            }
        } else {
            ResolvedType::Nominal {
                declaration: DeclarationId::new("drop.chain"),
                arguments: vec![ty],
            }
        };
    }
    ty
}
fn field(ty: ResolvedType) -> ResolvedFieldDeclaration {
    ResolvedFieldDeclaration {
        id: DeclarationId::new("drop.field"),
        name: "field".to_owned(),
        index: 0,
        ty,
        span: Span::default(),
    }
}
fn index(roots: usize, functions: bool) -> DeclarationIndex {
    let mut index = DeclarationIndex::default();
    let fields: Vec<_> = (0..roots).map(|_| field(chain(512, functions))).collect();
    index
        .case_fields
        .insert(DeclarationId::new("drop.case"), fields);
    let fields: Vec<_> = (0..roots).map(|_| field(chain(512, functions))).collect();
    index
        .record_fields
        .insert(DeclarationId::new("drop.record"), fields);
    index
}

#[test]
fn detached_legacy_case_roots_reuse_argument_backing_and_keep_complete_old_drop_independent() {
    let mut deltas = [0, 0];
    for (slot, old) in [true, false].into_iter().enumerate() {
        let ((), whole) = measure(|| {
            let index = index(1, false);
            let stopped = current_metrics().allocated;
            if old {
                index.drop_before_argument_reuse();
            } else {
                drop(index);
            }
            deltas[slot] = current_metrics().allocated - stopped;
        });
        assert!(!whole.invalid && whole.live == 0);
        assert_eq!(whole.allocated, whole.freed);
    }
    // Actual pinned Vec minimum capacity, never an observed-byte allowance.
    assert_eq!(deltas[0], 4 * std::mem::size_of::<ResolvedType>());
    assert_eq!(deltas[1], 0);
}

#[test]
fn multiple_case_record_roots_and_function_spines_drop_iteratively_without_new_backing() {
    for functions in [false, true] {
        // Sixteen depth-512 roots retain an argument Vec and identity Arc
        // per nominal node: those alone fill the fixed 16,384-pointer table.
        // Three slots per node cover both retained allocations and the
        // transient identity String; eight per root cover field/map metadata.
        // This preallocated observer storage changes no compiler/runtime quota.
        let pointer_limit = 2 * 8 * (3 * 512 + 8);
        let ((), whole) = measure_with_capacity(pointer_limit, || {
            let index = index(8, functions);
            let stopped = current_metrics().allocated;
            drop(index);
            assert_eq!(current_metrics().allocated, stopped);
        });
        assert!(!whole.invalid && whole.live == 0);
        assert_eq!(whole.allocated, whole.freed);
    }
}

#[test]
fn branching_unfunded_disposal_settles_every_allocation_and_original_sticky_ledger() {
    let (((), whole), refused, used, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let mut index = DeclarationIndex::default();
            let branches = (0..17)
                .map(|_| ResolvedType::Function {
                    parameters: (0..9).map(|_| chain(7, false)).collect(),
                    result: Box::new(chain(19, true)),
                })
                .collect();
            let fields = vec![field(ResolvedType::Nominal {
                declaration: DeclarationId::new("drop.branch"),
                arguments: branches,
            })];
            index
                .case_fields
                .insert(DeclarationId::new("drop.case"), fields);
            assert!(!reserve_active_required(1));
            // This legacy route may grow scratch for wider frontiers. It is
            // explicitly unfunded, and cannot spend/reset the active ledger.
            drop(index);
            assert!(!reserve_active_required(0));
        })
    });
    assert!(refused && used == 0);
    let first = first.unwrap();
    assert_eq!((first.requested, first.remaining, first.floor), (1, 0, 0));
    assert!(!whole.invalid && whole.live == 0);
    assert_eq!(whole.allocated, whole.freed);
}

#[test]
fn complete_previous_drop_and_existing_mixed_case_512_owner_remain_exact() {
    let current = include_str!("../../declaration_index.rs");
    let current = &current[current.find("impl Drop for DeclarationIndex {").unwrap()..];
    let complete_old = include_str!(
        "../../../../docs/evidence/legacy-linked-drop-reuse-828f/complete-drop-before.rs.txt"
    );
    let paid_prefix = complete_old
        .split_once("        let mut pending = Vec::new();")
        .unwrap()
        .0;
    assert!(
        current.starts_with(paid_prefix),
        "complete paid workspace arm is unchanged"
    );
    assert!(current.starts_with(&format!(
        "{paid_prefix}        self.drain_legacy_type_roots(legacy_drop::dispose);\n    }}\n}}"
    )));
    let saved = include_str!("before.rs");
    let start = saved.find("impl DeclarationIndex {").unwrap();
    let restored = saved[start..]
        .replacen(
            "impl DeclarationIndex {",
            "impl Drop for DeclarationIndex {",
            1,
        )
        .replacen(
            "    pub(in super::super) fn drop_before_argument_reuse(mut self) {",
            "    fn drop(&mut self) {",
            1,
        );
    let restored = restored.strip_suffix('\n').unwrap();
    assert_eq!(
        restored,
        include_str!(
            "../../../../docs/evidence/legacy-linked-drop-reuse-828f/complete-drop-before.rs.txt"
        )
    );
    assert_eq!(include_str!("../linked_owned_data/tests/legacy.rs"), include_str!("../../../../docs/evidence/legacy-linked-drop-reuse-828f/complete-existing-legacy-tests.rs.txt"));
}
