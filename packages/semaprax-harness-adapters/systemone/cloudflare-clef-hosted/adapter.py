#!/usr/bin/env python3
"""Cloudflare-hosted Clef / Clef-Flash decision.evaluate v1/v2 adapter.

Account id: SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID. Token: host secret
SEMAPRAX_HARNESS_SECRET_CLOUDFLARE. Route: SEMAPRAX_HARNESS_MODEL =
@cf/cloudflare/clef or @cf/cloudflare/clef-flash. Remote use requires
SEMAPRAX_HARNESS_REMOTE_APPROVED=1. See docs/HARNESS-CLOUDFLARE-CLEF-V1.md.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import cloudflare_backend as cb  # noqa: E402
import systemone_runtime as rt  # noqa: E402

PROVIDER_ID = "com.cloudflare/clef-decision"
UPSTREAM_VERSION = "workers-ai-clef"

ACCEPTED = [{"kind": "decision.evaluate", "version": v, "operations": ["evaluate"]} for v in (1, 2)]

if __name__ == "__main__":
    rt.run(
        rt.Config(os.environ, cb.CloudflareBackend(os.environ)),
        ACCEPTED,
        {"provider_id": PROVIDER_ID, "adapter_version": rt.ADAPTER_VERSION, "upstream_version": UPSTREAM_VERSION},
    )
