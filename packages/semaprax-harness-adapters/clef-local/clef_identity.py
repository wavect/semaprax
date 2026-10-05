"""Pinned Clef release identity shared by the adapter, worker and provisioner.

Standard library only. The lock (clef.lock.json) pins, per release, the exact
HF revision and the sha256 of every file that determines behavior. The call
identity digests four groups (backbone, head, code, tokenizer) plus the
revision, so a swapped head, patched inference code or different tokenizer
changes the identity that every receipt carries.
"""

import hashlib
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_LOCK = os.path.join(HERE, "clef.lock.json")
SCHEMA = "semaprax.clef-local-lock.v1"
GROUPS = ("backbone", "head", "code", "tokenizer")
HEAD_FILES = ("joint_head.safetensors", "joint_head_config.json")
CODE_FILE = "joint_schema_model.py"
LOCK_ENV = "SEMAPRAX_HARNESS_CLEF_LOCK"


class LockError(Exception):
    pass


def load_lock(path=None):
    try:
        with open(path or DEFAULT_LOCK, "rb") as f:
            doc = json.loads(f.read(4 << 20))
    except (OSError, ValueError) as err:
        raise LockError("clef lock is unreadable: " + type(err).__name__)
    if not isinstance(doc, dict) or doc.get("schema") != SCHEMA or not isinstance(doc.get("releases"), dict):
        raise LockError("clef lock has the wrong schema")
    return doc


def release(lock, model):
    rel = lock["releases"].get(model)
    if rel is None:
        raise LockError(f"no pinned Clef release named {model!r}")
    return rel


def group_digests(rel):
    out = {}
    for g in GROUPS:
        lines = sorted(f"{n}:{m['sha256']}" for n, m in rel["files"].items() if m["group"] == g)
        out[g] = "sha256:" + hashlib.sha256("\n".join(lines).encode()).hexdigest()
    return out


def identity_digest(rel):
    """One digest binding revision + backbone + head + code + tokenizer."""
    canon = json.dumps({"revision": rel["revision"], "groups": group_digests(rel)}, separators=(",", ":"), sort_keys=True)
    return "sha256:" + hashlib.sha256(canon.encode()).hexdigest()


def sha256_file(path, chunk=1 << 20):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            b = f.read(chunk)
            if not b:
                return h.hexdigest()
            h.update(b)


def verify_dir(model_dir, rel, progress=None):
    """Raise LockError('missing_file:X' | 'digest_mismatch:X') unless every pinned file matches."""
    for name, meta in rel["files"].items():
        p = os.path.join(model_dir, name)
        if not os.path.isfile(p):
            raise LockError("missing_file:" + name)
        if os.path.getsize(p) != meta["bytes"] or sha256_file(p) != meta["sha256"]:
            raise LockError("digest_mismatch:" + name)
        if progress:
            progress(name)
