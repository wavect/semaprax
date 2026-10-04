#!/usr/bin/env python3
"""Emit a deterministic structural receipt for the linked RI-13 Project fixture."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EXAMPLES = ROOT.parent
M1_PROJECT = EXAMPLES / "ri13-m1-regex-url/project"
M2_PROJECT = EXAMPLES / "ri13-m2-record-iterator/project"
M3_PROJECT = EXAMPLES / "ri13-m3-local-http/project"
TRACKED = [
    M1_PROJECT / "semaprax.toml", M1_PROJECT / "src/app.spx", M1_PROJECT / "src/tests.spx",
    M2_PROJECT / "semaprax.toml", M2_PROJECT / "app.spx", M2_PROJECT / "tests.spx",
    M3_PROJECT / "semaprax.toml", M3_PROJECT / "src/app.spx", M3_PROJECT / "src/tests.spx",
    ROOT / "linked/Cargo.toml", ROOT / "linked/Cargo.lock", ROOT / "linked/prepare/Cargo.toml", ROOT / "linked/prepare/Cargo.lock", ROOT / "linked/build.rs", ROOT / "linked/src/bin_prepare.rs",
    ROOT / "linked/src/main.rs",
]
REQUIRED = ["regex.run", "url.run", "ri13.event", "callback.factory", "callback.advance", "ri13.m3.score"]

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def label(path): return str(path.relative_to(EXAMPLES))
def main():
    m1_manifest = (M1_PROJECT / "semaprax.toml").read_text()
    m1_source = (M1_PROJECT / "src/app.spx").read_text()
    m2_source = (M2_PROJECT / "app.spx").read_text()
    m3_manifest = (M3_PROJECT / "semaprax.toml").read_text()
    m3_source = (M3_PROJECT / "src/app.spx").read_text()
    assert 'profile = "source-local-future.v1"' not in m1_manifest
    assert 'rust_async = ["ri13.m3.score"]' in m3_manifest
    for identity in ["regex.run", "url.run"]: assert f'@id("{identity}")' in m1_source
    for identity in ["ri13.event", "callback.factory", "callback.advance"]: assert f'@id("{identity}")' in m2_source
    assert '@id("ri13.m3.score")' in m3_source
    consumer = (ROOT / "linked/Cargo.toml").read_text()
    assert 'ri06-regex-owner = { path = "generated/regex"' in consumer
    assert 'ri06-url-owner = { path = "generated/url"' in consumer
    consumer_source = (ROOT / "linked/src/main.rs").read_text()
    for fragment in [
        'ri06_regex_owner::run()',
        'ri06_url_owner::run()',
        'deserialize_spxmirrorri13event',
        '.map(callback.as_fn())',
        'SpxStatefulProxy::new',
        '.map(stateful.as_fn_mut())',
        'm3::register',
        'Ok::<i64, ()>(43)',
    ]:
        assert fragment in consumer_source
    prepare = (ROOT / "linked/prepare/Cargo.toml").read_text()
    assert 'path = "../src/bin_prepare.rs"' in prepare
    receipt = {
        "schema": "semaprax.ri13.linked-project-receipt.v2",
        "inputs": {label(path): digest(path) for path in TRACKED},
        "profiles": {"m1": "scalar-package", "m2": "source-local", "m3": "source-local-future.v1"},
        "selected_identities": REQUIRED,
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
    print(json.dumps(receipt, indent=2, sort_keys=True))
if __name__ == "__main__": main()
