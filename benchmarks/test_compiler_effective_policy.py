"""Pure static controls: no process, RPC, filesystem acquisition or grant."""
import copy
from dataclasses import replace
import hashlib
import json
from pathlib import Path
import unittest
from types import MappingProxyType

from compiler_execution_authority import (AuthorityRefusal, ExecutionAuthority,
    ProvisioningPlan, InventoryEntry)
from compiler_effective_policy import (POLICY_SCHEMA, DESCRIPTION_SCHEMA, parse_policy,
    intersect_policies, is_subset, resolve_owned_config, validate_profile_description)


def policy(**updates):
    value = dict(schema=POLICY_SCHEMA, read_roots=[], read_files=[], write_roots=[],
                 write_files=[], deny_roots=[], exec_files=[], metadata_files=[], network=False,
                 unix_sockets=False, spawn=False)
    value.update(updates)
    return value


def fixture(root='/trial/sem'):
    root = Path(root)
    held = ('model="fixed"\nmodel_reasoning_effort="medium"\napproval_policy="never"\n'
            'sandbox_mode="workspace-write"\nweb_search="disabled"\n'
            '[sandbox_workspace_write]\nwritable_roots=["' + str(root) + '"]\n'
            'network_access=false\nexclude_slash_tmp=true\nexclude_tmpdir_env_var=true\n'
            '[apps._default]\nenabled=false\nopen_world_enabled=false\ndestructive_enabled=false\n').encode()
    digest = hashlib.sha256(held).hexdigest()
    configpath = Path('/protected/home/.codex/config.toml')
    inventory = tuple(InventoryEntry(Path('/store') / (c * 64), c * 64, 10, role)
                      for c, role in [('a', 'runtime'), ('b', 'compiler'), ('c', 'dependency')])
    inventory += (InventoryEntry(configpath, digest, len(held), 'configuration'),)
    env = MappingProxyType(dict(PATH='/protected/bin', HOME='/protected/home',
                                CODEX_HOME='/protected/home/.codex',
                                TMPDIR='/protected/tmp', LANG='C', LC_ALL='C', TZ='UTC'))
    from compiler_configuration_provisioning import RUNTIME_SHA256, CONFIG_SCHEMA_SHA256
    inventory = (replace(inventory[0], path=Path('/store') / RUNTIME_SHA256,
                         sha256=RUNTIME_SHA256),) + inventory[1:]
    inventory += (InventoryEntry(Path('/store') / CONFIG_SCHEMA_SHA256,
                                CONFIG_SCHEMA_SHA256, 10, 'schema'),)
    plan = ProvisioningPlan(inventory[0].path, RUNTIME_SHA256, (), root, env,
                           MappingProxyType({}), root, env, inventory[1].path, 'b'*64,
                           'd'*64, (root,), Path('/evidence'), inventory)
    import tomllib
    config = tomllib.loads(held.decode())
    name = dict(type='user', file=str(configpath))
    response = dict(config=config, layers=[dict(config=copy.deepcopy(config), name=name,
                                                version='v1')],
                    origins={k: dict(name=name, version='v1') for k in config})
    return plan, held, configpath, response


def profile_bytes(plan):
    from compiler_effective_policy import _candidate_profile
    d = description(plan)
    return _candidate_profile(plan.compiler_binary, tuple(map(Path, d['read_files'])),
                              Path(d['publication_directory']))


def description(plan):
    root = str(plan.workspace_root)
    inputs = [root+'/main.spx', root+'/semaprax.toml']
    reads = sorted([str(r.path) for r in plan.inventory] + inputs)
    metadata = sorted({str(a) for p in reads + [root+'/publication']
                       for a in Path(p).parents if a != Path('/')} )
    from compiler_effective_policy import _candidate_profile
    raw_profile = _candidate_profile(plan.compiler_binary, tuple(map(Path, reads)),
                                     Path(root+'/publication'))
    return dict(schema=DESCRIPTION_SCHEMA, cwd=root, compiler=str(plan.compiler_binary),
                argv=['json-codec', '.', '--source', 'main.spx', '--type', 'Record',
                      '--output', 'publication/result.json', '--retained-receipt',
                      '--receipt-stdout-seal', 'e'*64],
                publication_directory=root+'/publication', input_files=inputs,
                read_files=reads, metadata_files=metadata, environment=dict(plan.command_env),
                sandbox_launcher=str(plan.inventory[2].path), profile_sha256=hashlib.sha256(raw_profile).hexdigest(),
                agent_policy=policy(read_roots=[root, '/store'], read_files=reads, write_roots=[root],
                                    exec_files=[str(plan.compiler_binary)], metadata_files=metadata),
                executor_policy=policy(read_files=reads, read_roots=[root+'/publication'],
                                       write_roots=[root+'/publication'], metadata_files=metadata,
                                       exec_files=[str(plan.compiler_binary)]))


class FinitePolicyTests(unittest.TestCase):
    def test_intersection_and_exclusions(self):
        a = parse_policy(policy(read_roots=['/a'], deny_roots=['/a/private'], network=True))
        b = parse_policy(policy(read_roots=['/a/b'], read_files=['/a/one']))
        result = intersect_policies(a, b)
        self.assertEqual(result.read_roots, (Path('/a/b'),))
        self.assertEqual(result.read_files, (Path('/a/one'),))
        self.assertFalse(result.network)
        self.assertTrue(is_subset(result, a))
        self.assertTrue(is_subset(result, b))

    def test_parent_hole_must_survive(self):
        parent = parse_policy(policy(write_roots=['/a'], deny_roots=['/a/hole']))
        self.assertFalse(is_subset(parse_policy(policy(write_roots=['/a'])), parent))
        self.assertFalse(is_subset(parse_policy(policy(write_roots=['/a'],
                                                        deny_roots=['/a/hole/sub'])), parent))
        self.assertTrue(is_subset(parse_policy(policy(write_roots=['/a'],
                                                       deny_roots=['/a/hole'])), parent))
        self.assertTrue(is_subset(parse_policy(policy(write_roots=['/a/other'])), parent))

    def test_files_cannot_cover_tree_or_denied_exec(self):
        self.assertFalse(is_subset(parse_policy(policy(read_roots=['/a'])),
                                   parse_policy(policy(read_files=['/a']))))
        self.assertFalse(is_subset(parse_policy(policy(exec_files=['/a/x'])),
                                   parse_policy(policy(exec_files=['/a/x'], deny_roots=['/a']))))

    def test_unknown_and_nonliteral_refuse(self):
        for update in [dict(extra=False), dict(network=1), dict(read_roots=['/a/../b']),
                       dict(read_files=['/a', '/a']), dict(read_roots=['/']),
                       dict(write_roots=['relative']), dict(exec_files=['/a\0b'])]:
            with self.subTest(update=update), self.assertRaises(AuthorityRefusal):
                parse_policy(policy(**update))

    def test_algebra_membership_corpus(self):
        from compiler_effective_policy import _allows
        values = [parse_policy(policy(read_roots=['/a'])),
                  parse_policy(policy(read_roots=['/a/b'], deny_roots=['/a/b/x'])),
                  parse_policy(policy(read_files=['/a/b', '/z']))]
        for a in values:
            for b in values:
                both = intersect_policies(a, b)
                for raw in ['/a', '/a/b', '/a/b/x', '/a/b/x/y', '/a/c', '/z']:
                    p = Path(raw)
                    allowed = lambda q: _allows(p, q.read_roots, q.read_files, q)
                    self.assertEqual(allowed(both), allowed(a) and allowed(b))


class ResolutionTests(unittest.TestCase):
    def test_single_layer_and_description(self):
        plan, held, path, response = fixture()
        result = resolve_owned_config(plan, response, held, path, model='fixed', effort='medium')
        desc = validate_profile_description(plan, result, description(plan), profile_bytes=profile_bytes(plan))
        self.assertEqual(desc.contract_sha256, plan.contract_sha256)
        with self.assertRaises(AuthorityRefusal):
            ExecutionAuthority(desc)

    def test_complete_layer_origin_and_config_refusals(self):
        plan, held, path, response = fixture()
        variants = []
        for mutate in [lambda r: r.update(layers=[]),
                       lambda r: r['layers'].append(copy.deepcopy(r['layers'][0])),
                       lambda r: r['layers'][0]['name'].update(type='system'),
                       lambda r: r['layers'][0]['name'].update(profile='other'),
                       lambda r: r['layers'][0].update(disabledReason='off'),
                       lambda r: r['origins'].pop('model'),
                       lambda r: r['origins']['model'].update(version='other'),
                       lambda r: r['config'].update(env={}),
                       lambda r: r.update(unknown=True)]:
            r = copy.deepcopy(response)
            mutate(r)
            variants.append(r)
        for r in variants:
            with self.subTest(response=r), self.assertRaises(AuthorityRefusal):
                resolve_owned_config(plan, r, held, path, model='fixed', effort='medium')
        for raw in [held+b'\n', b'', b'bad']:
            with self.assertRaises(AuthorityRefusal):
                resolve_owned_config(plan, response, raw, path, model='fixed', effort='medium')

    def test_semantic_bool_number_collision_refuses(self):
        plan, held, path, response = fixture()
        response['config']['sandbox_workspace_write']['network_access'] = 0
        with self.assertRaises(AuthorityRefusal):
            resolve_owned_config(plan, response, held, path, model='fixed', effort='medium')

    def test_profile_widening_and_incomplete_capture_refuse(self):
        plan, held, path, response = fixture()
        resolved = resolve_owned_config(plan, response, held, path, model='fixed', effort='medium')
        for mutate in [lambda d: d['environment'].update(DYLD_INSERT_LIBRARIES='/evil'),
                       lambda d: d['executor_policy'].update(spawn=True),
                       lambda d: d['executor_policy'].update(write_roots=['/trial/sem']),
                       lambda d: d['agent_policy'].update(deny_roots=['/trial/sem/publication']),
                       lambda d: d['read_files'].pop(),
                       lambda d: d.update(input_files=['/trial/sem/main.spx']),
                       lambda d: d.update(publication_directory='/trial/sem'),
                       lambda d: d.update(sandbox_launcher='/foreign/launcher'),
                       lambda d: d['argv'].__setitem__(-1, '--bad'),
                       lambda d: d.update(profile_sha256='f'*64),
                       lambda d: d['executor_policy'].update(deny_roots=['/store']),
                       lambda d: d.update(unknown='field')]:
            d = description(plan)
            mutate(d)
            with self.subTest(description=d), self.assertRaises(AuthorityRefusal):
                validate_profile_description(plan, resolved, d, profile_bytes=profile_bytes(plan))

    def test_forged_resolved_object_and_profile_refuse(self):
        plan, held, path, response = fixture()
        resolved = resolve_owned_config(plan, response, held, path, model='fixed', effort='medium')
        for forged in [replace(resolved, normalized_config_sha256='0'*64),
                       replace(resolved, canonical_config='{}'),
                       replace(resolved, canonical_config=resolved.canonical_config.replace(
                           '"approval_policy":"never"',
                           '"approval_policy":"never","approval_policy":"never"')),
                       replace(resolved, configuration_sha256='0'*64),
                       replace(resolved, workspace_root=Path('/foreign'))]:
            with self.assertRaises(AuthorityRefusal):
                validate_profile_description(plan, forged, description(plan),
                                             profile_bytes=profile_bytes(plan))
        evil = b'(version 1)\n(allow default)\n'
        d = description(plan)
        d['profile_sha256'] = hashlib.sha256(evil).hexdigest()
        with self.assertRaises(AuthorityRefusal):
            validate_profile_description(plan, resolved, d, profile_bytes=evil)

    def test_matching_nested_denials_cannot_describe_undenied_profile(self):
        plan, held, path, response = fixture()
        resolved = resolve_owned_config(plan, response, held, path, model='fixed', effort='medium')
        d = description(plan)
        hole = str(plan.workspace_root / 'publication' / 'secret')
        d['agent_policy']['deny_roots'] = [hole]
        d['executor_policy']['deny_roots'] = [hole]
        with self.assertRaises(AuthorityRefusal):
            validate_profile_description(plan, resolved, d, profile_bytes=profile_bytes(plan))

    def test_profile_and_nonce_identity_not_erased(self):
        plan, held, path, response = fixture()
        resolved = resolve_owned_config(plan, response, held, path, model='fixed', effort='medium')
        one = description(plan)
        first = validate_profile_description(plan, resolved, one, profile_bytes=profile_bytes(plan))
        one['argv'][-1] = '9'*64
        second = validate_profile_description(plan, resolved, one, profile_bytes=profile_bytes(plan))
        self.assertNotEqual(first.common_fingerprint_sha256, second.common_fingerprint_sha256)

    def test_role_normalized_config_is_common_not_raw_contract(self):
        results = []
        for root in ['/trial/sem', '/trial/ts']:
            plan, held, path, response = fixture(root)
            results.append(resolve_owned_config(plan, response, held, path,
                                                 model='fixed', effort='medium'))
        self.assertEqual(results[0].normalized_config_sha256, results[1].normalized_config_sha256)
        self.assertNotEqual(results[0].configuration_sha256, results[1].configuration_sha256)


if __name__ == '__main__':
    unittest.main()
