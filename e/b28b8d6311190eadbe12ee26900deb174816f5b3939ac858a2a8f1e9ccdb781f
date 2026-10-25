use super::{
    checked_retention_prebound, dependency_identity_max, next_retention_prebound,
    next_retention_prebound_with_uncached_peak, retention_prebound, retention_prebound_mode,
    synthetic_builder_bytes_scoped, uncached_peak_prebound,
};
use crate::ast::Program;
fn fixture(count: usize, padding: usize, reverse: bool) -> Vec<Program> {
    let long = format!("consumer.{}", "x".repeat(padding));
    let mut provider = String::from("module provider;\n");
    for index in 0..count {
        provider.push_str(&format!(
            "@id(\"provider.f{index}\") fn f{index}(seed:i64)->i64 {{ seed+{index} }}\n"
        ));
    }
    let mut consumer = format!("module consumer;\n@id(\"{long}\") fn target()->i64{{0}}\n");
    if reverse {
        consumer = consumer.replacen(
            "module consumer;\n",
            "module consumer;\nuse function @id(\"provider.f0\") from provider as f0;\n",
            1,
        );
    } else {
        provider = provider.replacen(
            "module provider;\n",
            &format!("module provider;\nuse function @id(\"{long}\") from consumer as target;\n"),
            1,
        );
    }
    [provider, consumer]
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            crate::parse(
                &text,
                std::path::Path::new(if index == 0 {
                    "provider.spx"
                } else {
                    "consumer.spx"
                }),
            )
            .unwrap()
        })
        .collect()
}
#[test]
fn identity_prebound_preserves_every_legacy_accepted_receipt() {
    let programs = fixture(3, 40, true);
    let authored = super::super::index_authored(&programs).unwrap();
    let old = retention_prebound(&programs, &authored, false).unwrap();
    assert_eq!(
        checked_retention_prebound(&programs, &authored).unwrap(),
        old
    );
}
#[test]
fn identity_prebound_excludes_reverse_dependent_names_only_after_refusal() {
    let programs = fixture(310, 220, true);
    let authored = super::super::index_authored(&programs).unwrap();
    assert!(dependency_identity_max(&programs[0], &authored, &programs).unwrap() < 100);
    let unscoped = retention_prebound(&programs, &authored, false).unwrap().1;
    let scoped = retention_prebound(&programs, &authored, true).unwrap().1;
    assert!(
        scoped < unscoped,
        "excluding reverse dependents must save bytes"
    );
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
        }
    }
    let previous = super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(scoped));
    let _restore = Restore(previous);
    assert!(retention_prebound(&programs, &authored, false).is_err());
    assert_eq!(
        checked_retention_prebound(&programs, &authored).unwrap(),
        retention_prebound(&programs, &authored, true).unwrap()
    );
}
#[test]
fn identity_prebound_still_charges_reachable_long_identities() {
    let programs = fixture(800, 220, false);
    let authored = super::super::index_authored(&programs).unwrap();
    assert!(dependency_identity_max(&programs[0], &authored, &programs).unwrap() >= 220);
    let shorter = fixture(800, 16, false);
    let shorter_authored = super::super::index_authored(&shorter).unwrap();
    let modes = [(false, 0), (true, 0), (true, 2), (true, 3), (true, 4)];
    let short_min = modes
        .iter()
        .filter_map(|&(scoped, mode)| {
            retention_prebound_mode(&shorter, &shorter_authored, scoped, mode)
                .ok()
                .map(|cost| cost.1)
        })
        .min()
        .unwrap();
    let long_min = modes
        .iter()
        .filter_map(|&(scoped, mode)| {
            retention_prebound_mode(&programs, &authored, scoped, mode)
                .ok()
                .map(|cost| cost.1)
        })
        .min()
        .unwrap();
    assert!(
        long_min > short_min,
        "reachable long IDs must raise every receipt"
    );
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
        }
    }
    let previous = super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(short_min));
    let _restore = Restore(previous);
    assert!(checked_retention_prebound(&shorter, &shorter_authored).is_ok());
    assert!(checked_retention_prebound(&programs, &authored).is_err());
}

/// Issue #83. Pin the projection term itself instead of comparing complete
/// workspace limits, whose unrelated structural costs legitimately move as
/// the AST grows. Keep `16` literal here so increasing the production copy
/// factor reopens this regression rather than updating the test in lockstep.
#[test]
fn identity_length_uses_sixteen_bytes_per_slot_and_identity_byte() {
    let programs = fixture(24, 32, false);
    let authored = super::super::index_authored(&programs).unwrap();
    let program = &programs[0];
    let identity_slots = super::identity_slots::ast_program_identity_slots(program).unwrap();
    let short = synthetic_builder_bytes_scoped(program, &authored, &programs, Some(16), 0, true)
        .unwrap()
        .retained_hir;
    let long = synthetic_builder_bytes_scoped(program, &authored, &programs, Some(48), 0, true)
        .unwrap()
        .retained_hir;
    assert_eq!(long - short, identity_slots * (48 - 16) * 16);
}
#[test]
fn identity_prebound_json_cursor_package_is_bounded() {
    let programs = [
        (
            "dec.spx",
            include_str!("../../../std/data-json-dec/src/dec.spx"),
        ),
        (
            "examples.spx",
            include_str!("../../../std/data-json-dec/src/examples.spx"),
        ),
        (
            "tests.spx",
            include_str!("../../../std/data-json-dec/src/tests.spx"),
        ),
        ("io.spx", include_str!("../../../std/io/src/io.spx")),
    ]
    .into_iter()
    .map(|(path, text)| crate::parse(text, std::path::Path::new(path)).unwrap())
    .collect::<Vec<_>>();
    let authored = super::super::index_authored(&programs).unwrap();
    checked_retention_prebound(&programs, &authored)
        .expect("complete JSON cursor dependency pre-bound");
}

fn transient_fixture() -> Vec<Program> {
    let sources = [
            "module provider; @id(\"provider.compute\") fn compute(value: i64) -> i64 requires value >= 0 ensures result >= 0 { let first = value + 1; let second = first * 2; second + 3 }",
            "module first; use function @id(\"provider.compute\") from provider as compute; @id(\"first.run\") fn run() -> i64 { compute(1) }",
            "module second; use function @id(\"provider.compute\") from provider as compute; @id(\"second.run\") fn run() -> i64 { compute(2) }",
        ];
    sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            crate::parse(
                source,
                std::path::Path::new(if index == 0 {
                    "provider.spx"
                } else if index == 1 {
                    "first.spx"
                } else {
                    "second.spx"
                }),
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
}

#[test]
fn transient_import_clones_have_one_sequential_peak_only_in_final_fallback() {
    let programs = transient_fixture();
    let authored = super::super::index_authored(&programs).unwrap();
    assert!(!programs[0].functions[0].requires.is_empty());
    assert!(!programs[0].functions[0].ensures.is_empty());
    for consumer in &programs[1..] {
        let synthetic = super::synthetic_program(consumer, &authored, &programs).unwrap();
        let stub = synthetic
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(stub.body.kind, crate::ast::ExprKind::Int(0)));
        let mut expected = programs[0].functions[0].clone();
        expected.name = stub.name.clone();
        expected.requires = Vec::new();
        expected.ensures = Vec::new();
        expected.body = stub.body.clone();
        assert_eq!(*stub, expected, "ordinary import changed a signature field");
        assert!(stub.requires.is_empty() && stub.ensures.is_empty());
        assert_eq!(stub.requires.capacity(), 0);
        assert_eq!(stub.ensures.capacity(), 0);
    }
    let old = retention_prebound_mode(&programs, &authored, true, 2).unwrap();
    let peak = retention_prebound_mode(&programs, &authored, true, 3).unwrap();
    assert!(
        peak.0 < old.0,
        "two transient full-function clones were summed"
    );
    assert!(peak.1 < old.1);
    assert_eq!(
        retention_prebound_mode(&programs, &authored, true, 3).unwrap(),
        peak,
        "the final fallback receipt must be deterministic"
    );

    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
        }
    }
    let previous = super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(peak.1));
    let _restore = Restore(previous);
    assert!(retention_prebound_mode(&programs, &authored, true, 2).is_err());
    assert_eq!(
        retention_prebound_mode(&programs, &authored, true, 3).unwrap(),
        peak,
        "the new peak receipt admits the same graph at its exact bound"
    );
    super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(peak.1 - 1));
    let refusal = retention_prebound_mode(&programs, &authored, true, 3).unwrap_err();
    assert!(refusal.iter().all(|error| error.code == "SPX-G171"));
    let signature_only = retention_prebound_mode(&programs, &authored, true, 4).unwrap();
    assert!(signature_only.1 < peak.1);
    super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(signature_only.1));
    assert_eq!(
        retention_prebound_mode(&programs, &authored, true, 4).unwrap(),
        signature_only
    );
    super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(signature_only.1 - 1));
    assert!(retention_prebound_mode(&programs, &authored, true, 4)
        .unwrap_err()
        .iter()
        .all(|error| error.code == "SPX-G171"));
}

#[test]
fn next_prebound_skips_non_tighter_receipts_and_exhausts_at_mode_four() {
    let programs = transient_fixture();
    let authored = super::super::index_authored(&programs).unwrap();
    let mode_two = retention_prebound_mode(&programs, &authored, true, 2).unwrap();
    let mode_three = retention_prebound_mode(&programs, &authored, true, 3).unwrap();
    let mut mode = 1;
    assert!(mode_three.0 < mode_two.0);
    assert_eq!(
        next_retention_prebound(&programs, &authored, mode_two.0, &mut mode).unwrap(),
        mode_three
    );
    assert_eq!(mode, 3);
    let mode_four = retention_prebound_mode(&programs, &authored, true, 4).unwrap();
    assert!(mode_four.0 < mode_three.0);
    assert_eq!(
        next_retention_prebound(&programs, &authored, mode_three.0, &mut mode).unwrap(),
        mode_four
    );
    let exhausted =
        next_retention_prebound(&programs, &authored, mode_four.0, &mut mode).unwrap_err();
    assert_eq!(mode, 4);
    assert!(exhausted.iter().all(|error| error.code == "SPX-G171"));
}

#[test]
fn uncached_ast_peak_is_final_only_and_has_an_exact_bound() {
    let programs = transient_fixture();
    let authored = super::super::index_authored(&programs).unwrap();
    let mode_four = retention_prebound_mode(&programs, &authored, true, 4).unwrap();
    let peak = uncached_peak_prebound(&programs, &authored).unwrap();
    assert!(peak.0 < mode_four.0);

    let mut semantic_mode = 4;
    assert!(
        next_retention_prebound(&programs, &authored, mode_four.0, &mut semantic_mode).is_err()
    );
    assert_eq!(semantic_mode, 4);

    let mut uncached_mode = 4;
    assert_eq!(
        next_retention_prebound_with_uncached_peak(
            &programs,
            &authored,
            mode_four.0,
            &mut uncached_mode,
            true,
        )
        .unwrap(),
        peak,
    );
    assert_eq!(uncached_mode, 5);

    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
        }
    }
    let previous = super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(peak.0));
    let _restore = Restore(previous);
    assert_eq!(uncached_peak_prebound(&programs, &authored).unwrap(), peak);
    assert_eq!(
        super::checked_retention_prebound_with_uncached_peak(&programs, &authored, true).unwrap(),
        peak,
    );
    let ordinary_refusal = super::checked_retention_prebound(&programs, &authored).unwrap_err();
    let cached_refusal =
        super::checked_retention_prebound_with_uncached_peak(&programs, &authored, false)
            .unwrap_err();
    assert_eq!(
        ordinary_refusal
            .iter()
            .map(|error| error.code)
            .collect::<Vec<_>>(),
        cached_refusal
            .iter()
            .map(|error| error.code)
            .collect::<Vec<_>>(),
    );
    assert!(cached_refusal.iter().all(|error| error.code == "SPX-G171"));
    super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(peak.0 - 1));
    let refusal = uncached_peak_prebound(&programs, &authored).unwrap_err();
    assert!(refusal.iter().all(|error| error.code == "SPX-G171"));
}

struct LimitRestore(usize);
impl Drop for LimitRestore {
    fn drop(&mut self) {
        super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
    }
}

fn uniform_sequential_fixture() -> Vec<Program> {
    ["m0", "m1", "m2"]
        .into_iter()
        .map(|module| {
            let source = format!(
                "module {module}; @id(\"{module}.run\") fn run(seed: i64) -> i64 {{ let first = seed + 1; let second = first * 2; second - 3 }}"
            );
            crate::parse(&source, std::path::Path::new(&format!("{module}.spx"))).unwrap()
        })
        .collect()
}

/// Independent sequencing oracle over the unchanged per-module cost function:
/// it restates the documented overhead order and running retained sum, not
/// the production loop.
struct SequentialOracle {
    full: Vec<usize>,
    retained: Vec<usize>,
    order: Vec<usize>,
    forecast: usize,
}

fn sequential_oracle(
    programs: &[Program],
    authored: &std::collections::BTreeMap<&str, super::AuthoredDeclaration<'_>>,
) -> SequentialOracle {
    let mut full = Vec::new();
    let mut retained = Vec::new();
    for program in programs {
        let maximum = Some(dependency_identity_max(program, authored, programs).unwrap());
        for (include_imports, costs) in [(true, &mut full), (false, &mut retained)] {
            costs.push(
                synthetic_builder_bytes_scoped(
                    program,
                    authored,
                    programs,
                    maximum,
                    5,
                    include_imports,
                )
                .unwrap()
                .retained_hir,
            );
        }
    }
    let mut order = (0..programs.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        (
            std::cmp::Reverse(full[*index].saturating_sub(retained[*index])),
            programs[*index].path.clone(),
        )
    });
    let (mut prefix, mut forecast) = (0usize, 0usize);
    for index in &order {
        forecast = forecast.max(prefix + full[*index]);
        prefix += retained[*index];
        forecast = forecast.max(prefix);
    }
    SequentialOracle {
        full,
        retained,
        order,
        forecast,
    }
}

/// The help the first refused sequential step must carry at `limit`, or the
/// help of the first module whose own costing already refuses there.
fn expected_one_short_help(
    oracle: &SequentialOracle,
    programs: &[Program],
    authored: &std::collections::BTreeMap<&str, super::AuthoredDeclaration<'_>>,
    limit: usize,
) -> String {
    let mut earlier_full = 0usize;
    for (position, program) in programs.iter().enumerate() {
        let maximum = Some(dependency_identity_max(program, authored, programs).unwrap());
        let fits = [true, false].into_iter().all(|imports| {
            synthetic_builder_bytes_scoped(program, authored, programs, maximum, 5, imports).is_ok()
        });
        if !fits {
            let path = &program.path;
            return format!(
                "the static admission pre-charge refused before the graph builder ran, while forecasting module `{path}` on its own: that one module does not fit the {limit}-byte budget, which already carries {earlier_full} bytes for {position} earlier module(s). Splitting `{path}` into several modules cannot reduce the forecast, because the pre-charge sums every reachable module; reducing its admitted source can."
            );
        }
        earlier_full += oracle.full[position];
    }
    let mut retained = 0usize;
    for (earlier, index) in oracle.order.iter().enumerate() {
        let path = &programs[*index].path;
        let full = oracle.full[*index];
        if retained + full > limit {
            return format!(
                "the static admission pre-charge refused before the graph builder ran, in its final sequential uncached forecast: {earlier} earlier module(s) retain {retained} bytes of filtered output, and resolving `{path}` adds its complete {full}-byte forecast, which exceeds the {limit}-byte budget. Each complete module forecast is charged only while that module resolves; reducing the admitted source of `{path}` or the output retained by earlier modules can reduce this peak."
            );
        }
        let own = oracle.retained[*index];
        if retained + own > limit {
            return format!(
                "the static admission pre-charge refused before the graph builder ran, in its final sequential uncached forecast: {earlier} earlier module(s) retain {retained} bytes of filtered output, and `{path}` retains {own} more bytes, which exceeds the {limit}-byte budget. Retained output sums every reachable module, so splitting one module into several cannot reduce it; removing a dependency or reducing total reachable source can."
            );
        }
        retained += own;
    }
    panic!("no sequential step refuses at {limit}");
}

fn single_help(errors: &[crate::diagnostic::Diagnostic], limit: usize) -> String {
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "SPX-G171");
    assert_eq!(
        errors[0].message,
        format!("Workspace Semantic Graph `builder_bytes` exceeds {limit}")
    );
    errors[0]
        .help
        .clone()
        .expect("builder refusal names its bound")
}

/// The final uncached forecast must describe its own refused step. Before
/// this, it reused the partial module sum of the refused mode-5 pass that ran
/// first, which named neither its peak nor its retained output.
#[test]
fn uncached_sequential_forecast_attributes_its_own_cumulative_one_short_step() {
    let programs = uniform_sequential_fixture();
    let authored = super::super::index_authored(&programs).unwrap();
    let oracle = sequential_oracle(&programs, &authored);
    // Import-free modules retain their complete own output, so the peak is
    // the cumulative retained sum reached while resolving the last module.
    assert_eq!(oracle.full, oracle.retained);
    assert_eq!(oracle.forecast, oracle.retained.iter().sum::<usize>());
    assert_eq!(oracle.order, [0, 1, 2]);
    let layout = super::uncached_peak::uncached_output_layout(&programs, &authored).unwrap();
    assert_eq!(layout.forecast, oracle.forecast);
    assert_eq!(&layout.order[..programs.len()], oracle.order.as_slice());

    let forecast = oracle.forecast;
    let previous = super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(forecast));
    let _restore = LimitRestore(previous);
    // Exact bound: the production entry exhausts modes 0-5 and selects six.
    assert_eq!(
        super::initial_core_prebound(&programs, &authored, true).unwrap(),
        (forecast, forecast, true, 6)
    );
    assert_eq!(
        super::uncached_output_peak_prebound(&programs, &authored).unwrap(),
        (forecast, forecast)
    );
    // Later live attribution reports the charged peak, not the module sum.
    let dominant = (0..programs.len()).fold(0, |best, index| {
        if oracle.full[index] > oracle.full[best] {
            index
        } else {
            best
        }
    });
    let live = single_help(
        &[super::super::limit_error("builder_bytes", forecast)],
        forecast,
    );
    assert!(
        live.starts_with(&format!(
            "live graph construction, not the static admission pre-charge, exceeded the budget: the pre-charge forecast {forecast} bytes for 3 module(s) and fit within {forecast}, with `{}` the largest at {} bytes.",
            programs[dominant].path, oracle.full[dominant]
        )),
        "{live}"
    );

    let short = forecast - 1;
    super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(short));
    for program in &programs {
        let maximum = Some(dependency_identity_max(program, &authored, &programs).unwrap());
        assert!(
            synthetic_builder_bytes_scoped(program, &authored, &programs, maximum, 5, true).is_ok(),
            "one module alone must fit, so only the cumulative step can refuse"
        );
    }
    let expected = expected_one_short_help(&oracle, &programs, &authored, short);
    assert!(expected.contains(&format!(
        "2 earlier module(s) retain {} bytes",
        oracle.retained[0] + oracle.retained[1]
    )));
    assert!(expected.contains(&format!("resolving `{}`", programs[2].path)));
    // Hostile stale attribution: a refused summed pass runs first.
    let summed = single_help(
        &uncached_peak_prebound(&programs, &authored).unwrap_err(),
        short,
    );
    assert!(summed.contains("alone forecasts"), "{summed}");
    let refusal = super::uncached_peak::uncached_output_layout(&programs, &authored)
        .err()
        .unwrap();
    assert_eq!(single_help(&refusal, short), expected);
    // The production entry runs that same summed pass before mode six.
    let entry = super::initial_core_prebound(&programs, &authored, true).unwrap_err();
    let entry = single_help(&entry, short);
    assert_eq!(entry, expected);
    assert!(!entry.contains("alone forecasts"));
    // The same refusal is deterministic and leaves the exact bound admitted.
    assert_eq!(
        single_help(
            &super::uncached_output_peak_prebound(&programs, &authored).unwrap_err(),
            short
        ),
        expected
    );
    super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(forecast));
    assert_eq!(
        super::uncached_output_peak_prebound(&programs, &authored).unwrap(),
        (forecast, forecast)
    );
}

/// Imports give consumers a positive transient overhead, so they resolve
/// before the provider. One byte short, the help names exactly the step the
/// independent order reaches first.
#[test]
fn uncached_sequential_refusal_follows_the_independent_overhead_order() {
    let programs = transient_fixture();
    let authored = super::super::index_authored(&programs).unwrap();
    let oracle = sequential_oracle(&programs, &authored);
    for consumer in 1..programs.len() {
        assert!(oracle.full[consumer] > oracle.retained[consumer]);
    }
    assert_eq!(oracle.full[0], oracle.retained[0]);
    assert_eq!(
        oracle.order.last(),
        Some(&0),
        "the provider has no import overhead"
    );
    let layout = super::uncached_peak::uncached_output_layout(&programs, &authored).unwrap();
    assert_eq!(layout.forecast, oracle.forecast);
    assert_eq!(&layout.order[..programs.len()], oracle.order.as_slice());

    let previous = super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(oracle.forecast));
    let _restore = LimitRestore(previous);
    assert_eq!(
        super::uncached_peak::uncached_output_layout(&programs, &authored)
            .unwrap()
            .forecast,
        oracle.forecast
    );
    let short = oracle.forecast - 1;
    super::super::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(short));
    let expected = expected_one_short_help(&oracle, &programs, &authored, short);
    let refusal = super::uncached_peak::uncached_output_layout(&programs, &authored)
        .err()
        .unwrap();
    assert_eq!(single_help(&refusal, short), expected);
}
