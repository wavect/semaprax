"""Finite five-command source/build admission. No compiler, capture or mutation."""
from pathlib import Path
import ast,hashlib,json,re

def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream,'sha256').hexdigest()

def read(path):
    return json.loads(Path(path).read_text())

def literal_assignment(source, name):
    values=[node.value for node in ast.parse(source).body if isinstance(node,ast.Assign)
            and any(isinstance(target,ast.Name) and target.id==name for target in node.targets)]
    assert len(values)==1, ('unique literal assignment required',name)
    return ast.literal_eval(values[0])

def bind_preparation(root, round_path, manifest, preparation):
    root=Path(root).resolve();round_path=Path(round_path).resolve()
    contract_path=round_path/'producer-build-contract.json';contract=read(contract_path)
    assert contract['schema']=='semaprax.focused-batch-producer-build.v1'
    assert Path(contract['repository']).resolve()==root
    assert contract['frozen_source_head']==manifest['source_subject']==preparation['source']
    assert re.fullmatch('[0-9a-f]{40}',manifest['source_subject'])
    assert preparation['manifest_sha256']==sha(round_path/'expected-counts.json')
    assert preparation['driver_sha256']==manifest['prepared_driver_sha256']
    for name,digest in manifest['prepared_driver_sha256'].items():
        assert Path(name).name==name and sha(round_path/'drivers'/name)==digest
    commands=contract['required_ordered_build_commands']
    assert len(commands)==5 and all(isinstance(cmd,list) and cmd for cmd in commands)
    assert contract['producer_subset']==['semaprax::lib','semaprax::workspace','semaprax::production-cli']
    # Run only the manifest-bound pure planner definition, never driver top level.
    driver=(round_path/'drivers/build-owning.py').read_text();tree=ast.parse(driver)
    definitions=[node for node in tree.body if isinstance(node,ast.FunctionDef) and node.name=='planned_commands']
    assert len(definitions)==1
    namespace={'re':re}
    exec(compile(ast.Module(body=definitions,type_ignores=[]),'<manifest-bound-pure-command-planner>','exec'),namespace)
    assert namespace['planned_commands'](manifest,literal_assignment(driver,'rustup'))==commands
    target_relative=literal_assignment((round_path/'drivers/draft_contract.py').read_text(),'TARGET_RELATIVE')
    assert isinstance(target_relative,str) and not Path(target_relative).is_absolute()
    target=(root/target_relative).resolve()
    assert target.is_relative_to(root) and target.is_relative_to((root/'target').resolve()) and target!=(root/'target').resolve()
    return {'schema':'semaprax.current-five-build-binding.v1',
            'producer_contract_sha256':sha(contract_path),
            'preparation_receipt_sha256':sha(round_path/'preparation-receipt.json'),
            'manifest_sha256':sha(round_path/'expected-counts.json'),
            'driver_sha256':manifest['prepared_driver_sha256']['build-owning.py'],
            'draft_contract_sha256':manifest['prepared_driver_sha256']['draft_contract.py'],
            'private_target':str(target),'required_ordered_build_commands':commands}

def admit_actual_build(record, manifest, build, round_path):
    round_path=Path(round_path).resolve();root=Path(record['repository']).resolve()
    assert round_path==Path(record['required_round']).resolve()
    binding=record['current_five_build']
    assert binding==bind_preparation(root,round_path,manifest,read(round_path/'preparation-receipt.json'))
    assert binding['manifest_sha256']==record['required_manifest_sha256']
    assert manifest['source_subject']==build['source_subject']==record['frozen_source_head']
    assert manifest['source_tree']==build['source_tree']==record['frozen_source_tree']
    assert build['schema']=='semaprax.owning-build-bound.v3'
    commands=binding['required_ordered_build_commands']
    assert len(commands)==5 and commands==record['required_ordered_build_commands']==build['ordered_commands']
    assert len(build['results'])==5 and [row['command'] for row in build['results']]==commands
    assert build['exit_code']==0 and not build.get('artifact_refusals') and not build.get('resource_refusal')
    assert build.get('source_binding_status','unchanged')=='unchanged'
    for row in build['results']:
        assert row['executed'] and row['exit_code']==0 and not row.get('resource_refusal')
        assert not row.get('observation_errors') and row['source_binding_status']=='unchanged'
        assert row.get('group_settlement',{}).get('confirmed_empty')
        assert not row.get('timed_out')
    assert build['manifest_sha256']==binding['manifest_sha256']
    assert build['driver_sha256']==binding['driver_sha256']
    assert build['private_target_dir']==binding['private_target']
    assert build['cargo_build_jobs']==1 and build['executed_test_bodies']==0
    assert build['owning_build_metadata_sha256']==sha(round_path/'owning-build.jsonl')
    assert build['log_sha256']==sha(round_path/'owning-build.log')
    assert build['cargo_lock_sha256']==sha(root/'Cargo.lock')
    assert build['memory_samples_sha256']==sha(round_path/'memory-samples.jsonl')
    return Path(binding['private_target'])

def admit_artifact_roles(found, cli, record, build, receipt_sha, metadata_sha):
    assert set(found)=={'semaprax','workspace'} and set(cli)=={'semaprax','semaprax-full'}
    target=Path(record['current_five_build']['private_target']).resolve()
    built={str(Path(row['executable']).resolve()):row['executable_sha256'] for row in build['artifacts']}
    for group,is_test in [(found,True),(cli,False)]:
        for role,row in group.items():
            expected_package='semaprax-toolchain' if role=='semaprax-full' else 'semaprax'
            expected_kind=['test'] if role=='workspace' else ['lib'] if is_test else ['bin']
            assert row['package']==expected_package and row['target']['name']==role
            assert row['target']['kind']==expected_kind and row['profile_test'] is is_test
            path=Path(row['executable']).resolve()
            assert path.is_relative_to(target) and path.is_file()
            assert re.fullmatch('[0-9a-f]{64}',row['executable_sha256'])
            assert sha(path)==row['executable_sha256']
            assert row['source_head']==record['frozen_source_head']
            assert row['raw_Cargo_log_sha256']==metadata_sha
            assert row['completed_build_receipt_sha256']==receipt_sha
            if is_test:assert built[str(path)]==row['executable_sha256']
