#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::codegen::{emit_native_callable_admission, NativeCallableAdmissionArtifact};
use semaprax::conformance::{NormalizedStatus, TraceEvent, TraceOutcome, TraceResult};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::{
    build_owned_resource_corpus_v1, OwnedResourceCorpusArgument, OwnedResourceCorpusCase,
};
use semaprax::semantic_trace::OWNED_RESOURCE_CORPUS_V1_SCENARIOS;
use semaprax_native_host::{
    NativeCallableExecution, NativeCallableHost, NativeOwner, RejectedCall, ResultKind, ScalarValue,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);
const REQUIRED_SANITIZERS_ENV: &str = "SEMAPRAX_REQUIRE_CALLABLE_HOST_SANITIZERS";

fn sanitizers_required() -> bool {
    match std::env::var(REQUIRED_SANITIZERS_ENV) {
        Err(std::env::VarError::NotPresent) => false,
        Ok(value) if value == "1" => {
            #[cfg(target_os = "linux")]
            {
                true
            }
            #[cfg(not(target_os = "linux"))]
            {
                panic!("{REQUIRED_SANITIZERS_ENV}=1 requires the audited Linux Clang host lane")
            }
        }
        Ok(value) => {
            panic!("{REQUIRED_SANITIZERS_ENV} must be unset or exactly `1`, received `{value}`")
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("{REQUIRED_SANITIZERS_ENV} is not valid Unicode")
        }
    }
}

fn assert_required_sanitizer_environment() {
    if !sanitizers_required() {
        return;
    }
    let clang = Command::new("clang")
        .arg("--version")
        .output()
        .expect("mandatory callable-host sanitizers require Clang");
    assert!(
        clang.status.success(),
        "mandatory callable-host sanitizer Clang probe failed:\n{}",
        String::from_utf8_lossy(&clang.stderr)
    );
    for (name, required) in [
        ("ASAN_OPTIONS", ["halt_on_error=1", "detect_leaks=0"]),
        ("UBSAN_OPTIONS", ["halt_on_error=1", "print_stacktrace=1"]),
    ] {
        let value = std::env::var(name)
            .unwrap_or_else(|_| panic!("mandatory callable-host sanitizers require {name}"));
        for option in required {
            assert!(
                value.split(':').any(|candidate| candidate == option),
                "mandatory callable-host sanitizer option {name} must contain `{option}`"
            );
        }
    }
}

fn assert_sanitizer_instrumentation(library: &Path) {
    if !sanitizers_required() {
        return;
    }
    let symbols = Command::new("nm")
        .arg("-u")
        .arg(library)
        .output()
        .expect("mandatory callable-host sanitizers require `nm`");
    assert!(
        symbols.status.success(),
        "could not inspect mandatory sanitized provider symbols:\n{}",
        String::from_utf8_lossy(&symbols.stderr)
    );
    let mut output = symbols.stdout;
    output.extend_from_slice(&symbols.stderr);
    let output = String::from_utf8_lossy(&output);
    assert!(
        output.contains("__asan_"),
        "callable provider lacks required ASan instrumentation:\n{output}"
    );
    assert!(
        output.contains("__ubsan_"),
        "callable provider lacks required UBSan instrumentation:\n{output}"
    );
    // Clang does not link its ASan executable runtime into a shared object.
    // These unresolved callbacks plus the later successful `dlopen` prove
    // that the sanitizer-linked Rust test process supplies the runtime used by
    // the actual provider image; a compile-only shared-library probe cannot.
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Optimization {
    O0,
    O2,
}

impl Optimization {
    const ALL: [Self; 2] = [Self::O0, Self::O2];

    const fn flag(self) -> &'static str {
        match self {
            Self::O0 => "-O0",
            Self::O2 => "-O2",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::O0 => "o0",
            Self::O2 => "o2",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ObservedOutcome {
    Scalar(i64),
    Owned,
    Failure(NormalizedStatus),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Observation {
    outcome: ObservedOutcome,
    events: Vec<TraceEvent>,
    live_after_execution: usize,
    rotated_owner_reused: bool,
    final_live_owners: usize,
}

struct Fixture {
    directory: PathBuf,
    library: PathBuf,
    descriptor: Vec<u8>,
    getter_symbol: String,
    callable_symbol: String,
    dictionary: semaprax::semantic_trace::SemanticEventDictionary,
    trace_path_certificate: semaprax::trace_path_certificate::TracePathCertificate,
}

impl Fixture {
    fn compile(artifact: &NativeCallableAdmissionArtifact, optimization: Optimization) -> Self {
        let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-callable-corpus-{}-{}-{ordinal}",
            std::process::id(),
            optimization.label(),
        ));
        fs::create_dir(&directory).expect("create isolated callable-corpus directory");
        let directory = fs::canonicalize(directory).expect("canonical callable-corpus directory");
        let source = directory.join("provider.c");
        let library = directory.join(library_filename());
        fs::write(&source, artifact.provider_source())
            .expect("write compiler-generated callable provider");

        let required_sanitizers = sanitizers_required();
        let compiler_name = if required_sanitizers {
            "clang".into()
        } else {
            std::env::var_os("CC").unwrap_or_else(|| {
                if cfg!(windows) {
                    "clang".into()
                } else {
                    "cc".into()
                }
            })
        };
        let mut compiler = Command::new(compiler_name);
        #[cfg(target_os = "macos")]
        compiler.args(["-dynamiclib", "-fPIC", "-fvisibility=hidden"]);
        #[cfg(target_os = "linux")]
        compiler.args(["-shared", "-fPIC", "-fvisibility=hidden"]);
        #[cfg(target_os = "windows")]
        compiler.arg("-shared");
        if required_sanitizers {
            compiler.args([
                "-fsanitize=address,undefined",
                "-fno-omit-frame-pointer",
                "-fno-sanitize-recover=all",
            ]);
        }
        let output = compiler
            .args([
                optimization.flag(),
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .output()
            .expect("compile generated callable-corpus provider");
        assert!(
            output.status.success(),
            "generated callable-corpus provider failed at {:?}:\nstdout:\n{}\nstderr:\n{}",
            optimization,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let library = fs::canonicalize(library).expect("canonical callable-corpus library");
        assert_sanitizer_instrumentation(&library);
        Self {
            directory,
            library,
            descriptor: artifact.descriptor().to_vec(),
            getter_symbol: artifact.getter_symbol().to_owned(),
            callable_symbol: artifact.callable_symbol().to_owned(),
            dictionary: artifact.semantic_event_dictionary().clone(),
            trace_path_certificate: artifact.trace_path_certificate().clone(),
        }
    }

    unsafe fn open(&self) -> NativeCallableHost {
        // SAFETY: The fixture compiles the exact compiler-generated complete
        // provider in a private canonical directory. It exports only the bound
        // immutable descriptor getter and synchronous no-escape byte callable.
        unsafe {
            NativeCallableHost::open_admitted_callable_exact(
                &self.library,
                self.getter_symbol.as_bytes(),
                self.callable_symbol.as_bytes(),
                &self.descriptor,
                self.dictionary.clone(),
                self.trace_path_certificate.clone(),
            )
        }
        .expect("admit compiler-generated callable-corpus provider")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.directory) else {
            return;
        };
        if metadata.file_type().is_symlink() || metadata.is_file() {
            let _ = fs::remove_file(&self.directory);
        } else if metadata.is_dir() {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
}

fn library_filename() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "libcallable-corpus.dylib"
    }
    #[cfg(target_os = "linux")]
    {
        "libcallable-corpus.so"
    }
    #[cfg(target_os = "windows")]
    {
        "callable-corpus.dll"
    }
}

fn assert_artifact_determinism(
    first: &NativeCallableAdmissionArtifact,
    second: &NativeCallableAdmissionArtifact,
) {
    assert_eq!(first.descriptor(), second.descriptor());
    assert_eq!(first.getter_symbol(), second.getter_symbol());
    assert_eq!(first.callable_symbol(), second.callable_symbol());
    assert_eq!(first.call_contract(), second.call_contract());
    assert_eq!(first.max_request_bytes(), second.max_request_bytes());
    assert_eq!(first.max_response_bytes(), second.max_response_bytes());
    assert_eq!(first.provider_source(), second.provider_source());
    assert_eq!(first.event_dictionary(), second.event_dictionary());
    assert_eq!(
        first.trace_path_certificate(),
        second.trace_path_certificate()
    );
    assert_eq!(
        first.semantic_event_dictionary(),
        second.semantic_event_dictionary()
    );
    assert_eq!(
        first.codec_profile_fingerprint(),
        second.codec_profile_fingerprint()
    );
    assert_eq!(
        first.normalized_execution_projection(),
        second.normalized_execution_projection()
    );
}

fn executed<T>(
    result: Result<NativeCallableExecution<T>, RejectedCall>,
    scenario: &str,
) -> NativeCallableExecution<T> {
    match result {
        Ok(execution) => execution,
        Err(rejection) => panic!(
            "corpus scenario `{scenario}` rejected before commit: {:?}",
            rejection.rejection()
        ),
    }
}

fn adopt(host: &mut NativeCallableHost, owner_ordinal: usize, payload: u64) -> NativeOwner {
    // SAFETY: Every corpus argument models one fresh exclusive logical Token.
    // Equal payload bits remain distinct owners and carry no liveness meaning.
    unsafe { host.adopt_trusted_owner(owner_ordinal, payload) }
        .expect("adopt canonical corpus owner")
}

fn execute_case(host: &mut NativeCallableHost, case: &OwnedResourceCorpusCase) -> Observation {
    let mut owners = Vec::new();
    let mut scalars = Vec::new();
    let mut owner_ordinal = 0_usize;
    for argument in &case.arguments {
        match *argument {
            OwnedResourceCorpusArgument::Owned(payload) => {
                owners.push(adopt(host, owner_ordinal, payload));
                owner_ordinal += 1;
            }
            OwnedResourceCorpusArgument::Bool(value) => scalars.push(ScalarValue::Bool(value)),
            OwnedResourceCorpusArgument::I64(value) => scalars.push(ScalarValue::I64(value)),
        }
    }

    match host.result_kind() {
        ResultKind::ScalarI64 => {
            assert_eq!(case.expected_owned_result_ordinal, None);
            let (result, events) = executed(
                host.call_scalar_with_values(owners, scalars),
                case.scenario_id,
            )
            .into_parts();
            let events = owned_events(events);
            assert_eq!(events, case.reference.events, "{} trace", case.scenario_id);
            let outcome = match (&case.reference.outcome, result) {
                (TraceOutcome::Success { result: TraceResult::I64(expected) }, Ok(actual)) => {
                    assert_eq!(actual, *expected, "{} scalar result", case.scenario_id);
                    ObservedOutcome::Scalar(actual)
                }
                (TraceOutcome::Failure { status: expected, .. }, Err(actual)) => {
                    assert_eq!(&actual, expected, "{} status", case.scenario_id);
                    ObservedOutcome::Failure(actual)
                }
                (expected, actual) => panic!(
                    "scenario `{}` scalar outcome mismatch: expected {expected:?}, actual {actual:?}",
                    case.scenario_id
                ),
            };
            assert_eq!(host.live_owner_count(), 0, "{} liveness", case.scenario_id);
            Observation {
                outcome,
                events,
                live_after_execution: 0,
                rotated_owner_reused: false,
                final_live_owners: 0,
            }
        }
        ResultKind::OwnedInput { owner_ordinal } => {
            if let Some(expected) = case.expected_owned_result_ordinal {
                assert_eq!(
                    owner_ordinal, expected,
                    "{} owner mapping",
                    case.scenario_id
                );
            }
            let (result, events) = executed(
                host.call_owned_with_values(owners, scalars),
                case.scenario_id,
            )
            .into_parts();
            let events = owned_events(events);
            assert_eq!(events, case.reference.events, "{} trace", case.scenario_id);
            match (&case.reference.outcome, result) {
                (
                    TraceOutcome::Success {
                        result: TraceResult::Owned { .. },
                    },
                    Ok(owner),
                ) => {
                    assert_eq!(
                        host.live_owner_count(),
                        1,
                        "{} publication",
                        case.scenario_id
                    );
                    let owner = reuse_rotated_owner(host, case.function_id, owner);
                    assert_eq!(host.live_owner_count(), 1, "{} rotation", case.scenario_id);
                    drop(owner);
                    assert_eq!(
                        host.live_owner_count(),
                        0,
                        "{} final liveness",
                        case.scenario_id
                    );
                    Observation {
                        outcome: ObservedOutcome::Owned,
                        events,
                        live_after_execution: 1,
                        rotated_owner_reused: true,
                        final_live_owners: 0,
                    }
                }
                (
                    TraceOutcome::Failure {
                        status: expected, ..
                    },
                    Err(actual),
                ) => {
                    assert_eq!(&actual, expected, "{} status", case.scenario_id);
                    assert_eq!(
                        host.live_owner_count(),
                        0,
                        "{} failure liveness",
                        case.scenario_id
                    );
                    Observation {
                        outcome: ObservedOutcome::Failure(actual),
                        events,
                        live_after_execution: 0,
                        rotated_owner_reused: false,
                        final_live_owners: 0,
                    }
                }
                (expected, actual) => {
                    let actual = actual.map(|_| "owned");
                    panic!(
                        "scenario `{}` owned outcome mismatch: expected {expected:?}, actual {actual:?}",
                        case.scenario_id
                    )
                }
            }
        }
    }
}

fn owned_events(events: Vec<std::sync::Arc<TraceEvent>>) -> Vec<TraceEvent> {
    events
        .into_iter()
        .map(|event| event.as_ref().clone())
        .collect()
}

fn reuse_rotated_owner(
    host: &mut NativeCallableHost,
    function_id: &str,
    owner: NativeOwner,
) -> NativeOwner {
    let execution = match function_id {
        "token.identity" => executed(host.call_owned(vec![owner]), "identity-rotation-probe"),
        "token.choose-second" => {
            let first = adopt(host, 0, 0x5a5a);
            executed(
                host.call_owned_with_values(vec![first, owner], vec![ScalarValue::I64(17)]),
                "choose-second-rotation-probe",
            )
        }
        _ => panic!("unexpected owned-success corpus function `{function_id}`"),
    };
    match execution.into_parts().0 {
        Ok(owner) => owner,
        Err(status) => panic!("rotated owner probe failed: {status:?}"),
    }
}

#[test]
fn authoritative_corpus_executes_through_generated_callable_host_at_o0_and_o2() {
    assert_required_sanitizer_environment();
    let corpus = build_owned_resource_corpus_v1().expect("build authoritative owned corpus");
    assert_eq!(
        corpus
            .cases
            .iter()
            .map(|case| case.scenario_id)
            .collect::<Vec<_>>(),
        OWNED_RESOURCE_CORPUS_V1_SCENARIOS
    );
    let mut cases_by_function = BTreeMap::<&str, Vec<&OwnedResourceCorpusCase>>::new();
    for case in &corpus.cases {
        cases_by_function
            .entry(case.function_id)
            .or_default()
            .push(case);
    }

    let mut observations = BTreeMap::<&str, [Option<Observation>; 2]>::new();
    for (function_id, cases) in cases_by_function {
        let first =
            emit_native_callable_admission(&corpus.program, &DeclarationId::new(function_id))
                .expect("emit first callable-corpus artifact");
        let second =
            emit_native_callable_admission(&corpus.program, &DeclarationId::new(function_id))
                .expect("emit deterministic callable-corpus artifact");
        assert_artifact_determinism(&first, &second);

        for (optimization_index, optimization) in Optimization::ALL.into_iter().enumerate() {
            let fixture = Fixture::compile(&first, optimization);
            // SAFETY: Fixture::open documents the exact compiler-generated
            // provider and keeps its image alive for the complete host scope.
            let mut host = unsafe { fixture.open() };
            for case in &cases {
                let observation = execute_case(&mut host, case);
                observations
                    .entry(case.scenario_id)
                    .or_insert_with(|| [None, None])[optimization_index] = Some(observation);
            }
            assert_eq!(host.live_owner_count(), 0);
            drop(host);
            drop(fixture);
        }
    }

    assert_eq!(observations.len(), OWNED_RESOURCE_CORPUS_V1_SCENARIOS.len());
    for scenario in OWNED_RESOURCE_CORPUS_V1_SCENARIOS {
        assert!(observations.contains_key(scenario));
    }
    for (scenario, [o0, o2]) in observations {
        assert_eq!(
            o0.expect("O0 observation"),
            o2.expect("O2 observation"),
            "scenario `{scenario}` changed across optimization levels"
        );
    }
}
