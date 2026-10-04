import unittest, json
from worker.schedule import load, next_delay


class Units(unittest.TestCase):
    def test_config_keys(self):
        cfg = load()
        self.assertEqual(cfg, {"interval_ms": 30000, "jitter_ms": 5000})

    def test_delay_is_milliseconds(self):
        self.assertEqual(next_delay({"interval_ms": 30000, "jitter_ms": 4000}, 0.5), 32000)
