#!/usr/bin/env python3
"""Explicit, separate provisioning for the local Clef worker. Route evaluation never runs this.

    provision.py --profile clef-flash --dest DIR            # resource preview only (default, no network)
    provision.py --profile clef-flash --dest DIR --download --accept-resources

Downloads the PINNED revision's files from huggingface.co over https (stdlib
urllib), verifies every sha256 against clef.lock.json, and resumes nothing
silently. It never pip-installs: the python requirements are printed for you
to install into your own environment.
"""

import argparse
import os
import shutil
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import clef_identity as ident  # noqa: E402

GIB = 1 << 30
MARGIN = 4 * GIB  # keep this much free after the download


def preview(rel, dest, out=print):
    free = shutil.disk_usage(os.path.dirname(os.path.abspath(dest)) or ".").free
    out(f"release   {rel['repo']} @ {rel['revision']}  status={rel['status']}  license={rel['license']}")
    for name, m in rel["files"].items():
        out(f"  {m['bytes']:>14,d}  {m['group']:<9} {name}")
    out(f"download  {rel['total_bytes']:,d} bytes ({rel['total_bytes'] / GIB:.2f} GiB) weights+code, plus python deps: "
        + ", ".join(f"{k}{'==' + v if v != '*' else ''}" for k, v in rel["requirements"].items()))
    out(f"disk free {free / GIB:.2f} GiB at destination; needs {(rel['total_bytes'] + MARGIN) / GIB:.2f} GiB (download + {MARGIN // GIB} GiB margin)")
    out(f"devices   {', '.join(rel['devices'])}")
    return free >= rel["total_bytes"] + MARGIN


def fetch(rel, dest):
    os.makedirs(dest, exist_ok=True)
    for name, m in rel["files"].items():
        target = os.path.join(dest, name)
        if os.path.isfile(target) and os.path.getsize(target) == m["bytes"] and ident.sha256_file(target) == m["sha256"]:
            print("have", name)
            continue
        url = f"https://huggingface.co/{rel['repo']}/resolve/{rel['revision']}/{name}"
        part = target + ".part"
        with urllib.request.urlopen(url, timeout=60) as r, open(part, "wb") as f:
            shutil.copyfileobj(r, f, 1 << 20)
        if os.path.getsize(part) != m["bytes"] or ident.sha256_file(part) != m["sha256"]:
            os.remove(part)
            raise SystemExit("digest mismatch for " + name + "; nothing kept")
        os.replace(part, target)
        print("ok  ", name)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--profile", default="clef-flash")
    ap.add_argument("--dest", required=True)
    ap.add_argument("--lock", default=os.environ.get(ident.LOCK_ENV))
    ap.add_argument("--download", action="store_true")
    ap.add_argument("--accept-resources", action="store_true", help="acknowledge the previewed size and dependencies")
    a = ap.parse_args(argv)
    rel = ident.release(ident.load_lock(a.lock), a.profile)
    fits = preview(rel, a.dest)
    if not a.download:
        print("preview only; nothing downloaded. Re-run with --download --accept-resources to provision.")
        return 0
    if rel.get("status") != "supported":
        raise SystemExit(f"{a.profile} is not a supported profile (status {rel.get('status')}); refusing")
    if not a.accept_resources:
        raise SystemExit("refusing: pass --accept-resources after reading the preview")
    if not fits:
        raise SystemExit("refusing: not enough free disk for the pinned release")
    fetch(rel, a.dest)
    ident.verify_dir(a.dest, rel)
    print("verified; start the worker with: python3 worker.py --model-dir", a.dest, "--profile", a.profile)
    return 0


if __name__ == "__main__":
    sys.exit(main())
