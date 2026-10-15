#!/usr/bin/env python3
"""Independent old-hook/checkpoint and whole starter-patch custody verification."""
from pathlib import Path
import json,hashlib,subprocess,tempfile,os,re,argparse
parser=argparse.ArgumentParser();parser.add_argument('--basis',required=True);args=parser.parse_args()
R=Path.cwd();P=Path(__file__).resolve().parent;BASE=args.basis;sha=lambda b:hashlib.sha256(b).hexdigest()
assert re.fullmatch(r'[0-9a-f]{40}',BASE)
def blob(rev,p):return subprocess.check_output(['git','show',rev+':'+p],cwd=R)
def read(p):return json.loads((R/p).read_text())
proof=json.loads((P/'combined-source-proof.json').read_text());oldhooks=0;checkpoints=0
for packet in proof['packets']:
 name=packet['packet'];now=read(name)
 if str(P.relative_to(R)) in name:continue
 old=json.loads(blob(BASE,name));before=old if isinstance(old,list) else old.get('whole_sources',old.get('owners',[old]));after=now if isinstance(now,list) else now.get('whole_sources',now.get('owners',[now]));assert len(before)==len(after)
 reverse=Path(name).parent.name in {'loan-candidate-inventory-reuse','exit-status-directory-reuse','referenced-edge-ownership-proof','prefix-certified-expression-lookup','receiving-transitive-results-816','exit-prefix-reference-proof','status-fact-borrow-reuse','constructor-inline-failure-825','default-static-error-custody-825','prelude-forecast-286','pin-capture','region-entry-receiving-816'}
 repair={x['source']:x for x in packet['repairs']}
 for a,b in zip(before,after):
  source=a.get('source',a.get('file',a.get('path')));key='inverse' if 'inverse' in a else 'hooks' if 'hooks' in a else 'edits';prior=a.get(key,[]);current=b.get(key,[])
  assert len(current) in (len(prior),len(prior)+1),(name,source)
  if source in repair:
   info=repair[source];priorbytes=blob(info['checkpoint'],source);assert sha(priorbytes)==info['checkpoint_sha256'];extra=current[-1] if reverse else current[0];nk='current' if 'current' in extra else 'after' if 'after' in extra else 'new';bk='previous' if 'previous' in extra else 'old' if 'old' in extra else 'before'
   assert extra[nk].encode()==(R/source).read_bytes();assert extra[bk].encode()==priorbytes
   assert (current[:-1] if reverse else current[1:])==prior;checkpoints+=1
  else:assert current==prior
  for k,v in a.items():
   if k not in (key,'current_sha256','current_bytes'):assert b[k]==v,(name,source,k)
  oldhooks+=len(prior)
# Every preserved proof, archive and Python script retains exactly the recorded bytes.
for source,h in proof['preserved_prior_proofs_and_scripts'].items():
 digest=hashlib.sha256()
 with (R/source).open('rb') as f:
  for block in iter(lambda:f.read(1024*1024),b''):digest.update(block)
 assert digest.hexdigest()==h,source
for source,h in proof['source_inputs'].items():assert sha((R/source).read_bytes())==h
assert not subprocess.check_output(['git','diff','--name-only',BASE,'--','*.rs'],cwd=R).strip(),'all Rust stays frozen'
allowed={row['packet'] for row in proof['packets']}|{'docs/evidence/nominal-payload-827/source-inventory-activation-v14.json'}
tracked=subprocess.check_output(['git','diff','--name-only',BASE],cwd=R,text=True).splitlines()
unexpected={path for path in tracked if path not in allowed and not path.startswith(str(P.relative_to(R))+'/')}
assert not unexpected,unexpected

unchanged=[]
for row in read('docs/evidence/opt836-field-update/manifest.json')['complete_prior_independent_receivers_unchanged']:
 assert sha((R/row['path']).read_bytes())==row['sha256'];unchanged.append(row['path'])
for row in read('docs/evidence/record-copy-field-update-836-backends/unchanged-originals.json'):
 assert (R/row['source']).read_bytes()==(R/row['archive']).read_bytes();assert sha((R/row['source']).read_bytes())==row['sha256'];unchanged.append(row['source'])
# Current starter joins authenticate the full current source before their immutable patch proof.
for row in read(str(P.relative_to(R))+'/starter-current-bindings.json'):
 current=(R/row['source']).read_bytes();assert sha(current)==row['current_sha256']
 for edit in row['current_to_authored']:
  assert current.count(edit['new'].encode())==1
  current=current.replace(edit['new'].encode(),edit['old'].encode(),1)
 assert current==blob(row['authored_revision'],row['source'])
 assert sha(current)==row['authored_sha256']
# Scratch reuse retains the bounded actual reservation and backing receiver twice:
# original #834 current method and the new receiving #833 method.
layout=(R/'src/workspace_graph/expected_projection/private_layout.rs').read_text()
assert layout.count('        let capacity = capacity.min(count);')==2
for module in ['capacity_before','capacity_tests','before_registry']:
 assert ('mod '+module+';') in layout
 assert (R/('src/workspace_graph/expected_projection/private_layout/'+module+'.rs')).is_file()
# Apply each full archived starter inverse to its immutable authored tree in a private
# temporary Git index. This does not alter the worktree, refs, source, or Rust output.
patches=[];folder=R/'docs/evidence/application-authoring-batch-836-837-20261015'
for child,revision in [('starter-original','604e7fb05203a6701be6bdbcda8a181c5b969df2'),('starter-held-input-fix','2ae6fee203fb726980524885ec36da268d5361bd')]:
 d=folder/child;manifest=json.loads((d/'immediate-base-full-source.json').read_text())
 with tempfile.TemporaryDirectory(prefix='.patch-index-',dir=P) as temp:
  env=dict(os.environ,GIT_INDEX_FILE=str(Path(temp)/'index'))
  subprocess.run(['git','read-tree',revision],cwd=R,env=env,check=True)
  subprocess.run(['git','apply','--cached','--reverse','--unidiff-zero',str(d/'immediate-source-inverse.patch')],cwd=R,env=env,check=True)
  for row in manifest['files']:
   source=row['path'];fixed=(d/'old-source'/source).read_bytes();assert len(fixed)==row['bytes'];assert sha(fixed)==row['sha256'];assert fixed==blob(row['base'],source)
   projected=subprocess.check_output(['git','show',':'+source],cwd=R,env=env);assert projected==fixed
  patch_paths=re.findall(r'^diff --git a/(.+) b/(.+)$',(d/'immediate-source-inverse.patch').read_text(),re.M)
  for left,right in patch_paths:
   assert left==right
   baseline=subprocess.run(['git','show',manifest['base']+':'+right],cwd=R,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
   projected=subprocess.run(['git','show',':'+right],cwd=R,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
   assert baseline.returncode==projected.returncode and baseline.stdout==projected.stdout,(child,right,'complete patch path inverse')
  patches.append(dict(owner=child,authored_revision=revision,original_base=manifest['base'],all_patch_sources_equal=True,patch_paths=len(patch_paths),archived_patch_direction="forward bytes applied in reverse",archived_files=len(manifest['files'])))
result=dict(status='SOURCE_ONLY_NO_RUST_EXECUTION',old_hooks_preserved=oldhooks,authenticated_added_checkpoint_rows=checkpoints,all_rust_frozen_at=BASE,unchanged_independent_receivers=len(unchanged),starter_complete_patch_source_inverses=patches,starter_current_bindings=38,capacity834_original_and833_receiving_bound_retained=True)
print(json.dumps(result,indent=2))
