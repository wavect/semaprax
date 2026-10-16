#!/opt/homebrew/bin/python3
"""Finite whole-input component views; raw audits always retain joined bytes."""
import argparse,hashlib,json,os,pathlib,subprocess
E=pathlib.Path(__file__).resolve().parent;R=E.parents[2]
p=argparse.ArgumentParser();p.add_argument('--views',type=pathlib.Path,required=True);a=p.parse_args()
V=a.views.resolve();assert V.is_relative_to('/Volumes/Semaprax') and not V.is_relative_to(R)
D=json.loads((E/'composition.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
# Exact additive6db predecessor; original scripts/capsules archived in full.
import runpy
compose=runpy.run_path(str(R/'docs/evidence/main-eb2-finite-predecessors-20261016/finite.py'))['compose']

def git(c,path):return subprocess.check_output(['git','show',c+':'+path],cwd=R)
def project(raw,row):
    assert sha(raw)==row['current_sha256'],row['path']
    lines=raw.decode().splitlines(True)
    for edit in reversed(row['edits']):
        i=edit['start'];current=edit['current'].splitlines(True)
        assert lines[i:i+len(current)]==current
        lines[i:i+len(current)]=edit['private'].splitlines(True)
    private=''.join(lines).encode();assert sha(private)==row['private_sha256']
    return private
negative=0;views=[];full_inputs=0;capsule_files=0
common=pathlib.Path(subprocess.check_output(['git','rev-parse','--git-common-dir'],cwd=R,text=True).strip()).resolve();assert common.is_relative_to('/Volumes/Semaprax')
for name,component in D['components'].items():
    view=V/name;view.mkdir(parents=True,exist_ok=True)
    differences=[]
    for row in component['rows']:
        actual=(R/row['path']).read_bytes();actual=compose('next-'+name,row['path'],actual);private=project(actual,row)
        assert private==git(component['private_commit'],row['path'])
        full_inputs+=1
        if row['path'].startswith(component['capsule']+'/'):
            assert not row['edits'],'complete original capsule changed';capsule_files+=1
        if row['edits']:
            differences.append(row['path'])
            for wrong in [b'',actual+b' foreign\n',actual+actual]:
                try:project(wrong,row)
                except AssertionError:negative+=1
                else:raise AssertionError('altered current input accepted')
        dest=view/row['path'];dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(private)
    assert differences==D['physical_private_view_differences'][name]
    script=view/component['script'];assert script.read_bytes()==(R/component['script']).read_bytes()
    command=['/opt/homebrew/bin/python3',str(script)]
    if name=='831':command+=['--repo',str(view)]
    env=dict(os.environ,TMPDIR='/Volumes/Semaprax/runtime-tmp',PYTHONDONTWRITEBYTECODE='1',GIT_DIR=str(common))
    ran=subprocess.run(command,cwd=view,env=env,text=True,capture_output=True)
    (V/f'component-{name}.log').write_text(ran.stdout+ran.stderr)
    assert ran.returncode==0,(name,ran.stderr)
    # A source verifier's own output lives only in this new proof view.
    assert (R/component['script']).read_bytes()==git(component['private_commit'],component['script'])
    views.append({'component':name,'original_script':'PASS_SOURCE_ONLY','full_inputs':len(component['rows']),'finite_differences':len(differences)})
for guard in D['raw_guards']:
    raw=(R/guard).read_text()
    for subject in D['new_rust_subjects']:
        assert raw.count('/'+subject+'"')==1,(guard,subject)
assert len((R/D['raw_guards'][-1]).read_text().splitlines())==1500
registry=D['allow_registry'];registry_input=compose('next-registry',registry['path'],(R/registry['path']).read_bytes());assert sha(registry_input)==registry['current_sha256']
old=json.loads(git(D['base'],registry['path']));new=json.loads(registry_input)
assert new.keys()==old.keys()
for guard in old:
    assert new[guard].keys()==old[guard].keys()
    for policy in old[guard]:
        expected=dict(old[guard][policy]);assert not expected.keys()&set(D['new_rust_subjects'])
        expected.update({s:1 for s in D['new_rust_subjects']});assert new[guard][policy]==expected
for subject in D['new_rust_subjects']:assert len((R/subject).read_text().splitlines())<=1500
for row in D['additive_workspace_registration']:
    actual=(R/row['path']).read_bytes();actual=compose('next-registration',row['path'],actual);assert sha(actual)==row['current_sha256']
    before=(E/row['archive']).read_bytes();assert sha(before)==row['base_sha256'] and before==git(D['base'],row['path'])
    lines=before.decode().splitlines(True);offset=0
    for edit in row['additions']:
        new=edit['text'].splitlines(True);at=edit['line']+offset;lines[at:at]=new;offset+=len(new)
    assert ''.join(lines).encode()==actual
assert 'use semaprax::digest_hex;' in (R/'tests/workspace.rs').read_text()
registration='#[path = "../../src/source_reflows_merge/opt858_sequence_input.rs"]\nmod opt858_sequence_input;'
assert (R/'tests/workspace/semantic_graph.rs').read_text().count(registration)==1
assert (R/'tests/workspace'/'../../src/source_reflows_merge/opt858_sequence_input.rs').resolve()==R/'src/source_reflows_merge/opt858_sequence_input.rs'
assert len((R/'tests/workspace/semantic_graph.rs').read_text().splitlines())<=1500
proof={'status':'PASS_SOURCE_ONLY_FINITE_JOINED_COMPOSITION','base':D['base'],'component_views':views,'complete_inputs':full_inputs,'immutable_capsule_files':capsule_files,'whole_input_rejection_controls':negative,'raw_joined_subjects':len(D['new_rust_subjects']),'raw_join_count':3,'native_inventory_lines':1500,'original_scripts_changed':False,'explicit_workspace_helper_and_digest_registration':True,'overall_authority_or_qualification_refreshed':False,'Rust_execution':'NOT RUN'}
(V/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
