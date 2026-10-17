"""Closed final actual -> complete afa acquisitions; no old table rewrites."""
from pathlib import Path
import json,hashlib,re,subprocess
R=Path('/Volumes/Semaprax/worktrees/main-e177-source-integration-20261017');HEAD='5dff900cd1c3004437e3f706547ef55eda254563';AFA='afa1e4d9da88da6420776ae3184cc9efb552d022';REL='docs/evidence/main-e177-final-source-binding-20261017';D=R/REL;MOD='src/source_reflows_merge/main_e177_final_inputs.rs';H=lambda b:hashlib.sha256(b).hexdigest();G=lambda rev,p:subprocess.check_output(['git','show',rev+':'+p],cwd=R)
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R).decode().strip()==HEAD
files={}
def archive(raw,category):
 p=REL+'/'+category+'/'+H(raw)+'.txt';files[p]=raw;return dict(length=len(raw),sha256=H(raw),archive=p)
oldpath='docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json';mainpath='docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json';old=json.loads(G(AFA,oldpath));main=json.loads(G(AFA,mainpath));catalog={};wanted={}
def add(source,name,pair,raw):
 assert(len(raw),H(raw))==(pair['length'],pair['sha256']),(source,name)
 wanted.setdefault(source,[]).append(dict(name=name,**archive(raw,'views')))
for row in old['rows']:
 for view in [row['current'],*row['views']]:catalog[(row['source'],view['length'],view['sha256'])]=G(AFA,view['archive'])
 v=row['current'];add(row['source'],'whole-afa54-current',v,catalog[(row['source'],v['length'],v['sha256'])])
for row in main['rows']:
 source=row['path'];pair=dict(length=row['current_length'],sha256=row['current_sha256']);key=(source,pair['length'],pair['sha256']);raw=catalog.get(key)
 if raw is None:
  for rev in [AFA,HEAD,'a55df7587c9701be51424b6b35d2f9e0463da843','c46aa26a087ae1465c98de644dadd1082c85e895','99f1f06d361e16bd937f4a6eda19703e78f0e525']:
   r=subprocess.run(['git','show',rev+':'+source],cwd=R,capture_output=True)
   if r.returncode==0 and(len(r.stdout),H(r.stdout))==key[1:]:raw=r.stdout;break
 assert raw is not None,(source,pair)
 add(source,'whole-afa454-current',pair,raw)
# Bind the whole old guard's own current pins too, including old54 helper.
for source in json.loads((D/'hook-and-raw-inverses.json').read_bytes())['new_modules']:
 if source==MOD:continue
 try:raw=G(AFA,source)
 except subprocess.CalledProcessError:continue
 add(source,'whole-afa-module',dict(length=len(raw),sha256=H(raw)),raw)
# Only changed tuples need a bridge; every new actual module remains fully bound.
newmods=json.loads((D/'hook-and-raw-inverses.json').read_bytes())['new_modules'];sources=set(newmods)-{MOD}
for source,views in wanted.items():
 current=(R/source).read_bytes()
 if any((len(current),H(current))!=(v['length'],v['sha256'])for v in views):sources.add(source)
rows=[];needed=set()
for source in sorted(sources):
 b=(R/source).read_bytes();views=wanted.get(source,[]);unique=[];seen=set()
 for v in views:
  k=(v['length'],v['sha256'])
  if k not in seen:unique.append(v);seen.add(k)
 cur=archive(b,'current');rows.append(dict(source=source,compiled=source.endswith('.rs'),current=cur,views=unique));needed.add(cur['archive']);needed.update(v['archive']for v in unique)
anchors=[]
for path in [oldpath,mainpath]:
 b=G(AFA,path);assert(R/path).read_bytes()==b;v=archive(b,'tables');needed.add(v['archive']);anchors.append(dict(name='immutable-whole-afa',path=path,git_head=AFA,**v))
packet=dict(schema='semaprax.main-e177-final-whole-inputs.v1',semantic_head=HEAD,source_cap=9753094,self_binding_exclusions=[MOD],complete_old_table_anchors=anchors,rows=rows)
raw=(json.dumps(packet,indent=2)+'\n').encode();assert len(raw)<=262144,len(raw)
for p in needed:
 q=R/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(files[p])
p=REL+'/current-inputs.json';(R/p).write_bytes(raw)
# Complete already-admitted source acquisition algorithm, distinct schema/domain/cap.
s=G(AFA,'src/source_reflows_merge/next_big_batch_inputs.rs').decode();a=s.index('fn table_compiled(');b=s.index('fn read_view(',a)
# Replace only exact table/compiled match definitions, leaving FD/body/negative controls intact.
block='fn table_compiled(source: &str) -> Option<&\'static str> {\n    match source {\n'+''.join('        '+json.dumps(x['path'])+' => Some(include_str!("../../'+x['path']+'")),\n'for x in anchors)+'        _ => None,\n    }\n}\nfn compiled(source: &str) -> Option<&\'static str> {\n    match source {\n'+''.join('        '+json.dumps(r['source'])+' => Some(include_str!("../../'+r['source']+'")),\n'for r in rows if r['compiled'])+'        _ => None,\n    }\n}\n'
s=s[:a]+block+s[b:];s=s.replace('const SOURCE_CAP: usize = 9730253;','const SOURCE_CAP: usize = 9753094;').replace(oldpath,p).replace('semaprax.next-big-batch-whole-inputs.v1',packet['schema']).replace('assert_eq!(data.len(), 112927);','assert_eq!(data.len(), '+str(len(raw))+');').replace('336d79ff6cddc852d4750aa304b85a31494ac98c2e015bb52dfb615db7d98fe3',H(raw)).replace('assert_eq!(packet["rows"].as_array().unwrap().len(), 54);','assert_eq!(packet["rows"].as_array().unwrap().len(), '+str(len(rows))+');').replace('fn complete_current_and_parent_views_refuse_foreign_truncated_duplicate_and_wrong_stages()','fn latest_main_and_afa_complete_views_preserve_all_prior_controls_and_refuse_wrong_stages()')
# The global oldpath replacement also altered its table match: restore that one named anchor.
s=s.replace(json.dumps(p)+' => Some(include_str!("../../'+p+'")),',json.dumps(oldpath)+' => Some(include_str!("../../'+oldpath+'")),')
assert s.count(p)==2 and len(s.splitlines())<=1500
(R/MOD).write_text(s)
(D/'source-stage-receipt.json').write_text(json.dumps(dict(status='SOURCE_ONLY_NOT_RUNTIME',rows=len(rows),whole_views=sum(len(x['views'])for x in rows),packet_length=len(raw),packet_sha256=H(raw),helper_length=len(s.encode()),helper_sha256=H(s.encode()),helper_lines=len(s.splitlines()),anchors=anchors,all_new_modules=newmods,immutable_afa54_and_afa454_unchanged=True,source_cap_named_historical_only=9753094),indent=2)+'\n')
print(json.dumps({'rows':len(rows),'packet':len(raw),'helper_lines':len(s.splitlines())}))
