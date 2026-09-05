//! The provisioned-tool lanes: native C11 at O0 and O2, and Core-Wasm on Node.
//!
//! Both lanes report into the same closed vocabulary as the reference
//! interpreter. When `clang` or `node` is absent, or when a target refuses the
//! generated profile, the lane reports `Unavailable` with the exact reason.
//! That is an explicit outcome the checker records and prints; it is never
//! folded into the parity count.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use semaprax::ast::Program;
use semaprax::{codegen, wasm};

use super::grammar::Type;
use super::observe::{Case, Lane, LaneReport, LaneStatus, Observation};

/// Report whether a tool answers `--version`, and carry the exact identity
/// string when it does, so a discrepancy report names the toolchain that
/// produced it.
pub(crate) fn tool_identity(name: &str) -> Option<String> {
    let output = Command::new(name).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.lines().next().unwrap_or_default().trim().to_owned())
}

fn c_symbol(declaration_id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in declaration_id.bytes() {
        let _ = write!(symbol, "{byte:02x}");
    }
    symbol
}

/// A C probe that calls every observable case and prints one JSON line per
/// case, in the same shape the Node observer prints. The probe never asserts:
/// a wrong value or a wrong status has to reach the checker as data, not as a
/// process abort that hides which case disagreed.
fn native_probe(cases: &[Case]) -> String {
    let mut source = String::from(
        r#"
typedef spx_status_token (*spx_i64_case)(struct spx_context *, int64_t *);
typedef spx_status_token (*spx_bool_case)(struct spx_context *, bool *);

static void spx_emit_status(const char *id, struct spx_context *context, spx_status_token token) {
    const struct spx_normalized_status *status = spx_status_resolve(context, token);
    if (status == NULL) {
        printf("{\"id\":\"%s\",\"ok\":false,\"unresolved\":true}\n", id);
        return;
    }
    printf(
        "{\"id\":\"%s\",\"ok\":false,\"status\":{\"domain_id\":\"%s\",\"code\":%u}}\n",
        id, status->domain_id, (unsigned int)status->code
    );
}

static void spx_observe_i64(const char *id, spx_i64_case test_case) {
    struct spx_status_entry records[UINT32_C(4)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(911), records, UINT32_C(4), NULL, NULL, NULL)) {
        printf("{\"id\":\"%s\",\"ok\":false,\"init\":false}\n", id);
        return;
    }
    int64_t value = 0;
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"scalar\":\"i64\",\"value\":\"%lld\"}\n", id, (long long)value);
        return;
    }
    spx_emit_status(id, &context, token);
}

static void spx_observe_bool(const char *id, spx_bool_case test_case) {
    struct spx_status_entry records[UINT32_C(4)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(912), records, UINT32_C(4), NULL, NULL, NULL)) {
        printf("{\"id\":\"%s\",\"ok\":false,\"init\":false}\n", id);
        return;
    }
    bool value = false;
    spx_status_token token = test_case(&context, &value);
    if (token == SPX_STATUS_SUCCESS) {
        printf("{\"id\":\"%s\",\"ok\":true,\"scalar\":\"bool\",\"value\":\"%s\"}\n", id, value ? "true" : "false");
        return;
    }
    spx_emit_status(id, &context, token);
}

int main(void) {
"#,
    );
    for (stable_id, result) in cases {
        let symbol = c_symbol(stable_id);
        let observer = match result {
            Type::Bool => "spx_observe_bool",
            _ => "spx_observe_i64",
        };
        let _ = writeln!(source, "    {observer}(\"{stable_id}\", {symbol});");
    }
    source.push_str("    return 0;\n}\n");
    source
}

/// Compile and run the generated module through the native C11 backend at one
/// optimization level.
pub(crate) fn observe_native(
    cases: &[Case],
    program: &Program,
    root: &Path,
    optimization: &'static str,
) -> LaneReport {
    let lane = match optimization {
        "-O0" => Lane::NativeO0,
        _ => Lane::NativeO2,
    };
    if tool_identity("clang").is_none() {
        return LaneReport::unavailable(lane, "clang is not provisioned on this machine");
    }
    let generated = match codegen::emit_c(program) {
        Ok(generated) => generated,
        Err(error) => {
            return LaneReport::unavailable(
                lane,
                format!(
                    "the native backend does not admit this profile: {}: {}",
                    error.code, error.message
                ),
            )
        }
    };
    let source = root.join(format!("native{optimization}.c"));
    let executable = root.join(format!(
        "native{optimization}{}",
        std::env::consts::EXE_SUFFIX
    ));
    if let Err(error) = std::fs::write(&source, format!("{generated}\n{}", native_probe(cases))) {
        return LaneReport::unavailable(
            lane,
            format!("probe source could not be written: {error}"),
        );
    }
    // `-Werror` is deliberately absent here and only here. The existing fixed
    // fixture keeps it; generated code may legitimately produce an unused-value
    // warning, and a warning is not the subject of a differential comparison.
    let compile = format!(
        "clang -std=c11 {optimization} -Wall -Wextra -DSPX_NO_ENTRY_WRAPPER {} -o {}",
        source.display(),
        executable.display()
    );
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output();
    let commands = vec![compile, executable.display().to_string()];
    match compiled {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            return LaneReport {
                lane,
                status: LaneStatus::Aborted {
                    detail: format!(
                        "clang {optimization} rejected generated C: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                },
                commands,
            }
        }
        Err(error) => {
            return LaneReport::unavailable(lane, format!("clang could not be launched: {error}"))
        }
    }
    let run = Command::new(&executable).output();
    LaneReport {
        lane,
        status: transcript_status(run, cases),
        commands,
    }
}

/// Build the Core-Wasm web package with one scalar export per case and read the
/// results back through Node.
pub(crate) fn observe_core_wasm(cases: &[Case], program: &Program, root: &Path) -> LaneReport {
    let lane = Lane::CoreWasm;
    if tool_identity("node").is_none() {
        return LaneReport::unavailable(lane, "node is not provisioned on this machine");
    }
    let package = root.join("web");
    let exports = cases
        .iter()
        .map(|(stable_id, _)| stable_id.clone())
        .collect::<Vec<_>>();
    if let Err(error) = wasm::build_web_with_scalar_exports(program, &package, &exports) {
        return LaneReport::unavailable(
            lane,
            format!(
                "the Core-Wasm scalar export profile does not admit this module: {}: {}",
                error.code, error.message
            ),
        );
    }
    let script = root.join("observe.mjs");
    let case_list = exports
        .iter()
        .map(|id| format!("  {},", serde_json::to_string(id).unwrap_or_default()))
        .collect::<Vec<_>>()
        .join("\n");
    let observer = format!(
        r#"import {{ readFile }} from "node:fs/promises";
import {{ pathToFileURL }} from "node:url";
import {{ resolve }} from "node:path";

const packageDirectory = resolve(process.argv[2]);
const bindings = await import(pathToFileURL(resolve(packageDirectory, "semaprax.bindings.js")));
const runtime = await bindings.instantiateBytes(await readFile(resolve(packageDirectory, "app.wasm")));
const cases = [
{case_list}
];
for (const id of cases) {{
  const outcome = runtime.call(id);
  const observation = outcome.ok
    ? {{ id, ok: true, scalar: typeof outcome.value === "boolean" ? "bool" : "i64",
         value: typeof outcome.value === "boolean" ? String(outcome.value) : outcome.value.toString() }}
    : {{ id, ok: false, status: {{ domain_id: outcome.status.domain_id, code: outcome.status.code }} }};
  process.stdout.write(`${{JSON.stringify(observation)}}\n`);
}}
"#
    );
    if let Err(error) = std::fs::write(&script, observer) {
        return LaneReport::unavailable(
            lane,
            format!("Node observer could not be written: {error}"),
        );
    }
    let commands = vec![format!("node {} {}", script.display(), package.display())];
    let run = Command::new("node").arg(&script).arg(&package).output();
    LaneReport {
        lane,
        status: transcript_status(run, cases),
        commands,
    }
}

/// Turn a lane process result into the shared vocabulary. A process that dies,
/// prints nothing, or skips a case becomes an explicit abort or a missing case,
/// never an agreement.
pub(crate) fn transcript_status(
    run: Result<std::process::Output, std::io::Error>,
    cases: &[Case],
) -> LaneStatus {
    let output = match run {
        Ok(output) => output,
        Err(error) => {
            return LaneStatus::Aborted {
                detail: format!("the observer could not be launched: {error}"),
            }
        }
    };
    if !output.status.success() {
        return LaneStatus::Aborted {
            detail: format!(
                "observer exited with {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ),
        };
    }
    let transcript = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    let mut observations = BTreeMap::new();
    for line in transcript.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return LaneStatus::Aborted {
                detail: format!("observer printed a non-JSON line: {line}"),
            };
        };
        let Some(id) = value["id"].as_str() else {
            return LaneStatus::Aborted {
                detail: format!("observer printed a line without an id: {line}"),
            };
        };
        let observation = if value["ok"].as_bool() == Some(true) {
            Observation::Returned {
                scalar: match value["scalar"].as_str() {
                    Some("bool") => "bool",
                    _ => "i64",
                },
                value: value["value"].as_str().unwrap_or_default().to_owned(),
            }
        } else if value["status"].is_object() {
            Observation::Failed {
                domain: value["status"]["domain_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                code: value["status"]["code"].as_u64().unwrap_or(u64::MAX) as u32,
            }
        } else {
            Observation::Aborted {
                detail: format!("observer could not resolve an outcome: {line}"),
            }
        };
        observations.insert(id.to_owned(), observation);
    }
    for (stable_id, _) in cases {
        observations
            .entry(stable_id.clone())
            .or_insert(Observation::Aborted {
                detail: "the observer produced no line for this case".to_owned(),
            });
    }
    LaneStatus::Observed(observations)
}
