//! Deterministic grammar-driven differential compiler tests with shrinking.
//!
//! A seed names one module in the commonly admitted scalar subset. The module
//! is run through every lane that can execute it — canonical parse-format-parse
//! stability, graph identity, verifier/HIR agreement, the reference
//! interpreter, native C11 at O0 and O2, and Core-Wasm on Node — and every
//! lane answers in one closed vocabulary. Any disagreement is classified,
//! minimized, and rendered as a report that reproduces without this machine.
//!
//! This module lives in the `scalar_status_backend_equivalence` harness rather
//! than in a binary of its own: that harness already owns scalar backend
//! equivalence, already links the whole compiler once, and is already named by
//! CI. `AGENTS.md` forbids adding a top-level test file per subject.
//!
//! Lane availability is never implicit. When `clang` or `node` is absent, or a
//! target refuses the profile, the lane reports `Unavailable` with its reason
//! and is excluded from the compared set; it is never counted as a parity pass.

// This file is itself loaded through `#[path]`, so its child modules resolve
// against the harness directory rather than a `differential/` subdirectory.
// Each one therefore names its file explicitly.
#[path = "differential/backends.rs"]
mod backends;
#[path = "differential/grammar.rs"]
mod grammar;
#[path = "differential/injection.rs"]
mod injection;
#[path = "differential/observe.rs"]
mod observe;
#[path = "differential/report.rs"]
mod report;
#[path = "differential/shrink.rs"]
mod shrink;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::ast::Program;
use semaprax::parse;

use grammar::{Generator, Module, Shape, Type};
use observe::{Case, Comparison, Finding, Lane, LaneReport, Observation, WorkCounters};
use report::{RunReport, Toolchains};

/// The fuel budget every reference-interpreter observation runs under. Every
/// generated loop is bounded by construction, so a seed that exhausts this
/// budget is a capacity outcome worth reporting rather than a hang.
const MAX_STEPS: usize = 200_000;

/// Seeds that run on every pull request. They are fixed, not sampled, so a
/// green run means the same programs were checked as last time. `campaign`
/// below takes an arbitrary count and stays `#[ignore]`d for the larger bounded
/// run outside PR CI.
const PR_SEEDS: [u64; 16] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_0002,
    0x0000_0000_0000_0003,
    0x0000_0000_0000_0005,
    0x0000_0000_0000_0008,
    0x0000_0000_0000_000d,
    0x0000_0000_0000_0015,
    0x0000_0000_0000_0022,
    0x0000_0000_dead_beef,
    0x0123_4567_89ab_cdef,
    0x5eed_5eed_5eed_5eed,
    0x8000_0000_0000_0000,
    0xa5a5_a5a5_a5a5_a5a5,
    0xdead_c0de_dead_c0de,
    0xffff_ffff_ffff_fffe,
    0xffff_ffff_ffff_ffff,
];

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

fn temporary_root(label: &str) -> PathBuf {
    let ordinal = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-differential-{label}-{}-{ordinal}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a temporary root is creatable");
    root
}

/// Which lanes a run should attempt. The frontend and interpreter lanes need no
/// provisioned tool and always run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lanes {
    FrontendAndInterpreter,
    Every,
}

struct Run {
    frontend: Vec<Finding>,
    reference: LaneReport,
    lanes: Vec<LaneReport>,
    comparison: Comparison,
    counters: WorkCounters,
}

impl Run {
    fn agrees(&self) -> bool {
        self.frontend.is_empty() && self.comparison.agrees()
    }

    /// The set of finding classes this run produced. Shrinking is only allowed
    /// to keep a candidate that still produces every class the original did, so
    /// a candidate that merely stopped verifying can never be mistaken for a
    /// smaller reproducer of a backend disagreement.
    fn signature(&self) -> BTreeSet<&'static str> {
        self.frontend
            .iter()
            .chain(self.comparison.findings.iter())
            .map(|finding| finding.class)
            .collect()
    }
}

fn case_set(module: &Module) -> Vec<Case> {
    module
        .cases
        .iter()
        .map(|case| (case.stable_id.clone(), case.result))
        .collect()
}

/// Run one module through every requested lane and classify the result.
fn run_module(module: &Module, root: &Path, lanes: Lanes) -> Run {
    let source = module.render();
    let path = root.join(format!("s{:016x}.spx", module.seed));
    std::fs::write(&path, &source).expect("the generated module is writable");
    let frontend = observe::observe_frontend(&source, &path);
    if !frontend.findings.is_empty() {
        return Run {
            frontend: frontend.findings,
            reference: LaneReport::unavailable(
                Lane::Interpreter,
                "the frontend rejected the module before any lane ran",
            ),
            lanes: Vec::new(),
            comparison: Comparison::default(),
            counters: WorkCounters::default(),
        };
    }
    let cases = case_set(module);
    let (reference, counters) = observe::observe_interpreter(&cases, &path, MAX_STEPS);
    let mut others = Vec::new();
    if lanes == Lanes::Every {
        match parse(&source, &path) {
            Ok(program) => {
                others.push(backends::observe_native(&cases, &program, root, "-O0"));
                others.push(backends::observe_native(&cases, &program, root, "-O2"));
                others.push(backends::observe_core_wasm(&cases, &program, root));
            }
            Err(error) => {
                let reason = format!("the module stopped parsing: {}", error.code);
                others.push(LaneReport::unavailable(Lane::NativeO0, reason.clone()));
                others.push(LaneReport::unavailable(Lane::NativeO2, reason.clone()));
                others.push(LaneReport::unavailable(Lane::CoreWasm, reason));
            }
        }
    }
    let comparison = observe::compare(&reference, &others);
    Run {
        frontend: frontend.findings,
        reference,
        lanes: others,
        comparison,
        counters,
    }
}

/// Minimize a module that disagrees, then render the full report.
fn describe(module: &Module, run: &Run, root: &Path, lanes: Lanes) -> String {
    let toolchains = Toolchains::resolve();
    let wanted = run.signature();
    let minimized = shrink::minimize(module, shrink::DEFAULT_BUDGET, |candidate| {
        let candidate_root = root.join(format!("shrink-{:016x}", candidate_digest(candidate)));
        if std::fs::create_dir_all(&candidate_root).is_err() {
            return false;
        }
        let outcome = run_module(candidate, &candidate_root, lanes);
        let _ = std::fs::remove_dir_all(&candidate_root);
        let observed = outcome.signature();
        wanted.iter().all(|class| observed.contains(class))
    });
    RunReport {
        seed: module.seed,
        module,
        minimized: Some(&minimized.module),
        minimization_steps: minimized.steps,
        toolchains: &toolchains,
        reference: &run.reference,
        lanes: &run.lanes,
        frontend: &run.frontend,
        comparison: &run.comparison,
    }
    .render()
}

/// A stable name for one candidate's scratch directory. Rendering is
/// deterministic, so this is a pure function of the candidate.
fn candidate_digest(module: &Module) -> u64 {
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for byte in module.render().bytes() {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
    }
    digest
}

fn generate(seed: u64) -> Module {
    Generator::new(seed, Shape::default()).module()
}

#[test]
fn fixed_seeds_agree_across_every_available_lane() {
    let root = temporary_root("fixed-seeds");
    let toolchains = Toolchains::resolve();
    println!("clang: {:?}", toolchains.clang);
    println!("node: {:?}", toolchains.node);
    let mut compared_any_backend = false;
    let mut total_steps = 0_u64;
    let mut unavailable = Vec::new();
    for seed in PR_SEEDS {
        let module = generate(seed);
        let seed_root = root.join(format!("seed-{seed:016x}"));
        std::fs::create_dir_all(&seed_root).expect("a seed root is creatable");
        let run = run_module(&module, &seed_root, Lanes::Every);
        for (lane, reason) in &run.comparison.unavailable {
            unavailable.push(format!("{}: {reason}", lane.label()));
        }
        compared_any_backend |= !run.comparison.compared.is_empty();
        total_steps += run.counters.total();
        assert!(
            run.agrees(),
            "{}",
            describe(&module, &run, &seed_root, Lanes::Every)
        );
        let _ = std::fs::remove_dir_all(&seed_root);
    }
    // An absent tool is an explicit outcome, printed rather than swallowed.
    unavailable.sort();
    unavailable.dedup();
    if unavailable.is_empty() {
        println!("every backend lane ran for every fixed seed");
    } else {
        println!("lanes unavailable (never counted as parity):");
        for entry in &unavailable {
            println!("  {entry}");
        }
    }
    println!("reference-lane interpreter steps across the fixed seeds: {total_steps}");
    assert!(
        compared_any_backend || !unavailable.is_empty(),
        "a run must either compare a backend lane or record why it could not"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A wider, cheap sweep through the two lanes that need no provisioned tool.
/// This keeps PR CI meaningful on a machine that has neither `clang` nor
/// `node`: parse-format-parse stability, graph identity, verifier/HIR
/// agreement, and the reference interpreter still run on every seed.
#[test]
fn the_frontend_and_reference_interpreter_agree_on_a_wider_seed_sweep() {
    let root = temporary_root("frontend-sweep");
    for ordinal in 0..64_u64 {
        let seed = 0xa076_1d64_78bd_642f_u64.wrapping_mul(ordinal.wrapping_add(1));
        let module = generate(seed);
        let seed_root = root.join(format!("seed-{seed:016x}"));
        std::fs::create_dir_all(&seed_root).expect("a seed root is creatable");
        let run = run_module(&module, &seed_root, Lanes::FrontendAndInterpreter);
        assert!(
            run.agrees(),
            "{}",
            describe(&module, &run, &seed_root, Lanes::FrontendAndInterpreter)
        );
        let _ = std::fs::remove_dir_all(&seed_root);
    }
    let _ = std::fs::remove_dir_all(root);
}

/// The larger bounded campaign. It is deliberately not part of PR CI; run it
/// with `--ignored` and, optionally, `SEMAPRAX_DIFFERENTIAL_SEEDS`.
#[test]
#[ignore = "bounded campaign; run separately from PR CI"]
fn bounded_campaign_agrees_across_every_available_lane() {
    let count = std::env::var("SEMAPRAX_DIFFERENTIAL_SEEDS")
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(256);
    let root = temporary_root("campaign");
    for ordinal in 0..count {
        let seed = 0x9e37_79b9_7f4a_7c15_u64.wrapping_mul(ordinal.wrapping_add(1));
        let module = generate(seed);
        let seed_root = root.join(format!("seed-{seed:016x}"));
        std::fs::create_dir_all(&seed_root).expect("a seed root is creatable");
        let run = run_module(&module, &seed_root, Lanes::Every);
        assert!(
            run.agrees(),
            "{}",
            describe(&module, &run, &seed_root, Lanes::Every)
        );
        let _ = std::fs::remove_dir_all(&seed_root);
    }
    println!("campaign agreed on {count} seeds");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn one_seed_always_renders_the_same_module() {
    for seed in PR_SEEDS {
        assert_eq!(
            generate(seed).render(),
            generate(seed).render(),
            "seed {seed:#018x} is not deterministic"
        );
    }
    let distinct = PR_SEEDS
        .iter()
        .map(|seed| generate(*seed).render())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        distinct.len(),
        PR_SEEDS.len(),
        "distinct seeds must produce distinct modules"
    );
}

#[test]
fn the_grammar_covers_every_construct_this_tranche_claims() {
    let mut total = grammar::Coverage::default();
    for seed in PR_SEEDS {
        let module = generate(seed);
        total.accumulate(&grammar::coverage(&module));
    }
    // Each of these is a construct the issue names for the first tranche. A
    // refactor that stops generating one has to fail here rather than silently
    // narrow what the campaign checks.
    assert!(total.nested_operands > 0, "nested operands: {total:?}");
    assert!(
        total.helper_calls > 0,
        "parameterized helper calls: {total:?}"
    );
    assert!(total.conditionals > 0, "branches: {total:?}");
    assert!(total.bounded_loops > 0, "bounded loops: {total:?}");
    assert!(total.mutations > 0, "mutation: {total:?}");
    // Shadowing is the one construct the issue names that SEMAPRAX does not
    // admit: rebinding a live local is `SPX-T209`. The generator therefore must
    // never emit it, and the hostile fixture below pins the diagnostic.
    assert_eq!(total.shadowed_bindings, 0, "shadowing: {total:?}");
    assert!(total.contracts > 0, "contracts: {total:?}");
    assert!(total.lazy_operators > 0, "lazy evaluation: {total:?}");
    assert!(total.zero_divisors > 0, "checked failure: {total:?}");
}

#[test]
fn shadowing_is_rejected_rather_than_generated() {
    // The invalid-input side of the grammar: a module the generator will never
    // produce, pinned to the exact diagnostic so that admitting shadowing later
    // is a deliberate change and not a silent widening of the campaign.
    let root = temporary_root("shadowing");
    let source = "module test.differential.shadowing;\n\
                  \n\
                  @id(\"shadow.main\")\n\
                  fn main() -> i64\n\
                  {\n\
                  \x20   let v0 = 1;\n\
                  \x20   let v0 = 2;\n\
                  \x20   v0\n\
                  }\n";
    let path = root.join("shadowing.spx");
    std::fs::write(&path, source).expect("the fixture is writable");
    let findings = observe::observe_frontend(source, &path);
    assert!(
        findings
            .findings
            .iter()
            .any(|finding| finding.observed.contains("SPX-T209")),
        "shadowing must be rejected as SPX-T209: {:?}",
        findings.findings
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn no_generated_module_can_reach_ambient_authority() {
    // The generator has no vocabulary for effects, unsafe blocks, foreign
    // declarations, or string literals, and this pins that: a fuzzer-generated
    // file must never execute with filesystem, network, process, or signing
    // authority. `permit`/`uses` are the only routes to a declared effect, and
    // `unsafe` the only route past the checked boundary.
    for seed in PR_SEEDS {
        let source = generate(seed).render();
        for forbidden in [
            "permit",
            "uses ",
            "unsafe",
            "extern",
            "import",
            "\"",
            "'",
            "@capability",
        ] {
            let occurrences = source.match_indices(forbidden).count();
            let allowed = usize::from(forbidden == "\"") * source.matches("@id(\"").count() * 2;
            assert_eq!(
                occurrences, allowed,
                "seed {seed:#018x} rendered `{forbidden}`:\n{source}"
            );
        }
    }
}

#[test]
fn shrinking_reduces_a_reproducer_while_the_predicate_still_holds() {
    // A predicate that stands in for a discrepancy: the module must still
    // verify, and `differential.case0` must still select the exact
    // divide-by-zero arithmetic status. Shrinking has to preserve enough
    // structure to keep reproducing that, which is the property the issue asks
    // for; using a real pipeline predicate rather than a syntactic one is what
    // makes the test meaningful.
    let root = temporary_root("shrink");
    let mut chosen = None;
    for seed in 1..400_u64 {
        let module = generate(seed);
        if divide_by_zero_case(&module, &root) {
            chosen = Some((seed, module));
            break;
        }
    }
    let (seed, module) = chosen.expect("some seed below 400 selects a divide-by-zero status");
    let before = module.render();
    let minimized = shrink::minimize(&module, shrink::DEFAULT_BUDGET, |candidate| {
        divide_by_zero_case(candidate, &root)
    });
    let after = minimized.module.render();
    println!(
        "seed {seed:#018x}: {} bytes -> {} bytes in {} steps ({} predicate calls, budget exhausted: {})",
        before.len(),
        after.len(),
        minimized.steps,
        minimized.predicate_calls,
        minimized.budget_exhausted
    );
    assert!(
        minimized.steps > 0,
        "the shrinker made no progress on seed {seed:#018x}"
    );
    assert!(
        after.len() < before.len(),
        "the minimized module is not smaller:\n{after}"
    );
    assert!(
        divide_by_zero_case(&minimized.module, &root),
        "the minimized module stopped reproducing:\n{after}"
    );
    assert!(
        minimized.module.cases.len() == 1,
        "a single-case reproducer was expected, found {}:\n{after}",
        minimized.module.cases.len()
    );
    println!("minimized reproducer:\n{after}");
    let _ = std::fs::remove_dir_all(root);
}

/// The shrink predicate: does `differential.case0` still fail with the exact
/// divide-by-zero arithmetic status, on a module that still verifies?
fn divide_by_zero_case(module: &Module, root: &Path) -> bool {
    let Some(case) = module
        .cases
        .iter()
        .find(|case| case.stable_id == "differential.case0")
    else {
        return false;
    };
    let cases = vec![(case.stable_id.clone(), case.result)];
    let source = module.render();
    let path = root.join("candidate.spx");
    if std::fs::write(&path, &source).is_err() {
        return false;
    }
    if !observe::observe_frontend(&source, &path)
        .findings
        .is_empty()
    {
        return false;
    }
    let (report, _) = observe::observe_interpreter(&cases, &path, MAX_STEPS);
    let Some(observations) = report.observations() else {
        return false;
    };
    matches!(
        observations.get("differential.case0"),
        Some(Observation::Failed { domain, code })
            if domain == "semaprax.arithmetic.v1" && *code == 4
    )
}

#[test]
fn interpreter_work_counters_scale_with_the_loop_bound() {
    // A scaling fixture instead of a global time threshold: the same module at
    // three loop bounds, compared through the interpreter's own deterministic
    // step counter. Each iteration costs a constant number of steps, so the
    // first differences must be exactly proportional to the bound differences.
    let root = temporary_root("scaling");
    let measure = |bound: i64| -> u64 {
        let source = counted_loop_module(bound);
        let path = root.join(format!("scaling-{bound}.spx"));
        std::fs::write(&path, &source).expect("the fixture is writable");
        let findings = observe::observe_frontend(&source, &path);
        assert!(
            findings.findings.is_empty(),
            "the scaling fixture must verify: {:?}",
            findings.findings
        );
        assert!(findings.resolved_functions >= 2);
        let cases = vec![("scaling.total".to_owned(), Type::I64)];
        let (report, counters) = observe::observe_interpreter(&cases, &path, MAX_STEPS);
        assert!(
            matches!(
                report
                    .observations()
                    .and_then(|map| map.get("scaling.total")),
                Some(Observation::Returned { .. })
            ),
            "the scaling fixture must return a value, observed {:?}",
            report.status
        );
        counters.total()
    };
    let (small, medium, large) = (measure(2), measure(4), measure(8));
    let first = medium - small;
    let second = large - medium;
    println!("steps: bound 2 = {small}, bound 4 = {medium}, bound 8 = {large}");
    assert!(
        small < medium && medium < large,
        "steps must grow with the bound"
    );
    assert_eq!(
        second,
        first * 2,
        "the interpreter's step cost per iteration is not constant: \
         {small}, {medium}, {large}"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn counted_loop_module(bound: i64) -> String {
    format!(
        "module test.differential.scaling;\n\
         \n\
         @id(\"scaling.step\")\n\
         fn step(value: i64) -> i64\n\
         {{\n\
         \x20   value + 1\n\
         }}\n\
         \n\
         @id(\"scaling.total\")\n\
         fn total() -> i64\n\
         {{\n\
         \x20   let mut acc = 0;\n\
         \x20   let mut n = {bound};\n\
         \x20   while n > 0 {{\n\
         \x20       acc = step(acc);\n\
         \x20       n = n - 1;\n\
         \x20       n > 0\n\
         \x20   }}\n\
         \x20   acc\n\
         }}\n\
         \n\
         @id(\"app.main\")\n\
         fn main() -> i64\n\
         {{\n\
         \x20   total()\n\
         }}\n"
    )
}

/// Shared by the injection module: build a program from source or fail loudly.
fn program_of(source: &str, path: &Path) -> Program {
    parse(source, path).expect("a hand-written fixture parses")
}
