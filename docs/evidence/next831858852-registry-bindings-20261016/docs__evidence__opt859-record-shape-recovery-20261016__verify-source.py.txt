"""Exact source composition only: no Rust execution or runtime PASS claim."""
from pathlib import Path
import argparse,difflib,hashlib,json,re,subprocess
p=argparse.ArgumentParser();p.add_argument('--write-current',action='store_true');a=p.parse_args()
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
BASE='12e7f8a4d3fcb3c5464327a68a8e13506b43e303';PARENT='ad5fc3e203d9e89dde7840e2bd97194c02c88e98';sha=lambda b:hashlib.sha256(b).hexdigest()
def old(ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=R)
new=['src/source_verify/declaration/record_shape.rs','src/source_verify/declaration/record_shape_before.rs']
guards=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
if a.write_current:
 rows=[]
 for f in sorted((D/'before').iterdir()):
  path=f.name.replace('__','/');before=f.read_bytes();assert before==old(BASE,path);current=(R/path).read_bytes();left=before.decode().splitlines(True);right=current.decode().splitlines(True);off=[0]
  for line in right:off.append(off[-1]+len(line))
  inverse=[dict(start=off[u],end=off[v],after=''.join(right[u:v]),before=''.join(left[i:j]))for op,i,j,u,v in difflib.SequenceMatcher(None,left,right,autojunk=False).get_opcodes()if op!='equal']
  rows.append(dict(source=path,archive='before/'+f.name,before_sha256=sha(before),current_sha256=sha(current),current_length=len(current),inverse=inverse,changed=before!=current))
 capsule=dict(base=BASE,rows=rows,new_sources=[dict(source=x,sha256=sha((R/x).read_bytes()),length=len((R/x).read_bytes()))for x in new]);(D/'source-capsule.json').write_text(json.dumps(capsule,indent=2)+'\n')
m=json.loads((D/'source-capsule.json').read_text());assert m['base']==BASE

def restore(row,raw):
 assert sha(raw)==row['current_sha256'] and len(raw)==row['current_length']
 text=raw.decode();at=0;out=''
 for e in row['inverse']:
  assert at<=e['start']<=e['end']<=len(text) and text[e['start']:e['end']]==e['after'];out+=text[at:e['start']]+e['before'];at=e['end']
 result=(out+text[at:]).encode();assert sha(result)==row['before_sha256'];return result
neg=0
for row in m['rows']:
 before=(D/row['archive']).read_bytes();assert before==old(BASE,row['source']);raw=(R/row['source']).read_bytes();assert restore(row,raw)==before
 for bad in [raw+b' ',raw[:-1],b'!'+raw[1:]]:
  try:restore(row,bad)
  except AssertionError:neg+=1
  else:raise AssertionError(('whole source drift accepted',row['source']))
for row in m['new_sources']:
 b=(R/row['source']).read_bytes();assert len(b)==row['length'] and sha(b)==row['sha256'];assert len(b.splitlines())<=1500
 for guard in guards:assert row['source']in(R/guard).read_text()
# Complete original owning fixture, including every assertion and admitted sibling.
fixture='tests/owned_data/nested_owned_record_frontend_hir.rs';before=old(BASE,fixture);assert (R/fixture).read_bytes().startswith(before)
# The full old producer function is independently compiled without body edits.
producer='src/source_verify/declaration/declarations.rs';original=old(BASE,producer).decode();start=original.index('pub(super) fn check_byte_data_declarations');end=original.index('\npub(super) fn check_native_rust_imports',start);copied=(R/new[1]).read_text();assert copied[copied.index('pub(super) fn check_byte_data_declarations'):]==original[start:end]
# Existing Recovery/signature-origin and independent valid-pattern owner unchanged.
for path in ['src/source_verify/recovery.rs','src/source_verify/declared_type.rs','src/source_verify/declaration/functions.rs']:
 assert (R/path).read_bytes()==old(BASE,path)
# Every previously existing assertion body remains complete and in order.
def assertions(text):
 out=[]
 for hit in re.finditer(r'\bassert(?:_eq|_ne)?!\s*\(',text):
  at=hit.end();depth=1
  while depth:
   raw=re.match(r'r(#{0,})"',text[at:])
   if raw:close='"'+raw[1];at=text.index(close,at+raw.end())+len(close);continue
   if text[at]=='"':
    at+=1
    while text[at]!='"':at+=2 if text[at]=='\\'else 1
    at+=1;continue
   char=re.match(r"'(?:\\.|[^'\n])'",text[at:])
   if char:at+=char.end();continue
   if text.startswith('//',at):at=text.index('\n',at)+1;continue
   if text.startswith('/*',at):at=text.index('*/',at+2)+2;continue
   depth+=(text[at]=='(')-(text[at]==')');at+=1
  out.append(text[hit.start():at])
 return out
assertion_count=0
for row in m['rows']:
 if not row['source'].endswith('.rs'):continue
 before=assertions((D/row['archive']).read_text());current=iter(assertions((R/row['source']).read_text()))
 for body in before:
  assert any(body==candidate for candidate in current),(row['source'],'old assertion lost/changed/reordered');assertion_count+=1
# The same declaration/profile loop and body traversal remain live, no global stop.
decl=(R/'src/source_verify/declaration.rs').read_text();assert 'check_function_bodies('in decl and 'check_byte_data_declarations('in decl
shape=(R/new[0]).read_text()
for token in ['existing.code == refusal.code','existing.severity == refusal.severity','existing.message == refusal.message','existing.path == refusal.path','existing.span == refusal.span','existing.help == refusal.help','std::ptr::eq(item, declaration)','recovery.refused_annotation(0, &parameter.name, &parameter.ty, origin)']:
 assert token in shape
assert 'binding.ty != parameter.ty || binding.mode != parameter.mode'in shape
assert '#[cfg(test)]\nmod record_shape_before;'in decl
for path in guards:assert len((R/path).read_bytes().splitlines())<=1500 or path.startswith('tests/')
# Only the exact two helper subjects extend the existing finite allowed registry;
# parent ad5 is an independently committed second predecessor, never guessed scope.
registry='docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json';current=json.loads((R/registry).read_text());accepted=False
for ref in [BASE,PARENT]:
 expected=json.loads(old(ref,registry))
 for guard in guards:
  for key in ['required','maximum']:expected[guard][key].update({x:1 for x in new})
 accepted|=expected==current
assert accepted,'foreign allowed source registry'
proof=dict(mode='SOURCE_ONLY_NO_RUST_EXECUTION',complete_preimages=len(m['rows']),changed_whole_endpoints=sum(x['changed']for x in m['rows']),whole_current_drift_negatives_refused=neg,complete_original_fixture_prefix=True,complete_original_declaration_producer_body=True,recovery_and_signature_origin_byte_exact=True,valid_pattern_and_function_body_owners_byte_exact=True,complete_original_assertions_preserved=assertion_count,new_complete_rust_subjects=len(new),raw_joins=3,native_inventory_lines=len((R/guards[-1]).read_bytes().splitlines()),runtime_result='PENDING',six_field_runtime_controls='PENDING')
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
