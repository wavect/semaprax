"""Reproducible native CLI provisioning; never dispatches a model or copies auth."""
from __future__ import annotations
import argparse
import json
import os
import pathlib
import re
import shlex
import sys
import subprocess
import tempfile
from . import pilot_protocol as p
from .pilot_linux_host import admit_provision

VERSION = "2.1.286"
FINGERPRINT = "31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE"
RELEASE = "https://downloads.claude.ai/claude-code-releases/" + VERSION
KEY_URL = "https://downloads.claude.ai/keys/claude-code.asc"
BINARY_SHA256 = "0292fa22ac2fd43e16be9d0e511ddd8347280d6e0ebaca744ef5b27e05d8d0f8"
BINARY_BYTES = 241033208
NAME = "semaprax-issue332-provider"
GUEST_ENV = ["HOME=/home/pilot", "USER=pilot", "LOGNAME=pilot", "PATH=/usr/bin:/bin", "TMPDIR=/work",
             "LANG=C", "LC_ALL=C", "DISABLE_AUTOUPDATER=1", "DISABLE_UPDATES=1",
             "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1", "CLAUDE_CODE_SAFE_MODE=1"]


def manifest_admission(data):
    manifest = p.strict_json(data)
    platform = manifest.get("platforms", {}).get("linux-arm64", {})
    if manifest.get("version") != VERSION or platform != {"binary": "claude", "checksum": BINARY_SHA256, "size": BINARY_BYTES}:
        raise ValueError("native_release_identity_refused")
    return manifest


def verify_release(inputs, gpg):
    """No user's GPG keyring is read or changed; only an isolated public key."""
    inputs = pathlib.Path(inputs)
    files = {name: p.provenance.read_regular(inputs / name, limit) for name, limit in (
        ("manifest.json", 1024 * 1024), ("manifest.json.sig", 65536), ("claude-code.asc", 65536))}
    manifest = manifest_admission(files["manifest.json"])
    with tempfile.TemporaryDirectory(prefix="pilot-release-gpg-") as temporary:
        directory = pathlib.Path(temporary)
        for name, data in files.items():
            (directory / name).write_bytes(data)
        command = [str(gpg), "--batch", "--no-options", "--homedir", temporary]
        def run(args):
            result = subprocess.run([*command, *args], capture_output=True, timeout=15, check=True)
            if len(result.stdout) + len(result.stderr) > 65536:
                raise ValueError("signature_output_bound")
            return result.stdout.decode("utf-8", "strict")
        keys = run(["--with-colons", "--show-keys", str(directory / "claude-code.asc")])
        if [line.split(":")[9] for line in keys.splitlines() if line.startswith("fpr:")] != [FINGERPRINT]:
            raise ValueError("release_signing_key_refused")
        run(["--import", str(directory / "claude-code.asc")])
        status = run(["--status-fd", "1", "--verify", str(directory / "manifest.json.sig"), str(directory / "manifest.json")])
        if "[GNUPG:] VALIDSIG " + FINGERPRINT + " " not in status:
            raise ValueError("release_signature_refused")
    binary = p.provenance.read_regular(inputs / "claude", 256 * 1024 * 1024)
    if (len(binary) != BINARY_BYTES or p.digest(binary) != BINARY_SHA256 or binary[:6] != b"\x7fELF\x02\x01"
            or binary[18:20] != b"\xb7\x00"):
        raise ValueError("native_binary_identity_refused")
    return binary, files, {"manifest_sha256": p.digest(files["manifest.json"]), "signature_sha256": p.digest(files["manifest.json.sig"]),
                           "public_key_sha256": p.digest(files["claude-code.asc"]), "signer_fingerprint": FINGERPRINT,
                           "release_commit": manifest.get("commit"), "build_date": manifest.get("buildDate"),
                           "manifest_url": RELEASE + "/manifest.json", "binary_url": RELEASE + "/linux-arm64/claude",
                           "public_key_url": KEY_URL, "signature_status": status}


def provision(inputs, root, linux_provision, linux_digest, gpg):
    root = pathlib.Path(root)
    if (not root.is_absolute() or root != root.resolve() or root.is_relative_to(p.provenance.ROOT)
            or root.exists() or any(x in str(root) for x in (",", "\n", "\x00"))):
        raise ValueError("fresh_private_provider_root_required")
    pins = admit_provision(p.provenance.read_regular(pathlib.Path(linux_provision), 65536), linux_digest)
    binary, files, provenance = verify_release(pathlib.Path(inputs), pathlib.Path(gpg))
    root.mkdir(mode=0o700)
    for name in ("public", "private-home", "scratch"):
        (root / name).mkdir(mode=0o700)
    for name, data in {**files, "claude": binary}.items():
        path = root / "public" / name
        with path.open("xb") as stream:
            stream.write(data)
        path.chmod(0o500 if name == "claude" else 0o400)
    receipt = {"schema": "benchmark.cross_language.linux_provider_provision.v1", "version": VERSION,
               "binary_sha256": BINARY_SHA256, "binary_bytes": BINARY_BYTES,
               "linux_provision_sha256": linux_digest, "linux_pins": pins, "release": provenance,
               "guest_executable": "/opt/claude/claude", "guest_home": "/home/pilot", "guest_scratch": "/work",
               "authentication": "fresh_guest_login_required", "model_dispatches": 0}
    (root / "provider-provision.json").write_bytes(p.canonical(receipt))
    (root / "provider-provision.json").chmod(0o600)
    write_login_launcher(root, p.digest(p.canonical(receipt)))
    return receipt


def write_login_launcher(root, receipt_digest):
    command = ["/usr/bin/env", "PYTHONPATH=" + str(p.SUITE), sys.executable, "-m", "agent.pilot_linux_provider",
               "login", "--root", str(root), "--receipt-sha256", receipt_digest]
    path = pathlib.Path(root) / "login.sh"
    with path.open("x") as output:
        output.write("#!/bin/sh\nexec " + shlex.join(command) + "\n")
    path.chmod(0o700)


def launch_arguments(root, pins, name=NAME):
    root = pathlib.Path(root)
    if (root != root.resolve() or not root.is_absolute() or any(x in str(root) for x in (",", "\n", "\x00"))
            or not re.fullmatch(r"[a-z0-9][a-z0-9-]{1,62}", name) or os.getuid() == 0 or os.getgid() == 0):
        raise ValueError("provider_launch_identity_refused")
    return ["run", "--detach", "--name", name, "--progress", "none", "--platform", "linux/arm64",
            "--network", "default", "--read-only", "--cap-drop", "ALL", "--cpus", "1", "--memory", "2G",
            "--uid", str(os.getuid()), "--gid", str(os.getgid()), "--workdir", "/work",
            "--mount", f"type=bind,source={root / 'public'},target=/opt/claude,readonly",
            "--mount", f"type=bind,source={root / 'private-home'},target=/home/pilot",
            "--mount", f"type=bind,source={root / 'scratch'},target=/work",
            "--entrypoint", "/usr/bin/env", pins["image"], "-i", *GUEST_ENV, "/usr/bin/sleep", "infinity"]


def exec_arguments(arguments, *, interactive=False, name=NAME):
    # This helper's vocabulary cannot invoke print/prompt/setup-token/model paths.
    if arguments not in (["--version"], ["auth", "status", "--json"], ["auth", "login", "--claudeai"], ["auth", "login", "--help"]):
        raise ValueError("provider_provision_command_refused")
    return ["exec", *(["--interactive", "--tty"] if interactive else []), "--workdir", "/work", name,
            "/usr/bin/env", "-i", *GUEST_ENV, "/opt/claude/claude", *arguments]


def checked_root(root, expected_receipt_sha256):
    root = pathlib.Path(root)
    data = p.provenance.read_regular(root / "provider-provision.json", 65536)
    if p.digest(data) != expected_receipt_sha256:
        raise ValueError("provider_receipt_digest_refused")
    receipt = p.strict_json(data)
    pins = receipt["linux_pins"]
    if p.digest(p.canonical(pins)) != receipt["linux_provision_sha256"]:
        raise ValueError("provider_linux_receipt_refused")
    admit_provision(p.canonical(pins), receipt["linux_provision_sha256"])
    for directory in (root, root / "private-home", root / "scratch", root / "public"):
        stat = directory.lstat()
        if directory.is_symlink() or not directory.is_dir() or stat.st_uid != os.getuid() or stat.st_mode & 0o077:
            raise ValueError("provider_private_directory_refused")
    if p.provenance.file_digest(root / "public" / "claude", 256 * 1024 * 1024) != (BINARY_BYTES, BINARY_SHA256):
        raise ValueError("provider_staged_binary_drifted")
    cli = pathlib.Path(pins["container_path"])
    if p.provenance.file_digest(cli, 128 * 1024 * 1024)[1] != pins["container_sha256"]:
        raise ValueError("provider_container_cli_drifted")
    p.provenance.host_identity()
    environment = {"HOME": str(pathlib.Path.home()), "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}
    return root, pins, cli, environment


def start(root, expected_receipt_sha256, name=NAME):
    root, pins, cli, environment = checked_root(root, expected_receipt_sha256)
    def run(args):
        return subprocess.run([str(cli), *args], env=environment, capture_output=True, timeout=30, check=True)
    image = json.loads(run(["image", "inspect", pins["image"]]).stdout)
    if len(image) != 1 or image[0]["configuration"]["descriptor"]["digest"] != pins["image"].split("@")[1]:
        raise ValueError("provider_image_refused")
    run(launch_arguments(root, pins, name))
    try:
        version = run(exec_arguments(["--version"], name=name)).stdout.decode().strip()
        if version != VERSION + " (Claude Code)":
            raise ValueError("provider_version_refused")
        # Only nonsecret kernel/boot metadata is retained. No auth files are read.
        facts = run(["exec", name, "/bin/sh", "-c", "uname -m; uname -r; cat /proc/sys/kernel/random/boot_id"]).stdout.decode().splitlines()
        if len(facts) != 3 or facts[:2] != ["aarch64", pins["kernel_release"]] or not re.fullmatch(r"[a-f0-9-]{36}", facts[2]):
            raise ValueError("provider_guest_identity_refused")
        observed = {"version": version, "native_platform": "linux-arm64", "kernel_release": facts[1], "boot_id": facts[2],
                    "container": name, "executable": "/opt/claude/claude", "claude_sha256": BINARY_SHA256,
                    "home": "/home/pilot", "login": "pilot", "model_dispatches": 0}
        (root / "guest-observation.json").write_bytes(p.canonical(observed))
        return observed
    except Exception:
        subprocess.run([str(cli), "delete", "--force", name], env=environment, capture_output=True, timeout=15)
        raise


def login(root, expected_receipt_sha256):
    """Interactive user terminal only; never capture or retain its auth output."""
    root, _, cli, environment = checked_root(root, expected_receipt_sha256)
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        raise ValueError("login_requires_user_terminal")
    observed = p.strict_json(p.provenance.read_regular(root / "guest-observation.json", 65536))
    facts = subprocess.run([str(cli), "exec", NAME, "/bin/sh", "-c",
                            "sha256sum /opt/claude/claude; cat /proc/sys/kernel/random/boot_id"],
                           env=environment, capture_output=True, timeout=15, check=True).stdout.decode().splitlines()
    if facts != [BINARY_SHA256 + "  /opt/claude/claude", observed["boot_id"]]:
        raise ValueError("login_guest_identity_drifted")
    return subprocess.call([str(cli), *exec_arguments(["auth", "login", "--claudeai"], interactive=True)], env=environment)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("provision")
    for flag in ("inputs", "root", "linux-provision", "linux-digest", "gpg"):
        prepare.add_argument("--" + flag, required=True)
    for name in ("start", "login"):
        launch = commands.add_parser(name)
        launch.add_argument("--root", required=True)
        launch.add_argument("--receipt-sha256", required=True)
    args = parser.parse_args()
    if args.command == "provision":
        receipt = provision(args.inputs, args.root, args.linux_provision, args.linux_digest, args.gpg)
        print(json.dumps({"receipt_sha256": p.digest(p.canonical(receipt)), "model_dispatches": 0}))
    elif args.command == "start":
        print(json.dumps(start(args.root, args.receipt_sha256)))
    else:
        raise SystemExit(login(args.root, args.receipt_sha256))


if __name__ == "__main__":
    main()
