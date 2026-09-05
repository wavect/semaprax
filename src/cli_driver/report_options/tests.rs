//! Behavioral tests for the report and analysis option grammars.
//!
//! These parsers all walk `argv` by hand from a command-specific start index,
//! so the regressions worth guarding are index drift, a duplicate flag that
//! silently wins, a numeric value that wraps instead of refusing, and a
//! selection list that quietly accepts an empty or repeated entry.

use super::*;

fn argv(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_owned()).collect()
}

/// `workspace context|impact` options start at index 5:
/// `<command> <root> <entry> <kind> <target>`.
fn workspace_argv(tail: &[&str]) -> Vec<String> {
    let mut tokens = vec!["workspace", "root", "app", "declaration", "target"];
    tokens.extend_from_slice(tail);
    argv(&tokens)
}

/// `impact <source> <patch>` puts options at index 3.
fn impact_argv(tail: &[&str]) -> Vec<String> {
    let mut tokens = vec!["impact", "module.spx", "patch.json"];
    tokens.extend_from_slice(tail);
    argv(&tokens)
}

/// Most single-file report commands are `<command> <path>`: options at 2.
fn report_argv(tail: &[&str]) -> Vec<String> {
    let mut tokens = vec!["report", "module.spx"];
    tokens.extend_from_slice(tail);
    argv(&tokens)
}

// --- workspace analysis target kind ------------------------------------

#[test]
fn workspace_target_kind_admits_exactly_two_case_sensitive_names() {
    assert_eq!(
        workspace_analysis_target_kind("workspace-context", "declaration").unwrap(),
        workspace_analysis::WorkspaceAnalysisTargetKind::Declaration
    );
    assert_eq!(
        workspace_analysis_target_kind("workspace-context", "capability").unwrap(),
        workspace_analysis::WorkspaceAnalysisTargetKind::Capability
    );
    for rejected in ["Declaration", "decl", "", "declaration,capability"] {
        assert_eq!(
            workspace_analysis_target_kind("workspace-context", rejected).unwrap_err(),
            2,
            "{rejected}"
        );
    }
}

// --- workspace context --------------------------------------------------

#[test]
fn workspace_context_defaults_are_both_depth_four_one_mib_and_1024_nodes() {
    assert_eq!(
        workspace_context_options(&workspace_argv(&[])).unwrap(),
        workspace_analysis::WorkspaceContextOptions::new(
            workspace_analysis::WorkspaceAnalysisDirection::Both,
            4,
            1024 * 1024,
            1024,
        )
        .unwrap()
    );
}

#[test]
fn workspace_context_stores_every_option_and_ignores_their_order() {
    let expected = workspace_analysis::WorkspaceContextOptions::new(
        workspace_analysis::WorkspaceAnalysisDirection::Reverse,
        7,
        8192,
        11,
    )
    .unwrap();
    assert_eq!(
        workspace_context_options(&workspace_argv(&[
            "--direction",
            "reverse",
            "--depth",
            "7",
            "--max-bytes",
            "8192",
            "--max-nodes",
            "11",
        ]))
        .unwrap(),
        expected
    );
    assert_eq!(
        workspace_context_options(&workspace_argv(&[
            "--max-nodes",
            "11",
            "--max-bytes",
            "8192",
            "--depth",
            "7",
            "--direction",
            "reverse",
        ]))
        .unwrap(),
        expected
    );
}

#[test]
fn workspace_context_direction_names_are_exact() {
    for name in ["forward", "reverse", "both"] {
        assert!(workspace_context_options(&workspace_argv(&["--direction", name])).is_ok());
    }
    for rejected in ["Forward", "", "up"] {
        assert_eq!(
            workspace_context_options(&workspace_argv(&["--direction", rejected])).unwrap_err(),
            2,
            "{rejected}"
        );
    }
}

#[test]
fn workspace_context_rejects_duplicates_unknown_options_and_missing_values() {
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--depth", "1", "--depth", "2"])).unwrap_err(),
        2
    );
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--verbose"])).unwrap_err(),
        2
    );
    // An unexpected positional after the closed target list is an error, not
    // a silently ignored token.
    assert_eq!(
        workspace_context_options(&workspace_argv(&["extra"])).unwrap_err(),
        2
    );
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--depth"])).unwrap_err(),
        2
    );
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--depth", "--max-nodes"])).unwrap_err(),
        2
    );
}

#[test]
fn workspace_analysis_bounds_reject_one_past_each_limit() {
    // workspace_analysis admits depth <= 1024, bytes 4096..=16 MiB,
    // nodes 1..=8208. These are tighter than the single-file graph limits,
    // so a value the graph surface admits can still fail closed here.
    assert!(workspace_context_options(&workspace_argv(&["--depth", "1024"])).is_ok());
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--depth", "1025"])).unwrap_err(),
        2
    );
    assert!(workspace_context_options(&workspace_argv(&["--max-bytes", "4096"])).is_ok());
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--max-bytes", "4095"])).unwrap_err(),
        2
    );
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--max-bytes", "16777217"])).unwrap_err(),
        2
    );
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--max-nodes", "0"])).unwrap_err(),
        2
    );
    assert!(workspace_context_options(&workspace_argv(&["--max-nodes", "8208"])).is_ok());
    assert_eq!(
        workspace_context_options(&workspace_argv(&["--max-nodes", "8209"])).unwrap_err(),
        2
    );
}

// --- workspace impact ---------------------------------------------------

#[test]
fn workspace_impact_defaults_to_depth_sixteen_and_has_no_direction_option() {
    assert_eq!(
        workspace_impact_options(&workspace_argv(&[])).unwrap(),
        workspace_analysis::WorkspaceImpactOptions::new(16, 1024 * 1024, 1024).unwrap()
    );
    // `--direction` belongs to workspace-context alone; impact must refuse it
    // rather than accept and ignore it.
    assert_eq!(
        workspace_impact_options(&workspace_argv(&["--direction", "forward"])).unwrap_err(),
        2
    );
}

#[test]
fn workspace_impact_stores_each_numeric_option_and_rejects_duplicates() {
    assert_eq!(
        workspace_impact_options(&workspace_argv(&[
            "--depth",
            "3",
            "--max-bytes",
            "8192",
            "--max-nodes",
            "5",
        ]))
        .unwrap(),
        workspace_analysis::WorkspaceImpactOptions::new(3, 8192, 5).unwrap()
    );
    assert_eq!(
        workspace_impact_options(&workspace_argv(&["--depth", "3", "--depth", "3"])).unwrap_err(),
        2
    );
    assert_eq!(
        workspace_impact_options(&workspace_argv(&["--max-nodes"])).unwrap_err(),
        2
    );
}

// --- semantic impact ----------------------------------------------------

#[test]
fn impact_defaults_and_stores_every_option_regardless_of_order() {
    assert_eq!(
        impact_options(&impact_argv(&[])).unwrap(),
        impact::SemanticImpactOptions::new(1, 64 * 1024, 256).unwrap()
    );
    let expected = impact::SemanticImpactOptions::new(5, 4096, 12).unwrap();
    assert_eq!(
        impact_options(&impact_argv(&[
            "--depth",
            "5",
            "--max-bytes",
            "4096",
            "--max-nodes",
            "12"
        ]))
        .unwrap(),
        expected
    );
    assert_eq!(
        impact_options(&impact_argv(&[
            "--max-nodes",
            "12",
            "--depth",
            "5",
            "--max-bytes",
            "4096"
        ]))
        .unwrap(),
        expected
    );
}

#[test]
fn impact_bounds_reject_one_past_each_limit_and_never_wrap() {
    assert!(impact_options(&impact_argv(&["--depth", "1024"])).is_ok());
    assert_eq!(
        impact_options(&impact_argv(&["--depth", "1025"])).unwrap_err(),
        2
    );
    assert_eq!(
        impact_options(&impact_argv(&["--max-bytes", "2047"])).unwrap_err(),
        2
    );
    assert_eq!(
        impact_options(&impact_argv(&["--max-bytes", "16777217"])).unwrap_err(),
        2
    );
    assert_eq!(
        impact_options(&impact_argv(&["--max-nodes", "0"])).unwrap_err(),
        2
    );
    assert_eq!(
        impact_options(&impact_argv(&["--max-nodes", "65537"])).unwrap_err(),
        2
    );
    // Digit strings wider than usize diagnose instead of truncating.
    assert_eq!(
        impact_options(&impact_argv(&["--max-bytes", "184467440737095516160"])).unwrap_err(),
        2
    );
    assert_eq!(
        impact_options(&impact_argv(&["--depth", "-1"])).unwrap_err(),
        2
    );
    assert_eq!(
        impact_options(&impact_argv(&["--depth", "007"])).unwrap_err(),
        2
    );
}

#[test]
fn impact_rejects_duplicates_unknown_options_and_a_trailing_valueless_flag() {
    assert_eq!(
        impact_options(&impact_argv(&["--depth", "1", "--depth", "1"])).unwrap_err(),
        2
    );
    assert_eq!(impact_options(&impact_argv(&["--filters"])).unwrap_err(), 2);
    assert_eq!(
        impact_options(&impact_argv(&["patch.json"])).unwrap_err(),
        2
    );
    assert_eq!(impact_options(&impact_argv(&["--depth"])).unwrap_err(), 2);
    assert_eq!(
        impact_options(&impact_argv(&["--depth", "--max-bytes"])).unwrap_err(),
        2
    );
}

// --- openapi ------------------------------------------------------------

#[test]
fn openapi_requires_a_selection_and_keeps_repeated_functions_in_argv_order() {
    assert_eq!(openapi_options(&report_argv(&[])).unwrap_err(), 2);
    assert_eq!(
        openapi_options(&report_argv(&["--max-bytes", "4096"])).unwrap_err(),
        2
    );
    let (functions, options) = openapi_options(&report_argv(&[
        "--function",
        "beta",
        "--function",
        "alpha",
        "--max-bytes",
        "4096",
    ]))
    .unwrap();
    assert_eq!(functions, vec!["beta".to_owned(), "alpha".to_owned()]);
    assert_eq!(options.max_bytes, 4096);
}

#[test]
fn openapi_duplicate_rules_differ_between_function_and_max_bytes() {
    // `--function` is deliberately outside the duplicate table so selections
    // accumulate; the repeated-name refusal belongs to `openapi::render`.
    let (functions, _) =
        openapi_options(&report_argv(&["--function", "a", "--function", "a"])).unwrap();
    assert_eq!(functions, vec!["a".to_owned(), "a".to_owned()]);
    assert_eq!(
        openapi_options(&report_argv(&[
            "--function",
            "a",
            "--max-bytes",
            "4096",
            "--max-bytes",
            "8192",
        ]))
        .unwrap_err(),
        2
    );
}

#[test]
fn openapi_rejects_empty_selections_unknown_options_and_missing_values() {
    assert_eq!(
        openapi_options(&report_argv(&["--function", ""])).unwrap_err(),
        2
    );
    assert_eq!(
        openapi_options(&report_argv(&["--function", "a", "--verbose", "x"])).unwrap_err(),
        2
    );
    assert_eq!(openapi_options(&report_argv(&["a"])).unwrap_err(), 2);
    assert_eq!(
        openapi_options(&report_argv(&["--function"])).unwrap_err(),
        2
    );
    assert_eq!(
        openapi_options(&report_argv(&["--function", "a", "--max-bytes"])).unwrap_err(),
        2
    );
    assert_eq!(
        openapi_options(&report_argv(&["--function", "a", "--max-bytes", "2047"])).unwrap_err(),
        2
    );
}

#[test]
fn openapi_function_swallows_a_following_flag_as_a_selection_name() {
    // Documented current behavior: `--function` stores the next argv token
    // verbatim, so `--function --max-bytes` selects the literal symbol
    // `--max-bytes` and the byte budget stays at its default.
    let (functions, options) =
        openapi_options(&report_argv(&["--function", "--max-bytes"])).unwrap();
    assert_eq!(functions, vec!["--max-bytes".to_owned()]);
    assert_eq!(
        options.max_bytes,
        openapi::OpenApiOptions::default().max_bytes
    );
}

#[test]
fn openapi_compat_reads_its_options_from_index_three() {
    let tokens = argv(&["openapi-compat", "baseline.json", "module.spx"]);
    assert_eq!(
        openapi_compat_options(&tokens).unwrap().max_bytes,
        openapi::OpenApiOptions::default().max_bytes
    );
    let with_budget = argv(&[
        "openapi-compat",
        "baseline.json",
        "module.spx",
        "--max-bytes",
        "4096",
    ]);
    assert_eq!(
        openapi_compat_options(&with_budget).unwrap().max_bytes,
        4096
    );
    // openapi-compat has no --function surface.
    let with_function = argv(&[
        "openapi-compat",
        "baseline.json",
        "module.spx",
        "--function",
        "a",
    ]);
    assert_eq!(openapi_compat_options(&with_function).unwrap_err(), 2);
    let duplicated = argv(&[
        "openapi-compat",
        "baseline.json",
        "module.spx",
        "--max-bytes",
        "4096",
        "--max-bytes",
        "4096",
    ]);
    assert_eq!(openapi_compat_options(&duplicated).unwrap_err(), 2);
}

// --- properties ---------------------------------------------------------

#[test]
fn properties_defaults_and_stores_all_four_options_in_any_order() {
    assert_eq!(
        property_options(&report_argv(&[])).unwrap(),
        properties::PropertyTestOptions::default()
    );
    let expected = properties::PropertyTestOptions::new(8, 4, 4096, 7).unwrap();
    assert_eq!(
        property_options(&report_argv(&[
            "--max-cases",
            "8",
            "--max-functions",
            "4",
            "--max-bytes",
            "4096",
            "--seed",
            "7",
        ]))
        .unwrap(),
        expected
    );
    assert_eq!(
        property_options(&report_argv(&[
            "--seed",
            "7",
            "--max-bytes",
            "4096",
            "--max-functions",
            "4",
            "--max-cases",
            "8",
        ]))
        .unwrap(),
        expected
    );
}

#[test]
fn properties_bounds_reject_zero_and_one_past_each_case_and_function_limit() {
    assert_eq!(
        property_options(&report_argv(&["--max-cases", "0"])).unwrap_err(),
        2
    );
    assert!(property_options(&report_argv(&["--max-cases", "4096"])).is_ok());
    assert_eq!(
        property_options(&report_argv(&["--max-cases", "4097"])).unwrap_err(),
        2
    );
    assert_eq!(
        property_options(&report_argv(&["--max-functions", "0"])).unwrap_err(),
        2
    );
    assert!(property_options(&report_argv(&["--max-functions", "1024"])).is_ok());
    assert_eq!(
        property_options(&report_argv(&["--max-functions", "1025"])).unwrap_err(),
        2
    );
    assert_eq!(
        property_options(&report_argv(&["--max-bytes", "2047"])).unwrap_err(),
        2
    );
}

#[test]
fn properties_seed_accepts_the_whole_u64_range_and_refuses_beyond_it() {
    assert_eq!(
        property_options(&report_argv(&["--seed", "0"]))
            .unwrap()
            .seed,
        0
    );
    assert_eq!(
        property_options(&report_argv(&["--seed", "18446744073709551615"]))
            .unwrap()
            .seed,
        u64::MAX
    );
    assert_eq!(
        property_options(&report_argv(&["--seed", "18446744073709551616"])).unwrap_err(),
        2
    );
    assert_eq!(
        property_options(&report_argv(&["--seed", "-1"])).unwrap_err(),
        2
    );
}

#[test]
fn properties_rejects_duplicates_unknown_options_and_missing_values() {
    for option in ["--max-cases", "--max-functions", "--max-bytes", "--seed"] {
        assert_eq!(
            property_options(&report_argv(&[option, "8", option, "8"])).unwrap_err(),
            2,
            "{option}"
        );
        assert_eq!(
            property_options(&report_argv(&[option])).unwrap_err(),
            2,
            "{option}"
        );
    }
    assert_eq!(property_options(&report_argv(&["--cases"])).unwrap_err(), 2);
    assert_eq!(property_options(&report_argv(&["64"])).unwrap_err(), 2);
    assert_eq!(
        property_options(&report_argv(&["--max-cases", "--seed"])).unwrap_err(),
        2
    );
}

// --- hygienic generation ------------------------------------------------

#[test]
fn hygienic_selects_the_whole_registry_when_no_templates_are_named() {
    let options = hygienic_options(&report_argv(&[])).unwrap();
    assert_eq!(options.templates(), &hygienic::Template::REGISTRY[..]);
    assert_eq!(
        options.max_bytes(),
        hygienic::HygienicGenOptions::default().max_bytes()
    );
}

#[test]
fn hygienic_templates_are_canonicalized_to_registry_order() {
    // The parser preserves argv order; the constructor sorts into registry
    // order, so a reversed selection must equal the forward one.
    let reversed = hygienic_options(&report_argv(&[
        "--templates",
        "field-accessors,default-constructor",
    ]))
    .unwrap();
    assert_eq!(reversed.templates(), &hygienic::Template::REGISTRY[..]);
    let single = hygienic_options(&report_argv(&["--templates", "field-accessors"])).unwrap();
    assert_eq!(
        single.templates(),
        &[hygienic::Template::FieldAccessors][..]
    );
}

#[test]
fn hygienic_rejects_empty_unknown_and_repeated_template_ids() {
    for rejected in ["", "field-accessors,", "Field-Accessors", "accessors"] {
        assert_eq!(
            hygienic_options(&report_argv(&["--templates", rejected])).unwrap_err(),
            2,
            "{rejected}"
        );
    }
    assert_eq!(
        hygienic_options(&report_argv(&[
            "--templates",
            "field-accessors,field-accessors"
        ]))
        .unwrap_err(),
        2
    );
    assert_eq!(
        hygienic_options(&report_argv(&[
            "--templates",
            "field-accessors",
            "--templates",
            "default-constructor",
        ]))
        .unwrap_err(),
        2
    );
    assert_eq!(
        hygienic_options(&report_argv(&["--templates"])).unwrap_err(),
        2
    );
    assert_eq!(hygienic_options(&report_argv(&["--all"])).unwrap_err(), 2);
    assert_eq!(
        hygienic_options(&report_argv(&["--max-bytes", "2047"])).unwrap_err(),
        2
    );
    assert_eq!(
        hygienic_options(&report_argv(&["--max-bytes", "4096"]))
            .unwrap()
            .max_bytes(),
        4096
    );
}

// --- abi-report and c-header selection lists ---------------------------

#[test]
fn abi_report_accumulates_comma_and_repeated_selections_and_caps_at_sixty_four() {
    assert_eq!(
        abi_report_options(&report_argv(&[
            "--function",
            "beta,alpha",
            "--function",
            "gamma"
        ]))
        .unwrap()
        .functions,
        vec!["beta", "alpha", "gamma"]
    );
    assert_eq!(abi_report_options(&report_argv(&[])).unwrap_err(), 2);
    assert_eq!(
        abi_report_options(&report_argv(&["--function", "a,,b"])).unwrap_err(),
        2
    );
    assert_eq!(
        abi_report_options(&report_argv(&["--function", "a", "--function", "a"])).unwrap_err(),
        2
    );

    let names: Vec<String> = (0..64).map(|index| format!("f{index}")).collect();
    let admitted = names.join(",");
    assert_eq!(
        abi_report_options(&report_argv(&["--function", &admitted]))
            .unwrap()
            .functions
            .len(),
        64
    );
    let rejected = format!("{admitted},f64");
    assert_eq!(
        abi_report_options(&report_argv(&["--function", &rejected])).unwrap_err(),
        2
    );
}

#[test]
fn c_header_emit_header_is_a_valueless_flag_that_does_not_consume_the_next_option() {
    let (options, emit_header) = c_header_options(&report_argv(&["--function", "a"])).unwrap();
    assert!(!emit_header);
    assert_eq!(
        options.max_bytes,
        c_header::CHeaderOptions::default().max_bytes
    );

    let (options, emit_header) = c_header_options(&report_argv(&[
        "--emit-header",
        "--function",
        "a",
        "--max-bytes",
        "4096",
    ]))
    .unwrap();
    assert!(emit_header);
    assert_eq!(options.functions, vec!["a".to_owned()]);
    assert_eq!(options.max_bytes, 4096);

    assert_eq!(
        c_header_options(&report_argv(&[
            "--function",
            "a",
            "--emit-header",
            "--emit-header"
        ]))
        .unwrap_err(),
        2
    );
    assert_eq!(
        c_header_options(&report_argv(&[
            "--function",
            "a",
            "--max-bytes",
            "4096",
            "--max-bytes",
            "8192"
        ]))
        .unwrap_err(),
        2
    );
    assert_eq!(
        c_header_options(&report_argv(&["--function", "a", "--emit-source"])).unwrap_err(),
        2
    );
    assert_eq!(
        c_header_options(&report_argv(&["--max-bytes"])).unwrap_err(),
        2
    );
}

// --- the single `--max-bytes` report grammars --------------------------

#[test]
fn every_single_budget_report_grammar_shares_one_closed_option_table() {
    type Parse = fn(&[String]) -> Result<usize, u8>;
    let parsers: [(&str, usize, Parse); 6] = [
        (
            "freestanding-object",
            freestanding_object::FreestandingObjectOptions::default().max_bytes,
            |args| freestanding_object_options(args).map(|options| options.max_bytes),
        ),
        (
            "capability-manifest",
            capability_manifest::CapabilityManifestOptions::default().max_bytes,
            |args| capability_manifest_options(args).map(|options| options.max_bytes),
        ),
        (
            "package-report",
            package_report::PackageReportOptions::default().max_bytes,
            |args| package_report_options(args).map(|options| options.max_bytes),
        ),
        (
            "region-report",
            region_report::RegionReportOptions::default().max_bytes,
            |args| region_report_options(args).map(|options| options.max_bytes),
        ),
        (
            "simd-report",
            simd_report::SimdReportOptions::default().max_bytes,
            |args| simd_report_options(args).map(|options| options.max_bytes),
        ),
        (
            "protocol-check",
            protocol_check::ProtocolCheckOptions::default().max_bytes,
            |args| protocol_check_options(args).map(|options| options.max_bytes),
        ),
    ];
    for (name, default, parse) in parsers {
        assert_eq!(parse(&report_argv(&[])).unwrap(), default, "{name}");
        assert_eq!(
            parse(&report_argv(&["--max-bytes", "4096"])).unwrap(),
            4096,
            "{name}"
        );
        // Duplicate, unknown, missing value, a following flag as the value,
        // an unexpected positional, and both byte-window edges.
        assert_eq!(
            parse(&report_argv(&[
                "--max-bytes",
                "4096",
                "--max-bytes",
                "8192"
            ]))
            .unwrap_err(),
            2,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--verbose"])).unwrap_err(),
            2,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["module.spx"])).unwrap_err(),
            2,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--max-bytes"])).unwrap_err(),
            2,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--max-bytes", "--verbose"])).unwrap_err(),
            2,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--max-bytes", "2047"])).unwrap_err(),
            2,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--max-bytes", "2048"])).unwrap(),
            2048,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--max-bytes", "16777216"])).unwrap(),
            16 * 1024 * 1024,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--max-bytes", "16777217"])).unwrap_err(),
            2,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--max-bytes", "0"])).unwrap_err(),
            2,
            "{name}"
        );
        assert_eq!(
            parse(&report_argv(&["--max-bytes", "184467440737095516160"])).unwrap_err(),
            2,
            "{name}"
        );
    }
}
