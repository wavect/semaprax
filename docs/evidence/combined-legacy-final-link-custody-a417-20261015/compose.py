"""Metadata-only final composition, immutable 87f267 input, no Rust/Cargo execution."""
from pathlib import Path
import argparse,copy,hashlib,io,json,re,runpy,subprocess,contextlib
P=argparse.ArgumentParser();P.add_argument('--verify-only',action='store_true');args=P.parse_args()
R=Path.cwd();BASE='a41774c0b3522ea7e13798f1a337b0aa5f35714e';HERE=R/'docs/evidence/combined-legacy-final-link-custody-a417-20261015'
sha=lambda b:hashlib.sha256(b).hexdigest()
def blob(path,revision=BASE):return subprocess.check_output(['git','show',revision+':'+str(path)],cwd=R)
def load(path):return json.loads((R/path).read_bytes() if args.verify_only else blob(path))
if not args.verify_only:assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==BASE
SESSION='docs/evidence/selected-session-static-refusal-781/exact-source-inverses.json'
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
EXTRA=['docs/evidence/nested-byte-build-correction-a33-20261015/whole-source-inverses.json','docs/evidence/factored-epsilon-closure-sharing/exact-source-inverses.json','docs/evidence/nested-byte-traversal-780/final-source-inverses-v1.json']
EXTRA += [SESSION,'docs/evidence/selected-session-compile-correction-8795/exact-source-inverses.json','docs/evidence/legacy-nominal-sharing-827/exact-source-inverses.json']
assert len(SHARED)==21 and len(EXTRA)==6
REVERSE={'loan-candidate-inventory-reuse','exit-status-directory-reuse','referenced-edge-ownership-proof','prefix-certified-expression-lookup','receiving-transitive-results-816','exit-prefix-reference-proof','status-fact-borrow-reuse'}
CANDIDATES=[BASE,'87f267a62ceeec115f83ba180d9abfc495ca3c34','d002903fe','4807e7df6','07b76aa3c','e7b1d6dd8','63e782715','bf4779130','25b2ab2d5']
INV='docs/evidence/nominal-payload-827/source-inventory-activation-v14.json';inventory=load(INV);nominal={r['source']:r for r in inventory['rows']}
proof_paths=['docs/evidence/nested-byte-build-correction-a33-20261015/full-active-source-proof.json','docs/evidence/nested-byte-build-correction-a33-20261015/source-proof.json','docs/evidence/nested-byte-traversal-780/final-source-proof.json','docs/evidence/nested-byte-traversal-780/later-source-proof.json']
proof_paths += ['docs/evidence/combined-final-custody-e7b1-20261015/combined-source-proof.json','docs/evidence/combined-final-custody-e7b1-20261015/compose.py','docs/evidence/selected-session-compile-correction-8795/source-proof.json','docs/evidence/legacy-nominal-sharing-827/source-only-check.json','docs/evidence/selected-session-static-refusal-781/source-only-check.json']
proof_paths += ['docs/evidence/combined-cow-custody-87f2-20261015/compose.py','docs/evidence/combined-cow-custody-87f2-20261015/combined-source-proof.json']
proof_paths += ['docs/evidence/combined-help-custody-d766-20261015/compose.py','docs/evidence/combined-help-custody-d766-20261015/combined-source-proof.json']
preserved_proofs={p:sha((R/p).read_bytes()) for p in proof_paths};archives={};source_bytes={};write=[];summary=[]
cache={}
def candidate(revision,name):
 key=(revision,name)
 if key not in cache:
  p=subprocess.run(['git','show',revision+':'+name],cwd=R,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL);cache[key]=p.stdout if p.returncode==0 else None
 return cache[key]
for name in SHARED+EXTRA:
 packet=load(name);rows=packet if isinstance(packet,list)else packet['whole_sources'];prior=copy.deepcopy(rows);repairs=[];independent=0
 is_nominal=name==SHARED[-1];backward=Path(name).parent.name in REVERSE
 for old,row in zip(prior,rows):
  file=row.get('source',row.get('file'));b=(R/file).read_bytes();source_bytes[file]=b;current=b.decode('utf8');assert '\r' not in current,('CRLF',file)
  r=nominal[file] if is_nominal else row
  key='inverse' if 'inverse'in row else 'hooks' if 'hooks'in row else 'edits';edits=row.get(key,[]);nk='current' if key=='inverse' else 'after' if key=='hooks'or(edits and 'after'in edits[0])else'new'
  if r.get('new_file') and not edits:
   assert r.get('current_sha256',r.get('sha256'))==sha(b),(name,file,'independent fresh binding')
   independent+=1;continue
  ar=r.get('archive',r.get('original'));expected=r.get('original_sha256',r.get('before_sha256',r.get('old_sha256',r.get('sha256'))));arch=None
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
    text=text.replace(e[nk],e['before'],e.get('count',1))
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
   added={nk:current,'before':old_bytes.decode('utf8')}
   if nk=='after':added.update(name='legacy-final-link-a417-whole-source-preinverse',count=1)
   if key=='inverse':added['hook']='legacy-final-link-a417-whole-source-preinverse'
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
assert [x['rows']for x in summary[21:]]==[5,10,41,19,2,18]
# Validate every current input and fixed archive again before any metadata write.
for p,b in source_bytes.items():assert (R/p).read_bytes()==b,p
for p,h in archives.items():assert sha((R/p).read_bytes())==h,p
if not args.verify_only:
 for p,obj in write:(R/p).write_text(json.dumps(obj,indent=2)+'\n')
 inventory['combined_final_source_basis']=BASE;(R/INV).write_text(json.dumps(inventory,indent=2)+'\n')
# Execute the complete current 36-owner source-only reconstruction, not Rust.
log=io.StringIO()
with contextlib.redirect_stdout(log):ns=runpy.run_path(str(R/'docs/evidence/source-projection-integration-797b/source-only-inverses.py'))
assert len(ns['results'])==36,len(ns['results'])
for p,h in preserved_proofs.items():assert sha((R/p).read_bytes())==h,p
proof={'status':'SOURCE_ONLY_NO_RUST_OR_CARGO_EXECUTION','immutable_basis':BASE,'shared_packet_count':21,'shared_rows':236,'additional_rows':{'root_correction':5,'alias':10,'nested_byte_originals':41,'session':19,'session_correction':2,'legacy_nominal':18},'total_packets':len(summary),'total_rows':sum(x['rows']for x in summary),'packets':summary,'source_36':ns['results'],'source_36_output':log.getvalue(),'unchanged_original_archives':archives,'preserved_prior_proofs':preserved_proofs,'all_source_inputs_unchanged':True}
if not args.verify_only:(HERE/'combined-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
print(json.dumps({'mode':'verify-only'if args.verify_only else'composed','packets':len(summary),'rows':proof['total_rows'],'repairs':sum(len(x['repairs'])for x in summary),'source_36':'PASS','old_proofs_unchanged':True,'active_session19_and_correction2_and_own18':'PASS'}))
