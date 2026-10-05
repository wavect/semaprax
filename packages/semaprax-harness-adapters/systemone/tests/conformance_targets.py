"""Conformance targets for the SystemOne backends (jev, laya) over the loopback fakes."""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import fake_servers as fs  # noqa: E402

ENTRY = {"laya": "laya-local", "jev": "jev-hosted"}
MODES = {"ok": "ok", "wrong_question": "binding", "wrong_candidate": "unknown_option", "slow": "slow",
         "crash": "crash", "oversize": "oversize", "leak": "http500"}


class Running:
    def __init__(self, flavor, srv, env):
        self.argv = [sys.executable, os.path.join(ROOT, ENTRY[flavor], "adapter.py")]
        self.srv, self.env, self.secret = srv, env, fs.KEY
        self.mutate = None

    def stop(self):
        self.srv.stop()

    def posts(self):
        return [x for x in self.srv.log if x[0] == "POST"]

    def wait_in_flight(self, timeout):
        import time
        end = time.monotonic() + timeout
        while time.monotonic() < end and not self.posts():
            time.sleep(0.02)


class SystemOneTarget:
    crash_kind = "upstream"
    scoreless = False
    supports_v1 = True

    def __init__(self, flavor):
        self.flavor, self.name = flavor, flavor

    def start(self, fault="ok", **env_extra):
        mode = MODES.get(fault, "ok")
        if fault == "wrong_identity" and self.flavor == "laya":
            mode = "wrong_checkpoint"
        if fault == "abstain" and self.flavor == "laya":
            mode = "abstain"
        srv = fs.start(self.flavor, mode)
        env = {"SEMAPRAX_HARNESS_ENDPOINT": f"http://127.0.0.1:{srv.server_port}"}
        env["SEMAPRAX_HARNESS_SECRET_" + self.flavor.upper()] = fs.KEY
        if self.flavor == "jev":
            env["SEMAPRAX_HARNESS_MODEL"] = "jev-not-mine" if fault == "wrong_identity" else "jev-test-1"
            if fault == "abstain":
                env["SEMAPRAX_HARNESS_MIN_SCORE"] = "0.99"
        env.update(env_extra)
        return Running(self.flavor, srv, env)
