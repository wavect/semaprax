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
