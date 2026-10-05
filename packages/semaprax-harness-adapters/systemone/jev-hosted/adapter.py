#!/usr/bin/env python3
"""Jev-hosted decision.evaluate v1/v2 adapter (provider ai.typesafe/jev-decision).

Uses the documented authenticated TypeSafe API. The key comes only from the
host-provided SEMAPRAX_HARNESS_SECRET_JEV; remote use requires
SEMAPRAX_HARNESS_REMOTE_APPROVED=1. See docs/HARNESS-LAYA-JEV-V1.md.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import systemone_backend as be  # noqa: E402
import systemone_runtime as rt  # noqa: E402

PROVIDER_ID = "ai.typesafe/jev-decision"
UPSTREAM_VERSION = "typesafe-api-0.2.0"

ACCEPTED = [{"kind": "decision.evaluate", "version": v, "operations": ["evaluate"]} for v in (1, 2)]

if __name__ == "__main__":
    rt.run(
        rt.Config(os.environ, be.JevBackend(os.environ)),
        ACCEPTED,
        {"provider_id": PROVIDER_ID, "adapter_version": rt.ADAPTER_VERSION, "upstream_version": UPSTREAM_VERSION},
    )
