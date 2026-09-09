//! Explicit process callback conformance, including result staging and final settlement.
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"module process.fixture;
permit { process.execute, process.stderr.write, process.stdout.write }
@id("process.main") fn main()->i64{0}
@id("process.byte-is") fn byte_is(data:borrow Slice<u8>,index:usize,expected:u8)->bool {
    match byte_get(data,index) { Option::Some {value}=>value==expected, Option::None {}=>false, }
}
@id("process.run") fn run()->bool uses {process.execute,process.stderr.write,process.stdout.write} {
    let arguments=bytes_zeroed(4usize);
    let input=bytes_zeroed(0usize);
    let argv=bytes_as_slice(arguments);
    let stdin=bytes_as_slice(input);
    let sent=stdout_append(argv);
    let errors=stderr_append(argv);
    let output=process_run(7usize,argv,4usize,stdin,0usize,100usize,2usize,2usize);
    let bytes=bytes_as_slice(output);
    let header=byte_is(bytes,0usize,1u8);
    let exit=byte_is(bytes,8usize,28u8);
    let stdout=byte_is(bytes,32usize,65u8);
    let stderr=byte_is(bytes,33usize,33u8);
    header && exit && stdout && stderr && sent==4usize && errors==4usize
}
"#;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "semaprax-process-carrier-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("src")).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn emit(&self) -> (String, Vec<u8>) {
        for (path, source) in [
            ("src/app.spx", SOURCE),
            (
                "src/tests.spx",
                "module process.tests; @id(\"process.tests.main\") fn main()->i64{0}",
            ),
        ] {
            let parsed = semaprax::parse(source, std::path::Path::new(path)).unwrap();
            std::fs::write(self.0.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        std::fs::write(
            self.0.join("semaprax.toml"),
            r#"schema = "semaprax.manifest.v1"

[package]
name = "process-fixture"
version = "0.1.0"
profile = "process-io.v1"

[modules]
entry = "process.fixture"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["process.tests"]

[exports]
web = []

[command]
function = "process.run"

[capabilities]
required = ["process.execute", "process.stderr.write", "process.stdout.write"]
"#,
        )
        .unwrap();
        semaprax::project::with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            Ok((
                revision.process_c_source()?,
                revision.process_wasm_module()?,
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
fn process_native_callbacks_validate_wire_and_settle_before_publication() {
    if Command::new("clang").arg("--version").output().is_err() {
        assert!(
            std::env::var_os("SPX_REQUIRE_CLANG").is_none(),
            "clang is required by SPX_REQUIRE_CLANG"
        );
        return;
    }
    let fixture = Fixture::new();
    let (source, _) = fixture.emit();
    for optimization in ["-O0", "-O2"] {
        let c = fixture.0.join("process.c");
        let exe = fixture.0.join(format!("process{optimization}"));
        std::fs::write(
            &c,
            format!(
                "{}\n{source}\n{}",
                include_str!("process_alloc_fixture.c"),
                include_str!("process_fixture.c")
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
            "status {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        let hostile = Command::new(&exe).arg("unknown-status").output().unwrap();
        assert!(
            !hostile.status.success(),
            "unknown callback status was accepted"
        );
    }
}
#[test]
fn process_wasm_callbacks_validate_owned_wire_and_sticky_settlement() {
    if cfg!(windows) {
        return;
    }
    if Command::new("node").arg("--version").output().is_err() {
        assert!(
            std::env::var_os("SPX_REQUIRE_NODE").is_none(),
            "Node is required by SPX_REQUIRE_NODE"
        );
        return;
    }
    let fixture = Fixture::new();
    let (_, wasm) = fixture.emit();
    let module = fixture.0.join("process.wasm");
    let script = fixture.0.join("process.mjs");
    std::fs::write(&module, wasm).unwrap();
    std::fs::write(
        &script,
        format!(
            "{}\n{}",
            include_str!("environment_provider_fixture.mjs"),
            include_str!("process_io.mjs")
        ),
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script)
        .arg(&module)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn process_interpreter_matches_callback_wire_failures_and_sticky_settlement() {
    use semaprax::interpreter::CommandEvaluationOutcome;
    use semaprax::process_provider::{
        ProcessFailure as Failure, ProcessOutput, ProcessProvider, ProcessRequest,
        ProcessTermination,
    };
    struct Provider {
        mode: u32,
        runs: usize,
        settlements: usize,
    }
    impl ProcessProvider for Provider {
        fn run(&mut self, request: &ProcessRequest) -> Result<ProcessOutput, Failure> {
            self.runs += 1;
            assert_eq!(
                request,
                &ProcessRequest::from_wire(7, &[0; 4], 4, &[], 0, 100, 2, 2).unwrap()
            );
            if self.mode == 2 || self.mode == 4 {
                return Err(Failure::TimedOut);
            }
            if (10..=16).contains(&self.mode) {
                return Err([
                    Failure::InvalidInput,
                    Failure::AuthorityDenied,
                    Failure::LaunchFailed,
                    Failure::TimedOut,
                    Failure::CapacityExceeded,
                    Failure::IoFailure,
                    Failure::SettlementFailed,
                ][(self.mode - 10) as usize]);
            }
            Ok(ProcessOutput {
                termination: if self.mode == 5 {
                    ProcessTermination::Signalled(0)
                } else {
                    ProcessTermination::Exited(7)
                },
                stdout: if self.mode == 8 {
                    vec![65; 3]
                } else {
                    vec![65]
                },
                stderr: vec![33],
            })
        }
        fn settle(&mut self) -> Result<(), Failure> {
            self.settlements += 1;
            if self.mode == 3 || self.mode == 4 {
                Err(Failure::SettlementFailed)
            } else {
                Ok(())
            }
        }
    }
    let fixture = Fixture::new();
    let _ = fixture.emit();
    semaprax::project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        for _ in 0..2 {
            for (mode, code) in [
                (0, 0),
                (2, 4),
                (3, 7),
                (4, 4),
                (5, 6),
                (8, 5),
                (10, 1),
                (11, 2),
                (12, 3),
                (13, 4),
                (14, 5),
                (15, 6),
                (16, 7),
            ] {
                let mut provider = Provider {
                    mode,
                    runs: 0,
                    settlements: 0,
                };
                let result = snapshot.execute_process_command(
                    &Default::default(),
                    &mut provider,
                    1_000_000,
                )?;
                assert_eq!(provider.runs, 1);
                assert_eq!(provider.settlements, 1);
                if code == 0 {
                    assert!(
                        matches!(
                            result.evaluation.outcome,
                            CommandEvaluationOutcome::ReturnedBool(true)
                        ),
                        "{result:?}"
                    );
                    assert_eq!(result.stdout, vec![0; 4]);
                    assert_eq!(result.stderr, vec![0; 4]);
                } else {
                    let CommandEvaluationOutcome::LanguageFailure(status) =
                        result.evaluation.outcome
                    else {
                        panic!("{result:?}")
                    };
                    assert_eq!(status.domain_id(), "semaprax.process.v1");
                    assert_eq!(status.code(), code);
                    assert!(result.stdout.is_empty() && result.stderr.is_empty());
                }
            }
        }
        Ok(())
    })
    .unwrap();
}
