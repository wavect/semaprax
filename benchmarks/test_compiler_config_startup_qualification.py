"""Pure startup evaluator controls; no server, RPC, grant or OS sandbox probe."""
import copy
from dataclasses import replace
import hashlib
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from compiler_execution_authority import AuthorityRefusal, ExecutionAuthority
import compiler_config_startup_qualification as startup
from test_compiler_effective_policy import fixture


class StartupTests(unittest.TestCase):
    def test_exact_params_and_complete_result_evaluate(self):
        plan, held, path, response = fixture()
        request = startup.startup_request(plan)
        self.assertEqual(request, {'cwd': str(plan.workspace_root), 'includeLayers': True})
        result = startup.evaluate_startup(plan, request=request, response=response,
            held_config_bytes=held, model='fixed', effort='medium')
        self.assertEqual(result.configuration_sha256, hashlib.sha256(held).hexdigest())
        reordered = dict(reversed(list(response.items())))
        other = startup.evaluate_startup(plan, request=dict(reversed(list(request.items()))),
            response=reordered, held_config_bytes=held, model='fixed', effort='medium')
        self.assertEqual(result.request_params_sha256, other.request_params_sha256)
        self.assertEqual(result.response_result_sha256, other.response_result_sha256)
        self.assertEqual(result.scope, 'CONFIG_BYTES_VALIDATED_TRANSPORT_AND_ENFORCEMENT_UNQUALIFIED')
        with self.assertRaises(AuthorityRefusal):
            ExecutionAuthority(result)

    def test_request_widening_and_missing_layers_refuse(self):
        plan, held, path, response = fixture()
        for request in [{'cwd': '/foreign', 'includeLayers': True},
                        {'cwd': str(plan.workspace_root), 'includeLayers': 1},
                        {'cwd': str(plan.workspace_root), 'includeLayers': False},
                        {'cwd': str(plan.workspace_root)},
                        {'cwd': str(plan.workspace_root), 'includeLayers': True, 'qualified': True}]:
            with self.subTest(request=request), self.assertRaises(ValueError):
                startup.evaluate_startup(plan, request=request, response=response,
                    held_config_bytes=held, model='fixed', effort='medium')
        altered = dict(response)
        del altered['layers']
        with self.assertRaises(ValueError):
            startup.evaluate_startup(plan, request=startup.startup_request(plan), response=altered,
                held_config_bytes=held, model='fixed', effort='medium')

    def test_actual_schema_and_finite_config_both_apply(self):
        plan, held, path, response = fixture()
        for change in [lambda r: r.update(extra='foreign'),
                       lambda r: r['layers'][0].update(version=True),
                       lambda r: r['layers'][0]['name'].update(type='sessionFlags'),
                       lambda r: r['config'].update(approval_policy='on-request'),
                       lambda r: r['origins']['model'].update(version='foreign')]:
            changed = copy.deepcopy(response)
            change(changed)
            with self.subTest(response=changed), self.assertRaises(ValueError):
                startup.evaluate_startup(plan, request=startup.startup_request(plan), response=changed,
                    held_config_bytes=held, model='fixed', effort='medium')

    def test_pinned_schema_drift_refuses_before_validation(self):
        plan, held, path, response = fixture()
        with patch.object(startup.original, '_read', return_value=(b'{}', None)):
            with self.assertRaisesRegex(AuthorityRefusal, 'schema source drift'):
                startup.startup_request(plan)

    def test_actual_named_protected_acquisition_and_identity_control(self):
        plan, held, path, response = fixture()
        info = SimpleNamespace(st_dev=1, st_ino=2, st_size=len(held), st_mtime_ns=3,
                               st_ctime_ns=4, st_mode=0o100600, st_uid=0, st_gid=0)
        with patch.object(startup.original, '_read', return_value=(held, info)) as read:
            before = startup.acquire_configuration(plan)
            read.assert_called_once_with(path, len(held), protected=True)
        startup.require_same_configuration(before, before)
        changes = [replace(before, contract_sha256='0'*64),
                   replace(before, path=path.parent/'foreign'),
                   replace(before, data=held+b'\n'),
                   replace(before, sha256='0'*64),
                   replace(before, identity=(1, 9, len(held), 3, 4, 0o100600, 0, 0)),
                   replace(before, identity=(1, 2, len(held), 3, 4, 0o100600, 0, True))]
        for after in changes:
            with self.subTest(after=after), self.assertRaises(AuthorityRefusal):
                startup.require_same_configuration(before, after)
        with patch.object(startup.original, '_read', return_value=(held+b'\n', info)):
            with self.assertRaisesRegex(AuthorityRefusal, 'configuration drift'):
                startup.acquire_configuration(plan)


if __name__ == '__main__':
    unittest.main()
