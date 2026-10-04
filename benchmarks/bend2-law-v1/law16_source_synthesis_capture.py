#!/usr/bin/env python3
"""Capture and review host source-to-SMT renderer timings for LAW16."""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
from pathlib import Path
import shutil
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "fixtures/boolean-negation-project-v1/candidate/src/app.spx"
SCHEMA = "semaprax.bend2-law-benchmark.host-source-synthesis.v1"
SAMPLES = 30
TIMEOUT_SECONDS = 30
HELPER_SHA256 = "sha256:08539b12c0697d619a46448bbfe1e2fd934c867a7f209a071a345d9b6256381c"
SOURCE_TREE_COMMIT = "cdfc0cdd27aa248951134f70a71bf7d36b4798de"
HELPER_SOURCE = ROOT / "tools/law16_render_app_negate_smt.rs"
HELPER_SOURCE_SHA256 = "sha256:35185d41143938999b5f9da04cb8c970dd879a4817e502fa67ee3c235dd0383e"
SOURCE_SHA256 = "sha256:bc71b8bde8cb43cc10b49742e7e5abb3717cd28402a1ab53e7d549f540093063"
RENDERER_OUTPUT_SHA256 = "sha256:b6cfacc2c9aaa899c2ad5f41496b06ea42ba86123c95eb967477c5bdc27e3464"
SOLVER_INPUT_SHA256 = "sha256:f0938ea89248af8556a3a623759eeba7193213301b2d3bc4f1c478c113490e88"
RENDERER_SUFFIX = b"(get-model)\n"
NONCLAIMS = [
    "timing is host process source-to-SMT rendering plus deterministic suffix removal; it is not SEMAPRAX project-proof-check end-to-end timing",
    "host file, executable, and runtime caches are not isolated; samples are not cold/warm observations",
    "the normalized solver input hash matches the separately captured guest Z3 input bytes; no project proof_ref or source-proof claim is made",
    "the helper binary digest and helper source digest are recorded, but their build association is not attested",
    "no compilation, native execution, cross-route timing ratio, or winner is measured",
]


def digest_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def digest(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def normalize_renderer_output(data: bytes) -> bytes:
    """Remove only the exact model-query trailer from the pinned renderer output."""
    if not data.endswith(RENDERER_SUFFIX):
        raise ValueError("renderer output lacks the exact model-query suffix")
    return data[: -len(RENDERER_SUFFIX)]


def artifact(path: Path, root: Path) -> dict[str, object]:
    return {
        "path": path.relative_to(root).as_posix(),
        "bytes": path.stat().st_size,
        "sha256": digest(path),
    }


def checked(root: Path, value: object, label: str) -> Path:
    if not isinstance(value, dict) or set(value) != {"path", "bytes", "sha256"}:
        raise ValueError(f"{label} artifact reference is malformed")
    rel = Path(value["path"])
    if rel.is_absolute() or ".." in rel.parts or not rel.parts:
        raise ValueError(f"{label} artifact path escapes capture")
    path = root / rel
    base = root.resolve(strict=True)
    resolved = path.resolve(strict=True)
    if not resolved.is_relative_to(base) or path.is_symlink() or not path.is_file():
        raise ValueError(f"{label} artifact is not a regular in-capture file")
    if artifact(path, root) != value:
        raise ValueError(f"{label} artifact digest differs")
    return path


def summarize(values: list[int]) -> dict[str, int | float]:
    ordered = sorted(values)
    return {
        "count": len(values),
        "p50_ns": statistics.median(values),
        "p95_ns": ordered[(95 * len(ordered) + 99) // 100 - 1],
    }


def review(root: Path) -> dict[str, object]:
    root = root.resolve(strict=True)
    receipt = json.loads((root / "result.json").read_text())
    if receipt.get("schema") != SCHEMA or receipt.get("status") != "thirty_repetitions_authenticated":
        raise ValueError("source synthesis receipt identity or status differs")
    if receipt.get("sample_count") != SAMPLES or receipt.get("timeout_seconds") != TIMEOUT_SECONDS:
        raise ValueError("source synthesis repetition or timeout budget differs")
    if receipt.get("nonclaims") != NONCLAIMS:
        raise ValueError("source synthesis scope boundaries differ")
    if receipt.get("timing_scope") != "monotonic interval from helper process launch through exact suffix removal":
        raise ValueError("source synthesis timing scope differs")
    if receipt.get("helper_build_association") != "helper executable and source file digests are pinned independently; build association is not attested":
        raise ValueError("source synthesis helper build boundary differs")

    helper = receipt.get("helper")
    if not isinstance(helper, dict) or helper.get("sha256") != HELPER_SHA256:
        raise ValueError("source synthesis helper differs from the pinned executable")
    helper_path = checked(root, {key: helper.get(key) for key in ("path", "bytes", "sha256")}, "helper")
    if digest(helper_path) != HELPER_SHA256:
        raise ValueError("retained source synthesis helper digest differs")
    helper_source = receipt.get("helper_source")
    source_ref = checked(
        ROOT,
        {key: helper_source.get(key) for key in ("path", "bytes", "sha256")}
        if isinstance(helper_source, dict)
        else None,
        "helper source",
    )
    if (
        source_ref != HELPER_SOURCE
        or digest(source_ref) != HELPER_SOURCE_SHA256
        or receipt.get("source_tree_commit") != SOURCE_TREE_COMMIT
    ):
        raise ValueError("source synthesis helper source differs from its pinned commit")
    source = checked(ROOT, receipt.get("source"), "source")
    if source != SOURCE or digest(source) != SOURCE_SHA256:
        raise ValueError("source synthesis input differs from the pinned fixture")
    if receipt.get("solver_input_sha256") != SOLVER_INPUT_SHA256:
        raise ValueError("normalized solver input digest differs from the guest input pin")

    samples = receipt.get("samples")
    if not isinstance(samples, list) or len(samples) != SAMPLES:
        raise ValueError("source synthesis capture must contain thirty samples")
    if receipt.get("source_path_at_capture") != str(SOURCE):
        raise ValueError("source synthesis command input path differs")
    elapsed: list[int] = []
    for ordinal, sample in enumerate(samples, 1):
        if not isinstance(sample, dict) or sample.get("ordinal") != ordinal:
            raise ValueError("source synthesis samples are not in canonical order")
        if sample.get("exit_code") != 0 or sample.get("timed_out") is not False:
            raise ValueError(f"source synthesis sample {ordinal} did not complete")
        if sample.get("argv") != [str(helper_path), str(SOURCE)] or sample.get("source_sha256") != SOURCE_SHA256:
            raise ValueError(f"source synthesis sample {ordinal} command or input digest differs")
        duration = sample.get("elapsed_ns")
        if not isinstance(duration, int) or duration <= 0:
            raise ValueError(f"source synthesis sample {ordinal} has invalid duration")
        raw = checked(root, sample.get("renderer_stdout"), f"sample {ordinal} renderer stdout").read_bytes()
        err = checked(root, sample.get("stderr"), f"sample {ordinal} stderr").read_bytes()
        smt = checked(root, sample.get("solver_input"), f"sample {ordinal} solver input").read_bytes()
        if err or digest_bytes(raw) != RENDERER_OUTPUT_SHA256:
            raise ValueError(f"source synthesis sample {ordinal} renderer output differs")
        if normalize_renderer_output(raw) != smt or digest_bytes(smt) != SOLVER_INPUT_SHA256:
            raise ValueError(f"source synthesis sample {ordinal} normalized input differs")
        if sample.get("renderer_output_sha256") != digest_bytes(raw) or sample.get("solver_input_sha256") != digest_bytes(smt):
            raise ValueError(f"source synthesis sample {ordinal} output hash fields differ")
        elapsed.append(duration)

    summary = summarize(elapsed)
    if receipt.get("summary") != summary:
        raise ValueError("source synthesis timing summary differs from raw samples")
    return {
        "status": "thirty_repetitions_authenticated",
        "summary": summary,
        "renderer_output_sha256": RENDERER_OUTPUT_SHA256,
        "solver_input_sha256": SOLVER_INPUT_SHA256,
        "nonclaims": NONCLAIMS,
    }


def capture(helper: Path, output: Path) -> dict[str, object]:
    if output.exists() or not output.parent.is_dir():
        raise ValueError("output must be new beneath an existing directory")
    helper = helper.resolve(strict=True)
    if helper.is_symlink() or not helper.is_file() or digest(helper) != HELPER_SHA256:
        raise ValueError("helper differs from the pinned executable")
    if SOURCE.is_symlink() or digest(SOURCE) != SOURCE_SHA256:
        raise ValueError("source fixture differs from the pinned input")

    output.mkdir()
    tools_dir = output / "tools"
    tools_dir.mkdir()
    helper_copy = tools_dir / "law16_render_app_negate_smt"
    shutil.copyfile(helper, helper_copy)
    helper_copy.chmod(0o755)
    if digest(helper_copy) != HELPER_SHA256:
        raise ValueError("copied helper differs from the pinned executable")
    raw_dir = output / "raw"
    raw_dir.mkdir()
    samples: list[dict[str, object]] = []
    elapsed: list[int] = []
    for ordinal in range(1, SAMPLES + 1):
        argv = [str(helper_copy.resolve()), str(SOURCE)]
        start = time.perf_counter_ns()
        try:
            child = subprocess.run(
                argv, capture_output=True, timeout=TIMEOUT_SECONDS, check=False
            )
            raw_bytes, stderr = child.stdout, child.stderr
            exit_code, timed_out = child.returncode, False
        except subprocess.TimeoutExpired as error:
            raw_bytes, stderr = error.stdout or b"", error.stderr or b""
            exit_code, timed_out = None, True
        smt_bytes = b""
        if not timed_out:
            try:
                smt_bytes = normalize_renderer_output(raw_bytes)
            except ValueError:
                pass
        # Stop the measured interval before persisting evidence artifacts. The
        # phase includes process launch, renderer execution, and exact suffix
        # normalization only.
        duration = time.perf_counter_ns() - start
        stdout_path = raw_dir / f"{ordinal:02d}.renderer.stdout"
        stderr_path = raw_dir / f"{ordinal:02d}.stderr"
        stdout_path.write_bytes(raw_bytes)
        stderr_path.write_bytes(stderr)
        smt_path = raw_dir / f"{ordinal:02d}.solver-input.smt2"
        smt_path.write_bytes(smt_bytes)
        sample = {
            "ordinal": ordinal,
            "argv": argv,
            "source_sha256": SOURCE_SHA256,
            "exit_code": exit_code,
            "timed_out": timed_out,
            "elapsed_ns": duration,
            "renderer_stdout": artifact(stdout_path, output),
            "stderr": artifact(stderr_path, output),
            "solver_input": artifact(smt_path, output),
            "renderer_output_sha256": digest_bytes(raw_bytes),
            "solver_input_sha256": digest_bytes(smt_bytes),
        }
        samples.append(sample)
        elapsed.append(duration)
        if timed_out or exit_code != 0 or stderr or digest_bytes(raw_bytes) != RENDERER_OUTPUT_SHA256 or digest_bytes(smt_bytes) != SOLVER_INPUT_SHA256:
            receipt = _receipt(helper, helper_copy, output, samples, None, "incomplete")
            (output / "result.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
            raise ValueError(f"source synthesis sample {ordinal} failed its pinned-output check")

    receipt = _receipt(helper, helper_copy, output, samples, summarize(elapsed), "thirty_repetitions_authenticated")
    (output / "result.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    return review(output)


def _receipt(helper: Path, helper_copy: Path, output: Path, samples: list[dict[str, object]], summary: object, status: str) -> dict[str, object]:
    return {
        "schema": SCHEMA,
        "status": status,
        "sample_count": SAMPLES,
        "timeout_seconds": TIMEOUT_SECONDS,
        "helper": {**artifact(helper_copy, output), "original_path_at_capture": str(helper)},
        "helper_source": artifact(HELPER_SOURCE, ROOT),
        "source_tree_commit": SOURCE_TREE_COMMIT,
        "helper_build_association": "helper executable and source file digests are pinned independently; build association is not attested",
        "source": artifact(SOURCE, ROOT),
        "source_path_at_capture": str(SOURCE),
        "summary": summary,
        "samples": samples,
        "timing_scope": "monotonic interval from helper process launch through exact suffix removal",
        "renderer_output_sha256": RENDERER_OUTPUT_SHA256,
        "solver_input_sha256": SOLVER_INPUT_SHA256,
        "host": {"system": platform.system(), "release": platform.release(), "machine": platform.machine()},
        "nonclaims": NONCLAIMS,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--helper", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--review", type=Path)
    args = parser.parse_args(argv)
    if bool(args.output) == bool(args.review):
        parser.error("select exactly one of --output or --review")
    if args.output and args.helper is None:
        parser.error("--helper is required with --output")
    try:
        result = review(args.review) if args.review else capture(args.helper, args.output)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
