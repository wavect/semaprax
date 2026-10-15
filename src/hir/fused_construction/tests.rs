//! Owning definitions for the typed `FusedConstructionV1` derivation.
//!
//! Ordinary `check`/`resolve` keep SPX-T267 above 32 rows, so no checked HIR
//! exists for a selected count yet. `rescaled` resolves the genuine 32-row
//! program and rewrites only its count literals; every count literal in the
//! fixture text is right-aligned to four columns so both texts share every
//! enclosing span. That stand-in is mutated HIR, not a checked 33..4096 program
//! and never an authoritative body lease.
use super::*;

type Forgery = (&'static str, fn(&mut ResolvedProgram));
use crate::hir::{DeclarationId, ResolvedFieldInitializer};
mod join;

pub(super) const ORIGINAL: &str =
    include_str!("../../../tests/fixtures/record-traversal-maximum.spx");

pub(super) fn text(count: u32) -> String {
    ORIGINAL.replace("4096", &format!("{count:>4}"))
}

fn checked(count: u32) -> ResolvedProgram {
    let source = crate::check(&text(count), "fused-construction.spx").unwrap();
    crate::hir::resolve(&source).unwrap()
}

fn rescale(e: &mut E, from: u64, to: u64) {
    match &mut e.kind {
        K::Int(value) if u64::try_from(*value) == Ok(from) => *value = i64::try_from(to).unwrap(),
        K::Usize(value) if *value == from => *value = to,
        K::Binary { left, right, .. } => {
            rescale(left, from, to);
            rescale(right, from, to);
        }
        K::Call { args, .. } => args.iter_mut().for_each(|arg| rescale(arg, from, to)),
        K::If { condition, .. } => rescale(condition, from, to),
        _ => {}
    }
}

/// Genuine 32-row HIR with the capacity, bound and tail count literals moved.
pub(super) fn rescaled(count: u32) -> ResolvedProgram {
    let mut program = checked(32);
    let (from, to) = (32, u64::from(count));
    let K::Block { statements, tail } = &mut program.functions[0].body.kind else {
        panic!("body")
    };
    rescale(tail, from, to);
    let [S::Let {
        value: allocation, ..
    }, _, S::While { condition, .. }, ..] = statements.as_mut_slice()
    else {
        panic!("prefix")
    };
    rescale(allocation, from, to);
    rescale(condition, from, to);
    program
}

pub(super) fn block(program: &mut ResolvedProgram) -> &mut Vec<S> {
    let K::Block { statements, .. } = &mut program.functions[0].body.kind else {
        panic!("body")
    };
    statements
}
pub(super) fn condition(program: &mut ResolvedProgram) -> &mut E {
    let S::While { condition, .. } = &mut block(program)[2] else {
        panic!("loop")
    };
    condition
}
fn rows(program: &mut ResolvedProgram) -> &mut Vec<S> {
    let S::While { body, .. } = &mut block(program)[2] else {
        panic!("loop")
    };
    let K::Block { statements, .. } = &mut body.kind else {
        panic!("rows")
    };
    statements
}
pub(super) fn fields(program: &mut ResolvedProgram) -> &mut Vec<ResolvedFieldInitializer> {
    let S::Let { value, .. } = &mut rows(program)[0] else {
        panic!("row")
    };
    let K::ConstructRecord { fields, .. } = &mut value.kind else {
        panic!("record")
    };
    fields
}
pub(super) fn producer(program: &mut ResolvedProgram) -> &mut E {
    &mut fields(program)[2].value
}
pub(super) fn set_width(program: &mut ResolvedProgram, width: u64) {
    let K::Call { args, .. } = &mut producer(program).kind else {
        panic!("producer")
    };
    let K::Usize(value) = &mut args[0].kind else {
        panic!("width")
    };
    *value = width;
}
fn record(program: &mut ResolvedProgram) -> &mut ResolvedTypeDeclaration {
    program
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "app.leaf.row")
        .expect("row record")
}
fn suffix_value<'p>(program: &'p mut ResolvedProgram, name: &str) -> &'p mut S {
    block(program)
        .iter_mut()
        .find(|s| matches!(s, S::Let { binding, .. } if binding.name == name))
        .expect("suffix binding")
}

#[test]
fn legacy_counts_stay_ordinary_and_selected_counts_keep_t267() {
    for count in [1, 8, 32] {
        let program = checked(count);
        crate::hir::validate(&program).unwrap();
        assert_eq!(derive(&program).err(), Some(Refusal::Legacy));
        let ordinary = crate::hir::analyze_byte_data_capacity(&program).unwrap();
        assert_eq!(
            ordinary.function("app.main").unwrap().bytes_copy_sites,
            count
        );
    }
    for count in [33, 4096] {
        assert!(crate::check(&text(count), "fused-construction-refused.spx")
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-T267"));
        // Closed default: the ordinary capacity rebuild still refuses even
        // where the typed selection succeeds.
        let program = rescaled(count);
        assert!(derive(&program).is_ok());
        assert!(crate::hir::analyze_byte_data_capacity(&program).is_err());
        assert!(crate::hir::validate(&program).is_err());
    }
}

#[test]
fn selected_counts_bind_exact_typed_sites_and_shape_digest() {
    for count in [33, 64, 1000, 4096] {
        let program = rescaled(count);
        let selection = derive(&program).unwrap();
        assert_eq!(selection.rows(), count);
        assert_eq!(selection.width(), 3);
        assert_eq!(selection.extent(), u64::from(count) * 3);
        assert_eq!(
            selection.order(),
            [Role::Induction, Role::Text, Role::Payload]
        );
        assert!(matches!(&selection.producer().kind,
            K::Call { callee, .. } if callee.as_str() == crate::byte_ops::ZEROED_ID));
        assert!(matches!(&selection.push().kind,
            K::Call { callee, .. } if callee.as_str() == crate::vec_ops::PUSH_ID));
        assert_ne!(selection.producer().id, selection.push().id);
        assert!(selection.is_loop(selection.condition(), selection.body()));
        assert!(!selection.is_loop(selection.body(), selection.condition()));
        assert!(selection.still_held(&program));
        assert!(std::ptr::eq(selection.program(), &program));
        assert!(matches!(&selection.text().kind, K::String(text) if text == "é\u{0}"));
        let copy = program.clone();
        assert!(!selection.still_held(&copy));
        let digest = selection.shape_digest().unwrap();
        assert_eq!(selection.shape_digest(), Some(digest));
        // An equal copy has the same original shape but is not the held program.
        assert_eq!(derive(&copy).unwrap().shape_digest(), Some(digest));
        if count != 33 {
            let other = rescaled(33);
            assert_ne!(derive(&other).unwrap().shape_digest(), Some(digest));
        }
    }
}

#[test]
fn row_and_extent_bounds_are_inclusive() {
    assert_eq!(derive(&rescaled(4097)).err(), Some(Refusal::Rows));
    for (count, width, admitted) in [
        (4096, 32, true),
        (4096, 33, false),
        (33, 3971, true),
        (33, 3972, false),
        (64, 0, false),
    ] {
        let mut program = rescaled(count);
        set_width(&mut program, width);
        match derive(&program) {
            Ok(selection) => {
                assert!(admitted, "{count}x{width}");
                assert_eq!(selection.extent(), u64::from(count) * width);
                assert!(selection.extent() <= MAX_EXTENT);
            }
            Err(refusal) => {
                assert!(!admitted, "{count}x{width}");
                assert_eq!(refusal, Refusal::Extent);
            }
        }
    }
    // The capacity and the bound must name one count.
    let mut program = rescaled(64);
    let S::Let { value, .. } = &mut block(&mut program)[0] else {
        panic!("allocation")
    };
    rescale(value, 64, 65);
    assert!(matches!(derive(&program), Err(Refusal::Shape(_))));
}

#[test]
fn decoded_hir_forgeries_refuse_and_original_shape_is_unchanged() {
    let original = rescaled(64);
    let digest = derive(&original).unwrap().shape_digest().unwrap();
    let wire = crate::cache_codec::encode(&original).unwrap();
    let decoded = || -> ResolvedProgram { crate::cache_codec::decode(&wire).unwrap() };
    assert_eq!(derive(&decoded()).unwrap().shape_digest(), Some(digest));
    let forgeries: [Forgery; 15] = [
        ("polarity", |p| {
            if let K::Binary { op, .. } = &mut condition(p).kind {
                *op = BinaryOp::Le;
            }
        }),
        ("operand order", |p| {
            if let K::Binary { left, right, .. } = &mut condition(p).kind {
                std::mem::swap(left, right);
            }
        }),
        ("latch", |p| {
            let S::Assign { value, .. } = &mut rows(p)[2] else {
                panic!("latch")
            };
            rescale(value, 1, 2);
        }),
        ("start", |p| {
            let S::Let { value, .. } = &mut block(p)[1] else {
                panic!("start")
            };
            value.kind = K::Int(1);
        }),
        ("row ownership", |p| {
            let S::Let { binding, .. } = &mut rows(p)[0] else {
                panic!("row")
            };
            binding.ownership = O::Value;
        }),
        ("destination spelling", |p| {
            let S::Assign { binding, .. } = &mut rows(p)[1] else {
                panic!("push")
            };
            binding.name.push('x');
        }),
        ("producer builtin", |p| {
            if let K::Call { callee, .. } = &mut producer(p).kind {
                *callee = DeclarationId::new(crate::byte_ops::ZEROED_BOUNDED_ID);
            }
        }),
        ("producer identity", |p| {
            let S::Assign { value, .. } = &mut rows(p)[1] else {
                panic!("push")
            };
            let id = value.id.clone();
            producer(p).id = id;
        }),
        ("duplicate record", |p| {
            let record = record(p).clone();
            p.types.push(record);
        }),
        ("entrypoint", |p| {
            p.entrypoint = DeclarationId::new("app.other")
        }),
        ("field index", |p| {
            if let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &mut record(p).kind
            {
                fields.swap(0, 1);
            }
        }),
        ("field order", |p| fields(p).swap(1, 2)),
        ("shared value", |p| {
            let S::Let {
                binding: counter, ..
            } = &block(p)[1]
            else {
                panic!("induction")
            };
            let id = counter.id.clone();
            let S::Let { binding, .. } = &mut block(p)[0] else {
                panic!("destination")
            };
            binding.id = id;
        }),
        ("suffix rebinding", |p| {
            let S::Let {
                binding: vector, ..
            } = &block(p)[0]
            else {
                panic!("destination")
            };
            let id = vector.id.clone();
            let S::Let { binding, .. } = suffix_value(p, "rows") else {
                panic!("alias")
            };
            binding.id = id;
        }),
        ("suffix clone", |p| {
            let S::Let { value, .. } = suffix_value(p, "renewed") else {
                panic!("renewed")
            };
            if let K::Call { callee, .. } = &mut value.kind {
                *callee = DeclarationId::new(crate::vec_ops::CLONE_AT_ID);
            }
        }),
    ];
    for (name, forge) in forgeries {
        let mut forged = decoded();
        forge(&mut forged);
        assert_ne!(forged, original, "{name} changed nothing");
        assert!(derive(&forged).is_err(), "{name} was selected");
        assert_eq!(
            derive(&original).unwrap().shape_digest(),
            Some(digest),
            "{name} reached the original"
        );
    }
}

/// Rewrite every occurrence of one identity in the cache wire and decode it,
/// so the decoder recomputes each `ValueId` fingerprint: a consistent rename.
pub(super) fn rewire(program: &ResolvedProgram, from: &str, to: &str) -> ResolvedProgram {
    assert_eq!(from.len(), to.len());
    assert_ne!(from, to);
    let mut wire = crate::cache_codec::encode(program).unwrap();
    let (mut index, mut replaced) = (0, 0);
    while index + from.len() <= wire.len() {
        if &wire[index..index + from.len()] == from.as_bytes() {
            wire[index..index + from.len()].copy_from_slice(to.as_bytes());
            replaced += 1;
            index += from.len();
        } else {
            index += 1;
        }
    }
    assert!(replaced > 0, "{from} is absent from the wire");
    crate::cache_codec::decode(&wire).unwrap()
}

#[test]
fn decoded_consistent_renames_and_unique_foreign_identities_refuse() {
    let original = rescaled(64);
    let selection = derive(&original).unwrap();
    let vector = selection.vector().id.as_str().to_owned();
    let counter = selection.counter().id.as_str().to_owned();
    let producer = selection.producer().id.as_str().to_owned();
    let digest = selection.shape_digest().unwrap();
    assert!(vector.ends_with("body.s0") && counter.ends_with("body.s1"));
    for (name, from, to) in [
        (
            "renamed Vec",
            vector.clone(),
            vector.replace("body.s0", "body.q0"),
        ),
        (
            "renamed induction",
            counter.clone(),
            counter.replace("body.s1", "body.q1"),
        ),
        (
            "unique foreign producer",
            producer.clone(),
            format!("{}x", &producer[..producer.len() - 1]),
        ),
    ] {
        let forged = rewire(&original, &from, &to);
        assert_ne!(forged, original, "{name}");
        assert!(
            matches!(derive(&forged), Err(Refusal::Forged(_))),
            "{name} was selected"
        );
    }
    assert_eq!(derive(&original).unwrap().shape_digest(), Some(digest));
}

#[test]
fn vec_constructor_provenance_is_authenticated_not_walked_as_a_block() {
    let constructor = crate::hir::resolve(
        &crate::check(
            "module constructor; @id(\"main\") fn main()->i64 {let rows=vec_of<i64>(19,23); if vec_len<i64>(rows)==2usize {42}else{0}}",
            "fused-vec-of.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let K::Block { statements, .. } = &constructor.functions[0].body.kind else {
        panic!("constructor body")
    };
    let S::Let {
        value: expansion, ..
    } = &statements[0]
    else {
        panic!("constructor let")
    };
    assert!(crate::hir::vec_constructor::expansion(expansion).is_some());
    let mut program = rescaled(64);
    let S::Let { value, .. } = suffix_value(&mut program, "rows") else {
        panic!("alias")
    };
    let alias = value.clone();
    // An authentic expansion is still an allocation outside the closed domain.
    *value = expansion.clone();
    assert!(matches!(derive(&program), Err(Refusal::Suffix(_))));
    // A disagreeing expansion is forged, not an ordinary block.
    let S::Let { value, .. } = suffix_value(&mut program, "rows") else {
        panic!("alias")
    };
    let K::VecConstructor { count, .. } = &mut value.kind else {
        panic!("expansion")
    };
    *count += 1;
    assert_eq!(
        derive(&program).err(),
        Some(Refusal::Forged("Vec constructor expansion disagrees"))
    );
    let S::Let { value, .. } = suffix_value(&mut program, "rows") else {
        panic!("alias")
    };
    *value = alias;
    assert!(derive(&program).is_ok());
    // The destination itself may not be a constructor expansion.
    let S::Let { value, .. } = &mut block(&mut program)[0] else {
        panic!("destination")
    };
    *value = expansion.clone();
    assert!(derive(&program).is_err());
}

#[test]
fn census_scratch_is_charged_exactly_and_one_short_refuses() {
    let program = rescaled(64);
    let census = suffix::scratch_bytes();
    let (result, refused, used) =
        crate::bounded_output::with_limit_usage(census - 1, || derive(&program).map(|s| s.rows()));
    assert_eq!(result, Err(Refusal::Scratch));
    assert!(refused);
    assert_eq!(used, 0);
    let (result, refused, bytes) =
        crate::bounded_output::with_limit_usage(usize::MAX, || derive(&program).map(|s| s.rows()));
    assert_eq!(result, Ok(64));
    assert!(!refused);
    assert!(
        bytes > census,
        "the borrowed traversal replay is charged too"
    );
    let (result, refused, used) =
        crate::bounded_output::with_limit_usage(bytes, || derive(&program).map(|s| s.rows()));
    assert_eq!((result, refused, used), (Ok(64), false, bytes));
    let (result, refused, _) =
        crate::bounded_output::with_limit_usage(bytes - 1, || derive(&program).map(|s| s.rows()));
    assert!(result.is_err());
    assert!(refused);
}
