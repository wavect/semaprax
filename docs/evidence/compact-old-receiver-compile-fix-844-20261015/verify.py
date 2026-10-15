"""Finite full-source custody replay; never invokes Rust."""
from pathlib import Path
import contextlib, hashlib, io, json, subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
BASE='677e658f6f3671fdcc5942c1f3522598d83ab7b2'
h=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-677.json').read_text());assert len(rows)==4
named={}
for row in rows:
 p=R/row['source'];raw=p.read_bytes();old=row['edits'][0]['old'].encode()
 assert raw==row['edits'][0]['new'].encode() and h(raw)==row['current_sha256']
 assert h(old)==row['original_sha256'] and old==(R/row['archive']).read_bytes()
 assert old==subprocess.check_output(['git','show',BASE+':'+row['source']],cwd=R)
 named[p.resolve()]=old
prefix='src/cleanup_plan/replay/factored/'
borrow=(R/prefix/'compact_nodes/before/borrowed.rs').read_text()
oldborrow=named[(R/prefix/'compact_nodes/before/borrowed.rs').resolve()].decode()
original=(R/prefix/'borrowed_canonical.rs').read_text();grow=original[original.index('fn grow<T>('):]
assert borrow==oldborrow+'\n'+grow, 'all prior receiver bytes plus complete original independent grow'
root=(R/prefix/'compact_nodes/before/mod.rs').read_text();oldroot=named[(R/prefix/'compact_nodes/before/mod.rs').resolve()].decode()
original=(R/'src/cleanup_plan/replay/factored.rs').read_text();start=original.index('#[cfg(test)]\nenum CanonicalFrame');frame=original[start:original.index('\n\n',start)]
assert root==oldroot.replace('use super::super::{copy_regions, dag_memo, oracle_hook};','use super::super::{dag_memo, oracle_hook};').replace('#[path = "../tests.rs"]',frame+'\n\n#[path = "../tests.rs"]')
p=R/prefix/'compact_nodes.rs';assert p.read_text()==named[p.resolve()].decode().replace('mod before;','pub(super) mod before;'), 'only cfg-test module visibility'
assert '#[cfg(test)]\n#[path = "compact_nodes/before/mod.rs"]\npub(super) mod before;' in p.read_text()
# Every old packet, assertion body and independently compiled old canonical stays exact.
for packet in ['compact-automaton-844','compact-automaton-844-immediate7a87','ready-main-844-846-836-assembly-20261015']:
 for name in subprocess.check_output(['git','ls-tree','-r','--name-only',BASE,'--','docs/evidence/'+packet],cwd=R,text=True).splitlines():
  assert (R/name).read_bytes()==subprocess.check_output(['git','show',BASE+':'+name],cwd=R),name
for path in [prefix+'compact_nodes/tests.rs',prefix+'compact_nodes/before/canonical.rs',prefix+'compact_nodes/before/storage.rs',prefix+'compact_nodes/before/strategy.rs']:
 assert (R/path).read_bytes()==subprocess.check_output(['git','show',BASE+':'+path],cwd=R)
rb,rt=Path.read_bytes,Path.read_text
script=R/'docs/evidence/ready-main-844-846-836-assembly-20261015/verify.py'
code=script.read_text();out=io.StringIO()
def read_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
def read_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
try:
 Path.read_bytes=read_bytes;Path.read_text=read_text
 with contextlib.redirect_stdout(out):exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
finally:Path.read_bytes=rb;Path.read_text=rt
result={'status':'SOURCE_ONLY_NOT_EXECUTABLY_QUALIFIED','complete_current_to_677_inputs':len(rows),'new_test_bodies':0,'old_receiver_definitions_restored':['grow<T>','CanonicalFrame'],'unchanged_677_assembly_replay':json.loads(out.getvalue()),'actual_failure_json_sha256':h((D/'actual-677-compile-errors.json').read_bytes()),'rust_execution':False}
print(json.dumps(result,indent=2))
