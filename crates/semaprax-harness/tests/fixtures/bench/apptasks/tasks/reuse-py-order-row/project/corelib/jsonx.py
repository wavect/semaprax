import json


def dumps_stable(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))
