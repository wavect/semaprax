#!/usr/bin/env bash
# Runs inside ri13-linux-x86_64-index-prepare.sh's disposable guest.
set -euo pipefail

trap 'status=$?; printf "RI-13 Linux Rust API index preparation failed at line %s with status %s\\n" "$LINENO" "$status" > /output/preparation-failure.txt; exit "$status"' ERR

readonly target=x86_64-unknown-linux-gnu
readonly nightly=/nightly-toolchain
readonly expected_rustdoc='rustdoc 1.101.0-nightly (c36f14571 2026-10-01)'

test "$(uname -s)" = Linux
test "$(uname -m)" = x86_64
test -n "${RI13_EXPECTED_REVISION:-}"
test -n "${RI13_IMAGE_TAG:-}"
test -n "${RI13_IMAGE_DIGEST:-}"
test "$(git rev-parse HEAD)" = "$RI13_EXPECTED_REVISION"
test -z "$(git status --porcelain)"
for executable in cargo rustc rustdoc; do
    test -f "$nightly/bin/$executable"
    test ! -L "$nightly/bin/$executable"
done
test "$("$nightly/bin/rustdoc" --version)" = "$expected_rustdoc"

mkdir /output/raw /output/envelopes /output/capture /output/target

source_root() {
    local package=$1
    python3 - /cargo-home "$package" <<'PY'
from pathlib import Path
import sys
root, package = map(Path, sys.argv[1:])
matches = list(root.glob(f"registry/src/*/{package}"))
if len(matches) != 1 or not matches[0].is_dir() or matches[0].is_symlink():
    raise SystemExit(f"expected one regular unpacked source for {package}")
print(matches[0])
PY
}

capture() {
    local package=$1 version=$2 alias=$3 lock=$4 source_hash=$5 feature_hash=$6
    local selected_one=$7 selected_two=$8
    local root="/output/capture/$package"
    mkdir "$root"
    cat > "$root/Cargo.toml" <<EOF
[package]
name = "ri06-${package}-owner"
version = "0.1.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
${alias} = { package = "${package}", version = "=${version}" }
EOF
    cp "$lock" "$root/Cargo.lock"
    CARGO_TARGET_DIR=/output/target \
        "$nightly/bin/cargo" rustdoc --locked --offline -p "$package" --lib --target "$target" \
        --manifest-path "$root/Cargo.toml" -- -Z unstable-options --output-format json
    local raw="/output/target/$target/doc/$package.json"
    test -f "$raw"
    cp "$raw" "/output/raw/$package.json"
    python3 crates/semaprax-rust-api-index/tools/rustdoc_json_to_index.py \
        --rustdoc-json "/output/raw/$package.json" \
        --package-name "$package" --package-version "$version" \
        --source-sha256 "sha256:$source_hash" --renamed-from "$alias" \
        --target "$target" --feature-digest "sha256:$feature_hash" \
        --stable-rustc-version "$(rustc --version)" \
        --source-root "$(source_root "$package-$version")" \
        --rustdoc-version "$expected_rustdoc" --rustdoc-format-version 61 \
        --select "$selected_one" --select "$selected_two" \
        --output "/output/envelopes/$package-$version-index-envelope.json"
}

capture regex 1.13.1 regex_alias \
    crates/semaprax-toolchain/src/fixtures/ri06-regex-1.13.1.Cargo.lock \
    f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d \
    dcacb5b38acb8b53818ae1c0cb2020947aefbea8ac5a9380e49aa0e0ec4db1aa \
    regex::Regex::new regex::Regex::is_match
capture url 2.5.8 url_alias \
    crates/semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock \
    ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed \
    af269ab39e76ec749dfa30da5ce5878b153b3b47b25fc5bd1a61084a929b17a1 \
    url::Url::parse url::Url::as_str

python3 - <<'PY'
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path('/output')
files = [
    'raw/regex.json', 'raw/url.json',
    'envelopes/regex-1.13.1-index-envelope.json',
    'envelopes/url-2.5.8-index-envelope.json',
]
for name, package, version, alias in [
    ('envelopes/regex-1.13.1-index-envelope.json', 'regex', '1.13.1', 'regex_alias'),
    ('envelopes/url-2.5.8-index-envelope.json', 'url', '2.5.8', 'url_alias'),
]:
    document = json.loads((root / name).read_text())
    index = document['index']
    assert document['schema'] == 'semaprax.rustdoc-extractor.v2'
    assert index['schema'] == 'semaprax.rust-api-index.v2'
    assert index['target'] == 'x86_64-unknown-linux-gnu'
    assert (index['package']['name'], index['package']['version'], index['package']['renamed_from']) == (package, version, alias)

def output(*command):
    return subprocess.check_output(command, text=True).strip()

receipt = {
    'schema': 'semaprax.ri13.linux-x86_64-rust-api-index-preparation.v1',
    'revision': (root / 'revision').read_text().strip(),
    'target': 'x86_64-unknown-linux-gnu',
    'execution': 'linux-x86_64-under-rosetta',
    'image_tag': os.environ['RI13_IMAGE_TAG'],
    'image_digest': os.environ['RI13_IMAGE_DIGEST'],
    'stable_rustc': output('rustc', '--version'),
    'nightly_rustdoc': output('/nightly-toolchain/bin/rustdoc', '--version'),
    'files': {name: 'sha256:' + hashlib.sha256((root / name).read_bytes()).hexdigest() for name in files},
}
(root / 'receipt.json').write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
PY

# The raw captures and canonical envelopes are the retained provenance. Cargo's
# intermediate target is regenerated from those locked inputs and is removed to
# keep the bounded preparation artifact small.
rm -rf -- /output/target
