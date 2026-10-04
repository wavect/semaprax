#!/usr/bin/env python3
"""Emit a deterministic structural receipt for the linked RI-13 Project fixture."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT / "project"
TRACKED = [
    PROJECT / "semaprax.toml", PROJECT / "src/app.spx", PROJECT / "src/tests.spx",
    ROOT / "linked/Cargo.toml", ROOT / "linked/Cargo.lock", ROOT / "linked/prepare/Cargo.toml", ROOT / "linked/prepare/Cargo.lock", ROOT / "linked/build.rs", ROOT / "linked/src/bin_prepare.rs",
    ROOT / "linked/src/main.rs",
]
REQUIRED = ["regex.run", "url.run", "ri13.event", "callback.factory", "callback.advance", "ri13.m3.score"]

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    manifest = (PROJECT / "semaprax.toml").read_text()
    source = (PROJECT / "src/app.spx").read_text()
    assert 'profile = "source-local-future.v1"' in manifest
    assert 'web = ["regex.run", "url.run"]' in manifest
    assert 'rust_async = ["ri13.m3.score"]' in manifest
    for identity in REQUIRED: assert f'@id("{identity}")' in source
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
    ]:
        assert fragment in consumer_source
    prepare = (ROOT / "linked/prepare/Cargo.toml").read_text()
    assert 'path = "../src/bin_prepare.rs"' in prepare
    receipt = {"schema": "semaprax.ri13.linked-project-receipt.v1", "inputs": {str(path.relative_to(ROOT)): digest(path) for path in TRACKED}, "project_profile": "source-local-future.v1", "selected_identities": REQUIRED, "stages": ["prepare", "consumer"], "consumer_marker": "ri13-linked-project-ok"}
    print(json.dumps(receipt, indent=2, sort_keys=True))
if __name__ == "__main__": main()
