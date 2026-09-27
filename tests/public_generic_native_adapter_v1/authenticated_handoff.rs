//! Direct physical C entry tests, including controls which must fail when the
//! admission check or checked call is removed. No callback-only evidence.
use semaprax::public_generic_abi::{
    carrier::{
        frame::{CarrierFrameBinding, CarrierLeaf, LeafKind},
        trace::Direction,
    },
    descriptor::{
        producer::generate_public_generic_descriptor,
        verify::{verify_public_generic_descriptor, VerificationOptions},
    },
    native::authenticated::render_authenticated_identity_provider,
};
use sha2::{Digest as _, Sha256};
use std::{fs, path::Path, process::Command};

#[path = "authenticated_handoff/checked_nested_moves.rs"]
mod checked_nested_moves;
#[path = "authenticated_handoff/same_subject.rs"]
mod same_subject;

const REVISION: &str = "r07-native-identity-v1";
// The C runtime translates puts("...\n") to CRLF on Windows; Rust's
// println! emits LF. Keep each physical caller's bytes exact.
const C_SETTLED_STDOUT: &[u8] = if cfg!(windows) {
    b"authenticated-native-handoff-settled\r\n"
} else {
    b"authenticated-native-handoff-settled\n"
};
const SOURCE: &str = r#"
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

fn array(name: &str, bytes: &[u8]) -> String {
    format!(
        "static const uint8_t {name}[] = {{{}}};\n",
        bytes
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn remint(frame: &mut [u8]) {
    // The final canonical field is the 8-byte length plus 71-byte digest.
    let preimage_len = frame.len() - 79;
    let mut hash = Sha256::new();
    hash.update(b"semaprax.public-generic-carrier.v1.frame\0");
    hash.update((preimage_len as u64).to_le_bytes());
    hash.update(&frame[..preimage_len]);
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    );
    frame[preimage_len + 8..].copy_from_slice(digest.as_bytes());
}

fn fixture(guard: bool) -> (String, String) {
    let source = SOURCE.replace(
        "requires true",
        if guard {
            "requires true"
        } else {
            "requires false"
        },
    );
    let parsed = semaprax::parse(&source, Path::new("authenticated.spx")).unwrap();
    let program = semaprax::hir::resolve(&parsed).unwrap();
    let generated =
        generate_public_generic_descriptor(&program, REVISION, "auth.identity").unwrap();
    let descriptor = verify_public_generic_descriptor(
        &program,
        REVISION,
        "auth.identity",
        &crate::native_frame_admission::program_root(&program),
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap();
    let artifact = render_authenticated_identity_provider(&program, REVISION, &descriptor).unwrap();
    assert_eq!(
        artifact.source(),
        render_authenticated_identity_provider(&program, REVISION, &descriptor)
            .unwrap()
            .source()
    );
    let plan = CarrierFrameBinding::from_verified_descriptor(&descriptor, Direction::Input);
    let leaves: Vec<_> = plan
        .leaf_paths()
        .iter()
        .enumerate()
        .map(|(i, p)| CarrierLeaf::new(p, LeafKind::Bytes, vec![1 + i as u8, 7, 13]))
        .collect();
    let frame = plan.frame_with_leaves(leaves.clone()).encode();
    let mut duplicates = leaves.clone();
    duplicates[1] = duplicates[0].clone();
    let duplicate_path = plan.frame_with_leaves(duplicates).encode();
    let mut unknown_direction = frame.clone();
    let schema_len = u64::from_le_bytes(frame[..8].try_into().unwrap()) as usize;
    unknown_direction[8 + schema_len + 8] = b'x';
    remint(&mut unknown_direction);
    let mut oversized_metadata = frame.clone();
    oversized_metadata[8 + schema_len..8 + schema_len + 8].copy_from_slice(&65537u64.to_le_bytes());
    let mut count_offset = 0;
    for _ in 0..6 {
        let field_len =
            u64::from_le_bytes(frame[count_offset..count_offset + 8].try_into().unwrap()) as usize;
        count_offset += 8 + field_len;
    }
    let mut oversized_count_truncated = frame[..count_offset + 8].to_vec();
    oversized_count_truncated[count_offset..].copy_from_slice(&257u64.to_le_bytes());
    assert_eq!(
        semaprax::public_generic_abi::carrier::frame::parse_bounded(&oversized_count_truncated)
            .unwrap_err()
            .code,
        "SPX-PG802"
    );
    let mut substituted = leaves.clone();
    substituted[0] = CarrierLeaf::new("forged.path", LeafKind::Bytes, vec![1, 7, 13]);
    let wrong_path = plan.frame_with_leaves(substituted).encode();
    let mut wrong_tag = frame.clone();
    let path = plan.leaf_paths()[0].as_bytes();
    let tag = wrong_tag
        .windows(path.len())
        .position(|w| w == path)
        .unwrap()
        + path.len();
    let mut invalid_utf8 = frame.clone();
    invalid_utf8[tag - path.len()] = 0xff;
    remint(&mut invalid_utf8);
    for malformed in [
        &unknown_direction,
        &duplicate_path,
        &invalid_utf8,
        &oversized_metadata,
    ] {
        assert_eq!(
            semaprax::public_generic_abi::carrier::frame::parse_bounded(malformed)
                .unwrap_err()
                .code,
            "SPX-PG801"
        );
    }
    wrong_tag[tag] = 1;
    let mut corrupt = frame.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    let mut oversized = frame.clone();
    oversized[tag + 1..tag + 9].copy_from_slice(&65537u64.to_le_bytes());
    let mut constants = String::new();
    for (name, bytes) in [
        ("descriptor", artifact.descriptor_bytes()),
        ("binding", &artifact.binding().encode()),
        ("cleanup", descriptor.settlement().digest().as_bytes()),
        ("canonical", &frame),
        ("wrong_path", &wrong_path),
        ("unknown_direction", &unknown_direction),
        ("duplicate_path", &duplicate_path),
        ("invalid_utf8", &invalid_utf8),
        ("oversized_metadata", &oversized_metadata),
        ("oversized_count_truncated", &oversized_count_truncated),
        ("wrong_tag", &wrong_tag),
        ("corrupt", &corrupt),
        ("oversized", &oversized),
    ] {
        constants.push_str(&array(name, bytes));
    }
    let provider=format!("{}\nstatic size_t endpoint_calls;\n#define SPX_PG_OBSERVE_ENDPOINT() (++endpoint_calls)\n{}\n#undef malloc\n#undef free\nsize_t auth_allocations(void) {{ return fixture_allocations; }}\nsize_t auth_live(void) {{ return fixture_live; }}\nsize_t auth_calls(void) {{ return endpoint_calls; }}\n",include_str!("allocations.c"),artifact.source());
    let driver=format!("#include <assert.h>\n#include <string.h>\n#include <stdio.h>\n{}\n{}\n{}\n#define EXPECT_CALL_STATUS {}\n{}",semaprax::public_generic_abi::native::template::HEADER_V1,
        semaprax::public_generic_abi::native::authenticated::HEADER,constants,
        if guard {0} else {11},include_str!("authenticated_probe.c"));
    (provider, driver)
}

fn compile_run(provider: &str, driver: &str, expected_success: bool, label: &str) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-authenticated-handoff-{}-{}-{label}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("provider.c"), provider).unwrap();
    fs::write(root.join("driver.c"), driver).unwrap();
    let mut rust_driver = String::new();
    for line in driver.lines() {
        if let Some(definition) = line.strip_prefix("static const uint8_t ") {
            let (name, values) = definition.split_once("[] = {").unwrap();
            rust_driver.push_str(&format!(
                "const {}: &[u8] = &[{}];\n",
                name.to_uppercase(),
                values.strip_suffix("};").unwrap()
            ));
        }
        if let Some(value) = line.strip_prefix("#define EXPECT_CALL_STATUS ") {
            rust_driver.push_str(&format!("const EXPECT_CALL_STATUS: i32 = {value};\n"));
        }
    }
    rust_driver.push_str(include_str!("authenticated_probe.rs.txt"));
    fs::write(root.join("driver.rs"), rust_driver).unwrap();
    eprintln!("authenticated native evidence: {}", root.display());
    for opt in ["-O0", "-O2"] {
        let object = root.join(format!("provider{opt}.o"));
        let built = Command::new("clang")
            .args(["-std=c11", opt, "-Wall", "-Wextra", "-Werror", "-c"])
            .arg(root.join("provider.c"))
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        for (compiler, language, standard) in [("clang", "c", "c11"), ("clang++", "c++", "c++17")] {
            let executable = root.join(format!("probe{opt}-{language}"));
            let built = Command::new(compiler)
                .args([
                    "-x",
                    language,
                    &format!("-std={standard}"),
                    opt,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                ])
                .arg(root.join("driver.c"))
                .args(["-x", "none"])
                .arg(&object)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
            let run = Command::new(&executable).output().unwrap();
            assert_eq!(
                run.status.success(),
                expected_success,
                "{label}: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            if expected_success {
                assert_eq!(run.stdout, C_SETTLED_STDOUT);
            }
        }
        let executable = root.join(format!("probe{opt}-rust"));
        let built = Command::new("rustc")
            .args(["--edition=2021", "-C", "debuginfo=0", "-C"])
            .arg(format!("link-arg={}", object.display()))
            .arg(root.join("driver.rs"))
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let run = Command::new(&executable).output().unwrap();
        assert_eq!(
            run.status.success(),
            expected_success,
            "{label}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        if expected_success {
            assert_eq!(run.stdout, b"authenticated-native-handoff-settled\n");
        }
    }
}

#[test]
fn direct_native_authenticated_handoff_rejects_before_allocation_and_dispatch() {
    let (provider, driver) = fixture(true);
    compile_run(&provider, &driver, true, "canonical");
}

#[test]
fn selected_checked_contract_executes_and_settles() {
    let (provider, driver) = fixture(false);
    compile_run(&provider, &driver, true, "requires-false");
}

#[test]
fn native_admission_bypass_negative_control_is_detected() {
    let (provider, driver) = fixture(true);
    let check = "if (!spx_pg_bytes_equal(value,len,expected,expected_len)) binding_mismatch=1;";
    assert_eq!(provider.matches(check).count(), 1);
    compile_run(
        &provider.replace(check, "/* deliberate admission bypass */"),
        &driver,
        false,
        "negative-path-bypass",
    );
}

#[test]
fn native_checked_call_omission_negative_control_is_detected() {
    let (provider, driver) = fixture(false);
    let call = provider
        .lines()
        .find(|line| line.contains("(&context, &input, &result) != SPX_STATUS_SUCCESS"))
        .unwrap();
    // Model an identity stub which ignores the selected endpoint's requires
    // false guard but still claims it was dispatched. The runtime oracle must
    // reject it, independently of the callback count.
    compile_run(
        &provider.replace(call, "    result = input;"),
        &driver,
        false,
        "negative-checked-call-omission",
    );
}

#[test]
fn native_identity_profile_refuses_broader_body_before_emission() {
    let source = SOURCE.replace("{ value }", "{ let saved = value; saved }");
    let parsed = semaprax::parse(&source, Path::new("unsupported.spx")).unwrap();
    let program = semaprax::hir::resolve(&parsed).unwrap();
    let generated =
        generate_public_generic_descriptor(&program, REVISION, "auth.identity").unwrap();
    let descriptor = verify_public_generic_descriptor(
        &program,
        REVISION,
        "auth.identity",
        &crate::native_frame_admission::program_root(&program),
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap();
    let diagnostic = render_authenticated_identity_provider(&program, REVISION, &descriptor)
        .err()
        .unwrap();
    assert_eq!(diagnostic.code, "SPX-B103");
    assert!(diagnostic.message.contains("identity body"));
}
