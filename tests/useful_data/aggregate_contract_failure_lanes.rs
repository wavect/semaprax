use semaprax::cleanup_plan::{ExitContinuation, StatusLane};
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, hir, wasm};
use std::path::PathBuf;
use std::process::Command;

const SOURCE: &str = r#"
module test.contract_lanes;
@id("lanes.packet") record Packet { @id("lanes.payload") payload: Bytes, }
@id("lanes.predicate") fn predicate(mode: i64) -> bool {
    if mode == 2 { 1 / 0 == 0 } else { mode == 1 }
}
@id("lanes.guard") fn guard(packet: own Packet) -> Packet
    PHASE predicate(MODE)
{ packet }
@id("app.main") fn main() -> i64 {
    let input = [42u8];
    let packet = guard(Packet { payload: bytes_copy(array_as_slice(input)) });
    match own packet { Packet { payload } =>
        if byte_len(bytes_as_slice(payload)) == 1usize { 42 } else { 0 }, }
}
"#;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn contract_call_lanes_preserve_status_cleanup_and_reentry_on_all_backends() {
    for phase in ["requires", "ensures"] {
        for mode in 0..=2 {
            let source = SOURCE
                .replace("PHASE", phase)
                .replace("MODE", &mode.to_string());
            let program = semaprax::check(&source, "contract-lanes.spx").unwrap();
            let canonical = semaprax::format::canonical(&program);
            assert_eq!(
                semaprax::format::canonical(
                    &semaprax::parse(&canonical, "contract-lanes.spx").unwrap()
                ),
                canonical
            );
            let root = Fixture(std::env::temp_dir().join(format!(
                "spx-contract-lanes-{}-{phase}-{mode}",
                std::process::id()
            )));
            std::fs::create_dir(&root.0).unwrap();
            let source_path = root.0.join("input.spx");
            std::fs::write(&source_path, &canonical).unwrap();
            let interpreted = interpreter::interpret(
                &source_path,
                "app.main",
                &[],
                &InterpreterOptions::default(),
            )
            .unwrap();
            interpreter::verify_envelope(&interpreted.envelope).unwrap();
            assert_eq!(interpreted.returned, mode == 1);
            let outcome: serde_json::Value = serde_json::from_str(&interpreted.envelope).unwrap();
            let outcome = &outcome["payload"]["outcome"];
            let (domain, code) = if mode == 2 {
                ("semaprax.arithmetic.v1", 4)
            } else {
                (
                    "semaprax.contract.v1",
                    if phase == "requires" { 1 } else { 2 },
                )
            };
            if mode == 1 {
                assert_eq!(outcome["value"], "42");
            } else {
                assert_eq!(outcome["status"]["domain_id"], domain);
                assert_eq!(outcome["status"]["code"], code);
            }

            let generated = codegen::emit_c(&program).unwrap();
            let probe = format!(
                r#"
#include <string.h>
int main(void) {{
    struct spx_status_entry entries[16];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, 77, entries, 16, NULL, NULL, NULL)) return 10;
    for (int i = 0; i < 3; ++i) {{
        int64_t output = INT64_C(12345);
        spx_status_token token = spx_decl_6170702e6d61696e(&context, &output);
        if ({success}) {{
            if (token != SPX_STATUS_SUCCESS || output != 42) return 11;
        }} else {{
            const struct spx_normalized_status *status = spx_status_resolve(&context, token);
            if (status == NULL || strcmp(status->domain_id, "{domain}") || status->code != {code}) return 12;
            if (output != INT64_C(12345)) return 13;
        }}
    }}
    return 0;
}}
"#,
                success = u8::from(mode == 1)
            );
            let c_path = root.0.join("probe.c");
            std::fs::write(&c_path, format!("{generated}\n{probe}")).unwrap();
            for optimization in ["-O0", "-O2"] {
                let executable = root.0.join(format!(
                    "probe{optimization}{}",
                    std::env::consts::EXE_SUFFIX
                ));
                let compiled = Command::new("clang")
                    .args([
                        "-std=c11",
                        optimization,
                        "-Wall",
                        "-Wextra",
                        "-Werror",
                        "-DSPX_NO_ENTRY_WRAPPER",
                    ])
                    .arg(&c_path)
                    .arg("-o")
                    .arg(&executable)
                    .output()
                    .unwrap();
                assert!(
                    compiled.status.success(),
                    "{}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                let executed = Command::new(&executable).output().unwrap();
                assert!(
                    executed.status.success(),
                    "{phase}/{mode}/{optimization}: {executed:?}"
                );
            }

            wasm::build_web(&program, &root.0.join("web")).unwrap();
            let expected = if mode == 2 {
                "SEMAPRAX checked arithmetic failure: invalid division"
            } else {
                "SEMAPRAX contract failure"
            };
            let script = format!(
                r#"
import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes}} from './semaprax.js';
const {{instance}} = await instantiateBytes(await readFile('./app.wasm'), {{maxOwnedByteEntries:1}});
for (let i=0; i<3; ++i) {{
    if ({success}) {{
        if (instance.exports.semaprax_main() !== 42n) throw Error('wrong result');
    }} else {{
        let failed = false;
        try {{ instance.exports.semaprax_main(); }} catch (error) {{
            if (error.message !== '{expected}') throw error;
            failed = true;
        }}
        if (!failed) throw Error('missing failure');
    }}
}}
"#,
                success = mode == 1
            );
            std::fs::write(root.0.join("web/probe.mjs"), script).unwrap();
            let node = Command::new("node")
                .arg("probe.mjs")
                .current_dir(root.0.join("web"))
                .output()
                .unwrap();
            assert!(node.status.success(), "{phase}/{mode}: {node:?}");
        }
    }
}

#[test]
fn forged_contract_lane_is_rejected_before_wasm_emission() {
    let source = SOURCE.replace("PHASE", "ensures").replace("MODE", "0");
    let program = semaprax::check(&source, "contract-lanes.spx").unwrap();
    let mut resolved = hir::resolve(&program).unwrap();
    let guard = resolved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "lanes.guard")
        .unwrap();
    let expression = guard.ensures[0].id.clone();
    for lane in [StatusLane::OperationFailure, StatusLane::ContractFalse] {
        assert_eq!(
            guard
                .cleanup_plan
                .exits
                .iter()
                .filter(|exit| matches!(
                    &exit.continuation, ExitContinuation::ReturnFailure { source }
                        if source.expression == expression && source.lane == lane
                ))
                .count(),
            1
        );
    }
    let exit = guard
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(
                &exit.continuation, ExitContinuation::ReturnFailure { source }
                    if source.expression == expression && source.lane == StatusLane::ContractFalse
            )
        })
        .unwrap();
    let ExitContinuation::ReturnFailure { source } = &mut exit.continuation else {
        unreachable!()
    };
    source.lane = StatusLane::OperationFailure;
    assert_eq!(
        wasm::emit_resolved_module(&resolved).unwrap_err().code,
        "SPX-H006"
    );
}
