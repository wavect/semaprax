"""Closed current870 raw endpoint -> exact fixed51 canonical stage."""
import hashlib,json
from pathlib import Path
DATA_SHA256 = "6523645c1445ceffafe43ff87118f3f2d6f94686a39b444ff6f87cedf7460bc0"
RELATIVE = "docs/evidence/opt870-current-stage-custody-20261017/raw-to-stage.json"
def canonical_stage(repo, raw):
    data = (Path(repo) / RELATIVE).read_bytes()
    sha = lambda b: hashlib.sha256(b).hexdigest()
    assert sha(data) == DATA_SHA256
    row = json.loads(data)
    assert row["schema"] == "semaprax.canonical870-stage.v1"
    assert row["source"] == "src/cleanup_plan/replay/factored/canonical_shared.rs"
    assert len(raw) == row["raw_length"] and sha(raw) == row["raw_sha256"]
    lines = raw.decode().splitlines(keepends=True)
    assert b"".join(v.encode() for v in lines) == raw
    assert 0 < len(row["inverse"]) <= 64
    last = len(lines)
    for edit in row["inverse"][::-1]:
        a,z = edit["start"],edit["end"]
        assert 0 <= a <= z <= last <= len(lines)
        assert "".join(lines[a:z]) == edit["current"]
        lines[a:z] = edit["previous"].splitlines(keepends=True)
        last = a
    stage = "".join(lines).encode()
    assert len(stage) == row["stage_length"] and sha(stage) == row["stage_sha256"]
    return stage
