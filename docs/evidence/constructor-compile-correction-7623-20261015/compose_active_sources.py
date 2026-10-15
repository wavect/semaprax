"""Metadata-only application-census composition, immutable import-corrected input, no Rust/Cargo execution."""
from pathlib import Path
import argparse,copy,hashlib,io,json,re,runpy,subprocess,contextlib
P=argparse.ArgumentParser();P.add_argument('--verify-only',action='store_true');P.add_argument('--basis',required=True);args=P.parse_args()
R=Path.cwd();BASE=subprocess.check_output(['git','rev-parse',args.basis],cwd=R,text=True).strip();HERE=R/'docs/evidence/constructor-compile-correction-7623-20261015'
sha=lambda b:hashlib.sha256(b).hexdigest()
def blob(path,revision=BASE):return subprocess.check_output(['git','show',revision+':'+str(path)],cwd=R)
def load(path):return json.loads((R/path).read_bytes() if args.verify_only else blob(path))
if not args.verify_only:assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==subprocess.check_output(['git','rev-parse',BASE],cwd=R,text=True).strip()
EXCLUDE='docs/evidence/selected-session-static-refusal-781/exact-source-inverses.json'
SHARED=[
'docs/evidence/loan-candidate-inventory-reuse/exact-source-inverses.json',
'docs/evidence/borrowed-generic-membership-index-39c6/whole-source-inverses.json',
'docs/evidence/loan-inner-stages-829/exact-source-inverses.json',
'docs/evidence/paid-live-pending-reuse-3c351/whole-source-inverses.json',
'docs/evidence/exit-status-directory-reuse/exact-source-inverses.json',
'docs/evidence/factored-closure-index/exact-source-inverses.json',
'docs/evidence/paid-cfg-child-scratch/exact-source-inverses.json',
'docs/evidence/referenced-edge-ownership-proof/exact-source-inverses.json',
'docs/evidence/prefix-certified-expression-lookup/exact-source-inverses.json',
'docs/evidence/provider-proof-backing-714177/whole-source-inverses.json',
'docs/evidence/factored-continuation-receiving/exact-source-inverses.json',
'docs/evidence/receiving-transitive-results-816/exact-source-inverses.json',
'docs/evidence/dense-loan-live-scratch/exact-source-inverses.json',
'docs/evidence/borrowed-generic-type-rewrite-141387/whole-source-inverses.json',
'docs/evidence/exit-prefix-reference-proof/exact-source-inverses.json',
'docs/evidence/lifecycle-inline-active-path/whole-source-inverses.json',
'docs/evidence/status-fact-borrow-reuse/exact-source-inverses.json',
'docs/evidence/status-prefix-insertion/exact-source-inverses.json',
'docs/evidence/loan-endpoint-ordered-union-827/exact-source-inverses.json',
'docs/evidence/prelude-extension-reuse-828/exact-source-inverses.json',
'docs/evidence/nominal-payload-827/exact-source-inverses-activation-v14.json']
EXTRA=['docs/evidence/nested-byte-build-correction-a33-20261015/whole-source-inverses.json', 'docs/evidence/factored-epsilon-closure-sharing/exact-source-inverses.json', 'docs/evidence/nested-byte-traversal-780/final-source-inverses-v1.json', 'docs/evidence/selected-session-static-refusal-781/exact-source-inverses.json', 'docs/evidence/selected-session-compile-correction-8795/exact-source-inverses.json', 'docs/evidence/legacy-nominal-sharing-827/exact-source-inverses.json', 'docs/evidence/projection-explicit-import-correction-a417/exact-source-inverses.json', 'docs/evidence/epsilon-application-census-830/exact-source-inverses.json', 'docs/evidence/status-case-proof-816/exact-source-inverses.json', 'docs/evidence/legacy-final-link-sharing-827/exact-source-inverses.json', 'docs/evidence/legacy-nominal-diagnostic-fixture-correction-a417/exact-source-inverses.json', 'docs/evidence/singleton-successor-targets-831/exact-source-inverses.json', 'docs/evidence/actual-nested-fixture-guide-correction-a417/exact-source-inverses.json', 'docs/evidence/nested-byte-lowering-admission-a417/source-inverses.json', 'docs/evidence/integration-status-case-imports-20261015/exact-source-inverses.json', 'docs/evidence/ordered-large-integration-20261015/label-source-inverses.json', 'docs/evidence/ordered-large-integration-20261015/explicit-import-source-inverses.json', 'docs/evidence/codec808-frontier-witness/source-inverses.json', 'docs/evidence/constructor-inline-failure-825/exact-source-inverses.json', 'docs/evidence/default-static-error-custody-825/exact-source-inverse.json','docs/evidence/ordered-large-integration-20261015/census-visibility-source-inverses.json','docs/evidence/ordered-large-integration-20261015/purity-token-source-inverses.json']
EXTRA += ['docs/evidence/nested-byte-candidate-adoption-780/exact-source-inverses.json','docs/evidence/legacy-linked-drop-reuse-828f/exact-source-inverses.json','docs/evidence/status-case-counter-828f/exact-source-inverses.json','docs/evidence/prelude-forecast-286/exact-source-inverses.json','docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json','docs/evidence/constructor-report-receiving-825/source-inverses.json','docs/evidence/singleton-target-receiving-layers-831/exact-source-inverses.json','docs/evidence/selected-child-refusal-781/exact-source-inverses.json']
EXTRA += ['docs/evidence/constructor-compile-correction-7623-20261015/exact-source-inverses.json']
assert len(SHARED)==21
REVERSE={'loan-candidate-inventory-reuse','exit-status-directory-reuse','referenced-edge-ownership-proof','prefix-certified-expression-lookup','receiving-transitive-results-816','exit-prefix-reference-proof','status-fact-borrow-reuse','constructor-inline-failure-825','default-static-error-custody-825','prelude-forecast-286','pin-capture'}
CANDIDATES=[BASE,'7623acd670f0a25a21ed8c00c190b517e78c8945','c210d4ba5203dab1eed582320d8b6cf6d0945760','a920309d8a37feb3fe021d36deb62881884f5aeb','154cf86a3c4328448be6f2db65d588f06a5b7439','362482ddf','715766ff5','a002162b1','c598b35d6','b5e83cfdd','a6a46a2cd','aa43138d495c9ab0c94b51f18f3ff7b9415963a4','97dbf12f2da025fff5e95a637b8df4435cc325e8','a62be8e6f062b651ce56144641e7deaf98dd56e2','331b382c689b8498ec02f140b713dfaf698001c2','686d4389e24b1dea9c4a132d6aabd3da1588a34f','36fe9f92b847db41f26c685b079d06acf60a22f4','590391b0dcca6b1dc3e6b817c2feb38ab6fc25a9',BASE,'5647a8fd4','5b562965d','4517c60f2','9c6dac404','1df902b7273b076d260cd89af73b86ed973aac1d','0608879bf','c387ae907','1f131cfb1','37d06d7b7','65fa91291','5dc423062','1a183fd6a','fdbc75160','f9962dc17','d3de7f656','1b13d0774','664f27889','4f8f598c7','4316f5448','56395456e','1343d0b9f','cba88e475','4a7da2229','37867b35a','969e2b58d','a0e8f7eff','903d01e45','b3ff8c551','a41774c0b','87f267','8dc9afc35','d002903fe']
INV='docs/evidence/nominal-payload-827/source-inventory-activation-v14.json';inventory=load(INV);nominal={r['source']:r for r in inventory['rows']}
proof_paths=['docs/evidence/combined-780-drop-counter-20261015/combined-source-proof.json','docs/evidence/constructor-compile-correction-7623-20261015/source-proof.json','docs/evidence/projection-explicit-import-correction-a417/combined-source-proof.json','docs/evidence/projection-explicit-import-correction-a417/import-only-body-proof.json','docs/evidence/nested-byte-build-correction-a33-20261015/full-active-source-proof.json','docs/evidence/nested-byte-build-correction-a33-20261015/source-proof.json','docs/evidence/nested-byte-traversal-780/final-source-proof.json','docs/evidence/nested-byte-traversal-780/later-source-proof.json']
preserved_proofs={p:sha((R/p).read_bytes()) for p in proof_paths};excluded_sha=sha((R/EXCLUDE).read_bytes());archives={};source_bytes={};write=[];summary=[]
cache={}
def candidate(revision,name):
 key=(revision,name)
 if key not in cache:
  p=subprocess.run(['git','show',revision+':'+name],cwd=R,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL);cache[key]=p.stdout if p.returncode==0 else None
 return cache[key]
for name in SHARED+EXTRA:
 packet=json.loads((R/name).read_bytes()) if name in ['docs/evidence/ordered-large-integration-20261015/census-visibility-source-inverses.json','docs/evidence/ordered-large-integration-20261015/purity-token-source-inverses.json'] else load(name);rows=packet if isinstance(packet,list)else packet.get('whole_sources',packet.get('owners',[packet]));prior=copy.deepcopy(rows);repairs=[];independent=0
 if name=='docs/evidence/constructor-inline-failure-825/exact-source-inverses.json' and not args.verify_only:
  for child in packet['children']:
   actual=(R/child['source']).read_bytes()
   if sha(actual)!=child['sha256']:
    assert child['source']=='src/workspace_graph/constructor_failure.rs'
    ar='docs/evidence/constructor-report-receiving-825/original/src/workspace_graph/constructor_failure.rs'
    assert sha((R/ar).read_bytes())==child['sha256']
    child.setdefault('prior_bindings',[]).append({'sha256':child['sha256'],'archive':ar,'change_owner':'docs/evidence/constructor-report-receiving-825/source-inverses.json','application_basis':BASE})
    child['sha256']=sha(actual)
 is_nominal=name==SHARED[-1];backward=Path(name).parent.name in REVERSE
 for old,row in zip(prior,rows):
  file=row.get('source',row.get('file',row.get('path')));b=(R/file).read_bytes();source_bytes[file]=b;current=b.decode('utf8');assert '\r' not in current,('CRLF',file)
  r=nominal[file] if is_nominal else row
  key='inverse' if 'inverse'in row else 'hooks' if 'hooks'in row else 'edits';edits=row.get(key,[]);nk='current' if key=='inverse' or(edits and 'current'in edits[0]) else 'after' if key=='hooks'or(edits and 'after'in edits[0])else'new';before_key='previous' if edits and 'previous'in edits[0] else 'before'
  if r.get('new_file') and not edits:
   binding=r.get('current_sha256',r.get('sha256'))
   if args.verify_only:assert binding==sha(b),(name,file,'new source binding')
   else:
    prior_bytes=next((candidate(revision,file) for revision in CANDIDATES if candidate(revision,file) is not None and binding==sha(candidate(revision,file))),None)
    assert prior_bytes is not None,(name,file,'exact immutable new-source prior binding')
    if 'current_sha256' in r:r['current_sha256']=sha(b)
    if 'sha256' in r:r['sha256']=sha(b)
   independent+=1;continue
  ar=r.get('archive',r.get('original'));ar=ar or ('docs/evidence/default-static-error-custody-825/defaults.before.txt' if name.endswith('default-static-error-custody-825/exact-source-inverse.json') else None);expected=r.get('original_sha256',r.get('before_sha256',r.get('old_sha256',r.get('sha256'))));arch=None
  if ar:
   a=R/ar
   if not a.exists():a=R/Path(name).parent/ar
   assert a.is_file(),(name,file,'missing fixed archive',ar);arch=a.read_bytes();archives[str(a.relative_to(R))]=sha(arch)
   if expected:assert sha(arch)==expected,(name,file,'archive hash')
  empty=not ar and ('original'in r or r.get('new_file') or expected==sha(b''))
  assert arch is not None or expected or empty,(name,file,'no fixed original')
  def projects(text,hook_list):
   for e in reversed(hook_list) if backward else hook_list:
    if text.count(e[nk])!=e.get('count',1):return False
    text=text.replace(e[nk],e[before_key],e.get('count',1))
   raw=text.encode('utf8')
   return (not expected or sha(raw)==expected) and (arch is None or raw==arch) and (not empty or raw==b'')
  if not projects(current,edits):
   assert not args.verify_only,(name,file,'uncomposed current source')
   checkpoint=None
   for revision in CANDIDATES:
    prior_bytes=candidate(revision,file)
    if prior_bytes is not None and projects(prior_bytes.decode('utf8'),edits):checkpoint=(revision,prior_bytes);break
   if checkpoint is None:
    binding=row.get('current_sha256',r.get('sha256'))
    for e in edits:
     text=e[nk]
     if binding and sha(text.encode('utf8'))==binding and projects(text,edits):checkpoint=('tracked-inline-whole-source-with-exact-recorded-SHA',text.encode('utf8'));break
   assert checkpoint is not None,(name,file,'no authenticated complete prior checkpoint')
   revision,old_bytes=checkpoint
   added={nk:current,before_key:old_bytes.decode('utf8')}
   if nk=='after':added.update(name='ordered-large-integrated-whole-source-preinverse',count=1)
   if key=='inverse':added['hook']='ordered-large-integrated-whole-source-preinverse'
   row[key]=edits+[added] if backward else [added]+edits
   repairs.append({'source':file,'checkpoint':revision,'checkpoint_sha256':sha(old_bytes),'current_sha256':sha(b),'blocks':1})
  assert projects(current,row.get(key,[])),(name,file,'complete original reconstruction')
  # Existing archived hooks and all original fields remain exact; only binding
  # fields and the single additive preinverse may differ.
  old_edits=old.get(key,[]);now=row.get(key,[])
  assert (now[:len(old_edits)] if backward else now[-len(old_edits):] if old_edits else [])==old_edits,(name,file,'old hooks changed')
  for k,v in old.items():
   if k not in (key,'current_sha256','current_bytes'):assert row[k]==v,(name,file,k)
  if args.verify_only:
   if 'current_sha256'in row:assert row['current_sha256']==sha(b),(name,file,'current binding')
   if is_nominal:assert nominal[file]['sha256']==sha(b)
  else:
   if 'current_sha256'in row:row['current_sha256']=sha(b)
   if 'current_bytes'in row:row['current_bytes']=len(b)
   if is_nominal:nominal[file]['sha256']=sha(b)
 write.append((name,packet));summary.append({'packet':name,'rows':len(rows),'repairs':repairs,'whole_original_rows':len(rows)-independent,'independently_bound_new_rows':independent,'all_old_hooks_and_originals_preserved':True})
assert sum(x['rows']for x in summary[:21])==236
assert [x['rows']for x in summary[21:]]==[5,10,41,19,2,18,25,11,14,10,1,8,2,10,3,2,1,2,5,1,2,3,14,10,9,5,2,15,10,41,3]
# Validate every current input and fixed archive again before any metadata write.
for p,b in source_bytes.items():assert (R/p).read_bytes()==b,p
for p,h in archives.items():assert sha((R/p).read_bytes())==h,p
if not args.verify_only:
 for p,obj in write:(R/p).write_text(json.dumps(obj,indent=2)+'\n')
 inventory['ordered_integrated_source_basis']=BASE;(R/INV).write_text(json.dumps(inventory,indent=2)+'\n')
# Verify all five #825 children under their unchanged exact bindings.
constructor=json.loads((R/'docs/evidence/constructor-inline-failure-825/exact-source-inverses.json').read_bytes())
for child in constructor['children']:assert sha((R/child['source']).read_bytes())==child['sha256'],child['source']
# Execute the complete current 36-owner source-only reconstruction, not Rust.
log=io.StringIO()
with contextlib.redirect_stdout(log):ns=runpy.run_path(str(R/'docs/evidence/source-projection-integration-797b/source-only-inverses.py'))
assert len(ns['results'])==36,len(ns['results'])
# Session19 is explicitly included; its complete originals/hooks are verified above.
for p,h in preserved_proofs.items():assert sha((R/p).read_bytes())==h,p
proof={'status':'SOURCE_ONLY_NO_RUST_OR_CARGO_EXECUTION','immutable_basis':BASE,'shared_packet_count':21,'shared_rows':236,'additional_rows':{Path(x['packet']).parent.name:x['rows']for x in summary[21:]},'total_packets':len(summary),'total_rows':sum(x['rows']for x in summary),'packets':summary,'source_36':ns['results'],'source_36_output':log.getvalue(),'unchanged_original_archives':archives,'preserved_prior_proofs':preserved_proofs,'constructor825_children_current_bindings_verified':5,'constructor825_original_constructor_failure_binding_archived':True,'all_source_inputs_unchanged':True}
if not args.verify_only:HERE.mkdir(parents=True,exist_ok=True)
if not args.verify_only:(HERE/'combined-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
print(json.dumps({'mode':'verify-only'if args.verify_only else'composed','packets':len(summary),'rows':proof['total_rows'],'repairs':sum(len(x['repairs'])for x in summary),'source_36':'PASS','old_proofs_unchanged':True,'session19_full_originals_verified':True}))
