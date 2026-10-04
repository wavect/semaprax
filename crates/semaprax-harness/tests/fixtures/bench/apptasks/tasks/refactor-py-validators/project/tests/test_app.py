import unittest
from app.users import register
from app.invites import invite
from app.newsletter import subscribe


class AppTests(unittest.TestCase):
    def test_register(self):
        users = {}
        self.assertEqual(register("  Bob@Example.COM ", users), "bob@example.com")
        with self.assertRaises(ValueError):
            register("bob@example.com", users)

    def test_invite(self):
        s = set()
        self.assertEqual(invite("A@b.c", s), "a@b.c")
        self.assertEqual(s, {"a@b.c"})

    def test_subscribe(self):
        subs = []
        subscribe("X@y.z", subs)
        subscribe("x@y.z", subs)
        self.assertEqual(subs, ["x@y.z"])
