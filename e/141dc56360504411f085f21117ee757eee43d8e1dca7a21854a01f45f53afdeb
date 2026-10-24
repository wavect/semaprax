use super::{first, first_owned, first_reusing, WORK};
use crate::ast::{ImportFailure, ImportResult, Program, Type};
use crate::bounded_output::{clear_active_floor, set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use crate::workspace_graph::{build_owned, WorkspaceSource};

const INTERFACE: &str = r#"
@id("host.interface") interface Host permits { } {
 @id("host.echo") import rust fn echo(value:i64)->i64
 effects { } failure status "host.error.v1";
}
"#;
const CALLER: &str = include_str!("../function_caller.spx");
const PROVIDER: &str = include_str!("../function_provider.spx");
fn caller() -> Program {
    crate::parse(&format!("{CALLER}{INTERFACE}"), "caller.spx").unwrap()
}
fn run(program: &Program, mode: usize) -> Result<u64, Vec<crate::diagnostic::Diagnostic>> {
    let mut held = None;
    let mut work = WORK;
    match mode {
        0 => first(program),
        1 => first_reusing(program, &mut held, &mut work),
        2 => first_owned(program, &mut held, &mut work),
        _ => unreachable!(),
    }
}
fn linked(app: &str) -> Result<crate::hir::ResolvedProgram, Vec<crate::diagnostic::Diagnostic>> {
    let build = build_owned(vec![
        WorkspaceSource {
            path: "caller.spx".into(),
            source: app.into(),
        },
        WorkspaceSource {
            path: "provider.spx".into(),
            source: PROVIDER.into(),
        },
    ])?;
    build.linked_owned_data_api_program_with_roots("function_layout.app", &[])
}

#[test]
fn checked_unrelated_interface_keeps_function_only_private_layout_and_ordinary_ffi() {
    let source = format!("{CALLER}{INTERFACE}");
    let program = linked(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    let answer =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&program, "app.main", 100_000).unwrap();
    assert!(matches!(
        answer.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    ));
    for id in ["config", "report", "row"] {
        assert!(program.types.iter().any(|t| t.id.as_str() == id));
    }
    // The interface neither imports these source names nor authorizes calls.
    let illegal = source.replace("let report = make();", "let report = Report { }; ");
    assert!(linked(&illegal).is_err());
    // Exercise the ordinary FFI verifier directly as well: these imports are
    // unused by the linked entry, so pruning cannot stand in for validation.
    let ffi = format!("module ffi.app;{INTERFACE} @id(\"ffi.main\") fn main()->i64 {{0}}");
    let valid = crate::parse(&ffi, "ffi.spx").unwrap();
    crate::hir::validate(&crate::hir::resolve(&valid).unwrap()).unwrap();
    let invalid = crate::parse(&ffi.replace("value:i64", "value:Missing"), "ffi.spx").unwrap();
    assert!(crate::hir::resolve(&invalid).is_err());
    let duplicate = source.replace("@id(\"host.echo\")", "@id(\"app.main\")");
    assert!(linked(&duplicate).is_err());
}

#[test]
fn every_interface_import_identity_surface_is_censused_in_all_three_modes() {
    // Mutations here test identifier enumeration, not source admission: result
    // resource names and selected metadata remain subject to the normal verifier.
    for slot in 0..17 {
        let mut program = caller();
        let name = "_spx_layout_731".to_owned();
        let interface = &mut program.interfaces[0];
        match slot {
            0 => interface.name = name,
            1 => interface.stable_id = name,
            2 => interface.permits.push(name),
            n => {
                let import = &mut interface.imports[0];
                match n {
                    3 => import.name = name,
                    4 => import.stable_id = name,
                    5 => import.selected_signature = Some(name),
                    6 => import.selected_index_digest = Some(name),
                    7 => import.selected_receiver = Some(name),
                    8 => import.rust_path = Some(name),
                    9 => import.params[0].name = name,
                    10 => {
                        import.params[0].ty = Type::Named {
                            name,
                            arguments: vec![],
                        }
                    }
                    11 => import.result = ImportResult::OwnedResource { name },
                    12 => import.result = ImportResult::OwnedResultResourceI64 { name },
                    13 => import.result = ImportResult::BorrowedStr { owner: name },
                    14 => import.effects.push(name),
                    15 => import.failure = ImportFailure::Status { domain_id: name },
                    16 => import.consumes = name,
                    _ => unreachable!(),
                }
            }
        }
        for mode in 0..3 {
            assert_eq!(
                run(&program, mode).unwrap(),
                732,
                "slot {slot}, mode {mode}"
            );
        }
    }
    for result in [
        ImportResult::Unit,
        ImportResult::I64,
        ImportResult::Bool,
        ImportResult::ResultI64I64,
        ImportResult::OwnedString,
        ImportResult::OwnedOptionString,
        ImportResult::OwnedResultStringI64,
        ImportResult::OwnedResultStringOptionI64,
    ] {
        let mut program = caller();
        program.interfaces[0].imports[0].result = result;
        for mode in 0..3 {
            assert_eq!(run(&program, mode).unwrap(), 0);
        }
    }
}

#[test]
fn complete_executable_predecessor_and_plain_module_receipts_remain_exact() {
    let plain = crate::parse(CALLER, "caller.spx").unwrap();
    let with_interface = caller();
    for mode in 0..3 {
        let old = |p: &Program| {
            let mut held = None;
            let mut work = WORK;
            match mode {
                0 => super::super::before_interface_census::first(p),
                1 => super::super::before_interface_census::first_reusing(p, &mut held, &mut work),
                2 => super::super::before_interface_census::first_owned(p, &mut held, &mut work),
                _ => unreachable!(),
            }
        };
        let before = with_limit_usage_refusal(1 << 25, || old(&plain));
        let now = with_limit_usage_refusal(1 << 25, || run(&plain, mode));
        assert_eq!(now, before);
        assert!(old(&with_interface).is_err());
        assert!(run(&with_interface, mode).is_ok());
    }
    assert_eq!(
        before_source(include_str!("../names.rs")),
        include_str!("interface_before.rs")
    );
    let root = include_str!("../../private_layout.rs");
    let suffix = "\n#[cfg(test)]\n#[path = \"private_layout/names/interface_before.rs\"]\nmod before_interface_census;\n";
    assert_eq!(
        root.strip_suffix(suffix).unwrap(),
        include_str!("interface_root_before.txt")
    );
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value =
        serde_json::from_str(include_str!("interface_source_inverse.json")).unwrap();
    assert_eq!(packet["owners"].as_array().unwrap().len(), 4);
    for row in packet["owners"].as_array().unwrap() {
        let current = match row["source"].as_str().unwrap() {
            "src/workspace_graph/expected_projection/private_layout/names.rs" => {
                include_str!("../names.rs")
            }
            "src/workspace_graph/expected_projection/private_layout.rs" => {
                include_str!("../../private_layout.rs")
            }
            "docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md" => {
                include_str!("../../../../../docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md")
            }
            "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs" => {
                include_str!("../registry/scratch_tests.rs")
            }
            _ => panic!("unexpected complete #754 source owner"),
        };
        assert_eq!(
            current.len() as u64,
            row["current_length"].as_u64().unwrap()
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["current_sha256"]
        );
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, row["before_text"].as_str().unwrap());
        assert_eq!(
            restored.len() as u64,
            row["before_length"].as_u64().unwrap()
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
            ),
            row["before_sha256"]
        );
    }
}

#[test]
fn unrelated_unsupported_declarations_keep_the_original_complete_refusal() {
    let original = caller();
    for resource in [false, true] {
        let mut program = original.clone();
        let other = crate::parse(
            if resource {
                "module other; @id(\"r\") resource R { @id(\"r.drop\") drop trivial; }"
            } else {
                "module other; @id(\"p\") protocol P { @id(\"p.read\") fn read(value:i64)->i64; }"
            },
            "other.spx",
        )
        .unwrap();
        if resource {
            program.types.extend(other.types.iter().cloned());
        } else {
            program.protocols.extend(other.protocols.iter().cloned());
        }
        for mode in 0..3 {
            assert_eq!(
                run(&program, mode).unwrap_err(),
                super::super::before_interface_census::first(&program).unwrap_err()
            );
        }
    }
}

#[test]
fn bounded_metadata_type_depth_and_suffix_overflow_never_publish_an_alias() {
    for attack in 0..3 {
        let mut program = caller();
        let import = &mut program.interfaces[0].imports[0];
        match attack {
            0 => import.consumes = "_spx_layout_18446744073709551615".into(),
            1 => import.selected_signature = Some("x".repeat(WORK + 1)),
            2 => {
                let mut ty = Type::I64;
                for _ in 0..514 {
                    ty = Type::Named {
                        name: "Nested".into(),
                        arguments: vec![ty],
                    };
                }
                import.params[0].ty = ty;
            }
            _ => unreachable!(),
        }
        for mode in 0..3 {
            let error = run(&program, mode).unwrap_err();
            assert_eq!(error[0].code, "SPX-G172");
        }
    }
}

#[test]
fn exact_short_zero_nested_floor_sticky_and_full_drop_use_the_actual_new_invoice() {
    let program = caller();
    for mode in 0..3 {
        let (answer, overflow, required, _) =
            with_limit_usage_refusal(1 << 25, || run(&program, mode));
        assert!(answer.is_ok() && !overflow && required > 0);
        for limit in [required, required - 1, 0] {
            let (((), allocation), refused, paid, first) = with_limit_usage_refusal(limit, || {
                measure(|| {
                    let result = run(&program, mode);
                    assert_eq!(result.is_ok(), limit == required);
                    drop(result); // Diagnostic text is included through its real Drop.
                    if limit != required {
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        assert!(run(&program, mode).is_err());
                    }
                })
            });
            assert_eq!(refused, limit != required);
            assert_eq!(first.is_some(), limit != required);
            assert!(paid <= limit);
            assert!(
                !allocation.invalid
                    && allocation.live == 0
                    && allocation.allocated == allocation.freed
            );
        }
        let floor = 37;
        let (answer, refused, _, _) = with_limit_usage_refusal(required + floor, || {
            assert!(set_active_floor(floor));
            let (result, child_refused, child_paid, _) =
                with_limit_usage_refusal(required, || run(&program, mode));
            assert!(!child_refused);
            assert_eq!(child_paid, required);
            assert_eq!(crate::bounded_output::active_remaining(), Some(floor));
            clear_active_floor();
            result
        });
        assert!(answer.is_ok() && !refused);
        let (answer, refused, _, _) = with_limit_usage_refusal(required + floor - 1, || {
            assert!(set_active_floor(floor));
            let result = run(&program, mode);
            clear_active_floor();
            result
        });
        assert!(answer.is_err() && refused);
    }
}

// The old scratch control still inspects the actual complete source, through
// this exact additive inverse, before applying every unchanged old assertion.
pub(crate) fn before_source(current: &str) -> String {
    let mut source = current.to_owned();
    let stage = "    if !caller.interfaces.is_empty() {\n        // Pay the new receiving boundary before forming its borrowed arguments.\n        reserve_builder_structure(interface_census::CALL)?;\n        interface_census::scan(&mut *census, caller)?;\n    }\n";
    assert_eq!(source.matches(stage).count(), 2);
    source = source.replace(stage, "");
    let owned = stage.replace("&mut *census", "&mut census");
    assert_eq!(source.matches(&owned).count(), 1);
    source = source.replacen(&owned, "", 1);
    assert_eq!(source.matches("if !caller.protocols.is_empty()").count(), 3);
    source = source.replace(
        "if !caller.protocols.is_empty()",
        "if !caller.interfaces.is_empty()\n        || !caller.protocols.is_empty()",
    );
    let suffix = "\n#[path = \"names/interface_census.rs\"]\nmod interface_census;\n#[cfg(test)]\n#[path = \"names/interface_tests.rs\"]\nmod interface_tests;\n#[cfg(test)]\npub(super) use interface_tests::before_source as before_interface_source;\n";
    source = source.strip_suffix(suffix).unwrap().to_owned();
    assert_eq!(source, include_str!("interface_before.rs"));
    source
}
