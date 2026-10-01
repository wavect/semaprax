#!/usr/bin/env python3
"""Exercise the packaged reference service through a local Podman OCI runtime.

The caller supplies the static Linux executable whose digest is copied into an
offline OCI layout.  The journey imports that layout into a local Podman image,
then mounts the host-owned state, outbound, secret, and bundle directories as
the service's only runtime adapters.  It needs Linux and an explicitly selected
Podman daemon; it neither contacts a registry nor establishes release
provenance or publication.
"""
import argparse
import hashlib
import http.client
import json
from pathlib import Path
import os
import select
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import time

READY_TIMEOUT = 300
REQUEST_TIMEOUT = 30
MAX_READY_OUTPUT_BYTES = 64 * 1024
IMAGE = "semaprax-reference-service:local"
SECRET_VALUES = {
    "auth.pepper": bytes([1]) * 32,
    "auth.session": bytes([2]) * 32,
    "webhook.signing": bytes([3]) * 32,
}


def sha256(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def request(port_number, method, target, body=None, token=None):
    connection = http.client.HTTPConnection("127.0.0.1", port_number, timeout=REQUEST_TIMEOUT)
    headers = {}
    if body is not None:
        headers["Content-Type"] = "application/json"
    if token is not None:
        headers["Authorization"] = "Bearer " + token
    connection.request(method, target, body=body, headers=headers)
    response = connection.getresponse()
    payload = response.read().decode("utf-8")
    connection.close()
    if not payload:
        return response.status, None
    try:
        return response.status, json.loads(payload)
    except json.JSONDecodeError as error:
        raise AssertionError(f"{method} {target} returned non-JSON {payload!r}") from error


def expect(response, status, context):
    actual, body = response
    if actual != status:
        raise AssertionError(f"{context}: expected HTTP {status}, got {actual}: {body}")
    return body


def inventory(directory):
    return sorted(item.name for item in directory.iterdir())


def write_secrets(workspace):
    for name, value in SECRET_VALUES.items():
        (workspace / "secrets" / name).write_bytes(value)


def package(args, workspace):
    output = workspace / "package"
    command = [
        sys.executable, str(args.packager), "--format", "oci",
        "--checker", str(args.checker), "--executable", str(args.executable),
        "--executable-sha256", sha256(args.executable), "--project", str(args.project),
        "--config", str(args.config), "--output", str(output),
    ]
    result = subprocess.run(command, cwd=workspace, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=120, check=False, text=True)
    if result.returncode != 0:
        raise AssertionError(
            "reference-service OCI packager failed: "
            f"exit={result.returncode}, stdout={result.stdout!r}, stderr={result.stderr!r}"
        )
    receipt = json.loads((output / "service-package.json").read_text())
    if receipt["format"] != "oci" or "runtime_execution_not_performed" not in receipt["nonclaims"]:
        raise AssertionError("OCI package receipt does not describe the expected offline layout")
    return output


def oci_archive(layout, archive):
    # `podman load` consumes an OCI archive.  Adding only layout-owned entries
    # keeps temp roots and host directory names out of the imported image.
    with tarfile.open(archive, "w") as output:
        for item in sorted(layout.rglob("*"), key=lambda path: str(path.relative_to(layout))):
            output.add(item, arcname=str(item.relative_to(layout)), recursive=False)


class Podman:
    def __init__(self, executable, workspace):
        self.executable = executable
        self.workspace = workspace
        self.name = f"semaprax-reference-service-{os.getpid()}-{time.monotonic_ns()}"

    def command(self, *args, **kwargs):
        return subprocess.run([str(self.executable), *args], stdin=subprocess.DEVNULL,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                              **kwargs)

    def require_available(self):
        result = self.command("info", "--format", "{{.Host.OS}}", timeout=REQUEST_TIMEOUT)
        if result.returncode != 0 or result.stdout.strip() != "linux":
            raise RuntimeError("Podman must be available with a Linux OCI runtime")
        existing = self.command("image", "exists", IMAGE, timeout=REQUEST_TIMEOUT)
        if existing.returncode == 0:
            raise RuntimeError(f"refusing to replace existing local image {IMAGE}")

    def load(self, layout):
        archive = self.workspace / "reference-service.oci.tar"
        oci_archive(layout, archive)
        result = self.command("load", "--input", str(archive), timeout=120)
        if result.returncode != 0:
            raise AssertionError(f"OCI import failed: {result.stderr}")
        found = self.command("image", "exists", IMAGE, timeout=REQUEST_TIMEOUT)
        if found.returncode != 0:
            raise AssertionError(f"OCI import did not create {IMAGE}: {result.stdout} {result.stderr}")

    def start(self, port_number, state=None, config_override=None):
        command = [
            str(self.executable), "run", "--rm", "--name", self.name,
            "--network", "host", "--userns", "keep-id",
            "--user", f"{os.getuid()}:{os.getgid()}",
        ]
        for name in ("state", "outbound", "bundle"):
            command += ["--volume", f"{self.workspace / name}:/{name}:rw"]
        command += ["--volume", f"{self.workspace / 'secrets'}:/secrets:ro"]
        if config_override is not None:
            command += ["--volume", f"{config_override}:/service/service.config.json:ro"]
        command += [IMAGE, "--port", str(port_number)]
        if state is not None:
            command += ["--state", state]
        return subprocess.Popen(command, cwd=self.workspace, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    def stop(self, process):
        if process.poll() is None:
            self.command("stop", "--time", "1", self.name, timeout=REQUEST_TIMEOUT)
        process.wait(timeout=REQUEST_TIMEOUT)

    def cleanup(self):
        self.command("rm", "--force", self.name, timeout=REQUEST_TIMEOUT)
        self.command("image", "rm", IMAGE, timeout=REQUEST_TIMEOUT)


def wait_ready(process):
    deadline = time.monotonic() + READY_TIMEOUT
    buffered = b""
    descriptor = process.stdout.fileno()
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise AssertionError(f"OCI service exited before ready: {process.stderr.read()}")
        readable, _, _ = select.select([descriptor], [], [], 0.25)
        if not readable:
            continue
        chunk = os.read(descriptor, 4096)
        if not chunk:
            continue
        buffered += chunk
        if len(buffered) > MAX_READY_OUTPUT_BYTES:
            raise AssertionError("OCI service exceeded ready-output byte bound")
        while b"\n" in buffered:
            line, buffered = buffered.split(b"\n", 1)
            if line.startswith(b"ready "):
                return
    raise AssertionError("OCI service did not print ready before timeout")


def expect_refusal(runtime, workspace, label, expected, config_override=None):
    process = runtime.start(port(), config_override=config_override)
    try:
        result = process.wait(timeout=REQUEST_TIMEOUT)
        stderr = process.stderr.read()
    finally:
        runtime.stop(process)
    if result != 2 or expected not in stderr:
        raise AssertionError(f"{label} did not fail closed: exit={result}, stderr={stderr!r}")


def expect_missing_secret(runtime, workspace):
    expect_refusal(runtime, workspace, "missing secret", "cannot resolve every named host secret")
    if inventory(workspace / "state") or inventory(workspace / "outbound") or inventory(workspace / "bundle"):
        raise AssertionError("missing-secret OCI refusal touched physical runtime adapters")


def expect_unsupported_adapter(runtime, workspace, configuration):
    modified = configuration.replace(b'"adapter":"snapshot"', b'"adapter":"sqlite"')
    if modified == configuration:
        raise AssertionError("OCI fixture does not carry the expected snapshot adapter")
    override = workspace / "unsupported-adapter.config.json"
    override.write_bytes(modified)
    expect_refusal(runtime, workspace, "unsupported adapter",
                   "service database adapter sqlite is unsupported", override)
    if inventory(workspace / "state") or inventory(workspace / "outbound") or inventory(workspace / "bundle"):
        raise AssertionError("adapter OCI refusal touched physical runtime adapters")


def expect_bundle_refusal(runtime, workspace):
    poison = workspace / "bundle" / "service.config.json"
    poison.write_bytes(b'{"poisoned":true}\n')
    expect_refusal(runtime, workspace, "run bundle mismatch", "cannot write the run bundle")
    if inventory(workspace / "state") or inventory(workspace / "outbound"):
        raise AssertionError("bundle OCI refusal touched state or outbound adapters")
    if inventory(workspace / "bundle") != ["service.config.json"] or poison.read_bytes() != b'{"poisoned":true}\n':
        raise AssertionError("bundle OCI refusal changed the pre-existing bundle input")
    poison.unlink()


def journey(runtime, workspace, configuration):
    expect_unsupported_adapter(runtime, workspace, configuration)
    expect_missing_secret(runtime, workspace)
    write_secrets(workspace)
    expect_bundle_refusal(runtime, workspace)
    process = runtime.start(port())
    try:
        wait_ready(process)
        port_number = int(process.args[-1])
        expect(request(port_number, "POST", "/v1/register",
                       '{"username":"alice","password":"correct horse 7"}'), 201, "register")
        login = expect(request(port_number, "POST", "/v1/login",
                               '{"username":"alice","password":"correct horse 7"}'), 200, "login")
        token = login["token"]
        expect(request(port_number, "POST", "/v1/tasks", '{"title":"write the report"}', token),
               201, "create task")
        expect(request(port_number, "PATCH", "/v1/tasks/1", '{"status":"done"}', token),
               200, "update task")
        expect(request(port_number, "POST", "/v1/tasks", '{"title":"discard me"}', token),
               201, "create deletable task")
        expect(request(port_number, "DELETE", "/v1/tasks/2", "", token), 200, "delete task")
        enqueue = expect(request(port_number, "POST", "/v1/jobs/enqueue",
                                 '{"key":"job-1","desc":"task-1"}', token), 200, "enqueue")
        if enqueue["outcome"] != "created":
            raise AssertionError(f"first enqueue did not create: {enqueue}")
        completed = expect(request(port_number, "POST", "/v1/jobs/1/complete", "", token),
                           200, "complete job")
        if completed["webhook"] != "uncertain":
            raise AssertionError(f"physical outbound adapter did not record uncertainty: {completed}")
        state = completed["state"]
        outbound_before = inventory(workspace / "outbound")
        state_before = inventory(workspace / "state")
    finally:
        runtime.stop(process)

    restart_port = port()
    process = runtime.start(restart_port, state)
    try:
        wait_ready(process)
        health = expect(request(restart_port, "GET", "/v1/health", ""), 200, "restart health")
        if health["state"] != state:
            raise AssertionError("restart did not bind retained state digest")
        task = expect(request(restart_port, "GET", "/v1/tasks/1", "", token), 200, "restart task")
        if (task["title"], task["status"]) != ("write the report", "done"):
            raise AssertionError(f"restart lost task state: {task}")
        job = expect(request(restart_port, "GET", "/v1/jobs/1", "", token), 200, "restart job")
        if (job["state"], job["webhook"]) != ("completed", "uncertain"):
            raise AssertionError(f"restart lost job settlement: {job}")
        expect(request(restart_port, "POST", "/v1/jobs/1/complete", "", token),
               409, "repeat completion")
        duplicate = expect(request(restart_port, "POST", "/v1/jobs/enqueue",
                                   '{"key":"job-1","desc":"task-1"}', token),
                           200, "duplicate enqueue")
        if duplicate["outcome"] != "duplicate" or duplicate["state"] != state:
            raise AssertionError(f"restart redispatched job: {duplicate}")
        if inventory(workspace / "outbound") != outbound_before or inventory(workspace / "state") != state_before:
            raise AssertionError("restart changed settled physical adapter inventory")
    finally:
        runtime.stop(process)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packager", type=Path, required=True)
    parser.add_argument("--checker", type=Path, required=True)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--podman", type=Path, default=Path("podman"))
    args = parser.parse_args()
    for path in (args.packager, args.checker, args.executable, args.project, args.config):
        if not path.exists():
            parser.error(f"missing input: {path}")
    if shutil.which(str(args.podman)) is None and not args.podman.is_file():
        parser.error("Podman executable is unavailable")
    if sys.platform != "linux":
        parser.error("OCI runtime journey requires a Linux host network namespace")
    with tempfile.TemporaryDirectory(prefix="semaprax-reference-oci-") as temporary:
        workspace = Path(temporary).resolve(strict=True)
        for name in ("state", "outbound", "secrets", "bundle"):
            (workspace / name).mkdir()
        runtime = Podman(args.podman, workspace)
        runtime.require_available()
        try:
            runtime.load(package(args, workspace))
            journey(runtime, workspace, args.config.read_bytes())
        finally:
            runtime.cleanup()
    print("packaged reference-service OCI runtime journey passed")


if __name__ == "__main__":
    main()
