//! Byte assertions compose through ordinary owned Reader values and test codes.
use semaprax::diagnostic::Diagnostic;

use super::*;

#[test]
fn test_bytes_package_and_bundled_consumer_execute_across_engines() {
    run_conformance();
}

pub(super) fn run_conformance() {
    run_source_package("test-bytes", "std.test.bytes", "bytes.spx");
}

#[test]
fn agent_package_and_bundled_consumer_execute_across_engines() {
    run_source_package("agent", "std.agent", "agent.spx");
}

pub(super) fn run_if_supported(package: &PackageMetadata) -> bool {
    let library = match package.module.as_str() {
        "std.agent" => "agent.spx",
        "std.test.bytes" => "bytes.spx",
        _ => return false,
    };
    run_source_package(&package.directory, &package.module, library);
    true
}

pub(super) fn run_source_package(package: &str, module: &str, library: &str) {
    let directory = temporary(&format!("{package}-package"));
    std::fs::create_dir_all(directory.join("src")).unwrap();
    for file in [library, "examples.spx", "tests.spx"] {
        let source =
            std::fs::read_to_string(root().join("std").join(package).join("src").join(file))
                .unwrap();
        let parsed = semaprax::parse(&source, file).unwrap();
        assert_eq!(format::canonical(&parsed), source);
        std::fs::write(directory.join("src").join(file), source).unwrap();
    }
    let manifest_source =
        std::fs::read_to_string(root().join("std").join(package).join("semaprax.toml")).unwrap();
    for bundled in [false, true] {
        let manifest = if bundled {
            let base = manifest_source.split("[dependencies]").next().unwrap();
            format!(
                "{}\n\n[dependencies]\n{module} = \"=0.1.0\"\n",
                base.trim_end().replace(&format!("\"src/{library}\", "), "")
            )
        } else {
            manifest_source.clone()
        };
        std::fs::write(directory.join("semaprax.toml"), manifest).unwrap();
        project::with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            assert!(snapshot.manifest().web_exports().is_empty());
            let options = project::ProjectExecutionOptions::default();
            for _ in 0..2 {
                assert_eq!(snapshot.execute_entry(&options)?.outcome(), &project::ProjectExecutionOutcome::Returned(0));
                assert_eq!(snapshot.execute_test(&options)?.outcome(), &project::ProjectExecutionOutcome::Returned(0));
            }
            for (role, program) in [("entry", snapshot.entry_program()), ("tests", snapshot.test_program())] {
                let c = codegen::emit_hir_c(program).map_err(|d| vec![d])?;
                for optimization in ["-O0", "-O2"] {
                    let binary = directory.join(format!("{role}-{bundled}-{optimization}"));
                    compile_c(&c, &binary, optimization);
                    run_returns_zero(&binary);
                }
            }
            let wasm_path = directory.join("tests.wasm");
            std::fs::write(&wasm_path, snapshot.test_wasm_module()?).unwrap();
            let script = directory.join("tests.mjs");
            std::fs::write(&script, format!("{}\n{}", include_str!("../../useful_data/environment_provider_fixture.mjs"), r#"
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const module = new WebAssembly.Module(readFileSync(process.argv[2]));
const provider = environmentProvider(module);
const instance = new WebAssembly.Instance(module, provider.imports);
provider.attach(instance);
for(let run=0;run<4;run++) { assert.equal(instance.exports.semaprax_main(), 0n); provider.settled(); }
"#)).unwrap();
            let output = Command::new("node").arg(&script).arg(&wasm_path).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            Ok(())
        }).unwrap();
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn test_bytes_invalid_cursors_and_failure_bits_select_contract_failure() {
    let directory = temporary("test-bytes-contracts");
    std::fs::create_dir_all(directory.join("src")).unwrap();
    std::fs::write(
        directory.join("semaprax.toml"),
        r#"schema = "semaprax.manifest.v1"

[package]
name = "byte-test-consumer"
version = "0.1.0"
profile = "useful-data.v2"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.test.bytes = "=0.1.0"
"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("src/app.spx"),
        "module consumer.app;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    for (index, body) in [
        "let left = Reader {data: bytes_zeroed(1usize), position: 2usize}; let right = Reader {data: bytes_zeroed(0usize), position: 0usize}; if equal_remaining(left, right) {0} else {1}",
        "let left = Reader {data: bytes_zeroed(0usize), position: 0usize}; let right = Reader {data: bytes_zeroed(1usize), position: 2usize}; if equal_remaining(left, right) {0} else {1}",
        "let left = Reader {data: bytes_zeroed(1usize), position: 2usize}; let right = Reader {data: bytes_zeroed(0usize), position: 0usize}; failure_bit_equal_remaining(left, right, 4)",
        "let data = [255u8]; failure_bit_equal_bytes(array_as_slice(data), array_as_slice(data), 0)",
        "let left = Reader {data: bytes_zeroed(0usize), position: 0usize}; let right = Reader {data: bytes_zeroed(0usize), position: 0usize}; failure_bit_equal_remaining(left, right, -1)",
    ].into_iter().enumerate() {
        let source = format!("module consumer.tests;\nuse type @id(\"std.io.reader\") from std.io as Reader;\nuse function @id(\"std.test.bytes.equal-remaining\") from std.test.bytes as equal_remaining;\nuse function @id(\"std.test.bytes.failure-bit-equal\") from std.test.bytes as failure_bit_equal_bytes;\nuse function @id(\"std.test.bytes.failure-bit-equal-remaining\") from std.test.bytes as failure_bit_equal_remaining;\n@id(\"consumer.tests.main\") fn main()->i64 {{ {body} }}");
        let parsed = semaprax::parse(&source, "byte-test-contract.spx").unwrap();
        std::fs::write(directory.join("src/tests.spx"), format::canonical(&parsed)).unwrap();
        project::with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
            assert_contract_failure(snapshot, &directory, index)
        }).unwrap();
    }
    std::fs::remove_dir_all(directory).unwrap();
}

fn assert_contract_failure(
    snapshot: &project::ProjectSnapshot,
    directory: &Path,
    index: usize,
) -> Result<(), Vec<Diagnostic>> {
    for _ in 0..2 {
        let report = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
        let project::ProjectExecutionOutcome::LanguageFailure(status) = report.outcome() else {
            panic!("case {index}: {report:?}")
        };
        assert_eq!(status.class(), semaprax::conformance::StatusClass::Contract);
        assert_eq!(
            status.domain_id(),
            semaprax::conformance::CONTRACT_STATUS_DOMAIN_V1
        );
        assert_eq!(
            status.code(),
            semaprax::conformance::CONTRACT_REQUIRES_FALSE_CODE
        );
    }
    let c = codegen::emit_hir_c(snapshot.test_program()).map_err(|d| vec![d])?;
    for optimization in ["-O0", "-O2"] {
        let binary = directory.join(format!("failure-{index}-{optimization}"));
        compile_c(&c, &binary, optimization);
        let output = Command::new(binary).output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("SEMAPRAX contract failure"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let wasm = directory.join("failure.wasm");
    std::fs::write(&wasm, snapshot.test_wasm_module()?).unwrap();
    let script = directory.join("failure.mjs");
    std::fs::write(&script, format!("{}\n{}", include_str!("../../useful_data/environment_provider_fixture.mjs"), r#"
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const module = new WebAssembly.Module(readFileSync(process.argv[2]));
const provider = environmentProvider(module);
const failure = new Error('contract requires false');
provider.imports.env.spx_contract_fail = selector => { assert.equal(selector, 9); throw failure; };
const instance = new WebAssembly.Instance(module, provider.imports);
provider.attach(instance);
for(let run=0;run<4;run++) { assert.throws(()=>instance.exports.semaprax_main(), error=>error===failure); provider.settled(); }
"#)).unwrap();
    let output = Command::new("node").arg(script).arg(wasm).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn agent_advance_rejects_invalid_epoch() {
    let directory = temporary("agent-advance-contracts");
    std::fs::create_dir_all(directory.join("src")).unwrap();
    std::fs::write(
        directory.join("semaprax.toml"),
        r#"schema = "semaprax.manifest.v1"

[package]
name = "agent-advance-consumer"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.agent = "=0.1.0"
"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("src/app.spx"),
        "module consumer.app;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    for (index, epoch) in ["-1", "9223372036854775807"].into_iter().enumerate() {
        let source = format!("module consumer.tests;\nuse type @id(\"std.agent.context\") from std.agent as Context;\nuse function @id(\"std.agent.advance\") from std.agent as advance;\n@id(\"consumer.tests.main\") fn main() -> i64 {{ let context = Context {{ objective: bytes_zeroed(0usize), budget: 0, epoch: {epoch} }}; let next = advance(context); next.epoch }}");
        let parsed = semaprax::parse(&source, "agent-advance-contract.spx").unwrap();
        std::fs::write(directory.join("src/tests.spx"), format::canonical(&parsed)).unwrap();
        project::with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
            assert_contract_failure(snapshot, &directory, index)
        })
        .unwrap();
    }
    std::fs::remove_dir_all(directory).unwrap();
}
