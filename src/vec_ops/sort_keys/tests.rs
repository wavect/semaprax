use super::*;

#[test]
fn borrowed_layout_fields_keep_exact_wire_and_budget_without_owned_pairs() {
    use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
    use crate::test_allocation_observer::measure;
    use sha2::{Digest, Sha256};

    let fields = [
        ("sample.signed", ResolvedType::I64),
        ("sample.short", ResolvedType::I32),
        ("sample.byte", ResolvedType::U8),
        ("sample.size", ResolvedType::Usize),
        ("sample.character", ResolvedType::Char),
        ("sample.float", ResolvedType::F32),
        ("sample.double", ResolvedType::F64),
        ("sample.flag", ResolvedType::Bool),
        ("sample.raw", ResolvedType::Bytes),
        ("sample.text", ResolvedType::String),
    ];
    let element = "nominal:11:sample.item:0:";
    for (family, fields) in [
        ("copy-record", &fields[..8]),
        ("owned-record", &fields[2..]),
    ] {
        let previous = || {
            let fields: Vec<_> = fields
                .iter()
                .map(|(id, ty)| ((*id).to_owned(), ty.identity_key()))
                .collect();
            let mut length = part_len(family)? + part_len(element)? + 21;
            for (id, ty) in &fields {
                length += part_len(id)? + part_len(ty)?;
            }
            if length > MAX_DESCRIPTOR_BYTES || !reserve_active_required(length) {
                return None;
            }
            let mut out = String::with_capacity(length);
            part(&mut out, family);
            part(&mut out, element);
            out.push_str("4096:");
            for (id, ty) in fields {
                part(&mut out, &id);
                part(&mut out, &ty);
            }
            Some(out)
        };
        let current = || {
            layout_key(
                element,
                fields
                    .iter()
                    .map(|(id, ty)| Some((*id, field_identity_key(ty)?))),
                family,
                4096,
            )
        };
        let probe = |build: &dyn Fn() -> Option<String>, limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| build().map(|key| Sha256::digest(key.as_bytes())))
            })
        };
        let ((expected, old), refused, paid, first) = probe(&previous, usize::MAX);
        assert!(expected.is_some() && !refused && first.is_none());
        let ((actual, measured), refused, bytes, first) = probe(&current, paid);
        assert_eq!(actual, expected);
        assert_eq!(bytes, paid);
        assert!(!refused && first.is_none());
        assert_eq!(measured.allocations, 1);
        assert_eq!(measured.allocated, paid);
        assert!(measured.allocated < old.allocated);
        for metrics in [old, measured] {
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        for limit in [paid - 1, 0] {
            let ((_, _), old_refused, old_charge, old_first) = probe(&previous, limit);
            let ((value, metrics), refused, charged, first) = probe(&current, limit);
            assert!(value.is_none() && refused && old_refused);
            assert_eq!(first, old_first);
            assert_eq!(charged, old_charge);
            assert_eq!(metrics.allocated, 0);
        }
    }
}

const SOURCE: &str = "module sort; @id(\"r\") record R { @id(\"serial\") serial:i64, @id(\"key\") key:i64, } @id(\"main\") fn main()->i64 { let v=vec_with_capacity<R>(0usize);let w=vec_sort_by<R>(v,\"key:desc,serial:asc\");0 }";
fn resolved() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "sort.spx").unwrap()).unwrap()
}
fn call(program: &mut crate::hir::ResolvedProgram) -> &mut crate::hir::ResolvedExpr {
    let main = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("block")
    };
    let crate::hir::ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("let")
    };
    value
}
#[test]
fn source_literal_descriptors_canonical_graph_cache_replay_and_selective_prelude() {
    let ast = crate::check(SOURCE, "sort.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        crate::format::canonical(&crate::check(&canonical, "sort.spx").unwrap()),
        canonical
    );
    assert_eq!(
        crate::prelude::selected_for_program(&ast).0,
        crate::prelude::SCHEMA_V15
    );
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    assert!(graph.contains(PREFIX));
    assert!(graph.contains("semaprax.prelude.v15"));
    assert!(crate::graph::verify_json(
        &ast,
        &graph.replace("semaprax.prelude.v15", "semaprax.prelude.v14")
    )
    .is_err());
    let resolved = resolved();
    crate::hir::validate(&resolved).unwrap();
    let wire = crate::cache_codec::encode(&resolved).unwrap();
    let restored: crate::hir::ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&restored).unwrap();
    assert_eq!(wire, crate::cache_codec::encode(&restored).unwrap());
    let legacy = crate::check(
        &SOURCE.replace(
            "vec_sort_by<R>(v,\"key:desc,serial:asc\")",
            "vec_sort<R>(v)",
        ),
        "old.spx",
    )
    .unwrap();
    assert_ne!(
        crate::prelude::selected_for_program(&legacy).0,
        crate::prelude::SCHEMA_V15
    );
}
#[test]
fn malformed_duplicate_unknown_computed_generic_and_primitive_keys_fail_closed() {
    for policy in [
        "",
        "key:up",
        "key:asc,key:desc",
        "unknown:asc",
        "key:asc, serial:desc",
        "key:asc,",
        "key:asc:desc",
    ] {
        let source = SOURCE.replace("key:desc,serial:asc", policy);
        assert!(
            crate::check(&source, "bad.spx")
                .unwrap_err()
                .iter()
                .any(|d| d.code == "SPX-T311"),
            "{policy}"
        );
    }
    for source in [
        SOURCE.replace(
            "\"key:desc,serial:asc\"",
            "string_concat(\"key\",\":desc\")",
        ),
        SOURCE.replace("vec_sort_by<R>", "vec_sort_by<i64>"),
        SOURCE
            .replace("record R {", "record R<T> {")
            .replace("vec_with_capacity<R>", "vec_with_capacity<R<i64>>")
            .replace("vec_sort_by<R>", "vec_sort_by<R<i64>>"),
    ] {
        assert!(crate::check(&source, "bad.spx").is_err());
    }
    let mut source = SOURCE.to_owned();
    source.push_str(&format!(" @id(\"{PREFIX}forged\") fn helper()->i64 {{0}}"));
    assert!(crate::check(&source, "spoof.spx")
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-S113"));
}
#[test]
fn stale_foreign_layout_keys_type_and_malformed_hir_cache_descriptors_refuse() {
    let mut original = resolved();
    let crate::hir::ResolvedExprKind::Call {
        callee,
        type_arguments,
        ..
    } = &call(&mut original).kind
    else {
        panic!("call")
    };
    let id = callee.as_str().to_owned();
    let descriptor = decode(&id).unwrap();
    let element = type_arguments[0].clone();
    assert_eq!(
        replay(&original.declarations, &element, &id),
        Some(descriptor.clone())
    );
    let mut changed = descriptor.clone();
    changed.keys[0].field = DeclarationId::new("foreign");
    let mut layout = descriptor.clone();
    layout.layout.push('x');
    let mut duplicate = descriptor;
    duplicate.keys.push(duplicate.keys[0].clone());
    for forged in [
        format!("{id}trailing"),
        String::from(PREFIX),
        changed.identity().unwrap(),
        layout.identity().unwrap(),
        duplicate.identity().unwrap(),
    ] {
        assert!(replay(&original.declarations, &element, &forged).is_none());
        let mut bad = original.clone();
        let crate::hir::ResolvedExprKind::Call { callee, .. } = &mut call(&mut bad).kind else {
            panic!()
        };
        *callee = DeclarationId::new(forged);
        assert!(crate::hir::validate(&bad).is_err());
        if let Ok(wire) = crate::cache_codec::encode(&bad) {
            if let Ok(restored) = crate::cache_codec::decode::<crate::hir::ResolvedProgram>(&wire) {
                assert!(crate::hir::validate(&restored).is_err());
            }
        }
    }
    let mut bad = original;
    let crate::hir::ResolvedExprKind::Call { type_arguments, .. } = &mut call(&mut bad).kind else {
        panic!()
    };
    type_arguments[0] = ResolvedType::I64;
    assert!(crate::hir::validate(&bad).is_err());
}
#[test]
fn length_bound_canonical_decimal_and_truncation_are_unforgeable() {
    assert!(selector(&"a".repeat(MAX_SELECTOR_BYTES + 1)).is_none());
    assert!(decode(&format!("{PREFIX}01:r0:1:1:fa")).is_none());
    let descriptor = Descriptor {
        element: "r".into(),
        layout: "layout".into(),
        keys: vec![Key {
            field: DeclarationId::new("field"),
            descending: true,
        }],
    };
    let id = descriptor.identity().unwrap();
    assert_eq!(decode(&id), Some(descriptor));
    for end in 0..id.len() {
        assert!(decode(&id[..end]).is_none());
    }
    let huge = Descriptor {
        element: "r".into(),
        layout: "x".repeat(MAX_DESCRIPTOR_BYTES),
        keys: vec![Key {
            field: DeclarationId::new("f"),
            descending: false,
        }],
    };
    assert!(huge.identity().is_none());
}

#[test]
fn canonical_descriptor_decode_retains_the_exact_required_reservation() {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::vec_ops::{field_unique, field_update};

    let sort = Descriptor {
        element: "record".into(),
        layout: "layout:é".into(),
        keys: vec![Key {
            field: DeclarationId::new("field"),
            descending: false,
        }],
    };
    let unique = field_unique::Descriptor {
        element: sort.element.clone(),
        layout: sort.layout.clone(),
        field: sort.keys[0].field.clone(),
    };
    let update = field_update::Descriptor {
        element: sort.element.clone(),
        layout: sort.layout.clone(),
        field: sort.keys[0].field.clone(),
    };
    type Decode = fn(&str) -> Option<String>;
    let fixtures: [(String, Decode); 3] = [
        (sort.identity().unwrap(), |wire| decode(wire)?.identity()),
        (unique.identity().unwrap(), |wire| {
            field_unique::decode(wire)?.identity()
        }),
        (update.identity().unwrap(), |wire| {
            field_update::decode(wire)?.identity()
        }),
    ];

    for (wire, roundtrip) in fixtures {
        assert_eq!(roundtrip(&wire).as_deref(), Some(wire.as_str()));

        // Each roundtrip reserves once for validation and once for writing
        // the returned wire. A one-short second request must remain sticky.
        let length = wire.len();
        let (exact, refused, paid, first) =
            with_limit_usage_refusal(2 * length, || roundtrip(&wire));
        assert_eq!(exact.as_deref(), Some(wire.as_str()));
        assert!(!refused);
        assert_eq!(paid, 2 * length);
        assert!(first.is_none());

        let (short, refused, paid, first) =
            with_limit_usage_refusal(2 * length - 1, || roundtrip(&wire));
        assert!(short.is_none() && refused);
        assert_eq!(paid, length);
        assert_eq!(first.unwrap().requested, length);

        let (before_copy, refused, paid, first) =
            with_limit_usage_refusal(length - 1, || roundtrip(&wire));
        assert!(before_copy.is_none() && refused);
        assert_eq!(paid, 0);
        assert_eq!(first.unwrap().requested, length);

        let malformed = wire.replacen("6:record", "06:record", 1);
        assert_ne!(malformed, wire);
        let (value, refused, paid, first) = with_limit_usage_refusal(0, || roundtrip(&malformed));
        assert!(value.is_none());
        assert!(!refused);
        assert_eq!(paid, 0);
        assert!(first.is_none());
    }
}
