import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("law16_project_incremental_cell", ROOT / "law16_project_incremental_cell.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
CAPSULE = ROOT / "evidence/law16-project-incremental-cell-v1"


class ProjectIncrementalCellTests(unittest.TestCase):
    def test_exact_commands_select_only_the_two_project_cache_controls(self):
        binary = pathlib.Path("/tool/semaprax-tests")
        self.assertEqual(MODULE.command(binary, MODULE.SUCCESS_TEST), [str(binary), MODULE.SUCCESS_TEST, "--exact", "--nocapture"])
        self.assertEqual(MODULE.command(binary, MODULE.ATTACK_TEST), [str(binary), MODULE.ATTACK_TEST, "--exact", "--nocapture"])

    def test_success_acceptance_requires_the_real_cache_classification_output(self):
        output = (f"test {MODULE.SUCCESS_TEST} ... ok\nprovider edit: 2 modules cloned (17 AST nodes), 1 module reparsed (5 AST nodes)\n").encode()
        self.assertTrue(MODULE.accepted(0, output, MODULE.SUCCESS_TEST, ("provider edit:", "modules cloned", "module reparsed")))
        self.assertFalse(MODULE.accepted(0, output.replace(b"module reparsed", b"unexpected"), MODULE.SUCCESS_TEST, ("provider edit:", "modules cloned", "module reparsed")))
        self.assertFalse(MODULE.accepted(1, output, MODULE.SUCCESS_TEST, ("provider edit:",)))

    def test_project_inputs_bind_a_three_module_provider_consumer_project(self):
        MODULE.check_project_inputs()
        self.assertEqual([path.relative_to(MODULE.PROJECT).as_posix() for path in MODULE.PROJECT_FILES], ["semaprax.toml", "src/app.spx", "src/core.spx", "src/tests.spx"])
        self.assertIn("left + right", (MODULE.PROJECT / "src/core.spx").read_text())

    def test_committed_capsule_binds_the_three_module_success_and_signature_attack(self):
        review = MODULE.verify(CAPSULE)
        self.assertEqual(review["status"], "completed_local_project_incremental_cell")
        self.assertEqual(review["project_modules"], 3)
        self.assertEqual(review["raw_streams"], 4)
        self.assertEqual(review["overall_law16"], "incomplete")


if __name__ == "__main__":
    unittest.main()
