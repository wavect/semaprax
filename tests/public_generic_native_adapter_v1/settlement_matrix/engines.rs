//! Real engine and generated-consumer runners for the settlement matrix.
//! Every runner builds from the one checked subject below and reports the
//! shared receipt; missing toolchains fail the gate rather than skipping.
use super::{
    expected_cell, parse_receipts, Case, Engine, Expect, Kind, Observation, Subject, CASES,
};
use semaprax::{
    conformance::{StatusClass, CONTRACT_REQUIRES_FALSE_CODE, CONTRACT_STATUS_DOMAIN_V1},
    hir::{DeclarationId, ResolvedProgram},
    interpreter::retained_call::{
        evaluate_retained_call, prepare_retained_call, RetainedCallOutcome, RetainedField,
        RetainedRecord, RetainedValue,
    },
    public_generic_abi::{
        carrier::{
            frame::{parse_bounded, CarrierFrameBinding, CarrierLeaf, LeafKind},
            trace::Direction,
        },
        compiler_endpoint::{
            derive_admitted_public_generic_endpoint_v1, AdmittedPublicGenericEndpointV1,
        },
        native::{
            authenticated::{
                render_authenticated_allocating_provider, render_authenticated_identity_provider,
                AuthenticatedNativeAllocatingArtifact, AuthenticatedNativeIdentityArtifact,
            },
            binding::NativeProviderBindingV1,
        },
    },
    public_generic_consumer::{c_calling, cxx_calling, rust_calling, typescript_calling},
    wasm::PublicGenericWasmProviderArtifactV1,
};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

/// The same checked identity subject the authenticated same-subject cells use.
const IDENTITY_SOURCE: &str = r#"
module authenticated.native;
@id("auth.pair")
record Pair<T> {
    @id("auth.left")
    left: T,
    @id("auth.right")
    right: T,
}
@id("auth.identity")
fn identity(value: own Pair<Bytes>) -> Pair<Bytes>
    requires true
{ value }
@id("auth.main")
fn main() -> i64 { 0 }
"#;
/// The checked allocating body and helpers of the authenticated allocating cell.
const ALLOCATING_HELPERS: &str = r#"
@id("auth.offset")
fn offset(base: usize) -> usize { base }
@id("auth.remake")
fn remake(value: own Bytes, index: usize) -> Bytes {
    let unused_copy = bytes_copy(bytes_as_slice(value));
    let fresh = bytes_set(bytes_zeroed(3usize), index, 9u8);
    fresh
}
"#;
const ALLOCATING_BODY: &str = r"{
    let left = value.left;
    let right = value.right;
    let copied = bytes_copy(bytes_as_slice(left));
    let fresh = remake(right, offset(0usize));
    Pair<Bytes> { left: copied, right: fresh }
}";

fn replace_once(source: &str, from: &str, to: &str) -> String {
    assert_eq!(
        source.matches(from).count(),
        1,
        "exact subject edit: {from}"
    );
    source.replacen(from, to, 1)
}

fn subject_source(subject: Subject) -> String {
    match subject {
        Subject::Identity => IDENTITY_SOURCE.to_owned(),
        Subject::Refusing => replace_once(IDENTITY_SOURCE, "requires true", "requires false"),
        Subject::Allocating => replace_once(
            &replace_once(IDENTITY_SOURCE, "{ value }", ALLOCATING_BODY),
            "@id(\"auth.main\")",
            &format!("{ALLOCATING_HELPERS}@id(\"auth.main\")"),
        ),
    }
}

enum NativeArtifact {
    Identity(AuthenticatedNativeIdentityArtifact),
    Allocating(AuthenticatedNativeAllocatingArtifact),
}

impl NativeArtifact {
    fn source(&self) -> &str {
        match self {
            Self::Identity(artifact) => artifact.source(),
            Self::Allocating(artifact) => artifact.source(),
        }
    }
    fn descriptor_bytes(&self) -> &[u8] {
        match self {
            Self::Identity(artifact) => artifact.descriptor_bytes(),
            Self::Allocating(artifact) => artifact.descriptor_bytes(),
        }
    }
    fn binding(&self) -> &NativeProviderBindingV1 {
        match self {
            Self::Identity(artifact) => artifact.binding(),
            Self::Allocating(artifact) => artifact.binding(),
        }
    }
}

struct Built {
    subject: Subject,
    program: ResolvedProgram,
    endpoint: AdmittedPublicGenericEndpointV1,
    native: NativeArtifact,
    wasm: PublicGenericWasmProviderArtifactV1,
    input_plan: CarrierFrameBinding,
    result_plan: CarrierFrameBinding,
}

fn build(subject: Subject) -> Built {
    let source = subject_source(subject);
    let parsed = semaprax::check(&source, Path::new("settlement-matrix.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    let program = semaprax::hir::resolve(&parsed).unwrap();
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let native = match subject {
        Subject::Allocating => NativeArtifact::Allocating(
            render_authenticated_allocating_provider(&program, &revision, endpoint.descriptor())
                .unwrap(),
        ),
        _ => NativeArtifact::Identity(
            render_authenticated_identity_provider(&program, &revision, endpoint.descriptor())
                .unwrap(),
        ),
    };
    let wasm = semaprax::wasm::emit_public_generic_wasm_provider_v1(&program, &endpoint).unwrap();
    wasm.verify().unwrap();
    // One exact descriptor subject across every engine and consumer.
    assert_eq!(native.descriptor_bytes(), endpoint.descriptor_bytes());
    assert_eq!(wasm.descriptor_bytes(), endpoint.descriptor_bytes());
    eprintln!(
        "settlement matrix subject {subject:?}: descriptor {}",
        endpoint.descriptor().descriptor_digest()
    );
    let input_plan =
        CarrierFrameBinding::from_verified_descriptor(endpoint.descriptor(), Direction::Input);
    let result_plan =
        CarrierFrameBinding::from_verified_descriptor(endpoint.descriptor(), Direction::Result);
    Built {
        subject,
        program,
        endpoint,
        native,
        wasm,
        input_plan,
        result_plan,
    }
}

fn frame(built: &Built, case: &Case) -> Vec<u8> {
    let wrong = matches!(case.kind, Kind::WrongPath | Kind::RefusalEffects);
    let paths = built.input_plan.leaf_paths();
    assert_eq!(paths.len(), 2);
    let leaves = paths
        .iter()
        .zip([case.left.bytes(), case.right.bytes()])
        .enumerate()
        .map(|(index, (path, payload))| {
            let path = if wrong && index == 0 {
                replace_once(path, "auth.left", "auth.Left")
            } else {
                path.clone()
            };
            CarrierLeaf::new(path, LeafKind::Bytes, payload)
        })
        .collect();
    let encoded = built.input_plan.frame_with_leaves(leaves).encode();
    if wrong {
        let parsed = parse_bounded(&encoded).unwrap();
        assert_eq!(
            built.input_plan.validate_frame(&parsed).unwrap_err().code,
            "SPX-PG803"
        );
    }
    encoded
}

fn field(path: &str) -> String {
    format!(
        "field_{}",
        path.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn fields(built: &Built, template: &str) -> String {
    let descriptor = built.endpoint.descriptor();
    let mut text = template.to_owned();
    for (token, path) in [
        ("@INPUT0@", &descriptor.input_facts().owned_leaves[0]),
        ("@INPUT1@", &descriptor.input_facts().owned_leaves[1]),
        ("@OUTPUT0@", &descriptor.result_facts().owned_leaves[0]),
        ("@OUTPUT1@", &descriptor.result_facts().owned_leaves[1]),
    ] {
        text = text.replace(token, &field(path));
    }
    text
}

fn c_array(name: &str, bytes: &[u8]) -> String {
    let body = if bytes.is_empty() {
        "0".to_owned()
    } else {
        bytes
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    format!("static const uint8_t {name}[] = {{{body}}};\n")
}

fn c_cases(built: &Built, cases: &[usize]) -> String {
    let mut text = String::new();
    let mut rows = String::new();
    for (slot, index) in cases.iter().enumerate() {
        let case = &CASES[*index];
        let (left, right, carrier) = (case.left.bytes(), case.right.bytes(), frame(built, case));
        text.push_str(&c_array(&format!("mx_l{slot}"), &left));
        text.push_str(&c_array(&format!("mx_r{slot}"), &right));
        text.push_str(&c_array(&format!("mx_f{slot}"), &carrier));
        rows.push_str(&format!(
            "    {{\"{}\", {}u, {}u, mx_l{slot}, {}u, mx_r{slot}, {}u, mx_f{slot}, {}u}},\n",
            case.id,
            case.kind as u32,
            case.cycles,
            left.len(),
            right.len(),
            carrier.len()
        ));
    }
    format!(
        "{text}static const struct mx_case mx_cases[] = {{\n{rows}}};\n#define MX_CASE_COUNT {}\n",
        cases.len()
    )
}

fn native_provider(built: &Built) -> String {
    format!(
        "{}\n{}\n{}\n#undef malloc\n#undef free\n{}",
        include_str!("../allocations.c"),
        include_str!("../settlement_corpus/observations.c"),
        built.native.source(),
        include_str!("observe.c")
    )
}

fn run(command: &mut Command, label: &str) -> Vec<u8> {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{label}: required toolchain is missing: {error}"));
    assert!(
        output.status.success(),
        "{label}: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    output.stdout
}

fn tool(variable: &str, fallback: &str) -> Command {
    Command::new(env::var_os(variable).unwrap_or_else(|| fallback.into()))
}

fn no_frame(_: &[u8]) -> (Vec<u8>, Vec<u8>) {
    panic!("native receipts never carry a result frame")
}

fn native_raw(
    built: &Built,
    cases: &[usize],
    root: &Path,
    engine: Engine,
    control: bool,
) -> Vec<u8> {
    let directory = root.join(format!("{engine:?}"));
    fs::create_dir_all(&directory).unwrap();
    let descriptor = built.endpoint.descriptor();
    let driver = format!(
        "{}\n{}\n{}{}{}\n{}\n{}\n{}",
        semaprax::public_generic_abi::native::template::HEADER_V1,
        semaprax::public_generic_abi::native::authenticated::HEADER,
        c_array("descriptor", built.native.descriptor_bytes()),
        c_array("binding", &built.native.binding().encode()),
        c_array("cleanup", descriptor.settlement().digest().as_bytes()),
        include_str!("receipt.h"),
        c_cases(built, cases),
        include_str!("raw_native.c"),
    );
    fs::write(directory.join("provider.c"), native_provider(built)).unwrap();
    fs::write(directory.join("driver.c"), driver).unwrap();
    let executable = directory.join(format!("probe{}", env::consts::EXE_SUFFIX));
    let mut compile = tool("CLANG", "clang");
    compile.args(["-std=c11", "-Wall", "-Wextra", "-Werror"]);
    match engine {
        Engine::NativeO0 => compile.arg("-O0"),
        Engine::NativeO2 => compile.arg("-O2"),
        _ => compile.args(["-O1", "-fsanitize=address", "-fno-omit-frame-pointer"]),
    };
    if control {
        compile.arg("-DMX_SKIP_RESULT_RELEASE");
    }
    run(
        compile
            .arg(directory.join("provider.c"))
            .arg(directory.join("driver.c"))
            .arg("-o")
            .arg(&executable),
        "raw native compile",
    );
    let mut probe = Command::new(&executable);
    if control {
        probe.arg("success-small");
    }
    run(&mut probe, engine.label_for_runner())
}

fn write_files(root: &Path, files: &[(String, String)]) {
    for (name, contents) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

fn generated_c(built: &Built, cases: &[usize], root: &Path) -> Vec<u8> {
    let directory = root.join("generated-c11");
    fs::create_dir_all(&directory).unwrap();
    let descriptor = built.endpoint.descriptor();
    let consumer = match &built.native {
        NativeArtifact::Identity(artifact) => {
            c_calling::generate_authenticated_identity_calling_consumer_v1(descriptor, artifact)
        }
        NativeArtifact::Allocating(artifact) => {
            c_calling::generate_authenticated_allocating_calling_consumer_v1(descriptor, artifact)
        }
    }
    .unwrap();
    write_files(&directory, consumer.files());
    fs::write(directory.join("provider.c"), native_provider(built)).unwrap();
    fs::write(
        directory.join("driver.c"),
        format!(
            "{}\n{}\n{}",
            include_str!("receipt.h"),
            c_cases(built, cases),
            fields(built, include_str!("generated.c"))
        ),
    )
    .unwrap();
    let executable = directory.join(format!("probe{}", env::consts::EXE_SUFFIX));
    run(
        tool("CLANG", "clang")
            .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
            .arg(directory.join("provider.c"))
            .arg(directory.join("driver.c"))
            .arg("-o")
            .arg(&executable),
        "generated C11 compile",
    );
    run(&mut Command::new(executable), "generated C11 caller")
}

fn generated_cxx(built: &Built, cases: &[usize], root: &Path) -> Vec<u8> {
    let directory = root.join("generated-cxx17");
    fs::create_dir_all(&directory).unwrap();
    let descriptor = built.endpoint.descriptor();
    let consumer = match &built.native {
        NativeArtifact::Identity(artifact) => {
            cxx_calling::generate_authenticated_identity_calling_consumer_v1(descriptor, artifact)
        }
        NativeArtifact::Allocating(artifact) => {
            cxx_calling::generate_authenticated_allocating_calling_consumer_v1(descriptor, artifact)
        }
    }
    .unwrap();
    write_files(&directory, consumer.files());
    fs::write(directory.join("provider.c"), native_provider(built)).unwrap();
    fs::write(
        directory.join("driver.cpp"),
        format!(
            "{}\n{}\n{}",
            include_str!("receipt.h"),
            c_cases(built, cases),
            fields(built, include_str!("generated.cpp"))
        ),
    )
    .unwrap();
    let mut objects = Vec::new();
    for source in ["provider.c", c_calling::CONSUMER_SOURCE_FILE_NAME] {
        let object = directory.join(format!("{source}.o"));
        run(
            tool("CLANG", "clang")
                .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-c"])
                .arg(directory.join(source))
                .arg("-o")
                .arg(&object),
            "generated C++17 C objects",
        );
        objects.push(object);
    }
    let executable = directory.join(format!("probe{}", env::consts::EXE_SUFFIX));
    run(
        tool("CLANGXX", "clang++")
            .args(["-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror", "-I"])
            .arg(&directory)
            .arg(directory.join("driver.cpp"))
            .args(&objects)
            .arg("-o")
            .arg(&executable),
        "generated C++17 link",
    );
    run(&mut Command::new(executable), "generated C++17 caller")
}

fn rust_bytes(bytes: &[u8]) -> String {
    format!("&{bytes:?}")
}

fn generated_rust(built: &Built, cases: &[usize], root: &Path) -> Vec<u8> {
    let directory = root.join("generated-rust");
    let descriptor = built.endpoint.descriptor();
    let consumer = match &built.native {
        NativeArtifact::Identity(artifact) => {
            rust_calling::generate_authenticated_identity_calling_consumer_v1(descriptor, artifact)
        }
        NativeArtifact::Allocating(artifact) => {
            rust_calling::generate_authenticated_allocating_calling_consumer_v1(
                descriptor, artifact,
            )
        }
    }
    .unwrap();
    write_files(&directory, consumer.files());
    let mut rows = String::new();
    for index in cases {
        let case = &CASES[*index];
        rows.push_str(&format!(
            "    (\"{}\", {}, {}, {}, {}),\n",
            case.id,
            case.kind as u32,
            case.cycles,
            rust_bytes(&case.left.bytes()),
            rust_bytes(&case.right.bytes())
        ));
    }
    fs::write(
        directory.join("matrix_cases.rs"),
        format!("const CASES: &[(&str, u32, u32, &[u8], &[u8])] = &[\n{rows}];\n"),
    )
    .unwrap();
    fs::create_dir_all(directory.join("src/bin")).unwrap();
    fs::write(
        directory.join("src/bin/matrix.rs"),
        fields(built, include_str!("generated_rust.rs.txt")).replace(
            "@LIBRARY@",
            match built.native {
                NativeArtifact::Identity(_) => "spx_pg_private_authenticated_rust_v1",
                NativeArtifact::Allocating(_) => "spx_pg_private_authenticated_allocating_rust_v1",
            },
        ),
    )
    .unwrap();
    fs::write(directory.join("provider.c"), native_provider(built)).unwrap();
    let library = directory.join("provider-lib");
    fs::create_dir_all(&library).unwrap();
    let object = library.join("provider.o");
    run(
        tool("CLANG", "clang")
            .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-c"])
            .arg(directory.join("provider.c"))
            .arg("-o")
            .arg(&object),
        "generated Rust provider object",
    );
    run(
        tool("AR", "ar")
            .arg("rcs")
            .arg(library.join("libspx_pg_reference_provider.a"))
            .arg(&object),
        "generated Rust provider archive",
    );
    let target = env::var_os("CARGO_TARGET_DIR")
        .map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/agent-private"),
            PathBuf::from,
        )
        .join("generated-rust")
        .join(format!("settlement-matrix-{:?}", built.subject));
    let cargo = || {
        let mut command = tool("CARGO", "cargo");
        command
            .current_dir(&directory)
            .env("CARGO_TARGET_DIR", &target)
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("RUSTFLAGS", "-C debuginfo=0 -D warnings")
            .env("SPX_PG_PROVIDER_LIB_DIR", &library)
            .env("SPX_PG_PROVIDER_LIB_NAME", "spx_pg_reference_provider")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER");
        command
    };
    run(
        cargo().args(["generate-lockfile", "--offline"]),
        "generated Rust lock",
    );
    run(
        cargo().args(["run", "--locked", "--offline", "--quiet", "--bin", "matrix"]),
        "generated Rust caller",
    )
}

fn json_cases(built: &Built, cases: &[usize], with_frames: bool) -> Vec<serde_json::Value> {
    cases
        .iter()
        .map(|index| {
            let case = &CASES[*index];
            let mut value = serde_json::json!({
                "id": case.id,
                "kind": case.kind as u32,
                "cycles": case.cycles,
                "left": case.left.bytes(),
                "right": case.right.bytes(),
            });
            if with_frames {
                value["frame"] = serde_json::json!(frame(built, case));
            }
            value
        })
        .collect()
}

fn core_wasm(built: &Built, cases: &[usize], root: &Path) -> Vec<u8> {
    let directory = root.join("core-wasm");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("provider.wasm"), built.wasm.wasm()).unwrap();
    fs::write(
        directory.join("descriptor.bin"),
        built.wasm.descriptor_bytes(),
    )
    .unwrap();
    fs::write(directory.join("binding.bin"), built.wasm.binding_bytes()).unwrap();
    fs::write(
        directory.join("cases.json"),
        serde_json::to_vec(&json_cases(built, cases, true)).unwrap(),
    )
    .unwrap();
    fs::write(directory.join("driver.mjs"), include_str!("core_wasm.mjs")).unwrap();
    run(
        Command::new("node")
            .arg("driver.mjs")
            .current_dir(&directory),
        "Core Wasm driver (Node)",
    )
}

fn checked_tsc(candidate: &Path) -> Option<PathBuf> {
    let resolved = candidate.canonicalize().ok()?;
    let version = Command::new(&resolved).arg("--version").output().ok()?;
    (version.status.success()
        && std::str::from_utf8(&version.stdout).is_ok_and(|text| text.trim() == "Version 5.8.3"))
    .then_some(resolved)
}

fn tsc() -> PathBuf {
    if let Some(explicit) = env::var_os("SPX_PG_TSC").or_else(|| env::var_os("TSC")) {
        return checked_tsc(Path::new(&explicit)).expect("explicit tsc must be TypeScript 5.8.3");
    }
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        candidates.extend(env::split_paths(&path).map(|directory| directory.join("tsc")));
    }
    if let Some(home) = env::var_os("HOME") {
        candidates.push(PathBuf::from(&home).join("Library/pnpm/tsc"));
        candidates.push(PathBuf::from(home).join(".local/share/pnpm/tsc"));
    }
    candidates
        .iter()
        .find_map(|candidate| checked_tsc(candidate))
        .expect("generated TypeScript cell requires pinned TypeScript 5.8.3 (SPX_PG_TSC)")
}

fn generated_typescript(built: &Built, cases: &[usize], root: &Path) -> Vec<u8> {
    let descriptor = built.endpoint.descriptor();
    let shape = |paths: &[String]| {
        rust_calling::RecordShape::new(
            paths
                .iter()
                .cloned()
                .map(rust_calling::OwnedByteField::new)
                .collect(),
        )
    };
    let input = shape(&descriptor.input_facts().owned_leaves);
    let output = shape(&descriptor.result_facts().owned_leaves);
    let consumer = typescript_calling::generate_typescript_calling_consumer(
        built.wasm.descriptor_bytes(),
        built.wasm.binding(),
        &input,
        &output,
    )
    .unwrap();
    let package = root.join("typescript");
    write_files(&package, consumer.files());
    fs::write(root.join("provider.wasm"), built.wasm.wasm()).unwrap();
    let names = input
        .fields
        .iter()
        .map(|leaf| field(&leaf.identity))
        .collect::<Vec<_>>();
    fs::write(
        package.join("test/matrix.json"),
        serde_json::to_vec(&(names, json_cases(built, cases, false))).unwrap(),
    )
    .unwrap();
    fs::write(
        package.join("test/matrix.mjs"),
        include_str!("typescript.mjs"),
    )
    .unwrap();
    run(
        Command::new(tsc())
            .current_dir(&package)
            .args(["-p", "tsconfig.json"]),
        "generated TypeScript tsc",
    );
    run(
        Command::new("node")
            .current_dir(&package)
            .args(["test/matrix.mjs", "../provider.wasm"]),
        "generated TypeScript caller (Node)",
    )
}

fn interpreter(built: &Built, cases: &[usize]) -> Vec<Observation> {
    let prepared = prepare_retained_call(&built.program, built.endpoint.export_id()).unwrap();
    let mut observations = Vec::new();
    for index in cases {
        let case = &CASES[*index];
        let argument = RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("auth.pair"),
            fields: built
                .endpoint
                .descriptor()
                .input_facts()
                .fields
                .iter()
                .zip([case.left.bytes(), case.right.bytes()])
                .map(|(declared, bytes)| RetainedField {
                    field: DeclarationId::new(&declared.id),
                    value: RetainedValue::Bytes(bytes),
                })
                .collect(),
        });
        let cycles = if case.kind == Kind::Repeated {
            case.cycles
        } else {
            1
        };
        for cycle in 0..cycles {
            let evaluated = evaluate_retained_call(
                &built.program,
                &prepared,
                std::slice::from_ref(&argument),
                1_000_000,
            )
            .unwrap();
            let (primary, leaves) = match evaluated.outcome {
                RetainedCallOutcome::Returned(RetainedValue::Record(record)) => {
                    let mut bytes = record.fields.into_iter().map(|leaf| match leaf.value {
                        RetainedValue::Bytes(bytes) => bytes,
                        other => panic!("non-Bytes interpreter leaf: {other:?}"),
                    });
                    let pair = (bytes.next().unwrap(), bytes.next().unwrap());
                    assert!(bytes.next().is_none());
                    ("0".to_owned(), Some(pair))
                }
                // Physical status 11 is the ABI projection of this exact
                // checked contract status (semaprax.contract.v1, requires).
                RetainedCallOutcome::LanguageFailure(status)
                    if status.domain_id() == CONTRACT_STATUS_DOMAIN_V1
                        && status.code() == CONTRACT_REQUIRES_FALSE_CODE
                        && status.class() == StatusClass::Contract =>
                {
                    ("11".to_owned(), None)
                }
                other => (format!("{other:?}").replace(' ', "_"), None),
            };
            observations.push(Observation {
                label: if case.kind == Kind::Repeated {
                    format!("{}#{cycle}", case.id)
                } else {
                    case.id.to_owned()
                },
                primary,
                secondary: None,
                dispatch: None,
                live: None,
                peak: None,
                order: None,
                leaves,
                note: BTreeMap::from([(
                    "copyout".to_owned(),
                    evaluated.cleanup_events.len().to_string(),
                )]),
            });
        }
    }
    observations
}

impl Engine {
    fn label_for_runner(self) -> &'static str {
        match self {
            Self::NativeO0 => "raw native -O0",
            Self::NativeO2 => "raw native -O2",
            _ => "raw native ASan (local)",
        }
    }
}

fn decode_frame(built: &Built) -> impl Fn(&[u8]) -> (Vec<u8>, Vec<u8>) + '_ {
    move |bytes| {
        let parsed = parse_bounded(bytes).unwrap();
        built.result_plan.validate_frame(&parsed).unwrap();
        match parsed.leaves() {
            [left, right] => (left.payload().to_vec(), right.payload().to_vec()),
            _ => panic!("result frame lost its two leaves"),
        }
    }
}

/// Run every requested engine over every applicable case of every subject.
/// `control` runs only the skipped-release raw native negative control.
pub(super) fn execute_all(
    engines: &[Engine],
    control: bool,
) -> BTreeMap<(usize, Engine), Vec<Observation>> {
    let root = env::temp_dir().join(format!(
        "semaprax-settlement-matrix-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // Remove the evidence tree on every exit, including a failing receipt.
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let mut observed = BTreeMap::new();
    for subject in [Subject::Identity, Subject::Refusing, Subject::Allocating] {
        let built = build(subject);
        let subject_root = root.join(format!("{subject:?}"));
        for engine in engines {
            let cases = CASES
                .iter()
                .enumerate()
                .filter(|(_, case)| case.subject == subject)
                .filter(|(_, case)| {
                    !matches!(expected_cell(case, *engine), Expect::NotApplicable(_))
                })
                .filter(|(_, case)| !control || case.id == "success-small")
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if cases.is_empty() {
                continue;
            }
            let receipts = match engine {
                Engine::Interpreter => interpreter(&built, &cases),
                Engine::NativeO0 | Engine::NativeO2 | Engine::NativeAsan => parse_receipts(
                    &native_raw(&built, &cases, &subject_root, *engine, control),
                    &no_frame,
                ),
                Engine::CoreWasm => parse_receipts(
                    &core_wasm(&built, &cases, &subject_root),
                    &decode_frame(&built),
                ),
                Engine::GeneratedC11 => {
                    parse_receipts(&generated_c(&built, &cases, &subject_root), &no_frame)
                }
                Engine::GeneratedRust => {
                    parse_receipts(&generated_rust(&built, &cases, &subject_root), &no_frame)
                }
                Engine::GeneratedCxx17 => {
                    parse_receipts(&generated_cxx(&built, &cases, &subject_root), &no_frame)
                }
                Engine::GeneratedTypeScript => parse_receipts(
                    &generated_typescript(&built, &cases, &subject_root),
                    &no_frame,
                ),
            };
            for receipt in receipts {
                let id = receipt.label.split('#').next().unwrap().to_owned();
                let index = cases
                    .iter()
                    .copied()
                    .find(|index| CASES[*index].id == id)
                    .unwrap_or_else(|| panic!("{engine:?} reported unknown case {id}"));
                observed
                    .entry((index, *engine))
                    .or_insert_with(Vec::new)
                    .push(receipt);
            }
        }
    }
    observed
}
