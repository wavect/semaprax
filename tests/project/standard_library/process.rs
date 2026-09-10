//! Source-authored argv/cursor composition through all three process engines.
use super::*;
use semaprax::hosted_interpreter::HostedEnvironmentCommandInput;
use semaprax::interpreter::CommandEvaluationOutcome;
use semaprax::process_provider::{
    FixtureProcessProvider, FixtureProcessStep, ProcessOutput, ProcessRequest, ProcessTermination,
};

#[test]
fn process_package_executes_all_functions_with_registered_request_shape() {
    if cfg!(windows) {
        return;
    }
    run_conformance();
}

pub(super) fn run_conformance() {
    let directory = temporary("process-package");
    std::fs::create_dir_all(directory.join("src")).unwrap();
    for file in ["process.spx", "examples.spx", "tests.spx"] {
        let source = std::fs::read_to_string(root().join("std/process/src").join(file)).unwrap();
        let parsed = semaprax::parse(&source, file).unwrap();
        assert_eq!(semaprax::format::canonical(&parsed), source);
        std::fs::write(directory.join("src").join(file), source).unwrap();
    }
    let source = std::fs::read_to_string(root().join("std/process/semaprax.toml")).unwrap();
    let manifest = directory.join("semaprax.toml");
    for (command, bundled) in [
        ("std.process.examples.inspect", false),
        ("std.process.tests.conformance", false),
        ("std.process.tests.conformance", true),
    ] {
        let mut selected = source.replace("std.process.examples.inspect", command);
        if bundled {
            selected = selected
                .replace("name = \"std-process\"", "name = \"std-process-consumer\"")
                .replace("\"src/process.spx\", ", "")
                .replace("std.io =", "std.process =");
        }
        std::fs::write(&manifest, selected).unwrap();
        project::with_authenticated_project(&manifest, |snapshot| {
            for _ in 0..2 {
                let argv = [2, 0, 0, 0, 1, 0, 0, 0, 255, 0, 0, 0, 0];
                let request =
                    ProcessRequest::from_wire(7, &argv, argv.len(), b"B", 1, 100, 2, 2).unwrap();
                let output = ProcessOutput {
                    termination: ProcessTermination::Exited(u32::MAX),
                    stdout: b"A".to_vec(),
                    stderr: b"!".to_vec(),
                };
                let mut provider = FixtureProcessProvider::new([FixtureProcessStep {
                    request,
                    response: Ok(output),
                }]);
                let result = snapshot.execute_process_command(
                    &HostedEnvironmentCommandInput::default(),
                    &mut provider,
                    1_000_000,
                )?;
                assert!(
                    matches!(
                        result.evaluation.outcome,
                        CommandEvaluationOutcome::ReturnedBool(true)
                    ),
                    "{command}: {result:?}"
                );
                assert!(result.stdout.is_empty() && result.stderr.is_empty());
                assert_eq!(provider.remaining(), 0);
                assert_eq!(provider.settlements(), 1);
            }
            let revision = snapshot.retain_revision();
            let wasm = directory.join("process.wasm");
            std::fs::write(&wasm, revision.process_wasm_module()?).unwrap();
            let script = directory.join("process.mjs");
            let symbol = format!(
                "spx_data_{}",
                command
                    .bytes()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
            let js = format!(
                "{}\n{}",
                include_str!("../../useful_data/environment_provider_fixture.mjs"),
                include_str!("process_fixture.mjs").replace("COMMAND_SYMBOL", &symbol)
            );
            std::fs::write(&script, js).unwrap();
            let output = Command::new("node")
                .arg(&script)
                .arg(&wasm)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{command}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let c = revision.process_c_source()?;
            for optimization in ["-O0", "-O2"] {
                super::compile_and_run_c(
                    &format!("{c}\n{}", include_str!("process_fixture.c")),
                    &directory,
                    optimization,
                    "",
                );
            }
            Ok(())
        })
        .unwrap();
    }
    std::fs::remove_dir_all(directory).unwrap();
}
