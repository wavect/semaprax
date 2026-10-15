use super::{derive, geometry, Fault, Preparation, ID, NAME};
use crate::{ast, hir};

const BASE: &str = r#"module partition_probe;
@id("partition.split") fn split()->Vec<Bytes>{
 let buffer=bytes_zeroed(6usize);
 vec_with_capacity<Bytes>(3usize)
}
@id("partition.main") fn main()->i64{0}
"#;
fn proposed() -> String {
    BASE.replace(
        "vec_with_capacity<Bytes>(3usize)",
        "bytes_partition(buffer,3usize,2usize)",
    )
}
/// A pending-call receiver fixture built on a checked ordinary producer. It is
/// deliberately NOT a fully validated/admitted partition program: the registry,
/// canonical call commit, status inventory and backend are still closed.
pub(super) fn pending() -> (ast::Program, hir::ResolvedProgram) {
    let source = crate::parse(&proposed(), "partition.spx").unwrap();
    let checked = crate::check(BASE, "partition.spx").unwrap();
    let mut program = hir::resolve(&checked).unwrap();
    hir::validate(&program).unwrap();
    let authored = &source.functions[0];
    let ast::ExprKind::Block {
        tail: source_tail, ..
    } = &authored.body.kind
    else {
        panic!("source block")
    };
    let ast::ExprKind::Call { args, .. } = &source_tail.kind else {
        panic!("source call")
    };
    let function = &mut program.functions[0];
    function.span = authored.span;
    function.body.span = authored.body.span;
    let execution = hir::FunctionExecutionId::Monomorphic(function.id.clone());
    let hir::ResolvedExprKind::Block { statements, tail } = &mut function.body.kind else {
        panic!("typed block")
    };
    let hir::ResolvedStatement::Let { binding, .. } = &statements[0] else {
        panic!("producer")
    };
    let owner = hir::ResolvedExpr {
        id: hir::ExpressionId::new(&execution, "body.tail.arg.0"),
        ty: hir::ResolvedType::Bytes,
        ownership: hir::OwnershipMode::Own,
        kind: hir::ResolvedExprKind::Place(hir::Place {
            root: binding.id.clone(),
            projections: Vec::new(),
        }),
        span: args[0].span,
    };
    let scalar = |ordinal: usize, value: u64| hir::ResolvedExpr {
        id: hir::ExpressionId::new(&execution, &format!("body.tail.arg.{ordinal}")),
        ty: hir::ResolvedType::Usize,
        ownership: hir::OwnershipMode::Value,
        kind: hir::ResolvedExprKind::Usize(value),
        span: args[ordinal].span,
    };
    tail.kind = hir::ResolvedExprKind::Call {
        callee: hir::DeclarationId::new(ID),
        type_arguments: Vec::new(),
        instance: None,
        args: vec![owner, scalar(1, 3), scalar(2, 2)],
    };
    tail.span = source_tail.span;
    (source, program)
}
#[test]
fn exact_pending_operands_are_borrowed_and_default_registry_remains_closed() {
    assert_eq!(NAME, "bytes_partition");
    let (source, program) = pending();
    let mut work = 100_000;
    let proof = derive(
        &source,
        &program,
        &source.functions[0],
        &program.functions[0],
        &mut work,
    )
    .unwrap();
    assert_eq!(proof.operands().len(), 3);
    let hir::ResolvedExprKind::Block { tail, statements } = &program.functions[0].body.kind else {
        panic!("block")
    };
    assert!(std::ptr::eq(proof.call, tail.as_ref()));
    let hir::ResolvedStatement::Let { binding, value, .. } = &statements[0] else {
        panic!("let")
    };
    assert!(std::ptr::eq(proof.producer, value));
    assert!(std::ptr::eq(proof.binding, binding));
    assert!(proof.still_held(
        &source,
        &program,
        &source.functions[0],
        &program.functions[0]
    ));
    assert!(!proof.still_held(
        &source.clone(),
        &program,
        &source.functions[0],
        &program.functions[0]
    ));
    assert_eq!(
        proof.prepare(6).unwrap(),
        super::Geometry {
            count: 3,
            width: 2,
            extent: 6
        }
    );
    assert_eq!(
        proof.prepare(8),
        Err(Preparation::Authority(super::Refusal::Identity))
    );
    // The private source factory registers the operation's shape, while the
    // ordinary source and HIR boundaries still refuse its execution.
    assert_eq!(
        crate::byte_ops::by_name(NAME),
        Some(crate::byte_ops::ByteOp::Partition)
    );
    assert_eq!(
        crate::byte_ops::by_id(ID),
        Some(crate::byte_ops::ByteOp::Partition)
    );
    assert!(crate::check(&proposed(), "partition.spx")
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T263"
            && diagnostic.message == "byte partition requires the private checked source profile"));
    assert!(hir::validate(&program).is_err());
}
#[test]
fn source_hir_owner_dimension_callee_and_mode_mutations_refuse_stickily() {
    let (source, program) = pending();
    for mutation in 0..8 {
        let mut changed = program.clone();
        let function = &mut changed.functions[0];
        let hir::ResolvedExprKind::Block { statements, tail } = &mut function.body.kind else {
            panic!("block")
        };
        let hir::ResolvedExprKind::Call { callee, args, .. } = &mut tail.kind else {
            panic!("call")
        };
        match mutation {
            0 => *callee = hir::DeclarationId::new("foreign.partition"),
            1 => args[0].ownership = hir::OwnershipMode::Borrow,
            2 => args[0].ty = hir::ResolvedType::String,
            3 => args[1].kind = hir::ResolvedExprKind::Usize(4),
            4 => args[2].ownership = hir::OwnershipMode::Own,
            5 => args.swap(0, 1),
            6 => {
                let hir::ResolvedExprKind::Place(place) = &mut args[0].kind else {
                    panic!("owner")
                };
                place.root = hir::ValueId::local(
                    &hir::FunctionExecutionId::Monomorphic(function.id.clone()),
                    "body.s1",
                );
            }
            7 => {
                let hir::ResolvedStatement::Let { mutable, .. } = &mut statements[0] else {
                    panic!("let")
                };
                *mutable = true;
            }
            _ => unreachable!(),
        }
        let mut work = 100_000;
        assert!(
            derive(
                &source,
                &changed,
                &source.functions[0],
                &changed.functions[0],
                &mut work
            )
            .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(work, 0, "mutation {mutation}");
    }
    for extra in [
        r#"@id("shadow.zeroed") fn bytes_zeroed()->i64{0}"#,
        r#"@id("shadow.partition") fn bytes_partition()->i64{0}"#,
        r#"@id("core.bytes.zeroed") fn other_zeroed()->i64{0}"#,
        r#"@id("core.bytes.partition") fn other_partition()->i64{0}"#,
    ] {
        let changed = crate::parse(&format!("{}{extra}", proposed()), "partition.spx").unwrap();
        let mut work = 100_000;
        assert!(
            derive(
                &changed,
                &program,
                &changed.functions[0],
                &program.functions[0],
                &mut work
            )
            .is_err(),
            "shadow {extra}"
        );
        assert_eq!(work, 0);
    }
    for (from, to) in [
        ("buffer,3usize", "other,3usize"),
        ("3usize,2usize", "4usize,2usize"),
        ("bytes_zeroed(6usize)", "bytes_zeroed(8usize)"),
    ] {
        let changed = crate::parse(&proposed().replace(from, to), "partition.spx").unwrap();
        let mut work = 100_000;
        assert!(
            derive(
                &changed,
                &program,
                &changed.functions[0],
                &program.functions[0],
                &mut work
            )
            .is_err(),
            "source {to}"
        );
        assert_eq!(work, 0);
    }
}
#[test]
fn exact_and_one_short_work_cover_current_inventory_and_identity_bytes() {
    let (source, program) = pending();
    let initial = 100_000;
    let mut remaining = initial;
    drop(
        derive(
            &source,
            &program,
            &source.functions[0],
            &program.functions[0],
            &mut remaining,
        )
        .unwrap(),
    );
    let used = initial - remaining;
    assert!(used > 0);
    let mut exact = used;
    drop(
        derive(
            &source,
            &program,
            &source.functions[0],
            &program.functions[0],
            &mut exact,
        )
        .unwrap(),
    );
    assert_eq!(exact, 0);
    let mut short = used - 1;
    assert!(derive(
        &source,
        &program,
        &source.functions[0],
        &program.functions[0],
        &mut short
    )
    .is_err());
    assert_eq!(short, 0);
}
#[test]
fn geometry_preserves_error_priority_empty_case_overflow_and_existing_caps() {
    assert_eq!(
        geometry(0, 0, 1).unwrap(),
        super::Geometry {
            count: 0,
            width: 1,
            extent: 0
        }
    );
    assert_eq!(geometry(6, 3, 2).unwrap().extent, 6);
    assert_eq!(
        geometry(0, 0, u64::MAX).unwrap(),
        super::Geometry {
            count: 0,
            width: u64::MAX,
            extent: 0
        }
    );
    assert_eq!(
        geometry(131073, u64::MAX, 0),
        Err(Preparation::Authority(super::Refusal::Range))
    );
    assert_eq!(
        geometry(7, 8193, 1),
        Err(Preparation::Fault(Fault::ExtentMismatch))
    );
    assert_eq!(
        geometry(7, u64::MAX, 2),
        Err(Preparation::Fault(Fault::ExtentOverflow))
    );
    assert_eq!(
        geometry(1, u64::MAX, 0),
        Err(Preparation::Fault(Fault::WidthZero))
    );
    assert_eq!(
        geometry(0, u64::MAX, 2),
        Err(Preparation::Fault(Fault::ExtentOverflow))
    );
    assert_eq!(
        geometry(7, 3, 2),
        Err(Preparation::Fault(Fault::ExtentMismatch))
    );
    assert_eq!(
        geometry(8193, 8193, 1),
        Err(Preparation::Fault(Fault::MetadataCapacity))
    );
    assert_eq!(
        geometry(131073, 1, 131073),
        Err(Preparation::Authority(super::Refusal::Range))
    );
}
