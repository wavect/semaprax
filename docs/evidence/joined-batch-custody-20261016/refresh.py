"""Distinct joined source-only refresh. Historical receipts and algorithms remain immutable."""
from pathlib import Path
import argparse,difflib,hashlib,json,os,re,subprocess
p=argparse.ArgumentParser();p.add_argument('--apply',action='store_true');p.add_argument('--expected-head',required=True);a=p.parse_args();assert a.apply
R=Path(__file__).resolve().parents[3];D=R/'docs/evidence/a3-e47-final-custody-20261016';O=Path(__file__).resolve().parent
BASE='a82e00cc83f99b16249181c45710d962598577db';SOURCE='1746eaaf0';sha=lambda b:hashlib.sha256(b).hexdigest()
def g(*args):return subprocess.check_output(['git',*args],cwd=R)
def old(commit,path):return g('show',commit+':'+path)
def load(path):return json.loads(path.read_text())
def write(path,data):path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(data,indent=2)+'\n')
def inverse(before,after):
 x,y=before.decode(),after.decode();xx,yy=x.splitlines(True),y.splitlines(True);edits=[]
 for op,i,j,u,v in difflib.SequenceMatcher(None,xx,yy,autojunk=False).get_opcodes():
  if op=='equal':continue
  a,b=''.join(xx[i:j]),''.join(yy[u:v])
  if not b or y.count(b)!=1:return [{'new':y,'old':x}]
  edits.append({'new':b,'old':a})
 result=y
 for e in edits:
  if result.count(e['new'])!=1:return [{'new':y,'old':x}]
  result=result.replace(e['new'],e['old'],1)
 return edits if result==x else [{'new':y,'old':x}]
assert os.environ.get('TMPDIR')=='/Volumes/Semaprax/runtime-tmp'
assert g('rev-parse','HEAD').decode().strip()==a.expected_head
# Preserve all historical archives, tables, recipes and receipts exactly from the clean joined source.
protected={}
for prefix in ['docs/evidence/maina3-independent-custody-20261016','docs/evidence/maine47-independent-custody-20261016','docs/evidence/main8a-independent-custody-20261016','docs/evidence/main043-independent-custody-20261016/incoming']:
 for f in g('ls-tree','-r','--name-only','HEAD','--',prefix).decode().splitlines():protected[f]=sha((R/f).read_bytes())
for f in ['joined-refresh-receipt.json','postpin-refresh-receipt.json','generate-authority.py','verify-source.py','verify-authority.py','prepare-arm-overlay.py']:
 path=D/f;protected[str(path.relative_to(R))]=sha(path.read_bytes())
baseline=O/'protected-baseline.json'
if baseline.exists():
 assert load(baseline)['files']==protected
else:write(baseline,{'source':g('rev-parse','HEAD').decode().strip(),'files':protected})
# Main043 is first; its original verifier asserts complete old guards and all75 raw inputs.
main043=subprocess.check_output(['python3',str(R/'docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py'),'--repo',str(R),'--write-endpoints'],cwd=R);(O/'main043-proof.json').write_bytes(main043)
main44=subprocess.check_output(['python3',str(R/'docs/evidence/main44-independent-custody-20261016/refresh-and-prove.py'),'--refresh'],cwd=R);(R/'docs/evidence/main44-independent-custody-20261016/static-proof.json').write_bytes(main44);(O/'main44-proof.json').write_bytes(main44)
# Extend only the predecessor overlay: each added key existed in16c and changed in this batch.
domains=json.loads(old(SOURCE,'docs/evidence/a3-e47-final-custody-20261016/domains.json'));assert [len(d['keys'])for d in domains]==[53,30,2,2]
base=domains[0]['commit'];basefiles=set(g('ls-tree','-r','--name-only',base).decode().splitlines());changes=g('diff','--name-only',BASE,'HEAD').decode().splitlines()
add=sorted(p for p in changes if p in basefiles and not p.startswith('docs/evidence/') and p not in domains[0]['keys'])
assert len(add)>=24;domains[0]['keys']=sorted(set(domains[0]['keys'])|set(add));write(D/'domains.json',domains)
normalizer=R/'src/source_reflows_merge/maina3_custody.rs';code=normalizer.read_text();originalcode=code
for d in domains:
 rows=[]
 for f in d['keys']:
  before=old(d['commit'],f);raw=(R/f).read_bytes();archive=D/'before'/d['name']/f
  if archive.exists():assert archive.read_bytes()==before
  else:archive.parent.mkdir(parents=True,exist_ok=True);archive.write_bytes(before)
  rows.append({'source':f,'current_sha256':sha(raw),'original_sha256':sha(before),'edits':inverse(before,raw)})
 write(D/d['table'],rows);payload=(D/d['table']).read_bytes();assert len(payload)<=5081115
 code,n=re.subn(r'(Self::'+d['name']+r' => &)(\[[^\n]*\])(,\n)',lambda m:m[1]+json.dumps(d['keys'])+m[3],code);assert n==1
 code,n=re.subn(r'(Self::'+d['name']+r' => Spec \{path:.*?length: )\d+(, sha256: )\[[^\]]+\]',lambda m:m[1]+str(len(payload))+m[2]+str(list(hashlib.sha256(payload).digest())),code);assert n==1
contract=load(D/'arm-contract.json');selector=(R/contract['selector_source']).read_bytes();embedded=selector.replace(contract['runtime_literal'].encode(),contract['embedded_literal'].encode(),1)
head,tail=code.split('pub(super) fn normalize(',1)
for k,raw,count in [('common_source_sha256',selector,2),('embedded_source_sha256',embedded,1)]:
 before=str(list(bytes.fromhex(contract[k])));assert tail.count(before)==count;tail=tail.replace(before,str(list(hashlib.sha256(raw).digest())));contract[k]=sha(raw)
code=head+'pub(super) fn normalize('+tail
assert code.split('#[derive(Clone, Copy)]\npub(super) enum Input',1)[0]==originalcode.split('#[derive(Clone, Copy)]\npub(super) enum Input',1)[0]
normalizer.write_text(code)
# Rebind the same four original3220 endpoint views, keeping every original archive and predecessor.
C=R/'docs/evidence/compile-corrections-3220-20261016';correction=load(C/'source-corrections.json')
for row in correction['rows']:
 before=old(correction['base'],row['source']);assert sha(before)==row['before_sha256'];assert(R/row['archive']).read_bytes()==before;raw=(R/row['source']).read_bytes();row['current_sha256']=sha(raw);row['edits']=inverse(before,raw)
correction['scope']='Separate joined-batch20261016 complete four-endpoint source custody; no runtime qualification';write(C/'source-corrections.json',correction)
# Refresh current source fingerprints for complete new owners; every one must be present in all three raw joins.
guards=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs'];tracked=set(g('ls-files').decode().splitlines());owners=sorted(f for f in tracked-basefiles if f.endswith('.rs')and(f.startswith('src/')or f.startswith('tests/wasm/')))
assert len(owners)>=70
for f in owners:
 assert len((R/f).read_bytes().splitlines())<=1500
 for q in guards:assert f in (R/q).read_text(),(q,f)
write(D/'new-owners.json',[dict(source=f,sha256=sha((R/f).read_bytes()),lines=len((R/f).read_bytes().splitlines()))for f in owners])
contract.update(changed_preexisting=domains[0]['keys'],final_normalizer_sha256=sha(normalizer.read_bytes()),joined_batch_source_commit=g('rev-parse','HEAD').decode().strip(),execution='NONE_SOURCE_ONLY');write(D/'arm-contract.json',contract)
# All Rust source writes are complete. Generate authority exactly once, then run untouched source proof owners.
subprocess.run(['python3',str(D/'generate-authority.py'),'--repo',str(R),'--out',str(D)],cwd=R,check=True)
source=subprocess.check_output(['python3',str(D/'verify-source.py'),'--repo',str(R)],cwd=R);(D/'source-proof.json').write_bytes(source);(O/'source-proof.json').write_bytes(source)
authority=subprocess.check_output(['python3',str(D/'verify-authority.py')],cwd=R);(O/'authority-proof.json').write_bytes(authority)
for f,digest in protected.items():assert sha((R/f).read_bytes())==digest,('protected baseline changed',f)
# Main043/Main44 remain bound after final descriptor writes.
subprocess.run(['python3',str(R/'docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py'),'--repo',str(R)],cwd=R,check=True)
subprocess.run(['python3',str(R/'docs/evidence/main44-independent-custody-20261016/refresh-and-prove.py')],cwd=R,check=True)
write(O/'receipt.json',dict(status='PASS_JOINED_SOURCE_AND_AUTHORITY_ONLY',source_commit=g('rev-parse','HEAD').decode().strip(),domain_counts={d['name']:len(d['keys'])for d in domains},previous16c_added_keys=add,new_owners=len(owners),authority_generator_runs=1,source_proof_sha256=sha(source),authority_proof_sha256=sha(authority),guard_sha256={f:sha((R/f).read_bytes())for f in guards},all_six_original_static_negatives_refused=True,rust_executed=False,qualification='PENDING'))
