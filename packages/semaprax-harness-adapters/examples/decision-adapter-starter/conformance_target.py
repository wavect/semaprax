"""Conformance targets for the starter adapter (scoring and scoreless variants)."""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "sdk", "python"))

SECRET = "starter-secret-0123456789"


class Running:
    def __init__(self, env, mutate):
        self.argv = [sys.executable, os.path.join(HERE, "adapter.py")]
        self.env, self.secret, self.mutate = env, SECRET, mutate

    def stop(self):
        pass

    def posts(self):
        return []  # no upstream

    def wait_in_flight(self, timeout):
        import time
        time.sleep(0.3)


def _mutations():
    def question(p):
        p["task"] = "model-route/v9"
        return p

    def candidate(p):
        p["options"] = list(reversed(p["options"]))
        return p

    def identity(p):
        p["rendered"]["digest"] = "sha256:" + "0" * 64
        return p
    return {"wrong_question": question, "wrong_candidate": candidate, "wrong_identity": identity}


class StarterTarget:
    crash_kind = "process"

    def __init__(self, scoreless=False):
        self.scoreless = scoreless
        self.supports_v1 = not scoreless
        self.name = "starter-scoreless" if scoreless else "starter"

    def start(self, fault="ok", **env_extra):
        env = {"SEMAPRAX_HARNESS_SECRET_STARTER": SECRET, **env_extra}
        if self.scoreless:
            env["STARTER_SCORELESS"] = "1"
        if fault in ("slow", "crash", "oversize", "leak"):
            env["STARTER_FAULT"] = fault
        if fault == "abstain":
            env["STARTER_ABSTAIN"] = "1"
        return Running(env, _mutations().get(fault))
