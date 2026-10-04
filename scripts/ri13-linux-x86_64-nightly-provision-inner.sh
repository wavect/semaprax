#!/usr/bin/env bash
# Runs inside ri13-linux-x86_64-nightly-provision.sh's disposable guest.
set -euo pipefail

trap 'status=$?; printf "nightly provision failed at line %s with status %s\\n" "$LINENO" "$status" > /output/provision-failure.txt; exit "$status"' ERR

readonly date=2026-10-02
readonly target=x86_64-unknown-linux-gnu
readonly expected_rustdoc='rustdoc 1.101.0-nightly (c36f14571 2026-10-01)'
readonly manifest_url="https://static.rust-lang.org/dist/${date}/channel-rust-nightly.toml"
readonly manifest_sha256=50abfdf8df57de84ff7b3188b6cad2a9681a4165518712c9e504948bf896e2b3

test "$(uname -s)" = Linux
test "$(uname -m)" = x86_64
test -n "${RI13_IMAGE_TAG:-}"
test -n "${RI13_IMAGE_DIGEST:-}"

python3 - <<'PY'
import hashlib
from pathlib import Path
import tarfile

root = Path('/cargo-home')
expected = {
    'regex-1.13.1': 'f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d',
    'url-2.5.8': 'ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed',
}
for package, digest in expected.items():
    archives = list(root.glob(f'registry/cache/*/{package}.crate'))
    sources = list(root.glob(f'registry/src/*/{package}'))
    assert len(archives) == 1 and archives[0].is_file()
    assert len(sources) == 1 and sources[0].is_dir() and not sources[0].is_symlink()
    assert hashlib.sha256(archives[0].read_bytes()).hexdigest() == digest
    expected_files = {}
    with tarfile.open(archives[0], 'r:*') as archive:
        for member in archive.getmembers():
            if member.isdir():
                continue
            assert member.isfile() and not member.issym() and not member.islnk()
            relative = Path(member.name).relative_to(package)
            stream = archive.extractfile(member)
            assert stream is not None
            expected_files[relative.as_posix()] = hashlib.sha256(stream.read()).hexdigest()
    actual_files = {}
    for path in sources[0].rglob('*'):
        relative = path.relative_to(sources[0]).as_posix()
        if relative in {'.cargo-ok', '.cargo-checksum.json'}:
            continue
        if path.is_dir():
            assert not path.is_symlink()
            continue
        assert path.is_file() and not path.is_symlink()
        actual_files[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    assert actual_files == expected_files
PY

mkdir /output/toolchain /output/manifest /tmp/ri13-downloads /tmp/ri13-extract
python3 - "$manifest_url" "$manifest_sha256" <<'PY'
import hashlib
from pathlib import Path
import sys
from urllib.request import urlopen

url, expected = sys.argv[1:]
with urlopen(url) as response:
    data = response.read()
observed = hashlib.sha256(data).hexdigest()
if observed != expected:
    raise SystemExit(f'nightly manifest checksum mismatch: expected {expected}, got {observed}')
Path('/output/manifest/channel-rust-nightly.toml').write_bytes(data)
PY

component() {
    local package=$1 expected=$2
    local archive="/tmp/ri13-downloads/${package}.tar.xz"
    local extract="/tmp/ri13-extract/${package}"
    python3 - "$package" "$expected" "$archive" <<'PY'
import hashlib
from pathlib import Path
import sys
from urllib.request import urlopen

package, expected, destination = sys.argv[1:]
url = f'https://static.rust-lang.org/dist/2026-10-02/{package}-nightly-x86_64-unknown-linux-gnu.tar.xz'
with urlopen(url) as response:
    data = response.read()
observed = hashlib.sha256(data).hexdigest()
if observed != expected:
    raise SystemExit(f'{package} checksum mismatch: expected {expected}, got {observed}')
Path(destination).write_bytes(data)
PY
    mkdir "$extract"
    tar -xJf "$archive" -C "$extract"
    installer=$(find "$extract" -mindepth 2 -maxdepth 2 -type f -name install.sh)
    test "$(printf '%s\n' "$installer" | sed '/^$/d' | wc -l | tr -d ' ')" = 1
    sh "$installer" --prefix /output/toolchain
}

component rustc 8e326ba2de1664a1c2f6a53ae6f5d03dffbb68cfe8c2fd4db4daf09cbb4f9257
component cargo 8f93add5b0bc7e50a60caa18467fbbafb35220aadf45859404fa4c1c612fe64f
component rust-std 988d9dc0250988b5cb20dbf91ace6a623096f6b51af8c158edcce1a1f6ea2131

for executable in cargo rustc rustdoc; do
    test -f "/output/toolchain/bin/$executable"
    test ! -L "/output/toolchain/bin/$executable"
done
test "$(/output/toolchain/bin/rustdoc --version)" = "$expected_rustdoc"

python3 - <<'PY'
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path('/output')
toolchain = root / 'toolchain'
def output(*command):
    return subprocess.check_output(command, text=True).strip()

receipt = {
    'schema': 'semaprax.ri13.linux-x86_64-nightly-provision.v1',
    'target': 'x86_64-unknown-linux-gnu',
    'execution': 'linux-x86_64-under-rosetta',
    'image_tag': os.environ['RI13_IMAGE_TAG'],
    'image_digest': os.environ['RI13_IMAGE_DIGEST'],
    'nightly_date': '2026-10-02',
    'nightly_manifest_sha256': 'sha256:50abfdf8df57de84ff7b3188b6cad2a9681a4165518712c9e504948bf896e2b3',
    'components': {
        'rustc': 'sha256:8e326ba2de1664a1c2f6a53ae6f5d03dffbb68cfe8c2fd4db4daf09cbb4f9257',
        'cargo': 'sha256:8f93add5b0bc7e50a60caa18467fbbafb35220aadf45859404fa4c1c612fe64f',
        'rust-std': 'sha256:988d9dc0250988b5cb20dbf91ace6a623096f6b51af8c158edcce1a1f6ea2131',
    },
    'locked_sources': {
        'regex-1.13.1.crate': 'sha256:f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d',
        'url-2.5.8.crate': 'sha256:ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed',
    },
    'cargo_version': output(toolchain / 'bin/cargo', '--version'),
    'rustc_version': output(toolchain / 'bin/rustc', '--version'),
    'rustdoc_version': output(toolchain / 'bin/rustdoc', '--version'),
    'toolchain_files': {},
}
for name in ('cargo', 'rustc', 'rustdoc'):
    path = toolchain / 'bin' / name
    receipt['toolchain_files'][f'bin/{name}'] = 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()
(root / 'receipt.json').write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
PY
