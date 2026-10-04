import unittest

import full_u32_encoding_controls as control


class FullU32EncodingControls(unittest.TestCase):
    def test_infrastructure_failures_never_count_as_attack_rejection(self):
        for route in ("bend_normal", "bend_verdict", "semaprax_native"):
            for code in (None, 1, 70, 127):
                with self.subTest(route=route, code=code):
                    self.assertFalse(control.accepted(route, True, code, b"", b"missing compiler"))
        self.assertFalse(control.accepted("bend_verdict", True, 1, b"SOME PROOFS FAIL", b"syntax error"))
        self.assertFalse(control.accepted("semaprax_native", True, 70, b"", b"SEMAPRAX contract failure\ncontract: requires false"))

    def test_assurance_route_success_markers_are_not_interchangeable(self):
        normal = b"ALL PROOFS CHECK\nUse --verdict for mathematical validity.\n"
        verdict = b"ALL PROOFS CHECK\n"
        self.assertTrue(control.accepted("bend_normal", False, 0, normal, b""))
        self.assertTrue(control.accepted("bend_verdict", False, 0, verdict, b""))
        self.assertFalse(control.accepted("bend_verdict", False, 0, normal, b""))
        self.assertFalse(control.accepted("bend_normal", False, 0, verdict, b""))
        self.assertFalse(control.accepted("semaprax_native", False, 0, b"1\n", b""))

    def test_attacks_preserve_laws_and_original_witnesses(self):
        for name in control.MUTATIONS:
            source = (control.FIXTURES / name).read_text()
            attack = control.mutation(name, source)
            old, new = control.MUTATIONS[name]
            self.assertEqual(attack, source.replace(old, new))
            if name.endswith(".bend"):
                self.assertEqual(source[source.index("law case_0:"):], attack[attack.index("law case_0:"):])
            else:
                self.assertEqual([line for line in source.splitlines() if "requires " in line or "ensures " in line],
                                 [line for line in attack.splitlines() if "requires " in line or "ensures " in line])
            self.assertIn("4294967295", source)
            with self.assertRaises(ValueError):
                control.mutation(name, source.replace(old, "changed"))
            with self.assertRaises(ValueError):
                control.mutation(name, source + old)


if __name__ == "__main__":
    unittest.main()
