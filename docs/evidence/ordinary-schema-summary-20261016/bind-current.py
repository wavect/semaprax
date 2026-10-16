"""Derive complete current endpoints; immutable c228 preimages never change."""
from pathlib import Path
import argparse,difflib,hashlib,json,subprocess
p=argparse.ArgumentParser();p.add_argument('--write-current',action='store_true',required=True);a=p.parse_args()
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();rows=[]
for archived in sorted((D/'before').iterdir()):
 path=archived.name.replace('__','/');before=archived.read_bytes();assert before==subprocess.check_output(['git','show','c228bd6e184a02d7bbb2799b45154d4b4cd404e6:'+path],cwd=R)
 raw=(R/path).read_bytes();left=before.decode().splitlines(True);right=raw.decode().splitlines(True);offsets=[0]
 for line in right:offsets.append(offsets[-1]+len(line))
 inverse=[]
 for op,i,j,u,v in difflib.SequenceMatcher(None,left,right,autojunk=False).get_opcodes():
  if op!='equal':inverse.append(dict(start=offsets[u],end=offsets[v],after=''.join(right[u:v]),before=''.join(left[i:j])))
 rows.append(dict(source=path,archive='before/'+archived.name,before_sha256=sha(before),current_sha256=sha(raw),current_length=len(raw),changed=raw!=before,inverse=inverse))
new=['src/cleanup_plan/replay/schema/original_materializer.rs','src/cleanup_plan/replay/schema/ordinary_tests.rs'];guards=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
value=dict(base='c228bd6e184a02d7bbb2799b45154d4b4cd404e6',rows=rows,new_sources=[dict(source=p,sha256=sha((R/p).read_bytes()),length=len((R/p).read_bytes()))for p in new],raw_guards=guards,extra_raw={'tests/workspace/semantic_graph/source_inventory.rs':['src/cleanup/schema_summary.rs','src/cleanup/schema_summary/generic_record.rs','src/cleanup/schema_summary/support.rs','src/cleanup/schema_summary/tests.rs']},assertion_subjects=[r['source']for r in rows if r['source'].endswith('.rs')and 'assert' in (D/r['archive']).read_text()]);(D/'source-capsule.json').write_text(json.dumps(value,indent=2)+'\n')
