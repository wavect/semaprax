"""Runner-owned retained codec execution, opt-in shared runtime only.

Mailbox/tool arguments request work; only the private runtime channel reports it.
This source does not grant runtime authority, authorship, billing or ratio claims.
"""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path
import secrets

import compiler_input_capture as capture
import compiler_output_provenance as provenance
import compiler_retained_receipt as retained
from compiler_runtime_protocol import Refusal, closed, decode, encode
from compiler_shared_runtime import BACKEND, require_authority

MAX_REQUESTS = 128
MAX_ARGV_BYTES = 16384
MAX_SEAL_BYTES = 512


def publication_seal(raw, nonce, receipt):
    if (type(raw) is not bytes or not raw.endswith(b'\n')
            or len(raw) > MAX_SEAL_BYTES or raw.count(b'\n') != 1):
        raise Refusal('compiler stdout is not one complete bounded seal')
    seal = closed(decode(raw), ['schema', 'nonce', 'receipt_bytes', 'receipt_sha256'])
    if (seal['schema'] != 'semaprax.json-codec-publication-seal.v1'
            or seal['nonce'] != nonce or type(seal['receipt_bytes']) is not int
            or seal['receipt_bytes'] != len(receipt)
            or seal['receipt_sha256'] != 'sha256:' + hashlib.sha256(receipt).hexdigest()):
        raise Refusal('compiler stdout seal and retained artifact differ')
    return seal


def _request(value, authority, candidate):
    closed(value, ['cwd', 'argv'])
    argv, cwd = value['argv'], value['cwd']
    if (type(argv) is not list or not 1 <= len(argv) <= 32
            or any(type(arg) is not str or '\0' in arg for arg in argv)
            or sum(len(os.fsencode(arg)) for arg in argv) > MAX_ARGV_BYTES
            or type(cwd) is not str or cwd != str(authority.workspace_root)):
        raise Refusal('compiler argv/cwd request differs')
    authority.authorize_cwd(cwd)
    # No caller binary, shell, nonce, environment, output seal or policy override.
    values = retained.options(argv)
    if values is None:
        raise Refusal('compiler request is not closed retained codec grammar')
    output = capture._under(candidate, Path(cwd) / values['--output'])
    return tuple(argv), cwd, values, output


def _root_and_evidence(candidate, evidence):
    """Resource setup only; caller must already hold execution/path authority."""
    # Collision refuses before opening a held FD; a failed open leaves no FD.
    evidence.mkdir()
    try:
        return os.open(candidate, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    except BaseException:
        try:
            evidence.rmdir()
        except OSError:
            pass  # Never replace the original acquisition failure.
        raise


class ExecuteBroker:
    def __init__(self, session, candidate: Path, evidence: Path):
        from compiler_shared_runtime import Session
        if type(session) is not Session or not session.initialized or session.closed:
            raise Refusal('trusted runtime session is unavailable')
        require_authority(session.authority)
        self.session = session
        self.authority = session.authority
        self.candidate = Path(os.path.abspath(candidate))
        if str(self.candidate) != str(self.authority.workspace_root):
            raise Refusal('candidate must be the fixed qualified workspace root')
        self.authority.authorize_cwd(str(self.candidate))
        self.authority.check_private_path(evidence)
        self.root_fd = _root_and_evidence(self.candidate, evidence)
        self.evidence = evidence
        self.requests = 0
        self.records = []

    def execute(self, request):
        require_authority(self.authority)
        if self.requests >= MAX_REQUESTS:
            raise Refusal('trusted compiler request limit reached')
        self.requests += 1
        argv, cwd, values, output = _request(request, self.authority, self.candidate)
        identifier = secrets.token_hex(16)
        nonce = secrets.token_hex(32)
        destination = self.evidence / identifier
        destination.mkdir()
        record = {'schema': 'semaprax.runner-compiler-execution.v1', 'backend': BACKEND,
            'request': identifier, 'runtime_contract_sha256': self.authority.contract_sha256,
            'compiler_sha256': self.authority.compiler_sha256,
            'argv': list(argv), 'cwd': cwd, 'nonce': nonce,
            'status': 'not_executed', 'trusted_exit_code': None,
            'execution_independently_authenticated': False,
            'independent_repeat': None, 'generated_source_classification': None,
            'authored_token_subtraction': None, 'ratio_eligible': False,
            'actual_billed_usd': None, 'fixed_context_tokens': None}
        try:
            selected = capture._selection(self.candidate, cwd, values, self.root_fd)
            snapshot, snapshot_sha = provenance.capture_inputs(self.candidate, destination / 'before',
                selected, authorized_directory_fd=self.root_fd)
            before = provenance.validate_input_snapshot(snapshot, snapshot_sha)
            held = {row['path']: row for row in before}
            selected_rows = [held[relative] for relative in selected]
            compiler_argv = [str(self.authority.compiler_binary), *argv, '--receipt-stdout-seal', nonce]
            record['actual_argv'] = compiler_argv
            record['invocation_sha256'] = retained.invocation_sha256(
                str(self.authority.compiler_binary), compiler_argv[1:], cwd)
            record['before_snapshot_sha256'] = snapshot_sha
            # Actual execution occurs only here, once, through the private runtime.
            observation = self.session.execute(compiler_argv, cwd)
            record['trusted_exit_code'] = observation['exit_code']
            record['process_id'] = observation['process_id']
            record['runtime_request_id'] = observation['request_id']
            record['compiler_status_independently_observed'] = True
            record['stdout_sha256'] = hashlib.sha256(observation['stdout']).hexdigest()
            record['stderr_sha256'] = hashlib.sha256(observation['stderr']).hexdigest()
            (destination / 'stdout').write_bytes(observation['stdout'])
            (destination / 'stderr').write_bytes(observation['stderr'])
            if observation['exit_code'] != 0:
                record['status'] = 'compiler_failed'
                return record
            # Fixed-width no-follow read plus seal compares the exact retained bytes,
            # never an agent-supplied finish/status/hash field or later live fallback.
            raw = provenance._snapshot_read(self.root_fd, output, retained.MAX_BYTES)
            if len(raw) > retained.MAX_BYTES:
                raise Refusal('compiler receipt exceeds unchanged host bound')
            seal = publication_seal(observation['stdout'], nonce, raw)
            with (destination / 'compiler-receipt.json').open('xb') as stream:
                stream.write(raw)
            compiler = {'real_binary': str(self.authority.compiler_binary),
                        'binary_sha256': self.authority.compiler_sha256}
            report = validate_trusted_receipt(raw, list(argv), cwd, compiler, selected_rows)
            self.authority.check_current()
            record.update(status='trusted_compiler_receipt_retained',
                execution_independently_authenticated=True, seal=seal,
                receipt_sha256=hashlib.sha256(raw).hexdigest(), structural_report=report)
            return record
        except (ValueError, OSError, UnicodeError) as error:
            record.update(status='execution_or_observation_refused', refusal=str(error)[:1024])
            return record
        finally:
            raw_record = encode(record)
            with (destination / 'execution.json').open('xb') as stream:
                stream.write(raw_record)
            self.records.append({'request': identifier,
                'receipt_sha256': hashlib.sha256(raw_record).hexdigest(), 'status': record['status']})

    def __call__(self, arguments):
        record = self.execute(arguments)
        # Agent can read this response, but cannot write the private runtime or
        # runner receipt. It is never accepted back as a completion proof.
        text = encode({'request': record['request'], 'status': record['status'],
            'exit_code': record['trusted_exit_code'], 'receipt_sha256': record.get('receipt_sha256')}).decode()
        return {'success': record['status'] == 'trusted_compiler_receipt_retained',
                'contentItems': [{'type': 'inputText', 'text': text}]}

    def close(self):
        if self.root_fd is not None:
            os.close(self.root_fd)
            self.root_fd = None

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


# Preserve every v1 structural inventory, bytes, contract and derivation check.
# The old proxy-origin adapter remains unmodified. This separate entry is called
# only after this broker's trusted status/seal/authority boundary has succeeded.
from compiler_retained_receipt import (
    _object, _decode, _string, _integer, _hash, _digest, _manifest_paths, _files,
)

def validate_trusted_receipt(raw, argv, cwd, compiler, selected):
    values = retained.options(argv)
    if values is None:
        raise Refusal('trusted receipt selector differs')
    receipt=_object(_decode(raw),['schema','compiler_package_version','compiler_binary_sha256','repeat_kind',
        'independent_execution','publication_policy','held_inputs','derivations','bundle'])
    if (receipt['schema']!='semaprax.json-codec-retained-receipt.v1' or receipt['compiler_binary_sha256'] is not None
        or receipt['independent_execution'] is not False or receipt['repeat_kind']!='same-process.same-retained-revision.v1'
        or receipt['publication_policy']!='explicit-new-artifact.no-overwrite.v1'):raise ValueError('compiler receipt truth/compatibility labels differ')
    _string(receipt['compiler_package_version'],64)
    inputs=receipt['held_inputs']
    if type(inputs) is not list or not 2<=len(inputs)<=69:raise ValueError('compiler held inventory exceeds bound')
    held=[];total=0
    for value in inputs:
        row=_object(value,['role','path','bytes','sha256','source']);provenance._relative(row['path'],'held input')
        source=_string(row['source']);data=source.encode('utf-8');total+=len(data)
        if _integer(row['bytes'],16*1024*1024)!=len(data) or _hash(row['sha256'])!=_digest(data) or total>16*1024*1024:raise ValueError('compiler original bytes/digest differ')
        held.append(row)
    declared,subjects=_manifest_paths(held[0]['source'])
    expected_inputs=[('manifest','semaprax.toml')]+[('declared_source',p) for p in declared]+[('dependency_subject',p) for p in subjects]
    if [(row['role'],row['path']) for row in held]!=expected_inputs:raise ValueError('compiler held closure differs from complete manifest inventory')
    if values['--source'] not in declared:raise ValueError('receipt schema is not a declared original source')
    if len(selected)!=2:raise ValueError('receipt requires retained selected original pair')
    root=Path(selected[0]['path']).parent
    for row,logical in zip(selected,['semaprax.toml',values['--source']]):
        expected_row=next(value for value in held if value['path']==logical)
        if row['path']!=(root/logical).as_posix() or row['bytes']!=expected_row['bytes'] or row['sha256']!=expected_row['sha256'][7:]:raise ValueError('receipt differs from before-selected original bytes')
    bundle=_object(receipt['bundle'],['schema','authored_project_revision','candidate_project_revision','candidate_workspace_revision','manifest','authored_sources','candidate_sources','entries'])
    if bundle['schema']!='semaprax.json-codec-bundle.v1':raise ValueError('inner compiler bundle schema differs')
    for key in ['authored_project_revision','candidate_project_revision','candidate_workspace_revision']:_string(bundle[key],256)
    if _manifest_paths(_string(bundle['manifest']))!=(declared,subjects):raise ValueError('candidate manifest inventory differs')
    authored=_files(bundle['authored_sources']);candidate=_files(bundle['candidate_sources'])
    if [row['path'] for row in authored]!=[row['path'] for row in candidate]:raise ValueError('candidate source closure differs')
    originals={row['path']:row for row in authored};outputs={row['path']:row for row in candidate}
    if any(outputs[path]!=row for path,row in originals.items() if path!=values['--source']):raise ValueError('unselected candidate source differs from its complete original')
    for row in held[1:1+len(declared)]:
        if row['path'] not in originals or originals[row['path']]['source']!=row['source']:raise ValueError('resolved original source differs from held original')
    if type(bundle['entries']) is not list or len(bundle['entries'])!=1:raise ValueError('single invocation requires one exact entry')
    entry=_object(bundle['entries'][0],['source_path','record_id','profile','max_string_bytes','max_array_items','authored_sha256','candidate_sha256','generated_declarations'])
    if (entry['source_path']!=values['--source'] or entry['record_id']!=values['--type']
        or entry['profile']!=values.get('--profile','flat-scalars.v1')
        or entry['max_string_bytes']!=(int(values['--max-string-bytes']) if '--max-string-bytes' in values else None)
        or entry['max_array_items']!=(int(values['--max-array-items']) if '--max-array-items' in values else None)
        or entry['authored_sha256']!=originals[entry['source_path']]['sha256']
        or entry['candidate_sha256']!=outputs[entry['source_path']]['sha256']):raise ValueError('compiler selection/output binding differs')
    for key in ['max_string_bytes','max_array_items']:
        if entry[key] is not None:_integer(entry[key],256)
    ids=entry['generated_declarations']
    if type(ids) is not list or len(ids)>4096 or any(type(value)is not str for value in ids) or ids!=sorted(set(ids)):raise ValueError('compiler generated identity inventory differs')
    # Reconstruct the documented Rust struct field order, not arbitrary incoming object order.
    bundle['authored_sources']=authored;bundle['candidate_sources']=candidate;bundle['entries']=[entry]
    encoded=json.dumps(bundle,ensure_ascii=False,separators=(',',':')).encode('utf-8')
    if type(receipt['derivations']) is not list or len(receipt['derivations'])!=2:raise ValueError('two compiler derivation records are required')
    for ordinal,value in enumerate(receipt['derivations'],1):
        row=_object(value,['ordinal','project_revision','bundle_bytes','bundle_sha256'])
        if (_integer(row['ordinal'],2)!=ordinal or row['project_revision']!=bundle['authored_project_revision']
            or _integer(row['bundle_bytes'])!=len(encoded) or _hash(row['bundle_sha256'])!=_digest(encoded)):raise ValueError('compiler repeat bytes/revision binding differs')
    def inventory(kind,rows):
        return {'kind':kind,'files':[{'path':row['path'],'bytes':len(row['source'].encode('utf-8')),'sha256':row['sha256'][7:]} for row in rows]}
    return {'schema': 'semaprax.trusted-receipt-structure.v1',
        'status': 'structurally_valid_compiler_report',
        'compiler_package_version_reported': receipt['compiler_package_version'],
        'compiler_reported_held_input_closure_complete': True,
        'compiler_reported_same_revision_repeat_sha256': _digest(encoded),
        'semantic_replay_performed_by_host': False,
        'independent_execution_proof': None, 'input_closure_complete': False,
        'exact_compiler_input_binding': False, 'generated_source_classification': None,
        'authored_token_subtraction': None, 'ratio_eligible': False,
        'actual_billed_usd': None, 'fixed_context_tokens': None,
        'inventories': [inventory('compiler_reported_original_held_inputs', held),
            inventory('compiler_reported_resolved_original_sources', authored),
            inventory('compiler_reported_full_mixed_candidate_sources', candidate)]}


def run_shared_trial(authority, configuration, arm, prompt, candidate, evidence):
    """Explicit new backend entry; callers retain old codex-exec unless selected.

    Both arms use the exact matched configuration. SEM alone receives the codec
    tool, as declared in that configuration. No usage/billing baseline is retargeted.
    """
    from compiler_shared_runtime import Session, matched_configuration
    configuration = matched_configuration(configuration)
    require_authority(authority)
    authority.check_private_path(evidence)
    evidence.mkdir()
    with Session(authority, evidence / 'runtime', configuration) as session:
        thread = session.start_thread(arm)
        if arm == 'semaprax':
            with ExecuteBroker(session, candidate, evidence / 'compiler') as broker:
                result = session.run_turn(prompt, broker)
                result['compiler_requests'] = list(broker.records)
        else:
            result = session.run_turn(prompt, None)
            result['compiler_requests'] = []
        result['thread_start_binding'] = {
            'cwd': thread['cwd'], 'sandbox': thread['sandbox'],
            'runtimeWorkspaceRoots': thread['runtimeWorkspaceRoots'],
            'model': thread['model'], 'reasoningEffort': thread['reasoningEffort']}
    # Publish trial success only after the runner-owned runtime group settles.
    result['runtime_server_group_settled'] = True
    raw = encode(result)
    with (evidence / 'result.json').open('xb') as stream:
        stream.write(raw)
    return result
