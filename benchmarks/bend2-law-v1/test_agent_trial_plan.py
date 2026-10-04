#!/usr/bin/env python3
"""Focused LAW-16 agent-trial preregistration regressions."""
import importlib.util
import json
import pathlib
import tempfile
import unittest


MODULE = pathlib.Path(__file__).with_name("agent_trial_plan.py")
SPEC = importlib.util.spec_from_file_location("bend2_agent_trials", MODULE)
PLAN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PLAN)


def config(trials=10):
    return {
        "schema": PLAN.CONFIG_SCHEMA,
        "model": {
            "provider": "example",
            "name": "fixed-model",
            "configuration_sha256": "sha256:" + "a" * 64,
        },
        "tool_access": {"network": "disabled", "filesystem": "sandboxed", "shell": "bounded"},
        "fixed_budget": {"max_tokens": 1000, "max_cost_usd": "1.00"},
        "trials_per_cell": trials,
    }


class AgentTrialPlanTests(unittest.TestCase):
    def test_config_requires_ten_or_more_trials_and_pinned_model_configuration(self):
        with self.assertRaisesRegex(ValueError, "at least 10"):
            PLAN.require_config(config(9))
        invalid = config()
        invalid["model"]["configuration_sha256"] = "unpinned"
        with self.assertRaisesRegex(ValueError, "pinned sha256"):
            PLAN.require_config(invalid)

    def test_u32_cells_are_explicitly_unavailable_for_the_reviewed_scalar_profile(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fixture = root / "fixture.json"
            fixture.write_text(json.dumps({
                "schema": "semaprax.bend2-law-benchmark.fixture.v1",
                "id": "matched-v1", "numeric_domain": "u32 checked",
                "success": [{}], "attacks": {"gaming": [{}]},
            }))
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({
                "schema": PLAN.RUN.MANIFEST_SCHEMA,
                "bend": {"commit": "pinned", "telemetry_environment": {"BEND_NO_TELEMETRY": "1"}},
                "required_paths": list(PLAN.RUN.PATHS),
                "cells": [{"id": "matched-v1", "fixture": "fixture.json", "numeric_domain": "u32 checked", "laws": ["law"], "attacks": ["gaming"]}],
            }))
            configuration = root / "config.json"
            configuration.write_text(json.dumps(config()))
            document = PLAN.plan(manifest, configuration)
        self.assertEqual(document["status"], "unavailable")
        self.assertEqual(document["cells"][0]["status"], "unsupported")
        self.assertIn("does not admit numeric domain", document["cells"][0]["reason"])
        self.assertEqual(document["nonclaims"][0], "no agent trial executed")


if __name__ == "__main__":
    unittest.main()
