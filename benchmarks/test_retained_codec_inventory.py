"""Offline retained-byte accounting controls; no compiler/provider execution."""
import copy
import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import compiler_input_capture as capture
import live_campaign_common as common

class RetainedCodecInventoryTests(unittest.TestCase):
    def fixture(self, root):
        candidate = root / 'candidate';(candidate / 'src').mkdir(parents=True)
        original = b'module app; record Original { value: string }\n'
        (candidate / 'src/schema.spx').write_bytes(original)
        (candidate / 'src/unselected-helper.spx').write_bytes(b'unselected dependency\n')
        (candidate / 'semaprax.toml').write_bytes(b'[project]\nsource="src/schema.spx"\n')
        mailbox = root / 'mailbox';mailbox.mkdir();evidence = root / 'evidence';evidence.mkdir()
        compiler = {'source_commit': 'a' * 40, 'binary_sha256': 'b' * 64, 'real_binary': '/never-executed'}
        broker = capture.Broker(candidate, mailbox, evidence, compiler);self.addCleanup(broker.finish)
        start = '0' * 31 + '1';finish = '0' * 31 + '2'
        args = ['json-codec', 'semaprax.toml', '--source', 'src/schema.spx', '--type', 'app.row', '--output', 'derived.spx']
        first = broker.handle(start, {'operation': 'start', 'argv': args, 'cwd': str(candidate)})
        self.assertNotIn('capture_error', first)
        mixed = original + b'fn compiler_generated_helper() {}\n';(candidate / 'derived.spx').write_bytes(mixed)
        second = broker.handle(finish, {'operation': 'finish', 'start': start, 'exit_code': 0})
        self.assertNotIn('capture_error', second)
        (candidate / 'src/schema.spx').write_bytes(mixed);(candidate / 'derived.spx').unlink()
        return broker.finish(), original, mixed

    def measured(self, summary, counts=None):
        if counts is None:counts = lambda texts, metadata: [len(row['content']) ** 2 for row in texts]
        with patch.object(common, 'require_authored_source_tokenizer') as pinned, patch.object(common, 'tokenize_texts', side_effect=counts):
            result = capture.retained_file_inventories(summary, {'pinned-fixture': True});pinned.assert_called_once();return result

    def test_overwritten_schema_and_complete_mixed_output_stay_independent_without_subtraction(self):
        with tempfile.TemporaryDirectory() as directory:
            summary, original, mixed = self.fixture(Path(directory));result = self.measured(summary)
            self.assertEqual(result['status'], 'measured_proxy');inputs, outputs = result['inventories']
            self.assertEqual(inputs['kind'], 'original_selected_inputs');self.assertEqual(outputs['kind'], 'declared_full_mixed_output')
            self.assertEqual(inputs['request'], outputs['start_request'])
            schema = next(row for row in inputs['files'] if row['path'] == 'src/schema.spx')
            self.assertEqual(schema['sha256'], hashlib.sha256(original).hexdigest());self.assertEqual(schema['tokens'], len(original.decode()) ** 2)
            self.assertEqual(outputs['files'][0]['sha256'], hashlib.sha256(mixed).hexdigest());self.assertEqual(outputs['full_file_proxy_tokens'], len(mixed.decode()) ** 2)
            self.assertNotIn('src/unselected-helper.spx', [r['path'] for r in inputs['files']])
            for key in ['authored_token_subtraction','cumulative_authored_tokens','generated_source_classification','independent_execution_proof','repeat_output_sha256','actual_billed_usd','fixed_context_tokens']:self.assertIsNone(result[key])
            self.assertFalse(result['input_closure_complete']);self.assertFalse(result['exact_compiler_input_binding']);self.assertFalse(result['ratio_eligible'])

    def test_missing_tokenizer_retains_authenticated_originals_with_null_counts(self):
        with tempfile.TemporaryDirectory() as directory:
            summary, _, _ = self.fixture(Path(directory))
            with patch.object(common,'require_authored_source_tokenizer') as pinned,patch.object(common,'tokenize_texts') as runtime:
                result = capture.retained_file_inventories(summary,None)
                self.assertEqual(result['status'],'unmeasured');self.assertEqual(len(result['inventories']),2)
                self.assertTrue(all(x['full_file_proxy_tokens'] is None for x in result['inventories']));pinned.assert_not_called();runtime.assert_not_called()

    def test_retained_byte_and_receipt_drift_refuse_without_candidate_fallback(self):
        for which in ['bytes','request-receipt']:
            with self.subTest(which=which),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);summary,_,_=self.fixture(root)
                path=root/'evidence'/('0'*31+'1')/('before/inputs/src/schema.spx' if which=='bytes' else 'receipt.json')
                path.write_bytes(path.read_bytes()+b' ')
                with self.assertRaises(ValueError):self.measured(summary)

    def test_foreign_duplicate_escaping_and_orphan_receipts_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            summary,_,_=self.fixture(Path(directory))
            for path in ['../outside/receipt.json','/outside/receipt.json','foreign/receipt.json']:
                wrong=copy.deepcopy(summary);wrong['request_receipts'][0]['path']=path
                with self.assertRaises(ValueError):self.measured(wrong)
            duplicate=copy.deepcopy(summary);duplicate['request_receipts'].append(duplicate['request_receipts'][0])
            with self.assertRaises(ValueError):self.measured(duplicate)
            orphan=copy.deepcopy(summary);orphan['request_receipts']=orphan['request_receipts'][1:]
            with self.assertRaisesRegex(ValueError,'no retained original'):self.measured(orphan)

    def test_invalid_counts_and_broken_runtime_refuse_measurement(self):
        with tempfile.TemporaryDirectory() as directory:
            summary,_,_=self.fixture(Path(directory))
            for bad in ([],[True,1],[-1,1],[0.5,1],[1,1,1]):
                with self.assertRaises(ValueError):self.measured(summary,lambda texts,metadata:bad)
            with patch.object(common,'require_authored_source_tokenizer',side_effect=RuntimeError('broken local tokenizer')):
                with self.assertRaisesRegex(RuntimeError,'broken local tokenizer'):capture.retained_file_inventories(summary,{'pinned-fixture':True})

    def test_inventory_symlink_cannot_redirect_retained_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);summary,original,_=self.fixture(root)
            retained=root/'evidence'/('0'*31+'1')/'before/inputs/src/schema.spx'
            foreign=root/'outside';foreign.write_bytes(original);retained.unlink();retained.symlink_to(foreign)
            with self.assertRaises(ValueError):self.measured(summary)

    def test_real_capture_context_accepts_both_plan_layouts_and_refuses_conflicting_tool_hashes(self):
        real_broker = capture.Broker
        for layout in ['snapshot', 'legacy', 'conflict']:
            with self.subTest(layout=layout), tempfile.TemporaryDirectory() as directory:
                root=Path(directory);workspace=root/'workspace';candidate=workspace/'candidate'
                (candidate/'src').mkdir(parents=True)
                (candidate/'semaprax.toml').write_text('source="src/schema.spx"\n')
                original=b'original selected schema\n';mixed=original+b'generated helper\n'
                (candidate/'src/schema.spx').write_bytes(original)
                binary=root/'compiler';binary.write_bytes(b'never executed compiler fixture')
                hashes={'benchmarks/'+name:capture.provenance.digest(Path(capture.__file__).with_name(name))
                        for name in ['compiler_input_capture.py','compiler_output_provenance.py']}
                settings={'compiler_source_commit':'a'*40,'source_binary_sha256':capture.provenance.digest(binary)}
                if layout=='legacy':settings['harness_source_files_sha256']=hashes
                else:settings['harness_source_snapshot']={'files_sha256':hashes}
                if layout=='conflict':settings['harness_source_files_sha256']={**hashes,'benchmarks/compiler_input_capture.py':'f'*64}
                row={};held=[]
                def factory(*args,**kwargs):
                    broker=real_broker(*args,**kwargs);held.append(broker);return broker
                with patch.object(capture,'Broker',side_effect=factory):
                    if layout=='conflict':
                        with self.assertRaisesRegex(ValueError,'differs from immutable'):
                            with capture._captured_compiler(candidate,workspace,root/'artifacts','trial',settings,binary,row):
                                self.fail('conflicting tool hash gained capture')
                        continue
                    with capture._captured_compiler(candidate,workspace,root/'artifacts','trial',settings,binary,row) as proxy:
                        self.assertNotEqual(proxy,binary)
                        args=['json-codec','semaprax.toml','--source','src/schema.spx','--type','app.row','--output','derived.spx']
                        held[0].handle('0'*31+'1',{'operation':'start','argv':args,'cwd':str(candidate)})
                        (candidate/'derived.spx').write_bytes(mixed)
                        held[0].handle('0'*31+'2',{'operation':'finish','start':'0'*31+'1','exit_code':0})
                        (candidate/'src/schema.spx').write_bytes(mixed)
                    report=row['retained_compiler_file_inventories']
                    self.assertEqual(report['status'],'unmeasured');self.assertEqual(len(report['inventories']),2)
                    self.assertEqual(report['inventories'][0]['files'][1]['sha256'],hashlib.sha256(original).hexdigest())
                    self.assertTrue((root/'artifacts/compiler-input-retention/trial/summary.json').is_file())
                    self.assertFalse(report['input_closure_complete']);self.assertIsNone(report['authored_token_subtraction'])

if __name__=='__main__':unittest.main()
