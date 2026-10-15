#!/usr/bin/env python3
"""Bounded-memory metadata composition; all authority is immutable Git/source bytes."""
from pathlib import Path
from collections import OrderedDict
import argparse,copy,hashlib,json,re,subprocess,tempfile,os,runpy,contextlib,io
A=argparse.ArgumentParser();A.add_argument('--verify-only',action='store_true');A.add_argument('--basis',required=True);A.add_argument('--extra-packet',action='append',default=[]);A.add_argument('--source36-runner',default='docs/evidence/source36-main88884-merge-20261015/verify_source36.py');args=A.parse_args()
R=Path.cwd();HERE=Path(__file__).resolve().parent;BASE=args.basis
assert re.fullmatch(r'[0-9a-f]{40}',BASE)
OBSERVED_HEAD=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()
if not args.verify_only:assert OBSERVED_HEAD==BASE, 'exact immutable source basis required'
sha=lambda b:hashlib.sha256(b).hexdigest()
def digest(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
class Git:
 def __init__(self):
  self.p=subprocess.Popen(['git','cat-file','--batch'],cwd=R,stdin=subprocess.PIPE,stdout=subprocess.PIPE);self.cache=OrderedDict();self.bytes=0;self.peak=0
 def get(self,rev,source):
  key=(rev,source)
  if key in self.cache:self.cache.move_to_end(key);return self.cache[key]
  self.p.stdin.write((rev+':'+source+'\n').encode());self.p.stdin.flush();header=self.p.stdout.readline().decode().strip()
  if header.endswith(' missing'):data=None
  else:
   _,kind,size=header.split();assert kind=='blob';data=self.p.stdout.read(int(size));assert self.p.stdout.read(1)==b'\n'
  size=len(data) if data is not None else 0
  while self.cache and (self.bytes+size>16*1024*1024 or len(self.cache)>=128):
   _,old=self.cache.popitem(last=False);self.bytes-=len(old) if old is not None else 0
  if size<=16*1024*1024:self.cache[key]=data;self.bytes+=size;self.peak=max(self.peak,self.bytes)
  return data
G=Git()
def load(p):return json.loads((R/p).read_bytes())
def original(p):return json.loads(G.get(BASE,p))
PRIOR='docs/evidence/incremental-source-composition-d39fe67-20261015'
PRIOR_PACKET_BASIS='ea0be15f4dea6f3db36a031e17988e9fa105dbf5'
oldproof=original(PRIOR+'/combined-source-proof.json')
names=[x['packet'] for x in oldproof['packets']]
assert len(names)==90 and sum(row['rows'] for row in oldproof['packets'])==795
prior_names=set(names)
# All three prior command-line additions are now mandatory default inputs.
for required in ['docs/evidence/source36-after-23e8-20261015/source-inverses.json',
                 'docs/evidence/source36-after-23e8-20261015/source-projections.json',
                 'docs/evidence/field-update-integration-digest-correction-23e8-20261015/source-inverses.json']:
 assert required in names
histories=load(str(HERE.relative_to(R))+'/next-packet-histories.json')
seeds={row['packet']:row for row in histories}
for row in histories:
 assert digest(R/row['seed'])==row['seed_sha256']
 assert digest(R/row['complete_patch'])==row['patch_sha256']
names += list(seeds)
names += ['docs/evidence/ram-round-warning-corrections-20261015/source-inverses.json',
          'docs/evidence/source36-after-next-838-839-20261015/source-inverses.json',
          'docs/evidence/source36-after-next-838-839-20261015/source-projections.json',
          'docs/evidence/source36-main88884-merge-20261015/source-inverses.json',
          'docs/evidence/source36-main88884-merge-20261015/ours-projections.json',
          'docs/evidence/source36-main88884-merge-20261015/main-reflow-projections.json']
assert len(names)==len(set(names))==100
for extra in args.extra_packet:
 assert extra.startswith('docs/evidence/') and extra.endswith('.json') and '..' not in Path(extra).parts
 assert extra not in names
 names.append(extra)
REVERSE={'loan-candidate-inventory-reuse','exit-status-directory-reuse','referenced-edge-ownership-proof','prefix-certified-expression-lookup','receiving-transitive-results-816','exit-prefix-reference-proof','status-fact-borrow-reuse','constructor-inline-failure-825','default-static-error-custody-825','prelude-forecast-286','pin-capture','region-entry-receiving-816'}
script=G.get(BASE,'docs/evidence/compile-correction-e3eb-20261015/compose_active_sources.py').decode()
revisions=list(dict.fromkeys([BASE,'6e549d1ae','ea0be15f4dea6f3db36a031e17988e9fa105dbf5','5f1fecd42694c52fbdcec67f9ef3d3fd0b7d7f3b','d39fe676a7c1df3323888b25c9481f7bbb65feaa','15ac307443132d8631e8b16b036ba9a689d97139','f8ff67481513a720a50eb747188beb19bc6b9b83','41b6e19ad','40f77262950e9dc4bb1da6626f441f2f504cfe59','94c392e9b','1dee376ca','ae7285469','b3e4e2b71','c771609c1','d6bf81519','e3eb47683','cb7aeb1a1','75fca93b2','8d1c4bc59','315b70b27','bb12f4897','77f454702','8756ad07a']+re.findall(r"'([0-9a-f]{6,40})'",script)))
logcache={}
def candidates(source):
 yield from revisions
 if source not in logcache:logcache[source]=subprocess.check_output(['git','log','--all','--format=%H','--',source],cwd=R,text=True).splitlines()
 yield from logcache[source]
def checkpoint(source,predicate,binding=None):
 seen=set()
 for rev in candidates(source):
  if rev in seen:continue
  seen.add(rev);b=G.get(rev,source)
  if b is not None and (not binding or sha(b)==binding) and predicate(b):return rev,b
 raise AssertionError(('no immutable complete checkpoint',source,binding))
# Preserve every existing proof, Python driver, and before/original/checkpoint payload byte.
preserved={}
for p in (R/'docs/evidence').rglob('*'):
 if not p.is_file() or HERE in p.parents:continue
 rel=str(p.relative_to(R))
 if p.suffix=='.py' or 'proof' in p.name or p.suffix in ('.before','.gz') or '.before.' in p.name or '.patch.' in p.name or any(x in p.parts for x in ('original','originals','checkpoint','prior','old-source')):
  preserved[rel]=digest(p)
source_hashes={};archives={};summary=[];derived={};newbindings=[]
# Reuse every active row from the complete previous90 packet proof. Old836 and
# formatter derived manifests stay active at their existing paths; old proof,
# archive and driver bytes remain immutable. No historical manifest is re-derived.
INV='docs/evidence/nominal-payload-827/source-inventory-activation-v14.json';inventory=load(INV) if args.verify_only else original(INV);nominal={r['source']:r for r in inventory['rows']}
def archive_read(path,packet):
 q=R/path
 if not q.exists() and path not in derived:q=R/Path(packet).parent/path
 key=str(q.relative_to(R));raw=derived.get(key)
 if raw is None:raw=q.read_bytes()
 archives[key]=sha(raw);return raw
with tempfile.TemporaryDirectory(prefix='.composition-',dir=HERE) as tmp:
 staged=[]
 def stage(name,data):
  q=Path(tmp)/str(len(staged));q.write_bytes(data);staged.append((name,q))
 for n,data in derived.items():
  if isinstance(data,bytes):stage(n,data)
 for name in names:
  packet=load(name) if args.verify_only else load(seeds[name]['seed']) if name in seeds else json.loads(G.get(PRIOR_PACKET_BASIS,name)) if name in prior_names else copy.deepcopy(derived[name]) if name in derived else original(name)
  rows=packet if isinstance(packet,list) else packet.get('whole_sources',packet.get('owners',[packet]));repairs=[];independent=0;backward=Path(name).parent.name in REVERSE;isnom='nominal-payload-827/exact-source-inverses-activation-v14.json' in name
  for row in rows:
   oldrow=copy.deepcopy(row);source=row.get('source',row.get('file',row.get('path')));raw=(R/source).read_bytes();source_hashes[source]=sha(raw);text=raw.decode();record=nominal[source] if isnom else row
   key='inverse' if 'inverse' in row else 'hooks' if 'hooks' in row else 'edits';edits=row.get(key,[])
   nk='current' if key=='inverse' or (edits and 'current' in edits[0]) else 'after' if key=='hooks' or (edits and 'after' in edits[0]) else 'new'
   bk='previous' if edits and 'previous' in edits[0] else 'old' if edits and 'old' in edits[0] else 'before'
   if record.get('new_file') and not edits:
    binding=record.get('current_sha256',record.get('sha256'))
    if args.verify_only:assert binding==sha(raw),(name,source,'new binding')
    else:
     rev,prior=checkpoint(source,lambda b:True,binding);newbindings.append(dict(packet=name,source=source,prior_sha256=binding,prior_revision=rev,current_sha256=sha(raw)))
     if 'current_sha256' in record:record['current_sha256']=sha(raw)
     if 'sha256' in record:record['sha256']=sha(raw)
    independent+=1;continue
   ar=record.get('archive',record.get('original'))
   if not ar and name.endswith('default-static-error-custody-825/exact-source-inverse.json'):ar='docs/evidence/default-static-error-custody-825/defaults.before.txt'
   expected=record.get('original_sha256',record.get('before_sha256',record.get('old_sha256',record.get('sha256'))))
   archived=archive_read(ar,name) if ar else None
   if archived is not None and expected:assert sha(archived)==expected,(name,source,'archive SHA')
   empty=not ar and ('original' in record or record.get('new_file') or expected==sha(b''))
   assert archived is not None or expected or empty,(name,source,'no full original')
   def projects(b,es):
    t=b.decode()
    for e in reversed(es) if backward else es:
     if t.count(e[nk])!=e.get('count',1):return False
     t=t.replace(e[nk],e[bk],e.get('count',1))
    out=t.encode();return (not expected or sha(out)==expected) and (archived is None or out==archived) and (not empty or out==b'')
   if not projects(raw,edits):
    assert not args.verify_only,(name,source,'uncomposed source')
    rev,prior=checkpoint(source,lambda b:projects(b,edits));extra={nk:text,bk:prior.decode()}
    if nk=='after':extra.update(name='final-integrated-whole-source-preinverse',count=1)
    if key=='inverse':extra['hook']='final-integrated-whole-source-preinverse'
    row[key]=edits+[extra] if backward else [extra]+edits;repairs.append(dict(source=source,checkpoint=rev,checkpoint_sha256=sha(prior),current_sha256=sha(raw)))
   assert projects(raw,row.get(key,[])),(name,source,'whole original')
   oldedits=oldrow.get(key,[]);now=row.get(key,[])
   assert (now[:len(oldedits)] if backward else now[-len(oldedits):] if oldedits else [])==oldedits
   for k,v in oldrow.items():
    if k not in (key,'current_sha256','current_bytes'):assert row[k]==v,(name,source,'old field',k)
   if args.verify_only:
    if 'current_sha256' in row:assert row['current_sha256']==sha(raw),(name,source,'current SHA')
    if isnom:assert nominal[source]['sha256']==sha(raw)
   else:
    if 'current_sha256' in row:row['current_sha256']=sha(raw)
    if 'current_bytes' in row:row['current_bytes']=len(raw)
    if isnom:nominal[source]['sha256']=sha(raw)
  if isinstance(packet,dict):
   for child in packet.get('children',[]):
    source=child['source'];current=(R/source).read_bytes();source_hashes[source]=sha(current)
    if child['sha256']!=sha(current):
     assert not args.verify_only,(name,source,'child SHA');rev,prior=checkpoint(source,lambda b:True,child['sha256']);child.setdefault('prior_bindings',[]).append(dict(sha256=child['sha256'],revision=rev,application_basis=BASE));child['sha256']=sha(current)
  stage(name,(json.dumps(packet,indent=2)+'\n').encode());summary.append(dict(packet=name,rows=len(rows),repairs=repairs,whole_original_rows=len(rows)-independent,independently_bound_new_rows=independent,all_old_hooks_and_originals_preserved=True))
 if not args.verify_only:inventory['final_integrated_source_basis']=BASE
 stage(INV,(json.dumps(inventory,indent=2)+'\n').encode())
 for s,h in source_hashes.items():assert digest(R/s)==h
 for s,h in preserved.items():assert digest(R/s)==h,(s,'immutable historical proof/archive/script changed')
 if not args.verify_only:
  for name,temp in staged:
   q=R/name;q.parent.mkdir(parents=True,exist_ok=True);os.replace(temp,q)
# Full source36 uses its named authenticated Rust views and every original undo/assertion.
log=io.StringIO()
with contextlib.redirect_stdout(log):ns=runpy.run_path(str(R/args.source36_runner))
assert len(ns['results'])==36
assert args.source36_runner.startswith('docs/evidence/') and '..' not in Path(args.source36_runner).parts
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==OBSERVED_HEAD, 'HEAD drift during composition'
for s,h in preserved.items():assert digest(R/s)==h,s
proof=dict(status='SOURCE_ONLY_NO_RUST_EXECUTION',immutable_basis=BASE,total_packets=len(summary),total_rows=sum(x['rows'] for x in summary),packets=summary,source_36=ns['results'],source_36_output=log.getvalue(),source_inputs=source_hashes,unchanged_original_archives=archives,preserved_prior_proofs_and_scripts=preserved,new_source_prior_bindings=newbindings,candidate_cache_limit_bytes=16*1024*1024,candidate_cache_peak_bytes=G.peak,prior_active_packet_count=90,prior_active_row_count=795,prior_active_packet_basis=PRIOR_PACKET_BASIS,new_complete_stage_histories=histories)
if not args.verify_only:(HERE/'combined-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
print(json.dumps(dict(mode='verify-only' if args.verify_only else 'composed',packets=len(summary),rows=proof['total_rows'],repairs=sum(len(x['repairs']) for x in summary),source36='PASS',cache_peak_bytes=G.peak)))
G.p.terminate()
