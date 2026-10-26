"""Closed final current source admission before the complete afa strict guard."""
from pathlib import Path
import hashlib,json,os,stat
R=Path(__file__).resolve().parents[3]
CARRIER='tests/workspace/semantic_graph/source_inventory.rs'
SCRATCH=('src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch.rs','src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs')
def bounded(path,length,digest):
 assert 0<=length<=262144 and hasattr(os,'O_NOFOLLOW')
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(length);assert len(b)==length and not f.read(1)
  assert identity(st)==identity(os.fstat(fd))==identity(os.lstat(path))
  assert hashlib.sha256(b).hexdigest()==digest;return b
 finally:os.close(fd)
_CONFIG_PATH=R/'docs/evidence/merge-2e92-browser-20261018/guard-config.json'
_CONFIG_LENGTH=258174
_CONFIG_SHA256='f38fdff65a47bb60e319e11754ecaecc0e41ddcefce591ac956d9cd3ada87657'
_CONFIG_RAW=bounded(_CONFIG_PATH,_CONFIG_LENGTH,_CONFIG_SHA256)
# Closed field-only transport. Legacy list semantics and every other CONFIG value remain unchanged.
_CLOSED_SOURCE_SCHEMA='semaprax.closed-prior-sources.prefixes.v1'
_CLOSED_SOURCE_PREFIXES=(('R','src/cleanup_plan/replay/'),('W','src/workspace_graph/'),('H','src/hir/'),('D','docs/evidence/'),('T','tests/'),('N','crates/semaprax-native-rust-interop-builder/'),('S','src/source_reflows/'),('M','src/source_reflows_merge/'))
def _closed_source_json(value):
 return json.dumps(value,ensure_ascii=False,separators=(',',':'),allow_nan=False).encode('utf-8')
def _closed_source_parts(value):
 assert isinstance(value,str) and value and '\\' not in value and '\x00' not in value
 assert not value.startswith('/') and all(x not in ('','.','..') for x in value.split('/'))
 value.encode('utf-8')
def _decode_configuration(value):
 field=value['closed_prior_sources']
 if isinstance(field,list):return value # Entire original legacy reader remains the default.
 assert isinstance(field,dict) and set(field)=={'schema','sources'} and field['schema']==_CLOSED_SOURCE_SCHEMA
 assert len(_closed_source_json(value))+1<=262144
 sources=field['sources'];assert isinstance(sources,list)
 expanded_size=2+max(0,len(sources)-1)
 for source in sources:
  assert isinstance(source,str)
  if ':' in source:
   assert len(source)>=3 and source[1]==':' and source.count(':')==1
   prefix=next((p for code,p in _CLOSED_SOURCE_PREFIXES if code==source[0]),None);assert prefix is not None
   _closed_source_parts(source[2:])
   # Prefixes are pairwise disjoint ASCII directory prefixes; every eligible path has one spelling.
   expanded_size+=len(_closed_source_json(source))+len(prefix.encode('utf-8'))-2
  else:
   _closed_source_parts(source)
   assert not any(source.startswith(prefix) for _,prefix in _CLOSED_SOURCE_PREFIXES),('noncanonical closed source',source)
   expanded_size+=len(_closed_source_json(source))
 # Complete expanded canonical CONFIG budget, including all opaque fields and final newline.
 # No expanded path is joined or cloned before the whole bound is established.
 assert len(_closed_source_json(value))-len(_closed_source_json(field))+expanded_size+1<=9753094
 decoded=[]
 for source in sources:
  if ':' in source:
   prefix=next(p for code,p in _CLOSED_SOURCE_PREFIXES if code==source[0]);decoded.append(prefix+source[2:])
  else:decoded.append(source)
 result=dict(value);result['closed_prior_sources']=decoded
 return result
def _encode_configuration(value):
 sources=value['closed_prior_sources'];assert isinstance(sources,list)
 assert len(_closed_source_json(value))+1<=9753094
 encoded=[]
 for source in sources:
  assert isinstance(source,str)
  encoded.append(next((code+':'+source[len(prefix):] for code,prefix in _CLOSED_SOURCE_PREFIXES if source.startswith(prefix)),source))
 result=dict(value);result['closed_prior_sources']={'schema':_CLOSED_SOURCE_SCHEMA,'sources':encoded}
 assert len(_closed_source_json(result))+1<=262144
 assert _decode_configuration(result)==value
 return result

CONFIG=_decode_configuration(json.loads(_CONFIG_RAW))
def _configuration_current():
 assert bounded(_CONFIG_PATH,_CONFIG_LENGTH,_CONFIG_SHA256)==_CONFIG_RAW

def read(pin):
 _configuration_current()
 return bounded(R/pin['archive'],pin['length'],pin['sha256'])
def packet_fmt433():
 _configuration_current()
 p=CONFIG['packet'];v=json.loads(bounded(R/p['path'],p['length'],p['sha256']))
 assert v['schema']=='semaprax.main-e177-final-whole-inputs.v1'and len(v['rows'])==CONFIG['rows']
 assert len(v['rows'])==len({x['source']for x in v['rows']})
 return v

def packet_before_overlay():
 base=packet_fmt433()
 extension=CONFIG['extension'];ext=json.loads(bounded(R/extension['path'],extension['length'],extension['sha256']))
 assert ext['schema']=='semaprax.fmt433-local133-extension.v1'
 assert ext['base_sha256']==CONFIG['packet']['sha256']
 assert ext['compiled_upgrades']==['src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs']
 rows=list(base['rows']);old={r['source']:r for r in base['rows']};positions={r['source']:i for i,r in enumerate(rows)};seen=set()
 for row in ext['rows']:
  p=row['source'];assert p not in seen;seen.add(p)
  if p in positions:rows[positions[p]]=row
  else:positions[p]=len(rows);rows.append(row)
 assert len(rows)==519 and len(positions)==519
 for prior,current in zip(base['rows'],rows):
  assert current['source']==prior['source']
  assert current['compiled']==prior['compiled']or(prior['compiled']is False and current['compiled']is True and prior['source']in ext['compiled_upgrades'])
  assert current['views'][:len(prior['views'])]==prior['views']
  if current['current']!=prior['current']:assert dict(name='whole-fmt433-current',**prior['current'])in current['views']
  restored=dict(current);restored['compiled']=prior['compiled'];restored['current']=prior['current'];restored['views']=prior['views']
  if 'fragments'in prior:restored['fragments']=prior['fragments']
  assert restored==prior
 logical=dict(base);logical['rows']=rows
 restored=dict(logical);restored['rows']=base['rows'];assert restored==base
 by={r['source']:r for r in rows};edges=0
 for row in rows:
  for child in row.get('fragments',[]):
   assert not child.get('fragments') and child['compiled']is True and child==by[child['source']];edges+=1
 assert edges==10
 local=json.loads(read(ext['whole_local133']));assert len(local['rows'])==133
 for prior in local['rows']:
  row=by[prior['source']];assert row['compiled']==prior['compiled']
  assert all(v in row['views']for v in prior['views'])
  assert row['current']==prior['current']or dict(name='whole-local133-current',**prior['current'])in row['views']
  a=dict(row);a.pop('fragments',None);a['current']=prior['current'];a['views']=prior['views'];assert a==prior
 return logical

_OVERLAY_NS = None
_OVERLAY_RAW = None
def _overlay():
 global _OVERLAY_NS, _OVERLAY_RAW
 _configuration_current()
 program=bounded(R/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py',61632,'a638a83ea4747ad0bbef2bfa3ddafb8d93bc5a4397ff70fee8333e84987df2b4')
 assert len(program)<=65536
 if _OVERLAY_NS is None:
  _OVERLAY_NS={'__file__':str(R/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py')}
  exec(compile(program,_OVERLAY_NS['__file__'],'exec'),_OVERLAY_NS);_OVERLAY_RAW=program
 assert program==_OVERLAY_RAW
 return _OVERLAY_NS['Overlay'](R,bounded,read,_configuration_current,packet_before_overlay,_successor_rows,lambda source: CONFIG['modules'][source])
def packet():
 return _overlay().compose(packet_before_overlay())

def _successor_rows_4a6eb():
 _configuration_current()
 p=CONFIG['previous_successor'];v=json.loads(bounded(R/p['path'],p['length'],p['sha256']))
 assert v['schema']=='semaprax.clippy-complete-source-successor.v1'and len(v['rows'])==105
 assert v['source_cap']==9753094 and len(v['rows'])==len({x['source']for x in v['rows']})
 assert v['base_packet']==CONFIG['packet']
 p=v['base_packet'];bounded(R/p['path'],p['length'],p['sha256'])
 return v['rows']
def _successor_rows():
 _configuration_current()
 p=CONFIG['source_successor'];v=json.loads(bounded(R/p['path'],p['length'],p['sha256']))
 old_rows=_successor_rows_4a6eb();p=CONFIG['previous_successor'];old=json.loads(bounded(R/p['path'],p['length'],p['sha256']))
 assert old_rows==old['rows']and v['previous_packet']==CONFIG['previous_successor']
 assert len(v['rows'])==121 and len(v['rows'])==len({x['source']for x in v['rows']})
 for before,after in zip(old_rows,v['rows']):
  assert before['source']==after['source']
  if before['current']!=after['current']:assert any({k:x for k,x in pin.items()if k!='name'}==before['current']for pin in after['views'])
  restored=dict(after);restored['current']=before['current']
  if 'views'in before:restored['views']=before['views']
  else:restored.pop('views',None)
  assert restored==before
 restored=dict(v);restored['rows']=old_rows;restored.pop('previous_packet');assert restored==old
 return v['rows']
def _successor_pin(path,length,digest):
 row=next((r for r in _successor_rows()if r['source']==path),None)
 if row is None:return None
 return next((p for p in [row['before'],row['current'],*row.get('views',[])]if(length,digest)==(p['length'],p['sha256'])),None)
def _stage_source_before_overlay(path,pin):
 row=next((r for r in _successor_rows()if r['source']==path),None)
 if row is None:return bounded(R/path,pin['length'],pin['sha256'])
 p=row['current'];actual=bounded(R/path,p['length'],p['sha256']);assert actual==read(p)
 if(pin['length'],pin['sha256'])==(p['length'],p['sha256']):return actual
 p=row['before']
 if(pin['length'],pin['sha256'])==(p['length'],p['sha256']):return read(p)
 views=[v for v in row.get('views',[])if(pin['length'],pin['sha256'])==(v['length'],v['sha256'])]
 assert views,('foreign successor tuple',path)
 result=read(views[0]);assert all(read(v)==result for v in views);return result

def _stage_source(path,pin):
 actual=_overlay().stage_source(path,pin)
 if actual is not None:return actual
 return _stage_source_before_overlay(path,pin)

def latest_source(path,length,digest):
 assert path in CONFIG['closed_prior_sources'],('foreign named source acquisition',path)
 row=next((r for r in packet()['rows']if r['source']==path),None)
 if row is None:return _stage_source(path,dict(length=length,sha256=digest))
 p=row['current'];actual=_stage_source(path,p);assert actual==read(p)
 for child in row.get('fragments',[]):
  assert not child.get('fragments')and child['compiled']is True
  pin=child['current'];value=_stage_source(child['source'],pin);assert value==read(pin)
 if(length,digest)==(p['length'],p['sha256']):return actual
 if _successor_pin(path,length,digest)is not None:return _stage_source(path,dict(length=length,sha256=digest))
 views=[v for v in row['views']if(length,digest)==(v['length'],v['sha256'])];assert views,('foreign prior tuple',path,length,digest)
 b=read(views[0]);assert all(read(v)==b for v in views);return b

def public_previous():
 p=CONFIG['public'];before=read(p);s=before.decode();program=_AUTHENTICATED_PROGRAM
 assert isinstance(program,bytes)and len(program)<=65536
 assert bounded(R/CONFIG['this_guard'],len(program),hashlib.sha256(program).hexdigest())==program
 for e in CONFIG['public_edits']:
  new=e['after'].replace('@GUARD_LENGTH@',str(len(program))).replace('@GUARD_SHA256@',hashlib.sha256(program).hexdigest())
  assert s.count(e['before'])==e['count'];s=s.replace(e['before'],new)
 actual=s.encode();assert bounded(R/p['path'],len(actual),hashlib.sha256(actual).hexdigest())==actual
 return before

def legacy_module():
 p=CONFIG['guard'];raw=bounded(R/p['path'],p['length'],p['sha256']);assert raw==read(p);s=raw.decode()
 edits=[
  ("actual=bounded(R/path,current['length'],current['sha256'])","actual=_latest_source(path,current['length'],current['sha256'])",1),
  ("actual=bounded(R/path,row['length'],row['sha256']).decode()","actual=_latest_source(path,row['length'],row['sha256']).decode()",1),
  ("for owner,pin in CONFIG['required_modules'].items():bounded(R/owner,pin['length'],pin['sha256'])","for owner,pin in CONFIG['required_modules'].items():_latest_source(owner,pin['length'],pin['sha256'])",1),
  ("assert bounded(R/CONFIG['public_path'],len(actual),hashlib.sha256(actual).hexdigest())==actual","assert _latest_public()==actual",1),
 ]
 for old,new,count in edits:assert s.count(old)==count;s=s.replace(old,new)
 restored=s
 for old,new,count in reversed(edits):assert restored.count(new)==count;restored=restored.replace(new,old)
 assert restored.encode()==raw
 ns=dict(__file__=str(R/p['path']),_AUTHENTICATED_PROGRAM=raw,_latest_source=latest_source,_latest_public=public_previous)
 exec(compile(s,ns['__file__'],'exec'),ns);return ns

def authenticate_current(path,supplied):
 assert path in CONFIG['carriers'];p=CONFIG['carriers'][path];actual=bounded(R/path,p['length'],p['sha256']).decode();assert supplied==actual
 for source,pin in CONFIG['modules'].items():bounded(R/source,pin['length'],pin['sha256'])
 t=CONFIG['table'];raw=bounded(R/t['path'],t['length'],t['sha256']);prior=read(t['prior']);expected=prior.decode().splitlines()
 for x in t['insertions']:assert x['path']not in expected;expected.insert(x['at'],x['path'])
 assert raw==('\n'.join(expected)+'\n').encode()and len(expected)==len(set(expected))==t['rows']
 finite_distinct_owners={('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs'): (15407, 178036), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs'): (15516, 178153), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs'): (17290, 179242), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs'): (17147, 180755), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs'): (16991, 181886), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs'): (16817, 183147), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs'): (16631, 183329), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/bounded_output/stage_debits/route_debits/json_codec_probe.rs'): (11761, 183523), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs'): (16475, 186405), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs'): (15623, 186569), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs'): (15791, 186745), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs'): (16138, 186923), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs'): (16306, 187099), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests/paired_segments.rs'): (15166, 152549), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_segments_before.rs'): (15054, 152671), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/independent_segments.rs'): (15961, 153027), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs'): (39209, 124034), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs'): (39327, 124160), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs'): (41200, 126142), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs'): (41048, 131221), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs'): (40883, 132433), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs'): (40700, 133784), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs'): (40505, 133975), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/bounded_output/stage_debits/route_debits/json_codec_probe.rs'): (38735, 134471), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs'): (40340, 137569), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs'): (39443, 137742), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs'): (39620, 137927), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs'): (39985, 138114), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs'): (40162, 138299), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests/paired_segments.rs'): (38951, 105208), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_segments_before.rs'): (38831, 105339), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/independent_segments.rs'): (39799, 105722)}
 for source in CONFIG['modules']:
  literal='include_str!("'+p['base']+source+'")'
  if (path,source) in finite_distinct_owners:
   expected=finite_distinct_owners[path,source]
   assert isinstance(expected,tuple)and len(expected)==2,('exact distinct-owner offsets',path,source)
   assert actual.count(literal)==len(expected)and all(actual.startswith(literal,at)for at in expected),('exact distinct-owner literals',path,source,expected)
  else:assert actual.count(literal)==1,('required/max-one literal owner',path,source)
 owner=CONFIG['workspace_owner'];latest_source(owner['path'],owner['length'],owner['sha256'])
 return actual

def accepted_guard_extensions(root):
 assert Path(root).resolve()==R
 ns=legacy_module();old=ns['accepted_guard_extensions'](R);verify=old['verify_guard'];extract=old['fragments']
 def prior(path):
  p=CONFIG['prior_carriers'][path];return latest_source(path,p['length'],p['sha256']).decode()
 def verify_guard(path,current,original,refresh=False):
  assert not refresh;authenticate_current(path,current);return verify(path,prior(path),original,False)
 def fragments(path,current,original):
  authenticate_current(path,current);return extract(path,prior(path),original)
 result=dict(old);result.update(verify_guard=verify_guard,fragments=fragments)
 return result

def source(parent,path,length,digest):
 # Exact existing definition reader keeps its original whole root stage.
 return legacy_module()['source'](parent,path,length,digest)
