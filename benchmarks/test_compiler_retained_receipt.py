"""Focused offline synthetic protocol fixtures; no compiler/provider execution."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import compiler_input_capture as capture
import compiler_retained_receipt as retained
import live_campaign_common as common

class CompilerRetainedReceiptTests(unittest.TestCase):
    def fixture(self, root):
        candidate=root/'candidate';(candidate/'src').mkdir(parents=True)
        originals={'semaprax.toml':'schema = "semaprax.manifest.v1"\n[modules]\nsources = ["src/app.spx", "src/schema.spx", "src/tests.spx"]\n',
            'src/app.spx':'original app\n','src/schema.spx':'original schema\n','src/tests.spx':'original tests\n'}
        for path,text in originals.items():(candidate/path).write_text(text)
        compiler=root/'compiler';compiler.write_bytes(b'explicit configured compiler fixture, never executed')
        binding={'source_commit':'a'*40,'binary_sha256':capture.provenance.digest(compiler),'real_binary':str(compiler)}
        args=['json-codec','semaprax.toml','--source','src/schema.spx','--type','app.row','--output','receipt.json','--retained-receipt']
        def source(path,text):return {'path':path,'source':text,'sha256':'sha256:'+hashlib.sha256(text.encode()).hexdigest()}
        authored=[source(path,text) for path,text in sorted({**{k:v for k,v in originals.items() if k!='semaprax.toml'},'dependencies/std.example/0.1.0/example.spx':'compiler-bundled source fixture\n'}.items())]
        generated=copy.deepcopy(authored)
        for row in generated:
            if row['path']=='src/schema.spx':row.update(source(row['path'],row['source']+'mixed generated helper\n'))
        original_by_path={r['path']:r for r in authored};new_by_path={r['path']:r for r in generated}
        bundle={'schema':'semaprax.json-codec-bundle.v1','authored_project_revision':'original-revision','candidate_project_revision':'candidate-revision','candidate_workspace_revision':'workspace-revision','manifest':originals['semaprax.toml'],
            'authored_sources':authored,'candidate_sources':generated,'entries':[{'source_path':'src/schema.spx','record_id':'app.row','profile':'flat-scalars.v1','max_string_bytes':None,'max_array_items':None,
            'authored_sha256':original_by_path['src/schema.spx']['sha256'],'candidate_sha256':new_by_path['src/schema.spx']['sha256'],'generated_declarations':['generated.fixture']}]}
        encoded=json.dumps(bundle,ensure_ascii=False,separators=(',',':')).encode()
        held=[{'role':'manifest' if p=='semaprax.toml' else 'declared_source','path':p,'bytes':len(text.encode()),'sha256':'sha256:'+hashlib.sha256(text.encode()).hexdigest(),'source':text}for p,text in originals.items()]
        receipt={'schema':'semaprax.json-codec-retained-receipt.v1','compiler_package_version':'fixture-not-runtime','compiler_binary_sha256':None,'repeat_kind':'same-process.same-retained-revision.v1','independent_execution':False,'publication_policy':'explicit-new-artifact.no-overwrite.v1','held_inputs':held,
            'derivations':[{'ordinal':n,'project_revision':bundle['authored_project_revision'],'bundle_bytes':len(encoded),'bundle_sha256':'sha256:'+hashlib.sha256(encoded).hexdigest()}for n in [1,2]],'bundle':bundle}
        selected=[{'path':p,'bytes':len(originals[p].encode()),'sha256':hashlib.sha256(originals[p].encode()).hexdigest()}for p in ['semaprax.toml','src/schema.spx']]
        observation={'observer':'sandboxed_harness_proxy','binary_sha256_before':binding['binary_sha256'],'binary_sha256_after':binding['binary_sha256'],'invocation_sha256':retained.invocation_sha256(str(compiler),args,str(candidate)),'exit_code':0}
        return candidate,binding,args,receipt,selected,observation
    def raw(self,receipt):return json.dumps(receipt,ensure_ascii=False,separators=(',',':')).encode()+b'\n'
    def validate(self,fixture,receipt=None,observation=None,selected=None):
        candidate,binding,args,document,before,execution=fixture
        return retained.validate(self.raw(document if receipt is None else receipt),args,str(candidate),binding,execution if observation is None else observation,before if selected is None else selected)
    def test_complete_compiler_report_inventories_remain_distinct_and_all_authorship_execution_nulls_stay_honest(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=self.fixture(Path(directory));report=self.validate(fixture)
            self.assertTrue(report['compiler_reported_held_input_closure_complete']);self.assertEqual([len(i['files'])for i in report['inventories']],[4,4,4])
            for key in ['input_closure_complete','exact_compiler_input_binding','execution_independently_authenticated','semantic_replay_performed_by_host','ratio_eligible']:self.assertFalse(report[key])
            for key in ['independent_execution_proof','generated_source_classification','authored_token_subtraction','actual_billed_usd','fixed_context_tokens']:self.assertIsNone(report[key])
            with patch.object(common,'require_authored_source_tokenizer'),patch.object(common,'tokenize_texts',side_effect=lambda texts,metadata:[len(x['content'])**2 for x in texts]):
                measured=retained.measure(self.raw(fixture[3]),report,{'synthetic-pinned-fixture':True})
            self.assertEqual(len(measured['inventories']),3);self.assertFalse(measured['ratio_eligible'])
            for inventory in measured['inventories']:self.assertEqual(inventory['full_file_proxy_tokens'],sum(x['tokens']for x in inventory['files']))
            self.assertIsNone(measured['authored_token_subtraction'])
    def test_null_tokenizer_and_bad_counts_or_broken_runtime_never_invent_measurement(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=self.fixture(Path(directory));raw=self.raw(fixture[3]);report=self.validate(fixture)
            with patch.object(common,'tokenize_texts') as run:
                result=retained.measure(raw,report,None);run.assert_not_called();self.assertTrue(all(x['full_file_proxy_tokens'] is None for x in result['inventories']))
            for counts in [[],[True]*4,[-1]*4,[1.5]*4,[0]*5]:
                with patch.object(common,'require_authored_source_tokenizer'),patch.object(common,'tokenize_texts',return_value=counts):
                    with self.assertRaises(ValueError):retained.measure(raw,report,{})
            with patch.object(common,'require_authored_source_tokenizer',side_effect=RuntimeError('broken runtime')):
                with self.assertRaises(RuntimeError):retained.measure(raw,report,{})
    def test_closed_json_full_closure_and_forged_repeat_profile_output_and_truth_fields_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=self.fixture(Path(directory))
            def mutations(doc,n):
                if n==0:doc['held_inputs'].pop()
                if n==1:doc['held_inputs'][1]['source']+='x'
                if n==2:doc['held_inputs'][1]['path']='../escape'
                if n==3:doc['derivations'].pop()
                if n==4:doc['derivations'][1]['bundle_sha256']='sha256:'+'f'*64
                if n==5:doc['bundle']['entries'][0]['record_id']='other.record'
                if n==6:doc['bundle']['candidate_sources'][0]['source']+='x'
                if n==7:doc['independent_execution']=True
                if n==8:doc['compiler_binary_sha256']='sha256:'+'a'*64
                if n==9:doc['extra_authority']=True
                if n==10:doc['held_inputs'][0]['bytes']=True
                if n==11:doc['bundle']['entries'][0]['max_string_bytes']=True
                if n==12:doc['bundle']['authored_sources'].append(copy.deepcopy(doc['bundle']['authored_sources'][0]))
                if n==13:doc['derivations'][0]['ordinal']=True
                if n==14:
                    row=doc['bundle']['candidate_sources'][0];row['source']+='forged unselected content';row['sha256']='sha256:'+hashlib.sha256(row['source'].encode()).hexdigest()
                    encoded=json.dumps(doc['bundle'],ensure_ascii=False,separators=(',',':')).encode()
                    for repeat in doc['derivations']:repeat.update(bundle_bytes=len(encoded),bundle_sha256='sha256:'+hashlib.sha256(encoded).hexdigest())
            for n in range(15):
                changed=copy.deepcopy(fixture[3]);mutations(changed,n)
                with self.subTest(mutation=n),self.assertRaises((ValueError,KeyError)):self.validate(fixture,changed)
            raw=self.raw(fixture[3]);duplicate=raw.replace(b'{',b'{"schema":"forged",',1)
            with self.assertRaises(ValueError):retained.validate(duplicate,fixture[2],str(fixture[0]),fixture[1],fixture[5],fixture[4])
            with patch.object(retained,'MAX_BYTES',16),self.assertRaises(ValueError):self.validate(fixture)
    def test_exact_invocation_binary_before_after_status_and_selected_source_binding_refuse_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=self.fixture(Path(directory))
            for key,value in [('observer','runner-authenticated'),('binary_sha256_before','f'*64),('binary_sha256_after','f'*64),('invocation_sha256','f'*64),('exit_code',1),('exit_code',False)]:
                changed=dict(fixture[5]);changed[key]=value
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):self.validate(fixture,observation=changed)
            selected=copy.deepcopy(fixture[4]);selected[1]['sha256']='f'*64
            with self.assertRaises(ValueError):self.validate(fixture,selected=selected)
            self.assertNotEqual(retained.invocation_sha256('/compiler',['a','bc'],'/cwd'),retained.invocation_sha256('/compiler',['ab','c'],'/cwd'))
    def test_broker_retains_raw_receipt_and_independent_inventories_after_candidate_removal_and_detects_report_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);fixture=self.fixture(root);candidate,binding,args,receipt,selected,observation=fixture
            mailbox=root/'mailbox';mailbox.mkdir();evidence=root/'evidence';evidence.mkdir()
            broker=capture.Broker(candidate,mailbox,evidence,binding,retained_receipts_enabled=True);self.addCleanup(broker.finish)
            start=broker.handle('1'*32,{'operation':'start','argv':args,'cwd':str(candidate)});self.assertNotIn('capture_error',start)
            (candidate/'receipt.json').write_bytes(self.raw(receipt))
            finish=broker.handle('2'*32,{'operation':'finish','start':'1'*32,'exit_code':0,'compiler_receipt_observation':observation});self.assertNotIn('capture_error',finish)
            summary=broker.finish();(candidate/'receipt.json').unlink();(candidate/'src/schema.spx').write_text('later replacement')
            report=capture.retained_file_inventories(summary,None)
            self.assertEqual(report['inventories'][1]['kind'],'compiler_receipt_artifact');self.assertEqual(len(report['compiler_receipts']),1)
            self.assertEqual(report['compiler_receipts'][0]['status'],'structurally_valid_compiler_report');self.assertFalse(report['exact_compiler_input_binding'])
            adopted=evidence/finish['compiler_retained_receipt_adoption']['path'];adopted.write_bytes(adopted.read_bytes()+b' ')
            with self.assertRaisesRegex(ValueError,'runner binding'):capture.retained_file_inventories(summary,None)
    def test_malformed_receipt_preserves_attempt_status_and_never_labels_json_as_mixed_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);candidate,binding,args,*_=self.fixture(root)
            mailbox=root/'mailbox';mailbox.mkdir();evidence=root/'evidence';evidence.mkdir()
            broker=capture.Broker(candidate,mailbox,evidence,binding,retained_receipts_enabled=True);self.addCleanup(broker.finish)
            broker.handle('1'*32,{'operation':'start','argv':args,'cwd':str(candidate)})
            (candidate/'receipt.json').write_text('{"forged":true}')
            finish=broker.handle('2'*32,{'operation':'finish','start':'1'*32,'exit_code':0})
            self.assertEqual(finish['exit_code_proxy_reported'],0);self.assertIn('capture_error',finish)
            report=capture.retained_file_inventories(broker.finish(),None)
            self.assertEqual(report['inventories'][1]['kind'],'compiler_receipt_artifact');self.assertEqual(report['compiler_receipts'][0]['status'],'adoption_failed')
    def test_proxy_observes_configured_binary_and_preserves_the_exact_original_sandbox_invocation(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=self.fixture(Path(directory));candidate,binding,args,*_=fixture
            config={'compiler':binding,'retained_receipts_enabled':True}
            messages=[]
            def request(config,document):messages.append(document);return '1'*32
            with patch.object(capture,'_request',side_effect=request),patch.object(capture,'_measurement'),patch.object(capture.os,'getcwd',return_value=str(candidate)),patch.object(capture.subprocess,'run',return_value=subprocess.CompletedProcess([],0)) as run:
                self.assertEqual(capture.proxy_main(config,args),0)
            run.assert_called_once_with([binding['real_binary'],*args],check=False)
            before,finish=messages;self.assertEqual(before['argv'],args);self.assertEqual(before['cwd'],str(candidate))
            self.assertEqual(finish['compiler_receipt_observation']['binary_sha256_before'],binding['binary_sha256'])
            self.assertEqual(finish['compiler_receipt_observation']['binary_sha256_after'],binding['binary_sha256']);self.assertEqual(finish['exit_code'],0)
    def test_helper_is_pinned_optional_and_copied_only_by_matching_fresh_harness_plans(self):
        for enabled in [False,True]:
            with self.subTest(enabled=enabled),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);workspace=root/'workspace';candidate=workspace/'candidate';candidate.mkdir(parents=True)
                binary=root/'compiler';binary.write_bytes(b'never executed')
                names=['compiler_input_capture.py','compiler_output_provenance.py']+(['compiler_retained_receipt.py']if enabled else [])
                hashes={'benchmarks/'+name:capture.provenance.digest(Path(capture.__file__).with_name(name))for name in names}
                settings={'compiler_source_commit':'a'*40,'source_binary_sha256':capture.provenance.digest(binary),'harness_source_snapshot':{'files_sha256':hashes}}
                row={}
                with capture._captured_compiler(candidate,workspace,root/'artifacts','case',settings,binary,row):
                    copied=Path(row['compiler_authoring_proxy']['path']).parent/'compiler_retained_receipt.py'
                    self.assertEqual(copied.exists(),enabled)
                if enabled:
                    wrong=copy.deepcopy(settings);wrong['harness_source_snapshot']['files_sha256']['benchmarks/compiler_retained_receipt.py']='f'*64
                    with self.assertRaisesRegex(ValueError,'differs from immutable'):
                        with capture._captured_compiler(candidate,workspace,root/'other-artifacts','case',wrong,binary,{}):self.fail('unpinned helper accepted')

if __name__=='__main__':unittest.main()
