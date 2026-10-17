"""Pure closed v2 loader-route controls; no actual configuration/runtime I/O."""
import copy
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import compiler_configuration_provisioning as v2
import compiler_execution_authority as old


def contract():
    artifact = lambda role, byte: dict(path='/store/'+hashlib.sha256(byte).hexdigest(),
        sha256=hashlib.sha256(byte).hexdigest(), size=len(byte), role=role)
    runtime = dict(path='/store/'+v2.RUNTIME_SHA256, sha256=v2.RUNTIME_SHA256,
                   size=10, role='runtime')
    compiler = artifact('compiler', b'compiler')
    schema = dict(path='/store/'+v2.CONFIG_SCHEMA_SHA256, sha256=v2.CONFIG_SCHEMA_SHA256,
                  size=10, role='schema')
    config = artifact('configuration', b'model="fixed"\n')
    config['path'] = '/protected/home/.codex/config.toml'
    env = dict(PATH='/protected/bin', HOME='/protected/home', TMPDIR='/protected/tmp',
               CODEX_HOME='/protected/home/.codex', LANG='C', LC_ALL='C', TZ='UTC')
    return dict(schema=v2.SCHEMA, runtime_binary=runtime['path'], compiler_binary=compiler['path'],
                inventory=[runtime, compiler, config, schema],
                server_cwd='/trial/sem', workspace_root='/trial/sem', evidence_root='/protected/evidence',
                agent_writable_roots=['/trial/sem'], server_env=env, command_env=copy.deepcopy(env),
                sandbox_policy=dict(type='workspaceWrite', writableRoots=['/trial/sem'],
                    networkAccess=False, excludeSlashTmp=True, excludeTmpdirEnvVar=True),
                approval_policy='never', alternate_execution='disabled', enforcement_proof=None,
                configuration_loader=dict(kind='explicit-codex-home.v1',
                    codex_home=env['CODEX_HOME'], config_path=config['path'],
                    runtime_sha256=v2.RUNTIME_SHA256, config_schema_sha256=v2.CONFIG_SCHEMA_SHA256,
                    app_server_help_sha256=v2.APP_SERVER_HELP_SHA256))


def wire(value):
    raw = json.dumps(value, separators=(',', ':')).encode()
    return raw, hashlib.sha256(raw).hexdigest()


class UserHomeTests(unittest.TestCase):
    def test_named_route_restores_real_plan_and_keeps_v1_parser_unchanged(self):
        value = contract()
        raw, digest = wire(value)
        actual_parse = old._parse
        calls = []
        def observe(raw, digest):
            calls.append(json.loads(raw))
            return actual_parse(raw, digest)
        with patch.object(old, '_parse', side_effect=observe):
            plan = v2.parse_user_home(raw, digest)
        self.assertEqual(plan.contract_sha256, digest)
        self.assertEqual(str(v2.validate_user_home_plan(plan)), '/protected/home/.codex/config.toml')
        self.assertEqual(dict(plan.server_env), value['server_env'])
        self.assertEqual(plan.server_argv, (value['runtime_binary'], 'app-server', '--listen', 'stdio://'))
        self.assertEqual(len(calls), 1)
        adapted = calls[0]
        self.assertEqual(adapted['schema'], old.SCHEMA)
        self.assertNotIn('CODEX_HOME', adapted['server_env'])
        original_config = next(r for r in value['inventory'] if r['role'] == 'configuration')
        adapted_config = next(r for r in adapted['inventory'] if r['role'] == 'configuration')
        self.assertEqual(adapted_config['path'], '/protected/home/.codex/'+original_config['sha256'])
        self.assertEqual({k:v for k,v in adapted_config.items() if k != 'path'},
                         {k:v for k,v in original_config.items() if k != 'path'})
        with self.assertRaises(old.AuthorityRefusal):
            old._parse(raw, digest)
        with self.assertRaises(old.AuthorityRefusal):
            old.ExecutionAuthority(plan)

    def test_unknown_loader_environment_path_and_artifact_refuse(self):
        for mutate in [lambda c: c['configuration_loader'].update(extra=True),
                       lambda c: c['configuration_loader'].update(config_schema_sha256='0'*64),
                       lambda c: c['server_env'].update(CODEX_HOME='/foreign'),
                       lambda c: c['command_env'].update(CODEX_HOME='/foreign'),
                       lambda c: c['server_env'].update(DYLD_INSERT_LIBRARIES='/evil'),
                       lambda c: c['inventory'][2].update(path='/foreign/config.toml'),
                       lambda c: c['inventory'][1].update(path='/store/compiler'),
                       lambda c: c['inventory'][2].update(size=True),
                       lambda c: c['inventory'][2].update(size=262145),
                       lambda c: c['inventory'].append(copy.deepcopy(c['inventory'][2])),
                       lambda c: c.update(enforcement_proof={'qualified': True}),
                       lambda c: c.update(extra=True),
                       lambda c: c['sandbox_policy'].update(networkAccess=0),
                       lambda c: c['inventory'][0].update(sha256='0'*64, path='/store/'+'0'*64)]:
            c = contract()
            mutate(c)
            with self.subTest(contract=c), self.assertRaises(old.AuthorityRefusal):
                v2.parse_user_home(*wire(c))

    def test_duplicate_and_wrong_digest_refuse(self):
        raw, digest = wire(contract())
        with self.assertRaises(old.AuthorityRefusal):
            v2.parse_user_home(raw, '0'*64)
        duplicate = raw.replace(b'"approval_policy":"never"',
                                b'"approval_policy":"never","approval_policy":"never"')
        with self.assertRaises(old.AuthorityRefusal):
            v2.parse_user_home(duplicate, hashlib.sha256(duplicate).hexdigest())

    def test_preflight_reads_actual_named_config_only(self):
        value = contract()
        # Synthetic runtime bytes cannot have the real pinned runtime SHA, so
        # this test records all intended reads and refuses at runtime drift.
        raw, digest = wire(value)
        plan = v2.parse_user_home(raw, digest)
        seen = []
        def read(path, limit, protected):
            seen.append((str(path), protected))
            if str(path) == '/protected/contract':
                return raw, SimpleNamespace(st_mode=0o100755)
            entry = next(e for e in plan.inventory if e.path == path)
            return b'x'*entry.size, SimpleNamespace(st_mode=0o100755)
        # Reorder actual inventory so the named configuration is read first.
        # Its digest/bytes are genuine synthetic config, not a mocked grant.
        config = next(e for e in plan.inventory if e.role == 'configuration')
        from dataclasses import replace
        plan = replace(plan, inventory=(config,) + tuple(e for e in plan.inventory if e != config))
        def named_read(path, limit, protected):
            if path == config.path:
                seen.append((str(path), protected))
                return b'model="fixed"\n', SimpleNamespace(st_mode=0o100600)
            return read(path, limit, protected)
        with patch.object(v2, 'parse_user_home', return_value=plan), \
             patch.object(old, '_read', side_effect=named_read), \
             patch.object(old, '_directory'):
            with self.assertRaisesRegex(old.AuthorityRefusal, 'inventory byte drift'):
                v2.preflight_user_home(Path('/protected/contract'), digest)
        self.assertIn(('/protected/home/.codex/config.toml', True), seen)
        self.assertFalse(any(p.endswith(config.sha256) for p, _ in seen))


if __name__ == '__main__':
    unittest.main()
