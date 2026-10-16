"""Actual raw13 audit plus explicit physical private-source replay; no Rust execution."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess,tempfile,sys
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);p.add_argument('--write-endpoints',action='store_true');a=p.parse_args();R=a.repo.resolve();D=Path(__file__).resolve().parent;PRIVATE='96a49042bf7b204635f8ddef360254948fb63b49';P='docs/evidence/skeleton-constructor-funding-856-20261016';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=R)
# Exact additive6db predecessor; original scripts/capsules archived in full.
import runpy
compose=runpy.run_path(str(R/'docs/evidence/main-eb2-finite-predecessors-20261016/finite.py'))['compose']

def endpoint(path,value):
 raw=(json.dumps(value,indent=2)+'\n').encode()
 if a.write_endpoints:path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw)
 else:assert path.read_bytes()==raw,('stale joined856 binding',path)
metadata=subprocess.check_output(['git','ls-tree','-r','--name-only',PRIVATE,'--',P],cwd=R).decode().splitlines()
for f in metadata:assert(R/f).read_bytes()==git(PRIVATE,f),('private856 packet changed',f)
manifest=json.loads((R/P/'construction.json').read_text());promoted=json.loads((R/P/'promoted-modules.json').read_text());assert len(promoted)==13
# This audit ALWAYS reads actual complete raw guards, before any historical projection.
guards=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
for f in guards:
 literals=re.findall(r'include_str!\(\s*"([^"]+)"',(R/f).read_text())
 for q in promoted:assert any(s.endswith('/'+q)for s in literals),(f,q)
for f in promoted:assert compose('856-promoted',f,(R/f).read_bytes())==git(PRIVATE,f),('promoted production module drift',f)
unchanged=['src/cleanup_plan/replay/sequence_prefix.rs','src/cleanup_plan/replay/sequence_prefix/before.rs','src/cleanup_plan/replay/sequence_prefix/main_current.rs','src/cleanup_plan/replay/sequence_prefix/main24/helper.rs','src/cleanup_plan/replay/sequence_prefix/main_c11/helper.rs','src/cleanup_plan/replay/allocation.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay.rs','src/bounded_output.rs','src/bounded_output/checked_string.rs','src/hir/nodes/instance_key.rs','src/cleanup_plan/replay/skeleton_work/oracle_entry.rs']
files=set(metadata)|set(manifest['preexisting_paths'])|set(promoted)|set(guards)|set(unchanged)|{'src/hir/lexical_exit/owner_funded.rs','src/hir/funded.rs'}
reviewed_additional={'docs/ARCHITECTURE.md': {'commit': '1f1521e2d4f07ae06e4d1a1d90dbcbdbe92a50f3', 'sha256': '1057d4b0aabf76841f2c5c6b4e061c360f6ff221977e8060930b78480f159305'}, 'src/cleanup_plan/replay.rs': {'commit': '1f1521e2d4f07ae06e4d1a1d90dbcbdbe92a50f3', 'sha256': '0cf35a330c89ec7ae532f3ac39ecc4cb0b056cb9538966962ac04851c3a611bc'}, 'src/cleanup_plan/replay/physical_budget.rs': {'commit': 'd96247b3cb69282c3d1b835b2428779e8bde82c7', 'sha256': '95c57d360e5a501cb73d400f8991d6f51e139fc1adeb5b4fb8c62e831b4f7dda'}}

reviewed_main={f:{'commit':'6db5964817b0c3e2a368074995c32fe00b5228fa','sha256':sha(git('6db5964817b0c3e2a368074995c32fe00b5228fa',f))}for f in ['docs/ARCHITECTURE.md','src/cleanup_plan/replay.rs','src/cleanup_plan/replay/physical_budget.rs']}
for f,row in reviewed_additional.items():
 actual=(R/f).read_bytes();choices=[row,reviewed_main[f]];assert any(actual==git(x['commit'],f) and sha(actual)==x['sha256']for x in choices),('reviewed joined input drift',f)
allowed={'CHANGELOG.md',*guards,'src/bounded_output.rs','src/bounded_output/checked_string.rs','src/hir/nodes/instance_key.rs',*reviewed_additional,*[r['path']for r in json.loads((R/'docs/evidence/main-eb2-finite-predecessors-20261016/extensions.json').read_text())['rows']if r['domain']=='856-promoted']};bindings=[];privateviews={}
for f in sorted(files-set(metadata)):
 actual=(R/f).read_bytes();private=git(PRIVATE,f)
 if actual!=private:
  assert f in allowed,('unreviewed private856 overlap',f)
  row=dict(source=f,current_sha256=sha(actual),private_sha256=sha(private),current=actual.decode(),private=private.decode())
  assert row['current'].count(row['current'])==1;assert sha(row['private'].encode())==row['private_sha256'];bindings.append(row);privateviews[f]=private
 else:privateviews[f]=actual
endpoint(D/'joined856-current-to-private.json',bindings)
# Physical finite source projection. No Path hooks, raw-source filtering, compiler, or copied authority grant.
with tempfile.TemporaryDirectory(prefix='main8a-856-private-source-',dir='/Volumes/Semaprax/runtime-tmp')as temporary:
 stage=Path(temporary)
 for f in sorted(files):
  out=stage/f;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(privateviews.get(f,(R/f).read_bytes()))
 gitdir=subprocess.check_output(['git','rev-parse','--absolute-git-dir'],cwd=R,text=True).strip();env={**os.environ,'GIT_DIR':gitdir,'GIT_WORK_TREE':str(stage),'PYTHONDONTWRITEBYTECODE':'1','TMPDIR':'/Volumes/Semaprax/runtime-tmp'}
 result=subprocess.check_output([sys.executable,str(stage/P/'verify-source.py'),'--repo',str(stage)],cwd=stage,env=env)
 proof=json.loads(result);assert proof['status']=='SOURCE_ONLY_CONSTRUCTOR_FUNDING_CUSTODY_PASS'
endpoint(D/'joined856-source-proof.json',dict(status='PASS_JOINED856_SOURCE_ONLY',actual_raw_guards=3,actual_promoted_modules=13,explicit_private_views=[r['source']for r in bindings],unchanged_private_verifier_result=proof,physical_projection_only=True,rust_executed=False))
print(json.dumps(dict(status='PASS_JOINED856_SOURCE_ONLY',actual_raw_promoted_modules=13,explicit_private_views=len(bindings),rust_executed=False)))
