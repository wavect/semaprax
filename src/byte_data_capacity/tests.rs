use super::*;

fn slot(identity: &str, length: u32) -> ArrayStorageSlot {
    ArrayStorageSlot {
        identity: identity.to_owned(),
        kind: ArrayStorageKind::Binding,
        length,
    }
}

fn leaf(function: &str, slots: &[(&str, u32)]) -> FunctionCapacityInput {
    FunctionCapacityInput {
        function: function.to_owned(),
        array_slots: slots
            .iter()
            .map(|(identity, length)| slot(identity, *length))
            .collect(),
        execution: CapacityFlow::Empty,
    }
}

fn body(function: &str, execution: CapacityFlow) -> FunctionCapacityInput {
    FunctionCapacityInput {
        function: function.to_owned(),
        array_slots: Vec::new(),
        execution,
    }
}

fn call(site: &str, callee: &str) -> CapacityFlow {
    CapacityFlow::Call {
        site: site.to_owned(),
        callee: callee.to_owned(),
    }
}

fn copy(site: &str, conservative_payload_bytes: u64) -> CapacityFlow {
    CapacityFlow::BytesCopy {
        site: site.to_owned(),
        conservative_payload_bytes,
    }
}

fn stdin(site: &str, conservative_payload_bytes: u64) -> CapacityFlow {
    CapacityFlow::StdinRead {
        site: site.to_owned(),
        conservative_payload_bytes,
    }
}

fn stdout(site: &str, source: TranscriptSource) -> CapacityFlow {
    CapacityFlow::StdoutWrite {
        site: site.to_owned(),
        source,
    }
}

fn stderr(site: &str, source: TranscriptSource) -> CapacityFlow {
    CapacityFlow::StderrWrite {
        site: site.to_owned(),
        source,
    }
}

fn stdout_append(site: &str) -> CapacityFlow {
    CapacityFlow::StdoutAppend {
        site: site.to_owned(),
    }
}

fn stderr_append(site: &str) -> CapacityFlow {
    CapacityFlow::StderrAppend {
        site: site.to_owned(),
    }
}

fn repeat(condition: CapacityFlow, body: CapacityFlow) -> CapacityFlow {
    CapacityFlow::Loop {
        condition: Box::new(condition),
        body: Box::new(body),
    }
}

fn summarize(functions: &[FunctionCapacityInput], identity: &str) -> FunctionCapacitySummary {
    analyze(functions)
        .expect("capacity analysis succeeds")
        .function(identity)
        .expect("analyzed function has a summary")
        .clone()
}

/// Asserts the analysis fails, and that it fails with `diagnostic` and a detail
/// naming `needle`, so a regression cannot quietly swap one capacity
/// diagnostic for another.
fn rejects(
    functions: &[FunctionCapacityInput],
    diagnostic: CapacityDiagnostic,
    needle: &str,
) -> CapacityError {
    let error = analyze(functions).expect_err("capacity analysis rejects the input");
    assert_eq!(error.diagnostic, diagnostic, "{}", error.detail);
    assert_eq!(error.diagnostic.code(), diagnostic.code());
    assert!(error.detail.contains(needle), "{}", error.detail);
    error
}

#[test]
fn inline_frame_sums_slots_and_enforces_the_per_slot_and_per_frame_limits() {
    let summary = summarize(&[leaf("f", &[("a", 10), ("b", 20), ("c", 30)])], "f");
    assert_eq!(summary.inline_array_frame_bytes, 60);
    assert_eq!(summary.active_array_call_path_bytes, 60);

    // Both limits are inclusive: a single maximal slot is admitted.
    let widest = u32::try_from(MAX_ARRAY_BYTES).expect("array limit fits u32");
    let summary = summarize(&[leaf("f", &[("a", widest)])], "f");
    assert_eq!(summary.inline_array_frame_bytes, MAX_ARRAY_BYTES);
    assert_eq!(summary.active_array_call_path_bytes, MAX_ARRAY_BYTES);

    rejects(
        &[leaf("f", &[("a", widest + 1)])],
        CapacityDiagnostic::Array,
        "array storage slot `a` has length 65537",
    );
    // Each slot is individually legal, but the frame total is not.
    rejects(
        &[leaf("f", &[("a", widest), ("b", widest)])],
        CapacityDiagnostic::Array,
        "inline array frame uses 131072 bytes",
    );
}

#[test]
fn call_path_bytes_add_the_callee_frame_and_enforce_the_path_limit() {
    let program = [
        leaf("callee", &[("a", 100)]),
        FunctionCapacityInput {
            function: "caller".to_owned(),
            array_slots: vec![slot("b", 10)],
            execution: call("site", "callee"),
        },
    ];
    assert_eq!(
        summarize(&program, "caller").active_array_call_path_bytes,
        110
    );
    assert_eq!(
        summarize(&program, "callee").active_array_call_path_bytes,
        100
    );

    let widest = u32::try_from(MAX_ARRAY_BYTES).expect("array limit fits u32");
    // A caller with no storage of its own inherits exactly the callee path.
    let program = [
        leaf("callee", &[("a", widest)]),
        body("caller", call("site", "callee")),
    ];
    assert_eq!(
        summarize(&program, "caller").active_array_call_path_bytes,
        MAX_ACTIVE_ARRAY_CALL_PATH_BYTES
    );

    // One more byte anywhere on the path is refused even though every frame is
    // individually legal.
    let program = [
        leaf("callee", &[("a", widest)]),
        FunctionCapacityInput {
            function: "caller".to_owned(),
            array_slots: vec![slot("b", 1)],
            execution: call("site", "callee"),
        },
    ];
    rejects(
        &program,
        CapacityDiagnostic::Array,
        "active array call-path uses 65537 bytes",
    );
}

#[test]
fn alternative_takes_the_branch_maximum_where_sequence_sums() {
    let leaves = || [leaf("small", &[("a", 100)]), leaf("large", &[("b", 300)])];
    let branches = || [call("one", "small"), call("two", "large")];

    let mut sequence = Vec::from(leaves());
    sequence.push(body(
        "caller",
        CapacityFlow::Sequence(Vec::from(branches())),
    ));
    assert_eq!(
        summarize(&sequence, "caller").active_array_call_path_bytes,
        400
    );

    let mut alternative = Vec::from(leaves());
    alternative.push(body(
        "caller",
        CapacityFlow::Alternative(Vec::from(branches())),
    ));
    assert_eq!(
        summarize(&alternative, "caller").active_array_call_path_bytes,
        300
    );

    // Allocation folds the same way: sites and payload bytes add along a
    // sequence and take the maximum across alternatives.
    let copies = || [copy("one", 100), copy("two", 300)];
    let sequence = [body("f", CapacityFlow::Sequence(Vec::from(copies())))];
    let summary = summarize(&sequence, "f");
    assert_eq!(summary.bytes_copy_sites, 2);
    assert_eq!(summary.owned_byte_payload_bytes, 400);

    let alternative = [body("f", CapacityFlow::Alternative(Vec::from(copies())))];
    let summary = summarize(&alternative, "f");
    assert_eq!(summary.bytes_copy_sites, 1);
    assert_eq!(summary.owned_byte_payload_bytes, 300);
}

#[test]
fn a_loop_body_counts_its_call_path_once_rather_than_per_iteration() {
    let program = [
        leaf("callee", &[("a", 100)]),
        body(
            "caller",
            repeat(CapacityFlow::Empty, call("site", "callee")),
        ),
    ];
    assert_eq!(
        summarize(&program, "caller").active_array_call_path_bytes,
        100
    );
}

#[test]
fn cycle_detection_is_scoped_to_the_relevant_call_closure() {
    // A cycle that reaches no tracked resource is not part of any relevant
    // closure, so capacity analysis admits it.
    let summary = summarize(&[body("f", call("site", "f"))], "f");
    assert_eq!(summary.active_array_call_path_bytes, 0);
    assert_eq!(summary.bytes_copy_sites, 0);

    // The same cycle is refused once it can reach inline array storage.
    rejects(
        &[FunctionCapacityInput {
            function: "f".to_owned(),
            array_slots: vec![slot("a", 1)],
            execution: call("site", "f"),
        }],
        CapacityDiagnostic::Array,
        "call-graph cycle can reach nonzero inline array storage",
    );

    // Each closure reports its own diagnostic, so a mutual recursion that only
    // copies bytes is an allocation failure, not an array failure.
    rejects(
        &[
            body("a", call("a-site", "b")),
            body(
                "b",
                CapacityFlow::Sequence(vec![call("b-site", "a"), copy("copy", 1)]),
            ),
        ],
        CapacityDiagnostic::Allocation,
        "bytes_copy executable closure is cyclic",
    );
    rejects(
        &[
            body("a", call("a-site", "b")),
            body(
                "b",
                CapacityFlow::Sequence(vec![
                    call("b-site", "a"),
                    stdout("out", TranscriptSource::Fixed(1)),
                ]),
            ),
        ],
        CapacityDiagnostic::Transcript,
        "transcript-write executable closure is cyclic",
    );
}

#[test]
fn bytes_copy_site_and_payload_limits_are_inclusive() {
    let sites = usize::try_from(MAX_BYTES_COPY_SITES).expect("site limit fits usize");
    let saturating = (0..sites)
        .map(|index| copy(&format!("site-{index}"), MAX_ARRAY_BYTES))
        .collect::<Vec<_>>();
    let summary = summarize(&[body("f", CapacityFlow::Sequence(saturating))], "f");
    assert_eq!(summary.bytes_copy_sites, MAX_BYTES_COPY_SITES);
    // Sixteen maximal per-value copies land exactly on the owned payload limit.
    assert_eq!(
        summary.owned_byte_payload_bytes,
        MAX_OWNED_BYTE_PAYLOAD_BYTES
    );

    // A stdin read adds payload bytes without adding a bytes_copy site, so it
    // is the only way to push a legal number of sites past the owned payload
    // limit.
    let mut saturating = (0..sites)
        .map(|index| copy(&format!("site-{index}"), MAX_ARRAY_BYTES))
        .collect::<Vec<_>>();
    saturating.push(stdin("stdin", MAX_ARRAY_BYTES));
    rejects(
        &[body("f", CapacityFlow::Sequence(saturating))],
        CapacityDiagnostic::Allocation,
        "bytes_copy path admits 1114112 payload bytes",
    );

    let one_too_many = (0..=sites)
        .map(|index| copy(&format!("site-{index}"), 1))
        .collect::<Vec<_>>();
    rejects(
        &[body("f", CapacityFlow::Sequence(one_too_many))],
        CapacityDiagnostic::Allocation,
        "bytes_copy path reaches 17 sites",
    );

    // The per-value bound is checked while validating the flow, before any
    // path arithmetic runs.
    rejects(
        &[body("f", copy("site", MAX_ARRAY_BYTES + 1))],
        CapacityDiagnostic::Allocation,
        "bytes_copy site `site` admits 65537 bytes",
    );
    rejects(
        &[body("f", stdin("site", MAX_ARRAY_BYTES + 1))],
        CapacityDiagnostic::Allocation,
        "stdin_read site `site` admits 65537 bytes",
    );
}

#[test]
fn stdin_is_readable_at_most_once_per_executable_path() {
    rejects(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![stdin("one", 8), stdin("two", 8)]),
        )],
        CapacityDiagnostic::Allocation,
        "stdin_read path reaches 2 sites",
    );
    // Two reads on mutually exclusive branches are two paths of one read each.
    let summary = summarize(
        &[body(
            "f",
            CapacityFlow::Alternative(vec![stdin("one", 8), stdin("two", 8)]),
        )],
        "f",
    );
    assert_eq!(summary.stdin_read_sites, MAX_STDIN_READS_PER_PATH);
}

#[test]
fn bytes_copy_and_stdin_cannot_appear_under_a_loop() {
    for flow in [
        repeat(CapacityFlow::Empty, copy("site", 1)),
        repeat(copy("site", 1), CapacityFlow::Empty),
        repeat(CapacityFlow::Empty, stdin("site", 1)),
    ] {
        rejects(
            &[body("f", flow)],
            CapacityDiagnostic::Allocation,
            "bytes_copy is reachable from a while condition or body",
        );
    }
    // A loop with no allocation inside it is unaffected.
    let program = [FunctionCapacityInput {
        function: "f".to_owned(),
        array_slots: vec![slot("a", 8)],
        execution: repeat(CapacityFlow::Empty, CapacityFlow::Empty),
    }];
    assert_eq!(summarize(&program, "f").active_array_call_path_bytes, 8);
}

#[test]
fn transcript_bytes_sum_within_a_path_and_take_the_maximum_across_paths() {
    let summary = summarize(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![
                stdout("out", TranscriptSource::Fixed(10)),
                stderr("err", TranscriptSource::Fixed(20)),
            ]),
        )],
        "f",
    );
    assert_eq!(summary.stdout_write_sites, 1);
    assert_eq!(summary.stderr_write_sites, 1);
    assert_eq!(summary.transcript_bytes, 30);

    let summary = summarize(
        &[body(
            "f",
            CapacityFlow::Alternative(vec![
                stdout("one", TranscriptSource::Fixed(10)),
                stdout("two", TranscriptSource::Fixed(20)),
            ]),
        )],
        "f",
    );
    assert_eq!(summary.stdout_write_sites, MAX_STDOUT_WRITES_PER_PATH);
    assert_eq!(summary.transcript_bytes, 20);

    // The combined limit is inclusive.
    let summary = summarize(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![
                stdout(
                    "out",
                    TranscriptSource::Fixed(MAX_COMBINED_TRANSCRIPT_BYTES - 1),
                ),
                stderr("err", TranscriptSource::Fixed(1)),
            ]),
        )],
        "f",
    );
    assert_eq!(summary.transcript_bytes, MAX_COMBINED_TRANSCRIPT_BYTES);
    rejects(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![
                stdout(
                    "out",
                    TranscriptSource::Fixed(MAX_COMBINED_TRANSCRIPT_BYTES),
                ),
                stderr("err", TranscriptSource::Fixed(1)),
            ]),
        )],
        CapacityDiagnostic::Transcript,
        "combined stdout/stderr path admits 65537 bytes",
    );

    rejects(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![
                stdout("one", TranscriptSource::Fixed(1)),
                stdout("two", TranscriptSource::Fixed(1)),
            ]),
        )],
        CapacityDiagnostic::Transcript,
        "stdout_write path reaches 2 sites",
    );
    rejects(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![
                stderr("one", TranscriptSource::Fixed(1)),
                stderr("two", TranscriptSource::Fixed(1)),
            ]),
        )],
        CapacityDiagnostic::Transcript,
        "stderr_write path reaches 2 sites",
    );
}

#[test]
fn invocation_bounded_transcript_roots_cannot_be_mixed_or_republished() {
    // A single invocation-bounded root charges the whole transcript budget.
    for source in [
        TranscriptSource::CommandArguments,
        TranscriptSource::Stdin,
        TranscriptSource::Unknown,
    ] {
        let summary = summarize(&[body("f", stdout("out", source))], "f");
        assert_eq!(summary.transcript_bytes, MAX_COMBINED_TRANSCRIPT_BYTES);
    }

    for (source, needle) in [
        (
            TranscriptSource::CommandArguments,
            "one command-argument root is published more than once",
        ),
        (
            TranscriptSource::Stdin,
            "one stdin root is published more than once",
        ),
    ] {
        rejects(
            &[body(
                "f",
                CapacityFlow::Sequence(vec![stdout("out", source), stderr("err", source)]),
            )],
            CapacityDiagnostic::Transcript,
            needle,
        );
    }

    rejects(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![
                stdout("out", TranscriptSource::Unknown),
                stderr("err", TranscriptSource::Fixed(5)),
            ]),
        )],
        CapacityDiagnostic::Transcript,
        "an unauthenticated slice root cannot share a transcript path",
    );
    rejects(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![
                stdout("out", TranscriptSource::Fixed(5)),
                stderr("err", TranscriptSource::CommandArguments),
            ]),
        )],
        CapacityDiagnostic::Transcript,
        "fixed bytes cannot be added to an invocation-bounded transcript root",
    );
}

#[test]
fn appends_are_runtime_bounded_and_never_share_a_path_with_a_legacy_write() {
    let summary = summarize(&[body("f", stdout_append("out"))], "f");
    assert_eq!(summary.stdout_append_sites, 1);
    assert_eq!(summary.stdout_write_sites, 0);
    assert_eq!(summary.transcript_bytes, MAX_COMBINED_TRANSCRIPT_BYTES);

    // Appends are not subject to the one-write-per-path rule that governs the
    // legacy write operations; the runtime owns their cumulative bound.
    let summary = summarize(
        &[body(
            "f",
            CapacityFlow::Sequence(vec![
                stdout_append("one"),
                stdout_append("two"),
                stderr_append("three"),
            ]),
        )],
        "f",
    );
    assert_eq!(summary.stdout_append_sites, 2);
    assert_eq!(summary.stderr_append_sites, 1);
    assert_eq!(summary.transcript_bytes, MAX_COMBINED_TRANSCRIPT_BYTES);

    for flow in [
        CapacityFlow::Sequence(vec![
            stdout("out", TranscriptSource::Fixed(1)),
            stderr_append("append"),
        ]),
        CapacityFlow::Sequence(vec![
            stdout_append("append"),
            stderr("err", TranscriptSource::Fixed(1)),
        ]),
    ] {
        rejects(
            &[body("f", flow)],
            CapacityDiagnostic::Transcript,
            "legacy transcript writes and runtime-bounded appends cannot share an executable path",
        );
    }
}

#[test]
fn a_while_admits_only_direct_append_output_in_its_body() {
    rejects(
        &[body(
            "f",
            repeat(
                stdout("out", TranscriptSource::Fixed(1)),
                CapacityFlow::Empty,
            ),
        )],
        CapacityDiagnostic::Transcript,
        "transcript output is reachable from a while condition",
    );
    rejects(
        &[body("f", repeat(stdout_append("out"), CapacityFlow::Empty))],
        CapacityDiagnostic::Transcript,
        "transcript output is reachable from a while condition",
    );
    rejects(
        &[body(
            "f",
            repeat(
                CapacityFlow::Empty,
                stdout("out", TranscriptSource::Fixed(1)),
            ),
        )],
        CapacityDiagnostic::Transcript,
        "a while body may contain only direct runtime-bounded append output",
    );

    // A direct append in the body is the one admitted shape.
    let summary = summarize(
        &[body("f", repeat(CapacityFlow::Empty, stdout_append("out")))],
        "f",
    );
    assert_eq!(summary.stdout_append_sites, 1);
    assert_eq!(summary.transcript_bytes, MAX_COMBINED_TRANSCRIPT_BYTES);

    // The same append reached through a call is indirect, and a while body may
    // not contain it.
    rejects(
        &[
            body("appender", stdout_append("out")),
            body("f", repeat(CapacityFlow::Empty, call("site", "appender"))),
        ],
        CapacityDiagnostic::Transcript,
        "a while body may contain only direct runtime-bounded append output",
    );
}

#[test]
fn hostile_projections_are_refused_as_compiler_invariants() {
    for (program, needle) in [
        (
            vec![body("", CapacityFlow::Empty)],
            "function identity must be nonempty and NUL-free",
        ),
        (
            vec![body("f\0g", CapacityFlow::Empty)],
            "function identity must be nonempty and NUL-free",
        ),
        (
            vec![
                body("f", CapacityFlow::Empty),
                body("f", CapacityFlow::Empty),
            ],
            "duplicate function identity in byte-data capacity input",
        ),
        (
            vec![body("f", call("site", "missing"))],
            "unknown capacity callee `missing`",
        ),
        (
            vec![body("f", call("", "f"))],
            "call site identity must be nonempty and NUL-free",
        ),
        (
            vec![leaf("f", &[("", 1)])],
            "array storage slot identity must be nonempty and NUL-free",
        ),
        (
            vec![leaf("f", &[("a", 1), ("a", 2)])],
            "duplicate array storage slot `a`",
        ),
        // Call, copy, and write sites share one per-function identity space.
        (
            vec![body(
                "f",
                CapacityFlow::Sequence(vec![copy("s", 1), stdout("s", TranscriptSource::Fixed(1))]),
            )],
            "duplicate capacity site `s`",
        ),
        (
            vec![body(
                "f",
                CapacityFlow::Sequence(
                    (0..=MAX_FLOW_NODES)
                        .map(|_| CapacityFlow::Empty)
                        .collect::<Vec<_>>(),
                ),
            )],
            "capacity flow exceeds the compiler node bound",
        ),
    ] {
        rejects(&program, CapacityDiagnostic::Invariant, needle);
    }

    // The program-wide function bound is reported without a function name.
    let too_many = (0..=MAX_FUNCTIONS)
        .map(|index| body(&format!("f{index}"), CapacityFlow::Empty))
        .collect::<Vec<_>>();
    let error = rejects(
        &too_many,
        CapacityDiagnostic::Invariant,
        "byte-data capacity function count exceeds the compiler bound",
    );
    assert!(error.function.is_none());
}
