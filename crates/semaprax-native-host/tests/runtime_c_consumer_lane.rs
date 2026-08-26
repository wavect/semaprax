#![cfg(any(target_os = "linux", target_os = "macos"))]

//! Plain-C dynamic consumer lane: the same generated callable-v3 corpus
//! providers execute through a hand-written strict C consumer that dlopens
//! each provider and drives the descriptor getter + execute + settle wire
//! sequence directly, with no Rust host in the loop.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const GENERATOR: &str = env!("CARGO_BIN_EXE_private-c-consumer-v1-fixture");
const SCENARIOS: [&str; 3] = ["discard-two", "requires-false", "identity-max"];

fn expected_lines() -> [String; 3] {
    [
        "SEMAPRAX_C_CONSUMER_V1_OK case=discard-two outcome=scalar:0 payload=0 publication=no-owned finalizers=1:13,0:11".to_owned(),
        format!(
            "SEMAPRAX_C_CONSUMER_V1_OK case=requires-false outcome=failure:{selected} payload=0 publication=no-owned finalizers=0:18446744073709551615",
            selected = requires_false_selected_ordinal(),
        ),
        "SEMAPRAX_C_CONSUMER_V1_OK case=identity-max outcome=owned:0 payload=18446744073709551615 publication=owned finalizers=none".to_owned(),
    ]
}

fn requires_false_selected_ordinal() -> u32 {
    use semaprax::conformance::TraceEventKind;
    use semaprax::hir::DeclarationId;
    use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;
    use semaprax::semantic_trace::build_semantic_event_dictionary;
    let corpus = build_owned_resource_corpus_v1().expect("build owned corpus");
    let case = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "requires-false")
        .expect("requires-false corpus case");
    let dictionary =
        build_semantic_event_dictionary(&corpus.program, &DeclarationId::new(case.function_id))
            .expect("requires-false dictionary");
    case.reference
        .events
        .iter()
        .find_map(|event| {
            matches!(event.event, TraceEventKind::SelectFailure { .. })
                .then(|| dictionary.ordinal_for(&event.event))
                .flatten()
        })
        .expect("requires-false selected status")
}

struct Lane {
    directory: PathBuf,
}

impl Lane {
    fn new(optimization: &str) -> Self {
        let ordinal = NEXT_LANE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-c-consumer-v1-{}-{ordinal}",
            std::process::id()
        ));
        let output = Command::new(GENERATOR)
            .arg(&directory)
            .output()
            .expect("run private c-consumer fixture generator");
        assert!(
            output.status.success(),
            "generator failed at {optimization}:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(
            directory.is_dir(),
            "generator did not materialize the lane directory"
        );
        Self { directory }
    }

    fn compile_provider(&self, scenario: &str, optimization: &str) -> PathBuf {
        let library = library_name(scenario);
        let output = Command::new(compiler())
            .args(shared_flags())
            .args([
                optimization,
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-pedantic",
            ])
            .arg(self.directory.join(format!("provider-{scenario}.c")))
            .arg("-o")
            .arg(self.directory.join(&library))
            .output()
            .expect("compile generated v3 provider for the plain-C consumer");
        assert!(
            output.status.success(),
            "{scenario} provider compile failed at {optimization}:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        self.directory.join(library)
    }

    fn compile_consumer(&self, optimization: &str) -> PathBuf {
        let executable = self.directory.join(consumer_executable_name());
        let mut command = Command::new(compiler());
        command.args([
            optimization,
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pedantic",
        ]);
        if cfg!(target_os = "linux") {
            command.arg("-ldl");
        }
        let output = command
            .arg(self.directory.join("consumer.c"))
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("compile the strict C consumer");
        assert!(
            output.status.success(),
            "consumer compile failed at {optimization}:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        executable
    }

    fn run_consumer(
        &self,
        executable: &Path,
        scenario: &str,
        expected_line: &str,
        optimization: &str,
    ) {
        let output = Command::new(executable)
            .arg(self.directory.join(format!("manifest-{scenario}.txt")))
            .arg(scenario)
            .output()
            .expect("run the strict C consumer");
        assert!(
            output.status.success(),
            "{scenario} consumer run failed at {optimization} (status {:?}):\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("{expected_line}\n"),
            "{scenario} outcome line diverged at {optimization}"
        );
    }
}

impl Drop for Lane {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

static NEXT_LANE: AtomicU64 = AtomicU64::new(1);

fn compiler() -> std::ffi::OsString {
    std::env::var_os("CC").unwrap_or_else(|| "cc".into())
}

fn shared_flags() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["-dynamiclib", "-fPIC"]
    } else {
        &["-shared", "-fPIC"]
    }
}

fn library_name(scenario: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("libsemaprax-c-consumer-{scenario}.dylib")
    } else {
        format!("libsemaprax-c-consumer-{scenario}.so")
    }
}

fn consumer_executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "consumer.exe"
    } else {
        "consumer"
    }
}

#[test]
fn plain_c_consumer_drives_generated_callable_v3_providers_at_o0_and_o2() {
    let expected = expected_lines();
    for optimization in ["-O0", "-O2"] {
        let lane = Lane::new(optimization);
        let consumer = lane.compile_consumer(optimization);
        for (index, scenario) in SCENARIOS.iter().enumerate() {
            lane.compile_provider(scenario, optimization);
            lane.run_consumer(&consumer, scenario, &expected[index], optimization);
        }
    }
}
