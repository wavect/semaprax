"""Future-round qualification; immutable historical rounds retain their gate.

Expected boundary bytes come only from the independently authored SPEC facts.
This module has no model/provider dispatch and never edits the old oracle.
"""
from __future__ import annotations
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
MODULE_PATH = HERE / 'boundary-audit-v1' / 'audit.py'
spec = importlib.util.spec_from_file_location('loglens_boundary_facts_v1', MODULE_PATH)
facts = importlib.util.module_from_spec(spec)
spec.loader.exec_module(facts)
PROFILE = 'loglens-historical33-plus-spec-boundaries.v1'
CASE_TIMEOUT_SECONDS = 120


def metadata() -> dict:
    corpus = json.loads(facts.CORPUS.read_bytes())
    if facts.sha(facts.SPEC.read_bytes()) != corpus['spec_sha256']:
        raise ValueError('boundary qualification SPEC differs from frozen corpus')
    return {'profile': PROFILE, 'historical_checks': 33, 'boundary_checks': 2 * len(corpus['cases']),
            'spec_sha256': corpus['spec_sha256'], 'corpus_sha256': facts.sha(facts.CORPUS.read_bytes()),
            'facts_script_sha256': facts.sha(MODULE_PATH.read_bytes()),
            'qualification_script_sha256': facts.sha(Path(__file__).read_bytes()),
            'case_timeout_seconds': CASE_TIMEOUT_SECONDS,
            'criterion': 'historical build, candidate tests and all33 checks plus all16 independent SPEC boundary checks',
            'execution_mode': 'native or interpreter, as permitted by the unchanged prompt'}


def require_settings(settings: dict) -> None:
    expected = metadata()
    if settings.get('qualification') != expected:
        raise ValueError('future campaign qualification inventory differs from its frozen plan')
    if settings.get('seed_files_sha256', {}).get('benchmarks/cli-tokens-v1/SPEC.md') != expected['spec_sha256']:
        raise ValueError('future campaign seed SPEC differs from boundary qualification')


def check(candidate: Path, arm: str, env: dict[str, str], output: Path,
          settings: dict, historical: dict) -> dict:
    require_settings(settings)
    if output.exists():
        raise ValueError('qualification evidence directory must be new')
    output.mkdir(parents=True)
    corpus_bytes = facts.CORPUS.read_bytes()
    corpus = json.loads(corpus_bytes)
    (output / 'corpus.json').write_bytes(corpus_bytes)
    result = {**historical, 'historical_33_check_accepted': historical.get('accepted') is True,
              'qualification': settings['qualification'], 'boundary_checks': [], 'accepted': False}
    try:
        if historical.get('build', {}).get('status') == 'passed':
            result['execution'] = facts.execution_mode(candidate, arm)
            fixtures = candidate / 'acceptance-fixtures' / 'spec-boundaries-v1'
            fixtures.mkdir(parents=True, exist_ok=True)
            for case in corpus['cases']:
                content, _, _ = facts.materialize(case)
                name = case['name']
                target = fixtures / f'{name}.log'
                target.write_bytes(content)
                (output / f'{name}.input.log').write_bytes(content)
                expected_text, expected_json = facts.expected(case)
                for form, wanted in [('text', expected_text), ('json', expected_json)]:
                    (output / f'{name}.expected.{form}').write_bytes(wanted)
                    arguments = [target.relative_to(candidate).as_posix(), '--top', str(case['top'])]
                    if form == 'json':
                        arguments.append('--json')
                    check = facts.execute(['/bin/sh', str(candidate / 'run.sh'), *arguments],
                        candidate, env, output, f'{name}-{form}', CASE_TIMEOUT_SECONDS)
                    check.update({'name': f'spec-boundary-{name}-{form}', 'case': name, 'form': form,
                        'input_sha256': facts.sha(content), 'input_bytes': len(content),
                        'expected_stdout_sha256': facts.sha(wanted),
                        'seconds': check['elapsed_seconds'],
                        'status': 'timeout' if check['timed_out'] else 'passed' if check['exit_code'] == 0
                        and Path(check['stdout_path']).read_bytes() == wanted else 'failed'})
                    result['boundary_checks'].append(check)
        result['accepted'] = (result['historical_33_check_accepted']
            and len(historical.get('checks', [])) == 33
            and len(result['boundary_checks']) == settings['qualification']['boundary_checks']
            and all(c['status'] == 'passed' for c in result['boundary_checks']))
    except (OSError, ValueError) as error:
        result['qualification_failure'] = str(error)
    # Preserve the original checks separately for historical recounts. New
    # acceptance is exclusively the conjunction above, before source archival.
    (output / 'qualification.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    return result
