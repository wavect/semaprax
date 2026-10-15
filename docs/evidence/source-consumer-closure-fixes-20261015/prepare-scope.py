"""Review scope proposal only; parent authorization is external."""
from pathlib import Path
import json,subprocess,re,hashlib
R=Path.cwd();D=Path(__file__).resolve().parent;B='3d4b996c9a8cd2b100a3c594e741314c108b5591';sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
ns={};exec((D/'module-parser-original.py.txt').read_text(),ns);ns['R']=R;mask=ns['mask'];regs=[];orig=ns['parse']
def wrapped(p,prefix,chain,kind,package,custom_path=False):
 if p.exists():regs.append({'path':str(p.resolve().relative_to(R)),'chain':chain})
 return orig(p,prefix,chain,kind,package,custom_path)
ns['parse']=wrapped;wrapped(R/'src/lib.rs',[],[],'lib','semaprax')
def item(s,pattern):
 m=mask(s);z=re.search(pattern,m);assert z;start=m.rfind('#[',0,z.start());assert s[start:z.start()].strip().startswith('#[cfg(test)]');brace=z.end()-1 if m[z.end()-1]=='{' else m.index('{',z.end());j=brace+1;d=1
 while d:d+=(m[j]=='{')-(m[j]=='}');j+=1
 return start,j,s[start:j]
mixed={'src/cleanup_plan/replay/factored/canonical_shared.rs':r'pub\(super\)\s+fn\s+id\s*\(', 'src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs':r'mod\s+tests\s*\{','src/workspace_graph/expected_projection/type_rewrite/receiving.rs':r'mod\s+tests\s*\{'}
prod={'src/workspace_graph/generic_type_import/receiving.rs','src/workspace_graph/generic_type_import/parameter_names/receiving.rs','crates/semaprax-native-rust-owned-data-package/src/project_publication/application.rs'}
rows=json.loads((D/'current-to-3d4b.json').read_text());scope=[]
for r in rows:
 p=r['source'];old=r['edits'][0]['old'];new=r['edits'][0]['new'];record={'path':p,'before_sha256':sha(old),'after_sha256':sha(new)}
 if p in mixed:
  a,b,oi=item(old,mixed[p]);c,d,ni=item(new,mixed[p]);assert new[:c]+oi+new[d:]==old,p
  record.update(kind='whole_existing_cfg_test_item',before_item=oi,after_item=ni,before_item_bytes=[a,b],after_item_bytes=[c,d])
 elif p in prod:record.update(kind='parent_authorized_layout_equivalent_production_spelling',equivalence_packet='reference-control-size-spelling-20261015' if p.startswith('src/') else 'application-osstr-map-spelling-20261015')
 elif p.startswith('tests/'):record.update(kind='integration_test_source')
 else:
  found=[q for q in regs if q['path']==p];assert found,p
  assert all(any('#[cfg(test)]' in c['attributes'] for c in q['chain']) for q in found),p
  record.update(kind='cfg_test_only_module_source',registration_chains=[q['chain'] for q in found])
 scope.append(record)
# Actual exact owning test declarations from unchanged original parser.
selected=[]
for k,v in ns['IDX'].items():
 if v['source_path'] in {r['source'] for r in rows}:
  selected.append({'package':k[0],'target':k[1],'selector':k[2],'source_path':v['source_path'],'line':v['line']})
out={'proposal_not_authorization':True,'basis':B,'rust_path_count':len(scope),'paths':scope,'source_declared_changed_file_tests':selected,'new_test_bodies':0,'production_spelling_owners':sorted(prod)}
(D/'review-scope-proposal.json').write_text(json.dumps(out,indent=2)+'\n');print('scope PASS',len(scope),'paths;',len(selected),'source-declared tests in changed lib files')
