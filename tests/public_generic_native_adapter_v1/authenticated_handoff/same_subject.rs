//! One checked subject on two physical backends, not full settlement parity.
//! In particular, native consumes failed inputs; Wasm retains them until release.
//! Interpreter events describe result copy-out only, not physical handle cleanup.
#[path = "same_subject_c.rs"]
mod c;
#[path = "caller_hostility.rs"]
mod caller_hostility;
#[path = "checked_allocating.rs"]
mod checked_allocating;
#[path = "checked_moves.rs"]
mod checked_moves;
#[path = "same_subject_cxx.rs"]
mod cxx;
#[path = "same_subject_interpreter.rs"]
mod interpreter;
#[path = "same_subject_lifecycle.rs"]
mod lifecycle;
#[path = "profile_hostility.rs"]
mod profile_hostility;
#[path = "profile_rust.rs"]
mod profile_rust;
#[path = "wasm_comparison.rs"]
mod wasm_comparison;
#[path = "wasm_large_payload.rs"]
mod wasm_large_payload;
#[path = "same_subject_physical.rs"]
mod physical;
#[path = "same_subject_rust.rs"]
mod rust;
#[path = "same_subject_typescript.rs"]
mod typescript;
use super::{array, SOURCE};
use semaprax::public_generic_abi::{
    carrier::{
        frame::{parse_bounded, CarrierFrameBinding, CarrierLeaf, LeafKind},
        trace::Direction,
    },
    compiler_endpoint::derive_admitted_public_generic_endpoint_v1,
    native::authenticated::render_authenticated_identity_provider,
};
use std::{fs, path::Path, process::Command};

const PAYLOADS: [&[u8]; 2] = [&[1, 7, 13], &[2, 11, 17, 23]];

struct Subject {
    provider: String,
    driver: String,
    result_binding: CarrierFrameBinding,
    interpreted: (u32, Vec<Vec<u8>>),
}

fn decode_native(bytes: &[u8]) -> Vec<Vec<u8>> {
    fn length(bytes: &mut &[u8]) -> usize {
        assert!(bytes.len() >= 8, "truncated native length");
        let value = usize::try_from(u64::from_le_bytes(bytes[..8].try_into().unwrap())).unwrap();
        *bytes = &bytes[8..];
        value
    }
    let mut remaining = bytes;
    assert_eq!(length(&mut remaining), 2);
    let leaves = (0..2)
        .map(|_| {
            let size = length(&mut remaining);
            assert!(size <= 65_536 && size <= remaining.len());
            let result = remaining[..size].to_vec();
            remaining = &remaining[size..];
            result
        })
        .collect();
    assert!(remaining.is_empty(), "trailing native result bytes");
    leaves
}

fn native_run(root: &Path, provider: &str, driver: &str, opt: &str, negative: bool) -> Vec<u8> {
    fs::write(root.join("provider.c"), provider).unwrap();
    fs::write(root.join("driver.c"), driver).unwrap();
    let executable = root.join(format!("probe{opt}{}", std::env::consts::EXE_SUFFIX));
    let built = Command::new("clang")
        .args(["-std=c11", opt, "-Wall", "-Wextra", "-Werror"])
        .arg(root.join("provider.c"))
        .arg(root.join("driver.c"))
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("same-subject selector requires provisioned clang");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = Command::new(executable).output().unwrap();
    assert_eq!(
        run.status.success(),
        !negative,
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    if negative {
        assert_eq!(run.status.code(), Some(42));
        assert!(String::from_utf8_lossy(&run.stderr).contains("checked-status-divergence"));
        assert!(
            run.stdout.is_empty(),
            "mutant published an accepted observation"
        );
        return Vec::new();
    }
    let hex = std::str::from_utf8(&run.stdout).unwrap();
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap())
        .collect()
}

fn prepare_subject(root: &Path, guard: bool) -> Subject {
    let source = SOURCE.replace(
        "requires true",
        if guard {
            "requires true"
        } else {
            "requires false"
        },
    );
    let parsed = semaprax::check(&source, Path::new("same-subject.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    fs::write(root.join("subject.spx"), &revision).unwrap();
    let program = semaprax::hir::resolve(&parsed).unwrap();
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let interpreted = interpreter::observe(&program, &endpoint, guard);
    let native =
        render_authenticated_identity_provider(&program, &revision, endpoint.descriptor()).unwrap();
    let wasm = semaprax::wasm::emit_public_generic_wasm_provider_v1(&program, &endpoint).unwrap();
    wasm.verify().unwrap();
    assert_eq!(native.descriptor_bytes(), endpoint.descriptor_bytes());
    assert_eq!(native.descriptor_bytes(), wasm.descriptor_bytes());
    assert_ne!(native.binding().encode(), wasm.binding_bytes());
    assert_ne!(
        native.binding().provider_artifact_digest(),
        wasm.artifact_digest()
    );
    let input =
        CarrierFrameBinding::from_verified_descriptor(endpoint.descriptor(), Direction::Input);
    assert_eq!(input.leaf_paths(), ["@9:auth.left", "@10:auth.right"]);
    let frame = input
        .frame_with_leaves(
            input
                .leaf_paths()
                .iter()
                .zip(PAYLOADS)
                .map(|(path, payload)| CarrierLeaf::new(path, LeafKind::Bytes, payload.to_vec()))
                .collect(),
        )
        .encode();
    let mut constants = String::new();
    for (name, bytes) in [
        ("descriptor", native.descriptor_bytes()),
        ("binding", &native.binding().encode()),
        (
            "cleanup",
            endpoint.descriptor().settlement().digest().as_bytes(),
        ),
        ("canonical", &frame),
    ] {
        constants.push_str(&array(name, bytes));
    }
    let provider = format!("{}\nstatic size_t endpoint_calls;\n#define SPX_PG_OBSERVE_ENDPOINT() (++endpoint_calls)\n{}\n#undef malloc\n#undef free\nsize_t auth_live(void) {{ return fixture_live; }}\nsize_t auth_calls(void) {{ return endpoint_calls; }}\n", include_str!("../allocations.c"), native.source());
    c::observe(
        root,
        endpoint.descriptor(),
        &native,
        &provider,
        &frame,
        guard,
    );
    rust::observe(root, endpoint.descriptor(), &native, &provider, guard);
    cxx::observe(root, endpoint.descriptor(), &native, &provider, guard);
    let expected = if guard { 0 } else { 11 };
    let driver = format!(
        "{}\n{}\n{}\n#define EXPECT_CALL_STATUS {expected}\n{}",
        semaprax::public_generic_abi::native::template::HEADER_V1,
        semaprax::public_generic_abi::native::authenticated::HEADER,
        constants,
        include_str!("same_subject.c")
    );
    fs::write(root.join("provider.wasm"), wasm.wasm()).unwrap();
    fs::write(root.join("descriptor.bin"), wasm.descriptor_bytes()).unwrap();
    fs::write(root.join("binding.bin"), wasm.binding_bytes()).unwrap();
    fs::write(root.join("input.bin"), &frame).unwrap();
    assert_eq!(
        typescript::observe(root, &endpoint, &wasm, guard),
        interpreted
    );
    fs::write(root.join("probe.mjs"), include_str!("same_subject.mjs")).unwrap();
    let result =
        CarrierFrameBinding::from_verified_descriptor(endpoint.descriptor(), Direction::Result);
    Subject {
        provider,
        driver,
        result_binding: result,
        interpreted,
    }
}

#[test]
fn checked_identity_native_and_core_wasm_share_one_subject() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-same-subject-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    eprintln!("same-subject physical evidence: {}", root.display());
    for guard in [true, false] {
        let case = root.join(if guard { "true" } else { "false" });
        fs::create_dir(&case).unwrap();
        let root = &case;
        let Subject {
            provider,
            driver,
            result_binding,
            interpreted,
        } = prepare_subject(root, guard);
        let expected = if guard { 0 } else { 11 };
        assert_eq!(interpreted.0, expected);
        let execution = Command::new("node")
            .arg("probe.mjs")
            .arg(expected.to_string())
            .current_dir(root)
            .output()
            .expect("same-subject selector requires provisioned Node");
        assert!(
            execution.status.success(),
            "{}",
            String::from_utf8_lossy(&execution.stderr)
        );
        let wasm_bytes: Vec<u8> = serde_json::from_slice(&execution.stdout).unwrap();
        let wasm_leaves = if guard {
            let result = parse_bounded(&wasm_bytes).unwrap();
            result_binding.validate_frame(&result).unwrap();
            result
                .leaves()
                .iter()
                .map(|leaf| leaf.payload().to_vec())
                .collect::<Vec<_>>()
        } else {
            assert!(wasm_bytes.is_empty(), "failed call published a result");
            Vec::new()
        };
        assert_eq!(interpreted.1, wasm_leaves);
        for opt in ["-O0", "-O2"] {
            let bytes = native_run(root, &provider, &driver, opt, false);
            let native_leaves = if guard {
                decode_native(&bytes)
            } else {
                assert!(bytes.is_empty());
                Vec::new()
            };
            assert_eq!(native_leaves, wasm_leaves);
            if guard {
                assert_eq!(native_leaves, PAYLOADS);
            }
            eprintln!("same subject requires={guard} interpreter/native={opt}/core-wasm status={expected} exact leaves; backend-local settlement checked");
            if !guard {
                let call = provider
                    .lines()
                    .find(|line| line.contains("(&context, &input, &result) != SPX_STATUS_SUCCESS"))
                    .unwrap();
                assert_eq!(provider.matches(call).count(), 1);
                native_run(
                    root,
                    &provider.replace(call, "    result = input;"),
                    &driver,
                    opt,
                    true,
                );
                eprintln!("same subject {opt} checked-call omission rejected before observation");
            }
        }
    }
}
