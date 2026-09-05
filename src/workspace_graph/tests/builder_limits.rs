use super::*;

fn minimum_successful_builder_limit(sources: &[WorkspaceSource]) -> usize {
    assert!(build_owned_with_builder_limit(sources.to_vec(), MAX_BUILDER_BYTES).is_ok());
    let mut low = 0usize;
    let mut high = MAX_BUILDER_BYTES;
    while low < high {
        let middle = low + (high - low) / 2;
        if build_owned_with_builder_limit(sources.to_vec(), middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[test]
#[should_panic(
    expected = "private Workspace Semantic Graph builder limit cannot exceed the production maximum"
)]
fn private_builder_limit_cannot_widen_the_production_cap() {
    let _ = build_owned_with_builder_limit(Vec::new(), MAX_BUILDER_BYTES + 1);
}

fn assert_exact_builder_limit_error(error: &[Diagnostic], limit: usize) {
    assert_eq!(error[0].code, "SPX-G171");
    assert_eq!(
        error[0].message,
        format!("Workspace Semantic Graph `builder_bytes` exceeds {limit}")
    );
}

#[test]
fn all_four_generic_materializations_have_an_exact_minimum_builder_limit() {
    let app = canonical_source(
        "generic-limit/app.spx",
        r#"
module generic_limit.app;
@id("generic.limit.first") fn first<T, U>(left: T, right: U) -> T { left }
@id("generic.limit.main") fn main() -> i64 {
    let ii = first<i64, i64>(1, 2);
    let ib = first<i64, bool>(ii, true);
    let bi = first<bool, i64>(false, ib);
    if first<bool, bool>(bi, true) { ib } else { 0 }
}
"#,
    );
    let leaf = canonical_source(
        "generic-limit/leaf.spx",
        "module generic_limit.leaf;\n@id(\"generic.limit.leaf\") fn leaf() -> i64 { 0 }\n",
    );
    let sources = vec![app, leaf];
    let minimum = minimum_successful_builder_limit(&sources);
    assert!(minimum > 0);
    let first = build_owned_with_builder_limit(sources.clone(), minimum).unwrap();
    let second = build_owned_with_builder_limit(sources.clone(), minimum).unwrap();
    assert_eq!(first.edges, second.edges);
    assert_eq!(first.hir.declarations, second.hir.declarations);

    let error = match build_owned_with_builder_limit(sources, minimum - 1) {
        Ok(_) => panic!("minimum minus one must fail"),
        Err(error) => error,
    };
    assert_exact_builder_limit_error(&error, minimum - 1);
}

#[test]
fn late_module_work_has_an_exact_combined_minimum_builder_limit() {
    let provider = canonical_source(
        "late/a_provider.spx",
        r#"
module late.provider;
@id("late.value") fn value() -> i64 { 1 }
"#,
    );
    let minimal = canonical_source(
        "late/z_consumer.spx",
        r#"
module late.consumer;
@id("late.main") fn main() -> i64 { 0 }
"#,
    );
    let mut consumer = String::from(
        r#"
module late.consumer;
use function @id("late.value") from late.provider as value;
@id("late.main") fn main() -> i64 {
"#,
    );
    for index in 0..96 {
        consumer.push_str(&format!("let value_{index} = value();\n"));
    }
    consumer.push_str("0\n}\n");
    let consumer = canonical_source("late/z_consumer.spx", &consumer);

    let base = vec![provider.clone(), minimal];
    let combined = vec![provider, consumer];
    let base_minimum = minimum_successful_builder_limit(&base);
    let combined_minimum = minimum_successful_builder_limit(&combined);
    assert!(base_minimum < combined_minimum);
    assert!(build_owned_with_builder_limit(base, combined_minimum - 1).is_ok());

    let exact = build_owned_with_builder_limit(combined.clone(), combined_minimum).unwrap();
    let replay = build_owned_with_builder_limit(combined.clone(), combined_minimum).unwrap();
    assert_eq!(exact.edges, replay.edges);
    assert_eq!(exact.hir.declarations, replay.hir.declarations);
    let error = match build_owned_with_builder_limit(combined, combined_minimum - 1) {
        Ok(_) => panic!("late module must consume the final builder byte"),
        Err(error) => error,
    };
    assert_exact_builder_limit_error(&error, combined_minimum - 1);
}
