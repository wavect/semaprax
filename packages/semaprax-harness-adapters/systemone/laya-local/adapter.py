#!/usr/bin/env python3
"""Laya-local decision.evaluate v1/v2 adapter (provider ai.convai/laya-decision).

Talks to a user-selected, already running loopback Laya server named by
SEMAPRAX_HARNESS_ENDPOINT. It never starts a server and never downloads a
checkpoint. See docs/HARNESS-LAYA-JEV-V1.md.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import systemone_backend as be  # noqa: E402
import systemone_runtime as rt  # noqa: E402

PROVIDER_ID = "ai.convai/laya-decision"
UPSTREAM_VERSION = "laya-0.3.26"

ACCEPTED = [{"kind": "decision.evaluate", "version": v, "operations": ["evaluate"]} for v in (1, 2, 3)]

if __name__ == "__main__":
    rt.run(
        rt.Config(os.environ, be.LayaBackend(os.environ)),
        ACCEPTED,
        {"provider_id": PROVIDER_ID, "adapter_version": rt.ADAPTER_VERSION, "upstream_version": UPSTREAM_VERSION},
    )
