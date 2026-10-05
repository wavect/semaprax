"""Instrumented fake Clef release: a model dir whose `joint_schema_model.py` mirrors the real API.

The fake module has the real entry points (`load_release_model`, `systemone`) and a
model with a `generate` method; the worker's guard must keep it from ever running.
Behavior switches come from environment variables of the worker process.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
import clef_identity as ident  # noqa: E402

FAKE_CODE = '''
import os, sys, time

class FakeModel:
    def modules(self):
        return [self]
    def generate(self, *a, **k):
        return "free text"

def load_release_model(path, device="cuda", **kw):
    time.sleep(float(os.environ.get("FAKE_LOAD_SLEEP", "0")))
    return FakeModel(), object()

def systemone(model, processor, request, max_length=16384):
    if os.environ.get("FAKE_TRY_GENERATE"):
        model.generate("x")
    mode = os.environ.get("FAKE_MODE", "ok")
    if mode == "crash":
        os._exit(1)
    if mode == "slow":
        time.sleep(3)
    if mode == "http500":
        raise ValueError("boom " + os.environ.get("FAKE_SECRET", ""))
    qid = next(iter(request["questions"]))
    opts = list(request["questions"][qid]["criteria"])
    probs = {o: 1.0 / len(opts) for o in opts}
    probs[opts[-1]] += 0.2
    t = sum(probs.values())
    probs = {o: round(v / t, 4) for o, v in probs.items()}
    choice = max(probs, key=probs.get)
    ans = {"type": "choice", "choice": choice, "confidence": probs[choice], "probabilities": probs}
    if mode == "unknown_option":
        ans["choice"] = "nope"
    ans_key = "route-deadbeefdeadbeef" if mode == "binding" else qid
    out = {"model": request["model"], "answers": {ans_key: ans}, "usage": {"input_tokens": 61, "output_tokens": 0}}
    if mode == "oversize":
        out["pad"] = "x" * 200000
    return out
'''

FILES = {
    "config.json": ("backbone", b'{"fake": true}'),
    "model-00001-of-00001.safetensors": ("backbone", b"backbone-bytes"),
    "joint_head.safetensors": ("head", b"head-bytes"),
    "joint_head_config.json": ("head", b'{"hidden_size": 4}'),
    "joint_schema_model.py": ("code", FAKE_CODE.encode()),
    "tokenizer.json": ("tokenizer", b"{}"),
    "LICENSE": ("license", b"Apache-2.0"),
}


def build(root, revision="f" * 40, devices=("cuda",), status="supported", model="clef-flash"):
    """Write a fake model dir under root/model and a lock under root/lock.json -> (model_dir, lock_path)."""
    mdir = os.path.join(root, "model")
    os.makedirs(mdir, exist_ok=True)
    files = {}
    for name, (group, data) in FILES.items():
        with open(os.path.join(mdir, name), "wb") as f:
            f.write(data)
        files[name] = {"bytes": len(data), "sha256": ident.sha256_file(os.path.join(mdir, name)), "group": group}
    rel = {"status": status, "repo": "fake/clef-flash", "revision": revision, "license": "Apache-2.0", "base_model": "fake",
           "devices": list(devices), "requirements": {}, "total_bytes": sum(m["bytes"] for m in files.values()), "files": files}
    lock = {"schema": ident.SCHEMA, "releases": {model: rel}}
    path = os.path.join(root, "lock-%s.json" % revision[:6])
    with open(path, "w") as f:
        json.dump(lock, f)
    return mdir, path
