import unittest
from worker.schedule import load, next_delay


class T(unittest.TestCase):
    def test_delay(self):
        self.assertEqual(next_delay({"interval": 30, "jitter": 4}, 0.5), 32)
