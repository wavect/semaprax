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


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def label(path):
    return str(path.relative_to(EXAMPLES))


def read_sources():
    return {name: path.read_text(encoding="utf-8") for name, path in PATHS.items()}


def require(sources, name, fragment):
    if fragment not in sources[name]:
        raise ValueError(f"linked RI-13 {name} is missing {fragment!r}")


def validate_sources(sources):
    """Bind all structural claims to the separate M1, M2, and M3 inputs."""
    if 'profile = "source-local-future.v1"' in sources["m1_manifest"]:
        raise ValueError("M1 must not claim the M3 source-local-future profile")
    require(sources, "m3_manifest", 'rust_async = ["ri13.m3.score"]')
    for identity in ("regex.run", "url.run"):
        require(sources, "m1_source", f'@id("{identity}")')
    for identity in ("ri13.event", "callback.factory", "callback.advance"):
        require(sources, "m2_source", f'@id("{identity}")')
    require(sources, "m3_source", '@id("ri13.m3.score")')

    for fragment in (
        'let m1_project = examples.join("ri13-m1-regex-url/project")',
        "prepare_indexed_regex_url_project_packages(",
        "&m1_project.join(\"semaprax.toml\")",
        '"regex.run"',
        '"url.run"',
        "with_authenticated_project(&m2_project.join(\"semaprax.toml\"),",
        "prepare_native_rust_serde_iterator_callbacks(",
        '"ri13.event"',
        '"callback.factory"',
        '"callback.advance"',
        "with_authenticated_project(&m3_project.join(\"semaprax.toml\"),",
        "snapshot.render_source_local_future_rust_module()",
        'root.join("generated/m3.rs")',
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
    ):
        require(sources, "consumer_cargo", fragment)
    for fragment in (
        'root.join("generated/regex/src/regex_project.c")',
        'root.join("generated/url/src/url_project.c")',
        'root.join("generated/m2/module.c")',
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
        "m3::register",
        "Ok::<i64, ()>(43)",
        ".call_typed(41, 10_000)",
        "runtime.block_on(call).unwrap(), 84",
        "ri13-linked-copy-ledger:",
        'println!("ri13-linked-project-ok")',
    ):
        require(sources, "consumer", fragment)


def receipt(sources):
    validate_sources(sources)
    return {
        "schema": "semaprax.ri13.linked-project-receipt.v2",
        "inputs": {label(path): digest(path) for path in TRACKED},
        "profiles": {
            "m1": "scalar-package",
            "m2": "source-local",
            "m3": "source-local-future.v1",
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
    assert document["copied_byte_ledger"]["m3_generated_boundary"] == {
        "status": "exact",
        "copied_bytes_per_invocation": 0,
        "shape": "i64-to-i64",
    }
    for name, fragment in (
        ("prepare", "prepare_native_rust_serde_iterator_callbacks("),
        ("consumer", "m3::register"),
        ("m1_source", '@id("regex.run")'),
    ):
        mutant = dict(sources)
        mutant[name] = mutant[name].replace(fragment, "", 1)
        try:
            validate_sources(mutant)
        except ValueError:
            continue
        raise AssertionError(f"validator accepted mutant missing {fragment!r}")
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
