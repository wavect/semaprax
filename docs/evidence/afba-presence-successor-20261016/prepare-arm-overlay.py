"""Source-only overlay for a future clean chosen common head. Never applies it."""
from pathlib import Path
import argparse,json,subprocess,hashlib,re
A=argparse.ArgumentParser();A.add_argument('--repo',type=Path,required=True);A.add_argument('--expected-head',required=True);A.add_argument('--arm',choices=['runtime','embedded'],required=True);A.add_argument('--out',type=Path,required=True);a=A.parse_args()
R=a.repo.resolve();O=a.out.resolve();D=Path(__file__).resolve().parent
assert str(R).startswith('/Volumes/Semaprax/') and str(O).startswith('/Volumes/Semaprax/')
assert re.fullmatch('[0-9a-f]{40}',a.expected_head)
def git(*args):return subprocess.check_output(['git',*args],cwd=R,text=True).strip()
def sha(b):return hashlib.sha256(b).hexdigest()
assert git('rev-parse','HEAD')==a.expected_head and not git('status','--porcelain=v1','--untracked-files=all')
C=json.loads((D/'arm-contract.json').read_text());p=C['selector_source'];common=(R/p).read_bytes();assert sha(common)==C['common_source_sha256']
assert not O.exists();O.mkdir(parents=True)
old=C['runtime_literal'].encode();new=C['embedded_literal'].encode();assert common.count(old)==1 and new not in common
arm=common if a.arm=='runtime' else common.replace(old,new,1)
assert sha(arm)==C['common_source_sha256'if a.arm=='runtime'else'embedded_source_sha256']
q=O/'overlay'/p;q.parent.mkdir(parents=True);q.write_bytes(arm);(O/'whole-common-runtime-json.rs.txt').write_bytes(common)
# Actual arm metadata must reflect its actual selected module edge and disk hashes.
# Never teach the existing current_source checker to accept stale metadata.
h=(R/'tests/hir_module_boundaries.rs').read_text();w=(R/'tests/workspace/semantic_graph.rs').read_text()
hm=re.findall(r'docs/evidence/([^"\n]+)/hir.json',h);wm=re.findall(r'docs/evidence/([^"\n]+)/projection.json',w);assert len(hm)==len(wm)==1 and hm==wm
G=R/'docs/evidence'/hm[0]/'generate-authority.py';assert G.is_file()
assert sha(G.read_bytes())==sha((D/'generate-authority.py').read_bytes()),'existing authority generator drift requires explicit review'
receipt={'status':'SOURCE_OVERLAY_ONLY_NOT_APPLIED_NOT_FROZEN_NOT_EXECUTED','common_head':a.expected_head,'common_tree':git('rev-parse','HEAD^{tree}'),'arm':a.arm,'source':p,'common_sha256':sha(common),'arm_sha256':sha(arm),'sole_rust_change':None if a.arm=='runtime'else{'new':new.decode(),'old':old.decode()},'authority_generator_sha256':sha(G.read_bytes()),'after_explicit_apply_regenerate_actual_metadata_command':['python3',str(G),'--repo',str(R),'--out',str(G.parent)],'allowed_metadata_changes':[str(G.parent.relative_to(R)/x)for x in ['hir.json','projection.json','authority-source-proof.json']],'next_steps':'Parent explicitly applies overlay only in its own fresh arm worktree; regenerates truthful manifests; commits actual arm; original registration/frozen-source checks bind that exact head. No common source/fixture/old JSON alteration.'}
(O/'overlay-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt,indent=2))
