#!/usr/bin/env python3
"""Check complete inverse bytes without compiling or executing Rust owners."""
from pathlib import Path
import re,json,difflib,subprocess
root=Path(__file__).resolve().parents[3]
def string_at(t,i):
 while t[i].isspace():i+=1
 if t[i]=='r':
  m=re.match(r'r(#+)"',t[i:]); start=i+len(m[0]); end=t.index('"'+m[1],start);return t[start:end],end+1+len(m[1])
 j=i+1
 while True:
  if t[j]=='\\':j+=2;continue
  if t[j]=='"':return json.loads(t[i:j+1]),j+1
  j+=1
import hashlib
results=[]
def project(source,current):
 records=json.loads((root/"docs/evidence/source36-after-23e8-20261015/source-projections.json").read_text())
 matching=[row for row in records if row['source']==source];assert len(matching)==1
 row=matching[0];assert hashlib.sha256(current.encode()).hexdigest()==row['current_sha256']
 authenticated=subprocess.check_output(['git','show',row['checkpoint_revision']+':'+source],cwd=root)
 assert authenticated==(root/row['archive']).read_bytes()
 assert hashlib.sha256(authenticated).hexdigest()==row['original_sha256']
 for edit in row['edits']:
  assert current.count(edit['new'])==1
  current=current.replace(edit['new'],edit['old'],1)
 assert current.encode()==authenticated
 return current
def apply(current,section,p):
 for u in re.finditer(r'undo\(\s*&mut current,\s*',section):
  new,j=string_at(section,u.end());j=section.index(',',j)+1
  if section[j:].lstrip().startswith('&format!'):
   # Both existing complete original function reconstructions use the same three-fragment format.
   f=section.index('&format!',j);fmt,jj=string_at(section,section.index('(',f)+1);assert fmt=='{}{}{}'
   prefix,jj=string_at(section,section.index(',',jj)+1)
   inc=re.search(r'include_str!\(\s*"([^"]+)"\s*\)',section[jj:]);assert inc
   saved=(p.parent/inc[1]).resolve().read_text();marker='use super::*;\n\npub(super) ';assert saved.startswith(marker)
   saved=saved[len(marker):].rstrip('\n')
   suffix_start=section.index('r#"',jj+inc.end());suffix,jj=string_at(section,suffix_start);old=prefix+saved+suffix
  else:old,j=string_at(section,j)
  assert current.count(new)==1,(str(p),repr(new[:100]),current.count(new));current=current.replace(new,old,1)
 return current
for name in ['a','b']:
 p=root/f'src/cleanup_plan/replay/live_oracle/source_custody/{name}.rs';t=p.read_text();starts=list(re.finditer(r'let mut current =\s*(?:include_str!\("([^"]+)"\)\.to_owned\(\)|super::current_view::project\("([^"]+)", include_str!\("([^"]+)"\)\))\s*;',t))
 for k,m in enumerate(starts):
  end=starts[k+1].start() if k+1<len(starts) else len(t);section=t[m.end():end];src=(p.parent/(m[1] or m[3])).resolve();current=src.read_text()
  if m[2]:current=project(m[2],current)
  current=apply(current,section,p)
  ars=re.findall(r'include_str!\(\s*"([^"]+\.before)"\s*\)',section);assert ars;archive=(p.parent/ars[-1]).resolve();assert current==archive.read_text(),src
  results.append({'owner':str(p.relative_to(root)),'source':str(src.relative_to(root)),'archive':str(archive.relative_to(root)),'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'archive_bytes':len(archive.read_bytes()),'whole_inverse_equal':True})
p=root/'src/cleanup_plan/replay/paid_type_facts/inventory_encoded/tests/constructors.rs';t=p.read_text();start=t.index('    let mut current = include_str!("../../../../../hir.rs")');end=t.index('    assert_eq!(current,',start);current=apply((root/'src/hir.rs').read_text(),t[start:end],p);archive=root/'docs/evidence/schema-summary/hir.rs.before';assert current==archive.read_text()
results.append({'owner':str(p.relative_to(root)),'source':'src/hir.rs','archive':str(archive.relative_to(root)),'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'archive_bytes':len(archive.read_bytes()),'whole_inverse_equal':True})
paid=(root/'src/hir/declaration_index/flags/paid.rs').read_text();nominal_registration='#[path = \"../nominal_paid_facts.rs\"]\nmod nominal_constructor;\n';assert paid.count(nominal_registration)==1;paid=paid.replace(nominal_registration,'',1);suffix='\nmod inventory_constructor;\npub(crate) use inventory_constructor::{\n    identity as inventory_identity, substitute as inventory_substitute, InventoryPayer,\n};\n';assert paid.endswith(suffix);assert paid[:-len(suffix)]==(root/'docs/evidence/schema-summary/flags-paid.rs.before').read_text()
for n in ['key','substitute','storage']:assert (root/f'src/hir/declaration_index/flags/paid/{n}.rs').read_bytes()==(root/f'docs/evidence/schema-summary/flags-paid-{n}.rs.before').read_bytes()
p=root/'src/hir/validation/canonical_loans/ownership_tests.rs';t=p.read_text()
# Recognize only the named 824 projection wrapper, allowing Rust syntax spacing.
# Source bytes are never normalized: every inverse and final archive stays exact.
anchor=r'let\s+mut\s+current\s*=\s*super::super::function_lookup::paid_custody::project_current\(\s*"src/hir/validation.rs"\s*,\s*include_str!\(\s*"../../validation.rs"\s*\)\s*\)\s*;'
starts=list(re.finditer(anchor,t));assert len(starts)==1, 'unique named 824 validation projection wrapper'
start=starts[0].end();end=t.index('    let projected=current.replacen',start)
current=(root/'src/hir/validation.rs').read_text()
records=json.loads((root/'docs/evidence/compat-constructor-preflight-824/exact-source-inverses.json').read_text())
records=[record for record in records if record['source']=='src/hir/validation.rs']
assert len(records)==1, 'unique named 824 validation source record'
record=records[0]
# Mirror project_current's complete named inverses in the same reverse order.
for edit in reversed(record['edits']):
 assert current.count(edit['after'])==1, 'unique named 824 validation source inverse'
 current=current.replace(edit['after'],edit['before'],1)
assert current==(root/'docs/evidence/compat-constructor-preflight-824'/record['archive']).read_text()
current=apply(current,t[start:end],p)
for n,o in [('let canonical_loans = canonical_loans::Lookup::current(&seed)?;\n        Self::from_constructor(seed, canonical_loans, None)','let canonical_loans = canonical_loans::Inventory::rebuild(program)?;\n        Self::from_constructor(seed, canonical_loans::Lookup::Legacy(canonical_loans), None)'),("storage: Option<canonical_loans::Lookup<'a>>","storage: Option<&'a super::canonical_loan_storage::Storage<'a>>"),('            storage\n        } else {\n            canonical_loans::Lookup::current(&seed)?','            canonical_loans::Lookup::Borrowed(storage)\n        } else {\n            canonical_loans::Lookup::Legacy(canonical_loans::Inventory::rebuild(program)?)')]:
 assert current.count(n)==1;current=current.replace(n,o,1)
archive=root/'docs/evidence/compatibility-loan-custody/src__hir__validation.rs.before';assert current==archive.read_text()
results.append({'owner':str(p.relative_to(root)),'source':'src/hir/validation.rs','archive':str(archive.relative_to(root)),'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'archive_bytes':len(archive.read_bytes()),'whole_inverse_equal':True})
print('Source-only whole inverse proofs:',len(results),'all equal; unchanged constructor paid/key/substitute/storage archives all equal. No Rust execution.')
