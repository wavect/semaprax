use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cleanup_plan::{CleanupResultSource, ExitContinuation};
use crate::hir::{self, ResolvedFunction, ResolvedProgram};
use crate::parse;

use super::super::native_cleanup::classify;
use super::*;

static NEXT_TEST_BINARY: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"module test.native_cleanup_emit;

@id("token.type")
resource Token {
@id("token.drop")
drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.??/λ")
fn escaped_discard(value: own Token) -> i64 { 0 }

@id("token.contract-failure")
fn contract_failure(value: own Token) -> i64 requires false { 0 }

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64 requires number >= 0 { number + 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("native-cleanup-emit.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

fn complete_bindings(index: &NativeCleanupIndex<'_>) -> NativeCleanupBindings {
    let mut bindings = NativeCleanupBindings {
        context: "spx_bind_context".to_owned(),
        ..NativeCleanupBindings::default()
    };
    for slot in index.slots() {
        bindings.storage_values.insert(
            slot.slot.storage.clone(),
            format!("spx_bind_slot_{}", slot.slot.id.0),
        );
    }
    for edge in index.edges() {
        match &edge.condition {
            EdgeCondition::BooleanResult(expression, _) => {
                let next = bindings.boolean_values.len();
                bindings
                    .boolean_values
                    .entry(expression.clone())
                    .or_insert_with(|| format!("spx_bind_bool_{next}"));
            }
            // The native owned-resource slice rejects decision-chain
            // edges during binding validation before this point.
            EdgeCondition::VariantCase { .. } | EdgeCondition::ArmSelected { .. } => {}
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                let next = bindings.status_tokens.len();
                bindings
                    .status_tokens
                    .entry(source.clone())
                    .or_insert_with(|| format!("spx_bind_status_{next}"));
            }
            EdgeCondition::Always => {}
        }
    }
    for indexed in index.exits() {
        match &indexed.exit.continuation {
            ExitContinuation::CommitResult { source } => {
                bindings.result_out = Some("spx_bind_result_out".to_owned());
                if let CleanupResultSource::Scalar { expression } = source {
                    bindings
                        .scalar_results
                        .insert(expression.clone(), "spx_bind_scalar_result".to_owned());
                }
            }
            ExitContinuation::ReturnFailure { source } => {
                let next = bindings.status_tokens.len();
                bindings
                    .status_tokens
                    .entry(source.clone())
                    .or_insert_with(|| format!("spx_bind_status_{next}"));
            }
            ExitContinuation::Continue(_) | ExitContinuation::ReturnUnit => {}
        }
    }
    bindings
}

fn semantic_bindings(
    program: &ResolvedProgram,
    index: &NativeCleanupIndex<'_>,
) -> NativeCleanupBindings {
    let mut bindings = complete_bindings(index);
    bindings.semantic_events = Some(
        crate::semantic_trace::build_semantic_event_dictionary(program, index.function_id())
            .unwrap(),
    );
    bindings
}

#[test]
fn discard_emits_exact_plan_driven_c() {
    let program = program();
    let index = classify(&program, function(&program, "token.discard")).unwrap();
    let emitted = emit(&index, &complete_bindings(&index)).unwrap();
    let expected = concat!(
        "/* semaprax.native-cleanup-scaffold.v1 */\n",
        "bool spx_cleanup_flag_0 = false;\n",
        "spx_status_token spx_cleanup_selected_status = SPX_STATUS_SUCCESS;\n",
        "spx_cleanup_flag_0 = true;\n",
        "goto spx_cleanup_block_0;\n",
        "spx_cleanup_block_0:\n",
        "goto spx_cleanup_exit_0;\n",
        "spx_cleanup_exit_0:\n",
        "if (spx_cleanup_flag_0) {\n",
        "    spx_cleanup_flag_0 = false;\n",
        "{\n",
        "    struct spx_trace_event spx_cleanup_event = {0};\n",
        "    spx_cleanup_event.kind = SPX_TRACE_FINALIZE_BEGIN;\n",
        "    spx_cleanup_event.function_id = \"token.discard\";\n",
        "    spx_cleanup_event.data.finalize.source.storage.kind = SPX_TRACE_STORAGE_VALUE;\n",
        "    spx_cleanup_event.data.finalize.source.storage.value_id = \"declaration:13:token.discard:value:param:1:0\";\n",
        "    spx_cleanup_event.data.finalize.lifecycle_id = \"token.drop\";\n",
        "    spx_cleanup_event.data.finalize.guard_flag = UINT32_C(0);\n",
        "    spx_trace_push(spx_bind_context, &spx_cleanup_event);\n",
        "}\n",
        "{\n",
        "    struct spx_trace_event spx_cleanup_event = {0};\n",
        "    spx_cleanup_event.kind = SPX_TRACE_FINALIZE_END;\n",
        "    spx_cleanup_event.function_id = \"token.discard\";\n",
        "    spx_cleanup_event.data.finalize.source.storage.kind = SPX_TRACE_STORAGE_VALUE;\n",
        "    spx_cleanup_event.data.finalize.source.storage.value_id = \"declaration:13:token.discard:value:param:1:0\";\n",
        "    spx_cleanup_event.data.finalize.lifecycle_id = \"token.drop\";\n",
        "    spx_cleanup_event.data.finalize.guard_flag = UINT32_C(0);\n",
        "    spx_trace_push(spx_bind_context, &spx_cleanup_event);\n",
        "}\n",
        "}\n",
        "if (spx_cleanup_selected_status != SPX_STATUS_SUCCESS) spx_runtime_invariant_failure(\"cleanup result commit selected failure\");\n",
        "if (spx_cleanup_flag_0) spx_runtime_invariant_failure(\"cleanup scalar result commit retains a live resource\");\n",
        "*spx_bind_result_out = spx_bind_scalar_result;\n",
        "{\n",
        "    struct spx_trace_event spx_cleanup_event = {0};\n",
        "    spx_cleanup_event.kind = SPX_TRACE_RESULT_COMMIT;\n",
        "    spx_cleanup_event.function_id = \"token.discard\";\n",
        "    spx_cleanup_event.data.result_commit.source.kind = SPX_TRACE_RESULT_SCALAR;\n",
        "    spx_cleanup_event.data.result_commit.source.scalar_expression_id = \"declaration:13:token.discard:expression:4:body\";\n",
        "    spx_trace_push(spx_bind_context, &spx_cleanup_event);\n",
        "}\n",
        "return SPX_STATUS_SUCCESS;\n",
    );
    assert_eq!(emitted, expected);
}

#[test]
fn block_prologues_run_immediately_after_every_block_label() {
    let program = program();
    let index = classify(&program, function(&program, "token.checked")).unwrap();
    let bindings = complete_bindings(&index);
    let baseline = emit(&index, &bindings).unwrap();
    let no_op = emit_with_block_prologues(&index, &bindings, |_, _| Ok(())).unwrap();
    assert_eq!(no_op, baseline);

    let mut visited = Vec::new();
    let emitted = emit_with_block_prologues(&index, &bindings, |block, output| {
        visited.push(block);
        writeln!(output, "/* test block prologue {} */", block.0)
            .expect("writing to a string cannot fail");
        Ok(())
    })
    .unwrap();

    assert_eq!(
        visited,
        index
            .blocks()
            .iter()
            .map(|indexed| indexed.block.id)
            .collect::<Vec<_>>()
    );
    for indexed in index.blocks() {
        let block = indexed.block.id;
        assert!(emitted.contains(&format!(
            "{}:\n/* test block prologue {} */\n",
            block_label(block),
            block.0
        )));
    }
}

#[test]
fn block_prologue_error_is_propagated_without_visiting_later_blocks() {
    let program = program();
    let index = classify(&program, function(&program, "token.checked")).unwrap();
    assert!(index.blocks().len() > 1);
    let mut visited = Vec::new();
    let diagnostic = emit_with_block_prologues(&index, &complete_bindings(&index), |block, _| {
        visited.push(block);
        Err(Diagnostic::io(
            "SPX-TEST",
            format!("failed block prologue {}", block.0),
        ))
    })
    .unwrap_err();

    assert_eq!(visited, vec![index.entry()]);
    assert_eq!(diagnostic.code, "SPX-TEST");
    assert_eq!(
        diagnostic.message,
        format!("failed block prologue {}", index.entry().0)
    );
}

#[test]
fn c_literals_are_byte_exact_and_never_expose_trigraphs() {
    let escaped = c_string("safe??/λ\n\r\t\\\"\u{7f}");
    assert_eq!(escaped, "safe\\077\\077/\\316\\273\\n\\r\\t\\\\\\\"\\177");
    assert!(!escaped.contains("??"));
    assert!(escaped.is_ascii());

    let program = program();
    let index = classify(&program, function(&program, "token.??/λ")).unwrap();
    assert_eq!(index.function_id().as_str(), "token.??/λ");
    let emitted = emit(&index, &complete_bindings(&index)).unwrap();
    assert!(emitted.contains("token.\\077\\077/\\316\\273"));
    assert!(!emitted.contains("??"));
    assert!(emitted.is_ascii());
}

#[test]
fn emitted_discard_trace_compiles_and_runs_with_strict_c11() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = program();
    let index = classify(&program, function(&program, "token.discard")).unwrap();
    let emitted = emit(&index, &semantic_bindings(&program, &index)).unwrap();
    let identity_index = classify(&program, function(&program, "token.identity")).unwrap();
    let identity_bindings = semantic_bindings(&program, &identity_index);
    let identity = emit(&identity_index, &identity_bindings).unwrap();
    let mut identity_storage_parameters = String::new();
    for slot in identity_index.slots() {
        writeln!(
            identity_storage_parameters,
            "uintptr_t {},",
            identity_bindings.storage_values[&slot.slot.storage]
        )
        .expect("writing to a string cannot fail");
    }
    let identity_storage_arguments = identity_index
        .slots()
        .iter()
        .map(|slot| {
            if matches!(slot.slot.storage, StorageId::Value(_)) {
                "(uintptr_t)UINT32_C(73)".to_owned()
            } else {
                "(uintptr_t)UINT32_C(0)".to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let failure_index = classify(&program, function(&program, "token.contract-failure")).unwrap();
    let failure = emit(&failure_index, &semantic_bindings(&program, &failure_index)).unwrap();
    let escaped_index = classify(&program, function(&program, "token.??/λ")).unwrap();
    let escaped = emit(&escaped_index, &semantic_bindings(&program, &escaped_index)).unwrap();

    let mut runtime = String::new();
    super::super::native_runtime::emit_status_runtime(&mut runtime);
    super::super::native_trace_runtime::emit_trace_runtime(&mut runtime);
    let source = format!(
        "{runtime}\n\
         static __attribute__((noreturn)) void spx_runtime_invariant_failure(const char *message) {{ (void)message; abort(); }}\n\
         static void spx_test_retain_status_runtime(void) {{\n\
             (void)spx_status_resolve;\n\
             (void)spx_status_attach_detail;\n\
             (void)spx_status_resolve_detail;\n\
             (void)spx_status_record_requires_false;\n\
             (void)spx_status_record_ensures_false;\n\
             (void)spx_status_record_arithmetic;\n\
         }}\n\
         static spx_status_token spx_test_discard(\n\
             struct spx_context *spx_bind_context,\n\
             uintptr_t spx_bind_slot_0,\n\
             int64_t *spx_bind_result_out,\n\
             int64_t spx_bind_scalar_result\n\
         ) {{\n\
             (void)spx_bind_slot_0;\n\
         {emitted}\
         }}\n\
         static spx_status_token spx_test_identity(\n\
             struct spx_context *spx_bind_context,\n\
         {identity_storage_parameters}\
             uintptr_t *spx_bind_result_out\n\
         ) {{\n\
         {identity}\
         }}\n\
         static spx_status_token spx_test_contract_failure(\n\
             struct spx_context *spx_bind_context,\n\
             uintptr_t spx_bind_slot_0,\n\
             bool spx_bind_bool_0,\n\
             spx_status_token spx_bind_status_0,\n\
             int64_t *spx_bind_result_out,\n\
             int64_t spx_bind_scalar_result\n\
         ) {{\n\
             (void)spx_bind_slot_0;\n\
         {failure}\
         }}\n\
         static spx_status_token spx_test_escaped_discard(\n\
             struct spx_context *spx_bind_context,\n\
             uintptr_t spx_bind_slot_0,\n\
             int64_t *spx_bind_result_out,\n\
             int64_t spx_bind_scalar_result\n\
         ) {{\n\
             (void)spx_bind_slot_0;\n\
         {escaped}\
         }}\n\
         int main(void) {{\n\
             spx_test_retain_status_runtime();\n\
             struct spx_status_entry status_entries[UINT32_C(1)] = {{0}};\n\
             struct spx_context context = {{0}};\n\
             if (!spx_context_init(&context, UINT64_C(1), status_entries, UINT32_C(1), NULL, NULL, NULL)) return 1;\n\
             struct spx_trace_event events[UINT32_C(3)] = {{0}};\n\
             struct spx_trace_buffer trace = {{0}};\n\
             if (!spx_trace_buffer_init(&trace, events, UINT32_C(3))) return 2;\n\
             if (!spx_trace_attach_preflight(&context, &trace, UINT32_C(3))) return 3;\n\
             int64_t result = INT64_C(0);\n\
             if (spx_test_discard(&context, (uintptr_t)UINT32_C(99), &result, INT64_C(7)) != SPX_STATUS_SUCCESS) return 4;\n\
             if (result != INT64_C(7) || trace.length != UINT32_C(3)) return 5;\n\
             if (events[0].kind != SPX_TRACE_FINALIZE_BEGIN || events[1].kind != SPX_TRACE_FINALIZE_END || events[2].kind != SPX_TRACE_RESULT_COMMIT) return 6;\n\
             if (events[0].semantic_ordinal != UINT32_C(1) || events[1].semantic_ordinal != UINT32_C(2) || events[2].semantic_ordinal != UINT32_C(3)) return 26;\n\
             if (strcmp(events[0].function_id, \"token.discard\") != 0 || strcmp(events[1].function_id, \"token.discard\") != 0 || strcmp(events[2].function_id, \"token.discard\") != 0) return 7;\n\
             if (events[0].data.finalize.source.storage.kind != SPX_TRACE_STORAGE_VALUE || strcmp(events[0].data.finalize.source.storage.value_id, \"declaration:13:token.discard:value:param:1:0\") != 0 || strcmp(events[0].data.finalize.lifecycle_id, \"token.drop\") != 0 || events[0].data.finalize.guard_flag != UINT32_C(0)) return 8;\n\
             if (events[2].data.result_commit.source.kind != SPX_TRACE_RESULT_SCALAR || strcmp(events[2].data.result_commit.source.scalar_expression_id, \"declaration:13:token.discard:expression:4:body\") != 0) return 9;\n\
             struct spx_status_entry identity_status_entries[UINT32_C(1)] = {{0}};\n\
             struct spx_context identity_context = {{0}};\n\
             if (!spx_context_init(&identity_context, UINT64_C(2), identity_status_entries, UINT32_C(1), NULL, NULL, NULL)) return 10;\n\
             struct spx_trace_event identity_events[UINT32_C(3)] = {{0}};\n\
             struct spx_trace_buffer identity_trace = {{0}};\n\
             if (!spx_trace_buffer_init(&identity_trace, identity_events, UINT32_C(3)) || !spx_trace_attach_preflight(&identity_context, &identity_trace, UINT32_C(3))) return 11;\n\
             uintptr_t owned_result = (uintptr_t)UINT32_C(0);\n\
             if (spx_test_identity(&identity_context, {identity_storage_arguments}, &owned_result) != SPX_STATUS_SUCCESS || owned_result != (uintptr_t)UINT32_C(73)) return 12;\n\
             if (identity_trace.length != UINT32_C(3) || identity_events[0].kind != SPX_TRACE_TRANSFER || identity_events[1].kind != SPX_TRACE_TRANSFER || identity_events[2].kind != SPX_TRACE_RESULT_COMMIT) return 13;\n\
             if (identity_events[0].semantic_ordinal != UINT32_C(1) || identity_events[1].semantic_ordinal != UINT32_C(2) || identity_events[2].semantic_ordinal != UINT32_C(3)) return 27;\n\
             if (identity_events[0].data.transfer.source.storage.kind != SPX_TRACE_STORAGE_VALUE || identity_events[0].data.transfer.destination.storage.kind != SPX_TRACE_STORAGE_TEMPORARY || identity_events[1].data.transfer.source.storage.kind != SPX_TRACE_STORAGE_TEMPORARY || identity_events[1].data.transfer.destination.storage.kind != SPX_TRACE_STORAGE_PROVISIONAL_RESULT || identity_events[2].data.result_commit.source.kind != SPX_TRACE_RESULT_OWNED || identity_events[2].data.result_commit.source.owned_storage.storage.kind != SPX_TRACE_STORAGE_PROVISIONAL_RESULT) return 14;\n\
             struct spx_status_entry failure_status_entries[UINT32_C(1)] = {{0}};\n\
             struct spx_context failure_context = {{0}};\n\
             if (!spx_context_init(&failure_context, UINT64_C(3), failure_status_entries, UINT32_C(1), NULL, NULL, NULL)) return 15;\n\
             spx_status_token contract_status = SPX_STATUS_SUCCESS;\n\
             if (!spx_status_record_requires_false(&failure_context, &contract_status)) return 16;\n\
             struct spx_trace_event failure_events[UINT32_C(3)] = {{0}};\n\
             struct spx_trace_buffer failure_trace = {{0}};\n\
             if (!spx_trace_buffer_init(&failure_trace, failure_events, UINT32_C(3)) || !spx_trace_attach_preflight(&failure_context, &failure_trace, UINT32_C(3))) return 17;\n\
             int64_t poisoned_result = INT64_C(99);\n\
             if (spx_test_contract_failure(&failure_context, (uintptr_t)UINT32_C(88), false, contract_status, &poisoned_result, INT64_C(0)) != contract_status || poisoned_result != INT64_C(99)) return 18;\n\
             if (failure_trace.length != UINT32_C(3) || failure_events[0].kind != SPX_TRACE_SELECT_FAILURE || failure_events[1].kind != SPX_TRACE_FINALIZE_BEGIN || failure_events[2].kind != SPX_TRACE_FINALIZE_END) return 19;\n\
             if (failure_events[0].semantic_ordinal != UINT32_C(1) || failure_events[1].semantic_ordinal != UINT32_C(2) || failure_events[2].semantic_ordinal != UINT32_C(3)) return 28;\n\
             if (failure_events[0].data.select_failure.source.lane != SPX_TRACE_STATUS_CONTRACT_FALSE || strcmp(failure_events[0].data.select_failure.status.domain_id, \"semaprax.contract.v1\") != 0 || failure_events[0].data.select_failure.status.code != SPX_STATUS_CONTRACT_REQUIRES_FALSE || failure_events[0].data.select_failure.status.status_class != SPX_TRACE_STATUS_CLASS_CONTRACT) return 20;\n\
             struct spx_status_entry escaped_status_entries[UINT32_C(1)] = {{0}};\n\
             struct spx_context escaped_context = {{0}};\n\
             if (!spx_context_init(&escaped_context, UINT64_C(4), escaped_status_entries, UINT32_C(1), NULL, NULL, NULL)) return 21;\n\
             struct spx_trace_event escaped_events[UINT32_C(3)] = {{0}};\n\
             struct spx_trace_buffer escaped_trace = {{0}};\n\
             if (!spx_trace_buffer_init(&escaped_trace, escaped_events, UINT32_C(3)) || !spx_trace_attach_preflight(&escaped_context, &escaped_trace, UINT32_C(3))) return 22;\n\
             int64_t escaped_result = INT64_C(0);\n\
             if (spx_test_escaped_discard(&escaped_context, (uintptr_t)UINT32_C(55), &escaped_result, INT64_C(11)) != SPX_STATUS_SUCCESS || escaped_result != INT64_C(11)) return 23;\n\
             static const unsigned char expected_function_id[] = {{'t', 'o', 'k', 'e', 'n', '.', 0x3f, 0x3f, '/', 0xce, 0xbb, 0x00}};\n\
             if (escaped_trace.length != UINT32_C(3)) return 24;\n\
             if (escaped_events[0].semantic_ordinal != UINT32_C(1) || escaped_events[1].semantic_ordinal != UINT32_C(2) || escaped_events[2].semantic_ordinal != UINT32_C(3)) return 29;\n\
             if (memcmp(escaped_events[0].function_id, expected_function_id, sizeof(expected_function_id)) != 0 || memcmp(escaped_events[1].function_id, expected_function_id, sizeof(expected_function_id)) != 0 || memcmp(escaped_events[2].function_id, expected_function_id, sizeof(expected_function_id)) != 0) return 25;\n\
             return 0;\n\
         }}\n"
    );
    let suffix = NEXT_TEST_BINARY.fetch_add(1, Ordering::Relaxed);
    let binary = std::env::temp_dir().join(format!(
        "semaprax-native-cleanup-trace-{}-{suffix}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    let mut compiler = Command::new("clang")
        .args(["-x", "c", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg("-o")
        .arg(&binary)
        .arg("-")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("clang was available during the version probe");
    compiler
        .stdin
        .take()
        .expect("clang stdin")
        .write_all(source.as_bytes())
        .expect("write C fixture");
    let compiled = compiler.wait_with_output().expect("wait for clang");
    assert!(
        compiled.status.success(),
        "strict C11 compilation failed:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let run = Command::new(&binary)
        .output()
        .expect("run cleanup trace fixture");
    let _ = std::fs::remove_file(&binary);
    assert!(
        run.status.success(),
        "cleanup trace fixture exited {}",
        run.status
    );
}

#[test]
fn reverse_finalizers_clear_each_guard_before_begin_and_end_events() {
    let program = program();
    let index = classify(&program, function(&program, "token.discard-two")).unwrap();
    let emitted = emit(&index, &complete_bindings(&index)).unwrap();
    let second_clear = emitted.find("spx_cleanup_flag_1 = false;").unwrap();
    let second_begin = emitted[second_clear..]
        .find("spx_cleanup_event.kind = SPX_TRACE_FINALIZE_BEGIN;")
        .map(|offset| second_clear + offset)
        .unwrap();
    let second_end = emitted[second_begin..]
        .find("spx_cleanup_event.kind = SPX_TRACE_FINALIZE_END;")
        .map(|offset| second_begin + offset)
        .unwrap();
    let first_clear = emitted[second_end + 1..]
        .find("spx_cleanup_flag_0 = false;")
        .map(|offset| second_end + 1 + offset)
        .unwrap();
    let first_begin = emitted[first_clear..]
        .find("spx_cleanup_event.kind = SPX_TRACE_FINALIZE_BEGIN;")
        .map(|offset| first_clear + offset)
        .unwrap();
    let first_end = emitted[first_begin..]
        .find("spx_cleanup_event.kind = SPX_TRACE_FINALIZE_END;")
        .map(|offset| first_begin + offset)
        .unwrap();
    assert!(second_clear < second_begin);
    assert!(second_begin < second_end);
    assert!(second_end < first_clear);
    assert!(first_clear < first_begin);
    assert!(first_begin < first_end);
}

#[test]
fn contract_failure_scaffold_is_deterministic_and_sticky() {
    let program = program();
    let index = classify(&program, function(&program, "token.contract-failure")).unwrap();
    let bindings = complete_bindings(&index);
    let first = emit(&index, &bindings).unwrap();
    let second = emit(&index, &bindings).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("cleanup failure selection is not write-once"));
    assert!(first.contains("spx_cleanup_event.kind = SPX_TRACE_SELECT_FAILURE;"));
    assert!(first.contains(
        "spx_cleanup_event.data.select_failure.source.lane = SPX_TRACE_STATUS_CONTRACT_FALSE;"
    ));
    assert!(first.contains("spx_status_resolve(spx_bind_context, spx_bind_status_"));
    assert!(first.contains(
        "spx_cleanup_event.data.select_failure.status.domain_id = spx_cleanup_normalized_status->domain_id;"
    ));
    let terminal_status = first
        .rfind("cleanup failure return changed status")
        .expect("exact failure status assertion");
    let terminal_liveness = first
        .rfind("cleanup failure return retains a live resource")
        .expect("failure liveness assertion");
    let failure_return = first
        .rfind("return spx_cleanup_selected_status;")
        .expect("sticky failure return");
    assert!(terminal_status < terminal_liveness);
    assert!(terminal_liveness < failure_return);
    assert!(first.contains("spx_cleanup_exit_"));
}

#[test]
fn real_contract_and_checked_arithmetic_branches_emit_canonical_continue() {
    let program = program();
    let index = classify(&program, function(&program, "token.checked")).unwrap();
    let emitted = emit(&index, &complete_bindings(&index)).unwrap();

    emitted
        .find("if (spx_bind_bool_0) goto spx_cleanup_block_")
        .expect("contract success branch");
    emitted
        .find("else if (!spx_bind_bool_0) goto spx_cleanup_block_")
        .expect("contract failure branch");
    emitted
        .find("spx_cleanup_exit_1:\ngoto spx_cleanup_block_4;")
        .expect("empty-region continuation");
    emitted
        .find("== SPX_STATUS_SUCCESS) goto spx_cleanup_block_")
        .expect("checked arithmetic success branch");
    emitted
        .find("!= SPX_STATUS_SUCCESS) goto spx_cleanup_block_")
        .expect("checked arithmetic failure branch");
    emitted
        .find("spx_cleanup_selected_status = spx_bind_status_")
        .expect("sticky failure selection");
    assert!(emitted.contains(
        "strcmp(spx_cleanup_normalized_status->domain_id, \"semaprax.arithmetic.v1\") != 0"
    ));
    assert!(emitted
        .contains("spx_cleanup_normalized_status->code != SPX_STATUS_ARITHMETIC_ADD_OVERFLOW"));
    assert!(emitted.contains(
        "spx_cleanup_event.data.select_failure.source.lane = SPX_TRACE_STATUS_OPERATION_FAILURE;"
    ));
    let result_assertion = emitted
        .rfind("cleanup result commit selected failure")
        .expect("success terminal assertion");
    let result_write = emitted
        .rfind("*spx_bind_result_out = spx_bind_scalar_result;")
        .expect("scalar result publication");

    assert!(result_assertion < result_write);
}

#[test]
fn owned_result_requires_exact_provisional_liveness_before_publication() {
    let program = program();
    let index = classify(&program, function(&program, "token.identity")).unwrap();
    let bindings = complete_bindings(&index);
    let provisional = bindings
        .storage_values
        .get(&StorageId::ProvisionalResult)
        .expect("provisional result binding");
    let provisional_leaf = index
        .slot(&StorageId::ProvisionalResult)
        .expect("provisional cleanup slot")
        .leaf
        .flag;
    let emitted = emit(&index, &bindings).unwrap();

    assert!(emitted.contains("spx_cleanup_event.kind = SPX_TRACE_TRANSFER;"));
    assert!(emitted.contains(
        "spx_cleanup_event.data.transfer.source.storage.kind = SPX_TRACE_STORAGE_VALUE;"
    ));
    assert!(emitted.contains(
        "spx_cleanup_event.data.transfer.destination.storage.kind = SPX_TRACE_STORAGE_PROVISIONAL_RESULT;"
    ));
    assert!(emitted.contains("spx_cleanup_event.kind = SPX_TRACE_RESULT_COMMIT;"));
    assert!(emitted
        .contains("spx_cleanup_event.data.result_commit.source.kind = SPX_TRACE_RESULT_OWNED;"));
    assert!(emitted.contains(
        "spx_cleanup_event.data.result_commit.source.owned_storage.storage.kind = SPX_TRACE_STORAGE_PROVISIONAL_RESULT;"
    ));
    assert!(!emitted.contains("semaprax.cleanup."));

    let status = emitted
        .rfind("cleanup result commit selected failure")
        .expect("success status assertion");
    let other_dead = emitted
        .rfind("cleanup owned result commit retains another live resource")
        .expect("non-result liveness assertion");
    let result_live = emitted
        .rfind("cleanup publishes a dead owned result")
        .expect("result liveness assertion");
    let publication = emitted
        .rfind(&format!("*spx_bind_result_out = {provisional};"))
        .expect("owned publication");
    let clear = emitted[publication..]
        .find(&format!("spx_cleanup_flag_{} = false;", provisional_leaf.0))
        .map(|offset| publication + offset)
        .expect("post-publication liveness clear");
    let success_return = emitted[clear..]
        .find("return SPX_STATUS_SUCCESS;")
        .map(|offset| clear + offset)
        .expect("success return");

    assert!(status < other_dead);
    assert!(other_dead < result_live);
    assert!(result_live < publication);
    assert!(publication < clear);
    assert!(clear < success_return);
}

#[test]
fn emitter_rejects_cleanup_bearing_continue_independently() {
    let program = program();
    let index = classify(&program, function(&program, "token.contract-failure")).unwrap();
    let mut exit = index
        .exits()
        .iter()
        .find(|indexed| matches!(indexed.exit.continuation, ExitContinuation::Continue(_)))
        .expect("compiler contract continuation")
        .exit
        .clone();
    let finalizer = index
        .exits()
        .iter()
        .flat_map(|indexed| indexed.finalizers)
        .next()
        .expect("terminal cleanup")
        .clone();
    exit.finalize_in_order.push(finalizer);
    let mut output = String::new();
    let diagnostic =
        emit_continuation(&mut output, &index, &complete_bindings(&index), &exit).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("performs cleanup"));
    assert!(output.is_empty());

    let mut conditional = index
        .exits()
        .iter()
        .find(|indexed| matches!(indexed.exit.continuation, ExitContinuation::Continue(_)))
        .expect("compiler contract continuation")
        .exit
        .clone();
    let conditional_edge = index
        .edges()
        .iter()
        .find(|edge| !matches!(edge.condition, EdgeCondition::Always))
        .expect("contract branch")
        .id;
    conditional.continuation = ExitContinuation::Continue(conditional_edge);
    let diagnostic = emit_continuation(
        &mut output,
        &index,
        &complete_bindings(&index),
        &conditional,
    )
    .unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("unconditional edge"));
    assert!(output.is_empty());
}

#[test]
fn missing_and_extra_observation_bindings_fail_closed() {
    let program = program();
    let index = classify(&program, function(&program, "token.contract-failure")).unwrap();
    let mut missing = complete_bindings(&index);
    missing.boolean_values.clear();
    let diagnostic = emit(&index, &missing).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("missing boolean binding"));

    let mut extra = complete_bindings(&index);
    extra
        .storage_values
        .insert(StorageId::ProvisionalResult, "spx_bind_extra".to_owned());
    let diagnostic = emit(&index, &extra).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("unexpected storage binding"));

    let mut wrong_context = complete_bindings(&index);
    wrong_context.context = "spx_bind_other_context".to_owned();
    let diagnostic = emit(&index, &wrong_context).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("exactly `spx_bind_context`"));
}

#[test]
fn arbitrary_c_expressions_are_not_accepted_as_bindings() {
    let program = program();
    let index = classify(&program, function(&program, "token.discard")).unwrap();
    let mut bindings = complete_bindings(&index);
    bindings.result_out = Some("spx_bind_result_out + 1".to_owned());
    let diagnostic = emit(&index, &bindings).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("is not one C identifier"));
}

#[test]
fn keywords_aliases_and_scaffold_names_are_rejected() {
    let program = program();
    let index = classify(&program, function(&program, "token.discard")).unwrap();

    let mut keyword = complete_bindings(&index);
    keyword.result_out = Some("return".to_owned());
    let diagnostic = emit(&index, &keyword).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("reserved C keyword"));

    let mut alias = complete_bindings(&index);
    alias.result_out = alias.storage_values.values().next().cloned();
    let diagnostic = emit(&index, &alias).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("aliases two"));

    for identifier in [
        "true",
        "false",
        "bool",
        "NULL",
        "SPX_STATUS_SUCCESS",
        "__implementation",
        "_Reserved",
        "spx_runtime_invariant_failure",
        "SPX_PRIVATE",
    ] {
        let mut reserved = complete_bindings(&index);
        reserved.result_out = Some(identifier.to_owned());
        let diagnostic = emit(&index, &reserved).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("is reserved"));
    }

    for identifier in [
        "UINT32_MAX",
        "SIZE_MAX",
        "INT64_MAX",
        "PTRDIFF_MAX",
        "stderr",
        "spx_bind_",
    ] {
        let mut outside_allocator = complete_bindings(&index);
        outside_allocator.result_out = Some(identifier.to_owned());
        let diagnostic = emit(&index, &outside_allocator).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic
            .message
            .contains("dedicated `spx_bind_` namespace"));
    }
}

#[test]
fn initialize_without_a_physical_payload_source_fails_closed() {
    let program = program();
    let index = classify(&program, function(&program, "token.discard")).unwrap();
    let destination = index.slots()[0].leaf.place.clone();
    let at = match &index.exits()[0].exit.continuation {
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Scalar { expression },
        } => expression.clone(),
        continuation => panic!("unexpected continuation: {continuation:?}"),
    };
    let transition = CleanupTransition::Initialize { at, destination };
    let mut output = String::new();
    let diagnostic =
        emit_transition(&mut output, &index, &complete_bindings(&index), &transition).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("no physical payload source"));
    assert!(output.is_empty());
}
