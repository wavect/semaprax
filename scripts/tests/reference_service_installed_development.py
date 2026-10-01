#!/usr/bin/env python3
"""Run the packaged reference-service development journey from an independent path.

The caller explicitly supplies a locally trusted checker and executable.  This
script packages their exact executable bytes, then serves only from the copied
package path.  It establishes local installed-development execution evidence;
it does not establish release provenance, OCI execution, or publication.
"""
import argparse
import hashlib
import http.client
import json
from pathlib import Path
import select
import socket
import subprocess
import sys
import tempfile
import time

READY_TIMEOUT = 300
REQUEST_TIMEOUT = 30
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


class Server:
    def __init__(self, executable, package, workspace, state=None):
        self.port = port()
        self.executable = executable
        command = [
            str(executable), "serve",
            "--project", str(package / "service"),
            "--config", str(package / "service" / "service.config.json"),
            "--state-dir", str(workspace / "state"),
            "--outbound-dir", str(workspace / "outbound"),
            "--secrets-dir", str(workspace / "secrets"),
            "--bundle-dir", str(workspace / "bundle"),
            "--port", str(self.port),
        ]
        if state is not None:
            command += ["--state", state]
        self.process = subprocess.Popen(
            command,
            cwd=workspace,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.wait_ready()

    def wait_ready(self):
        deadline = time.monotonic() + READY_TIMEOUT
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                stderr = self.process.stderr.read()
                raise AssertionError(f"packaged service exited before ready: {stderr}")
            readable, _, _ = select.select([self.process.stdout], [], [], 0.25)
            if not readable:
                continue
            line = self.process.stdout.readline()
            if line.startswith("ready "):
                return
        self.stop()
        raise AssertionError("packaged service did not print ready before timeout")

    def stop(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=REQUEST_TIMEOUT)


def refuse(executable, package, workspace, label, expected):
    command = [
        str(executable), "serve",
        "--project", str(package / "service"),
        "--config", str(package / "service" / "service.config.json"),
        "--state-dir", str(workspace / "state"),
        "--outbound-dir", str(workspace / "outbound"),
        "--secrets-dir", str(workspace / "secrets"),
        "--bundle-dir", str(workspace / "bundle"),
        "--port", "9",
    ]
    result = subprocess.run(command, cwd=workspace, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            text=True, timeout=REQUEST_TIMEOUT, check=False)
    if result.returncode != 2 or expected not in result.stderr:
        raise AssertionError(f"{label} did not fail closed: exit={result.returncode}, stderr={result.stderr!r}")


def write_secrets(workspace):
    directory = workspace / "secrets"
    for name, value in SECRET_VALUES.items():
        (directory / name).write_bytes(value)


def inventory(directory):
    return sorted(item.name for item in directory.iterdir())


def package(args, workspace):
    output = workspace / "package"
    command = [
        sys.executable, str(args.packager), "--format", "development",
        "--checker", str(args.checker), "--executable", str(args.executable),
        "--executable-sha256", sha256(args.executable), "--project", str(args.project),
        "--config", str(args.config), "--output", str(output),
    ]
    result = subprocess.run(command, cwd=workspace, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=120, check=False, text=True)
    if result.returncode != 0:
        raise AssertionError(
            "reference-service packager failed: "
            f"exit={result.returncode}, stdout={result.stdout!r}, "
            f"stderr={result.stderr!r}"
        )
    copied = (output / "bin" / "semaprax-reference-service").resolve()
    if copied == args.executable.resolve() or not copied.is_file():
        raise AssertionError("packager did not create an independent runtime executable")
    receipt = json.loads((output / "service-package.json").read_text())
    if "runtime_execution_not_performed" not in receipt["nonclaims"]:
        raise AssertionError("package receipt lost its execution nonclaim")
    return output, copied


def journey(executable, package, workspace):
    original_config = (package / "service" / "service.config.json").read_text()
    adapter_config = original_config.replace('"adapter":"snapshot"', '"adapter":"sqlite"')
    (package / "service" / "service.config.json").write_text(adapter_config)
    refuse(executable, package, workspace, "unsupported adapter",
           "service database adapter sqlite is unsupported")
    if inventory(workspace / "state") or inventory(workspace / "outbound") or inventory(workspace / "bundle"):
        raise AssertionError("adapter refusal touched runtime directories")
    (package / "service" / "service.config.json").write_text(original_config)

    refuse(executable, package, workspace, "missing secret", "cannot resolve every named host secret")
    if inventory(workspace / "state") or inventory(workspace / "outbound") or inventory(workspace / "bundle"):
        raise AssertionError("secret refusal touched runtime directories")
    write_secrets(workspace)

    server = Server(executable, package, workspace)
    try:
        expect(request(server.port, "POST", "/v1/register",
                       '{"username":"alice","password":"correct horse 7"}'), 201, "register")
        login = expect(request(server.port, "POST", "/v1/login",
                               '{"username":"alice","password":"correct horse 7"}'), 200, "login")
        token = login["token"]
        expect(request(server.port, "POST", "/v1/tasks", '{"title":"write the report"}', token),
               201, "create task")
        expect(request(server.port, "PATCH", "/v1/tasks/1", '{"status":"done"}', token),
               200, "update task")
        expect(request(server.port, "POST", "/v1/tasks", '{"title":"discard me"}', token),
               201, "create deletable task")
        expect(request(server.port, "DELETE", "/v1/tasks/2", "", token),
               200, "delete task")
        enqueue = expect(request(server.port, "POST", "/v1/jobs/enqueue",
                                 '{"key":"job-1","desc":"task-1"}', token), 200, "enqueue")
        if enqueue["outcome"] != "created":
            raise AssertionError(f"first enqueue did not create: {enqueue}")
        completed = expect(request(server.port, "POST", "/v1/jobs/1/complete", "", token),
                           200, "complete job")
        if completed["webhook"] != "uncertain":
            raise AssertionError(f"offline delivery did not settle uncertain: {completed}")
        state = completed["state"]
        expect(request(server.port, "GET", "/v1/jobs/1", "", token), 200, "read job")
        outbound_before = inventory(workspace / "outbound")
        state_before = inventory(workspace / "state")
    finally:
        server.stop()

    server = Server(executable, package, workspace, state)
    try:
        health = expect(request(server.port, "GET", "/v1/health", ""), 200, "restart health")
        if health["state"] != state:
            raise AssertionError("restart did not bind the retained state digest")
        task = expect(request(server.port, "GET", "/v1/tasks/1", "", token), 200, "restart task")
        if (task["title"], task["status"]) != ("write the report", "done"):
            raise AssertionError(f"restart lost task state: {task}")
        job = expect(request(server.port, "GET", "/v1/jobs/1", "", token), 200, "restart job")
        if (job["state"], job["webhook"]) != ("completed", "uncertain"):
            raise AssertionError(f"restart lost job settlement: {job}")
        expect(request(server.port, "POST", "/v1/jobs/1/complete", "", token), 409, "repeat completion")
        duplicate = expect(request(server.port, "POST", "/v1/jobs/enqueue",
                                   '{"key":"job-1","desc":"task-1"}', token), 200, "duplicate enqueue")
        if duplicate["outcome"] != "duplicate" or duplicate["state"] != state:
            raise AssertionError(f"restart redispatched job: {duplicate}")
        if inventory(workspace / "outbound") != outbound_before or inventory(workspace / "state") != state_before:
            raise AssertionError("restart changed settled delivery or state inventory")
    finally:
        server.stop()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packager", type=Path, required=True)
    parser.add_argument("--checker", type=Path, required=True)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--config", type=Path, required=True)
    args = parser.parse_args()
    for path in (args.packager, args.checker, args.executable, args.project, args.config):
        if not path.exists():
            parser.error(f"missing input: {path}")
    with tempfile.TemporaryDirectory(prefix="semaprax-reference-installed-") as temporary:
        # Authenticate project ancestors against the physical macOS /private
        # tree rather than TemporaryDirectory's common /var spelling.
        workspace = Path(temporary).resolve(strict=True)
        for name in ("state", "outbound", "secrets", "bundle"):
            (workspace / name).mkdir()
        package_path, copied = package(args, workspace)
        journey(copied, package_path, workspace)
    print("packaged reference-service installed-development journey passed")


if __name__ == "__main__":
    main()
