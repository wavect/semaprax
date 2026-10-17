"""Pure static controls: no compiler, runtime, authority grant or RPC."""
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

import compiler_execution_authority as authority


def contract():
    rows = []
    for role in ("runtime", "compiler", "configuration", "schema", "dependency"):
        content = role.encode()
        digest = hashlib.sha256(content).hexdigest()
        rows.append(dict(path="/protected/objects/" + digest, sha256=digest,
                         size=len(content), role=role))
    env = dict(PATH="/protected/bin", HOME="/protected/home", TMPDIR="/protected/tmp",
               LANG="C", LC_ALL="C", TZ="UTC")
    return dict(schema=authority.SCHEMA, runtime_binary=rows[0]["path"],
                compiler_binary=rows[1]["path"], inventory=rows,
                workspace_root="/candidate", server_cwd="/candidate",
                evidence_root="/private/evidence", agent_writable_roots=["/candidate"],
                server_env=env, command_env=dict(env), approval_policy="never",
                alternate_execution="disabled", enforcement_proof=None,
                sandbox_policy=dict(type="workspaceWrite", writableRoots=["/candidate"],
                                    networkAccess=False, excludeSlashTmp=True,
                                    excludeTmpdirEnvVar=True))


def parse(value):
    raw = json.dumps(value).encode()
    return authority._parse(raw, hashlib.sha256(raw).hexdigest())


class StaticAuthorityTests(unittest.TestCase):
    def test_plan_is_immutable_and_never_a_grant(self):
        plan = parse(contract())
        self.assertEqual(plan.server_argv[1:], ("app-server", "--listen", "stdio://"))
        self.assertTrue(plan.unresolved)
        self.assertNotIsInstance(plan, authority.ExecutionAuthority)
        with self.assertRaises(TypeError):
            plan.server_env["PATH"] = "/candidate"
        with self.assertRaises(TypeError):
            plan.sandbox_policy["writableRoots"][0] = "/other"
        with self.assertRaises(authority.AuthorityRefusal):
            authority.ExecutionAuthority()

    def test_closed_contract_hostiles(self):
        mutations = [
            lambda c: c.update(schema="other"),
            lambda c: c.update(extra=True),
            lambda c: c.update(enforcement_proof={"qualified": True}),
            lambda c: c.update(approval_policy="on-request"),
            lambda c: c.update(alternate_execution="process/spawn"),
            lambda c: c.update(server_cwd="/other"),
            lambda c: c.update(evidence_root="/candidate/evidence"),
            lambda c: c.update(agent_writable_roots=["/candidate", "/protected"]),
            lambda c: c["sandbox_policy"].update(networkAccess=0),
            lambda c: c["sandbox_policy"].update(excludeSlashTmp=False),
            lambda c: c["sandbox_policy"].update(writableRoots=["/other"]),
            lambda c: c["server_env"].update(DYLD_INSERT_LIBRARIES="evil"),
            lambda c: c["server_env"].update(PATH="/protected/bin:/candidate"),
            lambda c: c["command_env"].update(LANG="other"),
            lambda c: c["inventory"][0].update(size=True),
            lambda c: c["inventory"][0].update(path="/protected/objects/../bad"),
            lambda c: c["inventory"].append(copy.deepcopy(c["inventory"][0])),
            lambda c: c.update(inventory=c["inventory"][:2]),
        ]
        for index, mutation in enumerate(mutations):
            with self.subTest(index=index):
                value = contract()
                mutation(value)
                with self.assertRaises(authority.AuthorityRefusal):
                    parse(value)

    def test_raw_json_hash_duplicates_and_encoding(self):
        for raw, expected in [(b'{"schema":1,"schema":2}', None),
                              ('{}'.encode('utf-16'), None),
                              (b'{}', "0" * 64)]:
            with self.assertRaises(authority.AuthorityRefusal):
                authority._parse(raw, expected or hashlib.sha256(raw).hexdigest())

    def test_descriptor_acquisition_refuses_links_size_and_mutable_protection(self):
        with tempfile.TemporaryDirectory(dir="/Volumes/Semaprax/runtime-tmp") as directory:
            root = Path(directory)
            file = root / "data"
            file.write_bytes(b"abc")
            self.assertEqual(authority._read(file, 3)[0], b"abc")
            for limit in (0, 2):
                with self.assertRaises(authority.AuthorityRefusal):
                    authority._read(file, limit)
            link = root / "link"
            link.symlink_to(file)
            for path in (link, root, root / "missing"):
                with self.assertRaises(authority.AuthorityRefusal):
                    authority._read(path, 100)
            with self.assertRaises(authority.AuthorityRefusal):
                authority._read(file, 3, protected=True)
            with self.assertRaises(authority.AuthorityRefusal):
                authority.admit(file, hashlib.sha256(b"abc").hexdigest())


if __name__ == "__main__":
    unittest.main()
