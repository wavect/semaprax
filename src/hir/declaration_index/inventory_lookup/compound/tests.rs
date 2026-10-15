use super::*;
use crate::hir::{DeclarationId, DeclarationIndex, TypeFacts};
use crate::test_allocation_observer::measure;
use std::cell::Cell;
fn nominal(name: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(name),
        arguments,
    }
}
fn cases() -> Vec<ResolvedType> {
    let mut types = vec![
        ResolvedType::Never,
        ResolvedType::Unit,
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::String,
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::StringMap,
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::MutFunctionI64,
        ResolvedType::ArrayU8(0),
        ResolvedType::ArrayU8(10),
        ResolvedType::ArrayU8(u32::MAX),
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("é\0:owner"),
            index: 19,
        },
        nominal("", vec![]),
        nominal(
            "é✓\0:row",
            vec![ResolvedType::I64, ResolvedType::ArrayU8(8192)],
        ),
        ResolvedType::Function {
            parameters: vec![],
            result: Box::new(ResolvedType::Never),
        },
        ResolvedType::Function {
            parameters: vec![
                ResolvedType::String,
                nominal("row", vec![ResolvedType::Bool]),
            ],
            result: Box::new(ResolvedType::Bytes),
        },
    ];
    types.push(nominal(
        "outer:0:",
        vec![types[26].clone(), nominal("inner", vec![ResolvedType::Str])],
    ));
    types.push(ResolvedType::TypeParameter {
        owner: DeclarationId::new(""),
        index: u32::MAX,
    });
    types
}
#[test]
fn complete_paid_compound_cursor_keeps_all_original_type_bytes_cache_facts_and_drop() {
    let types = cases();
    let mut index = DeclarationIndex::default();
    for (n, ty) in types.iter().enumerate() {
        index.type_facts_by_id.insert(
            ty.identity_key(),
            TypeFacts {
                copy: n % 2 == 0,
                contains_resource: true,
                sized: false,
                needs_drop: n % 3 == 0,
                layout_key: "forged-full-layout".to_owned(),
            },
        );
    }
    for ty in &types {
        let expected_key = ty.identity_key();
        let expected = index.type_facts_by_id.get(&expected_key);
        let plan = Plan::new(ty, &|_| true).unwrap();
        let mut bytes = plan.bytes();
        let actual: Vec<u8> = std::iter::from_fn(|| bytes.next(&|_| true)).collect();
        assert_eq!(actual, expected_key.as_bytes());
        let (found, observation) =
            measure(|| index.inventory_probes().cached_compound(ty, &mut |_| true));
        assert_eq!(found.unwrap().unwrap(), expected);
        assert!(!observation.invalid);
        assert_eq!(observation.allocated, 0);
        assert_eq!(observation.live, 0);
    }
}

#[test]
fn every_compound_work_refusal_withholds_full_cache_value_and_runs_no_later_callback() {
    let ty = nominal(
        "é\0suffix",
        vec![ResolvedType::Function {
            parameters: vec![
                ResolvedType::String,
                nominal("inner", vec![ResolvedType::Bool]),
            ],
            result: Box::new(ResolvedType::Bytes),
        }],
    );
    let mut index = DeclarationIndex::default();
    index.type_facts_by_id.insert(
        ty.identity_key(),
        TypeFacts {
            copy: false,
            contains_resource: true,
            sized: false,
            needs_drop: false,
            layout_key: "complete-hostile-cache-value".to_owned(),
        },
    );
    let mut total = 0usize;
    let expected = index
        .inventory_probes()
        .cached_compound(&ty, &mut |units| {
            total += units;
            true
        })
        .unwrap();
    assert!(expected.unwrap().is_some());
    // Check each actual nonzero cumulative-work boundary, including the first
    // complete exact success. No optimization-decline value can mask refusal.
    for limit in 0..=total {
        let mut remaining = limit;
        let mut refused = false;
        let mut late = 0;
        let (result, observation) = measure(|| {
            index.inventory_probes().cached_compound(&ty, &mut |units| {
                if refused {
                    late += 1;
                    return false;
                }
                match remaining.checked_sub(units) {
                    Some(value) => {
                        remaining = value;
                        true
                    }
                    None => {
                        refused = true;
                        false
                    }
                }
            })
        });
        assert_eq!(late, 0);
        assert!(!observation.invalid && observation.allocated == 0 && observation.live == 0);
        if limit < total {
            assert!(refused && result.is_err());
        } else {
            assert_eq!(result.unwrap(), expected);
        }
    }
    let mut calls = 0;
    assert!(DeclarationIndex::default()
        .inventory_probes()
        .cached_compound(&ty, &mut |_| {
            calls += 1;
            false
        })
        .is_err());
    assert_eq!(calls, 1);
}

#[test]
fn compound_optimization_decline_remains_distinct_from_work_refusal_and_keeps_saved_sources() {
    let wide = nominal("wide", vec![ResolvedType::I64; NODES + 1]);
    let index = DeclarationIndex::default();
    let used = Cell::new(0usize);
    assert!(Plan::new(&wide, &|n| {
        used.set(used.get() + n);
        true
    })
    .is_none());
    assert!(used.get() > 0);
    assert_eq!(
        index
            .inventory_probes()
            .cached_compound(&wide, &mut |_| true)
            .unwrap(),
        None
    );
    let text = nominal(&"x".repeat(MAX_BYTES + 1), Vec::new());
    assert_eq!(
        index
            .inventory_probes()
            .cached_compound(&text, &mut |_| true)
            .unwrap(),
        None
    );
    assert_eq!(
        ci_str!("../../flags/nominal_key.rs"),
        ci_str!("../../../../../docs/evidence/schema-summary/nominal-key.rs.before")
    );
    assert_eq!(
        ci_str!("../../flags/nominal_key/compound.rs"),
        ci_str!("../../../../../docs/evidence/schema-summary/nominal-compound.rs.before")
    );
}
