"""Pure generation and acquisition controls; no tool main or child execution."""
from pathlib import Path
import ast,hashlib,json,runpy
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
P=R/'docs/evidence/alias-fixtures-c874-20261017/run-common-corrected-fixtures.py'
sha=lambda b:hashlib.sha256(b).hexdigest()
row=json.loads((D/'full-public-wrapper-inverse.json').read_bytes())
raw=P.read_bytes();assert len(raw)==row['current_length']and sha(raw)==row['current_sha256']
restored=raw.decode()
for e in row['inverse']:
 assert restored.count(e['after'])==1;restored=restored.replace(e['after'],e['before'],1)
assert restored.encode()==(R/row['before_archive']).read_bytes()and sha(restored.encode())==row['before_sha256']
runner=R/'docs/evidence/compile-acquisition-35f-20261017/run-common.py'
assert runner.read_bytes()==(D/'run-common.original.py.txt').read_bytes()
ns=runpy.run_path(str(P),run_name='pure_import_not_main')
# The original main runs only its generation code. Its final exec is captured.
g=ns['main'].__globals__;captured=[]
g['compile']=lambda text,path,mode:(captured.append(text),compile(text,path,mode))[1]
g['exec']=lambda *args,**kwargs:None
ns['main']();assert len(captured)==1;generated=captured[0]
g.pop('compile');g.pop('exec')
edge=json.loads((D/'child-edit.json').read_bytes())
assert generated.count(edge['new_runner_child_dispatch'])==1
assert edge['old_runner_child_dispatch']not in generated
# Undo only child dispatch and the original loader substitution: whole runner.
a="spec=importlib.util.spec_from_file_location('compile35f_closed_acquisition',E/'stage.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)"
b='helper = _render_corrected_fixture_stage(raw)'
assert generated.replace(edge['new_runner_child_dispatch'],edge['old_runner_child_dispatch'],1).replace(b,a,1).encode()==runner.read_bytes()
ast.parse(generated)
runner_ns={'__file__':str(runner),'__name__':'pure_runner_not_main','_render_corrected_fixture_stage':ns['render_stage']}
exec(compile(generated,str(runner),'exec'),runner_ns)
programs={}
for tool in ('main','refresh','parts','checks'):
 path,text,proof=runner_ns['program'](tool,R);assert proof['whole_original_restored'];ast.parse(text);programs[tool]=text
# Generate the deepest refresh without executing its generated original body.
refresh=R/'docs/evidence/main67107-fronts-6e45-20261017/refresh-composed.py';joined=[]
space={'__file__':str(refresh),'__name__':'__main__','compile35f_stage':runner_ns['compile35f_stage'],
 'compile':lambda text,path,mode:(joined.append(text),compile(text,path,mode))[1],'exec':lambda *a,**k:None}
exec(compile(programs['refresh'],str(refresh),'exec'),space)
assert len(joined)==1
text=joined[0];ast.parse(text)
public="str(R/'docs/evidence/alias-fixtures-c874-20261017/run-common-corrected-fixtures.py')"
assert text.count(public)==1 and "str(R/'docs/evidence/compile-acquisition-35f-20261017/run-common.py')"not in text
# No other tool program gains a dispatch edit; every original proof still runs.
original_ns={'__file__':str(runner),'__name__':'pure_original_runner'}
# Use its already sealed stage adapter, without loading the old raw-only stage.
oldtext=runner.read_text().replace(a,b,1)
original_ns['_render_corrected_fixture_stage']=ns['render_stage']
exec(compile(oldtext,str(runner),'exec'),original_ns)
for tool in ('main','parts','checks'):
 assert original_ns['program'](tool,R)[1]==programs[tool]
# Existing importable acquisition API remains available and covers all seven.
stage=ns['render_stage']((runner.parent/'stage.py').read_bytes())
rows=json.loads((runner.parent/'sources.json').read_bytes())
for r in rows:
 old=(R/r['before_archive']).read_bytes()
 for value in [(R/r['source']).read_bytes(),old]:assert stage.stage(R,r['source'],value)==old
result={'status':'PASS_SOURCE_ONLY_NO_TOOL_MAIN_OR_CHILD_EXECUTION','complete_runner_restored':True,'complete_f030_wrapper_restored':True,'four_original_program_inverses':4,'deepest_refresh_child_public_default_parts':True,'complete_seven_acquisitions':14,'old_runner_stage_table_untouched':True}
(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
