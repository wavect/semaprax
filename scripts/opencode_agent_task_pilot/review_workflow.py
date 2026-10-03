"""Deterministic, blinded review packets and no-spend cohort accounting."""

import base64
import hashlib
import json
import os
from pathlib import Path
import stat

PACKET_SCHEMA = "semaprax.opencode-agent-task-pilot-blinded-review-packet.v1"
PACKET_FILES = (
    "candidate.diff",
    "candidate-source.json",
    "independent-acceptance-replay.json",
    "gateway.jsonl",
    "mcp-wire.jsonl",
)
MAX_PACKET_FILE = 2 * 1024 * 1024
MAX_PACKET_TOTAL = 8 * 1024 * 1024
MAX_PACKET_BYTES = 8 * 1024 * 1024
MAX_RECORD_BYTES = 2 * 1024 * 1024
MAX_MANIFEST_BYTES = 256 * 1024
MAX_PROTOCOL_BYTES = 256 * 1024
REPO_ROOT = Path(__file__).resolve().parents[2]
FROZEN_MANIFEST = REPO_ROOT / "benchmarks/agent-task-comparison-v1/manifest.json"
FROZEN_MANIFEST_SHA256 = "3a6b4610aa3b570d6a070afa5381582efb4559b63c4efa73c517f5ab3349e033"
FROZEN_TASK_SHA256 = {
    "owned-signature-migration-v1": "c305f8cba2b611d40db5cbe3ec7ff593617313b85df6ba5f45591fcf812f22b6",
    "signature-migration-v1": "c25845c9f863a173d5b90b46a25fbdab1cf367fb7b17440b7c8edec4c88e6191",
    "stale-signature-recovery-v1": "9da21b78f2a5bdfd8755e984d94fbcd5be5cacbe455220bd115282e095f64011",
}
FROZEN_FIXTURE_SHA256 = {
    "owned-signature-migration-v1": "e16b0e2ed2ba5386e7d3fe746c223b8e17a7c2fdd898d2cc2da7bc80ea452422",
    "signature-migration-v1": "753b58d0c3e8c6cfafaea6c66edb1caba744ca90ab957e87b0b87b07255971e4",
    "stale-signature-recovery-v1": "753b58d0c3e8c6cfafaea6c66edb1caba744ca90ab957e87b0b87b07255971e4",
}


def _sha(body):
    return hashlib.sha256(body).hexdigest()


def _canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


PROTOCOL_SCHEMA = "semaprax.opencode-agent-task-pilot-protocol.v1"


def load_frozen_protocol(path):
    """Read one exact two-model execution protocol without reading credentials.

    The protocol is deliberately a public, non-secret binding artifact.  It
    identifies a provider/model/revision and the approved budget/egress policy
    by identifier, never a credential value or environment-variable name.
    """
    body = _read_regular(path, MAX_PROTOCOL_BYTES)
    try:
        value = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("execution protocol is not JSON") from error
    if not isinstance(value, dict) or body != _canonical(value) + b"\n":
        raise ValueError("execution protocol is not canonical JSON")
    required = {
        "schema", "id", "manifest_sha256", "runner_revision", "budget_policy",
        "egress_policy", "models",
    }
    if set(value) != required or value.get("schema") != PROTOCOL_SCHEMA:
        raise ValueError("execution protocol schema or keys differ")
    if value.get("manifest_sha256") != FROZEN_MANIFEST_SHA256:
        raise ValueError("execution protocol manifest binding differs")
    for name in ("id", "runner_revision", "budget_policy", "egress_policy"):
        if not isinstance(value.get(name), str) or not value[name] or len(value[name]) > 65536:
            raise ValueError("execution protocol identity is invalid")
    models = value.get("models")
    if not isinstance(models, list) or len(models) != 2:
        raise ValueError("execution protocol requires exactly two models")
    model_ids = set()
    configured_models = set()
    for model in models:
        if not isinstance(model, dict) or set(model) != {
            "id", "provider", "model", "revision", "configured_model", "tokenizer", "configuration"
        }:
            raise ValueError("execution protocol model keys differ")
        for name in ("id", "provider", "model", "revision", "configured_model", "tokenizer", "configuration"):
            item = model.get(name)
            if not isinstance(item, str) or not item or len(item) > 65536 or item.strip() != item:
                raise ValueError("execution protocol model identity is invalid")
        if model["revision"].lower() == "latest" or model["configured_model"] != f"{model['provider']}/{model['model']}":
            raise ValueError("execution protocol model revision is mutable or mismatched")
        if model["id"] in model_ids or model["configured_model"] in configured_models:
            raise ValueError("execution protocol model identities are duplicated")
        model_ids.add(model["id"])
        configured_models.add(model["configured_model"])
    return value, _sha(body)


def _read_regular(path, limit):
    """Bounded O_NOFOLLOW read of one regular, single-link file."""
    fd = None
    created = False
    try:
        if not hasattr(os, "O_NOFOLLOW"):
            raise ValueError("safe no-follow file reads are unavailable")
        fd = os.open(Path(path), os.O_RDONLY | os.O_NOFOLLOW)
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_size > limit:
            raise ValueError("evidence is not a bounded regular file")
        def read_once():
            os.lseek(fd, 0, os.SEEK_SET)
            chunks, total = [], 0
            while total <= limit:
                block = os.read(fd, min(131072, limit + 1 - total))
                if not block:
                    break
                chunks.append(block); total += len(block)
            return b"".join(chunks), total
        first, total = read_once()
        second, second_total = read_once()
        after = os.fstat(fd)
        if (total > limit or second_total > limit or first != second or
                (before.st_dev, before.st_ino, before.st_size) != (after.st_dev, after.st_ino, after.st_size)):
            raise ValueError("evidence changed during bounded read")
        rebound_fd = os.open(Path(path), os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        try:
            rebound = os.fstat(rebound_fd)
            if (rebound.st_dev, rebound.st_ino) != (before.st_dev, before.st_ino):
                raise ValueError("evidence path rebound during held read")
        finally:
            os.close(rebound_fd)
        return first
    except OSError as error:
        raise ValueError("evidence is not readable") from error
    finally:
        if fd is not None:
            try:
                os.close(fd)
            except OSError:
                pass


def _candidate_digest(files):
    digest = hashlib.sha256()
    digest.update(b"semaprax.blinded-review-candidate.v1\0")
    for name, body in files:
        encoded_name = name.encode("utf-8")
        digest.update(len(encoded_name).to_bytes(8, "big"))
        digest.update(encoded_name)
        digest.update(len(body).to_bytes(8, "big"))
        digest.update(bytes.fromhex(_sha(body)))
    return digest.hexdigest()


def prepare_review_packet(evidence_dir, output):
    """Create a reviewer packet withholding direct lane/model/runner labels.

    The packet contains the candidate diff and hashes/lengths for the archived
    evidence. Identity-bearing ``record.json`` is deliberately not copied;
    task content and paths remain visible for the operator's blinded review.
    """
    evidence_dir = Path(evidence_dir)
    output = Path(output)
    if output.exists():
        raise ValueError("review packet output already exists")
    files = []
    total = 0
    for name in PACKET_FILES:
        path = evidence_dir / name
        if path.exists() or path.is_symlink():
            if path.is_symlink():
                raise ValueError(f"packet member {name} is a symlink")
            body = _read_regular(path, MAX_PACKET_FILE)
            total += len(body)
            if total > MAX_PACKET_TOTAL:
                raise ValueError("review packet evidence exceeds aggregate bound")
            files.append((name, body))
    if not any(name == "candidate.diff" for name, _ in files):
        raise ValueError("review packet requires candidate.diff")
    candidate_digest = _candidate_digest(files)
    packet = {
        "schema": PACKET_SCHEMA,
        "candidate_digest": candidate_digest,
        "candidate_diff_base64": base64.b64encode(dict(files)["candidate.diff"]).decode("ascii"),
        "candidate_diff_sha256": _sha(dict(files)["candidate.diff"]),
        "evidence": [
            {"name": name, "bytes": len(body), "sha256": _sha(body)}
            for name, body in files
        ],
    }
    encoded = _canonical(packet) + b"\n"
    if output.parent.resolve() != evidence_dir.resolve() or output.name != "review-packet.json":
        raise ValueError("review packet must be evidence-dir/review-packet.json")
    if len(encoded) > MAX_PACKET_BYTES:
        raise ValueError("review packet exceeds bound")
    fd = None
    try:
        if not hasattr(os, "O_NOFOLLOW"):
            raise ValueError("safe no-follow packet creation is unavailable")
        fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        created = True
        written = 0
        while written < len(encoded):
            count = os.write(fd, encoded[written:])
            if count <= 0:
                raise ValueError("review packet write made no progress")
            written += count
    except (OSError, ValueError) as error:
        if created:
            try:
                held = os.fstat(fd)
                current = os.stat(output, follow_symlinks=False)
                if (held.st_dev, held.st_ino) == (current.st_dev, current.st_ino):
                    os.unlink(output)
            except OSError:
                pass
        raise ValueError("review packet could not be created exclusively") from error
    finally:
        if fd is not None:
            os.close(fd)
    return dict(packet, packet_sha256=_sha(encoded))


def load_review_packet(path):
    path = Path(path)
    try:
        raw = _read_regular(path, MAX_PACKET_BYTES)
        packet = json.loads(raw.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("review packet is not readable canonical JSON") from error
    if not isinstance(packet, dict) or packet.get("schema") != PACKET_SCHEMA:
        raise ValueError("review packet schema differs")
    if _canonical(packet) + b"\n" != raw:
        raise ValueError("review packet is not canonical JSON")
    if set(packet) != {"schema", "candidate_digest", "candidate_diff_base64", "candidate_diff_sha256", "evidence"}:
        raise ValueError("review packet has an unexpected shape")
    if not isinstance(packet["evidence"], list) or not packet["evidence"]:
        raise ValueError("review packet evidence inventory is empty")
    try:
        diff = base64.b64decode(packet["candidate_diff_base64"], validate=True)
    except (TypeError, ValueError) as error:
        raise ValueError("review packet candidate diff is not canonical base64") from error
    if len(diff) > MAX_PACKET_FILE or _sha(diff) != packet["candidate_diff_sha256"]:
        raise ValueError("review packet candidate diff digest differs")
    entries = packet["evidence"]
    if not isinstance(entries[0], dict) or entries[0].get("name") != "candidate.diff":
        raise ValueError("review packet evidence must begin with candidate.diff")
    if entries[0].get("bytes") != len(diff) or entries[0].get("sha256") != _sha(diff):
        raise ValueError("review packet candidate diff entry differs from embedded bytes")
    digest = hashlib.sha256(b"semaprax.blinded-review-candidate.v1\0")
    seen = set()
    total = 0
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != {"name", "bytes", "sha256"}:
            raise ValueError("review packet evidence entry has an unexpected shape")
        name = entry["name"].encode("utf-8")
        if entry["name"] not in PACKET_FILES or entry["name"] in seen:
            raise ValueError("review packet evidence member is unsupported or duplicated")
        seen.add(entry["name"])
        if isinstance(entry["bytes"], bool) or not isinstance(entry["bytes"], int) or entry["bytes"] < 0:
            raise ValueError("review packet evidence length is invalid")
        if not isinstance(entry["sha256"], str) or len(entry["sha256"]) != 64:
            raise ValueError("review packet evidence digest is invalid")
        try:
            digest_bytes = bytes.fromhex(entry["sha256"])
        except ValueError as error:
            raise ValueError("review packet evidence digest is invalid") from error
        total += entry["bytes"]
        if total > MAX_PACKET_TOTAL:
            raise ValueError("review packet evidence exceeds aggregate bound")
        digest.update(len(name).to_bytes(8, "big")); digest.update(name)
        digest.update(entry["bytes"].to_bytes(8, "big")); digest.update(digest_bytes)
    if digest.hexdigest() != packet["candidate_digest"]:
        raise ValueError("review packet candidate digest is malformed")
    return packet


def candidate_digest_for_evidence(evidence_dir):
    evidence_dir = Path(evidence_dir)
    files = []
    total = 0
    for name in PACKET_FILES:
        path = evidence_dir / name
        if path.exists() or path.is_symlink():
            if path.is_symlink():
                raise ValueError(f"packet member {name} is a symlink")
            body = _read_regular(path, MAX_PACKET_FILE)
            total += len(body)
            if total > MAX_PACKET_TOTAL:
                raise ValueError("candidate evidence exceeds aggregate bound")
            files.append((name, body))
    if not any(name == "candidate.diff" for name, _ in files):
        raise ValueError("candidate.diff is absent")
    return _candidate_digest(files)


def audit_cohort(evidence_root, manifest_path, protocol_path=None):
    """Audit exact tuple accounting without invoking a model or network.

    A protocol-bound audit is the only form that can describe a future
    scoreable two-model cohort.  The protocol-free shape remains readable for
    the September historical archive, but is explicitly ineligible evidence.
    """
    root = Path(evidence_root)
    if Path(manifest_path).resolve() != FROZEN_MANIFEST.resolve():
        raise ValueError("cohort audit requires the frozen canonical manifest")
    manifest = json.loads(_read_regular(manifest_path, MAX_MANIFEST_BYTES).decode("utf-8"))
    if _sha(_read_regular(manifest_path, MAX_MANIFEST_BYTES)) != FROZEN_MANIFEST_SHA256:
        raise ValueError("frozen manifest bytes differ")
    if manifest.get("schema") != "semaprax.agent-task-comparison-manifest.v1" or manifest.get("repetitions") != 3:
        raise ValueError("frozen manifest schema or repetitions differ")
    if manifest.get("tasks") != [
        "benchmarks/agent-task-comparison-v1/tasks/signature-migration.json",
        "benchmarks/agent-task-comparison-v1/tasks/stale-signature-recovery.json",
        "benchmarks/agent-task-comparison-v1/tasks/owned-signature-migration.json",
    ]:
        raise ValueError("frozen manifest task paths differ")
    tasks = []
    task_digests = {}
    for task_path in manifest["tasks"]:
        task_body = _read_regular(REPO_ROOT / task_path, MAX_MANIFEST_BYTES)
        task = json.loads(task_body.decode("utf-8"))
        tasks.append(task["id"])
        task_digests[task["id"]] = _sha(task_body)
    if sorted(tasks) != ["owned-signature-migration-v1", "signature-migration-v1", "stale-signature-recovery-v1"]:
        raise ValueError("frozen manifest tasks differ")
    lanes = [row["id"] for row in manifest["lanes"] if row.get("availability") == "available"]
    if sorted(lanes) != ["semaprax-graph-operational", "semaprax-source-first"]:
        raise ValueError("frozen manifest available lanes differ")
    repetitions = manifest["repetitions"]
    protocol = None
    protocol_digest = None
    if protocol_path is not None:
        protocol, protocol_digest = load_frozen_protocol(protocol_path)
        model_ids = [model["id"] for model in protocol["models"]]
        expected = {
            (model_id, task, lane, trial)
            for model_id in model_ids for task in tasks for lane in lanes
            for trial in range(1, repetitions + 1)
        }
    else:
        expected = {(task, lane, trial) for task in tasks for lane in lanes for trial in range(1, repetitions + 1)}
    records = []
    for path in sorted(root.glob("*/record.json")):
        value = json.loads(_read_regular(path, MAX_RECORD_BYTES).decode("utf-8"))
        records.append((path, value))
    if protocol is None:
        actual = [(value.get("task"), value.get("lane"), value.get("trial")) for _, value in records]
    else:
        actual = [
            (
                value["model_identity"].get("id")
                if isinstance(value.get("model_identity"), dict)
                and isinstance(value["model_identity"].get("id"), str)
                else None,
                value.get("task"), value.get("lane"), value.get("trial"),
            )
            for _, value in records
        ]
    counts = {key: actual.count(key) for key in set(actual)}
    duplicates = sorted(key for key, count in counts.items() if count > 1)
    extra = sorted(set(actual) - expected)
    missing = sorted(expected - set(actual))
    invalid = []
    reasons = {}
    for (path, value), key in zip(records, actual):
        if value.get("manifest_sha256") != FROZEN_MANIFEST_SHA256:
            invalid.append(f"{path}: manifest binding differs")
        if value.get("task_sha256") != FROZEN_TASK_SHA256.get(value.get("task")):
            invalid.append(f"{path}: task binding differs")
        fixture_digest = value.get("fixture_inventory_sha256")
        if fixture_digest != FROZEN_FIXTURE_SHA256.get(value.get("task")):
            invalid.append(f"{path}: fixture binding is missing or malformed")
        if value.get("status") not in ("eligible", "ineligible"):
            invalid.append(f"{path}: invalid status")
        if value.get("outcome") not in ("completed", "failed", "aborted"):
            invalid.append(f"{path}: invalid outcome")
        if protocol is None:
            if value.get("status") != "ineligible":
                invalid.append(f"{path}: historical evidence must remain explicitly ineligible")
        else:
            model_id = value.get("model_identity", {}).get("id") if isinstance(value.get("model_identity"), dict) else None
            model = next((item for item in protocol["models"] if item["id"] == model_id), None)
            if value.get("protocol_sha256") != protocol_digest:
                invalid.append(f"{path}: execution protocol binding differs")
            if model is None or value.get("model_identity") != model:
                invalid.append(f"{path}: model identity differs from execution protocol")
        reason = value.get("reason")
        if value.get("status") == "ineligible":
            if not isinstance(reason, str) or not reason:
                invalid.append(f"{path}: ineligible record lacks explicit reason")
            else:
                reasons[str(key)] = reason
    failed = sum(value.get("outcome") == "failed" for _, value in records)
    complete = not (missing or duplicates or extra or invalid) and len(records) == len(expected)
    scoreable = protocol is not None and complete and all(value.get("status") == "eligible" for _, value in records)
    return {
        "schema": "semaprax.opencode-agent-task-pilot-cohort-audit.v1",
        "protocol_sha256": protocol_digest,
        "historical_evidence": protocol is None,
        "eligible_for_scoring": scoreable,
        "expected_tuples": len(expected), "retained_records": len(records),
        "missing": missing, "duplicate": duplicates, "extra": extra,
        "invalid": invalid, "failed_records_retained": failed,
        "aborted_records_retained": sum(value.get("outcome") == "aborted" for _, value in records),
        "ineligibility_reasons": reasons,
        "complete": complete,
        "claims": {
            "execution": "not_performed_by_audit",
            "comparative_result": "not_claimed",
            "historical_evidence": "explicitly_ineligible" if protocol is None else "not_historical",
        },
    }
