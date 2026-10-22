use super::*;
use crate::ast::{Expr, ExprKind, Span};
use crate::hir::resolve_expr::path_arena::{PathArena, PathId, Seg};
use crate::hir::borrowed_root_oracle::resolve_expr_frame::capacity_old;
use crate::hir::{Binding, DeclarationId, OwnershipMode, ResolvedType, ValueId};
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

fn scope(name: &str) -> Rc<BTreeMap<String, Binding>> {
    Rc::new(BTreeMap::from([(
        name.to_owned(),
        Binding {
            traversal_row: None,
            id: ValueId::new(name.to_owned()),
            ty: ResolvedType::Function {
                parameters: vec![ResolvedType::I64],
                result: Box::new(ResolvedType::I64),
            },
            ownership: OwnershipMode::Value,
            mutable: false,
        },
    )]))
}
fn old_owned(frames: &[Frame<'_>], current: &Frame<'_>) -> usize {
    let mut seen = HashSet::new();
    frames
        .iter()
        .chain(std::iter::once(current))
        .fold(0usize, |total, frame| {
            total.saturating_add(capacity_old::frame_owned_capacity(frame, &mut seen))
        })
}

#[test]
fn resolver_capacity_census_matches_saved_scope_set_without_observer_allocations() {
    let expr = Expr {
        kind: ExprKind::Int(0),
        span: Span::default(),
    };
    let shared = scope("shared");
    let distinct = scope("distinct");
    for count in [0, 1, 64, 512] {
        let frames: Vec<_> = (0..count)
            .map(|index| Frame::Enter {
                expr: &expr,
                bindings: if index % 3 == 0 {
                    Rc::clone(&distinct)
                } else {
                    Rc::clone(&shared)
                },
                path: PathId::ROOT,
            })
            .collect();
        let current = Frame::StartUpcast {
            source: &expr,
            bindings: Rc::clone(&shared),
            slot_parent: PathId::ROOT,
            slot_edge: Seg::ArgZero.into(),
            holder: DeclarationId::new("holder"),
            span: Span::default(),
            resume: Box::new(Frame::ForOwn(Box::new(
                crate::hir::borrowed_root_oracle::resolve_for_own::ForOwnFrame {
                    span: Span::default(),
                    path: PathId::ROOT,
                    statements: &[],
                    tail: &expr,
                    index: 0,
                    scope: Rc::clone(&distinct),
                    resolved: vec![],
                    source: None,
                },
            ))),
        };
        let (expected, old) =
            crate::test_allocation_observer::measure(|| old_owned(&frames, &current));
        assert!(!old.invalid && old.allocations > 0);
        assert_eq!(old.allocated, old.freed);
        assert_eq!(old.live, 0);
        let paths = PathArena::new("body");
        let results = Vec::new();
        let loops = [None];
        let total = frames.capacity() * std::mem::size_of::<Frame<'_>>()
            + expected
            + paths.owned_capacity()
            + std::mem::size_of_val(&loops);
        crate::hir::reset_iterative_phase_capacity_high_water();
        // Fixture construction and the independently allocating old oracle are
        // outside this observation. The actual note entry and all its repeated
        // uniqueness/payload walks, including their Drop, are inside it.
        let (_, new) = crate::test_allocation_observer::measure(|| {
            for _ in 0..8 {
                assert_eq!(owned_frames(&frames, &current), expected);
                note(&frames, &results, &current, &loops, &paths);
            }
        });
        assert!(!new.invalid);
        assert_eq!(
            (
                new.allocations,
                new.reallocations,
                new.allocated,
                new.freed,
                new.live
            ),
            (0, 0, 0, 0, 0)
        );
        assert_eq!(crate::hir::iterative_phase_capacity_high_water()[0], total);
    }
}
