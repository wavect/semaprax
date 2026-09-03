//! Phase-A admission: subject contracts, the selected closure, the
//! canonical specification parser, and the exact declared limits.

use super::*;

#[test]
fn project_phase_a_consumes_resolved_hir_and_emits_distinct_subject_contracts() {
    let source = r#"module project.entry;

@id("project.add")
fn add(left: i64, right: i64) -> i64
{
    left + right
}

@id("project.main")
fn main() -> i64
{
    0
}
"#;
    let program = crate::parse(source, Path::new("src/entry.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let manifest = "schema = \"semaprax.project.v1\"\nname = \"project\"\n";
    let subject = ProjectSubject {
        name: "project".to_owned(),
        manifest_bytes: manifest.len(),
        manifest_digest: raw_digest(manifest.as_bytes()),
        manifest_canonical: manifest.to_owned(),
        project_revision: format!("sha256:{}", "1".repeat(64)),
        workspace_revision: format!("sha256:{}", "2".repeat(64)),
        project_graph_digest: format!("sha256:{}", "3".repeat(64)),
        entry_module: "project.entry".to_owned(),
        sources: vec![ProjectSubjectSource {
            path: "src/entry.spx".to_owned(),
            source_graph_schema: "semaprax.graph.v14".to_owned(),
            source_revision: format!("sha256:{}", "4".repeat(64)),
            source_digest: raw_digest(source.as_bytes()),
            bytes: source.len(),
        }],
        exports: vec![ProjectSubjectExport {
            stable_id: "project.add".to_owned(),
            module: "project.entry".to_owned(),
            path: "src/entry.spx".to_owned(),
        }],
        imports: Vec::new(),
        capabilities: Vec::new(),
    };
    let canonical = render_project_subject(&subject);
    let (prepared, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        prepare_project_native_rust_interop_bounded(&resolved, canonical.as_bytes())
    });
    assert!(!overflowed);
    let prepared = prepared.expect("resolved Project Phase A");
    let expected_digest = domain_digest(PROJECT_SUBJECT_DOMAIN, canonical.as_bytes());
    assert_eq!(
        prepared.project_subject_digest(),
        Some(expected_digest.as_str())
    );
    assert_eq!(prepared.source_revision(), None);
    assert!(prepared.descriptor().contains(PROJECT_DESCRIPTOR_SCHEMA));
    assert!(prepared
        .descriptor()
        .contains("\"project_subject_digest\":"));
    assert!(!prepared.descriptor().contains("\"source_revision\":"));
    assert_eq!(prepared.closure(), &["project.add"]);

    drop(prepared);
    let mut lower = 0usize;
    let mut upper = MAX_BUILDER_BYTES;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let (result, overflowed) = crate::bounded_output::with_limit(middle, || {
            prepare_project_native_rust_interop_bounded(&resolved, canonical.as_bytes())
        });
        if result.is_ok() && !overflowed {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    let exact_limit = lower;
    let (exact, overflowed) = crate::bounded_output::with_limit(exact_limit, || {
        prepare_project_native_rust_interop_bounded(&resolved, canonical.as_bytes())
    });
    assert!(exact.is_ok());
    assert!(!overflowed);
    let (minus_one, overflowed) = crate::bounded_output::with_limit(exact_limit - 1, || {
        prepare_project_native_rust_interop_bounded(&resolved, canonical.as_bytes())
    });
    assert!(minus_one.is_err() || overflowed);
}

#[test]
fn language_command_io_is_outside_the_public_native_rust_sdk_closure() {
    let source = r#"module interop.command;

permit { process.args.read }

@id("interop.command.selected")
fn selected() -> i64 uses { process.args.read }
{
    if args_len() == 0usize { 0 } else { 1 }
}

@id("interop.command.main")
fn main() -> i64
{
    0
}
"#;
    let program = crate::parse(source, Path::new("native-rust-command-io.spx")).unwrap();
    let diagnostics = semaprax::verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let resolved = hir::resolve(&program).unwrap();
    let selected = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "interop.command.selected")
        .unwrap();
    let error = validate_selected_scalar_closure(&[selected]).unwrap_err();
    assert_eq!(error.code, "SPX-B107");
    assert_eq!(
        error.message,
        "Native Rust Interop declaration set is unsupported: scalar value signature required"
    );
}

#[test]
fn native_unit_import_is_exact_direct_unused_let_and_resolved_identity_scoped() {
    const UNIT_SOURCE: &str = r#"module interop.unit;

@id("host.unit")
interface HostUnit
    permits {  }
{
    @id("host.unit.ping")
    import rust fn ping(value: i64) -> unit
        effects {  }
        failure infallible;
}

@id("interop.unit.selected")
fn selected(value: i64) -> i64
{
    let acknowledged = ping(value);
    let outcome = value + 1;
    outcome
}

@id("interop.unit.unselected")
fn unselected(value: i64) -> i64
{
    let acknowledged = ping(value);
    let outcome = 7;
    outcome
}

@id("interop.unit.main")
fn main() -> i64
{
    0
}
"#;
    let prepared = prepare_source(UNIT_SOURCE, &["interop.unit.selected"], &["host.unit.ping"])
        .unwrap_or_else(|errors| panic!("unit prepare: {errors:?}"));
    assert_eq!(prepared.exports.len(), 1);
    assert_eq!(prepared.imports.len(), 1);
    assert!(prepared.imports[0].result == ScalarType::Unit);

    for hostile in [
            UNIT_SOURCE.replacen(
                "    let outcome = value + 1;\n    outcome",
                "    let outcome = value + 1;\n    acknowledged",
                1,
            ),
            UNIT_SOURCE.replacen(
                "    let outcome = value + 1;\n    outcome",
                "    let outcome = selected(acknowledged);\n    outcome",
                1,
            ),
            UNIT_SOURCE.replacen(
                "    let acknowledged = ping(value);",
                "    let acknowledged = { ping(value) };",
                1,
            ),
            UNIT_SOURCE.replacen(
                "    let acknowledged = ping(value);\n    let outcome = value + 1;",
                "    let acknowledged = 0;\n    let outcome = if ping(value) { 1 } else { 2 };",
                1,
            ),
            UNIT_SOURCE.replacen(
                "    let acknowledged = ping(value);\n    let outcome = value + 1;",
                "    let acknowledged = 0;\n    let outcome = if true { ping(value) } else { ping(value) };",
                1,
            ),
            UNIT_SOURCE
                .replacen(
                    "@id(\"interop.unit.selected\")",
                    "@id(\"interop.unit.helper\")\nfn helper(value: i64) -> unit\n{\n    ping(value)\n}\n\n@id(\"interop.unit.selected\")",
                    1,
                )
                .replacen(
                    "    let acknowledged = ping(value);",
                    "    let acknowledged = helper(value);",
                    1,
                ),
        ] {
            let errors =
                match prepare_source(&hostile, &["interop.unit.selected"], &["host.unit.ping"]) {
                    Ok(_) => panic!("hostile Unit use was accepted"),
                    Err(errors) => errors,
                };
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!(errors[0].code, "SPX-B107");
            assert_eq!(
                errors[0].message,
                "Native Rust Interop declaration set is unsupported: scalar value signature required"
            );
        }

    let unit_export = UNIT_SOURCE.replacen(
            "fn selected(value: i64) -> i64\n{\n    let acknowledged = ping(value);\n    let outcome = value + 1;\n    outcome",
            "fn selected(value: i64) -> unit\n{\n    let acknowledged = ping(value);\n    acknowledged",
            1,
        );
    let errors = match prepare_source(
        &unit_export,
        &["interop.unit.selected"],
        &["host.unit.ping"],
    ) {
        Ok(_) => panic!("Unit export was accepted"),
        Err(errors) => errors,
    };
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "SPX-B107");
    assert_eq!(
        errors[0].message,
        "Native Rust Interop declaration set is unsupported: scalar value signature required"
    );
}

#[test]
fn multi_export_contract_binds_global_capabilities_and_import_table_exactly() {
    assert_eq!(
        replay_symbol_hash("interop.add"),
        "ee967df46a76c68f1e8650d38ddb6886c897b34a82c4ea48ed3f70788e911326"
    );
    assert_eq!(
        replay_capabilities_digest(&["host.math".to_owned()]),
        "sha256:d510605f56f47934126eeac931a6b363d7da36f492af90bb36ff573b00fb7d84"
    );
    let source = r#"module interop.disjoint;

permit { cap.a, cap.b }

@id("host.a")
interface HostA permits { cap.a } {
    @id("host.a.call")
    import rust fn call_a(value: i64) -> i64
        effects { cap.a }
        failure infallible;
}

@id("host.b")
interface HostB permits { cap.b } {
    @id("host.b.call")
    import rust fn call_b(value: i64) -> i64
        effects { cap.b }
        failure infallible;
}

@id("export.a")
fn export_a(value: i64) -> i64 uses { cap.a } { call_a(value) }

@id("export.b")
fn export_b(value: i64) -> i64 uses { cap.b } { call_b(value) }

@id("interop.disjoint.main")
fn main() -> i64 { 0 }
"#;
    let program = crate::parse(source, Path::new("native-rust-disjoint.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical.as_bytes())),
        target: current_target().unwrap(),
        exports: vec!["export.a".to_owned(), "export.b".to_owned()],
        imports: vec!["host.a.call".to_owned(), "host.b.call".to_owned()],
        capabilities: vec!["cap.a".to_owned(), "cap.b".to_owned()],
    };
    let prepared = prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()).unwrap();
    for export in &prepared.exports {
        assert_eq!(export.capabilities, spec.capabilities);
        assert_eq!(export.required_imports, spec.imports);
    }
    assert_eq!(
        prepared
            .descriptor
            .matches("\"required_imports\":[\"host.a.call\",\"host.b.call\"]")
            .count(),
        2
    );
    assert!(prepared
        .generated_rust
        .contains("const EXPECTED_CAPABILITIES:&[&str]=&[\"cap.a\",\"cap.b\"]"));
    assert!(prepared.generated_c.contains("spxnr_validate_import_"));

    let first = call_digest(
        "export",
        "delimiter.test",
        &[],
        ScalarType::I64,
        &[],
        &[],
        &["a,b".to_owned(), "c".to_owned()],
        &[("a,b".to_owned(), "sha256:first".to_owned())],
        "status",
        0,
        &spec.target,
    )
    .unwrap();
    let second = call_digest(
        "export",
        "delimiter.test",
        &[],
        ScalarType::I64,
        &[],
        &[],
        &["a".to_owned(), "b,c".to_owned()],
        &[("a".to_owned(), "sha256:first".to_owned())],
        "status",
        0,
        &spec.target,
    )
    .unwrap();
    assert_ne!(first, second);

    let import_i64 = call_digest(
        "import",
        "same.id",
        &[ParameterFact {
            name: "value".to_owned(),
            ty: ScalarType::I64,
        }],
        ScalarType::I64,
        &[],
        &[],
        &[],
        &[],
        "infallible",
        0,
        &spec.target,
    )
    .unwrap();
    let import_bool = call_digest(
        "import",
        "same.id",
        &[ParameterFact {
            name: "value".to_owned(),
            ty: ScalarType::Bool,
        }],
        ScalarType::I64,
        &[],
        &[],
        &[],
        &[],
        "infallible",
        0,
        &spec.target,
    )
    .unwrap();
    assert_ne!(import_i64, import_bool);
    let export_for_i64 = call_digest(
        "export",
        "export.same",
        &[],
        ScalarType::I64,
        &[],
        &[],
        &["same.id".to_owned()],
        &[("same.id".to_owned(), import_i64)],
        "status",
        0,
        &spec.target,
    )
    .unwrap();
    let export_for_bool = call_digest(
        "export",
        "export.same",
        &[],
        ScalarType::I64,
        &[],
        &[],
        &["same.id".to_owned()],
        &[("same.id".to_owned(), import_bool)],
        "status",
        0,
        &spec.target,
    )
    .unwrap();
    assert_ne!(export_for_i64, export_for_bool);
}

#[test]
fn private_a_is_canonical_and_pure() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    assert_eq!(prepared.canonical_spec, spec);
    assert!(prepared.descriptor.ends_with('\n'));
    assert!(prepared.generated_c.contains("spxnr1_i_"));
    assert!(prepared.generated_header.contains("spxnr_context_v1"));
    assert!(prepared
        .generated_rust
        .starts_with("mod api{#![forbid(unsafe_code)]"));
    assert!(prepared
        .private_ffi_source
        .starts_with("#![allow(unsafe_code)]"));
}

#[test]
fn noncanonical_spec_is_b106_before_target_admission() {
    let (program, spec) = fixture();
    let noncanonical = spec.replacen("{\"schema\"", "{ \"schema\"", 1);
    let error = match prepare_native_rust_interop(&program, noncanonical.as_bytes()) {
        Ok(_) => panic!("noncanonical spec was accepted"),
        Err(error) => error,
    };
    assert_eq!(error[0].code, "SPX-B106");
}

#[test]
fn specification_parser_is_canonical_bounded_and_intent_bound() {
    fn assert_spec_error(program: &Program, source: &[u8], code: &str, message: &str) {
        let error = match parse_spec(program, source) {
            Ok(_) => panic!("hostile specification was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.code, code);
        assert_eq!(error.message, message);
    }

    let (program, canonical) = fixture();
    parse_spec(&program, canonical.as_bytes()).unwrap();
    let b106_message = "Native Rust Interop specification is not canonical semaprax.native-rust-interop-spec.v1 JSON";
    let schema_prefix = format!("\"schema\":{},", quote_json(SPEC_SCHEMA));
    let malformed = [
        format!(" \n{canonical}"),
        canonical.trim_end().to_owned(),
        canonical.replace('\n', "\r\n"),
        format!("\u{feff}{canonical}"),
        canonical.replacen(&schema_prefix, "", 1),
        canonical.replacen(
            &schema_prefix,
            &format!("{schema_prefix}{schema_prefix}"),
            1,
        ),
        canonical.replacen(&schema_prefix, &format!("{schema_prefix}\"extra\":0,"), 1),
        canonical.replacen(
            &format!("\"schema\":{}", quote_json(SPEC_SCHEMA)),
            "\"schema\":1",
            1,
        ),
        canonical.replacen(
            "\"exports\":[\"interop.add\"]",
            "\"exports\":[\"interop.add\",\"interop.add\"]",
            1,
        ),
        canonical.replacen("\"max_exports\":32", "\"max_exports\":31", 1),
        canonical.replacen(
            "no_resource_owned_borrow_shared_or_aggregate_abi",
            "xo_resource_owned_borrow_shared_or_aggregate_abi",
            1,
        ),
    ];
    for mutation in malformed {
        assert_spec_error(&program, mutation.as_bytes(), "SPX-B106", b106_message);
    }

    let exact_depth = format!("[[[[[[{canonical}]]]]]]");
    assert_eq!(json_depth(exact_depth.as_bytes()).unwrap(), MAX_JSON_DEPTH);
    assert_spec_error(&program, exact_depth.as_bytes(), "SPX-B106", b106_message);
    let over_depth = format!("[{exact_depth}]");
    assert_spec_error(
        &program,
        over_depth.as_bytes(),
        "SPX-B109",
        "Native Rust Interop max_json_depth exceeds 8",
    );

    let exact_cap = vec![b' '; MAX_SPEC_BYTES];
    assert_spec_error(&program, &exact_cap, "SPX-B106", b106_message);
    let over_cap = vec![b' '; MAX_SPEC_BYTES + 1];
    assert_spec_error(
        &program,
        &over_cap,
        "SPX-B109",
        "Native Rust Interop max_spec_bytes exceeds 1048576",
    );

    let mut exact_source_program = program.clone();
    exact_source_program.functions[0].name.clear();
    let source_overhead = crate::format::canonical(&exact_source_program).len();
    exact_source_program.functions[0].name = "a".repeat(MAX_SOURCE_BYTES - source_overhead);
    let exact_source = crate::format::canonical(&exact_source_program);
    assert_eq!(exact_source.len(), MAX_SOURCE_BYTES);
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (scratch_error, overflowed, consumed) = crate::bounded_output::with_limit_usage(
        canonical_format_scratch_capacity(&exact_source_program)
            .unwrap()
            .bytes()
            - 1,
        || canonical_source_bounded(&exact_source_program),
    );
    let scratch_error = scratch_error.unwrap_err();
    assert!(!overflowed);
    assert_eq!(consumed, 0, "rejected scratch reservation leaked budget");
    assert_eq!(scratch_error.code, "SPX-B109");
    assert_eq!(
        scratch_error.message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 0));
    let exact_peak = canonical_format_scratch_capacity(&exact_source_program)
        .unwrap()
        .bytes()
        .checked_add(MAX_SOURCE_BYTES)
        .unwrap();
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (bounded_source, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak, || {
            canonical_source_bounded(&exact_source_program)
        });
    let bounded_source = bounded_source.unwrap();
    assert!(!overflowed);
    assert_eq!(consumed, MAX_SOURCE_BYTES);
    assert_eq!(bounded_source.len(), MAX_SOURCE_BYTES);
    assert_eq!(bounded_source.capacity(), MAX_SOURCE_BYTES);
    assert_eq!(bounded_source, exact_source);
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 2));
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (peak_error, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak - 1, || {
            canonical_source_bounded(&exact_source_program)
        });
    let peak_error = peak_error.unwrap_err();
    assert!(!overflowed);
    assert_eq!(consumed, 0, "failed materialization leaked scratch budget");
    assert_eq!(peak_error.code, "SPX-B109");
    assert_eq!(
        peak_error.message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 1));
    let mut exact_source_spec = parse_spec(&program, canonical.as_bytes()).unwrap();
    exact_source_spec.source_revision = Some(domain_digest(SOURCE_DOMAIN, exact_source.as_bytes()));
    parse_spec(
        &exact_source_program,
        render_spec(&exact_source_spec).as_bytes(),
    )
    .unwrap();

    let mut over_program = exact_source_program;
    over_program.functions[0].name.push('a');
    let over_source = crate::format::canonical(&over_program);
    assert_eq!(over_source.len(), MAX_SOURCE_BYTES + 1);
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (bounded_source, overflowed, consumed) = crate::bounded_output::with_limit_usage(
        canonical_format_scratch_capacity(&over_program)
            .unwrap()
            .bytes(),
        || canonical_source_bounded(&over_program),
    );
    let bounded_source = bounded_source.unwrap_err();
    assert!(!overflowed);
    assert_eq!(consumed, 0, "over-limit counting pass allocated output");
    assert_eq!(bounded_source.code, "SPX-B109");
    assert_eq!(
        bounded_source.message,
        "Native Rust Interop max_source_bytes exceeds 16777216"
    );
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 1));
    assert_eq!(
        crate::format::canonical(&over_program),
        over_source,
        "bounded formatting mutated the source program"
    );
    let mut over_source_spec = exact_source_spec;
    over_source_spec.source_revision = Some(domain_digest(SOURCE_DOMAIN, over_source.as_bytes()));
    let error = match parse_spec(&over_program, render_spec(&over_source_spec).as_bytes()) {
        Ok(_) => panic!("over-limit source was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_source_bytes exceeds 16777216"
    );

    let spec = parse_spec(&program, canonical.as_bytes()).unwrap();
    for mutation in [
        {
            let mut value = spec.clone();
            value.module = "forged.module".to_owned();
            value
        },
        {
            let mut value = spec.clone();
            value.source_revision = Some("sha256:forged-source".to_owned());
            value
        },
    ] {
        assert_spec_error(
            &program,
            render_spec(&mutation).as_bytes(),
            "SPX-B107",
            "Native Rust Interop declaration set is unsupported: selected identity missing",
        );
    }
    let mut wrong_target = spec.clone();
    wrong_target.target.triple = "forged-unknown-target".to_owned();
    assert_spec_error(
        &program,
        render_spec(&wrong_target).as_bytes(),
        "SPX-B107",
        "Native Rust Interop declaration set is unsupported: target profile mismatch",
    );
    let mut wrong_capability = spec.clone();
    wrong_capability.capabilities = vec!["forged.capability".to_owned()];
    let error =
        match prepare_native_rust_interop(&program, render_spec(&wrong_capability).as_bytes()) {
            Ok(_) => panic!("forged capability was accepted"),
            Err(error) => error,
        };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B107");
    assert_eq!(
        error[0].message,
        "Native Rust Interop declaration set is unsupported: effect or capability mismatch"
    );

    let automatic_source = SOURCE.replacen("@id(\"interop.add\")\n", "", 1);
    let automatic_program = crate::parse(
        &automatic_source,
        Path::new("native-rust-interop-automatic-export.spx"),
    )
    .unwrap();
    let automatic_id = automatic_program
        .functions
        .iter()
        .find(|function| function.name == "add")
        .unwrap()
        .stable_id
        .clone();
    let mut automatic_spec = spec;
    automatic_spec.source_revision = Some(domain_digest(
        SOURCE_DOMAIN,
        crate::format::canonical(&automatic_program).as_bytes(),
    ));
    automatic_spec.exports = vec![automatic_id];
    let error = match prepare_native_rust_interop(
        &automatic_program,
        render_spec(&automatic_spec).as_bytes(),
    ) {
        Ok(_) => panic!("automatic export identity was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B107");
    assert_eq!(
        error[0].message,
        "Native Rust Interop declaration set is unsupported: explicit persistent ID required"
    );
}

#[test]
fn specification_shape_rejects_flat_container_and_scalar_explosions_before_decode() {
    let (program, _) = fixture();
    for element in ["[]", "0", "\"\""] {
        let mut hostile = String::with_capacity(MAX_SPEC_BYTES);
        hostile.push('[');
        let mut first = true;
        while hostile
            .len()
            .checked_add(usize::from(!first))
            .and_then(|length| length.checked_add(element.len()))
            .is_some_and(|length| length < MAX_SPEC_BYTES)
        {
            if !first {
                hostile.push(',');
            }
            hostile.push_str(element);
            first = false;
        }
        while hostile.len() + 1 < MAX_SPEC_BYTES {
            hostile.push(' ');
        }
        hostile.push(']');
        assert_eq!(hostile.len(), MAX_SPEC_BYTES);
        let error = match parse_spec(&program, hostile.as_bytes()) {
            Ok(_) => panic!("hostile generic JSON shape was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.code, "SPX-B106");
        assert_eq!(
            error.message,
            "Native Rust Interop specification is not canonical semaprax.native-rust-interop-spec.v1 JSON"
        );
    }
}

#[test]
fn export_import_and_parameter_count_limits_are_exact() {
    let mut source = String::from(
        "module interop.limit;\n\n@id(\"host.limit\")\ninterface HostLimit\n    permits {  }\n{\n",
    );
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let arguments = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    for index in 0..MAX_IMPORTS {
        write!(
                source,
                "    @id(\"host.{index:02}\")\n    import rust fn import_{index:02}({parameters}) -> i64\n        effects {{  }}\n        failure infallible;\n"
            )
            .unwrap();
    }
    source.push_str("}\n\n");
    for index in 0..MAX_EXPORTS {
        write!(
                source,
                "@id(\"export.{index:02}\")\nfn export_{index:02}({parameters}) -> i64\n{{\n    import_{index:02}({arguments})\n}}\n\n"
            )
            .unwrap();
    }
    source.push_str("@id(\"interop.limit.main\")\nfn main() -> i64\n{\n    0\n}\n");
    let program = crate::parse(&source, Path::new("native-rust-limits.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes())),
        target: current_target().unwrap(),
        exports: (0..MAX_EXPORTS)
            .map(|index| format!("export.{index:02}"))
            .collect(),
        imports: (0..MAX_IMPORTS)
            .map(|index| format!("host.{index:02}"))
            .collect(),
        capabilities: Vec::new(),
    };
    let prepared = prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()).unwrap();
    assert_eq!(prepared.exports.len(), MAX_EXPORTS);
    assert_eq!(prepared.imports.len(), MAX_IMPORTS);
    assert_eq!(prepared.closure.len(), MAX_EXPORTS);
    assert!(prepared
        .exports
        .iter()
        .all(|export| export.parameters.len() == MAX_PARAMETERS));
    assert!(prepared
        .imports
        .iter()
        .all(|import| import.parameters.len() == MAX_PARAMETERS));

    let mut over_exports = spec.clone();
    over_exports.exports.push("export.over".to_owned());
    let error = match parse_spec(&program, render_spec(&over_exports).as_bytes()) {
        Ok(_) => panic!("over-limit export set was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(error.message, "Native Rust Interop max_exports exceeds 32");

    let mut over_imports = spec;
    over_imports.imports.push("host.over".to_owned());
    let error = match parse_spec(&program, render_spec(&over_imports).as_bytes()) {
        Ok(_) => panic!("over-limit import set was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(error.message, "Native Rust Interop max_imports exceeds 32");
}

#[test]
fn closure_effect_and_identifier_limits_are_exact() {
    let effects = (0..MAX_EFFECTS)
        .map(|index| {
            let first = char::from(b'a' + u8::try_from(index / 26).unwrap());
            let second = char::from(b'a' + u8::try_from(index % 26).unwrap());
            format!("effect.e{first}{second}")
        })
        .collect::<Vec<_>>();
    let effect_list = effects.join(", ");
    let source = format!(
        "module interop.effects;\n\npermit {{ {effect_list} }}\n\n@id(\"host.effects\")\ninterface HostEffects\n    permits {{ {effect_list} }}\n{{\n    @id(\"host.effects.call\")\n    import rust fn host_call(value: i64) -> i64\n        effects {{ {effect_list} }}\n        failure infallible;\n}}\n\n@id(\"export.effects\")\nfn export_effects(value: i64) -> i64\n    uses {{ {effect_list} }}\n{{\n    host_call(value)\n}}\n\n@id(\"interop.effects.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
    );
    let program = crate::parse(&source, Path::new("native-rust-effects.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes())),
        target: current_target().unwrap(),
        exports: vec!["export.effects".to_owned()],
        imports: vec!["host.effects.call".to_owned()],
        capabilities: effects,
    };
    let prepared = prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()).unwrap();
    assert_eq!(prepared.exports[0].effects.len(), MAX_EFFECTS);
    assert_eq!(prepared.imports[0].effects.len(), MAX_EFFECTS);

    let mut over_effects = spec.clone();
    over_effects.capabilities.push("effect.over".to_owned());
    let error = match parse_spec(&program, render_spec(&over_effects).as_bytes()) {
        Ok(_) => panic!("over-limit capability set was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(error.message, "Native Rust Interop max_effects exceeds 64");

    for (length, code, message) in [
        (
            MAX_IDENTIFIER_BYTES,
            "SPX-B107",
            "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
        ),
        (
            MAX_IDENTIFIER_BYTES + 1,
            "SPX-B109",
            "Native Rust Interop max_identifier_bytes exceeds 128",
        ),
    ] {
        let mut identifier_spec = spec.clone();
        identifier_spec.capabilities = vec!["a".repeat(length)];
        let error =
            match prepare_native_rust_interop(&program, render_spec(&identifier_spec).as_bytes()) {
                Ok(_) => panic!("hostile identifier was accepted"),
                Err(error) => error,
            };
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, code);
        assert_eq!(error[0].message, message);
    }

    fn closure_fixture(count: usize) -> (Program, Spec) {
        let mut source = String::from(
            "module interop.closure;\n\n@id(\"host.closure\")\ninterface HostClosure\n    permits {  }\n{\n    @id(\"host.closure.leaf\")\n    import rust fn host_leaf(value: i64) -> i64\n        effects {  }\n        failure infallible;\n}\n\n",
        );
        for index in 0..count {
            let body = if index + 1 == count {
                "host_leaf(value)".to_owned()
            } else {
                format!("closure_{:03}(value)", index + 1)
            };
            write!(
                    source,
                    "@id(\"closure.{index:03}\")\nfn closure_{index:03}(value: i64) -> i64\n{{\n    {body}\n}}\n\n"
                )
                .unwrap();
        }
        source.push_str("@id(\"interop.closure.main\")\nfn main() -> i64\n{\n    0\n}\n");
        let program = crate::parse(&source, Path::new("native-rust-closure.spx")).unwrap();
        let canonical = crate::format::canonical(&program);
        let spec = Spec {
            module: program.module.clone(),
            source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical.as_bytes())),
            target: current_target().unwrap(),
            exports: vec!["closure.000".to_owned()],
            imports: vec!["host.closure.leaf".to_owned()],
            capabilities: Vec::new(),
        };
        (program, spec)
    }

    let (program, spec) = closure_fixture(MAX_CALL_DEPTH);
    let canonical_source = crate::format::canonical(&program);
    let mut hir_scan_stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let closure_phase =
        hir_pre_resolve_capacity(&program, canonical_source.len(), &mut hir_scan_stack)
            .unwrap()
            .phase_peaks()[6];
    let terms = hir_capacity_terms_for_test(&program, canonical_source.len()).unwrap();
    assert_eq!(terms.2, 0, "scalar closure has no retained cleanup payload");
    reset_closure_capacity_high_water();
    let prepared = prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()).unwrap();
    assert_eq!(prepared.closure.len(), MAX_CALL_DEPTH);
    let observed_closure_peak = closure_capacity_high_water();
    assert!(observed_closure_peak <= closure_phase);
    assert_eq!(observed_closure_peak, 8_220);
    let (program, spec) = closure_fixture(MAX_CALL_DEPTH + 1);
    let error = match prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()) {
        Ok(_) => panic!("over-limit call depth was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(
        error[0].message,
        "Native Rust Interop max_call_depth exceeds 32"
    );

    let cycle_source = "module interop.closure_cycle; @id(\"cycle.a\") fn a(value: i64) -> i64 { b(value) } @id(\"cycle.b\") fn b(value: i64) -> i64 { a(value) } @id(\"app.main\") fn main() -> i64 { 0 }";
    let cycle_program =
        crate::parse(cycle_source, Path::new("native-rust-closure-cycle.spx")).unwrap();
    let cycle_spec = Spec {
        module: cycle_program.module.clone(),
        source_revision: Some(domain_digest(
            SOURCE_DOMAIN,
            crate::format::canonical(&cycle_program).as_bytes(),
        )),
        target: current_target().unwrap(),
        exports: vec!["cycle.a".to_owned()],
        imports: Vec::new(),
        capabilities: Vec::new(),
    };
    let error =
        match prepare_native_rust_interop(&cycle_program, render_spec(&cycle_spec).as_bytes()) {
            Ok(_) => panic!("cyclic closure was accepted"),
            Err(error) => error,
        };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B107");
    assert_eq!(
        error[0].message,
        "Native Rust Interop declaration set is unsupported: selected closure is cyclic"
    );
}

#[test]
fn project_subject_carries_a_real_sorted_import_and_capability_selection() {
    let manifest = "schema = \"semaprax.project.v1\"\nname = \"project\"\n";
    let subject = ProjectSubject {
        name: "project".to_owned(),
        manifest_bytes: manifest.len(),
        manifest_digest: raw_digest(manifest.as_bytes()),
        manifest_canonical: manifest.to_owned(),
        project_revision: format!("sha256:{}", "1".repeat(64)),
        workspace_revision: format!("sha256:{}", "2".repeat(64)),
        project_graph_digest: format!("sha256:{}", "3".repeat(64)),
        entry_module: "project.entry".to_owned(),
        sources: vec![ProjectSubjectSource {
            path: "src/entry.spx".to_owned(),
            source_graph_schema: "semaprax.graph.v25".to_owned(),
            source_revision: format!("sha256:{}", "4".repeat(64)),
            source_digest: format!("sha256:{}", "5".repeat(64)),
            bytes: 42,
        }],
        exports: vec![ProjectSubjectExport {
            stable_id: "project.add".to_owned(),
            module: "project.entry".to_owned(),
            path: "src/entry.spx".to_owned(),
        }],
        imports: vec!["host.add".to_owned(), "host.sub".to_owned()],
        capabilities: vec!["host.math".to_owned()],
    };
    let canonical = render_project_subject(&subject);
    assert!(canonical
        .ends_with("\"imports\":[\"host.add\",\"host.sub\"],\"capabilities\":[\"host.math\"]}\n"));
    let (parsed, _budget) = parse_project_subject(canonical.as_bytes()).unwrap();
    assert_eq!(parsed.imports, subject.imports);
    assert_eq!(parsed.capabilities, subject.capabilities);
    assert_eq!(render_project_subject(&parsed), canonical);
    let unsorted = canonical.replacen(
        "[\"host.add\",\"host.sub\"]",
        "[\"host.sub\",\"host.add\"]",
        1,
    );
    assert!(parse_project_subject(unsorted.as_bytes()).is_err());
}
