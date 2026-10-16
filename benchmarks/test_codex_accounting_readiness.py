"""Focused offline readiness controls; no provider, compiler or benchmark runs."""
import copy
import importlib.util
import json
import os
import sys
import tempfile
import unittest
from contextlib import ExitStack, nullcontext
from pathlib import Path
from unittest.mock import patch

BENCHMARKS = Path(__file__).resolve().parent
sys.path.insert(0, str(BENCHMARKS))
sys.path.insert(0, str(BENCHMARKS / 'cli-tokens-v1'))


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


cli = load('_readiness_cli', BENCHMARKS / 'cli-tokens-v1/codex_campaign.py')
web = load('_readiness_web', BENCHMARKS / 'webapp-tokens-v2/codex_campaign.py')
shift = load('_readiness_shift', BENCHMARKS / 'event-sim-tokens-v1/codex_campaign.py')
common = cli.common


class AccountingReadinessTests(unittest.TestCase):
    def declaration(self, root):
        return {'package': common.TOKENIZER_PACKAGE, 'version': common.TOKENIZER_VERSION,
                'encoding_identity': common.TOKENIZER_ENCODING,
                'tokenizer_dir': str(root / 'tokenizer'), 'fingerprint_sha256': 'a' * 64,
                'claim': 'legacy-Claude tokenizer proxy; not exact current-model or billing tokenization'}

    def test_configured_session_root_preserves_thread_and_cwd_authentication(self):
        thread = '11111111-2222-3333-4444-555555555555'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); workspace = root / 'workspace'; workspace.mkdir()
            configured = root / 'configured'; sessions = configured / 'sessions'; sessions.mkdir(parents=True)
            transcript = sessions / ('rollout-' + thread + '.jsonl')
            transcript.write_text(json.dumps({'type': 'session_meta', 'payload': {
                'id': thread, 'cwd': str(workspace), 'email': 'redacted@example.invalid'}}) + '\n')
            with patch.dict(os.environ, {'CODEX_HOME': str(configured)}), patch.object(Path, 'home', return_value=root / 'other-home'):
                self.assertEqual(common.codex_sessions_root(), sessions.resolve())
                for adapter in (cli, web):
                    destination = root / ('copy-' + adapter.__name__)
                    self.assertEqual(adapter.copy_task_rollout([thread], workspace, destination), destination)
                    self.assertNotIn('redacted@example.invalid', destination.read_text())
                    self.assertIsNone(adapter.copy_task_rollout([thread], root / 'foreign', destination))
                    self.assertIsNone(adapter.copy_task_rollout(['foreign'], workspace, destination))
                    self.assertIsNone(adapter.copy_task_rollout([thread, thread], workspace, destination))
                duplicate = sessions / ('duplicate-' + thread + '.jsonl'); duplicate.write_bytes(transcript.read_bytes())
                self.assertIsNone(cli.copy_task_rollout([thread], workspace, root / 'refused'))
                duplicate.unlink()
                self.assertEqual(web.copy_task_rollout([thread], workspace, root / 'explicit', sessions_root=sessions), root / 'explicit')

    def test_unset_and_empty_codex_home_keep_default_without_environment_mutation(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(Path, 'home', return_value=Path(directory)):
            for configured in (None, ''):
                with patch.dict(os.environ, {}, clear=True):
                    if configured is not None:os.environ['CODEX_HOME'] = configured
                    before = dict(os.environ)
                    self.assertEqual(common.codex_sessions_root(), Path(directory) / '.codex/sessions')
                    self.assertEqual(dict(os.environ), before)

    def test_declared_tokenizer_is_required_and_drift_fails_without_inventing_tokens(self):
        with tempfile.TemporaryDirectory() as directory:
            declaration = self.declaration(Path(directory))
            for missing in (None, {}, {'fingerprint_sha256': 'a' * 64}):
                with self.assertRaisesRegex(ValueError, 'declared pinned'):
                    common.require_authored_source_tokenizer({'authored_source_tokenizer': missing})
            with patch.object(common, 'tokenizer_metadata', return_value=declaration) as metadata:
                common.require_authored_source_tokenizer({'authored_source_tokenizer': declaration})
                metadata.assert_called_once_with(declaration['tokenizer_dir'])
            drift = copy.deepcopy(declaration); drift['fingerprint_sha256'] = 'b' * 64
            with patch.object(common, 'tokenizer_metadata', return_value=drift):
                with self.assertRaisesRegex(ValueError, 'differs from its pinned'):
                    common.require_authored_source_tokenizer({'authored_source_tokenizer': declaration})
            # Offline inventory and calibration do not acquire a scored-launch prerequisite.
            self.assertIsNone(common.authored_source_metrics(Path(directory), None)['total_tokens'])
            self.assertIsNone(common.tokenizer_metadata(None))

    def test_all_scored_routes_refuse_missing_tokenizer_before_workspace_or_dispatch(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory)
            stack.enter_context(patch.object(common, 'require_compiler_binding'))
            stack.enter_context(patch.object(cli, 'require_compiler_build_receipt'))
            stack.enter_context(patch.object(shift.shiftsim, 'require_authoring_eligibility'))
            stack.enter_context(patch.object(cli.legacy.qualification, 'require_settings'))
            worktrees = [stack.enter_context(patch.object(cli.legacy, 'add_seed_worktree')),
                         stack.enter_context(patch.object(web, 'add_seed_worktree')),
                         stack.enter_context(patch.object(common, 'add_seed_worktree'))]
            dispatch = [stack.enter_context(patch.object(a, 'run_codex')) for a in (cli, web, shift.codex)]
            for adapter in (cli, web, shift):
                with self.assertRaisesRegex(ValueError, 'declared pinned'):
                    adapter.launch_trial.__wrapped__(root, root / 'artifacts', 'seed',
                        {'arm': 'semaprax', 'number': 1}, {'authored_source_tokenizer': None}, root / 'compiler')
            for call in worktrees + dispatch:call.assert_not_called()

    def calibration(self, root, invalid):
        settings = {'timeout_seconds': 1, 'codex_binary': 'never-executed', 'authored_source_tokenizer': None}
        observed = {'invalid_stream_lines': invalid, 'thread_ids': ['thread'], 'tool_item_counts': {'agent_message': 1}}
        trace = {'reconciled': True, 'model_observed': cli.MODEL, 'effort_observed': cli.EFFORT, 'model_requests': []}
        def process(command, workspace, env, stream, stderr, timeout):
            stream.write_text(json.dumps({'item': {'type': 'agent_message', 'text': 'READY'}}) + '\n')
            return {'process_exit_code': 0, 'timed_out': False}
        with ExitStack() as stack:
            for owner, name in [(common, 'require_compiler_binding'), (cli, 'require_compiler_build_receipt'),
                                (cli.legacy, 'add_seed_worktree'), (cli, 'cleanup_trial')]:
                stack.enter_context(patch.object(owner, name, return_value=None))
            prerequisite = stack.enter_context(patch.object(common, 'require_authored_source_tokenizer'))
            stack.enter_context(patch.object(cli, 'run_codex', side_effect=process))
            stack.enter_context(patch.object(cli, 'parse_exec_jsonl', return_value=observed))
            stack.enter_context(patch.object(cli, 'copy_task_rollout', return_value=root / 'trace'))
            stack.enter_context(patch.object(cli, 'trace_usage', return_value=trace))
            result = cli.launch_calibration.__wrapped__(root, root / 'artifacts', 'seed', settings, root / 'compiler')
            prerequisite.assert_not_called()
            return result

    def test_loglens_calibration_requires_zero_invalid_stream_lines_but_not_tokenizer(self):
        with tempfile.TemporaryDirectory() as directory:
            for invalid, expected in [(0, 'ready'), (1, 'failed'), (None, 'failed')]:
                row = self.calibration(Path(directory), invalid)
                self.assertEqual(row['status'], expected)
                self.assertEqual(row['telemetry_valid'], invalid == 0)
                self.assertIsNone(row['observed']['stable_context_tokens'])
                self.assertIsNone(row['list_price']['actual_billed_usd'])
                self.assertFalse(row['subtracted_from_trials'])

    def test_loglens_trial_does_not_accept_reconciled_but_malformed_outer_stream(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory);binary = root / 'compiler'
            settings = {'timeout_seconds': 1, 'codex_binary': 'never-executed', 'authored_source_tokenizer': self.declaration(root)}
            for owner, name in [(common, 'require_compiler_binding'), (cli, 'require_compiler_build_receipt'),
                                (cli.legacy.qualification, 'require_settings'), (cli.legacy, 'add_seed_worktree'),
                                (cli, 'cleanup_trial'), (cli.legacy, 'preserve_candidate_source')]:
                stack.enter_context(patch.object(owner, name, return_value=None))
            pinned = stack.enter_context(patch.object(common, 'require_authored_source_tokenizer'))
            stack.enter_context(patch.object(cli.compiler_capture, 'authoring_compiler', return_value=nullcontext(binary)))
            stack.enter_context(patch.object(cli.legacy, 'prompt_for', return_value='unchanged task'))
            stack.enter_context(patch.object(cli.selected_file_setup, 'prompt_note', return_value=''))
            stack.enter_context(patch.object(cli, 'run_codex', return_value={'timed_out': False, 'process_exit_code': 0}))
            stack.enter_context(patch.object(cli, 'parse_exec_jsonl', return_value={'invalid_stream_lines': 1, 'thread_ids': ['thread']}))
            stack.enter_context(patch.object(cli, 'copy_task_rollout', return_value=root / 'trace'))
            stack.enter_context(patch.object(cli, 'trace_usage', return_value={'reconciled': True, 'model_observed': cli.MODEL, 'effort_observed': cli.EFFORT, 'model_requests': []}))
            stack.enter_context(patch.object(cli.legacy, 'authored_source_metrics', return_value={'total_tokens': None}))
            stack.enter_context(patch.object(cli.legacy, 'trial_workspace_guard', return_value=({'status': 'passed'}, None)))
            acceptance = stack.enter_context(patch.object(cli.legacy, 'check_program_for_campaign'))
            row = cli.launch_trial.__wrapped__(root, root / 'artifacts', 'seed', {'arm': 'semaprax', 'number': 1}, settings, binary)
            self.assertEqual(row['status'], 'failed');self.assertFalse(row['telemetry_valid'])
            self.assertIn('invalid', row['failure']);acceptance.assert_not_called()
            self.assertEqual(pinned.call_count, 2)


if __name__ == '__main__':
    unittest.main()
