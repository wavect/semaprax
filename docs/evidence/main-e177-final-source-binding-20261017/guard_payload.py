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
_CONFIG_LENGTH=250638
_CONFIG_SHA256='da7083f329ab0c627b51d3937d6d733bc96a816b1a13123279890f3388a8fe8b'
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
def _decode_configuration_v1(value):
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
def _encode_configuration_v1(value):
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

# Version two extends only the closed allowlist field; v1 canonical spellings
# and the complete legacy list reader remain separately executable above.
_CLOSED_SOURCE_SCHEMA_V2='semaprax.closed-prior-sources.prefixes.v2'
_CLOSED_SOURCE_PREFIXES_V2=_CLOSED_SOURCE_PREFIXES+(('L', 'src/loan_plan/'), ('V', 'src/source_verify/'), ('B', 'src/bounded_output/'), ('A', 'src/ast/program_source/'), ('P', 'src/project/'), ('C', 'src/loop_calls/'), ('F', 'src/cleanup_plan/renewal_profile/'), ('G', 'src/codegen/'))
def _decode_configuration_v2(value):
 field=value['closed_prior_sources']
 if isinstance(field,list):return value # Entire original legacy reader remains the default.
 assert isinstance(field,dict) and set(field)=={'schema','sources'} and field['schema']==_CLOSED_SOURCE_SCHEMA_V2
 assert len(_closed_source_json(value))+1<=262144
 sources=field['sources'];assert isinstance(sources,list)
 expanded_size=2+max(0,len(sources)-1)
 for source in sources:
  assert isinstance(source,str)
  if ':' in source:
   assert len(source)>=3 and source[1]==':' and source.count(':')==1
   prefix=next((p for code,p in _CLOSED_SOURCE_PREFIXES_V2 if code==source[0]),None);assert prefix is not None
   _closed_source_parts(source[2:])
   # Prefixes are pairwise disjoint ASCII directory prefixes; every eligible path has one spelling.
   expanded_size+=len(_closed_source_json(source))+len(prefix.encode('utf-8'))-2
  else:
   _closed_source_parts(source)
   assert not any(source.startswith(prefix) for _,prefix in _CLOSED_SOURCE_PREFIXES_V2),('noncanonical closed source',source)
   expanded_size+=len(_closed_source_json(source))
 # Complete expanded canonical CONFIG budget, including all opaque fields and final newline.
 # No expanded path is joined or cloned before the whole bound is established.
 assert len(_closed_source_json(value))-len(_closed_source_json(field))+expanded_size+1<=9753094
 decoded=[]
 for source in sources:
  if ':' in source:
   prefix=next(p for code,p in _CLOSED_SOURCE_PREFIXES_V2 if code==source[0]);decoded.append(prefix+source[2:])
  else:decoded.append(source)
 result=dict(value);result['closed_prior_sources']=decoded
 return result
def _encode_configuration_v2(value):
 sources=value['closed_prior_sources'];assert isinstance(sources,list)
 assert len(_closed_source_json(value))+1<=9753094
 encoded=[]
 for source in sources:
  assert isinstance(source,str)
  encoded.append(next((code+':'+source[len(prefix):] for code,prefix in _CLOSED_SOURCE_PREFIXES_V2 if source.startswith(prefix)),source))
 result=dict(value);result['closed_prior_sources']={'schema':_CLOSED_SOURCE_SCHEMA_V2,'sources':encoded}
 assert len(_closed_source_json(result))+1<=262144
 assert _decode_configuration_v2(result)==value
 return result
def _decode_configuration_legacy(value):
 field=value['closed_prior_sources']
 if isinstance(field,dict) and field.get('schema')==_CLOSED_SOURCE_SCHEMA_V2:
  return _decode_configuration_v2(value)
 return _decode_configuration_v1(value)

_MODULE_PREFIX_SCHEMA = 'semaprax.configuration.module-prefixes.v3'
_MODULE_CONFIG_WIRE_CAP = 262144
_MODULE_CONFIG_EXPANDED_CAP = 9753094


def _configuration_path_parts(packed, prefixes):
    if ':' not in packed:
        _closed_source_parts(packed)
        assert not any(packed.startswith(prefix) for _, prefix in prefixes)
        return '', packed
    code, suffix = packed.split(':', 1)
    assert len(code) == 1 and ':' not in suffix
    prefix = next((prefix for marker, prefix in prefixes if marker == code), None)
    assert prefix is not None
    _closed_source_parts(suffix)
    return prefix, suffix


def _configuration_closed_source_delta(field):
    if type(field) is list:
        return 0
    assert type(field) is dict and set(field) == {'schema', 'sources'}
    if field['schema'] == _CLOSED_SOURCE_SCHEMA:
        prefixes = _CLOSED_SOURCE_PREFIXES
    else:
        assert field['schema'] == _CLOSED_SOURCE_SCHEMA_V2
        prefixes = _CLOSED_SOURCE_PREFIXES_V2
    sources = field['sources']
    assert type(sources) is list
    expanded_size = 2 + max(0, len(sources) - 1)
    for packed in sources:
        assert type(packed) is str
        prefix, _ = _configuration_path_parts(packed, prefixes)
        expanded_size += len(_closed_source_json(packed))
        if prefix:
            expanded_size += len(prefix.encode('utf-8')) - 2
    return expanded_size - len(_closed_source_json(field))


def _decode_configuration_modules_v3(value):
    field = value['modules']
    # A list envelope is disjoint from every legacy module dictionary.
    assert type(field) is list and len(field) == 2
    schema, entries = field
    assert type(schema) is str and schema == _MODULE_PREFIX_SCHEMA and type(entries) is list
    wire_size = len(_closed_source_json(value)) + 1
    assert wire_size <= _MODULE_CONFIG_WIRE_CAP
    expanded_size = 2 + max(0, len(entries) - 1)
    seen = set()
    rows = []
    for pair in entries:
        assert type(pair) is list and len(pair) == 2
        packed, metadata = pair
        assert type(packed) is str
        prefix, suffix = _configuration_path_parts(packed, _CLOSED_SOURCE_PREFIXES_V2)
        # Fixed disjoint prefixes and rejected raw aliases give each path one spelling.
        assert packed not in seen
        seen.add(packed)
        expanded_size += len(_closed_source_json(packed)) + 1 + len(_closed_source_json(metadata))
        if prefix:
            expanded_size += len(prefix.encode('utf-8')) - 2
        rows.append((prefix, suffix, metadata))
    expanded_size += wire_size - len(_closed_source_json(field))
    expanded_size += _configuration_closed_source_delta(value['closed_prior_sources'])
    assert expanded_size <= _MODULE_CONFIG_EXPANDED_CAP
    result = dict(_decode_configuration_legacy(value))
    result['modules'] = {prefix + suffix: metadata for prefix, suffix, metadata in rows}
    return result


def _decode_configuration(value):
    if 'modules' not in value or type(value['modules']) is dict:
        return _decode_configuration_legacy(value)
    return _decode_configuration_modules_v3(value)


def _encode_configuration(value):
    if 'modules' not in value:
        return _encode_configuration_v2(value)
    modules = value['modules']
    assert type(modules) is dict
    assert len(_closed_source_json(value)) + 1 <= _MODULE_CONFIG_EXPANDED_CAP
    entries = []
    for path, metadata in modules.items():
        _closed_source_parts(path)
        assert ':' not in path
        packed = next(
            (code + ':' + path[len(prefix):] for code, prefix in _CLOSED_SOURCE_PREFIXES_V2
             if path.startswith(prefix)),
            path,
        )
        entries.append([packed, metadata])
    encoded = dict(value)
    encoded['modules'] = [_MODULE_PREFIX_SCHEMA, entries]
    # Pack module keys first so a v2-only intermediate need not fit the wire cap.
    wire = _encode_configuration_v2(encoded)
    assert _decode_configuration(wire) == value
    return wire


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
 program=bounded(R/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py',65060,'225df5cf34c3a82c4b9a7b6aee90cf083b22e20d7f4d302522c28b3498dfce11')
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
 finite_distinct_owners={('tests/hir_module_boundaries.rs', 'src/cleanup_plan/replay/expression_facts/arena/prefix/compact.rs'): (106791, 144054), ('tests/hir_module_boundaries.rs', 'src/cleanup_plan/replay/expression_facts/arena/prefix/compact_tests.rs'): (106875, 144138), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_v2.rs'): (23273, 152555), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs'): (15596, 179557), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs'): (15705, 179674), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_fragments_v3.rs'): (23107, 180057), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/call_carriers.rs'): (23554, 251526), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs'): (17479, 180763), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs'): (22227, 181781), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs'): (17336, 182276), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_event_tests.rs'): (22113, 183284), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs'): (17180, 183407), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads.rs'): (21662, 183807), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_tests.rs'): (21887, 183920), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs'): (17006, 184668), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs'): (16820, 184850), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/bounded_output/stage_debits/route_debits/json_codec_probe.rs'): (11950, 185044), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_link_composition_tests.rs'): (21997, 187186), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs'): (16664, 187926), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs'): (15812, 188090), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs'): (15980, 188266), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs'): (16327, 188444), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs'): (16495, 188620), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_compiled.rs'): (23030, 152626), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests/paired_segments.rs'): (15355, 153837), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_segments_before.rs'): (15243, 153959), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/independent_segments.rs'): (16150, 154315), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/source_reflows_merge/boxed_inputs.rs'): (22967, 195986), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/call_carriers_joint.rs'): (23633, 251605), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs'): (23188, 246919), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs'): (24246, 247101), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs'): (24347, 247910), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs'): (22417, 244149), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs'): (22740, 244348), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/path_join/frontier_tests/custody.rs'): (22335, 245363), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/continuation_storage.rs'): (20807, 33854), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/continuation_storage/reached.rs'): (21053, 257063), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/continuation_storage/reached_tests.rs'): (21131, 257226), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/initial_report/build.rs'): (161753, 194487), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/retained_scratch.rs'): (23807, 248561), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/before903.rs'): (23889, 249131), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs'): (23985, 249960), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs'): (24116, 250091), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs'): (24525, 261118), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/diagnostics/borrowed_help_tests.rs'): (23477, 238434), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/factored/oracle_order.rs'): (21215, 53338), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/source_reflows_merge/ordinary_plan_inputs.rs'): (23344, 49258), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/identity_prebound_tests.rs'): (23718, 147227), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/workspace_graph/expected_projection/uncached_peak.rs'): (24446, 261039), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/expression_facts/arena/prefix/compact.rs'): (24881, 50583), ('tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/expression_facts/arena/prefix/compact_tests.rs'): (24968, 50670), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_v2.rs'): (46344, 106338), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs'): (39380, 126572), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs'): (39498, 126698), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_fragments_v3.rs'): (46160, 127117), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/call_carriers.rs'): (46581, 127475), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs'): (41371, 128680), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs'): (45190, 133319), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs'): (41219, 133850), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_event_tests.rs'): (45067, 134930), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs'): (41054, 135062), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads.rs'): (44580, 135489), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_tests.rs'): (44823, 135611), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs'): (40871, 136413), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs'): (40676, 136604), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/bounded_output/stage_debits/route_debits/json_codec_probe.rs'): (38906, 137100), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_link_composition_tests.rs'): (44942, 139404), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs'): (40511, 140198), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs'): (39614, 140371), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs'): (39791, 140556), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs'): (40156, 140743), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs'): (40333, 140928), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_compiled.rs'): (46074, 106418), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests/paired_segments.rs'): (39122, 107746), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_segments_before.rs'): (39002, 107877), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/independent_segments.rs'): (39970, 108260), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_pins_v10.rs'): (54180, 143354), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/source_reflows_merge/boxed_inputs.rs'): (46002, 74676), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/call_carriers_joint.rs'): (46669, 127563), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs'): (46250, 62266), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs'): (47336, 62466), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs'): (47446, 63347), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs'): (45398, 59226), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs'): (45748, 59443), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/path_join/frontier_tests/custody.rs'): (45307, 60566), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/hir.rs'): (153288, 187869), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/retained_scratch.rs'): (46861, 129527), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/before903.rs'): (46952, 130321), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs'): (47057, 131213), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs'): (47197, 131353), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs'): (47642, 146624), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/diagnostics/borrowed_help_tests.rs'): (46495, 130140), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/expected_projection/uncached_peak.rs'): (47554, 146536), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/expression_facts/arena/prefix/compact.rs'): (48034, 231895), ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/expression_facts/arena/prefix/compact_tests.rs'): (48130, 231991)}
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
