"""Symmetric disk admission and incident receipts for local paid campaigns.

Headroom is an operational floor, not a guarantee that another task cannot fill
the shared volume. Incidents preserve acceptance and cost denominators but
disqualify the campaign from a clean comparative headline.
"""
from __future__ import annotations

import functools
import json
import os
import shutil
import threading
import time
from pathlib import Path

MIN_FREE_BYTES = 5 * 1024**3
SAMPLE_SECONDS = 5
RESERVE_BYTES = 1024**2
DISK_ERRORS = ("no space left on device", "errno=28", "errno 28", "disk full")


def existing_parent(path: Path) -> Path:
    while not path.exists():
        path = path.parent
    return path


def snapshot(paths: list[Path]) -> list[dict]:
    volumes = {}
    for path in paths:
        parent = existing_parent(path.resolve())
        device = parent.stat().st_dev
        if device not in volumes:
            volumes[device] = {"device": device, "path": str(parent),
                               "free_bytes": shutil.disk_usage(parent).free}
    return list(volumes.values())


def require_headroom(paths: list[Path]) -> list[dict]:
    samples = snapshot(paths)
    if any(row["free_bytes"] < MIN_FREE_BYTES for row in samples):
        raise ValueError(f"paid launch refused: require {MIN_FREE_BYTES} free bytes on every output/rollout volume")
    return samples


def policy() -> dict:
    return {"minimum_free_bytes": MIN_FREE_BYTES, "sample_seconds": SAMPLE_SECONDS,
            "reserve_bytes_per_attempt": RESERVE_BYTES,
            "scope": "same admission and monitoring for both arms and separate calibration",
            "acceptance_unchanged": True, "contaminated_attempt_costs_retained": True}


class Monitor:
    def __init__(self, artifacts: Path, label: str):
        self.artifacts, self.label = artifacts, label
        self.paths = [artifacts, Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))]
        self.samples = require_headroom(self.paths)
        self.minimum = {row["device"]: row["free_bytes"] for row in self.samples}
        self.incidents: list[dict] = []
        directory = artifacts / "resource-receipts"
        directory.mkdir(parents=True, exist_ok=True)
        self.receipt = directory / f"{label}.json"
        self.reserve = directory / f"{label}.reserve"
        # Write actual bytes rather than a sparse truncate so this reserve can
        # be released to retain a small emergency row when the volume fills.
        with self.reserve.open("xb") as output:
            output.write(bytes(RESERVE_BYTES))
            output.flush()
            os.fsync(output.fileno())
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self._watch, daemon=True)
        self.write("prepared")

    def write(self, state: str, row: dict | None = None):
        document = {"schema": "semaprax.campaign-resource-receipt.v1", "label": self.label,
                    "state": state, "policy": policy(), "initial_volumes": self.samples,
                    "minimum_free_bytes_by_device": self.minimum, "incidents": self.incidents,
                    "paid_attempt_state": "inspect retained transcript; prepared alone does not prove a paid launch",
                    "row": row}
        temporary = self.receipt.with_suffix(".tmp")
        with temporary.open("w") as output:
            json.dump(document, output, indent=2, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(self.receipt)

    def sample(self):
        for row in snapshot(self.paths):
            self.minimum[row["device"]] = min(self.minimum.get(row["device"], row["free_bytes"]), row["free_bytes"])
            if row["free_bytes"] < MIN_FREE_BYTES and not self.incidents:
                self.reserve.unlink(missing_ok=True)
                self.incidents.append({"kind": "disk_headroom_below_floor", "unix_seconds": time.time(), **row})
                self.write("resource_contaminated")

    def _watch(self):
        while not self.stop.wait(SAMPLE_SECONDS):
            try:
                self.sample()
            except OSError as error:
                self.reserve.unlink(missing_ok=True)
                self.incidents.append({"kind": "resource_monitor_io_error", "error": str(error)})
                break

    def finish(self, row: dict):
        self.stop.set()
        if self.thread.ident is not None:
            self.thread.join()
        try:
            self.sample()
            stderr = self.artifacts / "transcripts" / f"{self.label}.stderr.txt"
            if stderr.is_file():
                # Rollout recorder failures may happen even if a coarse disk
                # sample misses the moment the volume was full.
                with stderr.open(errors="replace") as source:
                    for line in source:
                        if any(value in line.lower() for value in DISK_ERRORS):
                            self.incidents.append({"kind": "disk_error_in_retained_stderr", "evidence": str(stderr)})
                            break
        except OSError as error:
            self.incidents.append({"kind": "resource_finish_io_error", "error": str(error)})
        self.reserve.unlink(missing_ok=True)
        row["resource_assessment"] = {"contaminated": bool(self.incidents),
            "clean_comparison_eligible": not self.incidents, "receipt": str(self.receipt),
            "incidents": self.incidents, "minimum_free_bytes_by_device": {str(key): value for key, value in self.minimum.items()},
            "acceptance_unchanged": True, "cost_retained": True}
        self.write("finished", row)
        return row


def guarded_attempt(function):
    """Keep emergency attempt rows outside results.json without rerunning work."""
    @functools.wraps(function)
    def run(repo, artifacts, commit, *args, **kwargs):
        trial = kwargs.get("trial")
        if trial is None and args and isinstance(args[0], dict) and "arm" in args[0]:
            trial = args[0]
        label = f"{trial['arm']}-{trial['number']:02d}" if trial else "calibration"
        monitor = Monitor(Path(artifacts), label)
        monitor.thread.start()
        try:
            row = function(repo, artifacts, commit, *args, **kwargs)
        except OSError as error:
            monitor.reserve.unlink(missing_ok=True)
            monitor.incidents.append({"kind": "attempt_io_error", "error": str(error)})
            row = {**(trial or {}), "status": "failed", "runner_error": True,
                   "failure": str(error), "workspace_retained_for_review": True,
                   "provider_receipt_actual_usd": None, "observed": {}, "list_price": {},
                   "telemetry_valid": False,
                   "accounting_note": "interrupted attempt: recover retained transcript; missing usage is unknown"}
        except BaseException:
            monitor.stop.set()
            monitor.thread.join()
            monitor.reserve.unlink(missing_ok=True)
            raise
        return monitor.finish(row)
    return run
