//! Injected environment package composition, separate from public ABI exports.
use super::{project, root, temporary};
use semaprax::environment_snapshot::EnvironmentSnapshot;
use semaprax::hosted_interpreter::HostedEnvironmentCommandInput;
use semaprax::interpreter::CommandEvaluationOutcome;
use semaprax::project::{ProjectManifest, ProjectProfile};

#[test]
fn environment_manifest_is_canonical_and_authority_is_closed() {
    let source = std::fs::read_to_string(root().join("std/env/semaprax.toml")).unwrap();
    let manifest = ProjectManifest::parse(&source).unwrap();
    assert_eq!(manifest.project_profile(), ProjectProfile::EnvironmentIoV1);
    assert_eq!(manifest.schema(), "semaprax.project.v17");
    assert!(manifest.web_exports().is_empty());
    assert_eq!(manifest.to_canonical_toml(), source);
    for hostile in [
        source.replace("required = [\"process.environment.read\"]", "required = []"),
        source.replace("process.environment.read", "filesystem.read"),
        source.replace(
            "process.environment.read",
            "process.environment.read\", \"process.environment.read",
        ),
        source.replace("web = []", "web = [\"std.env.examples.inspect\"]"),
        source.replace("environment-io.v1", "filesystem-io.v1"),
    ] {
        assert!(ProjectManifest::parse(&hostile).is_err(), "{hostile}");
    }
}

#[test]
fn environment_package_executes_all_functions_with_injected_snapshot() {
    run_conformance();
}

pub(super) fn run_conformance() {
    let directory = temporary("environment-package");
    std::fs::create_dir_all(directory.join("src")).unwrap();
    for file in ["env.spx", "examples.spx", "tests.spx"] {
        let source = std::fs::read_to_string(root().join("std/env/src").join(file)).unwrap();
        let parsed = semaprax::parse(&source, file).unwrap();
        assert_eq!(semaprax::format::canonical(&parsed), source);
        std::fs::write(directory.join("src").join(file), source).unwrap();
    }
    let source = std::fs::read_to_string(root().join("std/env/semaprax.toml")).unwrap();
    let manifest = directory.join("semaprax.toml");
    for (command, bundled) in [
        ("std.env.examples.inspect", false),
        ("std.env.tests.conformance", false),
        ("std.env.tests.conformance", true),
    ] {
        let mut selected = source.replace("std.env.examples.inspect", command);
        if bundled {
            selected = selected
                .replace("name = \"std-env\"", "name = \"std-env-consumer\"")
                .replace("\"src/env.spx\", ", "")
                .replace("std.format =", "std.env =");
        }
        std::fs::write(&manifest, selected).unwrap();
        project::with_authenticated_project(&manifest, |snapshot| {
            let input = HostedEnvironmentCommandInput {
                environment: Some(
                    EnvironmentSnapshot::from_entries(vec![
                        ("Z".to_owned(), "é".to_owned()),
                        ("A".to_owned(), "alpha".to_owned()),
                    ])
                    .unwrap(),
                ),
                ..Default::default()
            };
            for _ in 0..2 {
                let result = snapshot.execute_environment_command(&input, 1_000_000)?;
                assert!(
                    matches!(
                        result.evaluation.outcome,
                        CommandEvaluationOutcome::ReturnedBool(true)
                    ),
                    "{command}: {result:?}"
                );
                assert!(result.stdout.is_empty() && result.stderr.is_empty());
            }
            let absent = snapshot.execute_environment_command(
                &HostedEnvironmentCommandInput::default(),
                1_000_000,
            )?;
            let CommandEvaluationOutcome::LanguageFailure(status) = absent.evaluation.outcome
            else {
                panic!("{absent:?}")
            };
            assert_eq!(status.domain_id(), "semaprax.environment-input.v1");
            assert_eq!(status.code(), 4);
            let revision = snapshot.retain_revision();
            let c = revision.environment_c_source()?;
            assert!(c.contains("spx_environment_command_run_v1"));
            let provider = include_str!("../../useful_data/environment_provider_fixture.c");
            let harness = r#"
int main(void) {
    struct spx_environment_snapshot_v1 environment;
    fixture_environment(&environment);
    struct spx_language_command_input_v1 input={0};
    for (unsigned i=0;i<3;++i) {
        struct spx_language_command_result_v1 result;
        if (spx_environment_command_run_v1(&input,&environment,&result)!=1 || !result.semantic_success || !result.matched || result.stdout_length || result.stderr_length) return 1;
    }
    return 0;
}
"#;
            for optimization in ["-O0", "-O2"] {
                super::compile_and_run_c(&format!("{c}\n{provider}\n{harness}"), &directory, optimization, "");
            }
            let wasm = directory.join("environment.wasm");
            std::fs::write(&wasm, revision.environment_wasm_module()?).unwrap();
            let script = directory.join("environment.mjs");
            let symbol = format!("spx_data_{}", command.bytes().map(|b|format!("{b:02x}")).collect::<String>());
            let js = format!(r#"
import {{readFileSync}} from 'node:fs';
{}
{}
const module=new WebAssembly.Module(readFileSync(process.argv[2]));
const provider=environmentProvider(module);
const snapshot=createEnvironmentProvider({{environment:[["Z","é"],["A","alpha"]]}});
Object.assign(provider.imports.env,snapshot.imports);
const instance=new WebAssembly.Instance(module,provider.imports);provider.attach(instance);
for(let i=0;i<3;i++){{
  snapshot.attach(instance.exports.memory??instance.exports.__spx_byte_memory);
  const result=instance.exports['{symbol}']();
  if(result!==1)throw Error('environment result '+result);
  provider.settled();
}}
"#, revision.environment_provider_source()?, include_str!("../../useful_data/environment_provider_fixture.mjs"));
            std::fs::write(&script,js).unwrap();
            let output=std::process::Command::new("node").arg(&script).arg(&wasm).output().unwrap();
            assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
            Ok(())
        })
        .unwrap();
    }
    std::fs::remove_dir_all(directory).unwrap();
}
