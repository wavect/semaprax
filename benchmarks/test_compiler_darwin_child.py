"""Pure candidate/descriptor controls. Never execute a process or mint a grant."""
from pathlib import Path
from types import MappingProxyType, SimpleNamespace
import errno
import hashlib
import os
import operator
import stat
import unittest
from unittest import mock

import compiler_darwin_child as child
from compiler_execution_authority import (AuthorityRefusal, ProvisioningPlan,
                                           InventoryEntry, admit)


class DarwinChildChecks(unittest.TestCase):
    def setUp(self):
        root = Path('/Volumes/Semaprax/trial')
        protected = Path('/Volumes/Semaprax/protected')
        blobs = [b'compiler', b'runtime', b'launcher']
        compiler, runtime, launcher = [protected / hashlib.sha256(b).hexdigest() for b in blobs]
        self.blobs = dict(zip((compiler, runtime, launcher), blobs))
        env = MappingProxyType({'PATH': str(protected), 'HOME': str(protected / 'home'),
              'TMPDIR': str(protected / 'tmp'), 'LANG': 'C', 'LC_ALL': 'C', 'TZ': 'UTC'})
        entries = tuple(InventoryEntry(p, p.name, len(self.blobs[p]), role) for p, role in
                        [(compiler, 'compiler'), (runtime, 'runtime'), (launcher, 'dependency')])
        self.plan = ProvisioningPlan(runtime, runtime.name,
            (str(runtime), 'app-server', '--listen', 'stdio://'), root, env,
            MappingProxyType({'type': 'workspaceWrite', 'writableRoots': (str(root),),
                 'networkAccess': False, 'excludeSlashTmp': True, 'excludeTmpdirEnvVar': True}),
            root, env, compiler, compiler.name, 'd' * 64, (root,), protected / 'evidence', entries)
        self.launcher = launcher
        self.inputs = (root / 'semaprax.toml', root / 'main.spx')
        self.publication = root / 'runner-output'
        self.argv = ('json-codec', '.', '--source', 'main.spx', '--type', 'app.Row',
                     '--output', 'runner-output/result.json', '--retained-receipt',
                     '--receipt-stdout-seal', 'e' * 64)

    def prepare(self, **changes):
        params = dict(plan=self.plan, argv=self.argv, input_files=self.inputs,
                      publication_directory=self.publication, sandbox_launcher=self.launcher)
        params.update(changes)
        return child.prepare_transition(**params)

    def test_exact_profile_and_receiving_shape_are_deterministic_and_not_grants(self):
        transition = self.prepare()
        self.assertEqual(transition, self.prepare())
        self.assertEqual(transition.argv, (str(self.plan.compiler_binary),) + self.argv)
        self.assertEqual(dict(transition.environment), dict(self.plan.command_env))
        self.assertEqual(hashlib.sha256(transition.profile.encode()).hexdigest(), transition.profile_sha256)
        self.assertIn('(deny default)', transition.profile)
        self.assertIn('; ' + child.PROFILE_VERSION, transition.profile)
        self.assertEqual(transition.profile.count('(allow process-exec '), 1)
        for forbidden in ['(allow process-fork', '(allow network', '(allow mach-lookup', '(import ']:
            self.assertNotIn(forbidden, transition.profile)
        self.assertIn('(subpath "' + str(self.publication) + '")', transition.profile)
        self.assertRaises(TypeError, operator.setitem, transition.environment, 'EXTRA', 'injected')
        with mock.patch('compiler_execution_authority.preflight', return_value=self.plan):
            self.assertRaises(AuthorityRefusal, admit, Path('/ignored'), '0' * 64)

    def test_closed_argv_nonce_and_fixed_project_refuse_before_any_os_transition(self):
        changed = [(), ('sh', '-c', 'echo forged'), self.argv[:-2],
                   self.argv[:-1] + ('foreign',), self.argv + ('--profile', 'other'),
                   self.argv[:1] + ('..',) + self.argv[2:],
                   self.argv[:1] + ('/foreign',) + self.argv[2:]]
        with mock.patch.object(child.os, 'execve') as execute:
            for argv in changed:
                self.assertRaises(AuthorityRefusal, self.prepare, argv=argv)
            execute.assert_not_called()

    def test_input_publication_and_pinned_launcher_boundaries_refuse(self):
        for changes in [dict(input_files=()), dict(input_files=(self.inputs[0],)),
            dict(input_files=self.inputs + (Path('/foreign/main.spx'),)),
            dict(input_files=self.inputs + self.inputs),
            dict(publication_directory=self.plan.workspace_root),
            dict(publication_directory=self.plan.workspace_root / 'nested' / 'output'),
            dict(publication_directory=Path('/foreign')),
            dict(sandbox_launcher=Path('/usr/bin/sandbox-exec'))]:
            self.assertRaises(AuthorityRefusal, self.prepare, **changes)
        for output in ['../escape', '/foreign/result', 'runner-output/sub/result', 'other/result']:
            args = list(self.argv); args[7] = output
            self.assertRaises(AuthorityRefusal, self.prepare, argv=tuple(args))

    def test_profile_path_injection_and_duplicate_grants_are_rejected(self):
        compiler = self.plan.compiler_binary
        for path in ['/Volumes/a"(allow default)', '/Volumes/a\\b', '/Volumes/é', '/Volumes/a\n']:
            self.assertRaises(AuthorityRefusal, child.profile_source, compiler,
                              (compiler, Path(path)), self.publication)
        self.assertRaises(AuthorityRefusal, child.profile_source, compiler,
                          (compiler, compiler), self.publication)

    def test_descriptor_closure_has_no_guessed_rlimit_or_best_effort_residue(self):
        pipe = SimpleNamespace(st_mode=stat.S_IFIFO)
        device = SimpleNamespace(st_mode=stat.S_IFCHR)
        def info(fd):
            if fd == 0: return device
            if fd in (1, 2): return pipe
            raise OSError(errno.EBADF, 'transient listdir fd')
        with mock.patch.object(child.os, 'listdir', side_effect=[['0','1','2','7'], ['0','1','2','8']]), \
             mock.patch.object(child.os, 'close') as close, \
             mock.patch.object(child.os, 'fstat', side_effect=info):
            child._close_extra_descriptors()
            close.assert_called_once_with(7)
        for listing in [['foreign'], ['0'] * (child.MAX_FDS + 1)]:
            with mock.patch.object(child.os, 'listdir', return_value=listing), \
                 mock.patch.object(child.os, 'close') as close:
                self.assertRaises(AuthorityRefusal, child._close_extra_descriptors)
                close.assert_not_called()
        with mock.patch.object(child.os, 'listdir', side_effect=[['0','1','2'],['0','1','2','7']]), \
             mock.patch.object(child.os, 'fstat', return_value=pipe):
            self.assertRaises(AuthorityRefusal, child._close_extra_descriptors)

    def test_irreversible_transition_uses_exec_and_never_a_shell_or_local_replay(self):
        transition = self.prepare()
        events = []
        class InterceptExec(BaseException):
            pass
        def execute(path, args, env):
            events.append('exec')
            self.assertEqual(path, str(self.launcher))
            self.assertEqual(args, (path, '-p', transition.profile) + transition.argv)
            self.assertEqual(env, dict(self.plan.command_env))
            raise InterceptExec()
        def read(path, size, protected):
            self.assertTrue(protected)
            data = self.blobs[path]
            self.assertEqual(size, len(data))
            events.append('protected-read')
            return data, SimpleNamespace(st_mode=stat.S_IFREG | 0o755)
        with mock.patch.object(child.os, 'uname', return_value=SimpleNamespace(sysname='Darwin')), \
             mock.patch.object(child, '_read', side_effect=read), \
             mock.patch.object(child, '_held_directory', return_value=9), \
             mock.patch.object(child.os, 'mkdir', side_effect=lambda *a,**k: events.append('exclusive-publication')), \
             mock.patch.object(child.os, 'fchdir', side_effect=lambda _: events.append('held-cwd')), \
             mock.patch.object(child.os, 'close'), mock.patch.object(child.os, 'umask'), \
             mock.patch.object(child.os, 'open', return_value=10), \
             mock.patch.object(child.os, 'fstat', return_value=SimpleNamespace(st_mode=stat.S_IFCHR)), \
             mock.patch.object(child.os, 'dup2', side_effect=lambda *a: events.append('closed-stdin')), \
             mock.patch.object(child, '_close_extra_descriptors', side_effect=lambda: events.append('close-fds')), \
             mock.patch.object(child.os, 'execve', side_effect=execute):
            self.assertRaises(InterceptExec, child._exec_transition, transition)
        self.assertEqual(events, ['protected-read'] * 3 + ['exclusive-publication', 'held-cwd',
                          'closed-stdin', 'close-fds', 'exec'])

    def test_dependency_drift_and_publication_collision_refuse_without_exec_or_adoption(self):
        transition = self.prepare()
        info = SimpleNamespace(st_mode=stat.S_IFREG | 0o755)
        with mock.patch.object(child.os, 'uname', return_value=SimpleNamespace(sysname='Darwin')), \
             mock.patch.object(child, '_read', return_value=(b'changed', info)), \
             mock.patch.object(child, '_held_directory') as held, \
             mock.patch.object(child.os, 'execve') as execute:
            self.assertRaises(AuthorityRefusal, child._exec_transition, transition)
            held.assert_not_called()
            execute.assert_not_called()
        def read(path, size, protected):
            return self.blobs[path], info
        with mock.patch.object(child.os, 'uname', return_value=SimpleNamespace(sysname='Darwin')), \
             mock.patch.object(child, '_read', side_effect=read), \
             mock.patch.object(child, '_held_directory', return_value=9), \
             mock.patch.object(child.os, 'mkdir', side_effect=FileExistsError('foreign directory')), \
             mock.patch.object(child.os, 'fchdir') as chdir, \
             mock.patch.object(child.os, 'close') as close, \
             mock.patch.object(child.os, 'execve') as execute:
            self.assertRaises(FileExistsError, child._exec_transition, transition)
            close.assert_called_once_with(9)
            chdir.assert_not_called()
            execute.assert_not_called()

    def test_direct_process_observation_cannot_claim_whole_child_settlement(self):
        response = {'exitCode': 7, 'stdout': '', 'stderr': ''}
        observed = child.observe_direct_exit('runner-owned', response, b'partial', b'refused')
        self.assertEqual((observed.exit_code, observed.stdout, observed.stderr), (7,b'partial',b'refused'))
        self.assertEqual(observed.whole_child_settlement, 'UNQUALIFIED_NO_FORK_ENFORCEMENT')
        self.assertRaises(TypeError, child.DirectChildExit, 'runner-owned', 0, b'', b'',
                          whole_child_settlement='QUALIFIED')
        for changed in [dict(response, exitCode=True), dict(response, exitCode=2**31),
                        dict(response, stdout='foreign'), dict(response, settled=True)]:
            self.assertRaises(AuthorityRefusal, child.observe_direct_exit, 'runner-owned', changed, b'', b'')
        self.assertRaises(AuthorityRefusal, child.observe_direct_exit, 'runner-owned', response, b'x'*513, b'')

    def test_v2_named_loader_composes_with_exact_child_env_and_refuses_drift(self):
        # Shared pure fixture has actual versioned loader shape, never real files,
        # runtime source protection, a live grant or authority admission.
        from test_compiler_effective_policy import fixture, description
        from compiler_effective_policy import resolve_owned_config, validate_profile_description
        from dataclasses import replace
        plan, held, path, response = fixture()
        d = description(plan)
        def prepare(value):
            return child.prepare_transition(value, tuple(d['argv']),
                tuple(Path(p) for p in d['input_files']), Path(d['publication_directory']),
                Path(d['sandbox_launcher']))
        transition = prepare(plan)
        self.assertEqual(dict(transition.environment)['CODEX_HOME'],
                         str(Path(plan.command_env['HOME']) / '.codex'))
        resolved = resolve_owned_config(plan, response, held, path, model='fixed', effort='medium')
        validated = validate_profile_description(plan, resolved, d,
                                                 profile_bytes=transition.profile.encode())
        self.assertEqual(validated.contract_sha256, plan.contract_sha256)
        for env in [dict(plan.command_env, CODEX_HOME='/foreign'),
                    dict(plan.command_env, DYLD_INSERT_LIBRARIES='/foreign')]:
            altered = replace(plan, server_env=MappingProxyType(env), command_env=MappingProxyType(env))
            self.assertRaises(AuthorityRefusal, prepare, altered)
        self.assertRaises(AuthorityRefusal, prepare, replace(plan, runtime_sha256='0' * 64))
        with mock.patch('compiler_configuration_provisioning.preflight_user_home', return_value=plan) as acquire, \
             mock.patch.object(child, '_exec_transition') as execute:
            child.execute_child(Path('/protected/contract'), plan.contract_sha256,
                tuple(d['argv']), tuple(Path(p) for p in d['input_files']),
                Path(d['publication_directory']), Path(d['sandbox_launcher']))
            acquire.assert_called_once_with(Path('/protected/contract'), plan.contract_sha256)
            self.assertEqual(execute.call_args.args[0], transition)

    def test_closed_entry_does_not_accept_caller_authority_or_extra_fields(self):
        with mock.patch.object(child, 'execute_child') as execute:
            for args in [[], ['--qualified', 'true'], ['--contract', '/a', '--contract-sha256', '0'*64,
                            '--request-json', '{"qualified":true}']]:
                self.assertRaises((AuthorityRefusal, ValueError), child.main, args)
            execute.assert_not_called()


if __name__ == '__main__':
    unittest.main()
