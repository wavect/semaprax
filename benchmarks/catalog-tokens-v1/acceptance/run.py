"""Independent byte-exact Catalog23 acceptance; never dispatches a model."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

BENCHMARK = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BENCHMARK))
from oracle import expected

SCHEMA = "semaprax.catalog-acceptance.v1"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--command-json", required=True)
    parser.add_argument("--report-json", required=True)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    command = json.loads(args.command_json)
    if not isinstance(command, list) or not command or not all(isinstance(x, str) and x for x in command):
        parser.error("command must be a nonempty JSON string array")
    if args.timeout <= 0:
        parser.error("timeout must be positive")
    corpus_path = BENCHMARK / "acceptance/corpus.json"
    corpus = json.loads(corpus_path.read_text())
    if hashlib.sha256((BENCHMARK / "SPEC.md").read_bytes()).hexdigest() != corpus["spec_sha256"]:
        raise ValueError("frozen catalog specification differs from corpus binding")
    if len(corpus["cases"]) != 23 or len({case["name"] for case in corpus["cases"]}) != 23:
        raise ValueError("catalog acceptance requires the complete unique 23-case inventory")
    rows = []
    for case in corpus["cases"]:
        raw = bytes.fromhex(case["input_hex"])
        frozen = (case["status"], bytes.fromhex(case["stdout_hex"]), bytes.fromhex(case["stderr_hex"]))
        if expected(raw) != frozen:
            raise ValueError(f"frozen corpus/oracle mismatch: {case['name']}")
        row = {"name": case["name"], "input_bytes": len(raw),
               "input_sha256": hashlib.sha256(raw).hexdigest()}
        try:
            result = subprocess.run(command, input=raw, stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, timeout=args.timeout, check=False)
            actual = (result.returncode, result.stdout, result.stderr)
            row.update({"accepted": actual == frozen, "status": result.returncode,
                        "stdout_hex": result.stdout.hex(), "stderr_hex": result.stderr.hex()})
        except subprocess.TimeoutExpired as error:
            row.update({"accepted": False, "timeout": True,
                        "stdout_hex": (error.stdout or b"").hex(), "stderr_hex": (error.stderr or b"").hex()})
        rows.append(row)
    report = {"schema": SCHEMA, "accepted": all(row["accepted"] for row in rows),
              "required_cases": 23, "cases": rows,
              "corpus_sha256": hashlib.sha256(corpus_path.read_bytes()).hexdigest(), "command": command,
              "compiler_source": None, "compiler_binary_sha256": None,
              "agent_trial": False, "token_cost": None}
    with Path(args.report_json).open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    return 0 if report["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
