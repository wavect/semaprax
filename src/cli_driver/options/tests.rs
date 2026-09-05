//! Behavioral tests for the bounded command-specific CLI option grammars.
//!
//! Every parser here reads a raw `argv` slice, so the regressions worth
//! guarding are the ones a hand-rolled loop gets wrong: a value-taking flag
//! that swallows the next flag, a duplicate flag that silently wins, a
//! numeric option that wraps instead of refusing, and an index cursor that
//! walks past the end of the slice.

use super::*;

/// Argv as the driver hands it to a parser: `args[0]` is the command name and
/// `args[1]` the positional path, so command options begin at index 2.
fn argv(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_owned()).collect()
}

fn serve(tail: &[&str]) -> Result<agent_transport::TransportLimits, u8> {
    let mut tokens = vec!["serve", "module.spx"];
    tokens.extend_from_slice(tail);
    serve_options(&argv(&tokens))
}

fn interpret(tail: &[&str]) -> Result<(String, Vec<String>, interpreter::InterpreterOptions), u8> {
    let mut tokens = vec!["interpret", "module.spx"];
    tokens.extend_from_slice(tail);
    interpret_options(&argv(&tokens))
}

fn context(tail: &[&str]) -> Result<ParsedContextOptions, u8> {
    let mut tokens = vec!["context", "module.spx", "symbol"];
    tokens.extend_from_slice(tail);
    context_options(&argv(&tokens))
}

/// `ParsedContextOptions` is deliberately not `Debug`, so refusals are
/// compared on the status alone.
fn context_status(tail: &[&str]) -> Result<(), u8> {
    context(tail).map(|_| ())
}

fn cxx_shim(tail: &[&str]) -> Result<(cxx_shim::CxxShimOptions, bool), u8> {
    let mut tokens = vec!["cxx-shim", "module.spx"];
    tokens.extend_from_slice(tail);
    cxx_shim_options(&argv(&tokens))
}

// --- serve -------------------------------------------------------------

#[test]
fn serve_defaults_to_the_transport_default_and_stores_an_explicit_value() {
    assert_eq!(
        serve(&[]).unwrap().max_request_bytes(),
        agent_transport::DEFAULT_MAX_REQUEST_BYTES
    );
    assert_eq!(
        serve(&["--max-request-bytes", "4096"])
            .unwrap()
            .max_request_bytes(),
        4096
    );
}

#[test]
fn serve_enforces_the_transport_window_at_both_edges() {
    // `agent_transport` admits 1024..=1048576. The parser must forward the
    // refusal instead of clamping an out-of-window request.
    assert_eq!(serve(&["--max-request-bytes", "1024"]).map(|_| ()), Ok(()));
    assert_eq!(
        serve(&["--max-request-bytes", "1048576"])
            .unwrap()
            .max_request_bytes(),
        1024 * 1024
    );
    assert_eq!(serve(&["--max-request-bytes", "1023"]).unwrap_err(), 2);
    assert_eq!(serve(&["--max-request-bytes", "1048577"]).unwrap_err(), 2);
    assert_eq!(serve(&["--max-request-bytes", "0"]).unwrap_err(), 2);
}

#[test]
fn serve_rejects_duplicates_unknown_options_and_a_missing_trailing_value() {
    assert_eq!(
        serve(&["--max-request-bytes", "4096", "--max-request-bytes", "8192"]).unwrap_err(),
        2
    );
    assert_eq!(serve(&["--verbose"]).unwrap_err(), 2);
    // An unexpected positional is not distinguished from an unknown flag.
    assert_eq!(serve(&["extra"]).unwrap_err(), 2);
    assert_eq!(serve(&["--max-request-bytes"]).unwrap_err(), 2);
}

#[test]
fn serve_refuses_a_following_flag_as_its_numeric_value() {
    assert_eq!(
        serve(&["--max-request-bytes", "--max-request-bytes"]).unwrap_err(),
        2
    );
}

// --- interpret ---------------------------------------------------------

#[test]
fn interpret_stores_the_function_repeated_args_and_max_bytes() {
    let (function, arguments, options) = interpret(&[
        "--function",
        "main",
        "--arg",
        "1",
        "--arg",
        "2",
        "--max-bytes",
        "4096",
    ])
    .unwrap();
    assert_eq!(function, "main");
    assert_eq!(arguments, vec!["1".to_owned(), "2".to_owned()]);
    assert_eq!(options.max_bytes, 4096);
    // `interpret` has no --max-steps option, so the step budget stays default.
    assert_eq!(
        options.max_steps,
        interpreter::InterpreterOptions::default().max_steps
    );
}

#[test]
fn interpret_requires_a_function_selection() {
    assert_eq!(interpret(&[]).unwrap_err(), 2);
    assert_eq!(interpret(&["--arg", "1"]).unwrap_err(), 2);
}

#[test]
fn interpret_arg_repeats_but_function_and_max_bytes_reject_a_second_use() {
    assert_eq!(
        interpret(&["--function", "a", "--function", "b"]).unwrap_err(),
        2
    );
    assert_eq!(
        interpret(&[
            "--function",
            "a",
            "--max-bytes",
            "4096",
            "--max-bytes",
            "8192"
        ])
        .unwrap_err(),
        2
    );
    // --arg is deliberately outside the duplicate table: it accumulates.
    let (_, arguments, _) = interpret(&["--function", "a", "--arg", "1", "--arg", "1"]).unwrap();
    assert_eq!(arguments, vec!["1".to_owned(), "1".to_owned()]);
}

#[test]
fn interpret_rejects_empty_values_unknown_options_and_missing_trailing_values() {
    assert_eq!(interpret(&["--function", ""]).unwrap_err(), 2);
    assert_eq!(interpret(&["--function", "a", "--arg", ""]).unwrap_err(), 2);
    assert_eq!(interpret(&["--max-steps", "10"]).unwrap_err(), 2);
    assert_eq!(interpret(&["main"]).unwrap_err(), 2);
    assert_eq!(interpret(&["--function"]).unwrap_err(), 2);
    assert_eq!(interpret(&["--function", "a", "--arg"]).unwrap_err(), 2);
    assert_eq!(
        interpret(&["--function", "a", "--max-bytes"]).unwrap_err(),
        2
    );
}

#[test]
fn interpret_max_bytes_honors_the_agent_context_byte_window_without_wrapping() {
    // graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES.
    assert_eq!(
        interpret(&["--function", "a", "--max-bytes", "2048"])
            .unwrap()
            .2
            .max_bytes,
        2048
    );
    assert_eq!(
        interpret(&["--function", "a", "--max-bytes", "16777216"])
            .unwrap()
            .2
            .max_bytes,
        16 * 1024 * 1024
    );
    assert_eq!(
        interpret(&["--function", "a", "--max-bytes", "2047"]).unwrap_err(),
        2
    );
    assert_eq!(
        interpret(&["--function", "a", "--max-bytes", "16777217"]).unwrap_err(),
        2
    );
    assert_eq!(
        interpret(&["--function", "a", "--max-bytes", "0"]).unwrap_err(),
        2
    );
    // A value wider than usize must diagnose, never truncate into range.
    assert_eq!(
        interpret(&[
            "--function",
            "a",
            "--max-bytes",
            "18446744073709551616000000"
        ])
        .unwrap_err(),
        2
    );
}

#[test]
fn interpret_refuses_a_following_flag_as_the_function_name() {
    // A value-less `--function` must be a usage error, not a silently adopted
    // flag name that fails much later as a symbol-not-found. A selection is
    // free text, so only the leading-dash filter can catch this shape.
    assert_eq!(interpret(&["--function", "--arg"]).unwrap_err(), 2);
    assert_eq!(
        interpret(&["--function", "--max-bytes", "4096"]).unwrap_err(),
        2
    );
    // `--max-bytes` needs no filter: a following flag is not a canonical
    // integer, so its own value grammar already refuses one.
    assert_eq!(
        interpret(&["--function", "a", "--max-bytes", "--arg"]).unwrap_err(),
        2
    );
}

#[test]
fn interpret_arg_still_admits_the_negative_scalar_literals_it_documents() {
    // `--arg` is deliberately outside the leading-dash filter: the interpreter
    // argument grammar admits negative integers and floats, so a `-`-prefixed
    // token is a legal value rather than a swallowed flag.
    let (_, arguments, _) = interpret(&[
        "--function",
        "a",
        "--arg",
        "-1",
        "--arg",
        "-0.0",
        "--arg",
        "-2.5e-3",
    ])
    .unwrap();
    assert_eq!(arguments, vec!["-1", "-0.0", "-2.5e-3"]);
    for literal in ["-1", "-0.0", "-2.5e-3"] {
        assert!(interpreter::parse_argument(literal).is_ok(), "{literal}");
    }
}

// --- canonical integer grammar ----------------------------------------

#[test]
fn canonical_integer_values_reject_signs_padding_and_non_digits() {
    for parser in [
        context_number as fn(&str, &str) -> Result<usize, u8>,
        property_number,
    ] {
        assert_eq!(parser("--depth", "0").unwrap(), 0);
        assert_eq!(parser("--depth", "12").unwrap(), 12);
        for rejected in [
            "", "-1", "+1", "01", "1_000", " 1", "1 ", "0x10", "1.0", "٣",
        ] {
            assert_eq!(parser("--depth", rejected).unwrap_err(), 2, "{rejected}");
        }
        // Digits that overflow usize are a diagnostic, never a wrap.
        assert_eq!(parser("--depth", "184467440737095516160").unwrap_err(), 2);
    }
}

#[test]
fn property_seed_spans_the_full_u64_range_and_refuses_one_past_it() {
    assert_eq!(property_seed("--seed", "0").unwrap(), 0);
    assert_eq!(
        property_seed("--seed", "18446744073709551615").unwrap(),
        u64::MAX
    );
    assert_eq!(
        property_seed("--seed", "18446744073709551616").unwrap_err(),
        2
    );
    assert_eq!(property_seed("--seed", "-1").unwrap_err(), 2);
    assert_eq!(property_seed("--seed", "01").unwrap_err(), 2);
}

// --- context -----------------------------------------------------------

#[test]
fn context_defaults_match_the_graph_defaults_and_stay_on_the_v1_surface() {
    let defaults = graph::AgentContextOptions::default();
    let parsed = context(&[]).unwrap();
    match parsed {
        ParsedContextOptions::V1(options) => assert_eq!(options, defaults),
        ParsedContextOptions::V2(_) => panic!("absent --direction must stay on the v1 surface"),
    }
}

#[test]
fn context_direction_is_the_only_switch_onto_the_v2_surface() {
    let parsed = context(&["--direction", "reverse"]).unwrap();
    match parsed {
        ParsedContextOptions::V2(options) => {
            assert_eq!(options.direction(), graph::AgentContextDirection::Reverse);
            assert_eq!(
                options.depth(),
                graph::AgentContextOptions::default().depth()
            );
        }
        ParsedContextOptions::V1(_) => panic!("--direction must select the v2 surface"),
    }
    assert_eq!(context_status(&["--direction", "Forward"]).unwrap_err(), 2);
    assert_eq!(context_status(&["--direction", "sideways"]).unwrap_err(), 2);
    assert_eq!(context_status(&["--direction", ""]).unwrap_err(), 2);
}

#[test]
fn context_stores_every_numeric_option_it_accepts() {
    let parsed = context(&[
        "--depth",
        "3",
        "--max-bytes",
        "4096",
        "--max-nodes",
        "17",
        "--filters",
        "contracts",
    ])
    .unwrap();
    let ParsedContextOptions::V1(options) = parsed else {
        panic!("no --direction was supplied");
    };
    assert_eq!(options.depth(), 3);
    assert_eq!(options.max_bytes(), 4096);
    assert_eq!(options.max_nodes(), 17);
    assert_eq!(
        options,
        graph::AgentContextOptions::new(3, 4096, 17, [graph::AgentContextFilter::Contracts])
            .unwrap()
    );
}

#[test]
fn context_option_order_does_not_change_the_parse() {
    let forward = context(&["--depth", "2", "--max-nodes", "9", "--max-bytes", "4096"]).unwrap();
    let reverse = context(&["--max-bytes", "4096", "--max-nodes", "9", "--depth", "2"]).unwrap();
    let (ParsedContextOptions::V1(forward), ParsedContextOptions::V1(reverse)) = (forward, reverse)
    else {
        panic!("no --direction was supplied");
    };
    assert_eq!(forward, reverse);
}

#[test]
fn context_bounds_reject_one_past_each_limit() {
    // graph limits: depth <= 1024, bytes 2048..=16 MiB, nodes 1..=65_536.
    assert_eq!(context_status(&["--depth", "1024"]), Ok(()));
    assert_eq!(context_status(&["--depth", "1025"]).unwrap_err(), 2);
    assert_eq!(context_status(&["--max-bytes", "2047"]).unwrap_err(), 2);
    assert_eq!(context_status(&["--max-bytes", "16777217"]).unwrap_err(), 2);
    assert_eq!(context_status(&["--max-nodes", "0"]).unwrap_err(), 2);
    assert_eq!(context_status(&["--max-nodes", "65536"]), Ok(()));
    assert_eq!(context_status(&["--max-nodes", "65537"]).unwrap_err(), 2);
}

#[test]
fn context_filters_reject_empty_unknown_and_repeated_names() {
    let parsed = context(&["--filters", "contracts,ownership"]).unwrap();
    assert_eq!(
        parsed.max_bytes(),
        graph::AgentContextOptions::default().max_bytes()
    );
    assert_eq!(context_status(&["--filters", ""]).unwrap_err(), 2);
    assert_eq!(context_status(&["--filters", "contracts,"]).unwrap_err(), 2);
    assert_eq!(context_status(&["--filters", "Contracts"]).unwrap_err(), 2);
    assert_eq!(
        context_status(&["--filters", "contracts,contracts"]).unwrap_err(),
        2
    );
}

#[test]
fn context_rejects_duplicates_unknown_options_and_missing_trailing_values() {
    assert_eq!(
        context_status(&["--depth", "1", "--depth", "2"]).unwrap_err(),
        2
    );
    assert_eq!(
        context_status(&["--direction", "both", "--direction", "both"]).unwrap_err(),
        2
    );
    assert_eq!(context_status(&["--verbose"]).unwrap_err(), 2);
    assert_eq!(context_status(&["module.spx"]).unwrap_err(), 2);
    assert_eq!(context_status(&["--depth"]).unwrap_err(), 2);
    // A following flag is not a canonical integer, so it cannot be swallowed.
    assert_eq!(context_status(&["--depth", "--max-nodes"]).unwrap_err(), 2);
}

// --- context projected onto the workspace surface ----------------------

#[test]
fn project_context_replaces_the_public_byte_budget_with_the_internal_one() {
    let ParsedContextOptions::V1(options) =
        context(&["--max-bytes", "2048", "--depth", "3", "--max-nodes", "9"]).unwrap()
    else {
        panic!("no --direction was supplied");
    };
    let projected = project_context_options(&ParsedContextOptions::V1(options)).unwrap();
    assert_eq!(
        projected,
        workspace_analysis::WorkspaceContextOptions::new(
            workspace_analysis::WorkspaceAnalysisDirection::Forward,
            3,
            16 * 1024 * 1024,
            9,
        )
        .unwrap()
    );
}

#[test]
fn project_context_carries_each_v2_direction_across_the_surface_boundary() {
    for (name, expected) in [
        (
            "forward",
            workspace_analysis::WorkspaceAnalysisDirection::Forward,
        ),
        (
            "reverse",
            workspace_analysis::WorkspaceAnalysisDirection::Reverse,
        ),
        ("both", workspace_analysis::WorkspaceAnalysisDirection::Both),
    ] {
        let parsed = context(&["--direction", name]).unwrap();
        assert_eq!(
            project_context_options(&parsed).unwrap(),
            workspace_analysis::WorkspaceContextOptions::new(
                expected,
                graph::AgentContextOptions::default().depth(),
                16 * 1024 * 1024,
                graph::AgentContextOptions::default().max_nodes(),
            )
            .unwrap()
        );
    }
}

#[test]
fn project_context_refuses_a_node_budget_the_single_file_surface_admits() {
    // The graph surface admits 65_536 nodes; the workspace surface caps at
    // 8208. A node budget between the two parses and then fails closed on
    // projection rather than being silently clamped.
    let parsed = context(&["--max-nodes", "8209"]).unwrap();
    assert_eq!(parsed.max_bytes(), 64 * 1024);
    assert_eq!(project_context_options(&parsed).unwrap_err(), 2);
    let admitted = context(&["--max-nodes", "8208"]).unwrap();
    assert!(project_context_options(&admitted).is_ok());
}

// --- cxx-shim / cxx-package -------------------------------------------

#[test]
fn cxx_shim_accumulates_comma_and_repeated_function_selections_in_order() {
    let (options, emit_fragment) =
        cxx_shim(&["--function", "beta,alpha", "--function", "gamma"]).unwrap();
    assert_eq!(options.functions, vec!["beta", "alpha", "gamma"]);
    assert!(!emit_fragment);
    assert_eq!(
        options.max_bytes,
        cxx_shim::CxxShimOptions::default().max_bytes
    );
}

#[test]
fn cxx_shim_emit_fragment_is_a_valueless_flag_that_rejects_a_second_use() {
    let (_, emit_fragment) = cxx_shim(&["--function", "a", "--emit-fragment"]).unwrap();
    assert!(emit_fragment);
    // The flag consumes no value, so the following option still parses.
    let (options, emit_fragment) =
        cxx_shim(&["--emit-fragment", "--function", "a", "--max-bytes", "4096"]).unwrap();
    assert!(emit_fragment);
    assert_eq!(options.max_bytes, 4096);
    assert_eq!(
        cxx_shim(&["--function", "a", "--emit-fragment", "--emit-fragment"]).unwrap_err(),
        2
    );
}

#[test]
fn cxx_package_does_not_admit_the_shim_only_fragment_flag() {
    let tokens = argv(&["cxx-package", "module.spx", "--function", "a"]);
    assert_eq!(
        cxx_package_options(&tokens).unwrap().functions,
        vec!["a".to_owned()]
    );
    let with_fragment = argv(&[
        "cxx-package",
        "module.spx",
        "--function",
        "a",
        "--emit-fragment",
    ]);
    assert_eq!(cxx_package_options(&with_fragment).unwrap_err(), 2);
}

#[test]
fn cxx_shim_requires_between_one_and_sixty_four_nonempty_selections() {
    assert_eq!(cxx_shim(&[]).unwrap_err(), 2);
    assert_eq!(cxx_shim(&["--function", ""]).unwrap_err(), 2);
    assert_eq!(cxx_shim(&["--function", "a,,b"]).unwrap_err(), 2);
    assert_eq!(cxx_shim(&["--function", "a,a"]).unwrap_err(), 2);

    let names: Vec<String> = (0..64).map(|index| format!("f{index}")).collect();
    let admitted = names.join(",");
    assert_eq!(
        cxx_shim(&["--function", &admitted])
            .unwrap()
            .0
            .functions
            .len(),
        64
    );
    let rejected = format!("{admitted},f64");
    assert_eq!(cxx_shim(&["--function", &rejected]).unwrap_err(), 2);
}

#[test]
fn cxx_shim_rejects_duplicate_max_bytes_unknown_options_and_missing_values() {
    assert_eq!(
        cxx_shim(&[
            "--function",
            "a",
            "--max-bytes",
            "4096",
            "--max-bytes",
            "8192"
        ])
        .unwrap_err(),
        2
    );
    assert_eq!(cxx_shim(&["--function", "a", "--verbose"]).unwrap_err(), 2);
    assert_eq!(cxx_shim(&["a"]).unwrap_err(), 2);
    assert_eq!(cxx_shim(&["--function"]).unwrap_err(), 2);
    assert_eq!(
        cxx_shim(&["--function", "a", "--max-bytes"]).unwrap_err(),
        2
    );
}

#[test]
fn cxx_shim_refuses_a_following_flag_as_a_selection_or_a_budget() {
    // A selection list is free text, so the leading-dash filter is what stops
    // `--function` from adopting the next flag and silently leaving the byte
    // budget at its default. The budget's own integer grammar covers the rest.
    assert_eq!(cxx_shim(&["--function", "--max-bytes"]).unwrap_err(), 2);
    assert_eq!(
        cxx_shim(&["--function", "--emit-fragment", "a"]).unwrap_err(),
        2
    );
    assert_eq!(
        cxx_shim(&["--function", "a", "--max-bytes", "--emit-fragment"]).unwrap_err(),
        2
    );
}

// --- plugin-manifest / ui-schema --------------------------------------

#[test]
fn single_budget_parsers_store_a_value_default_when_absent_and_fail_closed() {
    let plugin = |tail: &[&str]| {
        let mut tokens = vec!["plugin-manifest", "module.spx"];
        tokens.extend_from_slice(tail);
        plugin_manifest_options(&argv(&tokens)).map(|options| options.max_bytes)
    };
    let ui = |tail: &[&str]| {
        let mut tokens = vec!["ui-schema", "module.spx"];
        tokens.extend_from_slice(tail);
        ui_schema_options(&argv(&tokens)).map(|options| options.max_bytes)
    };

    assert_eq!(
        plugin(&[]).unwrap(),
        plugin_manifest::PluginManifestOptions::default().max_bytes
    );
    assert_eq!(
        ui(&[]).unwrap(),
        ui_schema::UiSchemaOptions::default().max_bytes
    );
    for parse in [
        &plugin as &dyn Fn(&[&str]) -> Result<usize, u8>,
        &ui as &dyn Fn(&[&str]) -> Result<usize, u8>,
    ] {
        assert_eq!(parse(&["--max-bytes", "4096"]).unwrap(), 4096);
        assert_eq!(parse(&["--max-bytes", "2047"]).unwrap_err(), 2);
        assert_eq!(parse(&["--max-bytes", "16777217"]).unwrap_err(), 2);
        assert_eq!(
            parse(&["--max-bytes", "4096", "--max-bytes", "8192"]).unwrap_err(),
            2
        );
        assert_eq!(parse(&["--max-bytes"]).unwrap_err(), 2);
        assert_eq!(parse(&["--max-bytes", "--verbose"]).unwrap_err(), 2);
        assert_eq!(parse(&["--verbose"]).unwrap_err(), 2);
        assert_eq!(parse(&["module.spx"]).unwrap_err(), 2);
    }
}

// --- process exit-status projection -----------------------------------

#[test]
fn stdout_failure_carries_the_stable_package_resolve_diagnostic_code() {
    assert_eq!(package_resolver_stdout_error().code, "SPX-I215");
}

#[test]
fn native_executable_suffix_never_replaces_an_existing_extension() {
    // A path that already carries an extension is returned untouched on every
    // platform, so `out.wasm` is never rewritten into `out.exe`.
    assert_eq!(
        with_native_executable_suffix(PathBuf::from("build/out.wasm")),
        PathBuf::from("build/out.wasm")
    );
    let bare = with_native_executable_suffix(PathBuf::from("build/out"));
    if std::env::consts::EXE_EXTENSION.is_empty() {
        assert_eq!(bare, PathBuf::from("build/out"));
    } else {
        assert_eq!(
            bare,
            PathBuf::from("build/out").with_extension(std::env::consts::EXE_EXTENSION)
        );
    }
}
