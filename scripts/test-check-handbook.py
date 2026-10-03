#!/usr/bin/env python3
"""Unit tests for the handbook checker; these do not execute Semaprax."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("handbook_checker", Path(__file__).with_name("check-handbook.py"))
assert SPEC and SPEC.loader
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)


class HandbookChecks(unittest.TestCase):
    def documents(self):
        return {
            "handbook/SUMMARY.md": "# Summary\n\n[Start](README.md)\n- [Lesson](lesson.md)\n",
            "handbook/README.md": "# Welcome\n\n[Lesson](lesson.md)\n",
            "handbook/lesson.md": "# Lesson\n\n```sh\nnot-a-shell-command [ignored](missing.md)\n```\n",
        }

    def validate(self, documents):
        return checker.validate_documents(documents, set(documents), set(documents) - {"handbook/SUMMARY.md"})

    def test_links_in_shell_fences_are_not_document_links(self):
        self.assertEqual(len(self.validate(self.documents())), 1)

    def test_missing_link_is_rejected(self):
        docs = self.documents()
        docs["handbook/lesson.md"] += "[Broken](missing.md)\n"
        with self.assertRaisesRegex(checker.HandbookError, "missing local target"):
            self.validate(docs)

    def test_nested_image_and_its_outer_link_are_both_seen(self):
        self.assertEqual(checker.targets("[![Alt](image.png)](lesson.md)"), ["image.png", "lesson.md"])

    def test_duplicate_and_missing_navigation_are_rejected(self):
        for navigation in ("\n- [Again](lesson.md)\n", "missing"):
            docs = self.documents()
            if navigation == "missing":
                docs["handbook/SUMMARY.md"] = "# Summary\n[Start](README.md)\n"
            else:
                docs["handbook/SUMMARY.md"] += navigation
            with self.assertRaisesRegex(checker.HandbookError, "exactly one SUMMARY"):
                self.validate(docs)

    def test_fences_must_close(self):
        with self.assertRaisesRegex(checker.HandbookError, "unclosed code fence"):
            checker.scan("a.md", "# A\n```semaprax\nmodule a;\n")

    def test_tilde_fences_are_supported(self):
        prose, blocks = checker.scan("a.md", "# A\n~~~text\na\n~~~\n")
        self.assertEqual(prose, "# A")
        self.assertEqual(blocks[0].source, "a\n")

    def test_marker_must_have_valid_json_and_an_adjacent_fence(self):
        for source in (
            '<!-- handbook-smoke: nope -->\n',
            '<!-- handbook-smoke: [] -->\n',
            '<!-- handbook-smoke: {"stdout":"x"} -->\nNot adjacent.\n',
            '<!-- handbook-smoke: {"stdout":"x"} -->\n',
        ):
            with self.assertRaises(checker.HandbookError):
                checker.scan("a.md", source)

    def test_external_urls_and_anchors_do_not_need_local_files(self):
        self.assertIsNone(checker.local_target("handbook/a.md", "https://example.org/page"))
        self.assertIsNone(checker.local_target("handbook/a.md", "#heading"))
        self.assertEqual(checker.local_target("handbook/a.md", "b.md#part"), "handbook/b.md")

    def test_local_path_cannot_escape_repository(self):
        with self.assertRaises(checker.HandbookError):
            checker.local_target("handbook/a.md", "../../outside")
        with self.assertRaises(checker.HandbookError):
            checker.local_target("handbook/a.md", "/absolute")

    def test_project_files_cannot_traverse_or_duplicate(self):
        for path_name in ("../outside.spx", "/absolute.spx", "src/../main.spx", "src\\main.spx"):
            block = checker.Block("a.md", 1, "semaprax", "", "project-file", {"group":"test", "path":path_name})
            with self.assertRaises(checker.HandbookError):
                checker.collect_examples([block])
        valid = checker.Block("a.md", 1, "semaprax", "", "project-file", {"group":"test", "path":"src/main.spx"})
        with self.assertRaisesRegex(checker.HandbookError, "duplicate project file"):
            checker.collect_examples([valid, valid])

    def test_unknown_smoke_fields_and_missing_manifest_are_rejected(self):
        typo = checker.Block("a.md", 1, "semaprax", "", "smoke", {"stdot":"42\n"})
        with self.assertRaises(checker.HandbookError):
            checker.collect_examples([typo])
        file = checker.Block("a.md", 1, "semaprax", "", "project-file", {"group":"test", "path":"src/main.spx"})
        with self.assertRaisesRegex(checker.HandbookError, "missing semaprax.toml"):
            checker.collect_examples([file])

    def smoke_blocks(self):
        return [
            checker.Block("a.md", 1, "semaprax", "module a;\n", "smoke", {"stdout":"42\n"}),
            checker.Block("p.md", 1, "toml", "", "project-file", {"group":"test", "path":"semaprax.toml", "stdout":"42\n", "test":True}),
        ]

    def test_smoke_driver_checks_output_and_calls_project_tests(self):
        def fake(compiler, arguments, cwd, timeout):
            return "42\n" if arguments[0] == "run" else ""
        with patch.object(checker, "invoke", side_effect=fake) as invoked:
            self.assertEqual(checker.run_examples(self.smoke_blocks(), Path("/fake/compiler"), 1), (1, 1))
            self.assertTrue(any(call.args[1][0] == "test" for call in invoked.call_args_list))
        with patch.object(checker, "invoke", return_value="wrong\n"):
            with self.assertRaisesRegex(checker.HandbookError, "expected"):
                checker.run_examples(self.smoke_blocks(), Path("/fake/compiler"), 1)

    def test_compiler_gate_refuses_an_empty_selection(self):
        with self.assertRaisesRegex(checker.HandbookError, "empty gate"):
            checker.run_examples([], Path("/fake/compiler"), 1)


if __name__ == "__main__":
    unittest.main()
