#!/opt/homebrew/bin/python3
"""Prepare exact frozen context and two explicit protocol overlays; never build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

BASE = "1ff0a4b81"
REGISTRATION = b'\n#[cfg(test)]\n#[path = "oracle847.rs"]\nmod oracle847;\n'
INVENTORIES = ["tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"]
ENV = dict(os.environ, TMPDIR="/Volumes/Semaprax/runtime-tmp", PYTHONDONTWRITEBYTECODE="1")
REPO = Path(__file__).resolve().parents[3]
def external(path):
    result = Path(path).resolve()
    if not result.is_relative_to("/Volumes/Semaprax"):
        raise ValueError("all task paths must stay on external SSD")
    return result

def git(*args, cwd=REPO):
    return subprocess.check_output(["git", *args], cwd=cwd, env=ENV)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def binding(root, head):
    """Authenticate every tracked file, including original test/source readers.
    Symlink bytes are authenticated without following their targets.
    """
    rows = []
    for item in git("ls-tree", "-rz", head, cwd=root).split(b"\0"):
        if not item:
            continue
        header, raw_path = item.split(b"\t", 1)
        mode, kind, blob = header.decode().split()
        if kind != "blob":
            raise ValueError("closed context does not admit submodule authority")
        name = os.fsdecode(raw_path)
        path = root / name
        data = os.fsencode(os.readlink(path)) if mode == "120000" else path.read_bytes()
        actual_blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        if actual_blob != blob:
            raise ValueError(f"source drift before overlay: {name}")
        rows.append({"path":name,"mode":mode,"git_blob":blob,"sha256":sha(data)})
    return rows

def prepare(root, head, driver, overlay):
    git("worktree", "add", "--detach", str(root), head)
    rows = binding(root, head)
    changes = []
    if overlay:
        # One existing unit engine binary, no extra top-level integration binary.
        for name in ["src/lib.rs", *INVENTORIES]:
            path = root / name
            before = path.read_bytes()
            if name == "src/lib.rs":
                after = before + REGISTRATION
            else:
                text = before.decode()
                match = re.search(r'include_str!\("((?:\.\./)+)src/[^"\n]+"\)', text)
                if not match:
                    raise ValueError(f"inventory anchor absent: {name}")
                old = match.group(0)
                joined = 'concat!(' + old + ', "\\n", include_str!("' + match.group(1) + 'src/oracle847.rs"))'
                after = (text[:match.start()] + joined + text[match.end():]).encode()
            path.write_bytes(after)
            changes.append({"path":name,"before_sha256":sha(before),"after_sha256":sha(after)})
        (root / "src/oracle847.rs").write_bytes(driver)
        changes.append({"path":"src/oracle847.rs","before_sha256":None,"after_sha256":sha(driver)})
    return {"root":str(root),"engine_head":head,"all_tracked_inputs":rows,"overlay_changes":changes,"driver_sha256":sha(driver) if overlay else None}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--destination", required=True)
    parser.add_argument("--current-head", required=True)
    args = parser.parse_args()
    external(REPO)
    if git("status", "--porcelain").strip():
        raise ValueError("commit the complete current source batch first")
    current = git("rev-parse", args.current_head).decode().strip()
    if current != git("rev-parse", "HEAD").decode().strip():
        raise ValueError("current source HEAD must be the explicitly named clean lane")
    frozen = git("rev-parse", BASE).decode().strip()
    destination = external(args.destination)
    destination.mkdir(parents=True, exist_ok=False)
    driver = (REPO / "docs/evidence/retained-wrapper-rename-847/oracle.rs").read_bytes()
    receipt = {"schema":"semaprax.private-847-source-overlay.v1","status":"source-only, not compiled", "baseline_label":"frozen-1ff counterfactual, not original 039 execution", "original_context":prepare(destination / "frozen-context/AI-Lang", frozen, driver, False), "old_overlay":prepare(destination / "old-overlay/AI-Lang", frozen, driver, True), "current_overlay":prepare(destination / "current-overlay/AI-Lang", current, driver, True)}
    (destination / "source-binding.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"receipt":str(destination / "source-binding.json"),"baseline":frozen,"current":current,"status":receipt["status"]}))
if __name__ == "__main__":
    main()
