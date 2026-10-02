"""Adversarial fixed-endpoint gate: all forwards are local test doubles."""
import concurrent.futures
import copy
import http.client
import importlib.util
import json
import os
from pathlib import Path
import selectors
import socket
import subprocess
import tempfile
import unittest
from unittest import mock

spec=importlib.util.spec_from_file_location('guard',Path(__file__).with_name('claude_forward_guard.py'))
g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
MODEL='claude-sonnet-5-5'

def body():
    return {'model':MODEL,'max_tokens':512,'messages':[{'role':'user','content':'fixture'}],
            'tools':[{'name':'mcp__semaprax__command','input_schema':{'type':'object'}}]}

def send(guard,value=None,*,raw=None,path=None,headers=None):
    c=http.client.HTTPConnection('127.0.0.1',guard.server.server_port,timeout=5)
    try:
        c.request('POST',path or '/'+guard.nonce+'/v1/messages?beta=true',
                  body=raw if raw is not None else json.dumps(value or body()).encode(),
                  headers=headers or {'Authorization':'Bearer PRIVATE_TEST_SENTINEL','Content-Type':'application/json'})
        r=c.getresponse();return r.status,r.read()
    finally:c.close()

class GuardTests(unittest.TestCase):
    def test_exact_model_output_bytes_modalities_tools_and_duplicate_keys(self):
        g.admit(json.dumps(body()).encode(),MODEL)
        cases=[]
        for key,value in [('model','other'),('max_tokens',513),('max_tokens',True),('service_tier','priority'),
                          ('safeguards',[{'type':'dangerous_tool_use','classifier_context':{'auto_mode':'private'}}]),
                          ('tools',[{'type':'web_search_20250305','name':'web_search'}]),
                          ('messages',[{'role':'user','content':[{'type':'image','source':{'type':'url','url':'https://example.com'}}]}])]:
            v=body();v[key]=value;cases.append(json.dumps(v).encode())
        v=body();v['messages'][0]['cache_control']={'type':'ephemeral'};cases.append(json.dumps(v).encode())
        v=body();v['messages'][0]['content']='é'*20000;cases.append(json.dumps(v,ensure_ascii=False).encode())
        cases += [b'{"model":"x","model":"y"}',b'{"max_tokens":NaN}',b' '*32769]
        for raw in cases:
            with self.subTest(raw=raw[:40]):
                with self.assertRaises(ValueError):g.admit(raw,MODEL)

    def test_atomic_forward_count_header_redaction_and_no_refund_on_exception(self):
        calls=[]
        def fake(raw,headers,timeout):
            calls.append((raw,headers));raise OSError('PRIVATE_TEST_SENTINEL must never be recorded')
        with mock.patch.object(g,'_tls_forward',side_effect=fake),g.Guard(MODEL) as guard:
            with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
                statuses=list(pool.map(lambda _:send(guard)[0],range(12)))
        receipt=guard.receipt()
        g.validate_receipt(receipt,MODEL)
        drift=copy.deepcopy(receipt);drift['forwarded_request_bytes']+=1
        with self.assertRaisesRegex(ValueError,'aggregate'):g.validate_receipt(drift,MODEL)
        self.assertEqual(len(calls),9);self.assertEqual(receipt['forward_count'],9)
        self.assertEqual(statuses,[400]*12)
        self.assertNotIn('PRIVATE_TEST_SENTINEL',json.dumps(receipt))
        self.assertTrue(all(event['status']=='reserved' for event in receipt['events'] if 'sequence' in event))

    def test_aggregate_bytes_bad_path_and_api_key_never_forward(self):
        with mock.patch.object(g,'_tls_forward',return_value=(200,'application/json',b'{}')) as forward,g.Guard(MODEL) as guard:
            self.assertEqual(send(guard,path='/v1/messages')[0],400)
            self.assertEqual(send(guard,headers={'X-Api-Key':'PRIVATE_TEST_SENTINEL'})[0],400)
            v=body();v['messages'][0]['content']='a'*25000
            self.assertEqual(send(guard,v)[0],200);self.assertEqual(send(guard,v)[0],200)
            self.assertEqual(send(guard,v)[0],400)
            self.assertEqual(forward.call_count,2)
        self.assertLessEqual(guard.receipt()['forwarded_request_bytes'],65536)

    def test_production_forward_has_fixed_verified_tls_destination_and_no_redirect(self):
        connection=mock.Mock();response=connection.getresponse.return_value
        response.read1.side_effect=[b'{}',b''];response.status=302
        with mock.patch.object(g.http.client,'HTTPSConnection',return_value=connection) as connect:
            with self.assertRaisesRegex(ValueError,'redirect'):g._tls_forward(b'{}',{'Authorization':'Bearer private'},1)
        args,kwargs=connect.call_args
        self.assertEqual(args,('api.anthropic.com',443))
        self.assertTrue(kwargs['context'].check_hostname)
        self.assertEqual(kwargs['context'].verify_mode,g.ssl.CERT_REQUIRED)
        self.assertEqual(connection.request.call_args.args[:2],('POST','/v1/messages?beta=true'))
        connection.close.assert_called_once()

    @unittest.skipUnless(Path('/usr/bin/sandbox-exec').exists(),'Darwin physical network profile')
    def test_physical_network_profile_allows_only_guard_port(self):
        with socket.socket() as allowed,socket.socket() as denied,tempfile.TemporaryDirectory() as temporary:
            for sock in (allowed,denied):sock.bind(('127.0.0.1',0));sock.listen()
            ports=[sock.getsockname()[1] for sock in (allowed,denied)]
            for port in ports:
                with socket.create_connection(('127.0.0.1',port),timeout=1):pass
            policy=Path(temporary)/'network.sb';policy.write_text(g.network_profile(ports[0]))
            script='import socket,json\nr=[]\nfor p in '+repr(ports)+':\n try:\n  s=socket.create_connection(("127.0.0.1",p),timeout=1);s.close();r.append(True)\n except OSError:r.append(False)\nprint(json.dumps(r))'
            result=subprocess.run(['/usr/bin/sandbox-exec','-f',str(policy),'/usr/bin/python3','-c',script],capture_output=True,timeout=5)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertEqual(json.loads(result.stdout),[True,False])

    def test_mcp_bridge_is_one_use_and_preserves_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()
            server=['/usr/bin/python3','-u','-c','import sys\nfor line in sys.stdin: sys.stdout.write(line);sys.stdout.flush()']
            with g.McpProcess(root,server,root):
                argv=['/usr/bin/python3',str(Path(__file__).with_name('claude_mcp_bridge.py')),
                      str(root/'mcp-from-server'),str(root/'mcp-to-server'),str(root/'claimed')]
                process=subprocess.Popen(argv,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
                selector=selectors.DefaultSelector();selector.register(process.stdout,selectors.EVENT_READ)
                try:
                    raw=b'{"jsonrpc":"2.0","id":1,"method":"initialize"}\n'
                    process.stdin.write(raw);process.stdin.flush()
                    self.assertTrue(selector.select(3));self.assertEqual(process.stdout.readline(),raw)
                    second=subprocess.run(argv,input=raw,capture_output=True,timeout=3)
                    self.assertEqual(second.returncode,2)
                finally:
                    process.stdin.close();process.wait(timeout=3)
                    selector.close();process.stdout.close();process.stderr.close()

if __name__=='__main__':unittest.main()
