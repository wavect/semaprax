#!/usr/bin/env python3
"""Archive complete immediate source/patch histories; no language execution."""
from pathlib import Path
import hashlib
import json
import subprocess

ROOT = Path.cwd()
HERE = Path(__file__).resolve().parent
STAGES = [
    ("canonical-839-initial", "15ac307443132d8631e8b16b036ba9a689d97139"),
    ("canonical-839-result-fee", "f8ff67481513a720a50eb747188beb19bc6b9b83"),
    ("weighted-838-checkpoint", "41b6e19ad"),
    ("weighted-838-amortized", "40f77262950e9dc4bb1da6626f441f2f504cfe59"),
]
sha = lambda value: hashlib.sha256(value).hexdigest()


def blob(revision, source):
    result = subprocess.run(["git", "cat-file", "blob", revision + ":" + source],
                            cwd=ROOT, capture_output=True)
    if result.returncode:
        assert not subprocess.check_output(["git", "ls-tree", revision, "--", source], cwd=ROOT), result.stderr
        return None
    return result.stdout


def write_once(path, value):
    if path.exists():
        assert path.read_bytes() == value, (str(path), "immutable prepared source bytes")
    else:
        path.write_bytes(value)


records = []
for label, revision in STAGES:
    revision = subprocess.check_output(["git", "rev-parse", revision], cwd=ROOT, text=True).strip()
    base = subprocess.check_output(["git", "rev-parse", revision + "^"], cwd=ROOT, text=True).strip()
    paths = subprocess.check_output(["git", "diff", "--name-only", base, revision], cwd=ROOT, text=True).splitlines()
    folder = HERE / "packets" / label
    folder.mkdir(parents=True, exist_ok=True)
    rows = []
    for source in paths:
        old, authored = blob(base, source), blob(revision, source)
        assert authored is not None, (label, source, "unexpected deletion")
        old = b"" if old is None else old
        archive = folder / "originals" / (source + ".before")
        archive.parent.mkdir(parents=True, exist_ok=True)
        write_once(archive, old)
        rows.append(dict(source=source, checkpoint_revision=revision,
                         archive=str(archive.relative_to(ROOT)),
                         original_sha256=sha(old), current_sha256=sha(authored),
                         edits=[dict(new=authored.decode(), old=old.decode())]))
    seed = HERE / "seeds" / (label + ".json")
    seed.parent.mkdir(exist_ok=True)
    write_once(seed, (json.dumps(rows, indent=2) + "\n").encode())
    packet = folder / "source-inverses.json"
    if not packet.exists():
        packet.write_bytes(seed.read_bytes())
    patch = folder / "complete-forward.patch"
    write_once(patch, subprocess.check_output(["git", "diff", "--binary", base, revision], cwd=ROOT))
    records.append(dict(label=label, packet=str(packet.relative_to(ROOT)),
                        seed=str(seed.relative_to(ROOT)), seed_sha256=sha(seed.read_bytes()),
                        original_base=base, authored_revision=revision,
                        complete_patch=str(patch.relative_to(ROOT)), patch_sha256=sha(patch.read_bytes()),
                        complete_patch_paths=paths, rows=len(rows)))
write_once(HERE / "next-packet-histories.json", (json.dumps(records, indent=2) + "\n").encode())
print(json.dumps(dict(new_stage_packets=len(records), complete_patch_paths=sum(r["rows"] for r in records))))
