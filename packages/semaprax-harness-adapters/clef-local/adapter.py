#!/usr/bin/env python3
"""Clef-local decision.evaluate v1/v2 adapter (provider ai.cloudflare/clef-local-decision).

Talks to an explicitly started loopback Clef worker named by
SEMAPRAX_HARNESS_ENDPOINT. It never starts a worker, installs a package,
downloads weights or code, or falls back to a hosted endpoint.
See docs/HARNESS-CLEF-LOCAL-V1.md.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import clef_backend as cb  # noqa: E402
import systemone_runtime as rt  # noqa: E402

PROVIDER_ID = "ai.cloudflare/clef-local-decision"
UPSTREAM_VERSION = "clef-flash-17f0b0ad"

ACCEPTED = [{"kind": "decision.evaluate", "version": v, "operations": ["evaluate"]} for v in (1, 2)]

if __name__ == "__main__":
    rt.run(
        rt.Config(os.environ, cb.ClefLocalBackend(os.environ)),
        ACCEPTED,
        {"provider_id": PROVIDER_ID, "adapter_version": rt.ADAPTER_VERSION, "upstream_version": UPSTREAM_VERSION},
    )
