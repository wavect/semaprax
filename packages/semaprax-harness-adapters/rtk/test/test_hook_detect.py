import json
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from hook_detect import detect_rtk_hook  # noqa: E402


class HookDetect(unittest.TestCase):
    def test_claude_settings_with_native_hook(self):
        s = json.dumps({"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "rtk hook claude"}]}]}})
        self.assertTrue(detect_rtk_hook(s))

    def test_legacy_script_and_absolute_path_hook(self):
        for cmd in ("/Users/x/.claude/hooks/rtk-rewrite.sh", "/opt/homebrew/bin/rtk hook cursor"):
            self.assertTrue(detect_rtk_hook(json.dumps({"hooks": {"PreToolUse": [{"hooks": [{"command": cmd}]}]}})))

    def test_vibe_toml_text(self):
        self.assertTrue(detect_rtk_hook('[[hooks]]\nname = "rtk-rewrite"\ncommand = "rtk hook vibe"\n'))

    def test_unrelated_settings_are_not_hooks(self):
        self.assertFalse(detect_rtk_hook(json.dumps({"hooks": {"PreToolUse": [{"hooks": [{"command": "my-linter --rtk-free"}]}]}})))
        self.assertFalse(detect_rtk_hook("{}"))
        self.assertFalse(detect_rtk_hook(""))


if __name__ == "__main__":
    unittest.main()
