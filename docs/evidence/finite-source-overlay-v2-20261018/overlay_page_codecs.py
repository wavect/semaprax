V10_SCHEMA = 'semaprax.finite-source-overlay-page.v10'
V10_PREFIX_LIMIT = 32
def v10_archive_slots(page, transform):
 result=copy.deepcopy(page)
 def pin_slot(value,named):
  if type(value) is not list:return
  count=3 if named else 2
  if len(value)==count+1:value[count]=transform(value[count])
 def row_slots(row):
  assert type(row) is dict, 'v10 literal row'
  if 'current' in row:pin_slot(row['current'],False)
  if 'views' in row:
   assert type(row['views']) is list, 'v10 views'
   for view in row['views']:pin_slot(view,True)
 def fragment_slots(fragments):
  if fragments is None:return
  assert type(fragments) is list, 'v10 fragments'
  for fragment in fragments:
   assert type(fragment) is dict, 'v10 fragment row'
   if 'canonical_source' not in fragment:row_slots(fragment)
 for group in ('replacements','additions'):
  assert type(result[group]) is list, 'v10 rows'
  for row in result[group]:
   assert type(row) is dict, 'v10 row'
   if 'old519_index' in row:
    assert group=='replacements' and keys(row,('old519_index',)), 'v10 indexed row'
    reference=row['old519_index']
    assert type(reference) is list and len(reference)==4, 'v10 old519 arity'
    pin_slot(reference[1],False)
    assert type(reference[2]) is list, 'v10 appended views'
    for view in reference[2]:pin_slot(view,True)
    fragment_slots(reference[3])
   else:
    row_slots(row)
    if 'fragments' in row:fragment_slots(row['fragments'])
 return result

def v10_prefix_table(page):
 prefixes=page.get('archive_prefixes')
 assert type(prefixes) is list and 0<len(prefixes)<=V10_PREFIX_LIMIT, 'v10 archive prefix count'
 assert all(type(prefix) is str and prefix.endswith('/') and normal(prefix[:-1]) for prefix in prefixes), 'v10 archive prefix directory'
 assert len(set(prefixes))==len(prefixes), 'v10 archive prefix directory'
 return prefixes

def expand_v10_archive_prefixes(page):
 assert page['schema']==V10_SCHEMA, 'v10 schema'
 prefixes=v10_prefix_table(page);uses=[0]*len(prefixes)
 def expand(value):
  if type(value) is str:
   assert not any(value.startswith(prefix) for prefix in prefixes), 'v10 uninterned archive'
   return value
  assert type(value) is list and len(value)==2, 'v10 archive reference'
  index,suffix=value
  assert type(index) is int and 0<=index<len(prefixes), 'v10 archive prefix index'
  assert type(suffix) is str, 'v10 archive suffix type'
  path=prefixes[index]+suffix
  assert normal(path), 'v10 expanded archive path'
  longest=max((len(prefix),position) for position,prefix in enumerate(prefixes) if path.startswith(prefix))[1]
  assert longest==index, 'v10 noncanonical archive prefix'
  uses[index]+=1
  return path
 result=v10_archive_slots(page,expand)
 assert all(count>=2 for count in uses), 'v10 unused archive prefix'
 del result['archive_prefixes'];result['schema']=V9_SCHEMA
 return result

def v10_candidate_prefixes(page):
 counts={}
 def collect(value):
  if type(value) is not str:return value
  parts=value.split('/')
  for end in range(1,len(parts)):
   prefix='/'.join(parts[:end])+'/'
   if normal(prefix[:-1]):counts[prefix]=counts.get(prefix,0)+1
  return value
 v10_archive_slots(page,collect)
 return sorted(prefix for prefix,count in counts.items() if count>=2)

def encode_v10_with_prefixes(page,prefixes):
 uses=[0]*len(prefixes)
 def encode(value):
  assert type(value) is str, 'v10 V9 archive type'
  choices=[(len(prefix),position,prefix) for position,prefix in enumerate(prefixes) if value.startswith(prefix)]
  if not choices:return value
  _,position,prefix=max(choices);uses[position]+=1
  return [position,value[len(prefix):]]
 result=v10_archive_slots(page,encode)
 assert all(count>=2 for count in uses), 'v10 unused archive prefix'
 result['schema']=V10_SCHEMA;result['archive_prefixes']=list(prefixes)
 return result

def encode_v10_archive_prefixes(page,page_cap=PAGE_CAP,headroom=0):
 assert page['schema']==V9_SCHEMA, 'v10 input schema'
 candidates=v10_candidate_prefixes(page);selected=[];size=len(canonical(page))
 while len(selected)<V10_PREFIX_LIMIT:
  choices=[]
  for prefix in candidates:
   if prefix in selected:continue
   trial=selected+[prefix]
   try:encoded=encode_v10_with_prefixes(page,trial)
   except AssertionError:continue
   choices.append((len(canonical(encoded)),prefix,encoded))
  assert choices, 'v10 no useful archive prefixes'
  next_size,prefix,encoded=min(choices,key=lambda row:row[:2])
  assert next_size<size, 'v10 archive prefixes do not reduce page'
  selected.append(prefix);size=next_size
  if size<=page_cap-headroom:break
 assert size<=page_cap-headroom, 'v10 fixed page cap'
 restored=expand_v10_archive_prefixes(encoded)
 assert canonical(restored)==canonical(page), 'v10 ENTIRE V9 page inverse'
 return encoded

V11_SCHEMA = 'semaprax.finite-source-overlay-page.v11'
def v11_digest(value, encode=False):
 assert type(value) is str, 'v11 digest type'
 if encode:
  assert len(value)==64 and all(c in '0123456789abcdef' for c in value), 'v11 hex digest'
  return base64.urlsafe_b64encode(bytes.fromhex(value)).decode('ascii').rstrip('=')
 assert len(value)==43 and all(c in 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_' for c in value), 'v11 digest alphabet or length'
 raw=base64.b64decode(value+'=',altchars=b'-_',validate=True)
 assert len(raw)==32 and base64.urlsafe_b64encode(raw).decode('ascii').rstrip('=')==value, 'v11 canonical digest'
 return raw.hex()

def transform_v11_digest_slots(page, encode=False):
 # Only positional V9 pin digests, one row/fragment level. Object pins,
 # unknown metadata, archive references and source/view names stay literal.
 result=copy.deepcopy(page)
 def pin_slot(value,named):
  if type(value) is not list:return
  count=3 if named else 2;offset=1 if named else 0
  assert len(value) in (count,count+1), 'v11 pin arity'
  if named:assert type(value[0]) is str, 'v11 view name'
  assert type(value[offset]) is int and 0<=value[offset]<=SOURCE_CAP, 'v11 length'
  value[offset+1]=v11_digest(value[offset+1],encode)
 def views(value):
  assert type(value) is list, 'v11 views'
  for view in value:pin_slot(view,True)
 def row(value):
  assert type(value) is dict, 'v11 literal row'
  if 'current' in value:pin_slot(value['current'],False)
  if 'views' in value:views(value['views'])
 def fragments(value):
  if value is None:return
  assert type(value) is list, 'v11 fragments'
  for fragment in value:
   assert type(fragment) is dict, 'v11 fragment row'
   if 'canonical_source' not in fragment:row(fragment)
 for group in ('replacements','additions'):
  assert type(result[group]) is list, 'v11 rows'
  for value in result[group]:
   assert type(value) is dict, 'v11 row'
   if 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v11 indexed row'
    reference=value['old519_index']
    assert type(reference) is list and len(reference)==4, 'v11 old519 arity'
    pin_slot(reference[1],False);views(reference[2]);fragments(reference[3])
   else:
    row(value)
    if 'fragments' in value:fragments(value['fragments'])
 return result

def expand_v11_digest_pins(page):
 assert page['schema']==V11_SCHEMA, 'v11 schema'
 result=transform_v11_digest_slots(page);result['schema']=V10_SCHEMA
 return result

def encode_v11_digest_pins(page):
 assert page['schema']==V10_SCHEMA, 'v11 input schema'
 result=transform_v11_digest_slots(page,True);result['schema']=V11_SCHEMA
 assert canonical(expand_v11_digest_pins(result))==canonical(page), 'v11 ENTIRE V10 page inverse'
 assert len(canonical(result))<=PAGE_CAP, 'v11 same one-page cap'
 return result

V12_SCHEMA = 'semaprax.finite-source-overlay-page.v12'
V12_KEYS = ('schema','index','total_pages','predecessor','replacements','additions','archive_prefixes')
def v12_view_slots(page, convert):
 # Exactly V11's named positional pin slots; no recursive metadata rewrite.
 def views(value):
  assert type(value) is list, 'v12 views'
  for view in value:
   if type(view) is dict:continue
   assert type(view) is list and len(view) in (3,4), 'v12 named pin arity'
   view[0]=convert(view[0])
 def row(value):
  assert type(value) is dict, 'v12 literal row'
  if 'views' in value:views(value['views'])
 def fragments(value):
  if value is None:return
  assert type(value) is list, 'v12 fragments'
  for fragment in value:
   assert type(fragment) is dict, 'v12 fragment row'
   if 'canonical_source' not in fragment:row(fragment)
 for group in ('replacements','additions'):
  assert type(page[group]) is list, 'v12 rows'
  for value in page[group]:
   assert type(value) is dict, 'v12 row'
   if 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v12 indexed row'
    ref=value['old519_index']
    assert type(ref) is list and len(ref)==4, 'v12 old519 arity'
    views(ref[2]);fragments(ref[3])
   else:
    row(value)
    if 'fragments' in value:fragments(value['fragments'])

def expand_v12_view_labels(page):
 assert keys(page,(*V12_KEYS,'view_labels')) and page['schema']==V12_SCHEMA, 'v12 envelope'
 assert len(canonical(page))<=PAGE_CAP, 'v12 same one-page cap'
 declarations=page['view_labels']
 assert type(declarations) is list, 'v12 dictionary'
 labels=[];unique=set()
 for index,declaration in enumerate(declarations):
  assert type(declaration) is list and len(declaration)==2, 'v12 declaration arity'
  assert type(declaration[0]) is int and declaration[0]==index, 'v12 duplicate or noncontiguous index'
  label=declaration[1]
  assert type(label) is str and label not in unique, 'v12 duplicate or nonstring label'
  unique.add(label);labels.append(label)
 seen=set();expanded=0
 def validate(index):
  nonlocal expanded
  assert type(index) is int and 0<=index<len(labels), 'v12 unknown label index'
  if index not in seen:
   assert index==len(seen), 'v12 noncanonical first-use order'
   seen.add(index)
  expanded+=len(canonical(labels[index]))-1
  assert expanded<=SOURCE_CAP, 'v12 expanded label bound'
  return index
 # First pass bounds expansion without publishing even one expanded label.
 result=copy.deepcopy(page);v12_view_slots(result,validate)
 assert len(seen)==len(labels), 'v12 unreferenced declaration'
 v12_view_slots(result,lambda index:labels[index])
 del result['view_labels'];result['schema']=V11_SCHEMA
 return result

def encode_v12_view_labels(page):
 assert keys(page,V12_KEYS) and page['schema']==V11_SCHEMA, 'v12 input envelope'
 result=copy.deepcopy(page);labels=[];indices={}
 def encode(label):
  assert type(label) is str, 'v12 V11 named label'
  if label not in indices:indices[label]=len(labels);labels.append(label)
  return indices[label]
 v12_view_slots(result,encode)
 result['schema']=V12_SCHEMA;result['view_labels']=[[i,label]for i,label in enumerate(labels)]
 assert len(canonical(result))<=PAGE_CAP, 'v12 same one-page cap'
 assert canonical(expand_v12_view_labels(result))==canonical(page), 'v12 ENTIRE V11 page inverse'
 return result

# V13 transports only repeated digests in V12's known positional pin slots.
V13_SCHEMA = 'semaprax.finite-source-overlay-page.v13'
def v13_digest_slots(page, convert):
 def pin(value,named):
  if type(value) is dict:return
  offset=int(named)
  assert type(value) is list and len(value) in (offset+2,offset+3), 'v13 pin arity'
  assert type(value[offset]) is int and 0<=value[offset]<=SOURCE_CAP, 'v13 pin length'
  value[offset+1]=convert(value[offset+1])
 def row(value):
  assert type(value) is dict, 'v13 literal row'
  if 'current' in value:pin(value['current'],False)
  if 'views' in value:
   assert type(value['views']) is list, 'v13 views'
   for view in value['views']:pin(view,True)
 def fragments(value):
  if value is None:return
  assert type(value) is list, 'v13 fragments'
  for fragment in value:
   assert type(fragment) is dict, 'v13 fragment row'
   if 'canonical_source' not in fragment:row(fragment)
 for group in ('replacements','additions'):
  assert type(page[group]) is list, 'v13 rows'
  for value in page[group]:
   assert type(value) is dict, 'v13 row'
   if 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v13 indexed row'
    ref=value['old519_index']
    assert type(ref) is list and len(ref)==4, 'v13 old519 arity'
    pin(ref[1],False)
    assert type(ref[2]) is list, 'v13 views'
    for view in ref[2]:pin(view,True)
    fragments(ref[3])
   else:
    row(value)
    if 'fragments' in value:fragments(value['fragments'])

def expand_v13_digest_table(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table')) and page['schema']==V13_SCHEMA, 'v13 envelope'
 assert len(canonical(page))<=PAGE_CAP, 'v13 same one-page cap'
 declarations=page['digest_table']
 assert type(declarations) is list, 'v13 dictionary'
 unique=set()
 for value in declarations:
  v11_digest(value)
  assert value not in unique, 'v13 duplicate declaration'
  unique.add(value)
 result=copy.deepcopy(page);del result['digest_table'];result['schema']=V12_SCHEMA
 expanded=len(canonical(result));uses=[0]*len(declarations);next_index=0;literals=set()
 def index(value):
  assert type(value) is int and 0<=value<len(declarations), 'v13 unknown digest index'
  return value
 def validate(value):
  nonlocal expanded,next_index
  if type(value) is str:
   v11_digest(value)
   assert value not in unique and value not in literals, 'v13 uninterned or aliased digest'
   literals.add(value)
  else:
   position=index(value)
   if uses[position]==0:
    assert position==next_index, 'v13 noncanonical first-use order'
    next_index+=1
   uses[position]+=1;expanded+=46-len(canonical(value))
  assert expanded<=SOURCE_CAP, 'v13 expanded bound'
  return value
 v13_digest_slots(result,validate)
 assert all(count>=2 for count in uses), 'v13 declaration requires repeated use'
 v13_digest_slots(result,lambda value:value if type(value)is str else declarations[index(value)])
 assert len(canonical(result))==expanded and expanded<=SOURCE_CAP, 'v13 expanded bound'
 return result

def encode_v13_digest_table(page):
 assert keys(page,(*V12_KEYS,'view_labels')) and page['schema']==V12_SCHEMA, 'v13 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v13 logical input bound'
 result=copy.deepcopy(page);counts={}
 def count(value):
  v11_digest(value);counts[value]=counts.get(value,0)+1
  return value
 v13_digest_slots(result,count)
 indices={};table=[]
 def encode(value):
  if counts[value]<2:return value
  if value not in indices:indices[value]=len(table);table.append(value)
  return indices[value]
 v13_digest_slots(result,encode)
 result['schema']=V13_SCHEMA;result['digest_table']=table
 assert len(canonical(result))<=PAGE_CAP, 'v13 same one-page cap'
 assert canonical(expand_v13_digest_table(result))==canonical(page), 'v13 ENTIRE V12 page inverse'
 return result

# Internal V13-only logical frontier; public V12 remains byte-identical above.
def _expand_v12_logical_view_labels(page):
 assert keys(page,(*V12_KEYS,'view_labels')) and page['schema']==V12_SCHEMA, 'v12 envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v12 same one-page cap'
 declarations=page['view_labels']
 assert type(declarations) is list, 'v12 dictionary'
 labels=[];unique=set()
 for index,declaration in enumerate(declarations):
  assert type(declaration) is list and len(declaration)==2, 'v12 declaration arity'
  assert type(declaration[0]) is int and declaration[0]==index, 'v12 duplicate or noncontiguous index'
  label=declaration[1]
  assert type(label) is str and label not in unique, 'v12 duplicate or nonstring label'
  unique.add(label);labels.append(label)
 seen=set();expanded=0
 def validate(index):
  nonlocal expanded
  assert type(index) is int and 0<=index<len(labels), 'v12 unknown label index'
  if index not in seen:
   assert index==len(seen), 'v12 noncanonical first-use order'
   seen.add(index)
  expanded+=len(canonical(labels[index]))-1
  assert expanded<=SOURCE_CAP, 'v12 expanded label bound'
  return index
 # First pass bounds expansion without publishing even one expanded label.
 result=copy.deepcopy(page);v12_view_slots(result,validate)
 assert len(seen)==len(labels), 'v12 unreferenced declaration'
 v12_view_slots(result,lambda index:labels[index])
 del result['view_labels'];result['schema']=V11_SCHEMA
 return result

# Private V14 intermediate only; public V13 functions above retain exact caps.
def _expand_v13_logical_digest_table(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table')) and page['schema']==V13_SCHEMA, 'v13 envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v13 same one-page cap'
 declarations=page['digest_table']
 assert type(declarations) is list, 'v13 dictionary'
 unique=set()
 for value in declarations:
  v11_digest(value)
  assert value not in unique, 'v13 duplicate declaration'
  unique.add(value)
 result=copy.deepcopy(page);del result['digest_table'];result['schema']=V12_SCHEMA
 expanded=len(canonical(result));uses=[0]*len(declarations);next_index=0;literals=set()
 def index(value):
  assert type(value) is int and 0<=value<len(declarations), 'v13 unknown digest index'
  return value
 def validate(value):
  nonlocal expanded,next_index
  if type(value) is str:
   v11_digest(value)
   assert value not in unique and value not in literals, 'v13 uninterned or aliased digest'
   literals.add(value)
  else:
   position=index(value)
   if uses[position]==0:
    assert position==next_index, 'v13 noncanonical first-use order'
    next_index+=1
   uses[position]+=1;expanded+=46-len(canonical(value))
  assert expanded<=SOURCE_CAP, 'v13 expanded bound'
  return value
 v13_digest_slots(result,validate)
 assert all(count>=2 for count in uses), 'v13 declaration requires repeated use'
 v13_digest_slots(result,lambda value:value if type(value)is str else declarations[index(value)])
 assert len(canonical(result))==expanded and expanded<=SOURCE_CAP, 'v13 expanded bound'
 return result


def _encode_v13_logical_digest_table(page):
 assert keys(page,(*V12_KEYS,'view_labels')) and page['schema']==V12_SCHEMA, 'v13 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v13 logical input bound'
 result=copy.deepcopy(page);counts={}
 def count(value):
  v11_digest(value);counts[value]=counts.get(value,0)+1
  return value
 v13_digest_slots(result,count)
 indices={};table=[]
 def encode(value):
  if counts[value]<2:return value
  if value not in indices:indices[value]=len(table);table.append(value)
  return indices[value]
 v13_digest_slots(result,encode)
 result['schema']=V13_SCHEMA;result['digest_table']=table
 assert len(canonical(result))<=SOURCE_CAP, 'v13 same one-page cap'
 assert canonical(_expand_v13_logical_digest_table(result))==canonical(page), 'v13 ENTIRE V12 page inverse'
 return result


# V14 interns whole positional pin tails. Object pins/unknown metadata are opaque.
V14_SCHEMA = 'semaprax.finite-source-overlay-page.v14'
def v14_pin_slots(page, convert):
 def pin(value,named):
  if type(value) is dict:return value
  assert type(value) is list, 'v14 pin arity'
  offset=int(named)
  assert len(value) in (offset+1,offset+2,offset+3), 'v14 pin arity'
  return value[:offset]+convert(value[offset:])
 def row(value):
  assert type(value) is dict, 'v14 literal row'
  if 'current' in value:value['current']=pin(value['current'],False)
  if 'views' in value:
   assert type(value['views']) is list, 'v14 views'
   value['views']=[pin(view,True) for view in value['views']]
 def fragments(value):
  if value is None:return
  assert type(value) is list, 'v14 fragments'
  for fragment in value:
   assert type(fragment) is dict, 'v14 fragment row'
   if 'canonical_source' not in fragment:row(fragment)
 for group in ('replacements','additions'):
  assert type(page[group]) is list, 'v14 rows'
  for value in page[group]:
   assert type(value) is dict, 'v14 row'
   if 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v14 indexed row'
    ref=value['old519_index']
    assert type(ref) is list and len(ref)==4, 'v14 old519 arity'
    ref[1]=pin(ref[1],False)
    assert type(ref[2]) is list, 'v14 views'
    ref[2]=[pin(view,True) for view in ref[2]]
    fragments(ref[3])
   else:
    row(value)
    if 'fragments' in value:fragments(value['fragments'])

def _v14_tail(value):
 assert type(value) is list and len(value) in (2,3), 'v14 tail arity'
 assert type(value[0]) is int and 0<=value[0]<=SOURCE_CAP, 'v14 tail length'
 # Full digest semantics are checked by logical V13; archives by unchanged older readers.
 assert type(value[1]) in (int,str), 'v14 tail digest type'
 return canonical(value)

def expand_v14_pin_tails(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V14_SCHEMA, 'v14 envelope'
 assert len(canonical(page))<=PAGE_CAP, 'v14 same one-page cap'
 declarations=page['pin_tails']
 assert type(declarations) is list, 'v14 dictionary'
 unique=set()
 for value in declarations:
  key=_v14_tail(value)
  assert key not in unique, 'v14 duplicate declaration'
  unique.add(key)
 result=copy.deepcopy(page);del result['pin_tails'];result['schema']=V13_SCHEMA
 uses=[0]*len(declarations);next_index=0;literals=set();expanded=len(canonical(result))
 def validate(value):
  nonlocal next_index,expanded
  if len(value)==1:
   position=value[0]
   assert type(position) is int and 0<=position<len(declarations), 'v14 unknown tail index'
   if uses[position]==0:
    assert position==next_index, 'v14 noncanonical first-use order'
    next_index+=1
   uses[position]+=1
   expanded+=len(canonical(declarations[position]))-len(canonical(value))
  else:
   key=_v14_tail(value)
   assert key not in unique and key not in literals, 'v14 uninterned or aliased tail'
   literals.add(key)
  assert expanded<=SOURCE_CAP, 'v14 expanded bound'
  return value
 v14_pin_slots(result,validate)
 assert all(count>=2 for count in uses), 'v14 declaration requires repeated use'
 assert expanded<=SOURCE_CAP, 'v14 expanded bound'
 v14_pin_slots(result,lambda value:copy.deepcopy(declarations[value[0]]) if len(value)==1 else value)
 assert len(canonical(result))==expanded, 'v14 expanded bound'
 _expand_v13_logical_digest_table(result)
 return result

def encode_v14_pin_tails(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table')) and page['schema']==V13_SCHEMA, 'v14 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v14 logical input bound'
 _expand_v13_logical_digest_table(page)
 result=copy.deepcopy(page);counts={}
 def count(value):
  key=_v14_tail(value);counts[key]=counts.get(key,0)+1
  return value
 v14_pin_slots(result,count)
 table=[];indices={}
 def encode(value):
  key=_v14_tail(value)
  if counts[key]<2:return value
  if key not in indices:indices[key]=len(table);table.append(copy.deepcopy(value))
  return [indices[key]]
 v14_pin_slots(result,encode)
 result['schema']=V14_SCHEMA;result['pin_tails']=table
 assert len(canonical(result))<=PAGE_CAP, 'v14 same one-page cap'
 assert canonical(expand_v14_pin_tails(result))==canonical(page), 'v14 ENTIRE V13 page inverse'
 return result

def encode_v14(page):
 return encode_v14_pin_tails(_encode_v13_logical_digest_table(page))

# Private V15 logical frontier; public V14 functions and wire caps remain exact.
def _expand_v14_logical_pin_tails(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V14_SCHEMA, 'v14 envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v14 same one-page cap'
 declarations=page['pin_tails']
 assert type(declarations) is list, 'v14 dictionary'
 unique=set()
 for value in declarations:
  key=_v14_tail(value)
  assert key not in unique, 'v14 duplicate declaration'
  unique.add(key)
 result=copy.deepcopy(page);del result['pin_tails'];result['schema']=V13_SCHEMA
 uses=[0]*len(declarations);next_index=0;literals=set();expanded=len(canonical(result))
 def validate(value):
  nonlocal next_index,expanded
  if len(value)==1:
   position=value[0]
   assert type(position) is int and 0<=position<len(declarations), 'v14 unknown tail index'
   if uses[position]==0:
    assert position==next_index, 'v14 noncanonical first-use order'
    next_index+=1
   uses[position]+=1
   expanded+=len(canonical(declarations[position]))-len(canonical(value))
  else:
   key=_v14_tail(value)
   assert key not in unique and key not in literals, 'v14 uninterned or aliased tail'
   literals.add(key)
  assert expanded<=SOURCE_CAP, 'v14 expanded bound'
  return value
 v14_pin_slots(result,validate)
 assert all(count>=2 for count in uses), 'v14 declaration requires repeated use'
 assert expanded<=SOURCE_CAP, 'v14 expanded bound'
 v14_pin_slots(result,lambda value:copy.deepcopy(declarations[value[0]]) if len(value)==1 else value)
 assert len(canonical(result))==expanded, 'v14 expanded bound'
 _expand_v13_logical_digest_table(result)
 return result


def _encode_v14_logical_pin_tails(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table')) and page['schema']==V13_SCHEMA, 'v14 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v14 logical input bound'
 _expand_v13_logical_digest_table(page)
 result=copy.deepcopy(page);counts={}
 def count(value):
  key=_v14_tail(value);counts[key]=counts.get(key,0)+1
  return value
 v14_pin_slots(result,count)
 table=[];indices={}
 def encode(value):
  key=_v14_tail(value)
  if counts[key]<2:return value
  if key not in indices:indices[key]=len(table);table.append(copy.deepcopy(value))
  return [indices[key]]
 v14_pin_slots(result,encode)
 result['schema']=V14_SCHEMA;result['pin_tails']=table
 assert len(canonical(result))<=SOURCE_CAP, 'v14 same one-page cap'
 assert canonical(_expand_v14_logical_pin_tails(result))==canonical(page), 'v14 ENTIRE V13 page inverse'
 return result


# Versioned V15 registry: source slots only; this is not a source allowlist.
V15_SCHEMA = 'semaprax.finite-source-overlay-page.v15'
V15_CODES = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh'
V15_SOURCE_PREFIXES = ('src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/', 'src/cleanup_plan/replay/', 'src/workspace_graph/expected_projection/', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/', 'src/hir/validation/frame_reuse/', 'src/workspace_graph/expected_projection/shared_local_callees/', 'src/cleanup_plan/replay/path_join/', 'src/cleanup_plan/replay/ordinary_plan_history/completed_fork/', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/', 'src/source_reflows_merge/', 'src/cleanup_plan/replay/expression_facts/', 'tests/wasm/scalar_browser_ci_contract/', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/', 'src/cleanup_plan/replay/expression_facts/receiving/retained/', 'src/cleanup_plan/replay/ordinary_plan_history/', 'src/hir/inspection/paid_nul/', 'src/cleanup_plan/replay/path_join/before_frontier/', 'src/cleanup_plan/replay/skeleton_work/', 'src/cleanup_plan/replay/path_join/selected/before_frontier/', 'src/workspace_graph/', 'src/bounded_output/', 'src/workspace_graph/expected_projection/borrowed_imports/typed/', 'src/cleanup_plan/replay/factored/', 'src/cleanup_plan/replay/continuation_storage/', 'src/cleanup_plan/replay/path_join/selected/frontier_tests/', 'src/source_reflows_merge/main_e177_overlay_digests_v13/', 'src/hir/validation/', 'src/codegen/native_owned_data_provider/', 'src/ast/program_source/', 'src/cleanup_plan/', 'src/workspace_graph/expected_projection/shared_local_callees/before903/', 'src/hir/resolve_expr/sort_keys/', 'src/workspace_graph/expected_projection/private_layout/', 'src/test_allocation_observer/', 'src/loan_plan/', 'src/cleanup_plan/replay/path_join/shared_insertions_before/', 'src/string_ops/conditions/', 'src/cli/help/authoring/', 'src/hir/lexical_exit/', 'src/bounded_output/stage_debits/route_debits/', 'src/literal_format/', 'src/hir/validation/scope_checkpoint/marks/', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/')

def v15_source_slots(page, convert):
 def row(value):
  assert type(value) is dict, 'v15 literal row'
  if 'source' in value:value['source']=convert(value['source'])
 def fragments(values):
  if values is None:return
  assert type(values) is list, 'v15 fragments'
  for value in values:
   assert type(value) is dict, 'v15 fragment row'
   if 'canonical_source' not in value:row(value)
 for group in ('replacements','additions'):
  assert type(page[group]) is list, 'v15 rows'
  for value in page[group]:
   assert type(value) is dict, 'v15 row'
   if 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v15 indexed row'
    ref=value['old519_index']
    assert type(ref) is list and len(ref)==4, 'v15 old519 arity'
    fragments(ref[3])
   else:
    row(value)
    if 'fragments' in value:fragments(value['fragments'])

def _v15_prefix(path):
 choices=[(len(prefix),index) for index,prefix in enumerate(V15_SOURCE_PREFIXES) if path.startswith(prefix)]
 return max(choices)[1] if choices else None

def _v15_source(value):
 assert type(value) is str, 'v15 source type'
 if ':' not in value:
  assert normal(value), 'v15 source path'
  assert _v15_prefix(value) is None, 'v15 uninterned source'
  return value
 assert len(value)>=3 and value[1]==':' and value[0] in V15_CODES, 'v15 source reference'
 index=V15_CODES.index(value[0]);path=V15_SOURCE_PREFIXES[index]+value[2:]
 assert normal(path), 'v15 expanded source path'
 assert _v15_prefix(path)==index, 'v15 noncanonical source prefix'
 return path

def expand_v15_source_paths(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V15_SCHEMA, 'v15 envelope'
 assert len(canonical(page))<=PAGE_CAP, 'v15 same one-page cap'
 result=copy.deepcopy(page);result['schema']=V14_SCHEMA
 expanded=len(canonical(result))
 def validate(value):
  nonlocal expanded
  path=_v15_source(value)
  expanded=expanded-len(canonical(value))+len(canonical(path))
  assert expanded<=SOURCE_CAP, 'v15 expanded bound'
  return value
 v15_source_slots(result,validate)
 assert expanded<=SOURCE_CAP, 'v15 expanded bound'
 v15_source_slots(result,_v15_source)
 assert len(canonical(result))==expanded, 'v15 expanded bound'
 _expand_v14_logical_pin_tails(result)
 return result

def encode_v15_source_paths(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V14_SCHEMA, 'v15 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v15 logical input bound'
 _expand_v14_logical_pin_tails(page)
 result=copy.deepcopy(page)
 def encode(value):
  assert type(value) is str and normal(value), 'v15 source path'
  index=_v15_prefix(value)
  return value if index is None else V15_CODES[index]+':'+value[len(V15_SOURCE_PREFIXES[index]):]
 v15_source_slots(result,encode);result['schema']=V15_SCHEMA
 assert len(canonical(result))<=PAGE_CAP, 'v15 same one-page cap'
 assert canonical(expand_v15_source_paths(result))==canonical(page), 'v15 ENTIRE V14 page inverse'
 return result

def encode_v15(page):
 return encode_v15_source_paths(_encode_v14_logical_pin_tails(_encode_v13_logical_digest_table(page)))


# Private V16 frontier; original public V15 functions and caps remain byte-exact.
def _expand_v15_logical_source_paths(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V15_SCHEMA, 'v15 envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v15 same one-page cap'
 result=copy.deepcopy(page);result['schema']=V14_SCHEMA
 expanded=len(canonical(result))
 def validate(value):
  nonlocal expanded
  path=_v15_source(value)
  expanded=expanded-len(canonical(value))+len(canonical(path))
  assert expanded<=SOURCE_CAP, 'v15 expanded bound'
  return value
 v15_source_slots(result,validate)
 assert expanded<=SOURCE_CAP, 'v15 expanded bound'
 v15_source_slots(result,_v15_source)
 assert len(canonical(result))==expanded, 'v15 expanded bound'
 _expand_v14_logical_pin_tails(result)
 return result
def _encode_v15_logical_source_paths(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V14_SCHEMA, 'v15 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v15 logical input bound'
 _expand_v14_logical_pin_tails(page)
 result=copy.deepcopy(page)
 def encode(value):
  assert type(value) is str and normal(value), 'v15 source path'
  index=_v15_prefix(value)
  return value if index is None else V15_CODES[index]+':'+value[len(V15_SOURCE_PREFIXES[index]):]
 v15_source_slots(result,encode);result['schema']=V15_SCHEMA
 assert len(canonical(result))<=SOURCE_CAP, 'v15 same one-page cap'
 assert canonical(_expand_v15_logical_source_paths(result))==canonical(page), 'v15 ENTIRE V14 page inverse'
 return result

V16_SCHEMA = 'semaprax.finite-source-overlay-page.v16'
_V16_ROW_KEYS = ('source','compiled','current','views')

def _v16_eligible(value):
 return keys(value,_V16_ROW_KEYS) or keys(value,(*_V16_ROW_KEYS,'fragments'))

def _v16_walk(page, inspect, publish=False, postorder=False):
 # Match V15's exact slot domain: only top-level rows own fragment slots.
 # A fragment row's own unknown fragments metadata is never traversed.
 def row(value, outer):
  if postorder:
   if outer and 'fragments' in value:fragments(value['fragments'])
   return inspect(value)
  changed=inspect(value)
  if outer and 'fragments' in changed:fragments(changed['fragments'])
  return changed
 def fragments(values):
  if values is None:return
  assert type(values) is list, 'v16 fragments'
  for index,value in enumerate(values):
   assert type(value) is dict, 'v16 fragment row'
   if 'canonical_source' not in value:
    changed=row(value,False)
    if publish:values[index]=changed
 for group in ('replacements','additions'):
  assert type(page[group]) is list, 'v16 rows'
  for index,value in enumerate(page[group]):
   assert type(value) is dict, 'v16 row'
   if 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v16 indexed row'
    ref=value['old519_index']
    assert type(ref) is list and len(ref)==4, 'v16 old519 arity'
    fragments(ref[3])
   else:
    changed=row(value,True)
    if publish:page[group][index]=changed

def _v16_tuple(value):
 assert keys(value,('source_row',)), 'v16 row envelope'
 items=value['source_row']
 assert type(items) is list and len(items) in (4,5), 'v16 row arity'
 assert type(items[0]) is str and type(items[1]) is bool, 'v16 source/compiled type'
 assert type(items[2]) in (list,dict) and type(items[3]) is list, 'v16 current/views type'
 # Only a top-level row's fifth payload is a fragment domain; the walker
 # validates that domain. The same key inside a fragment row is opaque.
 return items

def _v16_overhead(arity):
 fields=(*_V16_ROW_KEYS,'fragments')[:arity]
 # All payloads occur exactly once. Only fixed field names and punctuation expand.
 return 2+sum(len(canonical(key)) for key in fields)+(arity-1)-(len(b'{"source_row":[]}')+arity-1)

def expand_v16_source_rows(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V16_SCHEMA, 'v16 envelope'
 assert len(canonical(page))<=PAGE_CAP, 'v16 same one-page cap'
 expanded=len(canonical(page))
 def validate(value):
  nonlocal expanded
  if not keys(value,('source_row',)):
   assert not _v16_eligible(value), 'v16 literal row alias'
   return value
  items=_v16_tuple(value)
  expanded+=_v16_overhead(len(items))
  assert expanded<=SOURCE_CAP, 'v16 expanded bound'
  # A borrowed temporary dictionary permits traversal of the fragments only;
  # it does not copy any payload or publish an expanded source row.
  return {'fragments':items[4]} if len(items)==5 else {}
 _v16_walk(page,validate)
 assert expanded<=SOURCE_CAP, 'v16 expanded bound'
 result=copy.deepcopy(page);result['schema']=V15_SCHEMA
 def decode(value):
  if not keys(value,('source_row',)):return value
  items=_v16_tuple(value)
  return dict(zip((*_V16_ROW_KEYS,'fragments'),items))
 _v16_walk(result,decode,True)
 assert len(canonical(result))==expanded, 'v16 expanded bound'
 _expand_v15_logical_source_paths(result)
 return result

def encode_v16_source_rows(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V15_SCHEMA, 'v16 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v16 logical input bound'
 _expand_v15_logical_source_paths(page)
 result=copy.deepcopy(page)
 def encode(value):
  if not _v16_eligible(value):return value
  assert type(value['source']) is str and type(value['compiled']) is bool, 'v16 source/compiled type'
  assert type(value['current']) in (list,dict) and type(value['views']) is list, 'v16 current/views type'
  return {'source_row':[value[key] for key in (*_V16_ROW_KEYS,'fragments') if key in value]}
 _v16_walk(result,encode,True,True);result['schema']=V16_SCHEMA
 assert len(canonical(result))<=PAGE_CAP, 'v16 same one-page cap'
 assert canonical(expand_v16_source_rows(result))==canonical(page), 'v16 ENTIRE V15 page inverse'
 return result

def encode_v16(page):
 return encode_v16_source_rows(_encode_v15_logical_source_paths(_encode_v14_logical_pin_tails(_encode_v13_logical_digest_table(page))))

# V17 removes only the redundant V16 row-envelope key at its closed row slots.
# All public V16 entry points and their original wire cap stay unchanged.
V17_SCHEMA = 'semaprax.finite-source-overlay-page.v17'

def _v17_walk(page, inspect, publish=False, postorder=False):
 def row(value,outer):
  assert type(value) in (dict,list), 'v17 row type'
  def fragments_from(item):
   if type(item) is list:
    _v17_tuple(item)
    if len(item)==5:fragments(item[4])
   elif 'fragments' in item:fragments(item['fragments'])
  if postorder:
   if outer:fragments_from(value)
   return inspect(value)
  changed=inspect(value)
  if outer:fragments_from(changed)
  return changed
 def fragments(values):
  if values is None:return
  assert type(values) is list, 'v17 fragments'
  for index,value in enumerate(values):
   assert type(value) in (dict,list), 'v17 fragment row'
   if type(value) is list or 'canonical_source' not in value:
    changed=row(value,False)
    if publish:values[index]=changed
 for group in ('replacements','additions'):
  assert type(page[group]) is list, 'v17 rows'
  for index,value in enumerate(page[group]):
   if type(value) is dict and 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v17 indexed row'
    ref=value['old519_index']
    assert type(ref) is list and len(ref)==4, 'v17 old519 arity'
    fragments(ref[3])
   else:
    changed=row(value,True)
    if publish:page[group][index]=changed

def _v17_tuple(items):
 assert type(items) is list and len(items) in (4,5), 'v17 row arity'
 assert type(items[0]) is str and type(items[1]) is int and items[1] in (0,1), 'v17 source/compiled type'
 assert type(items[2]) in (list,dict) and type(items[3]) is list, 'v17 current/views type'
 return items

def expand_v17_source_rows(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V17_SCHEMA, 'v17 envelope'
 expanded=len(canonical(page))
 assert expanded<=PAGE_CAP, 'v17 same one-page cap'
 def validate(value):
  nonlocal expanded
  if type(value) is dict:
   assert not _v16_eligible(value) and not keys(value,('source_row',)), 'v17 literal row alias'
   return value
  items=_v17_tuple(value)
  expanded+=_v16_overhead(len(items))+len(b'{"source_row":[]}')-2+(4 if items[1]==0 else 3)
  assert expanded<=SOURCE_CAP, 'v17 expanded bound'
  return {'fragments':items[4]} if len(items)==5 else {}
 _v17_walk(page,validate)
 assert expanded<=SOURCE_CAP, 'v17 expanded bound'
 result=copy.deepcopy(page);result['schema']=V15_SCHEMA
 def decode(value):
  if type(value) is dict:return value
  items=_v17_tuple(value)
  decoded=dict(zip((*_V16_ROW_KEYS,'fragments'),items))
  decoded['compiled']=items[1]==1
  return decoded
 _v17_walk(result,decode,True)
 assert len(canonical(result))==expanded, 'v17 expanded bound'
 _expand_v15_logical_source_paths(result)
 return result

def encode_v17_source_rows(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V15_SCHEMA, 'v17 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v17 logical input bound'
 _expand_v15_logical_source_paths(page)
 result=copy.deepcopy(page)
 def encode(value):
  if not _v16_eligible(value):
   assert not keys(value,('source_row',)), 'v17 literal row alias'
   return value
  assert type(value['compiled']) is bool, 'v17 source/compiled type'
  items=[value[key] for key in (*_V16_ROW_KEYS,'fragments') if key in value]
  items[1]=int(items[1])
  return _v17_tuple(items)
 _v16_walk(result,encode,True,True);result['schema']=V17_SCHEMA
 assert len(canonical(result))<=PAGE_CAP, 'v17 same one-page cap'
 assert canonical(expand_v17_source_rows(result))==canonical(page), 'v17 ENTIRE V15 page inverse'
 return result

def encode_v17(page):
 return encode_v17_source_rows(_encode_v15_logical_source_paths(_encode_v14_logical_pin_tails(_encode_v13_logical_digest_table(page))))


# V11_VIEW_NAMES_BEGIN: only packed named-view tuple slots participate.
V11_SCHEMA = 'semaprax.finite-source-overlay-page.v11'
def v11_names_slots(page, convert):
 result=copy.deepcopy(page)
 def views(value):
  assert type(value) is list, 'v11 views'
  for view in value:
   if type(view) is list:
    assert view, 'v11 named tuple'
    view[0]=convert(view[0])
 def row(value):
  assert type(value) is dict, 'v11 literal row'
  if 'views' in value:views(value['views'])
 def fragments(value):
  if value is None:return
  assert type(value) is list, 'v11 fragments'
  for fragment in value:
   assert type(fragment) is dict, 'v11 fragment row'
   if 'canonical_source' not in fragment:row(fragment)
 for group in ('replacements','additions'):
  assert type(result[group]) is list, 'v11 rows'
  for value in result[group]:
   assert type(value) is dict, 'v11 row'
   if 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v11 indexed row'
    reference=value['old519_index']
    assert type(reference) is list and len(reference)==4, 'v11 old519 arity'
    views(reference[2]);fragments(reference[3])
   else:
    row(value)
    if 'fragments' in value:fragments(value['fragments'])
 return result

def expand_v11_names(envelope):
 assert keys(envelope,('schema','names','page')) and envelope['schema']==V11_SCHEMA, 'v11 envelope'
 assert len(canonical(envelope))<=PAGE_CAP, 'v11 wire cap'
 names=envelope['names'];assert type(names) is list and all(type(n) is str for n in names), 'v11 names'
 assert names==sorted(set(names)), 'v11 unique sorted names'
 page=envelope['page'];assert type(page) is dict and page.get('schema')==V10_SCHEMA, 'v11 inner schema'
 sizes=[len(canonical(n)) for n in names];used=set();materialized=0
 def name(value):
  nonlocal materialized
  if type(value) is int:
   assert 0<=value<len(names), 'v11 name index'
   index=value;size=sizes[index];value=names[index];used.add(index)
  else:
   assert type(value) is str, 'v11 literal name'
   size=len(canonical(value))
  materialized+=size;assert materialized<=PAGE_CAP, 'v11 materialized name cap'
  return value
 result=v11_names_slots(page,name)
 assert used==set(range(len(names))), 'v11 unused names'
 return result

def encode_v11_names(page):
 assert type(page) is dict and page.get('schema')==V10_SCHEMA, 'v11 encoding schema'
 counts={}
 def count(name):
  assert type(name) is str, 'v11 encoding name'
  counts[name]=counts.get(name,0)+1;return name
 v11_names_slots(page,count)
 names=sorted(n for n,count in counts.items() if count>1);indices={n:i for i,n in enumerate(names)}
 encoded=v11_names_slots(page,lambda name:indices.get(name,name))
 result=dict(schema=V11_SCHEMA,names=names,page=encoded)
 assert canonical(expand_v11_names(result))==canonical(page), 'v11 ENTIRE V10 page inverse'
 assert len(canonical(result))<=PAGE_CAP, 'v11 same wire cap'
 return result

def encode_v11_pins(before,policy,page):
 assert page['schema']==V9_SCHEMA, 'v11 encoding predecessor'
 intermediate=transform_v10_pin_slots(page,before,policy,True);intermediate['schema']=V10_SCHEMA
 assert canonical(expand_v10_pins(before,policy,intermediate))==canonical(page), 'v11 ENTIRE V9 intermediate inverse'
 return encode_v11_names(intermediate)
# V11_VIEW_NAMES_END

# OPT933: these public wire shapes share one historical schema spelling.
# Internal V12--V17 readers still call the complete original digest reader.
def expand_v11_compatible(page):
 assert type(page) is dict and page.get('schema')==V11_SCHEMA, 'v11 compatible schema'
 if keys(page,('schema','names','page')):
  return expand_v11_names(page)
 assert keys(page,('schema','index','total_pages','predecessor','replacements','additions','archive_prefixes')), 'v11 compatible envelope'
 return expand_v11_digest_pins(page)

# Direct public V10 also has two independently retained historical wire forms.
# Their exact complete outer shapes select provenance, never field absence.
def expand_v10_public_compatible(before,policy,page):
 assert type(page) is dict and page.get('schema')==V10_SCHEMA, 'v10 compatible schema'
 if keys(page,('schema','index','total_pages','predecessor','replacements','additions','archive_prefixes')):
  return expand_v10_archive_prefixes(page)
 assert keys(page,('schema','index','total_pages','predecessor','replacements','additions')), 'v10 compatible envelope'
 return expand_v10_pins(before,policy,page)

# OPT934: integer references only in the original top-level fragment slots.
V18_SCHEMA = 'semaprax.finite-source-overlay-page.v18'
def _v18_walk(page, row, reference, publish=False):
 def fragments(values):
  if values is None:return
  assert type(values) is list, 'v18 fragments'
  for index,value in enumerate(values):
   if type(value) is int or type(value) is bool or type(value) is float or (type(value) is dict and 'canonical_source' in value):changed=reference(value)
   else:
    assert type(value) in (dict,list), 'v18 fragment row'
    changed=row(value)
   if publish:values[index]=changed
 for group in ('replacements','additions'):
  assert type(page[group]) is list, 'v18 rows'
  for index,value in enumerate(page[group]):
   if type(value) is dict and 'old519_index' in value:
    assert group=='replacements' and keys(value,('old519_index',)), 'v18 indexed row'
    ref=value['old519_index'];assert type(ref) is list and len(ref)==4, 'v18 old519 arity'
    fragments(ref[3])
   else:
    assert type(value) in (dict,list), 'v18 row type'
    changed=row(value)
    if type(changed) is list:
     _v17_tuple(changed)
     if len(changed)==5:fragments(changed[4])
    elif 'fragments' in changed:fragments(changed['fragments'])
    if publish:page[group][index]=changed

def _v18_index(value, total):
 assert type(value) is int and 519<=value<total, 'v18 canonical addition index'
 return value

def _v18_preflight(page, limit):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V18_SCHEMA, 'v18 envelope'
 expanded=len(canonical(page));assert expanded<=PAGE_CAP, 'v18 same one-page cap'
 assert type(page['additions']) is list, 'v18 rows'
 total=519+len(page['additions'])
 def bound(amount):
  nonlocal expanded
  expanded+=amount;assert expanded<=min(limit,SOURCE_CAP), 'v18 expanded bound'
 def row(value):
  if type(value) is dict:
   assert not _v16_eligible(value) and not keys(value,('source_row',)), 'v18 literal row alias'
  else:
   items=_v17_tuple(value)
   bound(_v16_overhead(len(items))+len(b'{"source_row":[]}')-2+(4 if items[1]==0 else 3))
  return value
 def reference(value):
  if type(value) is dict:
   if keys(value,('canonical_source',)) and type(value['canonical_source']) is not str:
    _v18_index(value['canonical_source'],total);assert False, 'v18 wrapped integer alias'
  else:_v18_index(value,total);bound(21)
  return value
 _v18_walk(page,row,reference)
 assert expanded<=min(limit,SOURCE_CAP), 'v18 expanded bound'
 return expanded

def expand_v18_fragment_refs(page):
 expanded=_v18_preflight(page,SOURCE_CAP)
 result=copy.deepcopy(page);result['schema']=V15_SCHEMA
 def decode(value):
  if type(value) is dict:return value
  items=_v17_tuple(value);value=dict(zip((*_V16_ROW_KEYS,'fragments'),items));value['compiled']=items[1]==1;return value
 _v18_walk(result,decode,lambda value:{'canonical_source':value} if type(value) is int else value,True)
 assert len(canonical(result))==expanded, 'v18 expanded bound'
 _expand_v15_logical_source_paths(result)
 return result

def encode_v18_fragment_refs(page):
 assert keys(page,(*V12_KEYS,'view_labels','digest_table','pin_tails')) and page['schema']==V15_SCHEMA, 'v18 input envelope'
 assert len(canonical(page))<=SOURCE_CAP, 'v18 logical input bound'
 _expand_v15_logical_source_paths(page)
 result=copy.deepcopy(page)
 def row(value):
  if not _v16_eligible(value):
   assert not keys(value,('source_row',)), 'v18 literal row alias'
   return value
  assert type(value['compiled']) is bool, 'v18 source/compiled type'
  items=[value[key] for key in (*_V16_ROW_KEYS,'fragments') if key in value];items[1]=int(items[1]);return _v17_tuple(items)
 _v16_walk(result,row,True,True)
 total=519+len(result['additions'])
 def reference(value):
  assert type(value) is dict, 'v18 literal integer alias'
  if keys(value,('canonical_source',)) and type(value['canonical_source']) is not str:return _v18_index(value['canonical_source'],total)
  return value
 _v18_walk(result,lambda value:value,reference,True);result['schema']=V18_SCHEMA
 assert len(canonical(result))<=PAGE_CAP, 'v18 same one-page cap'
 assert canonical(expand_v18_fragment_refs(result))==canonical(page), 'v18 ENTIRE V15 page inverse'
 return result

def encode_v18(page):
 return encode_v18_fragment_refs(_encode_v15_logical_source_paths(_encode_v14_logical_pin_tails(_encode_v13_logical_digest_table(page))))

# V19 is an explicit metadata successor; it grants no source or archive authority.
V19_SCHEMA = 'semaprax.finite-source-overlay-page.v19'
_V19_SOURCE = 'src/workspace_graph/expected_projection/borrowed_imports.rs'
_V19_OWNER = 'src/workspace_graph/expected_projection.rs'
_V19_INDEX = 389
_V19_DECLARATION = ('#[path = "expected_projection/borrowed_imports.rs"]\n'
                    'pub(in crate::workspace_graph) mod borrowed_imports;\n')

def _v19_successor(page):
 assert len(canonical(page)) <= PAGE_CAP, 'v19 same one-page cap'
 assert keys(page, ('schema','compiled_successor','page')) and page['schema']==V19_SCHEMA, 'v19 envelope'
 inner=page['page']
 assert type(inner) is dict and inner.get('schema')==V18_SCHEMA, 'v19 complete v18 page'
 entry=page['compiled_successor']
 assert keys(entry, ('source','prior_row_sha256','current','registration')), 'v19 single successor'
 assert entry['source']==_V19_SOURCE, 'v19 admitted source'
 digest=entry['prior_row_sha256']
 assert type(digest) is str and re.fullmatch('[0-9a-f]{64}',digest), 'v19 prior row digest'
 for value in (entry['current'],entry['registration'].get('current') if type(entry['registration']) is dict else None):
  assert keys(value, ('length','sha256','archive')), 'v19 current pin'
  pin(value);assert value['archive']=='e/'+value['sha256'], 'v19 exact immutable archive'
 registration=entry['registration']
 assert keys(registration, ('owner','current','start','text')), 'v19 registration fields'
 assert registration['owner']==_V19_OWNER, 'v19 admitted owner'
 assert type(registration['start']) is int and 2<=registration['start']<=SOURCE_CAP, 'v19 declaration start'
 assert registration['text']==_V19_DECLARATION, 'v19 exact declaration'
 assert registration['start']+len(registration['text'].encode())<=registration['current']['length'], 'v19 declaration bound'
 return entry

def authenticate_v19_registration(overlay,page):
 entry=_v19_successor(page);registration=entry['registration']
 assert callable(overlay.compiled_current), 'v19 trusted registration binding required'
 for source,current in ((entry['source'],entry['current']),(registration['owner'],registration['current'])):
  expected=overlay.compiled_current(source)
  assert keys(expected,('length','sha256')) and expected=={key:current[key]for key in('length','sha256')}, 'v19 stale trusted current binding'
 source=overlay.current({'source':entry['source'],'compiled':True,'current':entry['current'],'views':[]})
 owner=overlay.current({'source':registration['owner'],'compiled':True,'current':registration['current'],'views':[]})
 assert type(owner) is bytes and type(source) is bytes, 'v19 ordinary current bytes'
 literal=registration['text'].encode();start=registration['start']
 assert owner.count(literal)==1 and owner[start:start+len(literal)]==literal, 'v19 actual declaration span'
 # The exact two-line declaration must begin after a blank line. An adjacent
 # cfg/test attribute or a declaration embedded in another line cannot qualify.
 assert owner[start-2:start]==b'\n\n', 'v19 declaration attribute boundary'
 return entry

def compose_v19(before,policy,page):
 entry=_v19_successor(page)
 assert keys(policy, ('schema','page','predecessor','replacements','additions','new_fragment_edges')), 'v19 policy fields'
 assert policy['schema']=='semaprax.finite-source-overlay-policy.v2', 'v19 policy schema'
 assert policy['replacements'].count(entry['source'])==1, 'v19 explicit replacement'
 assert page['page'].get('predecessor')==policy['predecessor'], 'v19 original predecessor'
 assert H(canonical(before))==policy['predecessor']['logical_sha256'], 'v19 original base digest'
 assert before['source_cap']==SOURCE_CAP and len(before['rows'])==policy['predecessor']['rows']==519, 'v19 original base shape'
 assert before['rows'][_V19_INDEX]['source']==entry['source'], 'v19 original source position'
 original=before['rows'][_V19_INDEX]
 assert original['compiled'] is False and H(canonical(original))==entry['prior_row_sha256'], 'v19 entire old false row'
 derived=copy.deepcopy(before);derived['rows'][_V19_INDEX]['compiled']=True
 inverse=copy.deepcopy(derived);inverse['rows'][_V19_INDEX]['compiled']=False
 assert canonical(inverse)==canonical(before), 'v19 ENTIRE original base inverse'
 adjusted_policy=copy.deepcopy(policy);adjusted_page=copy.deepcopy(page['page'])
 adjusted_policy['predecessor']['logical_sha256']=H(canonical(derived))
 adjusted_page['predecessor']=copy.deepcopy(adjusted_policy['predecessor'])
 # The old decoder still owns every view, unknown-field, order, fragment,
 # whole-row inverse and cap check. The derived base is never physical history.
 result=compose_values(derived,adjusted_policy,adjusted_page)
 rows={row['source']:row for row in result['rows']}
 assert len(rows)==len(result['rows']), 'v19 canonical unique rows'
 assert rows[entry['source']]['compiled'] is True and rows[entry['source']]['current']==entry['current'], 'v19 successor current'
 owner=rows[entry['registration']['owner']]
 assert owner['compiled'] is True and owner['current']==entry['registration']['current'], 'v19 owner current'
 inverse=copy.deepcopy(result);inverse['rows']=copy.deepcopy(before['rows'])
 assert canonical(inverse)==canonical(before), 'v19 complete predecessor object inverse'
 return result

def encode_v19(page,compiled_successor):
 result={'schema':V19_SCHEMA,'compiled_successor':copy.deepcopy(compiled_successor),'page':copy.deepcopy(page)}
 _v19_successor(result)
 assert canonical(result['page'])==canonical(page) and canonical(result['compiled_successor'])==canonical(compiled_successor), 'v19 ENTIRE inputs retained'
 return result
