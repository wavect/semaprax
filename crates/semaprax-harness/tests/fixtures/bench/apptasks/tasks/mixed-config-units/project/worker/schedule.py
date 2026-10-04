import json


def load(path="config/schedule.json"):
    with open(path) as f:
        return json.load(f)


def next_delay(cfg, rand01):
    """Seconds to wait before the next run; rand01 in [0, 1)."""
    return cfg["interval"] + cfg["jitter"] * rand01
