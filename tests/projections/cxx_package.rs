//! Separate-translation-unit evidence for C++ scalar package v1.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::cxx_shim::{
    generate_package, verify_package_envelope, CxxShimOptions, PACKAGE_SCHEMA,
};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const SOURCE: &str = r#"
module test.cxx;

@id("cxx.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
{ left + right }

@id("cxx.mixed")
fn mixed(flag: bool, count: i64, small: u8, medium: i32, code: char, ratio: f32, precise: f64) -> f64
{ precise }

@id("cxx.bool")
fn pick_bool(value: bool) -> bool { value }

@id("cxx.u8")
fn pick_u8(value: u8) -> u8 { value }

@id("cxx.i32")
fn pick_i32(value: i32) -> i32 { value }

@id("cxx.char")
fn pick_char(value: char) -> char { value }

@id("cxx.ensure")
fn ensure_nonnegative(value: i64) -> i64
    ensures result >= 0
{ value }

@id("cxx.f32")
fn pick_f32(value: f32) -> f32 { value }

@id("cxx.overflow")
fn overflow(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "semaprax-cxx-package-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&path).expect("create unique C++ package fixture");
        Self(path)
    }

    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).expect("write C++ package fixture file");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn options() -> CxxShimOptions {
    CxxShimOptions::new(
        [
            "cxx.add",
            "cxx.bool",
            "cxx.char",
            "cxx.ensure",
            "cxx.f32",
            "cxx.i32",
            "cxx.mixed",
            "cxx.overflow",
            "cxx.u8",
        ]
        .map(str::to_owned)
        .to_vec(),
        1024 * 1024,
    )
    .expect("valid C++ package options")
}

const PACKAGE_PAYLOAD_DOMAIN: &[u8] = b"semaprax.cxx-package.payload.v1\0";
const HEADER_DOMAIN: &[u8] = b"semaprax.cxx-package.header.v1\0";
const PROVIDER_DOMAIN: &[u8] = b"semaprax.cxx-package.provider.v1\0";
const SHIM_PAYLOAD_DOMAIN: &[u8] = b"semaprax.cxx-shim.payload.v1\0";

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn remint_artifact(package: &mut serde_json::Value, name: &str, domain: &[u8]) {
    let text = package["payload"][name]["text"]
        .as_str()
        .expect("artifact text")
        .to_owned();
    package["payload"][name]["bytes"] = serde_json::json!(text.len());
    package["payload"][name]["sha256"] = serde_json::json!(digest(domain, text.as_bytes()));
}

fn append_artifact(package: &mut serde_json::Value, name: &str, suffix: &str, domain: &[u8]) {
    let mut text = package["payload"][name]["text"]
        .as_str()
        .expect("artifact text")
        .to_owned();
    text.push_str(suffix);
    package["payload"][name]["text"] = serde_json::json!(text);
    remint_artifact(package, name, domain);
}

fn remint_envelope(value: &mut serde_json::Value, domain: &[u8]) -> String {
    let payload = serde_json::to_string(&value["payload"]).expect("render reminted payload");
    value["bytes"] = serde_json::json!(payload.len());
    value["digest"] = serde_json::json!(digest(domain, payload.as_bytes()));
    serde_json::to_string(value).expect("render reminted envelope")
}

fn replace_shim(package: &mut serde_json::Value, mutate: impl FnOnce(&mut serde_json::Value)) {
    let mut shim: serde_json::Value = serde_json::from_str(
        package["payload"]["shim_envelope"]
            .as_str()
            .expect("shim envelope text"),
    )
    .expect("shim envelope JSON");
    mutate(&mut shim);
    package["payload"]["shim_envelope"] =
        serde_json::json!(remint_envelope(&mut shim, SHIM_PAYLOAD_DOMAIN));
}

fn assert_remint_rejected(mut package: serde_json::Value, source: &Path, options: &CxxShimOptions) {
    let reminted = remint_envelope(&mut package, PACKAGE_PAYLOAD_DOMAIN);
    assert!(
        verify_package_envelope(&reminted, source, options).is_err(),
        "self-consistent package remint was accepted"
    );
}

fn generated() -> (Fixture, PathBuf, String) {
    let fixture = Fixture::new();
    let source = fixture.write("subject.spx", SOURCE);
    let envelope = generate_package(&source, &options()).expect("generate C++ package");
    (fixture, source, envelope)
}

#[test]
fn package_is_deterministic_closed_and_tamper_evident() {
    let fixture = Fixture::new();
    let source = fixture.write("subject.spx", SOURCE);
    let first = generate_package(&source, &options()).expect("first package");
    let second = generate_package(&source, &options()).expect("second package");
    assert_eq!(first, second);
    let package = verify_package_envelope(&first, &source, &options()).expect("verify package");
    assert!(package.header.contains("spx_cxx_status_v1"));
    assert!(package
        .provider_c
        .starts_with("#define SPX_NO_ENTRY_WRAPPER 1\n"));
    assert!(package.shim_envelope.contains("semaprax.cxx-shim.v1"));

    for needle in ["SPX_CXX_SUCCESS_V1", "spx_cxx_call_6378782e616464"] {
        let mut changed = first.clone();
        let offset = changed.find(needle).expect("tamper anchor");
        changed.replace_range(offset..offset + 1, "X");
        assert!(verify_package_envelope(&changed, &source, &options()).is_err());
    }
    let value: serde_json::Value = serde_json::from_str(&first).unwrap();
    let mut object = value.as_object().unwrap().clone();
    object.insert("surplus".to_owned(), serde_json::Value::Bool(true));
    assert!(verify_package_envelope(
        &serde_json::to_string(&object).unwrap(),
        &source,
        &options()
    )
    .is_err());
}

#[test]
fn self_consistent_artifact_and_surplus_wrapper_remints_fail_closed() {
    let (_, source, envelope) = generated();
    let original: serde_json::Value = serde_json::from_str(&envelope).unwrap();

    for (artifact, domain, suffix) in [
        (
            "header",
            HEADER_DOMAIN,
            "\nextern \"C\" void spx_foreign_surplus(void);\n",
        ),
        (
            "provider_c",
            PROVIDER_DOMAIN,
            "\nvoid spx_foreign_surplus(void) {}\n",
        ),
    ] {
        let mut changed = original.clone();
        append_artifact(&mut changed, artifact, suffix, domain);
        assert_remint_rejected(changed, &source, &options());
    }

    let mut both = original;
    append_artifact(
        &mut both,
        "header",
        "\nspx_cxx_status_v1 spx_cxx_call_666f72676564(int64_t *);\n",
        HEADER_DOMAIN,
    );
    append_artifact(
        &mut both,
        "provider_c",
        "\nspx_cxx_status_v1 spx_cxx_call_666f72676564(int64_t *out) { *out = 7; return 0; }\n",
        PROVIDER_DOMAIN,
    );
    assert_remint_rejected(both, &source, &options());
}

#[test]
fn self_consistent_revision_source_selection_and_empty_inventory_remints_reject() {
    let (_, source_path, envelope) = generated();
    let original: serde_json::Value = serde_json::from_str(&envelope).unwrap();

    let mut revision = original.clone();
    revision["payload"]["revision"] = serde_json::json!(
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    assert_remint_rejected(revision, &source_path, &options());

    let mut source = original.clone();
    replace_shim(&mut source, |shim| {
        shim["payload"]["source"]["sha256"] = serde_json::json!(
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    });
    assert_remint_rejected(source, &source_path, &options());

    let mut selection = original.clone();
    replace_shim(&mut selection, |shim| {
        shim["payload"]["selection"]["requested"] = serde_json::json!(1);
        shim["payload"]["selection"]["admitted"] = serde_json::json!(1);
    });
    assert_remint_rejected(selection, &source_path, &options());

    let mut empty = original;
    replace_shim(&mut empty, |shim| {
        shim["payload"]["functions"] = serde_json::json!([]);
        shim["payload"]["selection"]["requested"] = serde_json::json!(0);
        shim["payload"]["selection"]["admitted"] = serde_json::json!(0);
    });
    assert_remint_rejected(empty, &source_path, &options());
}

#[test]
fn independently_valid_package_cannot_substitute_for_the_authorized_subject() {
    let (fixture, source, envelope) = generated();
    let other_source = fixture.write(
        "other.spx",
        SOURCE.replace("{ left + right }", "{ left - right }"),
    );
    let substituted =
        generate_package(&other_source, &options()).expect("independently valid other package");
    verify_package_envelope(&substituted, &other_source, &options())
        .expect("other package is internally valid for its own subject");
    assert!(verify_package_envelope(&substituted, &source, &options()).is_err());
    verify_package_envelope(&envelope, &source, &options())
        .expect("authorized package remains valid");
}

#[test]
fn package_budget_accepts_the_exact_minimum_and_rejects_one_byte_less() {
    let fixture = Fixture::new();
    let source = fixture.write("budget.spx", SOURCE);
    let mut low = 1024usize;
    let mut high = 1024 * 1024usize;
    while low < high {
        let middle = low + (high - low) / 2;
        let options = CxxShimOptions::new(vec!["cxx.add".to_owned()], middle).unwrap();
        if generate_package(&source, &options).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let exact = CxxShimOptions::new(vec!["cxx.add".to_owned()], low).unwrap();
    generate_package(&source, &exact).expect("exact minimum package budget");
    let below = CxxShimOptions::new(vec!["cxx.add".to_owned()], low - 1).unwrap();
    let errors = generate_package(&source, &below).expect_err("minimum minus one");
    assert!(errors.iter().any(|error| error.code == "SPX-X103"));
}

#[test]
fn package_source_hard_cap_rejects_before_parsing() {
    let fixture = Fixture::new();
    let source = fixture.0.join("oversized.spx");
    let file = fs::File::create(&source).expect("create oversized source witness");
    file.set_len((16 * 1024 * 1024 + 1) as u64)
        .expect("size oversized source witness");
    let errors = generate_package(&source, &options()).expect_err("oversized source");
    assert!(errors.iter().any(|error| error.code == "SPX-X103"));
}

#[test]
fn excluded_or_empty_selections_never_form_a_partial_package() {
    assert!(CxxShimOptions::new(Vec::new(), 1024 * 1024).is_err());
    let fixture = Fixture::new();
    let source = fixture.write(
        "excluded.spx",
        "module test.excluded;\n@id(\"cxx.owned\")\nfn owned(value: string) -> string { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let selected = CxxShimOptions::new(vec!["cxx.owned".to_owned()], 1024 * 1024).unwrap();
    let errors = generate_package(&source, &selected).expect_err("excluded package");
    assert!(errors.iter().any(|error| error.code == "SPX-X106"));
}

#[cfg(unix)]
#[test]
fn separate_c_and_cpp_translation_units_compile_link_and_execute() {
    let (fixture, source, envelope) = generated();
    let package =
        verify_package_envelope(&envelope, &source, &options()).expect("verified package");
    let header = fixture.write("semaprax.hpp", package.header);
    let provider = fixture.write("provider.c", package.provider_c);
    let consumer = fixture.write(
        "consumer.cpp",
        r#"#include "semaprax.hpp"
#include <cstdint>

int main() {
    std::int64_t value = 91;
    if (spx_cxx_call_6378782e616464(20, 22, &value) != SPX_CXX_SUCCESS_V1 || value != 42) return 1;
    value = 91;
    if (spx_cxx_call_6378782e616464(-1, 2, &value) != SPX_CXX_SEMANTIC_FAILURE_V1 || value != 91) return 2;
    if (spx_cxx_call_6378782e616464(1, 2, nullptr) != SPX_CXX_ADAPTER_FAILURE_V1) return 3;
    double mixed = 0.0;
    if (spx_cxx_call_6378782e6d69786564(true, 7, 8, -9, 65, 1.25f, 2.5, &mixed) != SPX_CXX_SUCCESS_V1 || mixed != 2.5) return 4;
    bool boolean = false;
    if (spx_cxx_call_6378782e626f6f6c(true, &boolean) != SPX_CXX_SUCCESS_V1 || !boolean) return 5;
    std::uint8_t small = 0;
    if (spx_cxx_call_6378782e7538(UINT8_C(251), &small) != SPX_CXX_SUCCESS_V1 || small != UINT8_C(251)) return 6;
    std::int32_t medium = 0;
    if (spx_cxx_call_6378782e693332(INT32_C(-1234567), &medium) != SPX_CXX_SUCCESS_V1 || medium != INT32_C(-1234567)) return 7;
    std::uint32_t code = 0;
    if (spx_cxx_call_6378782e63686172(UINT32_C(0x10ffff), &code) != SPX_CXX_SUCCESS_V1 || code != UINT32_C(0x10ffff)) return 8;
    code = UINT32_C(77);
    if (spx_cxx_call_6378782e63686172(UINT32_C(0xd800), &code) != SPX_CXX_ADAPTER_FAILURE_V1 || code != UINT32_C(77)) return 12;
    if (spx_cxx_call_6378782e63686172(UINT32_C(0x110000), &code) != SPX_CXX_ADAPTER_FAILURE_V1 || code != UINT32_C(77)) return 13;
    if (spx_cxx_call_6378782e63686172(UINT32_C(65), &code) != SPX_CXX_SUCCESS_V1 || code != UINT32_C(65)) return 14;
    float ratio = 0.0f;
    if (spx_cxx_call_6378782e663332(1.25f, &ratio) != SPX_CXX_SUCCESS_V1 || ratio != 1.25f) return 9;
    value = 91;
    if (spx_cxx_call_6378782e6f766572666c6f77(INT64_MAX, &value) != SPX_CXX_SEMANTIC_FAILURE_V1 || value != 91) return 10;
    value = 91;
    if (spx_cxx_call_6378782e6f766572666c6f77(INT64_C(40), &value) != SPX_CXX_SUCCESS_V1 || value != 41) return 11;
    value = 91;
    if (spx_cxx_call_6378782e656e73757265(INT64_C(-1), &value) != SPX_CXX_SEMANTIC_FAILURE_V1 || value != 91) return 15;
    if (spx_cxx_call_6378782e656e73757265(INT64_C(5), &value) != SPX_CXX_SUCCESS_V1 || value != 5) return 16;
    return 0;
}
"#,
    );
    let c_object = fixture.0.join("provider.o");
    let cpp_object = fixture.0.join("consumer.o");
    let executable = fixture.0.join("consumer");
    run(
        compiler("CC", "cc"),
        &["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-c"],
        &provider,
        &c_object,
        "C provider compile",
    );
    run(
        compiler("CXX", "c++"),
        &["-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror", "-c"],
        &consumer,
        &cpp_object,
        "C++ consumer compile",
    );
    let output = Command::new(compiler("CXX", "c++"))
        .arg(&c_object)
        .arg(&cpp_object)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("link C++ consumer");
    assert!(
        output.status.success(),
        "C++ consumer link failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Command::new(executable)
        .status()
        .expect("run consumer")
        .success());
    assert!(header.exists());
}

#[test]
fn cli_emits_only_the_canonical_envelope_and_rejects_shim_only_options() {
    let fixture = Fixture::new();
    let source = fixture.write("subject.spx", SOURCE);
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("cxx-package")
        .arg(&source)
        .args(["--function", "cxx.add", "--max-bytes", "1048576"])
        .output()
        .expect("run cxx-package");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.ends_with('\n'));
    assert_eq!(stdout.matches('\n').count(), 1);
    let envelope = stdout.strip_suffix('\n').unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(envelope).unwrap()["schema"],
        PACKAGE_SCHEMA
    );
    let cli_options =
        CxxShimOptions::new(vec!["cxx.add".to_owned()], 1024 * 1024).expect("CLI options");
    verify_package_envelope(envelope, &source, &cli_options).expect("CLI package verifies");

    let rejected = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("cxx-package")
        .arg(&source)
        .args(["--function", "cxx.add", "--emit-fragment"])
        .status()
        .expect("run rejected cxx-package");
    assert_eq!(rejected.code(), Some(2));
}

fn compiler(variable: &str, fallback: &str) -> OsString {
    std::env::var_os(variable).unwrap_or_else(|| OsString::from(fallback))
}

#[cfg(unix)]
fn run(compiler: OsString, flags: &[&str], input: &Path, output: &Path, label: &str) {
    let result = Command::new(compiler)
        .args(flags)
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert!(
        result.status.success(),
        "{label} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
