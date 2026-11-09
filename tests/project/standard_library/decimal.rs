//! Exact unsigned decimal source arithmetic; no machine-integer operand limit.
use super::*;
use semaprax::project::{ProjectExecutionOptions, ProjectExecutionOutcome};
use std::process::Command;

pub(super) fn run_conformance() {
    let metadata = packages()
        .into_iter()
        .find(|p| p.module == "std.int.decimal")
        .unwrap();
    assert_eq!(metadata.targets, ["interpreter", "native-c11"]);
    let scratch = temporary("decimal");
    project::with_authenticated_project(
        &root().join("std/int-decimal/semaprax.toml"),
        |snapshot| {
            snapshot.check()?;
            let options = ProjectExecutionOptions::default();
            assert_eq!(
                snapshot.execute_entry(&options)?.outcome(),
                &ProjectExecutionOutcome::Returned(0)
            );
            assert_eq!(
                snapshot.execute_test(&options)?.outcome(),
                &ProjectExecutionOutcome::Returned(0)
            );
            for (role, program) in [
                ("examples", snapshot.entry_program()),
                ("tests", snapshot.test_program()),
            ] {
                let c = codegen::emit_hir_c(program).map_err(|error| vec![error])?;
                for optimization in ["-O0", "-O2"] {
                    let binary = scratch.join(format!("decimal-{role}-{optimization}"));
                    compile_c(&c, &binary, optimization);
                    run_returns_zero(&binary);
                }
            }
            Ok(())
        },
    )
    .unwrap();
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
#[cfg_attr(windows, ignore = "native command fixture is Unix-only")]
fn decimal_executes_on_admitted_backends() {
    run_conformance();
}

#[test]
#[cfg_attr(windows, ignore = "native command fixture is Unix-only")]
fn bundled_decimal_consumer_checks_contract_failures_before_arithmetic() {
    let scratch = temporary("decimal-consumer");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        r#"schema = "semaprax.manifest.v1"

[package]
name = "decimal-consumer"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.int.decimal = "=0.1.0"
"#,
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    // First witness success through the ordinary bundled dependency route,
    // then independently pin malformed input, unsigned underflow and zero.
    for (index, body) in [
        "let divisor = \"3\"; if divide(add(\"999999999999999999999999\", \"1\"), string_as_str(divisor)) == \"333333333333333333333333\" { 0 } else { 1 }",
        "string_len(canonicalize(\"\"))",
        "string_len(canonicalize(\"1x\"))",
        "string_len(add(\"01\", \"2\"))",
        "let right = \"2\"; string_len(subtract(\"1\", string_as_str(right)))",
        "let divisor = \"0\"; string_len(divide(\"100\", string_as_str(divisor)))",
        "let divisor = \"-1\"; string_len(divide(\"100\", string_as_str(divisor)))",
        "let divisor = \"2\"; string_len(divide(\"1x\", string_as_str(divisor)))",
    ].into_iter().enumerate() {
        let source = format!(r#"module consumer.app;
use function @id("std.int.decimal.canonicalize") from std.int.decimal as canonicalize;
use function @id("std.int.decimal.add") from std.int.decimal as add;
use function @id("std.int.decimal.subtract") from std.int.decimal as subtract;
use function @id("std.int.decimal.divide") from std.int.decimal as divide;
@id("consumer.main") fn main() -> i64 {{ {body} }}
"#);
        let parsed = semaprax::parse(&source, "consumer.spx").unwrap();
        std::fs::write(scratch.join("src/app.spx"), format::canonical(&parsed)).unwrap();
        project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            assert!(snapshot.workspace_manifest().contains("dependencies/std.int.decimal/0.1.0/decimal.spx"));
            let report = snapshot.execute_entry(&ProjectExecutionOptions::default())?;
            if index == 0 {
                assert_eq!(report.outcome(), &ProjectExecutionOutcome::Returned(0));
            } else {
                let ProjectExecutionOutcome::LanguageFailure(status) = report.outcome() else { panic!("case {index}: {report:?}"); };
                assert_eq!(status.class(), semaprax::conformance::StatusClass::Contract);
                assert_eq!(status.domain_id(), semaprax::conformance::CONTRACT_STATUS_DOMAIN_V1);
                assert_eq!(status.code(), semaprax::conformance::CONTRACT_REQUIRES_FALSE_CODE);
            }
            let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
            for optimization in ["-O0", "-O2"] {
                let binary = scratch.join(format!("decimal-{index}-{optimization}"));
                compile_c(&c, &binary, optimization);
                let output = Command::new(&binary).output().unwrap();
                if index == 0 { assert!(output.status.success()); assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0"); }
                else { assert!(!output.status.success()); assert!(String::from_utf8_lossy(&output.stderr).contains("SEMAPRAX contract failure")); }
            }
            Ok(())
        }).unwrap();
    }
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn decimal_does_not_claim_the_frozen_standalone_string_wasm_profile() {
    let library = include_str!("../../../std/int-decimal/src/decimal.spx");
    let source = format!(
        "{library}\n@id(\"decimal.probe\") fn main() -> i64 {{ let divisor = \"3\"; string_len(divide(\"1000\", string_as_str(divisor))) }}\n"
    );
    let program = semaprax::parse(&source, "decimal-wasm.spx").unwrap();
    let error = wasm::internal_strings::emit_module(
        &program,
        &["decimal.probe".to_owned()],
        wasm::internal_strings::InternalStringOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W111");
}

#[test]
#[cfg_attr(windows, ignore = "native command fixture is Unix-only")]
fn bounded_division_preserves_large_dividends_on_native_o0_o2() {
    let library = include_str!("../../../std/int-decimal/src/decimal.spx");
    let scratch = temporary("decimal-large-division");
    for digits in [33_000, 65_536] {
        for (divisor, expected_digit) in [("1", '9'), ("3", '3')] {
            let dividend = "9".repeat(digits);
            let expected = expected_digit.to_string().repeat(digits);
            let source = format!(
                r#"{library}
@id("decimal.large-division.main")
fn main() -> i64 {{
    let divisor = "{divisor}";
    let quotient = divide("{dividend}", string_as_str(divisor));
    if string_len(quotient) == {digits} && quotient == "{expected}" {{ 0 }} else {{ 1 }}
}}
"#
            );
            let program = semaprax::check(&source, "decimal-large-division.spx").unwrap();
            let canonical = format::canonical(&program);
            let reparsed = semaprax::check(&canonical, "decimal-large-division.spx").unwrap();
            assert_eq!(canonical, format::canonical(&reparsed));
            let c = codegen::emit_c(&reparsed).unwrap();
            for optimization in ["-O0", "-O2"] {
                let binary = scratch.join(format!("decimal-{digits}-{divisor}-{optimization}"));
                compile_c(&c, &binary, optimization);
                run_returns_zero(&binary);
            }
        }
    }
    std::fs::remove_dir_all(scratch).unwrap();
}
