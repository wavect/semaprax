#!/usr/bin/env python3
"""Validate and describe the linked RI-13 fixture without starting Cargo."""

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent
EXAMPLES = ROOT.parent
M1_PROJECT = EXAMPLES / "ri13-m1-regex-url/project"
M2_PROJECT = EXAMPLES / "ri13-m2-record-iterator/project"
M3_PROJECT = EXAMPLES / "ri13-m3-local-http/project"
UNIFIED_PROJECT = ROOT / "unified-project"
M1_REGEX_INDEX = EXAMPLES.parent / "crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
M1_URL_INDEX = EXAMPLES.parent / "crates/semaprax-rust-api-index/fixtures/url-2.5.8-index-envelope.json"
M1_REGEX_LOCK = EXAMPLES.parent / "crates/semaprax-toolchain/src/fixtures/ri06-regex-1.13.1.Cargo.lock"
M1_URL_LOCK = EXAMPLES.parent / "crates/semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock"
PATHS = {
    "m1_manifest": M1_PROJECT / "semaprax.toml",
    "m1_source": M1_PROJECT / "src/app.spx",
    "m1_tests": M1_PROJECT / "src/tests.spx",
    "m2_manifest": M2_PROJECT / "semaprax.toml",
    "m2_source": M2_PROJECT / "app.spx",
    "m2_tests": M2_PROJECT / "tests.spx",
    "m3_manifest": M3_PROJECT / "semaprax.toml",
    "m3_source": M3_PROJECT / "src/app.spx",
    "m3_tests": M3_PROJECT / "src/tests.spx",
    "unified_manifest": UNIFIED_PROJECT / "semaprax.toml",
    "unified_source": UNIFIED_PROJECT / "src/app.spx",
    "unified_tests": UNIFIED_PROJECT / "src/tests.spx",
    "m1_regex_index": M1_REGEX_INDEX,
    "m1_url_index": M1_URL_INDEX,
    "m1_regex_lock": M1_REGEX_LOCK,
    "m1_url_lock": M1_URL_LOCK,
    "consumer_cargo": ROOT / "linked/Cargo.toml",
    "consumer_lock": ROOT / "linked/Cargo.lock",
    "prepare_cargo": ROOT / "linked/prepare/Cargo.toml",
    "prepare_lock": ROOT / "linked/prepare/Cargo.lock",
    "build": ROOT / "linked/build.rs",
    "prepare": ROOT / "linked/src/bin_prepare.rs",
    "consumer": ROOT / "linked/src/main.rs",
}
TRACKED = tuple(PATHS.values())
REQUIRED_IDENTITIES = (
    "regex.run",
    "url.run",
    "ri13.event",
    "callback.factory",
    "callback.advance",
    "ri13.m3.score",
)
LOCK_DIGESTS = {
    "m1_regex_lock": "133955c5b309a56339b5cbc0210b01a4d0899c2ef58700c4216fe0bdc1d7e287",
    "m1_url_lock": "b1cfca6e929aeab1558a9693230853318ad3ced4934b663a7e1b1bdd433d9fb5",
}
CALLBACK_ADVANCE = """@id(\"callback.advance\")
fn advance(state: i64, value: i64) -> i64
    requires value >= 0
{
    state + value
}"""


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def label(path):
    if path.is_relative_to(EXAMPLES):
        return str(path.relative_to(EXAMPLES))
    return str(path.relative_to(EXAMPLES.parent))


def read_sources():
    return {name: path.read_text(encoding="utf-8") for name, path in PATHS.items()}


def require(sources, name, fragment):
    if fragment not in sources[name]:
        raise ValueError(f"linked RI-13 {name} is missing {fragment!r}")


def validate_sources(sources):
    """Bind the linked M1/M2/M3 route to its selected Project inputs."""
    if 'profile = "source-local-future.v1"' in sources["m1_manifest"]:
        raise ValueError("M1 must not claim the M3 source-local-future profile")
    require(sources, "m3_manifest", 'rust_async = ["ri13.m3.score"]')
    for identity in ("regex.run", "url.run"):
        require(sources, "m1_source", f'@id("{identity}")')
    for identity in ("ri13.event", "callback.factory", "callback.advance"):
        require(sources, "m2_source", f'@id("{identity}")')
    require(sources, "m3_source", '@id("ri13.m3.score")')
    require(sources, "unified_manifest", 'profile = "source-local-future-indexed-rust.v1"')
    require(sources, "unified_manifest", "[rust-dependencies]")
    for identity in REQUIRED_IDENTITIES:
        require(sources, "unified_source", f'@id("{identity}")')
    for index in ("m1_regex_index", "m1_url_index"):
        require(sources, index, '"target":"aarch64-apple-darwin"')
    for name, expected in LOCK_DIGESTS.items():
        if hashlib.sha256(sources[name].encode()).hexdigest() != expected:
            raise ValueError(f"linked RI-13 {name} drifted from its selected package lock")
    require(sources, "unified_source", CALLBACK_ADVANCE)
    for interface in ("RegexHost", "UrlHost"):
        require(sources, "unified_source", f"interface {interface}\n    permits {{  }}")

    indexed_snapshot = 'with_authenticated_indexed_regex_url_project_packages('
    if sources["prepare"].count(indexed_snapshot) != 1:
        raise ValueError("linked preparation must derive M1, M2, and M3 from one indexed snapshot")
    for fragment in (
        'let unified = root.parent().unwrap().join("unified-project")',
        "with_authenticated_indexed_regex_url_project_packages(",
        "&unified.join(\"semaprax.toml\")",
        '"regex.run"',
        '"url.run"',
        indexed_snapshot,
        "prepare_native_rust_serde_iterator_callbacks_from_authenticated_project_source(",
        '"ri13.event"',
        '"callback.factory"',
        '"callback.advance"',
        "let (m1, m2, m3, project_revision) = with_authenticated_indexed_regex_url_project_packages(",
        "m1.subject_digest, project_revision, m2.source_revision, project_revision,",
        "snapshot.render_source_local_future_rust_module()",
        'root.join("generated/m3.rs")',
        'root.join("generated/linked-subject.json")',
        "semaprax.ri13.linked-subject.v1",
        'let Some(directory) = env::var_os("RI13_RUST_API_INDEX_DIR")',
        'admit_linux_index("regex-1.13.1-index-envelope.json", REGEX_INDEX)',
        'admit_linux_index("url-2.5.8-index-envelope.json", URL_INDEX)',
        "LINUX_X86_64_TARGET",
    ):
        require(sources, "prepare", fragment)

    for fragment in (
        'name = "prepare"',
        'path = "../src/bin_prepare.rs"',
    ):
        require(sources, "prepare_cargo", fragment)
    for fragment in (
        'name = "consumer"',
        'path = "src/main.rs"',
        'ri06-regex-owner = { path = "generated/regex"',
        'ri06-url-owner = { path = "generated/url"',
        'semaprax-native-rust-interop = { path = "../../../crates/semaprax-native-rust-interop-builder"',
        'semaprax-rust-api-index = { path = "../../../crates/semaprax-rust-api-index"',
    ):
        require(sources, "consumer_cargo", fragment)
    for fragment in (
        'root.join("generated/regex/src/regex_project.c")',
        'root.join("generated/url/src/url_project.c")',
        'root.join("generated/m2/module.c")',
        'root.join("generated/linked-subject.json")',
        '\\"project_revision\\": \\"sha256:',
    ):
        require(sources, "build", fragment)

    for fragment in (
        "ri06_regex_owner::run(), Ok(41)",
        "ri06_url_owner::run(), Ok(41)",
        "spx_result_owner_adapter_copied_bytes(), 0",
        "adapter_copied_bytes(), 0",
        ".map(callback.as_fn())",
        "SpxStatefulProxy::new",
        ".map(stateful.as_fn_mut())",
        "assert_eq!(states, [11, 13])",
        "with_authenticated_indexed_regex_url_project(manifest,",
        "m3::register",
        'join("unified-project/semaprax.toml")',
        "Ok::<i64, ()>(43)",
        ".call_typed(41, 10_000)",
        "runtime.block_on(call).unwrap(), 84",
        "ri13-linked-copy-ledger:",
        'println!("ri13-linked-project-ok")',
    ):
        require(sources, "consumer", fragment)
    if "ri13-m3-local-http/project/semaprax.toml" in sources["consumer"]:
        raise ValueError("linked consumer must retain the unified M3 Project revision")


def receipt(sources):
    validate_sources(sources)
    return {
        "schema": "semaprax.ri13.linked-project-receipt.v2",
        "inputs": {label(path): digest(path) for path in TRACKED},
        "profiles": {"linked": "source-local-future-indexed-rust.v1"},
        "m1_index_target": {
            "target": "aarch64-apple-darwin",
            "admission": "host-native-only",
            "linux_result": "SPX-B112",
            "reason": "the pinned Rust API indexes are target-specific and the package generator requires the current native target",
        },
        "unified_project_candidate": {
            "path": "ri13-combined-app/unified-project/semaprax.toml",
            "admission": "closed",
            "diagnostic": None,
            "reason": "one Project admits only the exact Regex/Url dependency and export shape; untrusted combinations remain refused",
        },
        "selected_identities": list(REQUIRED_IDENTITIES),
        "stages": ["prepare", "consumer"],
        "consumer_marker": "ri13-linked-project-ok",
        "copied_byte_ledger": {
            "schema": "semaprax.ri13.linked-copy-ledger.v1",
            "m3_generated_boundary": {
                "status": "exact",
                "copied_bytes_per_invocation": 0,
                "shape": "i64-to-i64",
            },
            "m3_host_callback_payload": {
                "status": "exact",
                "copied_bytes_per_invocation": 0,
                "shape": "i64-to-Future<Result<i64,()>>",
            },
            "m3_foreign_http_body": {
                "status": "not_exercised",
                "copied_bytes_per_invocation": None,
                "reason": "the linked consumer's scalar callback performs no HTTP body transfer",
            },
        },
    }


def self_test():
    sources = read_sources()
    document = receipt(sources)
    assert document["schema"] == "semaprax.ri13.linked-project-receipt.v2"
    assert document["stages"] == ["prepare", "consumer"]
    assert document["consumer_marker"] == "ri13-linked-project-ok"
    assert document["unified_project_candidate"]["admission"] == "closed"
    assert document["copied_byte_ledger"]["m3_generated_boundary"] == {
        "status": "exact",
        "copied_bytes_per_invocation": 0,
        "shape": "i64-to-i64",
    }
    for name, fragment in (
        ("prepare", "prepare_native_rust_serde_iterator_callbacks_from_authenticated_project_source("),
        ("prepare", 'let Some(directory) = env::var_os("RI13_RUST_API_INDEX_DIR")'),
        ("prepare", 'with_authenticated_indexed_regex_url_project_packages('),
        ("consumer", 'with_authenticated_indexed_regex_url_project(manifest,'),
        ("build", '\\"project_revision\\": \\"sha256:'),
        ("consumer", "m3::register"),
        ("consumer", 'join("unified-project/semaprax.toml")'),
        ("m1_source", '@id("regex.run")'),
        ("unified_source", CALLBACK_ADVANCE),
        ("unified_source", "interface RegexHost\n    permits {  }"),
        ("prepare", "m1.subject_digest, project_revision, m2.source_revision, project_revision,"),
    ):
        mutant = dict(sources)
        mutant[name] = mutant[name].replace(fragment, "", 1)
        try:
            validate_sources(mutant)
        except ValueError:
            continue
        raise AssertionError(f"validator accepted mutant missing {fragment!r}")
    mutant = dict(sources)
    mutant["consumer"] = mutant["consumer"].replace(
        "unified-project/semaprax.toml",
        "ri13-m3-local-http/project/semaprax.toml",
        1,
    )
    try:
        validate_sources(mutant)
    except ValueError:
        pass
    else:
        raise AssertionError("validator accepted a consumer retaining the standalone M3 Project")
    for name in LOCK_DIGESTS:
        mutant = dict(sources)
        mutant[name] = mutant[name].replace("version", "drifted-version", 1)
        try:
            validate_sources(mutant)
        except ValueError:
            continue
        raise AssertionError(f"validator accepted drifted {name}")
    print("ri13-linked-receipt-self-test-ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        self_test()
        return
    print(json.dumps(receipt(read_sources()), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
