use super::*;
const SOURCE: &str = r#"module checked.native;
@id("checked.native.text") fn text() -> string { string_concat("a", "β") }
@id("checked.native.main") fn main() -> i64 {
    let text = text();
    string_len(text)
}
"#;
#[test]
fn exact_source_native_receiver_uses_checked_helpers_and_closed_default_routes() {
    let source = crate::parse(SOURCE, "checked-native.spx").unwrap();
    let owner = crate::cleanup_plan::checked_allocation::prepare(
        &source,
        &DeclarationId::new("checked.native.main"),
    )
    .unwrap()
    .receive()
    .unwrap();
    let projection = Projection::receive(&owner).unwrap();
    let first = projection.emit().unwrap();
    assert_eq!(first, projection.emit().unwrap());
    assert!(first.contains("spx_checked_allocation_invoke_v1("));
    assert!(first.contains("release_state = ctx->allocator_state"));
    assert!(first.contains("header->release(header->release_state, header)"));
    assert!(first.contains("spx_checked_native_concat_v1(spx_ctx,"));
    assert!(first.contains("checked concat commit epochs"));
    assert!(!first.contains("int main(void)"));
    assert!(super::super::emit_hir_c_with_labels(
        owner.program(),
        &HashMap::new(),
        NativeOutputProfile::Legacy,
        None
    )
    .is_err());
    assert!(super::super::emit_hir_c_with_labels(
        owner.program(),
        &HashMap::new(),
        NativeOutputProfile::CheckedAllocation,
        None
    )
    .is_err());
}

#[test]
fn generated_native_null_sweep_settles_committed_inputs_and_reenters_at_o0_o2() {
    run_native(SOURCE, 4, 3, false);
}

#[test]
fn generated_native_string_bytes_refusals_keep_empty_owners_and_utf8_precedence() {
    const BYTES: &str = r#"module checked.native;
@id("checked.native.main") fn main() -> i64 {
    let raw = bytes_zeroed(2usize);
    let view = bytes_as_slice(raw);
    let text = string_from_utf8(view);
    string_len(text)
}
"#;
    const EMPTY: &str = r#"module checked.native;
@id("checked.native.main") fn main() -> i64 {
    let raw = bytes_zeroed(0usize);
    let view = bytes_as_slice(raw);
    let text = string_from_utf8(view);
    string_len(text)
}
"#;
    const INVALID: &str = r#"module checked.native;
@id("checked.native.main") fn main() -> i64 {
    let raw = bytes_set(bytes_zeroed(1usize), 0usize, 255u8);
    let view = bytes_as_slice(raw);
    let text = string_from_utf8(view);
    string_len(text)
}
"#;
    run_native(BYTES, 3, 2, false);
    run_native(EMPTY, 2, 0, false);
    run_native(INVALID, 1, 0, true);
}

#[test]
fn generated_native_format_null_sweep_settles_all_committed_fields_and_prefixes() {
    const FORMAT: &str = r#"module checked.native;
@id("checked.native.main") fn main() -> i64 {
    let text = string_format("{}{}", "a", "β");
    string_len(text)
}
"#;
    const EMPTY: &str = r#"module checked.native;
@id("checked.native.main") fn main() -> i64 {
    let text = string_format("");
    string_len(text)
}
"#;
    // Two field literals, empty accumulator, two joins, tail owning-read clone.
    run_native(FORMAT, 6, 3, false);
    // Empty accumulator, literal empty part, join, tail owning-read clone.
    run_native(EMPTY, 4, 0, false);
}

#[test]
fn escaped_native_string_result_retains_provider_and_rejects_stale_borrow_drop() {
    for (source, allocations, expected, contract_failure) in [
        (
            r#"module checked.native;
@id("checked.native.publish") fn publish() -> string { string_concat("a", "β") }
@id("checked.native.main") fn main() -> i64 { 0 }
"#,
            3usize,
            "aβ",
            0u32,
        ),
        (
            r#"module checked.native;
@id("checked.native.publish") fn publish() -> string { "" }
@id("checked.native.main") fn main() -> i64 { 0 }
"#,
            1usize,
            "",
            0u32,
        ),
        (
            r#"module checked.native;
@id("checked.native.publish") fn publish() -> string requires false { string_concat("a", "β") }
@id("checked.native.main") fn main() -> i64 { 0 }
"#,
            0usize,
            "",
            1u32,
        ),
        (
            r#"module checked.native;
@id("checked.native.publish") fn publish() -> string ensures false { string_concat("a", "β") }
@id("checked.native.main") fn main() -> i64 { 0 }
"#,
            3usize,
            "",
            2u32,
        ),
    ] {
        let source = crate::parse(source, "checked-native-escaped.spx").unwrap();
        // The old scalar factory remains closed for an owning result.
        let entry = DeclarationId::new("checked.native.publish");
        assert!(crate::cleanup_plan::checked_allocation::prepare(&source, &entry).is_err());
        let owner = crate::cleanup_plan::checked_allocation::prepare_string_result(&source, &entry)
            .unwrap()
            .receive()
            .unwrap();
        let emitted = Projection::receive(&owner).unwrap().emit().unwrap();
        assert!(!emitted.contains("spx_checked_allocation_invoke_v1("));
        assert!(emitted.contains("spx_checked_string_result_invoke_v1("));
        let probe = format!("#define ESCAPED_REQUESTS {allocations}u\n#define ESCAPED_CONTRACT_FAILURE {contract_failure}u\n#define ESCAPED_TEXT \"{}\"\n#define ESCAPED_LENGTH {}u\n{}", c_string(expected), expected.len(), include_str!("escaped_probe.c"));
        run_generated(&emitted, &probe);
    }
}

fn run_native(source: &str, allocations: usize, result: i64, semantic_failure: bool) {
    let source = crate::parse(source, "checked-native.spx").unwrap();
    let owner = crate::cleanup_plan::checked_allocation::prepare(
        &source,
        &DeclarationId::new("checked.native.main"),
    )
    .unwrap()
    .receive()
    .unwrap();
    // The unchanged default owning read at main's tail clones text. This
    // request is distinct from the callee/conversion result construction.
    let main = owner
        .program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "checked.native.main")
        .unwrap();
    assert_eq!(
        main.cleanup_plan
            .status_sources
            .iter()
            .filter(|source| matches!(
                &source.producer,
                crate::cleanup_plan::StatusProducer::CheckedAllocation {
                    operation: crate::cleanup_plan::AllocationOperation::StringClone,
                    ..
                }
            ))
            .count(),
        1
    );
    let emitted = Projection::receive(&owner).unwrap().emit().unwrap();
    let probe = format!("#define CHECKED_EXPECT_ALLOCATIONS {allocations}u\n#define CHECKED_EXPECT_RESULT INT64_C({result})\n#define CHECKED_EXPECT_SEMANTIC {}\n{}", usize::from(semantic_failure), include_str!("probe.c"));
    run_generated(&emitted, &probe);
}
fn run_generated(emitted: &str, probe: &str) {
    let root = std::env::temp_dir().join(format!(
        "checked-native-allocation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("probe.c");
    std::fs::write(&input, format!("{emitted}\n{probe}")).unwrap();
    let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = std::process::Command::new(&compiler)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&input)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = std::process::Command::new(&binary).output().unwrap();
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
        // The C probe writes stdout in text mode, which is CRLF on Windows.
        let expected: &[u8] = if cfg!(windows) {
            b"checked-native-allocation: ok\r\n"
        } else {
            b"checked-native-allocation: ok\n"
        };
        assert_eq!(ran.stdout, expected);
        assert!(ran.stderr.is_empty());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn checked_runtime_zero_anchor_preserves_existing_authority_conjunctions_and_refuses_drift() {
    for suffix in [
        ";",
        " &&\n        context->borrowed_str_depth == UINT32_C(0);",
        " &&\n        context->vec_next_generation == UINT64_C(0);",
    ] {
        let prelude = format!("struct spx_string_v10 {{ uint64_t len; char data[]; }};\nstruct spx_context {{\n    uint32_t call_depth;\n}};\nstatic bool zero(const struct spx_context *context) {{ return\n        context->call_depth == UINT32_C(0){suffix}\n}}\n");
        let joined = super::join_runtime(prelude.clone()).unwrap();
        assert!(joined.contains(&format!("        context->call_depth == UINT32_C(0) && context->checked_allocation == NULL{suffix}")));
        for drift in [
            prelude.replace(
                "        context->call_depth",
                "        context->different_depth",
            ),
            format!("{prelude}\n        context->call_depth == UINT32_C(0);\n"),
        ] {
            assert_eq!(super::join_runtime(drift).unwrap_err().code, "SPX-B103");
        }
    }
}
