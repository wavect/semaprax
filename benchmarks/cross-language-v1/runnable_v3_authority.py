"""Deny-default Darwin execution authority, using the existing bounded runner."""
from __future__ import annotations
import json
import base64
import os
import pathlib
import socket
import time
import runnable_adapter as v1
import runnable_v3_provenance as p

SYSCTLS = ("kern.ostype", "kern.osrelease", "kern.version", "kern.hostname", "hw.machine")
SANDBOX = pathlib.Path("/usr/bin/sandbox-exec")


def profile(runtime, phase):
    phase = pathlib.Path(phase)
    if phase != phase.resolve() or not phase.is_dir() or phase.is_symlink():
        raise p.Error("phase_directory_not_private")
    literal = lambda path: "(literal " + json.dumps(str(path), ensure_ascii=True) + ")"
    subtree = lambda path: "(subpath " + json.dumps(str(path), ensure_ascii=True) + ")"
    ancestors = sorted({str(x) for path in (runtime.root, phase) for x in (*path.parents, path)})
    return ("(version 1)(deny default)(allow process-exec " + literal(runtime.node) + ")"
            + "(allow file-read* " + " ".join([literal("/"), literal(runtime.node), subtree(runtime.root / "typescript"),
                                               subtree(phase), subtree("/usr/lib"), subtree("/System/Library")]) + ")"
            + "(allow file-read-metadata " + " ".join(literal(x) for x in ancestors) + ")"
            + "(allow file-write* " + subtree(phase) + ")"
            + '(allow file-read* file-write* (literal "/dev/null"))'
            + "(allow sysctl-read " + " ".join("(sysctl-name " + json.dumps(x) + ")" for x in SYSCTLS) + ")")


class Authority:
    def __init__(self, runtime):
        self.runtime = runtime
        data = p.read_regular(SANDBOX, 1024 * 1024)
        v1._admit_host_executable(SANDBOX, "sha256:" + p.digest(data), "sandbox")
        self.sandbox_hash = p.digest(data)
        self.commands = []

    def launch(self, command, cwd, deadline):
        self.runtime.check()
        sandbox_now = p.read_regular(SANDBOX, 1024 * 1024)
        if p.digest(sandbox_now) != self.sandbox_hash:
            raise p.Error("sandbox_identity_drifted")
        v1._admit_host_executable(SANDBOX, "sha256:" + self.sandbox_hash, "sandbox")
        if not command or command[0] != str(self.runtime.node):
            raise p.Error("unbound_process_command_refused")
        policy = profile(self.runtime, cwd)
        wrapped = [str(SANDBOX), "-p", policy, *command]
        code, out, err, reason = v1._run_bounded_group(wrapped, pathlib.Path(cwd), deadline, dict(v1.CLOSED_ENVIRONMENT))
        self.commands.append({"argv": command, "sandbox_argv": wrapped[:3], "cwd": str(cwd),
                              "env": dict(v1.CLOSED_ENVIRONMENT), "status": code, "stdout": out.decode("utf-8", "replace"),
                              "stderr": err.decode("utf-8", "replace"), "stdout_base64": base64.b64encode(out).decode(),
                              "stderr_base64": base64.b64encode(err).decode(), "failure": reason})
        return (None if reason else code), out.decode("utf-8", "replace"), reason or err.decode("utf-8", "replace")

    def preflight(self, root):
        """Known host-authored probe only; never arbitrary external source."""
        root = pathlib.Path(root)
        phase = root / "authority-phase"
        phase.mkdir(mode=0o700)
        sibling = root / "hidden-sibling"
        sibling.mkdir(mode=0o700)
        forbidden = sibling / "canary"
        forbidden.write_text("forbidden-private-canary")
        listener = socket.socket()
        listener.bind(("127.0.0.1", 0))
        listener.listen()
        address = listener.getsockname()
        control = socket.create_connection(address, timeout=2)
        accepted, _ = listener.accept()
        control.close()
        accepted.close()
        code = ("const fs=require('fs'),cp=require('child_process'),net=require('net');"
                "let failures=0;function denied(name,f){try{f();console.log(name+':UNEXPECTED');failures++}"
                "catch(e){console.log(name+':'+e.code);if(e.code!=='EPERM')failures++}}"
                "denied('read',()=>fs.readFileSync(" + json.dumps(str(forbidden)) + "));"
                "denied('write',()=>fs.writeFileSync(" + json.dumps(str(forbidden) + "-write") + ",'x'));"
                "for(const executable of ['/usr/bin/true',process.execPath]){"
                "const r=cp.spawnSync(executable,executable===process.execPath?['--version']:[]);"
                "console.log('spawn:'+executable+':'+(r.error?r.error.code:r.status));"
                "if(!r.error||r.error.code!=='EPERM')failures++}"
                "const c=net.connect(" + str(address[1]) + ",'127.0.0.1');"
                "c.on('connect',()=>{console.log('network:UNEXPECTED');failures++;c.destroy()});"
                "c.on('error',e=>{console.log('network:'+e.code);if(e.code!=='EPERM')failures++});"
                "setTimeout(()=>{c.destroy();process.exitCode=failures?1:0},100);")
        try:
            status, stdout, stderr = self.launch([str(self.runtime.node), "-e", code], phase, time.monotonic() + 10)
        finally:
            listener.close()
        expected = ["read:EPERM", "write:EPERM", "spawn:/usr/bin/true:EPERM",
                    "spawn:" + str(self.runtime.node) + ":EPERM", "network:EPERM"]
        if status != 0 or stdout.splitlines() != expected:
            raise p.Error("sandbox_authority_probe_failed:" + stderr[-200:])
        return {"listener_positive_connect": True, "listener_positive_accept": True,
                "listener_address": list(address), "denials": stdout.splitlines(), "sandbox_sha256": self.sandbox_hash}
