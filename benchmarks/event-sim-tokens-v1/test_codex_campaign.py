import unittest
from unittest.mock import patch

import codex_campaign


class CodexShiftSimTests(unittest.TestCase):
    def test_capabilities_reuses_shared_no_model_preflight(self):
        with patch.object(codex_campaign.codex, "capabilities", return_value={"status": "ready"}) as check:
            self.assertEqual(codex_campaign.main.__name__, "main")
            self.assertEqual(codex_campaign.codex.capabilities("fixture"), {"status": "ready"})
        check.assert_called_once_with("fixture")

    def test_command_is_shared_and_disables_plugins_for_both_arms(self):
        command = codex_campaign.codex.codex_command("prompt")
        self.assertIn("--ignore-user-config", command)
        self.assertEqual([command[i + 1] for i, value in enumerate(command) if value == "--disable"],
                         ["apps", "plugins", "memories", "multi_agent", "skill_search"])
        self.assertEqual(codex_campaign.ARMS, ("semaprax", "typescript"))


if __name__ == "__main__":
    unittest.main()
