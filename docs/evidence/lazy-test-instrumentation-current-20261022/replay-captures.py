"""Replay private component captures using complete real producer modules."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
for name in ["rustc", "serde-json", "deps", "scratch"]:
    parser.add_argument("--" + name, required=True)
args = parser.parse_args()
evidence = Path(__file__).resolve().parent
root = evidence.parents[2]
scratch = Path(args.scratch).resolve()
scratch.mkdir(parents=True, exist_ok=True)
version = subprocess.check_output([args.rustc, "--version"], text=True)
if not version.startswith("rustc 1.97.1 "):
    raise ValueError("replay requires the recorded Rust 1.97.1 toolchain")
expected = json.loads((evidence / "callback-captures.json").read_text())
originals = root / "docs/evidence/lazy-test-instrumentation-v1"
record = next(row for row in json.loads((originals / "sources.json").read_text())
              if row["path"] == "src/workspace_graph/forecast_observation.rs")
old = (originals / record["archive"]).read_text()
sha = lambda value: hashlib.sha256(value.encode()).hexdigest()
assert sha(old) == record["before_sha256"]
for change in reversed(record["inverse"]):
    assert old.count(change["before"]) == 1
    old = old.replace(change["before"], change["after"], 1)
assert sha(old) == record["after_sha256"] == expected["old_complete_producer_sha256"]
current = (root / record["path"]).read_text()
assert sha(current) == expected["current_complete_producer_sha256"]
for variant, prefix in [("old", old), ("current", current)]:
    suffix = (evidence / (variant + "-snapshot-suffix.rs.txt")).read_text()
    (scratch / (variant + "-snapshot.rs")).write_text(prefix + suffix)
(scratch / "driver.rs").write_bytes((evidence / "callback-driver.rs.txt").read_bytes())
binary = scratch / "callbacks"
subprocess.run([args.rustc, "--edition=2021", "-Copt-level=0", "-Cdebuginfo=0",
                str(scratch / "driver.rs"), "-L", "dependency=" + args.deps,
                "--extern", "serde_json=" + args.serde_json, "-o", str(binary)], check=True)
actual = []
for case in range(5):
    row = {"case": case}
    for variant in ["old", "current"]:
        result = subprocess.run([str(binary), variant, str(case)],
                                capture_output=True, text=True, check=True)
        captures = [json.loads(line[len("CAPTURE "):]) for line in result.stdout.splitlines()
                    if line.startswith("CAPTURE ")]
        assert len(captures) == 1
        row[variant] = captures[0]
        values = [json.loads(line) for line in result.stderr.splitlines() if line.startswith("{")]
        row[variant + "_published_passes"] = [value for value in values
                                              if value["schema"] == "semaprax.forecast-pass.v1"]
    actual.append(row)
assert actual == expected["cases"], "complete real old/current capture corpus drifted"
print("All five complete old/current callback capture corpora match the sealed local evidence.")
