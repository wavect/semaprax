"""Pure qualification controls: no native build, subprocess, RPC or kernel probe."""
import base64
import copy
import ctypes
import errno
from dataclasses import replace
import hashlib
import io
import json
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace, MappingProxyType
import unittest
from unittest import mock

import compiler_qualification_runtime as q
from compiler_execution_authority import AuthorityRefusal
from compiler_runtime_protocol import Protocol, encode
from test_compiler_effective_policy import fixture


class QualificationChecks(unittest.TestCase):
    def setUp(self):
        self.plan,self.held,self.config,self.response = fixture('/Volumes/Semaprax/trial')
        self.plan = replace(self.plan, sandbox_policy=MappingProxyType({
            'type':'workspaceWrite','writableRoots':(str(self.plan.workspace_root),),
            'networkAccess':False,'excludeSlashTmp':True,'excludeTmpdirEnvVar':True}))
        self.store = SimpleNamespace(check_current=lambda:None)

    def runtime(self):
        runtime = object.__new__(q._Runtime)
        runtime.plan,self.command = self.plan,('/protected/probe','environment')
        runtime.store = self.store
        runtime.protocol = Protocol()
        runtime.sequence = 2
        runtime.process_ids = set()
        runtime.approved_commands = frozenset([self.command])
        runtime.send = mock.Mock()
        return runtime

    def test_outer_barrier_covers_declared_files_and_ancestors_without_writable_root_grant(self):
        text = q.outer_profile_source(self.plan,(Path('/store'),))
        self.assertIn('(deny file-write* (subpath "/store"))',text)
        self.assertIn('(deny file-write* (literal "'+str(self.config)+'"))',text)
        self.assertIn('(deny file-write* (literal "/"))',text)
        self.assertNotIn('(deny file-write* (subpath "'+str(self.plan.workspace_root),text)
        for roots in [(),(Path('/'),),(self.plan.workspace_root,),
                      (Path('/store'),Path('/store')),(Path('/uncovered'),)]:
            self.assertRaises(AuthorityRefusal,q.outer_profile_source,self.plan,roots)

    def test_no_model_or_unknown_method_can_reach_transport(self):
        runtime = self.runtime()
        for method in ['turn/start','thread/start','process/spawn','thread/shellCommand']:
            self.assertRaises(AuthorityRefusal,runtime.exchange,method,{},1)
        runtime.send.assert_not_called()
        self.assertEqual(runtime.sequence,2)
        del runtime.send
        self.assertRaises(AuthorityRefusal,runtime.send,{'method':'turn/start'})

    def test_direct_send_cannot_bypass_scope_validation(self):
        runtime = self.runtime()
        del runtime.send
        for message in [
            {'id':3,'method':'command/exec','params':q._probe_params(self.plan,self.command)},
            {'id':3,'method':'config/read','params':{}},
            {'method':'initialized'},
        ]:
            with mock.patch.object(q.select,'select') as ready:
                self.assertRaises(AuthorityRefusal,runtime.send,message)
                ready.assert_not_called()
        self.assertEqual(runtime.sequence,2)

    def test_scope_refusals_precede_id_mutation_and_send(self):
        for change in [dict(command=['sh','-c','fake']),dict(cwd='/foreign'),
                       dict(env={'DYLD_INSERT_LIBRARIES':'fake'}),
                       dict(sandboxPolicy={'type':'dangerFullAccess'}),
                       dict(streamStdin=True),dict(disableTimeout=True),dict(timeoutMs=0),
                       dict(outputBytesCap=0),dict(processId=True)]:
            runtime = self.runtime()
            params = q._probe_params(self.plan,self.command)
            params.update(change)
            self.assertRaises((AuthorityRefusal,ValueError),runtime.exchange,'command/exec',params,1)
            self.assertEqual(runtime.sequence,2)
            runtime.send.assert_not_called()

    def test_response_ids_stream_caps_and_process_replays_are_closed(self):
        for message in [{'id':True,'result':{}},{'id':2,'result':{}},
                        {'id':3,'result':{},'error':{}},{'id':3,'method':'foreign','params':{}}]:
            runtime=self.runtime(); runtime.next=mock.Mock(return_value=message)
            self.assertRaises((AuthorityRefusal,ValueError),runtime.exchange,'command/exec',
                              q._probe_params(self.plan,self.command),1)
        runtime=self.runtime(); params=q._probe_params(self.plan,self.command)
        runtime.next=mock.Mock(side_effect=[{'method':'command/exec/outputDelta','params':{
            'processId':params['processId'],'stream':'stdout','capReached':False,
            'deltaBase64':base64.b64encode(b'actual').decode()}},
            {'id':3,'result':{'exitCode':7,'stdout':'','stderr':''}}])
        result=runtime.exchange('command/exec',params,1)
        self.assertEqual(result,{'result':{'exitCode':7,'stdout':'','stderr':''},
                                'streams':{'stdout':b'actual','stderr':b''}})
        self.assertRaises(AuthorityRefusal,runtime.exchange,'command/exec',params,1)
        runtime=self.runtime(); params=q._probe_params(self.plan,self.command)
        runtime.next=mock.Mock(return_value={'method':'command/exec/outputDelta','params':{
            'processId':params['processId'],'stream':'stdout','capReached':True,'deltaBase64':''}})
        self.assertRaises(AuthorityRefusal,runtime.exchange,'command/exec',params,1)

    def test_actual_error_object_is_preserved_not_guessed_into_exit_status(self):
        runtime=self.runtime(); error={'code':-123,'message':'selected','data':{'detail':'whole'}}
        runtime.next=mock.Mock(return_value={'id':3,'error':error})
        result=runtime.exchange('command/exec',q._probe_params(self.plan,self.command),1)
        self.assertIs(result['runtime_error'],error)
        self.assertNotIn('result',result)

    def test_empty_acl_uses_darwin_status_and_unknown_acl_refuses(self):
        acl=object.__new__(q._DarwinAcl)
        lib=SimpleNamespace(acl_get_fd_np=mock.Mock(return_value=123),
            acl_valid=mock.Mock(return_value=0),acl_free=mock.Mock(return_value=0))
        def empty(*args): ctypes.set_errno(errno.EINVAL); return -1
        lib.acl_get_entry=mock.Mock(side_effect=empty); acl.lib=lib
        acl.empty(7); lib.acl_free.assert_called_once_with(123)
        for result,error in [(0,0),(-1,errno.EIO)]:
            lib.acl_get_entry.side_effect=lambda *a,r=result,e=error:(ctypes.set_errno(e),r)[1]
            self.assertRaises(AuthorityRefusal,acl.empty,7)
        lib.acl_get_fd_np.return_value=None
        self.assertRaises(AuthorityRefusal,acl.empty,7)

    def test_private_creation_refuses_symlink_parent_and_existing_leaf(self):
        with tempfile.TemporaryDirectory(dir='/Volumes/Semaprax/runtime-tmp') as name:
            root = Path(name)
            q._make_private_directory(root/'fresh')
            self.assertEqual(stat.S_IMODE((root/'fresh').stat().st_mode),0o700)
            self.assertRaises(FileExistsError,q._make_private_directory,root/'fresh')
            (root/'link').symlink_to(root/'fresh',target_is_directory=True)
            self.assertRaises(OSError,q._make_private_directory,root/'link'/'blocked')
            self.assertFalse((root/'fresh'/'blocked').exists())

    def test_candidate_configuration_has_no_root_protection_or_lease_claim(self):
        info=SimpleNamespace(st_dev=1,st_ino=2,st_mode=stat.S_IFREG|0o600,
            st_uid=501,st_gid=20,st_size=len(self.held),st_mtime_ns=3,st_ctime_ns=4)
        with mock.patch.object(q,'_read',return_value=(self.held,info)) as read:
            observed=q._candidate_configuration(self.plan)
            read.assert_called_once_with(self.config,len(self.held))
            self.assertEqual(observed.data,self.held)
            self.assertFalse(hasattr(observed,'check_current'))
        with mock.patch.object(q,'_read',return_value=(self.held+b' ',info)):
            self.assertRaises(AuthorityRefusal,q._candidate_configuration,self.plan)

    def test_unknown_store_mode_and_changed_fixed_source_launch_nothing(self):
        with mock.patch.object(q.subprocess,'Popen') as launch:
            self.assertRaises(AuthorityRefusal,q._collect_plan,Path('/ignored'),'0'*64,'claimed-qualified')
            dependency=next(e.path for e in self.plan.inventory if e.role=='dependency')
            with mock.patch.object(q,'_read',return_value=(b'changed probe source',None)):
                self.assertRaises(AuthorityRefusal,q._build_probe,self.plan,self.store,
                                  dependency,Path('/Volumes/Semaprax/evidence'))
            launch.assert_not_called()

    def test_probe_controls_require_real_permission_denial_not_missing_target(self):
        document={'schema':'semaprax.darwin-probe.v1','case':'read','pid':123,'uid':501,
                  'result':-1,'errno':errno.EACCES}
        with mock.patch.object(q.os,'geteuid',return_value=501):
            outcome={'result':{'exitCode':0,'stdout':'','stderr':''},
                     'streams':{'stdout':encode(document),'stderr':b''}}
            self.assertEqual(q._check_probe('read',outcome,{},denied=True),document)
            for error in [errno.ENOENT,errno.EAGAIN,0]:
                changed=copy.deepcopy(outcome)
                changed['streams']['stdout']=encode(dict(document,errno=error))
                self.assertRaises(AuthorityRefusal,q._check_probe,'read',changed,{},denied=True)

    def test_reaped_leader_is_only_observed_never_signaled(self):
        process=SimpleNamespace(pid=123,returncode=0)
        with mock.patch.object(q.os,'getpgrp',return_value=456), \
             mock.patch.object(q.os,'getpgid') as identity, \
             mock.patch.object(q.os,'killpg',side_effect=ProcessLookupError) as send:
            q._settle(process)
            send.assert_called_once_with(123,0)
            identity.assert_not_called()
        with mock.patch.object(q.os,'getpgrp',return_value=456), \
             mock.patch.object(q.os,'killpg') as send, \
             mock.patch.object(q.time,'monotonic',side_effect=[0,3]):
            self.assertRaises(AuthorityRefusal,q._settle,process)
            send.assert_called_once_with(123,0)

    def test_owned_group_timeout_escalates_before_reap_then_observes_bounded_exit(self):
        process=SimpleNamespace(pid=123,returncode=None)
        def wait(timeout):
            if process.wait.call_count == 1:
                raise q.subprocess.TimeoutExpired('owned',timeout)
            process.returncode=-9
            return -9
        process.wait=mock.Mock(side_effect=wait)
        def send(pid,sig):
            if sig==0: raise ProcessLookupError
        with mock.patch.object(q.os,'getpgrp',return_value=456), \
             mock.patch.object(q.os,'getpgid',return_value=123), \
             mock.patch.object(q.os,'killpg',side_effect=send) as signal_group:
            q._settle(process)
            self.assertEqual(signal_group.call_args_list,[mock.call(123,q.signal.SIGTERM),
                mock.call(123,q.signal.SIGKILL),mock.call(123,0)])
        process.returncode=None
        with mock.patch.object(q.os,'getpgrp',return_value=456), \
             mock.patch.object(q.os,'getpgid',return_value=999), \
             mock.patch.object(q.os,'killpg') as send:
            self.assertRaises(AuthorityRefusal,q._settle,process)
            send.assert_not_called()
        with mock.patch.object(q.os,'getpgrp',return_value=456), \
             mock.patch.object(q.os,'getpgid',return_value=123), \
             mock.patch.object(q.os,'killpg',side_effect=PermissionError):
            self.assertRaises(PermissionError,q._settle,process)

    def test_selector_failure_settles_owned_producer_and_closes_both_pipes(self):
        process=SimpleNamespace(stdout=mock.Mock(),stderr=mock.Mock(),returncode=None)
        compiler=next(e.path for e in self.plan.inventory if e.role=='dependency')
        source=b'fixed source'
        with mock.patch.object(q,'_read',return_value=(source,None)), \
             mock.patch.object(q,'PROBE_SOURCE_SHA256',hashlib.sha256(source).hexdigest()), \
             mock.patch.object(q,'_write_new') as write, \
             mock.patch.object(q.subprocess,'Popen',return_value=process), \
             mock.patch.object(q,'_acquire_group',return_value='owned'), \
             mock.patch.object(q,'_settle') as settle, \
             mock.patch.object(q.selectors,'DefaultSelector',side_effect=RuntimeError('selector setup')):
            self.assertRaisesRegex(RuntimeError,'selector setup',q._build_probe,
                self.plan,self.store,compiler,Path('/Volumes/Semaprax/evidence'))
            settle.assert_called_once_with(process,'owned')
            process.stdout.close.assert_called_once()
            process.stderr.close.assert_called_once()
            record=json.loads(write.call_args.args[1])
            self.assertEqual(record['selected_error']['message'],'selector setup')

    def test_partial_runtime_cleanup_forces_nonblocking_before_draining(self):
        runtime=self.runtime()
        runtime.process=SimpleNamespace(stdin=mock.Mock(),stdout=mock.Mock(),stderr=mock.Mock())
        runtime.group='owned';runtime.buffer=bytearray();runtime.stderr_bytes=0
        runtime.process.stdout.fileno.return_value=10
        runtime.process.stderr.fileno.return_value=11
        runtime.selector=mock.Mock();runtime.wire=mock.Mock();runtime.stderr=mock.Mock()
        events=[]
        with mock.patch.object(q,'_settle'), \
             mock.patch.object(q.os,'set_blocking',side_effect=lambda fd,b:events.append(('safe',fd,b))), \
             mock.patch.object(q.os,'read',side_effect=lambda fd,n:(events.append(('read',fd)),b'')[1]):
            runtime.close()
        self.assertEqual(events[:2],[('safe',10,False),('safe',11,False)])
        self.assertEqual(events[2:],[('read',10),('read',11)])

    def test_cleanup_failure_is_recorded_with_or_without_selected_primary(self):
        runtime=SimpleNamespace(close=mock.Mock(side_effect=RuntimeError('late settlement')))
        for primary in [None,ValueError('selected')]:
            with mock.patch.object(q,'_write_new') as write:
                if primary is None:
                    self.assertRaisesRegex(RuntimeError,'late settlement',q._cleanup_runtime,
                        runtime,Path('/Volumes/Semaprax/evidence'),primary,[{'case':'completed'}])
                else:
                    q._cleanup_runtime(runtime,Path('/Volumes/Semaprax/evidence'),primary,[{'case':'completed'}])
                record=json.loads(write.call_args.args[1])
                self.assertEqual(record['selected_primary_preserved'],primary is not None)
                self.assertEqual(record['completed_observations'],[{'case':'completed'}])
                self.assertFalse(record['authority_granted'])

    def test_cleanup_evidence_write_failure_does_not_replace_selected_failure(self):
        error=RuntimeError('selected cleanup')
        runtime=SimpleNamespace(close=mock.Mock(side_effect=error))
        with mock.patch.object(q,'_write_new',side_effect=OSError('evidence full')):
            primary=ValueError('primary')
            q._cleanup_runtime(runtime,Path('/Volumes/Semaprax/evidence'),primary,[])
            self.assertIn('evidence full',primary.__notes__[0])
            q._record_primary_refusal(Path('/Volumes/Semaprax/evidence'),primary,[])
            self.assertIn('primary evidence write failed',primary.__notes__[1])
            self.assertEqual(str(primary),'primary')
            with self.assertRaises(RuntimeError) as selected:
                q._cleanup_runtime(runtime,Path('/Volumes/Semaprax/evidence'),None,[])
            self.assertIs(selected.exception,error)
            self.assertIsInstance(selected.exception.__cause__,OSError)

    def test_termination_retains_full_error_and_never_kills_a_reused_pid(self):
        document={'schema':'semaprax.darwin-probe.v1','case':'timeout','pid':123,'uid':501,
                  'result':0,'errno':0}
        error={'code':-321,'message':'actual timeout','data':{'retained':True}}
        outcome={'runtime_error':error,'streams':{'stdout':encode(document),'stderr':b'actual'}}
        with mock.patch.object(q.os,'geteuid',return_value=501), \
             mock.patch.object(q.os,'kill',side_effect=ProcessLookupError) as observe:
            result=q._termination_observation('timeout',outcome)
            self.assertIs(result['actual_runtime_error'],error)
            self.assertIsNone(result['actual_terminal'])
            observe.assert_called_once_with(123,0)
            self.assertIn('NOT_DESCENDANT',result['scope'])


if __name__=='__main__':
    unittest.main()
