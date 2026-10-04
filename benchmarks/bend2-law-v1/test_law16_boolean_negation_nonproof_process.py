import importlib.util
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("nonproof", ROOT / "law16_boolean_negation_nonproof_process.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
CAPSULE_SPEC = importlib.util.spec_from_file_location("nonproof_capsule", ROOT / "law16_boolean_negation_nonproof_capsule.py")
CAPSULE = importlib.util.module_from_spec(CAPSULE_SPEC)
CAPSULE_SPEC.loader.exec_module(CAPSULE)


class NonproofProcessTests(unittest.TestCase):
    def test_command_shapes_keep_ordinary_and_nonproof_routes_separate(self):
        bend = MODULE.bend_ordinary_command(pathlib.Path("bun"), pathlib.Path("main.ts"), pathlib.Path("fresh.bend"))
        semaprax = MODULE.semaprax_check_command(pathlib.Path("semaprax"), pathlib.Path("fresh.spx"))
        self.assertEqual(bend, [pathlib.Path("bun"), pathlib.Path("main.ts"), pathlib.Path("fresh.bend")])
        self.assertNotIn("--verdict", bend)
        self.assertEqual(semaprax, [pathlib.Path("semaprax"), "check", pathlib.Path("fresh.spx")])
        self.assertNotIn("project-proof-check", semaprax)

    def test_prepared_plan_is_explicitly_unrun_and_bounded(self):
        plan = json.loads((ROOT / "evidence/law16-boolean-negation-nonproof-process-plan-v1.json").read_text())
        self.assertEqual(plan["status"], "prepared_not_executed")
        self.assertEqual(plan["samples_per_state"], 30)
        self.assertEqual(plan["cold_cache"]["status"], "unavailable")
        self.assertIn("no unrun result", plan["nonclaims"])
        self.assertEqual(CAPSULE.BEND_COMMIT, MODULE.BEND_COMMIT)
        self.assertEqual(CAPSULE.SEMAPRAX_SHA256, MODULE.SEMAPRAX_SHA256)


if __name__ == "__main__":
    unittest.main()
