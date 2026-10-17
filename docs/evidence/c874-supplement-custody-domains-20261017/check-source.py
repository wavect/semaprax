"""Pure full-input/source checks; no Rust, Cargo, generators or ROOT mutation."""
from pathlib import Path
import hashlib,json,subprocess,importlib.util,tempfile,shutil,os
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
h=lambda b:hashlib.sha256(b).hexdigest()
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def fixed(row):
 b=(D/row['archive']).read_bytes();assert len(b)==row['length']and h(b)==row['sha256']and len(b)<=2097152;return b
rows=json.loads((D/'paid-inputs.json').read_bytes());assert len(rows)==7
for row in rows:
 raw=fixed(row['current']);assert raw==(R/row['path']).read_bytes()
 assert fixed(row['view'])==subprocess.check_output(['git','show',row['view']['head']+':'+row['path']],cwd=R)
recipe=json.loads((R/'docs/evidence/opt870-inline-paid-many-51c1-20261017/frozen-receivers.json').read_bytes())
by={r['path']:r for r in rows}
def view(path):return fixed(by[path]['view'])if path in by else(R/path).read_bytes()
canonical=view('src/cleanup_plan/replay/factored/canonical_shared.rs');assert h(canonical)==recipe['canonical']['current_sha256']
for key,path,original in [('canonical','src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs','complete-canonical-before.rs.txt'),('compare','src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/compare.rs','complete-factored-before.rs.txt')]:
 row=recipe[key];frozen=view(path).decode();assert frozen.startswith(row['header'])and frozen.endswith(row['suffix']);body=frozen[len(row['header']):len(frozen)-len(row['suffix'])if row['suffix']else len(frozen)];assert body.count(row['visibility_after'])==1;body=body.replace(row['visibility_after'],row['visibility_before'],1).encode();old=(R/'docs/evidence/opt870-inline-paid-many-51c1-20261017'/original).read_bytes();assert h(old)==row['sha256'];assert body==old[row['start']:row['end']]
assert(R/'src/cleanup_plan/replay/factored.rs').read_bytes()==(R/'docs/evidence/opt870-inline-paid-many-51c1-20261017/complete-factored-before.rs.txt').read_bytes()
for row in recipe['helpers']:assert len(view(row['path']))==row['length']and h(view(row['path']))==row['sha256']
# Every original production receiving/order predicate remains on RAW source.
raw=(R/'src/cleanup_plan/replay/factored/canonical_shared.rs').read_text()
for phase,receiver,call in [('factored composed promotion receivers','paid_many::PROMOTE_RECEIVING','paid_many::promote('),('factored composed flat add receivers','paid_many::ADD_RECEIVING','paid_many::add('),('factored composed flat finish receivers','paid_many::FINISH_RECEIVING','own_weighted(function, budget, flat)')]:
 assert raw.count(phase)==raw.count(receiver)==2;offset=0
 for _ in range(2):
  at=raw.index(receiver,offset);end=raw.index(call,at);assert phase in raw[at:end]and '?;'in raw[at:end];offset=end+len(call)
assert raw.count('if budget.pays_allocations() { paid_many::note_branch(')==2
receiver=json.loads((D/'receiver-input.json').read_bytes());source=receiver['source'];actual=(R/source).read_bytes();assert len(actual)==receiver['current_length']and h(actual)==receiver['current_sha256'];assert actual==(D/receiver['current_archive']).read_bytes()
stage=(D/receiver['stage_archive']).read_bytes();assert len(stage)==receiver['stage_length']and h(stage)==receiver['stage_sha256'];assert stage==subprocess.check_output(['git','show','49219d230734e000e2884b5b4fb71e61cdf0eb0b:'+source],cwd=R)
original=json.loads((R/'docs/evidence/main043-semantic-acquisition-20261017/current-to-043-stage.json').read_bytes());assert len(original)==6 and stage.decode()==original['current']
augmented=dict(original,previous_length=len(original['previous'].encode()));assert len(augmented)==7;assert h(original['previous'].encode())==original['previous_sha256']
text=stage.decode();lines=text.splitlines(keepends=True)
for edit in reversed(original['inverse']):a,z=edit['start'],edit['end'];assert''.join(lines[a:z])==edit['current'];lines[a:z]=edit['previous'].splitlines(keepends=True)
assert''.join(lines)==original['previous']and augmented['previous_length']==len(''.join(lines).encode())
# Full9 original MAIN restores, including followupReceiverMAIN's named stage.
stages=json.loads((R/'docs/evidence/main67107-fronts-6e45-20261017/stages.json').read_bytes());assert len(stages)==9
for row in stages:
 text=stage.decode()if row['domain']=='followupReceiverMAIN'else row['current'];assert h(text.encode())==row['current_sha256'];lines=text.splitlines(keepends=True)
 for edit in reversed(row['inverse']):a,z=edit['start'],edit['end'];assert''.join(lines[a:z])==edit['current'];lines[a:z]=edit['previous'].splitlines(keepends=True)
 b=''.join(lines).encode();assert h(b)==row['previous_sha256'];assert b==(R/'docs/evidence/main67107-fronts-6e45-20261017'/row['archive']).read_bytes()
module=load('format_input',D/'format-namespace.py');assert module.previous_namespace(R,actual.decode()).encode()==(D/'receiver.c874-before.rs.txt').read_bytes()
bcc=load('bcc_whole',R/'docs/evidence/mainbcc-ordinary-source-domain-20261017/check-source.py');old=bcc.previous_receiver(R,actual.decode());assert h(old.encode())=='6816d73e025e149033a83a0bce3f0f1bd3072563fad3a8b715395274a6a8eb2b'
negative=0
with tempfile.TemporaryDirectory(prefix='eight-source-controls-',dir='/Volumes/Semaprax/runtime-tmp')as tmp:
 root=Path(tmp);e=root/'docs/evidence/c874-supplement-custody-domains-20261017';e.mkdir(parents=True)
 for name in ['receiver-input.json',receiver['current_archive'],receiver['format_archive']]:shutil.copyfile(D/name,e/name)
 target=root/source;target.parent.mkdir(parents=True);target.write_bytes(actual)
 assert module.previous_namespace(root,actual.decode()).encode()==(D/'receiver.c874-before.rs.txt').read_bytes()
 for changed in [b'',actual[:-1],actual+b' ',actual.replace(b'\n',b'\r\n')]:
  target.write_bytes(changed)
  try:module.previous_namespace(root,changed.decode())
  except(AssertionError,OSError,UnicodeError):negative+=1
  else:raise AssertionError('raw physical mutation admitted')
 target.write_bytes(actual)
 for changed in ['',actual.decode()+' ',actual.decode()[:-1]]:
  try:module.previous_namespace(root,changed)
  except(AssertionError,OSError,UnicodeError):negative+=1
  else:raise AssertionError('provided source mutation admitted')
 archive=e/receiver['format_archive'];before=archive.read_bytes();archive.write_bytes(before+b' ')
 try:module.previous_namespace(root,actual.decode())
 except(AssertionError,OSError,UnicodeError):negative+=1
 else:raise AssertionError('archive mutation admitted')
 binder=load('source_binder',D/'bind-current-inputs.py');good=root/'content.txt';binder.retain(good,b'exact');a=good.stat();binder.retain(good,b'exact');b=good.stat();assert(a.st_ino,a.st_mtime_ns)==(b.st_ino,b.st_mtime_ns)
 for name,targetname in [('broken','missing'),('live','content.txt')]:
  path=root/name;path.symlink_to(root/targetname)
  try:binder.retain(path,b'exact')
  except(AssertionError,OSError):negative+=1
  else:raise AssertionError('symlink archive admitted')
 try:binder.retain(good,b'foreign')
 except AssertionError:negative+=1
 else:raise AssertionError('archive overwritten')
 assert good.read_bytes()==b'exact'
print(json.dumps({'state':'PASS_PURE_SOURCE_ONLY','paid_views':7,'complete_original_frozen_receivers':2,'complete_old_helper_inventory':115,'actual_raw_production_preflight_families':3,'complete_main_stages':9,'old_record_retained_fields':6,'derived_runtime_record_fields':7,'full_bcc_previous_namespace_predicates_pass':True,'pure_negative_controls':negative,'runtime':'UNEXECUTED'}))
