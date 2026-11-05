#!/usr/bin/env python3
"""Check ten complete formatting-only source pairs without changing any body."""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[3]
DATA = Path(__file__).with_name("inverses.json")
assert hashlib.sha256(DATA.read_bytes()).hexdigest() == "eb3e544be99a0019102d48f3ce5790646c89fb3dcb56247d70a95979aa61dc96"
rows = json.loads(DATA.read_bytes())
assert len(rows) == len({row["name"] for row in rows}) == 10
formatter = shutil.which("rustfmt")
assert formatter, "rustfmt is required for the independent formatting comparison"
for row in rows:
    bodies = []
    for version in ["current", "before"]:
        sha = row[f"{version}_sha256"]
        path = ROOT / "e" / sha
        assert not path.is_symlink()
        body = path.read_bytes()
        assert len(body) == row[f"{version}_bytes"]
        assert hashlib.sha256(body).hexdigest() == sha
        bodies.append(body)
    restored = bodies[0]
    for offset, current, before in reversed(row["inverse"]):
        current, before = current.encode(), before.encode()
        assert restored[offset:offset + len(current)] == current
        restored = restored[:offset] + before + restored[offset + len(current):]
    assert restored == bodies[1], row["name"]
    for prefix, suffix in [(b"", b""), (b"impl Example {\n", b"\n}")]:
        outputs = [
            subprocess.run([formatter, "--edition", "2021", "--emit", "stdout"],
                           input=prefix + body + suffix, capture_output=True, check=False)
            for body in bodies
        ]
        if all(output.returncode == 0 for output in outputs):
            assert outputs[0].stdout == outputs[1].stdout, row["name"]
            break
    else:
        raise AssertionError(f"cannot parse complete pair: {row['name']}")
print("PASS: all ten full formatting pairs, original bytes and literal contents preserved")
