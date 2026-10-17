"""Focused source/whole-inverse controls; never invokes common refresh or Rust."""
from pathlib import Path
import hashlib,json,runpy,shutil,tempfile,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
P='docs/evidence/alias-fixtures-c874-20261017/run-common-corrected-fixtures.py'
BASE='e7ec963816907ab8866932d319d723b1aea09e2b'
def git(path):return subprocess.check_output(['git','show',BASE+':'+path],cwd=R)
old=(D/'run-common-corrected-fixtures.before.py').read_bytes();assert old==git(P)
for path in ['docs/evidence/compile-acquisition-35f-20261017/'+n for n in ('sources.json','stage.py','run-common.py')]:assert (R/path).read_bytes()==git(path)
ns=runpy.run_path(str(R/P),run_name='closed_source_control_not_main')
stage=ns['render_stage']((R/'docs/evidence/compile-acquisition-35f-20261017/stage.py').read_bytes())
rows=json.loads((R/'docs/evidence/compile-acquisition-35f-20261017/sources.json').read_bytes())
for row in rows:
    key=row['source'];before=(R/row['before_archive']).read_bytes()
    assert stage.stage(R,key,(R/key).read_bytes())==before
    assert stage.stage(R,key,before)==before
helper=ns['m'];target=helper.TARGET;seal=json.loads((D/'seal.json').read_bytes())
reader=stage.stage.__globals__['_factored_file']
actual=(R/target).read_bytes();previous=(R/seal['previous_archive']).read_bytes();before=(R/seal['original35f_archive']).read_bytes()
for value in (actual,previous,before):assert helper.supplied(R,target,value,reader)==(before if value==before else previous)
refusals=0
def refuses(fn):
    global refusals
    try:fn()
    except (AssertionError,OSError,ValueError,UnicodeError):refusals+=1;return
    raise AssertionError('hostile source admitted')
for bad in (b'',b'foreign',actual[:-1],actual+b' ',previous+previous,before+b' '):refuses(lambda bad=bad:helper.supplied(R,target,bad,reader))
assert helper.supplied(R,'foreign',b'unchanged',reader)==b'unchanged'
with tempfile.TemporaryDirectory(prefix='compile35f-ingress-',dir='/Volumes/Semaprax/runtime-tmp') as tmp:
    root=Path(tmp)
    paths=[target,helper.TABLE,str(helper.REL/'seal.json'),seal['current_archive'],seal['previous_archive'],seal['original35f_archive']]
    for path in paths:(root/path).parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/path,root/path)
    assert helper.acquire(root,reader)==(actual,previous,before)
    for path in paths:
        p=root/path;raw=p.read_bytes()
        for bad in (raw[:-1],raw+b' ',b'foreign'):
            p.write_bytes(bad);refuses(lambda:helper.acquire(root,reader))
        p.write_bytes(raw)
        p.unlink();p.symlink_to(R/path);refuses(lambda:helper.acquire(root,reader));p.unlink();p.write_bytes(raw)
    (root/target).write_bytes(previous);refuses(lambda:helper.supplied(root,target,previous,reader));(root/target).write_bytes(actual)
    # Authenticate helper bytes before any imported definition can execute.
    boot=R/P;prefix=boot.read_text().split("m=types.ModuleType",1)[0]
    local=root/'docs/evidence/alias-fixtures-c874-20261017/run-common-corrected-fixtures.py';local.parent.mkdir(parents=True,exist_ok=True)
    helperpath=root/helper.REL/'owned_stage.py';helperpath.write_bytes((D/'owned_stage.py').read_bytes()+b'\nraise RuntimeError("untrusted helper executed")\n')
    refuses(lambda:exec(compile(prefix,str(local),'exec'),{'__file__':str(local)}))
# Restore every original byte in the modified Rust owner with explicit finite edits.
rust='src/source_reflows_merge/main67107_inputs.rs';original=(D/'main67107_inputs.before.rs.txt').read_text();assert original.encode()==git(rust)
current=(R/rust).read_text();appended=current[len(current.split('\n\nconst OWNED_COMPILE35F_PATH:',1)[0]):]
edits=[{'before':'    let source = row["source"].as_str().unwrap();\n    assert!(COMPILE35F_KEYS.binary_search(&source).is_ok());','after':'    let source = row["source"].as_str().unwrap();\n    assert!(COMPILE35F_KEYS.binary_search(&source).is_ok());\n    let owned = owned_compile35f_input(source, current);\n    let current = owned.as_str();'}, {'before':'    if source != FACTORED_CORE_PATH {\n        let raw=std::fs::read_to_string(root.join(source)).unwrap();','after':'    if source == OWNED_COMPILE35F_PATH { return owned_compile35f_input(source, OWNED_COMPILE35F_RAW); }\n    if source != FACTORED_CORE_PATH {\n        let raw=std::fs::read_to_string(root.join(source)).unwrap();'}, {'before':'','after':appended}]
restored=current
for e in reversed(edits):assert restored.count(e['after'])==1;restored=restored.replace(e['after'],e['before'],1)
assert restored==original
(D/'rust-finite-inverse.json').write_text(json.dumps(edits,indent=2)+'\n')
assert len(current.splitlines())<=1500
result={'status':'PASS_SOURCE_ONLY_NO_COMMON_OR_RUST_EXECUTION','known_keys':len(rows),'whole_stage_successes':14,'known_supplied_channels':3,'refusals':refusals,'original_stage_and_seven_table_unchanged':True,'whole_rust_inverse':True,'rust_lines':len(current.splitlines())}
(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
