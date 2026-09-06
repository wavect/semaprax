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

fn stub_charge_provider(statements: usize) -> WorkspaceSource {
    let mut provider =
        String::from("\nmodule stub.provider;\n@id(\"stub.wide\") fn wide(seed: i64) -> i64 {\n");
    for index in 0..statements {
        provider.push_str(&format!(
            "let step_{index} = seed + {index} * 3 - {index} / 2;\n"
        ));
    }
    provider.push_str("seed\n}\n");
    canonical_source("stub/provider.spx", &provider)
}

fn stub_charge_prebound(sources: &[WorkspaceSource], module: &str) -> usize {
    let programs = parsed_sources(sources);
    let authored = index_authored(&programs).expect("fixture identities are unique");
    let subject = programs
        .iter()
        .find(|program| program.module == module)
        .expect("fixture declares the subject module");
    synthetic_builder_bytes(subject, &authored, &programs)
        .expect("fixture stays inside the builder budget")
        .raw_clone_and_hir
}

/// A consumer projects an imported function as a stub: the rewritten
/// signature, no contract, and the return type's default body. Growing the
/// provider's body must therefore not grow the consumer's pre-bound at the
/// resolved-structure rate, only by the transient clone the projection makes
/// and discards. Charging the imported body as resolved structure made every
/// importing module a second full copy of its provider.
#[test]
fn imported_function_bodies_are_not_charged_as_resolved_structure() {
    let consumer = canonical_source(
        "stub/consumer.spx",
        r#"
module stub.consumer;
use function @id("stub.wide") from stub.provider as wide;
@id("stub.main") fn main() -> i64 { wide(1) }
"#,
    );
    let small = vec![stub_charge_provider(4), consumer.clone()];
    let large = vec![stub_charge_provider(64), consumer];

    let definition_growth = stub_charge_prebound(&large, "stub.provider")
        - stub_charge_prebound(&small, "stub.provider");
    let import_growth = stub_charge_prebound(&large, "stub.consumer")
        - stub_charge_prebound(&small, "stub.consumer");
    assert!(definition_growth > 0);
    assert!(
        import_growth * 8 < definition_growth,
        "an imported body must not be charged as resolved structure: \
         definition grew by {definition_growth}, the importer by {import_growth}"
    );
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

fn identity_scale_workspace(
    module_pad: usize,
    functions: usize,
    body: usize,
) -> Vec<WorkspaceSource> {
    let module = format!("scale{}", "e".repeat(module_pad));
    let mut library = format!("\nmodule {module};\n");
    for index in 0..functions {
        library.push_str(&format!(
            "@id(\"{module}.step{index:03}\")\nfn step{index:03}(seed: i64) -> i64 {{\n"
        ));
        for statement in 0..body {
            library.push_str(&format!("let hold_{statement} = seed + {statement};\n"));
        }
        library.push_str(&format!("seed + {index}\n}}\n"));
    }
    let entry = format!(
        "\nmodule {module}.entry;\nuse function @id(\"{module}.step000\") from {module} as step000;\n@id(\"{module}.entry.main\")\nfn main() -> i64 {{ step000(0) }}\n"
    );
    vec![
        canonical_source("scale/library.spx", &library),
        canonical_source("scale/entry.spx", &entry),
    ]
}

/// Issue #83. The identity term charges every identity slot the longest
/// identity in scope, so the copy factor decides how far a declaration's
/// *name* can move the budget. At sixty-four copies a thirty-two byte module
/// segment more than doubled the minimum builder limit of this fixture, which
/// is why renaming `document` to `doc` bought `std.data.json.doc` about 960
/// bytes of admitted source. Measured retained identity bytes are 0.87 to
/// 1.11 per slot per identity byte, so naming must stay a minor term.
#[test]
fn identity_length_does_not_dominate_the_builder_pre_bound() {
    let short = minimum_successful_builder_limit(&identity_scale_workspace(0, 24, 0));
    let long = minimum_successful_builder_limit(&identity_scale_workspace(32, 24, 0));
    assert!(short < long, "a longer identity must still cost something");
    assert!(
        long * 4 < short * 5,
        "thirty-two identity bytes may not cost a quarter of the pre-bound \
         again: short {short}, long {long}"
    );
}

/// Issue #83. `std.data.json.doc` was admitted at 12,216 source bytes and
/// refused at 12,292, so every further standard-library slice was blocked.
/// Padding the six `std.data.json.*` packages until `SPX-G171` fires moved
/// their admitted totals from 12.3-14.2 KB to 19.7-22.1 KB. This fixture is
/// less identity-dense than a `std` package, so it is pinned at its own
/// ceiling: twenty-one kilobytes must fit, where the previous factor stopped
/// this shape at 19.4 KB.
#[test]
fn a_twenty_one_kilobyte_workspace_fits_the_production_builder_budget() {
    let sources = identity_scale_workspace(0, 310, 0);
    let bytes: usize = sources.iter().map(|source| source.source.len()).sum();
    assert!(
        bytes >= 21 * 1024,
        "fixture must exceed twenty-one kilobytes, got {bytes}"
    );
    assert!(build_owned_with_builder_limit(sources, MAX_BUILDER_BYTES).is_ok());
}

/// Issue #83. The pre-bound stays a refusal, not an advisory: a workspace the
/// resolver cannot fit is still reported as `SPX-G171` before it is linked,
/// and before any other workspace counter is exhausted.
#[test]
fn the_builder_pre_bound_still_refuses_an_oversized_workspace() {
    let sources = identity_scale_workspace(0, 64, 128);
    let error = match build_owned_with_builder_limit(sources, MAX_BUILDER_BYTES) {
        Ok(_) => panic!("an oversized workspace must be refused"),
        Err(error) => error,
    };
    assert_exact_builder_limit_error(&error, MAX_BUILDER_BYTES);
}
