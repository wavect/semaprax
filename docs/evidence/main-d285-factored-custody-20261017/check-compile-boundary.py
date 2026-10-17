#!/usr/bin/env python3
"""Read-only actual Python compile-stage boundary controls; no Rust execution."""
from pathlib import Path
import ast,hashlib,importlib.util,json,tempfile,shutil
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('factored_compile_stage',R/'docs/evidence/compile-acquisition-35f-20261017/stage.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
rows=m.records(R);row=next(r for r in rows if r['source']==m.FACTORED_CORE)
raw=(R/m.FACTORED_CORE).read_bytes();old=(D/'ours-core.before.txt').read_bytes();before=(R/row['before_archive']).read_bytes()
assert m.stage(R,m.FACTORED_CORE,raw)==before
assert m.stage(R,m.FACTORED_CORE,old)==before
assert m.stage(R,m.FACTORED_CORE,before)==before
assert m.stage(R,'foreign',b'unchanged')==b'unchanged'
positives=4;negatives=0
def refuses(call):
 global negatives
 try:call()
 except (AssertionError,OSError,UnicodeError):negatives+=1;return
 raise AssertionError('hostile accepted')
for bad in [b'',raw+b' ',raw+raw,b'!'+raw[1:],b'foreign',raw.replace(b'\n',b'\r\n')]:refuses(lambda:m.stage(R,m.FACTORED_CORE,bad))
# Full original stage and run-common remain finitely reconstructible.
oldstage=(D/'compile-stage.before.py.txt').read_text()
current=(R/'docs/evidence/compile-acquisition-35f-20261017/stage.py').read_text()
prefix=current[:current.index('\n# Exact outer core relocation only.')]
prefix=prefix.replace(" raw=_factored_core_input(repo,source,raw)\n actual=_factored_core_physical(repo,source);assert sha(actual)==row['current_sha256'],'actual new raw compile source'", " actual=(repo/source).read_bytes();assert sha(actual)==row['current_sha256'],'actual new raw compile source'",1)
assert prefix==oldstage
oldrun=(D/'run-common.before.py.txt').read_bytes();newrun=(R/'docs/evidence/compile-acquisition-35f-20261017/run-common.py').read_bytes()
assert newrun.replace(hashlib.sha256(current.encode()).hexdigest().encode(),hashlib.sha256(oldstage.encode()).hexdigest().encode(),1)==oldrun
assert m.sha((R/'docs/evidence/compile-acquisition-35f-20261017/sources.json').read_bytes())==m.DATA_SHA
# Source/metadata drift and filesystem hostiles use only fresh SSD copies.
with tempfile.TemporaryDirectory(prefix='factored-core-controls-',dir='/Volumes/Semaprax/runtime-tmp') as tmp:
 T=Path(tmp);leaf=T/m.FACTORED_CORE;leaf.parent.mkdir(parents=True)
 archive=T/'docs/evidence/main-d285-factored-custody-20261017/ours-core.before.txt';archive.parent.mkdir(parents=True);archive.write_bytes(old)
 leaf.write_bytes(raw);assert m._factored_core_physical(T,m.FACTORED_CORE)==old;positives+=1
 for bad in [raw[:-1],raw+b' ',b'!'+raw[1:],raw.replace(b'\n',b'\r\n')]:
  leaf.write_bytes(bad);refuses(lambda:m._factored_core_physical(T,m.FACTORED_CORE))
 leaf.unlink();refuses(lambda:m._factored_core_physical(T,m.FACTORED_CORE))
 leaf.symlink_to(R/m.FACTORED_CORE);refuses(lambda:m._factored_core_physical(T,m.FACTORED_CORE));leaf.unlink()
 leaf.mkdir();refuses(lambda:m._factored_core_physical(T,m.FACTORED_CORE));leaf.rmdir();leaf.write_bytes(raw)
 archive.write_bytes(old+b'\r\n');refuses(lambda:m._factored_core_physical(T,m.FACTORED_CORE))
 archive.write_bytes(b'!'+old[1:]);refuses(lambda:m._factored_core_physical(T,m.FACTORED_CORE))
 refuses(lambda:m._factored_file(leaf,m.FACTORED_CAP+1,'0'*64))
 # Correct SHA with invalid UTF8 still refuses.
 leaf.write_bytes(b'\xff');refuses(lambda:m._factored_file(leaf,1,m.sha(b'\xff')))
spec=importlib.util.spec_from_file_location('factored_common_dispatch',R/'docs/evidence/compile-acquisition-35f-20261017/run-common.py');dispatch=importlib.util.module_from_spec(spec);spec.loader.exec_module(dispatch)
for tool in ['main','refresh','parts','checks']:
 _,program,proof=dispatch.program(tool,R);assert proof['whole_original_restored'];ast.parse(program)
oldhelper=(D/'main67107_inputs.before.rs.txt').read_bytes();newhelper=(R/'src/source_reflows_merge/main67107_inputs.rs').read_bytes()
prefix=newhelper[:newhelper.index(b'\n// One closed outer acquisition for the original seven-source compile domain.')]
prefix=prefix.replace(b'    let acquired = factored_core_input(source, current);\n    let current = acquired.as_str();\n',b'',1).replace(b'    let raw = factored_core_physical(source);\n    assert_eq!(digest(&raw), row["current_sha256"], "actual new compile subject");',b'    let raw = std::fs::read_to_string(root.join(source)).unwrap();\n    assert_eq!(digest(&raw), row["current_sha256"], "actual new compile subject");',1)
assert prefix==oldhelper
print(json.dumps({'state' :'PASS_SOURCE_ONLY','actual_python_boundary_positive':positives,'actual_python_boundary_negative':negatives,'seven_table_sha_unchanged':True,'old_stage_and_dispatch_finite_inverse':True,'rust':'UNEXECUTED'}))
