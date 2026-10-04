import unittest
from worker.schedule import next_delay


class T(unittest.TestCase):
    def test_delay(self):
        self.assertEqual(next_delay({"interval_ms": 30000, "jitter_ms": 4000}, 0.5), 32000)
