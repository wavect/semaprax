//! Byte-data capacity inputs derived from resolved HIR.
//!
//! These pin the storage inventory the capacity authority consumes: how many
//! bytes an aggregate slot charges, which expressions earn a slot at all, the
//! order slots are reported in, and the exact frame boundary between an
//! admitted program and `SPX-T261`.

use std::path::Path;

use crate::byte_data_capacity::{ArrayStorageKind, ArrayStorageSlot, TranscriptSource};

use super::value_index_counters::{BASELINE_SEARCHES, BUILDS, USE_BASELINE, VISITS};
use super::*;

/// Nested aggregate whose inline byte payload (3 + 4) is not visible from any
/// single field, so a walk that stops at the first level under-charges it.
const NESTED: &str = r#"
module test.hir_byte_capacity_payload;

@id("data.inner")
record Inner {
    @id("data.inner.tail")
    tail: [u8; 4],
}

@id("data.outer")
record Outer {
    @id("data.outer.head")
    head: [u8; 3],
    @id("data.outer.inner")
    inner: Inner,
    @id("data.outer.count")
    count: i64,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn resolved(source: &str, path: &str) -> ResolvedProgram {
    let ast = crate::parse(source, Path::new(path)).expect("fixture parses");
    crate::hir::resolve(&ast).expect("fixture resolves")
}

fn nominal(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    }
}

fn shape(slots: &[ArrayStorageSlot]) -> Vec<(ArrayStorageKind, u32)> {
    slots.iter().map(|slot| (slot.kind, slot.length)).collect()
}

#[test]
fn inline_array_payload_sums_every_nested_fixed_array_field() {
    let program = resolved(NESTED, "hir-byte-capacity-payload.spx");
    assert_eq!(
        inline_array_payload_bytes(&program, &nominal("data.outer")).unwrap(),
        7,
        "a nested record charges its own arrays plus its children's"
    );
    assert_eq!(
        inline_array_payload_bytes(&program, &nominal("data.inner")).unwrap(),
        4
    );
    assert_eq!(
        inline_array_payload_bytes(&program, &ResolvedType::ArrayU8(9)).unwrap(),
        9
    );
    // Scalars and borrowed views hold no inline bytes.
    assert_eq!(
        inline_array_payload_bytes(&program, &ResolvedType::I64).unwrap(),
        0
    );
    assert_eq!(
        inline_array_payload_bytes(&program, &ResolvedType::SliceU8).unwrap(),
        0
    );
}

#[test]
fn inline_array_payload_fails_closed_on_unknown_and_unsubstituted_types() {
    let program = resolved(NESTED, "hir-byte-capacity-payload.spx");
    // Reporting 0 bytes for a slot the compiler cannot size would silently
    // under-allocate a frame, so both cases must be diagnostics.
    assert_eq!(
        inline_array_payload_bytes(&program, &nominal("data.absent"))
            .expect_err("unknown nominal type fails closed")
            .code,
        "SPX-H006"
    );
    let unsubstituted = ResolvedType::TypeParameter {
        owner: DeclarationId::new("data.outer"),
        index: 0,
    };
    assert_eq!(
        inline_array_payload_bytes(&program, &unsubstituted)
            .expect_err("unresolved type parameter fails closed")
            .code,
        "SPX-H006"
    );
}

#[test]
fn only_byte_bearing_slots_are_recorded_and_the_empty_array_still_is() {
    let program = resolved(NESTED, "hir-byte-capacity-payload.spx");
    let mut slots = Vec::new();
    push_array_slot(
        &program,
        &mut slots,
        "empty".to_owned(),
        ArrayStorageKind::Binding,
        &ResolvedType::ArrayU8(0),
    )
    .unwrap();
    // `[u8; 0]` is a real byte slot with no bytes; it must stay in the
    // inventory so identity accounting keeps seeing it.
    assert_eq!(shape(&slots), vec![(ArrayStorageKind::Binding, 0)]);

    push_array_slot(
        &program,
        &mut slots,
        "scalar".to_owned(),
        ArrayStorageKind::Binding,
        &ResolvedType::I64,
    )
    .unwrap();
    assert_eq!(slots.len(), 1, "a scalar slot is not byte storage");

    push_array_slot(
        &program,
        &mut slots,
        "outer".to_owned(),
        ArrayStorageKind::Parameter,
        &nominal("data.outer"),
    )
    .unwrap();
    assert_eq!(
        shape(&slots),
        vec![
            (ArrayStorageKind::Binding, 0),
            (ArrayStorageKind::Parameter, 7),
        ]
    );
}

#[test]
fn capacity_inputs_report_parameters_then_result_then_body_slots() {
    let source = r#"
module test.hir_byte_capacity_inventory;

@id("bytes.count")
fn count(input: [u8; 2]) -> usize
{
    let local = [1u8, 2u8, 3u8];
    byte_len(array_as_slice(local)) + byte_len(array_as_slice(input))
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = resolved(source, "hir-byte-capacity-inventory.spx");
    let inputs = byte_data_capacity_inputs(&program).expect("capacity inputs derive");
    let count = inputs
        .iter()
        .find(|input| input.function == "bytes.count")
        .expect("function reported");
    // Parameter first, then the (zero-byte, therefore absent) provisional
    // result, then the body in authored order. A `let` bound directly to an
    // array literal is the literal's destination, so it charges one binding
    // slot rather than a binding plus a temporary.
    assert_eq!(
        shape(&count.array_slots),
        vec![
            (ArrayStorageKind::Parameter, 2),
            (ArrayStorageKind::Binding, 3),
        ]
    );
    assert!(
        count.array_slots[0].identity != count.array_slots[1].identity,
        "each slot is separately addressable"
    );
    // Inputs are keyed by stable identity, not by name, and follow resolution
    // order so the summaries line up with the functions they describe.
    assert_eq!(
        inputs
            .iter()
            .map(|input| input.function.clone())
            .collect::<Vec<_>>(),
        vec!["bytes.count".to_owned(), "app.main".to_owned()]
    );
}

#[test]
fn an_array_literal_argument_charges_both_staging_and_a_temporary() {
    let source = r#"
module test.hir_byte_capacity_staging;

@id("bytes.take")
fn take(value: [u8; 2]) -> i64
{
    0
}

@id("bytes.stage")
fn stage() -> i64
{
    take([1u8, 2u8])
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = resolved(source, "hir-byte-capacity-staging.spx");
    let inputs = byte_data_capacity_inputs(&program).expect("capacity inputs derive");
    let stage = inputs
        .iter()
        .find(|input| input.function == "bytes.stage")
        .expect("function reported");
    // The argument is staged for the callee and also materialized as a
    // temporary, because a literal is not written into the staging slot in
    // place. Collapsing these two would under-report the caller's frame.
    assert_eq!(
        shape(&stage.array_slots),
        vec![
            (ArrayStorageKind::CallStaging, 2),
            (ArrayStorageKind::Temporary, 2),
        ]
    );
    assert!(stage.array_slots[0].identity.ends_with(".arg.0"));
}

fn frame_source(extra: &str) -> String {
    let mut source = String::new();
    source.push_str("module test.hir_byte_capacity_frame;\n\n@id(\"bytes.hold\")\n");
    source.push_str("fn hold(buffer: [u8; 65536]) -> i64\n{\n");
    source.push_str(extra);
    source.push_str("    0\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    source
}

#[test]
fn a_frame_of_exactly_the_inline_array_limit_is_admitted() {
    let source = frame_source("");
    let ast = crate::parse(&source, Path::new("hir-byte-capacity-frame.spx")).expect("parses");
    let program = crate::hir::resolve(&ast).expect("a frame at the limit resolves");
    let inputs = byte_data_capacity_inputs(&program).expect("capacity inputs derive");
    let hold = inputs
        .iter()
        .find(|input| input.function == "bytes.hold")
        .expect("function reported");
    assert_eq!(
        u64::from(hold.array_slots.iter().map(|slot| slot.length).sum::<u32>()),
        crate::byte_data_capacity::MAX_INLINE_ARRAY_FRAME_BYTES,
        "the fixture sits exactly on the boundary it is testing"
    );
    assert!(analyze_byte_data_capacity(&program).is_ok());
}

#[test]
fn one_byte_past_the_inline_array_limit_is_rejected() {
    let source = frame_source("    let extra = [0u8; 1];\n");
    let ast = crate::parse(&source, Path::new("hir-byte-capacity-frame.spx")).expect("parses");
    let diagnostics = crate::hir::resolve(&ast).expect_err("one byte over is rejected");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T261"),
        "{diagnostics:#?}"
    );
}

fn stdout_argument(function: &ResolvedFunction) -> &ResolvedExpr {
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, args, .. } = &expression.kind {
            if callee.as_str() == crate::host_io_ops::STDOUT_WRITE_ID {
                return &args[0];
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    panic!("fixture writes to stdout")
}

#[test]
fn transcript_source_is_fixed_for_a_known_array_root_and_unknown_otherwise() {
    let source = r#"
module test.hir_byte_capacity_transcript;

permit { process.stdout.write }

@id("bytes.fixed")
fn fixed() -> usize
    uses { process.stdout.write }
{
    let sample = [97u8, 98u8];
    let view = array_as_slice(sample);
    stdout_write(view)
}

@id("bytes.opaque")
fn opaque(view: borrow Slice<u8>) -> usize
    uses { process.stdout.write }
{
    stdout_write(view)
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = resolved(source, "hir-byte-capacity-transcript.spx");
    let find = |id: &str| {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .expect("function resolved")
    };
    // A view rooted in a local fixed array carries its exact length, so the
    // transcript budget can be charged statically.
    assert_eq!(
        byte_slice_transcript_source(
            &ValueFactIndex::new(&program),
            stdout_argument(find("bytes.fixed"))
        ),
        TranscriptSource::Fixed(2)
    );
    // A borrowed parameter has no statically known extent, and reporting a
    // length for it would under-charge the transcript budget.
    assert_eq!(
        byte_slice_transcript_source(
            &ValueFactIndex::new(&program),
            stdout_argument(find("bytes.opaque"))
        ),
        TranscriptSource::Unknown
    );
}

#[test]
fn call_targets_resolve_only_through_their_own_template() {
    let source = r#"
module test.hir_byte_capacity_targets;

@id("app.identity")
fn identity<T>(value: T) -> T
{
    value
}

@id("app.plain")
fn plain(value: i64) -> i64
{
    value
}

@id("app.main")
fn main() -> i64
{
    plain(identity<i64>(1))
}
"#;
    let program = resolved(source, "hir-byte-capacity-targets.spx");
    let plain = DeclarationId::new("app.plain");
    let identity = DeclarationId::new("app.identity");

    assert_eq!(
        program
            .resolve_call_target(&plain, None)
            .expect("monomorphic target resolves")
            .name,
        "plain"
    );
    // A generic template is not a callable function on its own; only one of
    // its instances is.
    assert!(program.resolve_call_target(&identity, None).is_none());

    let instance = program
        .function_instances
        .first()
        .expect("one instance discovered");
    assert_eq!(
        program
            .resolve_call_target(&identity, Some(&instance.id))
            .expect("instance target resolves")
            .return_type,
        ResolvedType::I64
    );
    // The template identity is part of the key: an instance must never be
    // reachable through an unrelated callee.
    assert!(program
        .resolve_call_target(&plain, Some(&instance.id))
        .is_none());
}

/// Classification fixture: direct stdin, non-stdin owned bytes under the same
/// authored name, fixed arrays, `let`/`assign`, parameters, ordinary functions
/// and a concrete generic instance (generic functions are effect-free here).
const VALUE_FACTS: &str = r#"
module test.hir_byte_capacity_value_facts;

permit { process.stdin.read, process.stdout.write, process.stderr.write }

@id("io.stdin")
fn echo_stdin() -> usize
    uses { process.stdin.read, process.stdout.write }
{
    let input = stdin_read();
    let view = bytes_as_slice(input);
    stdout_write(view)
}

@id("io.owned")
fn echo_owned() -> usize
    uses { process.stderr.write }
{
    let input = bytes_zeroed(4usize);
    let view = bytes_as_slice(input);
    stderr_write(view)
}

@id("io.fixed")
fn echo_fixed() -> usize
    uses { process.stdout.write }
{
    let sample = [97u8, 98u8, 99u8];
    let view = array_as_slice(sample);
    let mut written = 0usize;
    written = stdout_write(view);
    written
}

@id("io.parameter")
fn echo_parameter(view: borrow Slice<u8>) -> usize
    uses { process.stderr.write }
{
    stderr_write(view)
}

@id("io.generic")
fn relay<T>(value: T) -> T
{
    let kept = value;
    kept
}

@id("app.main")
fn main() -> i64
{
    relay<i64>(0)
}
"#;

fn transcript_sources(program: &ResolvedProgram) -> Vec<(String, TranscriptSource)> {
    use crate::byte_data_capacity::CapacityFlow;
    let mut sources = Vec::new();
    for input in byte_data_capacity_inputs(program).expect("capacity inputs derive") {
        let mut pending = vec![&input.execution];
        while let Some(flow) = pending.pop() {
            match flow {
                CapacityFlow::StdoutWrite { source, .. } => {
                    sources.push((format!("{}:stdout", input.function), *source))
                }
                CapacityFlow::StderrWrite { source, .. } => {
                    sources.push((format!("{}:stderr", input.function), *source))
                }
                CapacityFlow::Sequence(children) | CapacityFlow::Alternative(children) => {
                    pending.extend(children.iter().rev())
                }
                CapacityFlow::Loop { condition, body } => {
                    pending.push(body);
                    pending.push(condition);
                }
                _ => {}
            }
        }
    }
    sources
}

fn with_baseline<T>(baseline: bool, run: impl FnOnce() -> T) -> T {
    USE_BASELINE.with(|flag| flag.set(baseline));
    let result = run();
    USE_BASELINE.with(|flag| flag.set(false));
    result
}

fn reset_counters() {
    for counter in [&BUILDS, &VISITS, &BASELINE_SEARCHES] {
        counter.with(|count| count.set(0));
    }
}

fn counters() -> (usize, usize, usize) {
    (
        BUILDS.with(std::cell::Cell::get),
        VISITS.with(std::cell::Cell::get),
        BASELINE_SEARCHES.with(std::cell::Cell::get),
    )
}

#[test]
fn value_facts_classify_transcripts_conservatively_by_binding_identity() {
    let program = resolved(VALUE_FACTS, "hir-byte-capacity-value-facts.spx");
    assert!(!program.function_instances.is_empty());
    let mut sources = transcript_sources(&program);
    sources.sort();
    // Only a direct `stdin_read` initializer is stdin; owned bytes bound to
    // the same authored name in another function are a different binding and
    // stay unknown, as do borrowed parameters.
    assert_eq!(
        sources,
        [
            ("io.fixed:stdout".to_owned(), TranscriptSource::Fixed(3)),
            ("io.owned:stderr".to_owned(), TranscriptSource::Unknown),
            ("io.parameter:stderr".to_owned(), TranscriptSource::Unknown),
            ("io.stdin:stdout".to_owned(), TranscriptSource::Stdin),
        ]
    );
    let baseline = with_baseline(true, || {
        let mut sources = transcript_sources(&program);
        sources.sort();
        sources
    });
    assert_eq!(baseline, sources);
}

fn many_sites(sites: usize, unrelated: usize) -> String {
    let mut source = String::from(
        "module test.hir_byte_capacity_value_scale;\n\npermit { process.stdin.read, process.stdout.write }\n\n",
    );
    for index in 0..sites {
        source.push_str(&format!(
            "@id(\"io.site{index}\")\nfn site{index}() -> usize\n    uses {{ process.stdin.read, process.stdout.write }}\n{{\n    let input = stdin_read();\n    let view = bytes_as_slice(input);\n    stdout_write(view)\n}}\n\n"
        ));
    }
    for index in 0..unrelated {
        source.push_str(&format!(
            "@id(\"pure.f{index}\")\nfn pure{index}(value: i64) -> i64\n{{\n    let doubled = value + value;\n    let tripled = doubled + value;\n    tripled\n}}\n\n"
        ));
    }
    source.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    source
}

#[test]
fn value_fact_index_is_built_once_per_analysis_and_scales_linearly() {
    let mut visits = Vec::new();
    for (sites, unrelated) in [(1, 0), (4, 0), (16, 0), (4, 12)] {
        let program = resolved(
            &many_sites(sites, unrelated),
            "hir-byte-capacity-value-scale.spx",
        );
        reset_counters();
        analyze_byte_data_capacity(&program).expect("admitted");
        let (builds, node_visits, searches) = counters();
        // One index per complete analysis, no whole-program search per site.
        assert_eq!((builds, searches), (1, 0), "{sites} sites");
        visits.push(node_visits);

        // The replaced route searched the whole program once per lookup.
        reset_counters();
        with_baseline(true, || analyze_byte_data_capacity(&program)).expect("admitted");
        let (builds, _, searches) = counters();
        assert_eq!((builds, searches), (0, sites), "{sites} sites");
    }
    // Node visits are one pass over the program: each extra output site and
    // each unrelated function adds a constant, independent of site count.
    let per_site = visits[1] - visits[0];
    assert_eq!(per_site % 3, 0);
    assert_eq!(visits[2] - visits[1], per_site * 4);
    assert!(visits[3] > visits[1]);
    assert_eq!((visits[3] - visits[1]) % 12, 0);

    // Each analysis builds afresh; nothing is retained across programs.
    let programs =
        [2, 3].map(|sites| resolved(&many_sites(sites, 0), "hir-byte-capacity-value-scale.spx"));
    reset_counters();
    for program in &programs {
        analyze_byte_data_capacity(program).expect("admitted");
    }
    assert_eq!(counters().0, 2);
}

#[test]
fn a_new_program_with_alike_identities_gets_fresh_facts() {
    let stdin = resolved(&many_sites(1, 0), "hir-byte-capacity-fresh.spx");
    let owned = resolved(
        &many_sites(1, 0).replace(
            "let input = stdin_read();",
            "let input = bytes_zeroed(1usize);",
        ),
        "hir-byte-capacity-fresh.spx",
    );
    assert_eq!(
        transcript_sources(&stdin),
        [("io.site0:stdout".to_owned(), TranscriptSource::Stdin)]
    );
    assert_eq!(
        transcript_sources(&owned),
        [("io.site0:stdout".to_owned(), TranscriptSource::Unknown)]
    );
}

/// Every resolvable corpus program yields identical capacity inputs, transcript
/// facts and summaries under the index and the replaced whole-program
/// searches; every refused program yields identical diagnostics.
#[test]
fn value_fact_index_matches_the_baseline_over_the_corpus() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = vec![
        ("value-facts.spx".to_owned(), VALUE_FACTS.to_owned()),
        ("scale.spx".to_owned(), many_sites(3, 2)),
        ("frame-limit.spx".to_owned(), frame_source("")),
        (
            "frame-over.spx".to_owned(),
            frame_source("    let extra = [0u8; 1];\n"),
        ),
    ];
    let mut directories = vec![root.join("examples"), root.join("tests/fixtures")];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut entries = entries
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "spx") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    sources.push((path.display().to_string(), text));
                }
            }
        }
    }
    let (mut admitted, mut refused, mut with_transcripts) = (0, 0, 0);
    type Outcome = Result<
        (
            Vec<crate::byte_data_capacity::FunctionCapacityInput>,
            Result<crate::byte_data_capacity::ProgramCapacitySummary, (String, String)>,
        ),
        Vec<(String, String)>,
    >;
    for (path, text) in &sources {
        let Ok(ast) = crate::parse(text, Path::new(path)) else {
            continue;
        };
        let outcome = |baseline: bool| -> Outcome {
            with_baseline(baseline, || match crate::hir::resolve(&ast) {
                Ok(program) => Ok((
                    byte_data_capacity_inputs(&program).expect("admitted inputs derive"),
                    analyze_byte_data_capacity(&program)
                        .map_err(|diagnostic| (diagnostic.code.to_string(), diagnostic.message)),
                )),
                Err(diagnostics) => Err(diagnostics
                    .into_iter()
                    .map(|diagnostic| (diagnostic.code.to_string(), diagnostic.message))
                    .collect()),
            })
        };
        let indexed = outcome(false);
        assert_eq!(indexed, outcome(true), "{path}");
        match &indexed {
            Ok((inputs, _)) => {
                admitted += 1;
                if inputs.iter().any(|input| {
                    format!("{:?}", input.execution).contains("StdoutWrite")
                        || format!("{:?}", input.execution).contains("StderrWrite")
                }) {
                    with_transcripts += 1;
                }
            }
            Err(_) => refused += 1,
        }
    }
    assert!(admitted >= 40, "{admitted} admitted corpus programs");
    assert!(
        with_transcripts >= 3,
        "{with_transcripts} transcript programs"
    );
    assert!(refused >= 1, "{refused} refused corpus programs");
}

fn bindings_in(expression: &ResolvedExpr, out: &mut Vec<ValueId>) {
    // Unlike the index, this walk enters closure bodies, so their bindings are
    // compared too and must stay outside the index exactly as before.
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::Block { statements, .. } => {
                for statement in statements {
                    if let ResolvedStatement::Let { binding, .. }
                    | ResolvedStatement::Assign { binding, .. } = statement
                    {
                        out.push(binding.id.clone());
                    }
                }
            }
            ResolvedExprKind::Closure { body, .. } => pending.push(body),
            _ => {}
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
}

#[test]
fn every_indexed_fact_equals_the_replaced_whole_program_search() {
    let mut fixtures = vec![
        resolved(VALUE_FACTS, "hir-byte-capacity-value-facts.spx"),
        resolved(&many_sites(2, 2), "hir-byte-capacity-value-scale.spx"),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Larger real programs, including closures, when they resolve standalone.
    for example in [
        "examples/spxgrep-lines-project/src/app.spx",
        "examples/spxgrep-language-command-project/src/app.spx",
        "examples/iterator-operations.spx",
        "examples/lazy-iterator-adapters.spx",
    ] {
        let text = std::fs::read_to_string(root.join(example)).expect("example exists");
        if let Ok(ast) = crate::parse(&text, Path::new(example)) {
            if let Ok(program) = crate::hir::resolve(&ast) {
                fixtures.push(program);
            }
        }
    }
    assert!(fixtures.len() >= 3, "{} fixtures resolved", fixtures.len());
    let mut compared = 0;
    let mut instance_bindings = 0;
    for program in &fixtures {
        let mut values = Vec::new();
        for function in program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ) {
            values.extend(function.params.iter().map(|parameter| parameter.id.clone()));
            bindings_in(&function.body, &mut values);
        }
        for instance in &program.function_instances {
            let mut local = Vec::new();
            bindings_in(&instance.function.body, &mut local);
            instance_bindings += local.len() + instance.function.params.len();
        }
        let index = ValueFactIndex::new(program);
        for value in &values {
            assert_eq!(index.value_type(value), resolved_value_type(program, value));
            assert_eq!(
                index.is_stdin(value),
                resolved_value_is_stdin(program, value)
            );
            compared += 1;
        }
    }
    assert!(compared >= 50, "{compared} bindings compared");
    assert!(
        instance_bindings >= 2,
        "{instance_bindings} instance bindings"
    );
}
