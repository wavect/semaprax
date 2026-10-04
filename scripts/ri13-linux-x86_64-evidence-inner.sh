#!/usr/bin/env bash
# Runs inside the disposable container started by ri13-linux-x86_64-evidence.sh.
# This is an emulated x86_64 guest on M1/M2/M3 Apple silicon: no performance
# result from this route is interpreted as native Apple-silicon performance.
set -euo pipefail

test "$(uname -s)" = Linux
test "$(uname -m)" = x86_64
test -n "${RI13_EXPECTED_REVISION:-}"
test -n "${RI13_IMAGE_TAG:-}"
test -n "${RI13_IMAGE_DIGEST:-}"
test "${RI13_RUST_API_INDEX_DIR:-}" = /rust-api-index
test -f "$RI13_RUST_API_INDEX_DIR/regex-1.13.1-index-envelope.json"
test -f "$RI13_RUST_API_INDEX_DIR/url-2.5.8-index-envelope.json"
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
import os
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
    "image_tag": os.environ["RI13_IMAGE_TAG"],
    "image_digest": os.environ["RI13_IMAGE_DIGEST"],
    "rustc": output("rustc", "--version"),
    "cargo": output("cargo", "--version"),
    "clang": output("/usr/bin/clang", "--version").splitlines()[0],
}, indent=2, sort_keys=True))
PY

python3 examples/ri13-combined-app/measure.py \
    --fresh-target \
    --warm-stage-pass \
    --target-dir "$combined_target" \
    --evidence-dir /evidence/combined-raw \
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
import hashlib
import json
import subprocess
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
    "m3_negative_controls", "linked_prepare", "linked_consumer",
]
assert [stage["stage"] for stage in combined["warm_build_and_consumer_stages"]] == [
    "m1_prepare", "m1_consumer", "m2_prepare", "m2_consumer", "m3_prepare", "m3_consumer",
    "m3_negative_controls", "linked_prepare", "linked_consumer",
]
assert combined["build_stage_measurement"]["schema"] == "semaprax.ri13.build-stage-measurement.v1"
assert combined["build_stage_measurement"]["cold"]["counts"] == {"passed": 9, "failed": 0, "skipped": 0}
assert combined["build_stage_measurement"]["warm"]["status"] == "passed"
assert combined["build_stage_measurement"]["warm"]["counts"] == {"passed": 9, "failed": 0, "skipped": 0}
assert combined["result_totals"]["schema"] == "semaprax.ri13.result-totals.v1"
assert combined["result_totals"]["executed_total"] == {"passed": 22, "failed": 0, "skipped": 0}
raw = combined["raw_artifacts"]
assert raw["schema"] == "semaprax.ri13.raw-artifacts.v1"
assert raw["directory"] == "/evidence/combined-raw"
assert raw["max_bytes"] == 16 * 1024 * 1024
assert len(raw["files"]) == 44
assert raw["total_bytes"] == sum(item["bytes"] for item in raw["files"])
assert raw["total_bytes"] <= raw["max_bytes"]
for item in raw["files"]:
    candidate = root / "combined-raw" / item["path"]
    assert candidate.is_file() and not candidate.is_symlink()
    assert candidate.stat().st_size == item["bytes"]
    assert "sha256:" + hashlib.sha256(candidate.read_bytes()).hexdigest() == item["sha256"]
subprocess.check_call([
    "python3", "examples/ri13-combined-app/measure.py", "--verify-raw-artifacts",
    str(root / "combined-receipt.json"),
])
inventory = combined["generated_code_inventory"]
assert inventory["schema"] == "semaprax.ri13.generated-code-inventory.v1"
assert set(inventory["groups"]) == {"m1", "m2", "m3", "linked"}
assert inventory["total_bytes"] > 0
assert all(
    item["bytes"] > 0 and item["sha256"].startswith("sha256:")
    for group in inventory["groups"].values()
    for item in group["files"]
)
assert set(combined["batch_throughput"]["routes"]) == {
    "direct_rust", "handwritten_adapter", "generated_semaprax",
}
linked = json.loads((root / "linked-receipt.json").read_text())
assert linked["schema"] == "semaprax.ri13.linked-project-receipt.v2"
assert linked["stages"] == ["prepare", "consumer"]
assert "ri13-linked-project-ok" in (root / "linked-consumer.log").read_text()

# Promote the executed combined ledger into the Linux receipt without
# reclassifying foreign-library work as an observed adapter copy.
copy = combined["linked_copy_ledger"]
regex = copy["m1"]["regex_result_owner"]
url = copy["m1"]["url_owner_view"]
record = copy["m2"]["serde_record"]
callback = copy["m2"]["iterator_callback"]
assert (regex["status"], regex["adapter_copy_events"], regex["adapter_copied_bytes"], regex["adapter_borrowed_scan_input_bytes"], regex["borrow_matches_target"]) == ("measured", 0, 0, 28, True)
assert (url["status"], url["adapter_copy_events"], url["adapter_copied_bytes"], url["borrow_matches_target"]) == ("measured", 0, 0, True)
assert record["input_json_bytes"] == record["output_json_bytes"] == 25
assert record["generated_mirror_string_clone_copied_bytes"] == 3
assert callback == {"fn_invocations": 1, "fn_mut_invocations": 1, "scalar_argument_result_copied_bytes": 0}
for unavailable in (
    regex["foreign_target_copied_bytes"],
    url["foreign_target_copied_bytes"],
    record["deserialize_owned_string_copied_bytes"],
):
    assert unavailable["status"] == "unavailable"
    assert isinstance(unavailable["reason"], str) and unavailable["reason"]
m3 = combined["m3_copy_ledger"]
assert m3["schema"] == "semaprax.ri13.m3-copy-ledger.v1"
assert "generated i64 boundary" in m3["exact_copy_domains"]

copy_accounting = {
    "schema": "semaprax.ri13.linux-x86_64-copy-accounting.v1",
    "m1_regex_buffer_scan": {
        "adapter": {
            "status": "exact",
            "copy_events": regex["adapter_copy_events"],
            "copied_bytes": regex["adapter_copied_bytes"],
            "borrowed_input_bytes": regex["adapter_borrowed_scan_input_bytes"],
        },
        "foreign_regex": regex["foreign_target_copied_bytes"],
    },
    "m1_url_ownership": {
        "adapter": {
            "status": "exact",
            "copy_events": url["adapter_copy_events"],
            "copied_bytes": url["adapter_copied_bytes"],
        },
        "foreign_url": url["foreign_target_copied_bytes"],
    },
    "m2_generic_record": {
        "generated_mirror": {
            "status": "exact",
            "input_json_bytes": record["input_json_bytes"],
            "output_json_bytes": record["output_json_bytes"],
            "string_clone_copied_bytes": record["generated_mirror_string_clone_copied_bytes"],
        },
        "foreign_deserialization": record["deserialize_owned_string_copied_bytes"],
    },
    "m2_iterator_callback": {
        "status": "exact",
        **callback,
    },
    "m3_foreign_http_and_text": {
        "status": "unavailable",
        "domains": m3["unmeasured_copy_domains"],
    },
}

files = [
    "environment.json", "combined-receipt.json", "combined.log",
    "combined-self-test.log", "linked-receipt-self-test.log",
    "linked-receipt.json", "linked-prepare.log", "linked-consumer.log", "revision",
]
files.extend(f"combined-raw/{item['path']}" for item in raw["files"])
assert len(files) == len(set(files))
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
    "container": {"architecture": "amd64", "rosetta": True, "memory": "6G", "network": "none", "image_tag": environment["image_tag"], "image_digest": environment["image_digest"]},
    "cargo": {"offline": True, "jobs": 1, "target_root": "/evidence/target"},
    "stages": [stage["stage"] for stage in combined["full_build_and_consumer_stages"]],
    "linked_check": "ri13-linked-project-ok",
    "performance_claim": "none",
    "copy_accounting": copy_accounting,
    "combined_raw_artifacts": raw,
    "output_digests": json.loads((root / "output-digests.json").read_text())["sha256"],
}, indent=2, sort_keys=True) + "\n")
PY
