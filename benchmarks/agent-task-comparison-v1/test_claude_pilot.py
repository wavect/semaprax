"""Focused native transport and explicit-waiver gates; zero model requests."""
import base64
import copy
import importlib.util
import json
import os
import shlex
import selectors
import subprocess
import time
from pathlib import Path
import tempfile
import unittest
import urllib.request
from unittest import mock

spec = importlib.util.spec_from_file_location('native_task_pilot', Path(__file__).with_name('claude_pilot.py'))
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)


def envelope():
    return {'type': 'result', 'subtype': 'success', 'is_error': False, 'session_id': 'fixture',
            'num_turns': 3, 'queued_turn_count': 0, 'subagent_stats': {'spawned': 0},
            'permission_denials': [], 'total_cost_usd': .01,
            'modelUsage': {'claude-haiku-4-5-20251001': {'canonicalModel': 'claude-haiku-4-5', 'provider': 'firstParty'}},
            'usage': {'input_tokens': 10, 'output_tokens': 12, 'cache_creation_input_tokens': 0, 'cache_read_input_tokens': 0}}


def fixture_forward(environment):
    request={'model':m.MODELS[0]['model'],'max_tokens':512,
             'messages':[{'role':'user','content':'fixture'}],
             'tools':[{'name':'mcp__semaprax__command','input_schema':{'type':'object'}}]}
    message=urllib.request.Request(environment['ANTHROPIC_BASE_URL']+'/v1/messages',
        data=json.dumps(request).encode(),headers={'Authorization':'Bearer nonsecret-fixture'})
    with urllib.request.urlopen(message,timeout=3) as response:response.read()


class NativePilotTests(unittest.TestCase):
    def setUp(self):
        patcher=mock.patch.object(m.guard_module,'_tls_forward',return_value=(200,'application/json',b'{}'))
        patcher.start();self.addCleanup(patcher.stop)

    def test_exact_usage_key_and_canonical_identity_are_distinct(self):
        result = m.usage(m.canonical(envelope()), m.MODELS[0], m.CAPS)
        self.assertEqual(result['usage']['input_tokens'], 10)
        self.assertIsNone(result['subscription_invoice_cost_usd'])
        for change in (lambda x: x['modelUsage']['claude-haiku-4-5-20251001'].update(canonicalModel='other'),
                       lambda x: x.update(num_turns=33), lambda x: x.update(total_cost_usd=2),
                       lambda x: x['usage'].update(input_tokens=True), lambda x: x.update(queued_turn_count=1)):
            value = envelope(); change(value)
            with self.assertRaises(ValueError):m.usage(m.canonical(value), m.MODELS[0], m.CAPS)

    def test_terminal_success_includes_the_final_non_tool_turn(self):
        value=envelope();value.update(num_turns=m.CAPS['max_turns']+1,terminal_reason='completed')
        observed=m.usage(m.canonical(value),m.MODELS[0],m.CAPS)
        self.assertEqual(observed['num_turns'],m.CAPS['max_turns']+1)
        value['num_turns']+=1
        with self.assertRaisesRegex(ValueError,'turn_bound'):m.usage(m.canonical(value),m.MODELS[0],m.CAPS)

    def test_review_waiver_preserves_historical_ineligibility_and_other_failures(self):
        protocol = {'authority': {'review': {'authorization': 'explicit user fixture waiver'}}, 'runner_revision': 'a' * 40}
        record = {'status': 'ineligible', 'provider_usage': {'status': 'observed'}, 'eligibility': {'reasons': ['blinded active review time: absent']}}
        result = m.Transport('0' * 64).classify_record(record, protocol)
        self.assertEqual(result['status'], 'ineligible')
        self.assertTrue(result['eligible_for_technical_scoring_under_waiver'])
        self.assertIsNone(result['human_review']['active_ms'])
        self.assertIsNone(result['human_review']['reviewer_id'])
        record['eligibility']['reasons'].append('typed stale/recovery metrics: absent')
        result = m.Transport('0' * 64).classify_record(record, protocol)
        self.assertFalse(result['eligible_for_technical_scoring_under_waiver'])

    def test_frozen_protocol_has_eighteen_positions_for_each_model_and_refuses_drift(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            exe = root / 'exe'; exe.write_bytes(b'fixture'); exe.chmod(0o700)
            home=root/'home'; home.mkdir(mode=0o700)
            evidence=root/'evidence'; evidence.mkdir(mode=0o700)
            authority = {'claude': str(exe), 'compiler': str(exe), 'home': str(home), 'login': 'fixture',
                         'evidence_root': str(evidence), 'authorization': 'no inference fixture',
                         'review': {'mode': 'operator_recorded_user_waiver', 'authorization': 'explicit fixture waiver'}}
            protocol = m.freeze(authority)
            self.assertEqual(protocol['required_records'], 36)
            self.assertEqual(len(protocol['schedule']['rows']), 18)
            path = root / 'protocol.json'; m.write(path, protocol)
            self.assertEqual(m.load(path, m.sha(path.read_bytes()))[0], protocol)
            bad = copy.deepcopy(authority); bad['review']['mode'] = 'ai_is_human'
            with self.assertRaises(ValueError):m.freeze(bad)
            exe.write_bytes(b'drifted')
            with self.assertRaisesRegex(ValueError, 'drift'):m.load(path, m.sha(path.read_bytes()))

    def test_private_disjoint_roots_and_nonrefundable_aggregate_budget(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()
            with self.assertRaisesRegex(ValueError,'overlap'):m.require_disjoint(root,root/'nested')
            a=root/'a';a.mkdir(mode=0o700);b=root/'b';b.mkdir(mode=0o700)
            m.require_disjoint(a,b)
            protocol={'authority':{'evidence_root':str(a)},'models':m.MODELS,
                      'caps':dict(m.CAPS,max_estimated_api_usd=.05,cohort_max_estimated_api_usd=.1)}
            first=m.reserve(protocol,'a'*64,'haiku45-01')
            self.assertEqual(first['cohort_reserved_micro_usd'],50000)
            with self.assertRaisesRegex(ValueError,'already_reserved'):m.reserve(protocol,'a'*64,'haiku45-01')
            with self.assertRaisesRegex(ValueError,'unknown'):m.reserve(protocol,'a'*64,'haiku45-02')
            m.settle_cost(protocol,'a'*64,'haiku45-01',.01)
            m.reserve(protocol,'a'*64,'haiku45-02');m.settle_cost(protocol,'a'*64,'haiku45-02',.01)
            with self.assertRaisesRegex(ValueError,'exhausted'):m.reserve(protocol,'a'*64,'haiku45-03')
            protocol['authority']['evidence_root']=str(b)
            m.reserve(protocol,'a'*64,'sonnet55-01');m.settle_cost(protocol,'a'*64,'sonnet55-01',None)
            with self.assertRaisesRegex(ValueError,'halted'):m.reserve(protocol,'a'*64,'sonnet55-02')
            self.assertEqual(json.loads((b/'dispatch-budget.json').read_bytes())['cells'],['sonnet55-01'])

    def test_guarded_unknown_cost_retains_full_reservation_and_can_continue(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()
            protocol={'authority':{'evidence_root':str(root)},'models':m.MODELS,'caps':m.CAPS}
            first=m.reserve(protocol,'a'*64,'haiku45-01')
            m.settle_cost(protocol,'a'*64,'haiku45-01',None,bounded_incomplete=True)
            second=m.reserve(protocol,'a'*64,'sonnet55-01')
            self.assertEqual(first['reserved_api_equivalent_micro_usd'],250000)
            self.assertEqual(second['cohort_reserved_micro_usd'],500000)
            ledger=json.loads((root/'dispatch-budget.json').read_bytes())
            self.assertIsNone(ledger['reported_costs']['haiku45-01']);self.assertIsNone(ledger['halted'])
            m.settle_cost(protocol,'a'*64,'sonnet55-01',.26,bounded_incomplete=True)
            with self.assertRaisesRegex(ValueError,'halted'):m.reserve(protocol,'a'*64,'haiku45-02')

    @unittest.skipUnless(os.environ.get('SEMAPRAX_PILOT_CLAUDE'), 'explicit native metadata probe required')
    def test_native_isolation_physically_suppresses_hostile_session_hook(self):
        with tempfile.TemporaryDirectory(prefix='pilot-customization-') as temporary:
            root=Path(temporary).resolve();home=root/'home';home.mkdir();(home/'.claude').mkdir();work=root/'work';work.mkdir()
            marker=root/'hook-ran'
            settings={'hooks':{'SessionStart':[{'hooks':[{'type':'command','command':'/usr/bin/touch '+shlex.quote(str(marker))}]}]}}
            (home/'.claude/settings.json').write_text(json.dumps(settings))
            environment={'HOME':str(home),'USER':'fixture','LOGNAME':'fixture','PATH':'/usr/bin:/bin',
                         'DISABLE_AUTOUPDATER':'1','CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC':'1'}
            request=b'{"type":"control_request","request_id":"init","request":{"subtype":"initialize"}}\n'
            results=[]
            for flags,expected in (([],True),(m.ISOLATION_FLAGS,False)):
                marker.unlink(missing_ok=True)
                command=[os.environ['SEMAPRAX_PILOT_CLAUDE'],'--print','--input-format','stream-json','--output-format','stream-json',
                         '--verbose','--tools','','--no-session-persistence','--permission-prompts','none',*flags]
                result=m.native.capture(command,work,environment,request,15)
                messages=[json.loads(line) for line in base64.b64decode(result['stdout_base64']).splitlines()]
                self.assertIsNone(result['failure']);self.assertEqual(result['exit_code'],0)
                self.assertEqual(marker.exists(),expected)
                self.assertFalse(any(message.get('type')=='result' for message in messages))
                results.append({'flags':flags,'hook_executed':marker.exists(),'capture':result})
            if os.environ.get('SEMAPRAX_PILOT_METADATA_EVIDENCE'):
                m.write(os.environ['SEMAPRAX_PILOT_METADATA_EVIDENCE'],{'user_messages':0,'model_result_messages':0,'observations':results})

    @unittest.skipUnless(os.environ.get('SEMAPRAX_PILOT_CLAUDE'), 'explicit native metadata probe required')
    def test_native_isolation_preserves_only_explicit_confined_mcp_discovery(self):
        with tempfile.TemporaryDirectory(dir='/private/tmp',prefix='pilot-mcp-') as temporary:
            root=Path(temporary).resolve()
            for name in ('home','state','candidate'):(root/name).mkdir(mode=0o700)
            home=root/'home';state=root/'state';candidate=root/'candidate'
            (home/'.claude').mkdir();marker=root/'hook-ran'
            (home/'.claude/settings.json').write_text(json.dumps({'hooks':{'SessionStart':[{'hooks':[{'type':'command','command':'/usr/bin/touch '+str(marker)}]}]}}))
            (state/'mcp.py').write_bytes((m.ROOT/'scripts/opencode_agent_task_pilot/mcp_gateway.py').read_bytes())
            gateway=state/'gateway';gateway.write_text('#!/bin/sh\nexit 17\n');gateway.chmod(0o700)
            profile=state/'policy.sb';profile.write_text(m.pilot.seatbelt_profile(candidate,(m.ROOT,home),state))
            confined=m.pilot.sandboxed('/usr/bin/env',profile,['-i','PATH=/usr/bin:/bin','HOME='+str(state),'/usr/bin/python3',str(state/'mcp.py'),str(gateway)])
            bridge=state/'bridge.py';bridge.write_bytes(Path(__file__).with_name('claude_mcp_bridge.py').read_bytes())
            config=state/'mcp.json';config.write_text(json.dumps({'mcpServers':{'semaprax':{'type':'stdio','command':'/usr/bin/python3','args':[str(bridge),str(state/'mcp-from-server'),str(state/'mcp-to-server'),str(state/'claimed')]}}}))
            outer=state/'network.sb';outer.write_text(m.guard_module.network_profile(65534))
            env={'HOME':str(home),'USER':'fixture','LOGNAME':'fixture','PATH':'/usr/bin:/bin','TMPDIR':str(state),
                 'DISABLE_AUTOUPDATER':'1','CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC':'1','ENABLE_TOOL_SEARCH':'false'}
            argv=[os.environ['SEMAPRAX_PILOT_CLAUDE'],'--print','--input-format','stream-json','--output-format','stream-json',
                  '--verbose','--tools','','--allowedTools','mcp__semaprax__command','--no-session-persistence','--permission-prompts','none',
                  '--mcp-config',str(config),*m.ISOLATION_FLAGS]
            broker=m.guard_module.McpProcess(state,confined,candidate);broker.__enter__()
            process=subprocess.Popen(['/usr/bin/sandbox-exec','-f',str(outer),*argv],cwd=candidate,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            selector=selectors.DefaultSelector();selector.register(process.stdout,selectors.EVENT_READ)
            messages=[];connected=None;last=0;end=time.monotonic()+10
            def send(identity,subtype):
                process.stdin.write(json.dumps({'type':'control_request','request_id':identity,'request':{'subtype':subtype}})+'\n');process.stdin.flush()
            try:
                send('init','initialize')
                while time.monotonic()<end and process.poll() is None and connected is None:
                    if time.monotonic()-last>1:send('status','mcp_status');last=time.monotonic()
                    for key,_ in selector.select(.1):
                        line=key.fileobj.readline()
                        if line:
                            value=json.loads(line);messages.append(value)
                            servers=value.get('response',{}).get('response',{}).get('mcpServers',[])
                            if servers and all(server.get('status')=='connected' for server in servers):connected=servers
                self.assertIsNotNone(connected)
                self.assertEqual([server['name'] for server in connected],['semaprax'])
                self.assertEqual([tool['name'] for tool in connected[0]['tools']],['command'])
                self.assertFalse(marker.exists())
                self.assertFalse(any(value.get('type')=='result' for value in messages))
                self.assertTrue(any(value.get('response',{}).get('response',{}).get('commands')==[] for value in messages))
            finally:
                process.stdin.close()
                try:process.wait(timeout=5)
                except subprocess.TimeoutExpired:process.kill();process.wait()
                selector.close();process.stdout.close();process.stderr.close();broker.__exit__()

    def test_transport_closes_environment_and_confines_only_mcp_gateway(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); exe=root/'exe';exe.write_bytes(b'fixture')
            for name in ('home','evidence','state','candidate'):(root/name).mkdir(mode=0o700)
            protocol={'authority':{'claude':str(exe),'home':str(root/'home'),'evidence_root':str(root/'evidence'),'login':'fixture'},
                      'pins':{'claude':m.sha(exe.read_bytes())},'host':{'native_platform':'darwin-arm64'},
                      'caps':m.CAPS,'cli_version':'2.1.286'}
            mcp={'semaprax':{'command':['/usr/bin/python3','server.py','gateway','wire'],
                            'environment':{'SEMAPRAX_PILOT_GATEWAY':'fixture-config'}}}
            version={'failure':None,'exit_code':0,'stdout_base64':base64.b64encode(b'2.1.286 (Claude Code)\n').decode()}
            response={'failure':None,'exit_code':0,'stdout_base64':base64.b64encode(m.canonical(envelope())).decode(),'stderr_base64':''}
            def capture(argv,cwd,env,prompt,timeout,**kwargs):
                self.assertNotIn('ANTHROPIC_API_KEY',env)
                if argv[-1]=='--version':return version
                self.assertIn('--strict-mcp-config',argv);self.assertIn('--max-turns',argv)
                self.assertNotIn('--safe-mode',argv);self.assertIn('--restricted',argv)
                self.assertNotIn('CLAUDE_CODE_SAFE_MODE',env);self.assertIn('--disable-slash-commands',argv)
                self.assertEqual(argv[argv.index('--tools')+1],'')
                self.assertEqual(argv[argv.index('--allowedTools')+1],'mcp__semaprax__command')
                fixture_forward(env);kwargs['on_started']();return response
            transport=m.Transport('0'*64)
            with mock.patch.object(m.native,'capture',side_effect=capture),mock.patch.object(m.native,'MANAGED',[]), \
                    mock.patch.object(m,'reserve',return_value={}),mock.patch.object(m,'settle_cost'):
                out,err,raw,session,usage=transport.execute(protocol,m.MODELS[0],root/'state',root/'candidate',root/'policy.sb',mcp,'task','semaprax-source-first',m.CAPS['seconds'])
            self.assertEqual(out,raw);self.assertEqual(session,'fixture');self.assertEqual(transport.receipt['dispatches'],1)
            server=transport.receipt['mcp_config']['mcpServers']['semaprax']
            self.assertEqual(server['command'],'/usr/bin/python3')
            confined=transport.receipt['confined_mcp_command']
            self.assertTrue(confined[0].endswith('sandbox-exec'))
            self.assertIn('TMPDIR='+str(root/'state'),confined)
            self.assertIn('-i',confined);self.assertNotIn('ANTHROPIC_API_KEY',' '.join(confined))
            self.assertEqual(transport.receipt['mcp_process_starts'],1)

    def test_wrong_canonical_model_with_valid_cost_halts_next_dispatch(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();exe=root/'exe';exe.write_bytes(b'fixture')
            for name in ('home','evidence','state','candidate'):(root/name).mkdir(mode=0o700)
            protocol={'authority':{'claude':str(exe),'home':str(root/'home'),'evidence_root':str(root/'evidence'),'login':'fixture'},
                      'pins':{'claude':m.sha(exe.read_bytes())},'host':{'native_platform':'darwin-arm64'},
                      'caps':m.CAPS,'models':m.MODELS,'cli_version':'2.1.286'}
            mcp={'semaprax':{'command':['/usr/bin/python3','server.py','gateway','wire'],
                            'environment':{'SEMAPRAX_PILOT_GATEWAY':'fixture-config'}}}
            bad=envelope();bad['modelUsage']['claude-haiku-4-5-20251001']['canonicalModel']='wrong'
            version={'failure':None,'exit_code':0,'stdout_base64':base64.b64encode(b'2.1.286 (Claude Code)\n').decode()}
            response={'failure':None,'exit_code':0,'stdout_base64':base64.b64encode(m.canonical(bad)).decode(),'stderr_base64':''}
            def capture(argv,*args,**kwargs):
                if argv[-1]=='--version':return version
                fixture_forward(args[1]);kwargs['on_started']();return response
            with mock.patch.object(m.native,'capture',side_effect=capture),mock.patch.object(m.native,'MANAGED',[]):
                with self.assertRaisesRegex(m.pilot.PilotFailure,'provenance_mismatch'):
                    m.Transport('a'*64,'haiku45-01').execute(protocol,m.MODELS[0],root/'state',root/'candidate',root/'policy.sb',mcp,'task','semaprax-source-first',m.CAPS['seconds'])
            ledger=json.loads((root/'evidence'/'dispatch-budget.json').read_bytes())
            self.assertEqual(ledger['reported_costs']['haiku45-01'],.01)
            self.assertIn('native_admission_failed',ledger['halted'])
            with self.assertRaisesRegex(ValueError,'halted'):m.reserve(protocol,'a'*64,'haiku45-02')

    def test_bounded_turn_exhaustion_is_failed_task_without_integrity_halt(self):
        value=envelope();value.update(subtype='error_max_turns',is_error=True,terminal_reason='max_turns',num_turns=m.CAPS['max_turns']+1)
        value['usage']['cache_read_input_tokens']=321221
        counters=m.usage(m.canonical(value),m.MODELS[0],m.CAPS)
        for change in (lambda x:x.update(num_turns=m.CAPS['max_turns']+2),
                       lambda x:x['usage'].update(cache_read_input_tokens=m.CAPS['max_cache_read_tokens']+1),
                       lambda x:x['usage'].update(input_tokens=m.CAPS['max_reported_tokens'])):
            invalid=copy.deepcopy(value);change(invalid)
            with self.assertRaises(ValueError):m.usage(m.canonical(invalid),m.MODELS[0],m.CAPS)
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();exe=root/'exe';exe.write_bytes(b'fixture')
            for name in ('home','evidence','state','candidate'):(root/name).mkdir(mode=0o700)
            protocol={'authority':{'claude':str(exe),'home':str(root/'home'),'evidence_root':str(root/'evidence'),'login':'fixture',
                                  'review':{'authorization':'fixture waiver'}},'runner_revision':'a'*40,
                      'pins':{'claude':m.sha(exe.read_bytes())},'host':{'native_platform':'darwin-arm64'},
                      'caps':m.CAPS,'models':m.MODELS,'cli_version':'2.1.286'}
            mcp={'semaprax':{'command':['/usr/bin/python3','server.py','gateway','wire'],
                            'environment':{'SEMAPRAX_PILOT_GATEWAY':'fixture-config'}}}
            version={'failure':None,'exit_code':0,'stdout_base64':base64.b64encode(b'2.1.286 (Claude Code)\n').decode()}
            response={'failure':None,'exit_code':1,'stdout_base64':base64.b64encode(m.canonical(value)).decode(),'stderr_base64':''}
            def capture(argv,*args,**kwargs):
                if argv[-1]=='--version':return version
                fixture_forward(args[1]);kwargs['on_started']();return response
            transport=m.Transport('a'*64,'haiku45-01')
            with mock.patch.object(m.native,'capture',side_effect=capture),mock.patch.object(m.native,'MANAGED',[]):
                observed=transport.execute(protocol,m.MODELS[0],root/'state',root/'candidate',root/'policy.sb',mcp,'task','semaprax-source-first',m.CAPS['seconds'])[-1]
            self.assertEqual(observed,counters)
            ledger=json.loads((root/'evidence'/'dispatch-budget.json').read_bytes());self.assertIsNone(ledger['halted'])
            m.reserve(protocol,'a'*64,'sonnet55-01')
            record={'status':'ineligible','outcome':'completed','failure':None,'provider_usage':observed,
                    'eligibility':{'reasons':['blinded active review time: absent']}}
            classified=transport.classify_record(record,protocol)
            self.assertEqual(classified['outcome'],'failed');self.assertEqual(classified['failure'],'native_provider_error_max_turns')
            self.assertEqual(classified['status'],'ineligible');self.assertIsNone(classified['human_review']['active_ms'])


if __name__ == '__main__':unittest.main()
