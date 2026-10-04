#!/usr/bin/env python3
"""Replay the bounded balance witness through pinned Bend and installed Z3."""
from __future__ import annotations

import argparse, hashlib, json, os, pathlib, subprocess

SCHEMA = "semaprax.bend2-law-benchmark.bounded-balance-preflight.v1"
DECLARATIONS = ("app.balance.debit-after", "app.balance.credit-after", "app.balance.total-after")


def digest(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(root: pathlib.Path, name: str, argv: list[str], env: dict[str, str]) -> dict:
    done = subprocess.run(argv, capture_output=True, check=False, env=env)
    out, err = root / f"{name}.stdout", root / f"{name}.stderr"
    out.write_bytes(done.stdout); err.write_bytes(done.stderr)
    return {"argv": argv, "exit_code": done.returncode, "stdout_sha256": digest(out), "stderr_sha256": digest(err)}


def project(root: pathlib.Path, source: bytes) -> pathlib.Path:
    (root / "core").mkdir(parents=True); (root / "src").mkdir(); (root / "tests").mkdir()
    (root / "src/app.spx").write_bytes(source)
    (root / "core/core.spx").write_text('module app.core;\n\n@id("app.core.value")\nfn value() -> i64\n{\n    0\n}\n')
    (root / "tests/tests.spx").write_text('module app.tests;\n\n@id("app.tests.main")\nfn main() -> i64\n{\n    0\n}\n')
    manifest = root / "semaprax.toml"
    manifest.write_text('schema = "semaprax.project.v1"\nname = "law16-bounded-balance"\nentry = "app"\nsources = ["core/core.spx", "src/app.spx", "tests/tests.spx"]\nweb_exports = ["app.main"]\ntests = ["app.tests"]\n')
    return manifest


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--bend-root", required=True, type=pathlib.Path); p.add_argument("--bun", required=True, type=pathlib.Path)
    p.add_argument("--semaprax", required=True, type=pathlib.Path); p.add_argument("--z3", required=True, type=pathlib.Path)
    p.add_argument("--artifacts", required=True, type=pathlib.Path); p.add_argument("--output", required=True, type=pathlib.Path)
    a = p.parse_args(argv)
    if a.artifacts.exists() or a.output.exists(): p.error("artifacts and output must be new")
    a.artifacts.mkdir(); fixtures = pathlib.Path(__file__).with_name("fixtures")
    env = dict(os.environ, BEND_NO_TELEMETRY="1"); bend = [str(a.bun), str(a.bend_root / "bend2/main.ts")]
    rows: dict[str, object] = {"schema": SCHEMA, "domain": "fixed values 20,30,7 and results 13,37,50 within 0..100; no U32 wrapping", "nonclaims": ["not a checked-u32 cell", "source proof does not prove lowering or execution", "not an agent or comparative result"]}
    for kind, file in (("candidate", "bend-bounded-balance-transfer-v1.bend"), ("attack", "bend-bounded-balance-transfer-law-gaming-v1.bend")):
        source = fixtures / file; rows[f"bend_{kind}"] = {route: invoke(a.artifacts, f"bend-{kind}-{route}", bend + [str(source)] + (["--verdict"] if route == "verdict" else []), env) for route in ("normal", "verdict")}
    for kind, file in (("candidate", "semaprax-bounded-balance-transfer-v1.spx"), ("attack", "semaprax-bounded-balance-transfer-law-gaming-v1.spx")):
        manifest = project(a.artifacts / f"project-{kind}", (fixtures / file).read_bytes())
        rows[f"z3_{kind}"] = {declaration: invoke(a.artifacts, f"z3-{kind}-{declaration}", [str(a.semaprax), "project-proof-check", str(manifest), "--tool", "z3", "--executable", str(a.z3), "--version-line", "Z3 version 4.12.5 - 64 bit", "--host-profile", "trusted-local", "--source", "src/app.spx", "--declaration", declaration, "--ensures", "0"], env) for declaration in DECLARATIONS}
    ok = all(row[route]["exit_code"] == 0 for row in (rows["bend_candidate"],) for route in ("normal", "verdict")) and all(row[route]["exit_code"] != 0 for row in (rows["bend_attack"],) for route in ("normal", "verdict")) and all(row[declaration]["exit_code"] == 0 for row in (rows["z3_candidate"],) for declaration in DECLARATIONS) and all(rows["z3_attack"][d]["exit_code"] != 0 for d in (DECLARATIONS[0], DECLARATIONS[2]))
    rows["status"] = "completed_bounded_witness" if ok else "failed"
    a.output.write_text(json.dumps(rows, indent=2, sort_keys=True) + "\n")
    return 0 if ok else 1


if __name__ == "__main__": raise SystemExit(main())
