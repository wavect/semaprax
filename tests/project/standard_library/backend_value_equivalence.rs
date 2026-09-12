//! Issue #102: `run_examples_and_conformance`'s general per-package loop
//! asserted the interpreter, native, and Core Wasm backends each
//! independently returned `0`, but never compared one backend's actual
//! computed value to another's. Three backends each self-reporting success
//! is not equivalence — a backend that always reports success regardless of
//! what it actually computed would pass that loop undetected, and every
//! existing std package's `examples`/`tests` closure reduces its result to
//! the boolean sentinel `0`/`1` by convention, so even a genuine
//! interpreter-vs-native-vs-Wasm value comparison over those closures alone
//! has no discriminating power: the ground truth is always `0`.
//!
//! This module proves the cross-backend comparison the sibling fix wires
//! into `run_examples_and_conformance` (interpreter value vs native's
//! `printf`-ed value vs Wasm's returned value, instead of three independent
//! `== 0` checks) actually has teeth, by exercising it against a fixture
//! whose result is a real, non-trivial `i64` computed from live calls into
//! `std.core` and `std.num` — not a boolean success flag. A backend that
//! miscompiled any one of `compare`, `clamp`, `max`, or `sign`, or any of
//! the arithmetic combining them, would almost certainly produce a
//! different number, not just a different zero-ness.

use std::process::Command;

use semaprax::{codegen, project, wasm};

use super::temporary::temporary;

/// The independently hand-computed result of the `discriminator.compute`
/// fixture below, using the documented semantics of the std functions it
/// calls (`std.core.compare` is a three-way spaceship compare, `std.core.max`
/// picks the larger operand, `std.core.clamp` bounds a value into a closed
/// range, and `std.num.sign` is a three-way sign test):
///
/// ```text
/// a = compare(3, 9)        = -1   (3 < 9)
/// b = clamp(57, -20, 20)   = 20   (57 saturates at the high bound)
/// c = sign(-42)            = -1
/// d = max(a, c)            = max(-1, -1) = -1
/// e = compare(9, 3)        = 1    (9 > 3)
/// value = a*1000 + b*37 + c*13 + d*7 + e
///       = -1000  + 740   + -13  + -7  + 1
///       = -279
/// ```
///
/// This is not `0` or `1`: a backend that silently flipped a comparison, an
/// operand order, or an arithmetic operator would produce a different
/// concrete `i64`, not merely a different truth value.
const GROUND_TRUTH: i64 = -279;

const MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n\
[package]\n\
name = \"backend-value-equivalence\"\n\
version = \"0.1.0\"\n\n\
[modules]\n\
entry = \"discriminator.app\"\n\
sources = [\"src/app.spx\", \"src/tests.spx\"]\n\
tests = [\"discriminator.tests\"]\n\n\
[exports]\n\
web = [\"discriminator.compute\"]\n\n\
[dependencies]\n\
std.core = \"~0.1.0\"\n\
std.num = \"^0.1.0\"\n";

const APP_SPX: &str = "module discriminator.app;\n\
use function @id(\"std.core.compare\") from std.core as compare;\n\
use function @id(\"std.core.clamp\") from std.core as clamp;\n\
use function @id(\"std.core.max\") from std.core as max;\n\
use function @id(\"std.num.sign\") from std.num as sign;\n\n\
@id(\"discriminator.compute\")\n\
fn main() -> i64\n\
{\n    \
    let a = compare(3, 9);\n    \
    let b = clamp(57, -20, 20);\n    \
    let c = sign(-42);\n    \
    let d = max(a, c);\n    \
    let e = compare(9, 3);\n    \
    a * 1000 + b * 37 + c * 13 + d * 7 + e\n\
}\n";

const TESTS_SPX: &str = "module discriminator.tests;\n\
use function @id(\"std.core.compare\") from std.core as compare;\n\n\
@id(\"discriminator.tests.main\")\n\
fn main() -> i64\n\
{\n    \
    if compare(1, 2) == -1 { 0 } else { 1 }\n\
}\n";

/// Parses the `i64` a node script printed as its sole line of stdout.
fn parse_node_i64(output: &std::process::Output) -> i64 {
    assert!(
        output.status.success(),
        "node conformance script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<i64>()
        .unwrap_or_else(|error| panic!("node script printed a non-i64 result {stdout:?}: {error}"))
}

/// Proves the cross-backend comparison wired into `run_examples_and_conformance`
/// (issue #102) is discriminating: it exercises a fixture whose interpreter,
/// native, and Core Wasm result is a non-trivial computed `i64`
/// (`GROUND_TRUTH = -279`, not the boolean `0`/`1` every std package's own
/// `examples`/`tests` closure reduces to), and cross-asserts all three
/// backends computed the exact same value — not merely that each backend
/// independently reported "success".
#[test]
fn discriminating_fixture_agrees_across_interpreter_native_and_wasm() {
    if cfg!(windows) {
        return;
    }
    let scratch = temporary("backend-value-equivalence");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(scratch.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(scratch.join("src/app.spx"), APP_SPX).unwrap();
    std::fs::write(scratch.join("src/tests.spx"), TESTS_SPX).unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();

        // Interpreter.
        let entry = snapshot.execute_entry(&options)?;
        let interpreter_value = match entry.outcome() {
            project::ProjectExecutionOutcome::Returned(value) => *value,
            other => panic!("discriminator fixture failed on the interpreter: {other:?}"),
        };
        assert_eq!(
            interpreter_value, GROUND_TRUTH,
            "the interpreter's own computed value diverged from the hand-computed ground truth; \
             the fixture's arithmetic (or the documented std semantics it relies on) needs \
             re-deriving, not the cross-backend comparison below"
        );

        // Native (both optimization levels): the entry wrapper `codegen`
        // emits always `printf("%lld\n", result)` the full `i64` result
        // before returning process exit code `0` (see
        // `src/codegen/native_emit/mod.rs`), so the printed value carries
        // the actual computed result, not just a pass/fail signal.
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            let binary = scratch.join(format!("discriminator{}", optimization.to_lowercase()));
            let c_path = binary.with_extension("c");
            std::fs::write(&c_path, &c).unwrap();
            let compiled = Command::new("clang")
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&c_path)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "clang {optimization} failed: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let run = Command::new(&binary).output().unwrap();
            assert!(run.status.success(), "{} failed", binary.display());
            let native_value = String::from_utf8_lossy(&run.stdout)
                .trim()
                .parse::<i64>()
                .unwrap_or_else(|error| panic!("native {optimization} printed a non-i64 result: {error}"));
            assert_eq!(
                native_value, interpreter_value,
                "native ({optimization}) computed {native_value}, the interpreter computed \
                 {interpreter_value} for the identical closure — this is exactly the divergence \
                 issue #102 requires this comparison to catch"
            );
        }

        // Core Wasm, via the same raw `emit_resolved_module` + hand-supplied
        // host imports the general per-package loop in
        // `run_examples_and_conformance` uses (the `useful-text-consumer.v1`
        // npm facade this fixture tried first requires an exported function
        // with a borrowed `str` parameter, which a zero-arg `i64` fixture
        // cannot satisfy).
        let module_bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        let wasm_path = scratch.join("discriminator.wasm");
        std::fs::write(&wasm_path, module_bytes).unwrap();
        let script = scratch.join("discriminator-wasm.mjs");
        std::fs::write(
            &script,
            format!(
                r#"const bytes = await (await import("node:fs/promises")).readFile("./{}");
const checked = (operation) => (a, b) => {{ const value = operation(a, b); if (value < -(1n<<63n) || value > (1n<<63n)-1n) throw new RangeError(); return value; }};
const imports = {{env:{{spx_add:checked((a,b)=>a+b),spx_sub:checked((a,b)=>a-b),spx_mul:checked((a,b)=>a*b),spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:(a)=>-a,spx_contract_fail:()=>{{throw new Error();}}}}}};
const linked = await WebAssembly.instantiate(bytes, imports);
console.log(linked.instance.exports.semaprax_main().toString());
"#,
                wasm_path.file_name().unwrap().to_string_lossy()
            ),
        )
        .unwrap();
        let node = Command::new("node")
            .arg(script.file_name().unwrap())
            .current_dir(&scratch)
            .output()
            .unwrap();
        let wasm_value = parse_node_i64(&node);
        assert_eq!(
            wasm_value, interpreter_value,
            "Core Wasm computed {wasm_value}, the interpreter computed {interpreter_value} for \
             the identical closure — this is exactly the divergence issue #102 requires this \
             comparison to catch"
        );

        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}
