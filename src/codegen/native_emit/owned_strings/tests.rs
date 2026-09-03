use super::*;
use crate::codegen::COutput;

#[test]
fn direct_sink_preserves_bytes_and_exact_budget_charges() {
    let render = |wrapped| {
        crate::bounded_output::with_limit_usage(100, || {
            let mut output = crate::bounded_output::CappedString::new();
            if wrapped {
                let mut direct = FunctionOutput::Direct(&mut output);
                direct.push_str("ordinary body");
                direct.push('\n');
            } else {
                output.push_str("ordinary body");
                output.push('\n');
            }
            output.into_string()
        })
    };
    assert_eq!(render(false), render(true));
}

#[test]
fn exhausted_or_duplicate_cell_identity_is_diagnostic_not_panic() {
    let mut cells = OwnedStrings::default();
    cells.register("spx_internal_0", true).unwrap();
    assert!(cells.register("spx_internal_0", true).is_err());
    let (result, overflowed) = crate::bounded_output::with_limit(0, || {
        let name = crate::bounded_output::budgeted_format(format_args!("spx_internal_{}", 1));
        cells.register(&name, true)
    });
    assert!(overflowed);
    assert!(result.is_err());
}

#[test]
fn inline_owner_cells_cover_ordinary_and_owned_data_provider_strings() {
    let checked = crate::check(
            "module test.inline; @id(\"word\") fn word() -> string { \"value\" } @id(\"main\") fn main() -> i64 { 0 }",
            "inline.spx",
        ).unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let ordinary = crate::codegen::emit_hir_c(&program).unwrap();
    let provider = crate::codegen::emit_hir_c_for_owned_data_provider(&program).unwrap();
    assert!(provider.contains("spx_result_live"));
    assert!(provider.contains("invalid String transfer"));
    assert!(provider.contains("live String overwritten"));
    assert!(ordinary.contains("spx_result_live"));
    assert!(ordinary.contains("invalid String transfer"));
    assert!(!ordinary.contains("strlen(spx_source)"));
    assert!(ordinary.contains("struct spx_string_v10"));
    assert!(!provider.contains("strlen(spx_source)"));
    assert!(provider.contains("struct spx_string_v10"));
    let current = crate::codegen::emit_hir_c_for_owned_utf8_provider(&program).unwrap();
    assert!(current.contains("char *spx_internal_0 = NULL;"));
    assert!(current.contains("bool spx_internal_0_live = false;"));
    assert!(current.contains("invalid String transfer"));
    assert!(current.contains("if (!spx_result_live)"));
    let publish = current
        .find("*spx_result_out = spx_result;\n    spx_result_live = false;")
        .unwrap();
    assert!(publish > current.find("if (!spx_result_live)").unwrap());
}

fn resolved(source: &str) -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "inline-presence.spx").unwrap()).unwrap()
}

#[test]
fn presence_checks_signature_body_and_both_contract_phases() {
    use super::super::NativeOutputProfile as Profile;
    let program = resolved(
        r#"module test.presence;
@id("param") fn parameter(text: string) -> i64 { 1 }
@id("result") fn result_text() -> string { "result" }
@id("body") fn body() -> i64 { let text = "body"; 1 }
@id("requires") fn before() -> i64 requires string_is_empty("") { 1 }
@id("ensures") fn after() -> i64 ensures string_is_empty("") { 1 }
@id("main") fn main() -> i64 { 0 }
"#,
    );
    for function in &program.functions {
        let has_strings = function.id.as_str() != "main";
        assert_eq!(super::super::function_uses_strings(function), has_strings);
        for profile in [
            Profile::Legacy,
            Profile::StdoutTranscript,
            Profile::OwnedDataProvider,
        ] {
            assert_eq!(profile.tracks_strings(function), has_strings);
        }
        // V10 deliberately retains its previous always-on selection,
        // including zero-String functions, while all frozen profiles stay off.
        assert!(Profile::OwnedUtf8Provider.tracks_strings(function));
        for profile in [
            Profile::UsefulDataCommand,
            Profile::LanguageCommandIo,
            Profile::LineCommandIo,
        ] {
            assert!(!profile.tracks_strings(function));
            assert!(!profile.tracks_present_strings());
        }
    }
}

#[test]
fn ordinary_and_provider_discovery_include_instantiated_string_runtime_groups() {
    let program = resolved(
        r#"module test.instance_strings;
@id("measure") fn measure<T>(value: T) -> i64 { string_len_chars("hé") }
@id("main") fn main() -> i64 { measure<i64>(1) }
"#,
    );
    assert_eq!(program.function_instances.len(), 1);
    assert!(!super::super::program_uses_strings(&program, false));
    assert!(!super::super::program_uses_string_ops(&program, false));
    assert!(!super::super::program_uses_string_ops_v2(&program, false));
    assert!(super::super::program_uses_strings(&program, true));
    assert!(super::super::program_uses_string_ops(&program, true));
    assert!(super::super::program_uses_string_ops_v2(&program, true));
    let native = crate::codegen::emit_hir_c(&program).unwrap();
    assert!(native.contains("static __attribute__((unused)) char *spx_string_from_literal("));
    assert!(native.contains("spx_string_len_chars(const char *"));
    assert!(native.contains("live String overwritten"));
    let provider = crate::codegen::emit_hir_c_for_owned_data_provider(&program).unwrap();
    assert!(provider.contains("static __attribute__((unused)) char *spx_string_from_literal("));
    assert!(provider.contains("spx_string_len_chars(const char *"));
    assert!(provider.contains("struct spx_string_v10"));
    assert!(provider.contains("live String overwritten"));
}

#[test]
fn string_free_function_emission_matches_frozen_route_bytes_and_budget() {
    use super::super::NativeOutputProfile as Profile;
    let program = resolved("module test.scalar; @id(\"main\") fn main() -> i64 { 40 + 2 }");
    let emit = |profile| {
        crate::bounded_output::with_limit_usage(1_000_000, || {
            super::super::emit_hir_c_with_labels(
                &program,
                &std::collections::HashMap::new(),
                profile,
                None,
            )
            .unwrap()
        })
    };
    // These profiles shared the exact scalar projection before correction.
    assert_eq!(emit(Profile::Legacy), emit(Profile::OwnedDataProvider));
}

#[test]
fn string_free_owned_bytes_provider_retains_frozen_prelude_and_budget() {
    use super::super::{NativeOutputProfile as Profile, StringRuntimeSelection};
    let program = resolved(
            "module test.provider_bytes; @id(\"payload\") fn payload(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) } @id(\"main\") fn main() -> i64 { 0 }",
        );
    for function in &program.functions {
        assert!(!Profile::OwnedDataProvider.tracks_strings(function));
    }
    let abi = super::super::native_resource::build_resource_abi(&program).unwrap();
    let render = |selection| {
        crate::bounded_output::with_limit_usage(1_000_000, || {
            let mut output = crate::bounded_output::CappedString::new();
            super::super::emit_native_prelude_profile(&mut output, &abi, &program, selection);
            output.into_string()
        })
    };
    // The literal old selector is an independent control, not another
    // profile now routed through the corrected String selector.
    assert_eq!(
        render(Profile::OwnedDataProvider.string_runtime()),
        render(StringRuntimeSelection::FROZEN),
    );
}

#[test]
fn length_aware_runtime_groups_do_not_grant_provider_carriers() {
    use super::super::{NativeOutputProfile as Profile, StringRuntimeSelection};
    let program = resolved(
        r#"module test.length_runtime;
@id("main") fn main() -> i64 { string_len_chars("a\u{0}b") }
"#,
    );
    let abi = super::super::native_resource::build_resource_abi(&program).unwrap();
    let render = |selection| {
        crate::bounded_output::with_limit_usage(1_000_000, || {
            let mut text = crate::bounded_output::CappedString::new();
            super::super::emit_native_prelude_profile(&mut text, &abi, &program, selection);
            text.into_string()
        })
    };
    for profile in [
        Profile::Legacy,
        Profile::StdoutTranscript,
        Profile::OwnedDataProvider,
    ] {
        let (text, overflowed, _) = render(profile.string_runtime());
        assert!(!overflowed);
        assert!(text.contains(super::super::NATIVE_LENGTH_DELIMITED_STRING_RUNTIME_C));
        assert!(text.contains(super::super::NATIVE_LENGTH_DELIMITED_STRING_OPS_RUNTIME_C));
        assert!(text.contains(super::super::NATIVE_LENGTH_DELIMITED_STRING_OPS_V2_RUNTIME_C));
        assert!(!text.contains("borrowed_str_depth"));
        assert!(!text.contains("} spx_bytes_v1;"));
    }
    let v10 = render(Profile::OwnedUtf8Provider.string_runtime());
    // The frozen V10 representation and carrier decisions remain identical.
    assert_eq!(
        v10,
        render(StringRuntimeSelection {
            length_delimited: true,
            provider_carriers: true,
            include_instances: false,
        })
    );
    assert!(v10.0.contains("borrowed_str_depth"));
    assert!(v10.0.contains("} spx_bytes_v1;"));
    for profile in [
        Profile::UsefulDataCommand,
        Profile::LanguageCommandIo,
        Profile::LineCommandIo,
    ] {
        assert_eq!(profile.string_runtime(), StringRuntimeSelection::FROZEN);
    }
    let frozen = render(StringRuntimeSelection::FROZEN);
    assert!(frozen.0.contains(super::super::NATIVE_STRING_RUNTIME_C));
    assert!(frozen.0.contains(super::super::NATIVE_STRING_OPS_RUNTIME_C));
    assert!(frozen
        .0
        .contains(super::super::NATIVE_STRING_OPS_V2_RUNTIME_C));
    let callable = crate::bounded_output::with_limit_usage(1_000_000, || {
        let mut text = crate::bounded_output::CappedString::new();
        super::super::emit_native_prelude(&mut text, &abi, &program);
        text.into_string()
    });
    assert_eq!(frozen, callable);
}
