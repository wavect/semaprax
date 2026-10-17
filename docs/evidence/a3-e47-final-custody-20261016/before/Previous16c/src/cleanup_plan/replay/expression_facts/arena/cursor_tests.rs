use super::*;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module cursor.facts;
@id("cursor.identity") fn identity<T>(value: T) -> T { value }
@id("cursor.add") fn add(left: i64, right: i64) -> i64 { left + right }
@id("cursor.main") fn main() -> i64 requires true ensures result >= 0 {
    let value = add(1, 2);
    if value > 0 { identity<i64>(value) } else { 0 }
}"#,
            "cursor-facts.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn build(function: &ResolvedFunction, old: bool) -> Result<Arena<'_>, Diagnostic> {
    let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
    let mut budget = ReplayBudget::unobserved();
    if old {
        before_cursor::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical),
            &mut budget,
        )
    } else {
        super::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical),
            &mut budget,
        )
    }
}
fn compare(function: &ResolvedFunction) {
    match (build(function, false), build(function, true)) {
        (Ok(actual), Ok(original)) => {
            assert!(actual.balanced());
            assert!(original.balanced());
            let actual = actual.entries().collect::<Vec<_>>();
            let original = original.entries().collect::<Vec<_>>();
            assert_eq!(actual.len(), original.len());
            for ((key, fact), (old_key, old_fact)) in actual.into_iter().zip(original) {
                assert_eq!(key, old_key);
                match (fact, old_fact) {
                    (Some(fact), Some(old)) => {
                        assert_eq!(fact.callee, old.callee);
                        assert_eq!(fact.instance, old.instance);
                        assert_eq!(fact.arguments, old.arguments);
                        assert_eq!(fact.type_arguments, old.type_arguments);
                    }
                    (None, None) => {}
                    _ => panic!("complete fact presence changed"),
                }
            }
        }
        (Err(actual), Err(original)) => assert_eq!(actual, original),
        _ => panic!("complete original collector verdict changed"),
    }
}
#[test]
fn actual_full_fact_owners_and_duplicate_error_match_the_complete_original_entry() {
    let program = program();
    for function in &program.functions {
        compare(function);
    }
    let mut main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "cursor.main")
        .unwrap()
        .clone();
    let duplicate = main.requires[0].clone();
    main.ensures.push(duplicate);
    assert!(build(&main, true).is_err());
    compare(&main);
}
#[test]
fn original_514_frame_boundary_is_unchanged_on_raw_collector_census_inputs() {
    // These deliberately assembled census inputs test the structural cursor,
    // not source/HIR admission or execution of an over-depth program.
    let program = crate::hir::resolve(
        &crate::check(
            "module cursor.depth; @id(\"cursor.depth.main\") fn main() -> i64 { 0 }",
            "cursor-depth.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let template = &program.functions[0];
    let ResolvedExprKind::Block { tail, .. } = &template.body.kind else {
        panic!("source block");
    };
    let mut root = tail.as_ref().clone();
    root.id = ExpressionId::from_owned("depth.1".into());
    for depth in 1..=515 {
        if depth >= 513 {
            let mut function = template.clone();
            function.body = root.clone();
            assert_eq!(build(&function, true).is_ok(), depth <= 514);
            compare(&function);
        }
        let mut layer = template.body.clone();
        layer.id = ExpressionId::from_owned(format!("depth.{}", depth + 1));
        let ResolvedExprKind::Block { tail, .. } = &mut layer.kind else {
            unreachable!();
        };
        **tail = root;
        root = layer;
    }
}

#[test]
fn complete_original_capacity_entry_and_producer_are_byte_bound() {
    use sha2::{Digest, Sha256};
    let sha = |text: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
        )
    };
    let capacity_old = include_str!(
        "../../../../../docs/evidence/expression-fact-arena-cursor-v1/capacity.before.rs.txt"
    );
    let producer_old = include_str!(
        "../../../../../docs/evidence/expression-fact-arena-cursor-v1/producer.before.rs.txt"
    );
    let arena_old = include_str!(
        "../../../../../docs/evidence/expression-fact-arena-cursor-v1/arena.before.rs.txt"
    );
    assert_eq!(
        sha(capacity_old),
        "ee3bbd81a54e829a6859f5636ed272bc8a9f28177f4f8f4cf99b590243c967bd"
    );
    assert_eq!(
        sha(producer_old),
        "8186fb1a71dff699b07640e6f545b6a725ec1803ce6d7c83bad70e94c8b95441"
    );
    assert_eq!(
        sha(arena_old),
        "836f873c4a23a62dcb52eb1768b08f36581c7212c1b047cd06561791961dfb4b"
    );
    let capacity_now = include_str!("../capacity.rs");
    assert!(capacity_now.starts_with(capacity_old));
    let owner_start = capacity_now.find("/// One funded function owns").unwrap();
    let owner_end = capacity_now
        .find("\n#[cfg(test)]\n#[path = \"capacity/cursor_tests.rs\"]")
        .unwrap();
    assert_eq!(
        sha(&capacity_now[owner_start..owner_end]),
        "f1404ed792ce1a02121eaf193cdecd8fd4f37f806371566c56e5ee564ad31a17"
    );
    assert_eq!(include_str!("producer_before_cursor.rs"), producer_old);
    let old_entry = include_str!("before_cursor.rs");
    assert_eq!(
        old_entry
            .strip_prefix("use super::producer_before_cursor as producer;\nuse super::*;\n\n")
            .unwrap(),
        &arena_old[arena_old.find("pub(super) fn build<").unwrap()..]
    );
    let arena_now = include_str!("../arena.rs");
    let registration = "\n#[cfg(test)]\n#[path = \"arena/cursor_tests.rs\"]\nmod cursor_tests;\n#[cfg(test)]\n#[path = \"arena/producer_before_cursor.rs\"]\nmod producer_before_cursor;\n\n#[cfg(test)]\n#[path = \"arena/before_cursor.rs\"]\nmod before_cursor;\n";
    assert_eq!(arena_now.matches(registration).count(), 1);
    assert_eq!(
        arena_now.replace(registration, "").replace(
            "capacity::ArenaCapacity::new(function, payer)",
            "capacity::FactCapacity::with_payer(function, payer)"
        ),
        arena_old
    );
    let producer_now = include_str!("producer.rs");
    assert_eq!(
        sha(producer_now),
        "98ff536df6532dc0d9f0b52c89533a9cdf21506789da8e4c9eb43adec339db48"
    );
    let inverse: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/evidence/expression-fact-arena-cursor-v1/producer-inverses.json"
    ))
    .unwrap();
    let mut restored = producer_now.to_owned();
    for change in inverse.as_array().unwrap() {
        let current = change["current"].as_str().unwrap();
        let previous = change["previous"].as_str().unwrap();
        assert_eq!(restored.matches(current).count(), 1);
        restored = restored.replacen(current, previous, 1);
    }
    assert_eq!(restored, producer_old);
}

#[test]
fn shallow_actual_fact_owner_releases_all_backing_and_costs_less_than_the_full_original() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "cursor.main")
        .unwrap();
    let mut charges = Vec::new();
    for old in [true, false] {
        let (metrics, refused, charged) =
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                crate::test_allocation_observer::measure(|| drop(build(function, old).unwrap())).1
            });
        assert!(!refused && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= charged && metrics.highwater <= charged);
        charges.push(charged);
    }
    assert!(charges[1] < charges[0], "actual shallow collector billing");
}
