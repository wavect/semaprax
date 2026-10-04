#!/usr/bin/env bash
# Runs inside the disposable container started by ri13-linux-x86_64-evidence.sh.
# This is an emulated x86_64 guest on M1/M2/M3 Apple silicon: no performance
# result from this route is interpreted as native Apple-silicon performance.
set -euo pipefail

test "$(uname -s)" = Linux
test "$(uname -m)" = x86_64
test -n "${RI13_EXPECTED_REVISION:-}"
test "$(git rev-parse HEAD)" = "$RI13_EXPECTED_REVISION"
test -z "$(git status --porcelain)"
test -x /usr/bin/clang
command -v cargo >/dev/null
command -v python3 >/dev/null

combined_target=/evidence/target/combined
linked_target=/evidence/target/linked
test ! -e "$combined_target"
test ! -e "$linked_target"

python3 examples/ri13-combined-app/measure.py --self-test --output /evidence/unused-measurement.json \
    > /evidence/combined-self-test.log
python3 examples/ri13-combined-app/linked-receipt.py --self-test \
    > /evidence/linked-receipt-self-test.log

python3 - <<'PY' > /evidence/environment.json
import json
import platform
import subprocess

def output(*command):
    return subprocess.check_output(command, text=True).strip()

print(json.dumps({
    "schema": "semaprax.ri13.linux-x86_64-environment.v1",
    "revision": output("git", "rev-parse", "HEAD"),
    "system": platform.system(),
    "machine": platform.machine(),
    "execution": "linux-x86_64-under-rosetta",
    "rustc": output("rustc", "--version"),
    "cargo": output("cargo", "--version"),
    "clang": output("/usr/bin/clang", "--version").splitlines()[0],
}, indent=2, sort_keys=True))
PY

python3 examples/ri13-combined-app/measure.py \
    --fresh-target \
    --target-dir "$combined_target" \
    --output /evidence/combined-receipt.json \
    > /evidence/combined.log 2>&1

CARGO_TARGET_DIR="$linked_target" cargo run --locked --offline \
    --manifest-path examples/ri13-combined-app/linked/prepare/Cargo.toml \
    --bin prepare > /evidence/linked-prepare.log 2>&1
CARGO_TARGET_DIR="$linked_target" cargo run --locked --offline \
    --manifest-path examples/ri13-combined-app/linked/Cargo.toml \
    --bin consumer > /evidence/linked-consumer.log 2>&1
python3 examples/ri13-combined-app/linked-receipt.py > /evidence/linked-receipt.json

python3 - <<'PY'
import json
from pathlib import Path

root = Path("/evidence")
environment = json.loads((root / "environment.json").read_text())
assert environment["system"] == "Linux"
assert environment["machine"] == "x86_64"
assert environment["execution"] == "linux-x86_64-under-rosetta"
assert environment["revision"] == (root / "revision").read_text().strip()
combined = json.loads((root / "combined-receipt.json").read_text())
assert [stage["stage"] for stage in combined["full_build_and_consumer_stages"]] == [
    "m1_prepare", "m1_consumer", "m2_prepare", "m2_consumer", "m3_prepare", "m3_consumer",
    "linked_prepare", "linked_consumer", "m3_route_measurement", "m3_batch_throughput_measurement",
]
assert set(combined["batch_throughput"]["routes"]) == {
    "direct_rust", "handwritten_adapter", "generated_semaprax",
}
linked = json.loads((root / "linked-receipt.json").read_text())
assert linked["schema"] == "semaprax.ri13.linked-project-receipt.v2"
assert linked["stages"] == ["prepare", "consumer"]
assert "ri13-linked-project-ok" in (root / "linked-consumer.log").read_text()

import hashlib
files = [
    "environment.json", "combined-receipt.json", "combined.log",
    "combined-self-test.log", "linked-receipt-self-test.log",
    "linked-receipt.json", "linked-prepare.log", "linked-consumer.log", "revision",
]
(root / "output-digests.json").write_text(json.dumps({
    "schema": "semaprax.ri13.linux-x86_64-output-digests.v1",
    "sha256": {
        name: hashlib.sha256((root / name).read_bytes()).hexdigest()
        for name in files
    },
}, indent=2, sort_keys=True) + "\n")

(root / "receipt.json").write_text(json.dumps({
    "schema": "semaprax.ri13.linux-x86_64-receipt.v1",
    "revision": environment["revision"],
    "guest": {"system": environment["system"], "machine": environment["machine"]},
    "container": {"architecture": "amd64", "rosetta": True, "memory": "4G", "network": "none"},
    "cargo": {"offline": True, "jobs": 1, "target_root": "/evidence/target"},
    "stages": [stage["stage"] for stage in combined["full_build_and_consumer_stages"]],
    "linked_check": "ri13-linked-project-ok",
    "performance_claim": "none",
    "output_digests": json.loads((root / "output-digests.json").read_text())["sha256"],
}, indent=2, sort_keys=True) + "\n")
PY
