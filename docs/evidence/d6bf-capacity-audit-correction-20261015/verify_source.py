from pathlib import Path
import subprocess,json,hashlib,re,shutil
R=Path(__file__).resolve().parents[3]; E=Path(__file__).resolve().parent
BASIS='d6bf8151942190a956eb57350fa8b8892a8c68e5'
def blob(p):return subprocess.check_output(['git','show',BASIS+':'+p],cwd=R)
def current(p):return (R/p).read_bytes() if (R/p).is_file() else blob(p)
def sha(b):return hashlib.sha256(b).hexdigest()
paths=[r['source'] for r in json.loads((E/'reference-slots.json').read_text())];paths=list(dict.fromkeys(paths));paths.append('src/workspace_graph/expected_projection/private_layout/capacity_tests.rs')
# Archive order was fixed at creation.
paths=['src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs','src/workspace_graph/expected_projection/cost/constructor.rs','src/workspace_graph/expected_projection/type_rewrite/receiving.rs','src/workspace_graph/expected_projection/private_layout/capacity_tests.rs']
# No formatter/tool execution: only wrap newly introduced imports and arithmetic.
for p in paths:
 s=(R/p).read_text()
 s=s.replace('    use super::{ast_function_cost, ast_type_declaration_cost, pay_default_teardown, teardown_function, Cost, Receiver, Setup, Type};','    use super::{\n        ast_function_cost, ast_type_declaration_cost, pay_default_teardown, teardown_function,\n        Cost, Receiver, Setup, Type,\n    };')
 s=s.replace('    use super::{identity, missing, rewrite_with_layout, vector_element, Failure, Program, Receiver, Setup, Type};','    use super::{\n        identity, missing, rewrite_with_layout, vector_element, Failure, Program, Receiver, Setup,\n        Type,\n    };')
 s=s.replace('            size_of::<&crate::ast::ExprKind>() + size_of_val(&carrier_free) + size_of_val(&outside_closure),','            size_of::<&crate::ast::ExprKind>()\n                + size_of_val(&carrier_free)\n                + size_of_val(&outside_closure),')
 (R/p).write_text(s)
metadata=['docs/evidence/constructor-cost-propagation-825/source-inverses.json','docs/evidence/constructor-signature-rewrite-825/source-inverses.json','docs/evidence/private-layout-entry-capacity-834/exact-source-inverses.json']
counts=[]
for j,p in enumerate(metadata):
 before=blob(p); rows=json.loads(before)
 (E/'original'/f'metadata-{j:02}.before.json').write_bytes(before)
 for row in rows:
  old=blob(row['source']).decode();new=current(row['source']).decode()
  if old!=new:
   row['edits'].insert(0,{'new':new,'before':old});row['current_sha256']=sha(new.encode())
  restored=new
  for edit in row['edits']:
   assert restored.count(edit['new'])==1,(p,row['source'],'ambiguous')
   restored=restored.replace(edit['new'],edit['before'],1)
  expected=row.get('original_bytes')
  if expected is None:expected=blob(row['archive']).decode()
  assert restored==expected,(p,row['source'],'inverse')
  if 'original_sha256' in row:assert sha(restored.encode())==row['original_sha256']
  if 'current_sha256' in row:assert sha(new.encode())==row['current_sha256'],(p,row['source'],'currenthash')
 target=R/p;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(json.dumps(rows,indent=2,ensure_ascii=False)+'\n');counts.append({'metadata':p,'rows':len(rows),'all_full_inverses':'PASS'})
local=[]
for i,p in enumerate(paths):
 old=blob(p);assert old==(E/'original'/f'{i:02}.before').read_bytes();new=current(p)
 local.append({'source':p,'basis':'d6bf8151942190a956eb57350fa8b8892a8c68e5','current_sha256':sha(new),'original_sha256':sha(old),'archive':f'docs/evidence/d6bf-capacity-audit-correction-20261015/original/{i:02}.before','edits':[{'new':new.decode(),'before':old.decode()}]})
(E/'source-inverses.json').write_text(json.dumps(local,indent=2,ensure_ascii=False)+'\n')
slots=json.loads((E/'reference-slots.json').read_text());assert len(slots)==61
# Replay all slot substitutions and lexical borrow ending to prove production body equality.
for p in paths[:3]:
 old=blob(p).decode();lines=old.splitlines(keepends=True)
 for r in sorted([r for r in slots if r['source']==p],key=lambda r:(r['line'],r['column']),reverse=True):
  i=r['line']-1;c=r['column']-1;assert lines[i][c:].startswith(r['before']);lines[i]=lines[i][:c]+r['new']+lines[i][c+len(r['before']):]
 reconstructed=''.join(lines).split('#[cfg(test)]')[0]
 if p==paths[0]:reconstructed=reconstructed.replace('''                let mut cost = Cost::new(owner, false)?;
                ast_type_cost(ty, &mut cost)?;
                let total = cost.raw.total;
                drop(cost);''','''                let total = {
                    let mut cost = Cost::new(owner, false)?;
                    ast_type_cost(ty, &mut cost)?;
                    cost.raw.total
                };''')
 assert re.sub(r'\s+','',reconstructed)==re.sub(r'\s+','',current(p).decode().split('#[cfg(test)]')[0])
 # Entire old test bodies unchanged after the one explicit import replacement.
 if '#[cfg(test)]' in old:
  a=old.split('#[cfg(test)]',1)[1].replace('    use super::*;','')
  b=current(p).decode().split('#[cfg(test)]',1)[1];b=re.sub(r'    use super::\{.*?\};\n','',b,count=1,flags=re.S)
  assert re.sub(r'\s+','',a)==re.sub(r'\s+','',b)
# Every original capacity assertion and all code after fixture construction is byte exact.
p=paths[3];assert blob(p).decode().split('fn census',1)[1]==current(p).decode().split('fn census',1)[1]
for p in ['src/workspace_graph/expected_projection/private_layout/capacity_before.rs','tests/workspace/semantic_graph.rs','tests/workspace/semantic_graph/source_inventory.rs','src/workspace_graph/expected_projection/private_layout/capacity_tests/owner_controls.rs','src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs']:
 assert current(p)==blob(p),p
# Full joined tree remains audited. No removal of any inventory line, no glob survives.
projection=list((R/'src/workspace_graph/expected_projection').rglob('*.rs'))
assert all('use super::*;' not in p.read_text() for p in projection)
for p in paths:assert len(current(p).splitlines())<=1500
logs=E/'actual-prior-failures'; raw=logs
receipts=[]
for name in ['owning-selected-59.log','owning-selected-60.log','owning-selected-80.log','clippy-production.log']:
 data=(logs/name).read_bytes();(raw/name).write_bytes(data);receipts.append({'file':name,'sha256':sha(data),'bytes':len(data),'head':'d6bf8151942190a956eb57350fa8b8892a8c68e5'})
proof={'basis':'d6bf8151942190a956eb57350fa8b8892a8c68e5','status':'SOURCE_ONLY_STATIC_PASS','runtime':'UNEXECUTED; actual prior failures archived, no corrective runtime result claimed','reference_slot_replacements':61,'lexical_borrow_end_replacements':1,'test_bodies_and_assertions':'UNCHANGED except two explicit imports and fixture module function','old_receiver_and_source_audit_joins':'BYTE_EXACT_UNCHANGED','direct_metadata_inverses':counts,'actual_prior_receipts':receipts,'global_composition':'NOT RUN; parent owns integrated composition'}
(E/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
print(json.dumps(proof,indent=2))
