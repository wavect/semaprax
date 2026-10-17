"""Closed two-fixture acquisition before the unchanged35f common algorithms."""
from pathlib import Path
import hashlib,json,types,sys
D=Path(__file__).resolve().parent;R=D.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
DATA_SHA='97332f8c15c3e824634ac248f307bd755ed69319773d10c363b99df4e0e12bae'
RUN_SHA='718df1a7b1de29928ae924725f98078275dcf41685862c9232b2aad327342a78'
STAGE_SHA='8ae05d285bba94107d1073d792938057f6c997f3f9ee9e9d6b52dd525fc67e30'
def fixture_stage(repo,source,raw,read_fixed):
 data=(D/'acquisition.json').read_bytes();assert sha(data)==DATA_SHA
 rows=json.loads(data);assert len(rows)==2
 row=next((r for r in rows if r['source']==source),None)
 if row is None:return raw
 assert 0<=row['current_len']<=262_144 and 0<=row['before_len']<=262_144
 actual=read_fixed(repo/source,row['current_len'],row['current_sha256'])
 before=read_fixed(repo/row['archive'],row['before_len'],row['before_sha256'])
 text=actual.decode()
 for edit in row['inverse']:
  assert text.count(edit['after'])==1;text=text.replace(edit['after'],edit['before'],1)
 assert text.encode()==before
 if raw in (actual,before):return before
 # Only the old seven-row channel may pass its exact older35f intermediate.
 table=(repo/'docs/evidence/compile-acquisition-35f-20261017/sources.json').read_bytes()
 assert sha(table)=='61af497922931df374510a7d8acd4b8691481f960407f1670c4d4e4dc37625fe'
 old=next((r for r in json.loads(table)if r['source']==source),None)
 if old is not None:
  archived=read_fixed(repo/old['before_archive'],38678,old['before_sha256'])
  if raw==archived:return raw
 raise AssertionError('foreign corrected fixture stage')
def render_stage(raw):
 assert sha(raw)==STAGE_SHA;original=raw.decode();text=original
 edits=[
  (' raw=_factored_core_input(repo,source,raw)',' raw=_corrected_fixture_stage(repo,source,raw)\n raw=_factored_core_input(repo,source,raw)'),
  (' if source!=FACTORED_CORE:return(repo/source).read_bytes()',' if source!=FACTORED_CORE:return _corrected_fixture_stage(repo,source,(repo/source).read_bytes())'),
 ]
 for a,b in edits:assert text.count(a)==1;text=text.replace(a,b,1)
 restored=text
 for a,b in reversed(edits):assert restored.count(b)==1;restored=restored.replace(b,a,1)
 assert restored.encode()==raw
 namespace={'__name__':'closed_corrected_fixture_stage'}
 exec(compile(text,'closed35f-stage-with-two-fixtures','exec'),namespace)
 namespace['_corrected_fixture_stage']=lambda repo,source,raw:fixture_stage(repo,source,raw,namespace['_factored_file'])
 return types.SimpleNamespace(stage=namespace['stage'])
def main():
 path=R/'docs/evidence/compile-acquisition-35f-20261017/run-common.py'
 original=path.read_bytes();assert sha(original)==RUN_SHA
 a="spec=importlib.util.spec_from_file_location('compile35f_closed_acquisition',E/'stage.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)"
 b='helper = _render_corrected_fixture_stage(raw)'
 text=original.decode();assert text.count(a)==1;text=text.replace(a,b,1)
 assert text.replace(b,a,1).encode()==original
 exec(compile(text,str(path),'exec'),{'__file__':str(path),'__name__':'__main__','_render_corrected_fixture_stage':render_stage})
if __name__=='__main__':main()
