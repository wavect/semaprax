//! Executable evidence for Freestanding Object Profile v1
//! (`semaprax.freestanding.v1`).
//!
//! Pins canonical golden envelope and translation-unit bytes, exercises the
//! CLI contract and exit codes, proves determinism, rejects tampering per
//! digest field including forged-but-re-signed payloads, and — unlike the
//! sibling projection tranches — actually compiles the emitted translation
//! unit into a relocatable object with `-ffreestanding -nostdlib` and checks
//! its real symbol surface. No target is executed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::freestanding_object::{self, FreestandingObjectOptions};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-freestanding-evidence-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(args)
        .output()
        .expect("semaprax binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

/// The exact domain-separated digest function used by the envelope.
fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

const MEANING_PATH: &str = "examples/meaning.spx";
const PAYLOAD_DOMAIN: &[u8] = b"semaprax.freestanding.payload.v1\0";
const TRANSLATION_UNIT_DOMAIN: &[u8] = b"semaprax.freestanding.translation-unit.v1\0";

/// Golden envelope digest over the exact library bytes for the canonical
/// example, emitted through the relative repository path so the fixture is
/// machine-independent.
#[test]
fn golden_envelope_digest_is_pinned() {
    let envelope = freestanding_object::generate(Path::new(MEANING_PATH), &Default::default())
        .expect("envelope");
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:d51d7cc001a8e47e6dc4aeadacd39588a616b3fe72464052fa5d83d0ea8a4e18"
    );
    assert!(envelope.contains("\"schema\":\"semaprax.freestanding.v1\""));
    assert!(envelope.contains("\"no_runtime\":true"));
}

/// The bare freestanding translation unit is path-independent: the same
/// program emits byte-identical units regardless of where the source lives,
/// and the bytes match the pinned golden digest.
#[test]
fn golden_translation_unit_digest_is_pinned_and_path_independent() {
    let from_examples =
        freestanding_object::unit_text(Path::new(MEANING_PATH), &Default::default()).expect("unit");

    let copied = write_temp(&std::fs::read_to_string(MEANING_PATH).unwrap());
    let from_temp = freestanding_object::unit_text(&copied, &Default::default()).expect("unit");
    cleanup(&copied);

    assert_eq!(from_examples, from_temp);
    assert_eq!(
        sha256_hex(from_examples.as_bytes()),
        "sha256:7dc78c346536a1d8173a76839a73971bafd6ce1b3c515d5c8fbcb765f39a9cc9"
    );
}

#[test]
fn double_runs_are_byte_identical_in_library_and_cli() {
    let first =
        freestanding_object::generate(Path::new(MEANING_PATH), &Default::default()).expect("first");
    let second = freestanding_object::generate(Path::new(MEANING_PATH), &Default::default())
        .expect("second");
    assert_eq!(first, second);

    let args = ["freestanding-object", MEANING_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert_eq!(second_out, format!("{first}\n"));
}

#[test]
fn verify_envelope_returns_the_embedded_translation_unit() {
    let envelope = freestanding_object::generate(Path::new(MEANING_PATH), &Default::default())
        .expect("envelope");
    let verified = freestanding_object::verify_envelope(&envelope).expect("verified");
    assert_eq!(
        verified.translation_unit,
        freestanding_object::unit_text(Path::new(MEANING_PATH), &Default::default()).unwrap()
    );
}

#[test]
fn verify_envelope_rejects_tampering_per_digest_field() {
    let envelope = freestanding_object::generate(Path::new(MEANING_PATH), &Default::default())
        .expect("envelope");

    // Mutating any payload member invalidates the outer digest.
    let payload_member_tampered = envelope.replace("\"admitted\":2", "\"admitted\":1");
    assert_ne!(payload_member_tampered, envelope);
    assert!(freestanding_object::verify_envelope(&payload_member_tampered).is_err());

    // Mutating the outer digest field itself fails authentication.
    const DIGEST_KEY: &str = "\"digest\":\"sha256:";
    let start = envelope.find(DIGEST_KEY).expect("digest member") + DIGEST_KEY.len();
    let end = envelope[start..].find('"').expect("digest ends") + start;
    let forged_digest = format!(
        "{}{}{}",
        &envelope[..start],
        "0000000000000000000000000000000000000000000000000000000000000000",
        &envelope[end..]
    );
    assert!(freestanding_object::verify_envelope(&forged_digest).is_err());

    // Structural damage and foreign schemas fail closed.
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(freestanding_object::verify_envelope(&truncated).is_err());
    assert!(freestanding_object::verify_envelope("not json").is_err());
    let foreign = envelope.replace("semaprax.freestanding.v1", "semaprax.freestanding.v2");
    assert!(freestanding_object::verify_envelope(&foreign).is_err());
}

/// A payload mutated in the embedded translation unit and then fully re-signed
/// (inner unit digest and outer payload digest both recomputed) must still be
/// rejected by the replayed profile assertions.
#[test]
fn re_signed_payloads_are_still_caught_by_assertion_replay() {
    const SCHEMA: &str = "semaprax.freestanding.v1";
    let envelope = freestanding_object::generate(Path::new(MEANING_PATH), &Default::default())
        .expect("envelope");
    let unit =
        freestanding_object::unit_text(Path::new(MEANING_PATH), &Default::default()).expect("unit");

    fn json_escape(value: &str) -> String {
        let mut output = String::with_capacity(value.len());
        for character in value.chars() {
            match character {
                '"' => output.push_str("\\\""),
                '\\' => output.push_str("\\\\"),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                other => output.push(other),
            }
        }
        output
    }

    fn serde_json_string(value: &str) -> String {
        format!("\"{}\"", json_escape(value))
    }

    // Plant a hosted printf call into the embedded translation unit and fix
    // up every digest so only assertion replay can catch the mutation.
    let planted = unit.replace("(void)message;", "printf(\"hosted diagnostics are back\");");
    assert_ne!(planted, unit);
    let mutated = envelope
        .replace(&json_escape(&unit), &json_escape(&planted))
        .replace(
            &domain_digest(TRANSLATION_UNIT_DOMAIN, unit.as_bytes()),
            &domain_digest(TRANSLATION_UNIT_DOMAIN, planted.as_bytes()),
        );
    const PAYLOAD_KEY: &str = "\"payload\":";
    let payload_start = mutated.find(PAYLOAD_KEY).expect("payload member") + PAYLOAD_KEY.len();
    let payload = &mutated[payload_start..mutated.len() - 1];
    let resigned = format!(
        "{{\"schema\":\"{SCHEMA}\",\"digest\":{},\"bytes\":{},\"payload\":{payload}}}",
        serde_json_string(&domain_digest(PAYLOAD_DOMAIN, payload.as_bytes())),
        payload.len(),
    );
    assert_ne!(resigned, envelope);
    let error = freestanding_object::verify_envelope(&resigned)
        .expect_err("replayed assertions must reject the planted printf");
    assert!(
        error.to_string().contains("no_runtime"),
        "expected a no_runtime replay failure: {error}"
    );
}

#[test]
fn budget_exhaustion_fails_closed_with_exit_one() {
    let args = ["freestanding-object", MEANING_PATH, "--max-bytes", "2048"];
    let (code, out, err) = cli(&args);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("SPX-A103"), "{err}");
    let tiny = FreestandingObjectOptions::new(2048).expect("in bounds");
    let errors = freestanding_object::generate(Path::new(MEANING_PATH), &tiny)
        .expect_err("must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-A103"));
}

#[test]
fn modules_outside_the_scalar_profile_fail_closed_with_spx_a102() {
    let cases: [(&str, &str); 6] = [
        (
            r#"
module test.bad;
permit { io.release }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "permits",
        ),
        (
            r#"
module test.bad;

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "generic_function",
        ),
        (
            r#"
module test.bad;

@id("probe.wide")
fn wide(ratio: f64) -> f64 { ratio }

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "unsupported_parameter_type",
        ),
        (
            r#"
module test.bad;

@id("probe.borrowed")
fn borrowed(target: borrow Buffer, amount: i64) -> i64 { amount }

@id("app.main")
fn main() -> i64 { 0 }

resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
"#,
            "unsupported_parameter_mode",
        ),
        (
            r#"
module test.bad;

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { helper(0) }
"#,
            "automatic_identity",
        ),
        (
            r#"
module test.bad;

resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "type declarations",
        ),
    ];
    for (source, needle) in cases {
        let path = write_temp(source);
        let args = ["freestanding-object", path.to_str().expect("utf8 path")];
        let (code, out, err) = cli(&args);
        assert_eq!(code, 1, "{source}");
        assert!(out.is_empty());
        assert!(err.contains("SPX-A102"), "{err}");
        assert!(err.contains(needle), "{err}");
        cleanup(&path);
    }
}

#[test]
fn cli_rejects_bad_invocations_with_usage_exit_codes() {
    // Unknown option.
    let (code, _, _) = cli(&["freestanding-object", MEANING_PATH, "--functions", "x"]);
    assert_eq!(code, 2);
    // Missing option value.
    let (code, _, _) = cli(&["freestanding-object", MEANING_PATH, "--max-bytes"]);
    assert_eq!(code, 2);
    // Duplicate flag.
    let (code, _, _) = cli(&[
        "freestanding-object",
        MEANING_PATH,
        "--max-bytes",
        "4096",
        "--max-bytes",
        "8192",
    ]);
    assert_eq!(code, 2);
    // Malformed number.
    let (code, _, _) = cli(&["freestanding-object", MEANING_PATH, "--max-bytes", "12x"]);
    assert_eq!(code, 2);
    // Out-of-bounds budget.
    let (code, _, _) = cli(&["freestanding-object", MEANING_PATH, "--max-bytes", "512"]);
    assert_eq!(code, 2);
    // Missing file.
    let (code, _, _) = cli(&["freestanding-object", "examples/missing.spx"]);
    assert_eq!(code, 1);
}

const HOST_REFERENCES: &[&str] = &[
    "int main(",
    "#ifndef SPX_NO_ENTRY_WRAPPER",
    "#include <stdio.h>",
    "#include <stdlib.h>",
    "printf",
    "fprintf",
    "fputs",
    "stderr",
    "abort(",
    "spx_public_failure",
];

/// The emitted unit differs from the hosted native lane exactly by the
/// documented host-scaffolding removals plus the three documented
/// substitutions; it contains zero host/main references while the hosted
/// lane contains every one of them, keeps the failstop substitution, and
/// promotes exactly two linkage sites per admitted function.
#[test]
fn emitted_unit_differs_from_hosted_lane_only_by_documented_delta() {
    use semaprax::{codegen, parse};

    let source = std::fs::read_to_string(MEANING_PATH).unwrap();
    let program = parse(&source, Path::new(MEANING_PATH)).expect("parses");
    let hosted = codegen::emit_c(&program).expect("native projection");
    let unit =
        freestanding_object::unit_text(Path::new(MEANING_PATH), &Default::default()).expect("unit");

    for reference in HOST_REFERENCES {
        assert!(
            !unit.contains(reference),
            "freestanding unit must not contain host reference {reference}"
        );
        assert!(
            hosted.contains(reference),
            "hosted lane is expected to contain {reference}"
        );
    }
    assert!(unit.contains("for (;;) {"));
    assert!(unit.contains("spx_status_token spx_decl_6d6174682e616464(struct spx_context *spx_ctx, int64_t spx_param_0"));
    assert!(
        !hosted
            .lines()
            .any(|line| line.starts_with("spx_status_token spx_decl_")),
        "hosted lane must keep every module function internal"
    );
    // Every non-blank unit line either exists verbatim in the hosted lane or
    // is part of the three documented substitutions. The contract-argument
    // replacement is enumerated exactly so emitter drift cannot broaden the
    // accepted delta silently.
    const CONTRACT_ARGUMENT_SUBSTITUTION: &[&str] = &[
        "size_t arguments_length = 0;",
        "while (arguments[arguments_length] != '\\0' &&",
        "arguments_length + 1 < sizeof detail.failure_arguments) {",
        "detail.failure_arguments[arguments_length] = arguments[arguments_length];",
        "arguments_length += 1;",
        "if (arguments[arguments_length] != '\\0') {",
        "spx_runtime_invariant_failure(\"contract argument detail overflow\");",
        "detail.failure_arguments[arguments_length] = '\\0';",
    ];
    for line in unit.lines() {
        let trimmed = line.trim();
        let contract_call_elision = trimmed
            .strip_prefix("spx_status = spx_rt_contract(")
            .and_then(|arguments| arguments.strip_suffix(");"))
            .is_some_and(|arguments| {
                hosted.contains(&format!(
                    "spx_status = spx_rt_contract_with_arguments({arguments}, spx_contract_arguments);"
                ))
            });
        let documented = trimmed
            == "/* SEMAPRAX freestanding profile: hosted diagnostics are excluded. */"
            || trimmed == "(void)message;"
            || trimmed == "for (;;) {"
            || trimmed == "}"
            || CONTRACT_ARGUMENT_SUBSTITUTION.contains(&trimmed)
            || contract_call_elision
            || line.starts_with("spx_status_token spx_decl_");
        assert!(
            documented || hosted.contains(line),
            "unexpected invented line: {line}"
        );
    }
}

/// Cross-platform C compiler discovery mirroring the existing native lanes:
/// `CC`, then `CLANG`, then plain `cc`/`clang` probes.
fn find_compiler() -> Option<String> {
    if let Some(from_env) = std::env::var_os("CC") {
        return Some(from_env.to_string_lossy().into_owned());
    }
    if let Some(from_env) = std::env::var_os("CLANG") {
        return Some(from_env.to_string_lossy().into_owned());
    }
    ["cc", "clang"]
        .into_iter()
        .find(|candidate| Command::new(candidate).arg("--version").output().is_ok())
        .map(str::to_owned)
}

fn find_nm() -> Option<String> {
    if let Some(from_env) = std::env::var_os("NM") {
        return Some(from_env.to_string_lossy().into_owned());
    }
    if Command::new("nm").arg("--version").output().is_ok() || Command::new("nm").output().is_ok() {
        return Some("nm".to_owned());
    }
    None
}

/// Evidence discipline: compile the real emitted translation unit with
/// `-ffreestanding -nostdlib -c` into a relocatable object and verify with nm
/// that no undefined libc symbols remain beyond the declared allowed set
/// (`memcpy`, `strcmp`). For `i64` arithmetic on 64-bit targets no
/// compiler-rt intrinsics appear; both allowed symbols carry documented
/// justifications inside the envelope itself.
#[test]
fn relocatable_object_compiles_freestanding_with_clean_symbol_surface() {
    let Some(compiler) = find_compiler() else {
        eprintln!(
            "skipping freestanding object evidence: no cc/clang compiler available \
             (set CC or CLANG to enable)"
        );
        return;
    };
    let Some(nm) = find_nm() else {
        eprintln!(
            "skipping freestanding object evidence: no nm/llvm-nm available (set NM to enable)"
        );
        return;
    };

    let dir =
        std::env::temp_dir().join(format!("semaprax-freestanding-obj-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let source_path = dir.join("meaning.c");
    let unit =
        freestanding_object::unit_text(Path::new(MEANING_PATH), &Default::default()).expect("unit");
    std::fs::write(&source_path, &unit).expect("write unit");

    let compile = |object: &Path| {
        let mut command = Command::new(&compiler);
        if cfg!(windows) {
            // Clang otherwise writes the current time into the COFF header,
            // making identical compilations differ when they cross a second.
            command.arg("-mno-incremental-linker-compatible");
        }
        command
            .args([
                "-std=c11",
                "-O0",
                "-ffreestanding",
                "-nostdlib",
                "-fno-stack-protector",
                "-D_FORTIFY_SOURCE=0",
                "-c",
            ])
            .arg(&source_path)
            .arg("-o")
            .arg(object)
            .output()
            .expect("spawn compiler")
    };
    let object_a = dir.join("meaning-a.o");
    let object_b = dir.join("meaning-b.o");
    let first = compile(&object_a);
    assert!(
        first.status.success(),
        "emitted translation unit did not compile freestanding: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = compile(&object_b);
    assert!(second.status.success());
    assert_eq!(
        std::fs::read(&object_a).expect("object a"),
        std::fs::read(&object_b).expect("object b"),
        "object compilation must be deterministic for identical input bytes"
    );

    let symbols = Command::new(&nm).arg(&object_a).output().expect("run nm");
    assert!(symbols.status.success());
    let stdout = String::from_utf8_lossy(&symbols.stdout);

    const ALLOWED_UNDEFINED: &[&str] = &["memcpy", "strcmp"];
    let mut undefined: Vec<&str> = Vec::new();
    let mut external_defined: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 {
            continue;
        }
        let name = fields[2].trim_start_matches('_');
        if fields[1] == "U" {
            undefined.push(name);
        } else if fields[1].chars().all(|c| c.is_ascii_uppercase()) && !fields[1].is_empty() {
            external_defined.push(name.to_owned());
        }
    }
    for symbol in &undefined {
        assert!(
            ALLOWED_UNDEFINED.contains(symbol),
            "unexpected undefined symbol `{symbol}` in the freestanding object; \
             allowed set is exactly {ALLOWED_UNDEFINED:?} with envelope-documented justifications"
        );
    }
    for expected in ["spx_decl_6170702e6d61696e", "spx_decl_6d6174682e616464"] {
        assert!(
            external_defined.iter().any(|symbol| symbol == expected),
            "module symbol `{expected}` must be exported by the relocatable object"
        );
    }

    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&object_a);
    let _ = std::fs::remove_file(&object_b);
    let _ = std::fs::remove_dir(&dir);
}
