//! Exact injected environment carriers across C11 and Core-Wasm.
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
const BODY: &str = "let count=env_len(); let name=env_name_utf8(0usize); let value=env_value_utf8(1usize); count==2usize && str_len_bytes(name)==1 && str_len_bytes(value)==2";
fn effects(body: &str) -> &'static str {
    if body.contains("stdout_append") || body.contains("stderr_append") {
        "process.environment.read, process.stderr.write, process.stdout.write"
    } else {
        "process.environment.read"
    }
}
fn source(body: &str) -> String {
    let effects = effects(body);
    format!("module environment.fixture;\npermit {{{effects}}}\n@id(\"environment.run\") fn run()->bool uses {{{effects}}} {{{body}}}\n@id(\"environment.main\") fn main()->i64 {{0}}\n")
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "semaprax-environment-carrier-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("src")).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn emit(&self, body: &str) -> (String, Vec<u8>) {
        let parsed =
            semaprax::parse(&source(body), std::path::Path::new("environment.spx")).unwrap();
        std::fs::write(
            self.0.join("src/app.spx"),
            semaprax::format::canonical(&parsed),
        )
        .unwrap();
        let tests = semaprax::parse(
            "module environment.tests; @id(\"environment.tests.main\") fn main()->i64{0}",
            std::path::Path::new("environment-tests.spx"),
        )
        .unwrap();
        std::fs::write(
            self.0.join("src/tests.spx"),
            semaprax::format::canonical(&tests),
        )
        .unwrap();
        std::fs::write(
            self.0.join("semaprax.toml"),
            r#"schema = "semaprax.manifest.v1"

[package]
name = "environment-fixture"
version = "0.1.0"
profile = "environment-io.v1"

[modules]
entry = "environment.fixture"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["environment.tests"]

[exports]
web = []

[command]
function = "environment.run"

[capabilities]
required = ["process.environment.read"]
"#
            .replace(
                "required = [\"process.environment.read\"]",
                &format!(
                    "required = [{}]",
                    effects(body)
                        .split(", ")
                        .map(|effect| format!("\"{effect}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ),
        )
        .unwrap();
        semaprax::project::with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok((
                snapshot.retain_revision().environment_c_source()?,
                snapshot.retain_revision().environment_wasm_module()?,
            ))
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn environment_native_snapshots_none_empty_unicode_and_invalid_are_distinct() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let fixture = Fixture::new();
    let (generated, _) = fixture.emit(BODY);
    let harness = r#"
static int invoke(const struct spx_environment_snapshot_v1 *env, int success, uint32_t code) {
    struct spx_language_command_input_v1 input={0};
    struct spx_language_command_result_v1 result;
    memset(&result, 0x5a, sizeof(result));
    if (spx_environment_command_run_v1(&input, env, &result)!=1) return 1;
    if (result.semantic_success != (success!=0) || result.stdout_length!=0 || result.stderr_length!=0) return 2;
    if (success) return !result.matched;
    return result.status_code!=code || strcmp(result.status_domain,"semaprax.environment-input.v1")!=0;
}
int main(void) {
    struct spx_environment_snapshot_v1 env={0};
    if (invoke(NULL,0,4) || invoke(&env,0,1)) return 1;
    fixture_environment(&env);
    for (int i=0;i<3;++i) if(invoke(&env,1,0)) return 2;
    struct spx_language_command_input_v1 input={0};
    struct spx_language_command_result_v1 result;
    for (int bad=0;bad<4;++bad) {
        env.entries[1].name=(spx_str_v1){.data=(const uint8_t*)"B",.len=1};
        env.entries[1].value=(spx_str_v1){.data=(const uint8_t*)"ok",.len=2};
        if (bad==0) env.entries[1].name=(spx_str_v1){.data=(const uint8_t*)"A",.len=1};
        if (bad==1) env.entries[1].name=(spx_str_v1){.data=(const uint8_t*)"=",.len=1};
        if (bad==2) env.entries[1].value=(spx_str_v1){.data=(const uint8_t*)"\xc0\x80",.len=2};
        if (bad==3) env.entries[1].value=(spx_str_v1){.data=NULL,.len=1};
        memset(&result,0x5a,sizeof(result));
        if(spx_environment_command_run_v1(&input,&env,&result)!=0 || result.semantic_success || result.stdout_length || result.stderr_length) return 3;
    }
    fixture_environment(&env);
    static uint8_t payload[65536];
    memset(payload,65,sizeof(payload));
    input.stdin_snapshot=(spx_slice_u8_v1){.ptr=payload,.len=65527};
    if(spx_environment_command_run_v1(&input,&env,&result)!=1 || !result.semantic_success || !result.matched) return 4;
    input.stdin_snapshot.len=65528;
    if(spx_environment_command_run_v1(&input,&env,&result)!=0 || result.semantic_success || result.stdout_length || result.stderr_length) return 5;
    input.stdin_snapshot.len=0;
    env.count=257;
    if(spx_environment_command_run_v1(&input,&env,&result)!=0) return 6;
    return 0;
}
"#;
    for optimization in ["-O0", "-O2"] {
        let c = fixture.0.join("fixture.c");
        let exe = fixture.0.join(format!("fixture{optimization}"));
        std::fs::write(
            &c,
            format!(
                "{generated}\n{}\n{harness}",
                include_str!("environment_provider_fixture.c")
            ),
        )
        .unwrap();
        let output = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&c)
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(&exe).output().unwrap();
        assert!(
            output.status.success(),
            "{:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn environment_wasm_success_status_and_hostile_view_results_are_checked() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let fixture = Fixture::new();
    for (label, body) in [("lookup", BODY), ("empty", "env_len()==0usize"),
        ("emptyvalue", "let count=env_len(); let value=env_value_utf8(0usize); count==1usize && str_is_empty(value)"),
        ("boundary", "let count=env_len(); let value=env_value_utf8(0usize); count==1usize && str_len_bytes(value)==65535")] {
        let (_, wasm) = fixture.emit(body);
        let binary = fixture.0.join(format!("{label}.wasm"));
        std::fs::write(&binary, wasm).unwrap();
        let script = fixture.0.join("fixture.mjs");
        std::fs::write(&script, include_str!("environment_io.mjs")).unwrap();
        let output = Command::new("node")
            .arg(&script)
            .arg(&binary)
            .arg(label)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "environment carriers verified\n"
        );
    }
}

#[test]
fn environment_interpreter_combined_input_exact_and_first_over() {
    use semaprax::environment_snapshot::EnvironmentSnapshot;
    use semaprax::hosted_interpreter::{
        execute_environment_command, HostedCommandInput, HostedEnvironmentCommandInput,
    };
    use semaprax::interpreter::CommandEvaluationOutcome;
    let source = source(BODY);
    let parsed =
        semaprax::parse(&source, std::path::Path::new("environment-boundary.spx")).unwrap();
    let program = semaprax::hir::resolve(&parsed).unwrap();
    let environment = EnvironmentSnapshot::from_entries(vec![
        ("A".to_owned(), "alpha".to_owned()),
        ("Z".to_owned(), "é".to_owned()),
    ])
    .unwrap();
    for _ in 0..2 {
        let mut input = HostedEnvironmentCommandInput {
            command: HostedCommandInput {
                arguments: Vec::new(),
                stdin: vec![65; 65527],
            },
            environment: Some(environment.clone()),
        };
        let result =
            execute_environment_command(&program, "environment.run", &input, 50_000).unwrap();
        assert_eq!(
            result.evaluation.outcome,
            CommandEvaluationOutcome::ReturnedBool(true)
        );
        assert!(result.stdout.is_empty() && result.stderr.is_empty());
        input.command.stdin.push(65);
        assert!(execute_environment_command(&program, "environment.run", &input, 50_000).is_err());
    }
}

#[test]
fn environment_append_transcripts_publish_only_after_complete_success() {
    use semaprax::environment_snapshot::EnvironmentSnapshot;
    use semaprax::hosted_interpreter::{
        execute_environment_command, HostedEnvironmentCommandInput,
    };
    use semaprax::interpreter::CommandEvaluationOutcome;
    let fixture = Fixture::new();
    let prefix="let name=env_name_utf8(0usize); let bytes=str_as_bytes(name); let first=stdout_append(bytes); let second=stderr_append(bytes);";
    for fail in [false, true] {
        let body = format!(
            "{prefix} {}",
            if fail {
                "let missing=env_value_utf8(99usize); str_is_empty(missing)"
            } else {
                "first==1usize && second==1usize"
            }
        );
        let (c, wasm) = fixture.emit(&body);
        let parsed = semaprax::parse(
            &source(&body),
            std::path::Path::new("environment-append.spx"),
        )
        .unwrap();
        let program = semaprax::hir::resolve(&parsed).unwrap();
        let input = HostedEnvironmentCommandInput {
            environment: Some(
                EnvironmentSnapshot::from_entries(vec![
                    ("A".to_owned(), "alpha".to_owned()),
                    ("Z".to_owned(), "é".to_owned()),
                ])
                .unwrap(),
            ),
            ..Default::default()
        };
        for _ in 0..3 {
            let result =
                execute_environment_command(&program, "environment.run", &input, 50_000).unwrap();
            if fail {
                let CommandEvaluationOutcome::LanguageFailure(status) = result.evaluation.outcome
                else {
                    panic!("failure expected")
                };
                assert_eq!(status.domain_id(), "semaprax.environment-input.v1");
                assert_eq!(status.code(), 1);
            } else {
                assert_eq!(
                    result.evaluation.outcome,
                    CommandEvaluationOutcome::ReturnedBool(true)
                );
            }
            assert_eq!(result.stdout, if fail { vec![] } else { vec![65] });
            assert_eq!(result.stderr, if fail { vec![] } else { vec![65] });
        }
        if Command::new("clang").arg("--version").output().is_ok() {
            let harness = format!(
                r#"
int main(void) {{
    struct spx_environment_snapshot_v1 environment;fixture_environment(&environment);
    struct spx_language_command_input_v1 input={{0}};
    for(int i=0;i<3;++i) {{
        struct spx_language_command_result_v1 result;
        if(spx_environment_command_run_v1(&input,&environment,&result)!=1) return 1;
        if({fail}) {{
            if(result.semantic_success || result.status_code!=1 || strcmp(result.status_domain,"semaprax.environment-input.v1")!=0 || result.stdout_length || result.stderr_length) return 2;
        }} else if(!result.semantic_success || !result.matched || result.stdout_length!=1 || result.stderr_length!=1 || result.stdout_bytes[0]!=65 || result.stderr_bytes[0]!=65) return 3;
    }} return 0;
}}
"#,
                fail = u8::from(fail)
            );
            let file = fixture.0.join("append.c");
            std::fs::write(
                &file,
                format!(
                    "{c}\n{}\n{harness}",
                    include_str!("environment_provider_fixture.c")
                ),
            )
            .unwrap();
            for optimization in ["-O0", "-O2"] {
                let exe = fixture.0.join(format!("append{optimization}"));
                let output = Command::new("clang")
                    .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
                    .arg(&file)
                    .arg("-o")
                    .arg(&exe)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(Command::new(&exe).status().unwrap().success());
            }
        }
        if Command::new("node").arg("--version").output().is_ok() {
            let file = fixture.0.join("append.wasm");
            std::fs::write(&file, wasm).unwrap();
            let script = fixture.0.join("append.mjs");
            std::fs::write(
                &script,
                format!(
                    "{}\n{}",
                    include_str!("environment_provider_fixture.mjs"),
                    include_str!("environment_append.mjs")
                ),
            )
            .unwrap();
            let output = Command::new("node")
                .arg(&script)
                .arg(&file)
                .arg(if fail { "failure" } else { "success" })
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
fn environment_production_provider_constructs_one_checked_combined_snapshot() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let fixture = Fixture::new();
    let script = fixture.0.join("provider-constructor.mjs");
    std::fs::write(
        &script,
        format!(
            "{}\n{}",
            include_str!("../../src/wasm/environment_provider.mjs"),
            include_str!("environment_provider_constructor.mjs")
        ),
    )
    .unwrap();
    let output = Command::new("node").arg(&script).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "environment provider constructor verified\n"
    );
}
