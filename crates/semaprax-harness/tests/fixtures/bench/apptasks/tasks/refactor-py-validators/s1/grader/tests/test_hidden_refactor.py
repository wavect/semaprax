import unittest
from app.users import register
from app.invites import invite
from app.newsletter import subscribe


class Hidden(unittest.TestCase):
    def test_invalid_everywhere(self):
        for bad in ["nope", "@x", "x@", "   "]:
            with self.assertRaisesRegex(ValueError, "^invalid email$"):
                register(bad, {})
            with self.assertRaisesRegex(ValueError, "^invalid email$"):
                invite(bad, set())
            with self.assertRaisesRegex(ValueError, "^invalid email$"):
                subscribe(bad, [])

    def test_validators_module(self):
        from app.validators import normalize_email
        self.assertEqual(normalize_email("  Q@R.s "), "q@r.s")
        with self.assertRaisesRegex(ValueError, "^invalid email$"):
            normalize_email("zzz")
