import importlib.util
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("cold_warm", ROOT / "law16_boolean_negation_cold_warm.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ColdWarmTests(unittest.TestCase):
    def test_pins_and_canonical_fixture_inputs_are_declared(self):
        self.assertEqual(MODULE.BEND_COMMIT, "947db722640c86247849343657bf2f7ef01cb7f1")
        self.assertEqual(MODULE.SEMAPRAX_SHA256, "sha256:cc9dd3ca99a74dd973cbfb904621e27b801d8ddea6065873d24c14de9dee1d89")
        self.assertEqual(MODULE.SAMPLES, 30)

    def test_commands_bind_the_admitted_negation_contract(self):
        bend = MODULE.bend_command(pathlib.Path("bun"), pathlib.Path("main.ts"), pathlib.Path("fresh.bend"))
        semaprax = MODULE.semaprax_command(pathlib.Path("semaprax"), pathlib.Path("z3"), pathlib.Path("fresh-project"))
        self.assertEqual(bend[-1], "--verdict")
        self.assertEqual(semaprax[-4:], ["--declaration", "app.negate", "--ensures", "0"])
        self.assertIn(MODULE.Z3_VERSION, semaprax)
        self.assertEqual(semaprax[2], (pathlib.Path("fresh-project").resolve() / "semaprax.toml"))
        self.assertEqual(json.loads(MODULE.json_command(semaprax)), [str(part) for part in semaprax])

    def test_prepared_plan_makes_no_unrun_claim(self):
        plan = json.loads((ROOT / "evidence/law16-boolean-negation-cold-warm-plan-v1.json").read_text())
        self.assertEqual(plan["status"], "prepared_not_executed")
        self.assertEqual(plan["samples_per_state"], 30)
        self.assertEqual(plan["cold_cache"]["status"], "unavailable")
        self.assertIn("no unrun result", plan["nonclaims"])


if __name__ == "__main__":
    unittest.main()
