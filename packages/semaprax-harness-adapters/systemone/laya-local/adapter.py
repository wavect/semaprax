#!/usr/bin/env python3
"""Laya-local decision.evaluate/v1 adapter (provider ai.convai/laya-decision).

Talks to a user-selected, already running loopback Laya server named by
SEMAPRAX_HARNESS_ENDPOINT. It never starts a server and never downloads a
checkpoint. See docs/HARNESS-LAYA-JEV-V1.md.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import systemone_runtime as rt  # noqa: E402

PROVIDER_ID = "ai.convai/laya-decision"
UPSTREAM_VERSION = "laya-0.3.26"

if __name__ == "__main__":
    rt.run(
        rt.Config(os.environ, "laya"),
        [{"kind": "decision.evaluate", "version": 1, "operations": ["evaluate"]}],
        {"provider_id": PROVIDER_ID, "adapter_version": rt.ADAPTER_VERSION, "upstream_version": UPSTREAM_VERSION},
    )
