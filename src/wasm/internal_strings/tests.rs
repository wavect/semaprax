use super::*;
use std::path::Path;

#[path = "tests/nesting.rs"]
mod nesting;
#[path = "tests/work_bounds.rs"]
mod work_bounds;

const SOURCE: &str = r#"
module test.standalone_strings;
@id("s.keep")
fn keep(value: string) -> string { value }
@id("s.flag")
fn flag(value: bool) -> bool { value }
@id("s.main")
fn main() -> i64 {
    let value = keep("a\u{0}λ");
    match string_len(value) {
        4 if string_contains(value, "λ") => 42,
        _ => 0,
    }
}
"#;

fn program(source: &str) -> Program {
    let program = crate::parse(source, Path::new("standalone-strings.spx")).unwrap();
    let diagnostics = crate::verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    program
}

#[test]
fn internal_string_parameters_require_canonical_owned_hir() {
    let resolved = crate::hir::resolve(&program(SOURCE)).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let helper = resolved
        .functions
        .iter()
        .position(|function| function.id.as_str() == "s.keep")
        .unwrap();
    assert_eq!(
        resolved.functions[helper].params[0].ownership,
        crate::hir::OwnershipMode::Own
    );
    let selected = ["s.main".to_owned()];
    assert!(admission::prepare(&resolved, &selected).is_ok());
    for mode in [
        crate::hir::OwnershipMode::Value,
        crate::hir::OwnershipMode::Borrow,
        crate::hir::OwnershipMode::Shared,
    ] {
        let mut malformed = resolved.clone();
        malformed.functions[helper].params[0].ownership = mode;
        // The shared call index validates HIR before profile signature checks;
        // malformed ownership must remain that earlier diagnostic, not be
        // repaired or admitted by this profile.
        assert_eq!(
            admission::prepare(&malformed, &selected)
                .err()
                .unwrap()
                .code,
            "SPX-H006"
        );
    }
}

#[test]
fn standalone_profile_rejects_string_view_without_authenticated_carrier_conversion() {
    let source = program(
        r#"module test.standalone_string_view;
@id("s.main") fn main() -> i64 {
    let owned = "view";
    let borrowed = string_as_str(owned);
    str_len_bytes(borrowed)
}
"#,
    );
    let error = emit_module(
        &source,
        &["s.main".to_owned()],
        InternalStringOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W111");
    assert!(error.message.contains("outside the closed profile"));
}

#[test]
fn canonical_selection_validates_guarded_module_and_exact_closed_inventory() {
    let source = program(SOURCE);
    let canonical = crate::format::canonical(&source);
    let reparsed = program(&canonical);
    assert_eq!(
        crate::graph::revision(&source),
        crate::graph::revision(&reparsed)
    );
    let first = emit_module(
        &source,
        &["s.main".into(), "s.flag".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    let second = emit_module(
        &reparsed,
        &["s.flag".into(), "s.main".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    assert_eq!(first.wasm_bytes(), second.wasm_bytes());
    assert_eq!(first.descriptor(), second.descriptor());
    assert_eq!(first.runtime_source(), second.runtime_source());
    wasmparser::Validator::new()
        .validate_all(first.wasm_bytes())
        .unwrap();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut memories = 0;
    for payload in wasmparser::Parser::new(0).parse_all(first.wasm_bytes()) {
        match payload.unwrap() {
            wasmparser::Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    assert_eq!(import.module, "semaprax.internal-strings.v1");
                    imports.push(import.name.to_owned());
                }
            }
            wasmparser::Payload::ExportSection(section) => {
                for export in section {
                    exports.push(export.unwrap().name.to_owned());
                }
            }
            wasmparser::Payload::MemorySection(section) => {
                for memory in section {
                    let memory = memory.unwrap();
                    assert_eq!(memory.initial, 4);
                    assert_eq!(memory.maximum, Some(4));
                    assert!(!memory.shared && !memory.memory64);
                    memories += 1;
                }
            }
            wasmparser::Payload::StartSection { .. } | wasmparser::Payload::TableSection(_) => {
                panic!("unexpected authority")
            }
            _ => {}
        }
    }
    assert_eq!(memories, 1);
    assert_eq!(
        imports,
        [
            "literal",
            "clone",
            "concat",
            "from_char",
            "byte_len",
            "char_len",
            "eq",
            "starts_with",
            "contains",
            "drop"
        ]
    );
    assert_eq!(
        exports,
        [
            "memory",
            "__spx_stack_pointer",
            "__spx_call_0",
            "__spx_call_1"
        ]
    );
}

#[test]
fn selection_and_options_fail_closed_without_widening_public_strings() {
    let source = program(SOURCE);
    for ids in [
        vec![],
        vec!["s.main".into(), "s.main".into()],
        vec!["s.keep".into()],
        vec!["absent".into()],
    ] {
        assert_eq!(
            emit_module(&source, &ids, InternalStringOptions::default())
                .unwrap_err()
                .code,
            "SPX-W111"
        );
    }
    for options in [
        InternalStringOptions {
            max_string_bytes: 65_537,
            ..Default::default()
        },
        InternalStringOptions {
            max_live_bytes: 16_777_217,
            ..Default::default()
        },
        InternalStringOptions {
            max_cumulative_bytes: 67_108_865,
            ..Default::default()
        },
        InternalStringOptions {
            max_live_owners: Some(0),
            ..Default::default()
        },
        InternalStringOptions {
            max_live_owners: Some(65_537),
            ..Default::default()
        },
    ] {
        assert_eq!(
            emit_module(&source, &["s.main".into()], options)
                .unwrap_err()
                .code,
            "SPX-W111"
        );
    }
    let zero = InternalStringOptions {
        max_string_bytes: 0,
        max_live_bytes: 0,
        max_cumulative_bytes: 0,
        max_live_owners: Some(1),
    };
    assert!(emit_module(&source, &["s.main".into()], zero).is_ok());
    let recursive = program("module test.recursive; @id(\"r.main\") fn main() -> i64 { main() }");
    assert_eq!(
        emit_module(
            &recursive,
            &["r.main".into()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
}

#[test]
fn string_mutation_and_direct_string_loop_storage_keep_source_rejections() {
    for (body, code) in [
        ("let mut value = \"x\"; value = \"y\"; 0", "SPX-U105"),
        (
            "let mut index = 0; while index < 1 { let value = \"x\"; index = index + 1; 0 } 0",
            "SPX-T252",
        ),
    ] {
        let source = format!("module test.rejected; @id(\"r.main\") fn main() -> i64 {{ {body} }}");
        let program = crate::parse(&source, Path::new("rejected.spx")).unwrap();
        assert!(crate::verify::verify(&program)
            .iter()
            .any(|diagnostic| diagnostic.code == code));
        assert!(emit_module(
            &program,
            &["r.main".into()],
            InternalStringOptions::default()
        )
        .is_err());
    }
}

#[test]
fn unrelated_declarations_cannot_change_selected_artifacts_or_planning() {
    let baseline = emit_module(
        &program(SOURCE),
        &["s.main".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    let additions = r#"
@id("unused.record") record Unused { @id("unused.record.value") value: i64, }
@id("unused.record_value") fn record_value() -> Unused { Unused { value: 3 } }
@id("unused.generic") fn generic<T>(value: T) -> T { value }
@id("unused.instance") fn instance() -> i64 { generic<i64>(9) }
@id("unused.recursive") fn recursive() -> i64 { recursive() }
@id("unused.arguments") fn arguments() -> i64 uses { process.args.read } {
    if args_len() == 0usize { 0 } else { 1 }
}
@id("unused.range") fn range(input: borrow Slice<u8>) -> bool {
    let view = byte_range(input, 0usize, byte_len(input));
    byte_len(view) == 0usize
}
@id("unused.host") interface UnusedHost permits {} {
    @id("unused.host.echo")
    import rust fn unused_echo(value: i64) -> unit effects {} failure infallible;
}
"#;
    let augmented = format!(
        "{}\n{additions}",
        SOURCE.replace(
            "module test.standalone_strings;",
            "module test.standalone_strings;\npermit { process.args.read }",
        )
    );
    let augmented = program(&augmented);
    let resolved = crate::hir::resolve(&augmented).unwrap();
    assert!(!resolved.types.is_empty());
    assert!(!resolved.interfaces.is_empty());
    assert!(!resolved.function_templates.is_empty());
    assert!(!resolved.function_instances.is_empty());
    assert!(!resolved.permits.is_empty());
    assert!(resolved.functions.iter().any(
        |function| function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V4
    ));
    let isolated = emit_module(
        &augmented,
        &["s.main".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    assert_eq!(baseline.wasm_bytes(), isolated.wasm_bytes());
    assert_eq!(baseline.descriptor(), isolated.descriptor());
    assert_eq!(baseline.runtime_source(), isolated.runtime_source());
    for forbidden in [
        "unused.record_value",
        "unused.instance",
        "unused.arguments",
        "unused.range",
        "unused.recursive",
    ] {
        assert_eq!(
            emit_module(
                &augmented,
                &[forbidden.into()],
                InternalStringOptions::default()
            )
            .unwrap_err()
            .code,
            "SPX-W111"
        );
    }
}
