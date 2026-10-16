#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib
R=Path(__file__).resolve().parents[3];P=Path(__file__).resolve().parent;d=json.loads((P/'source-inverses.json').read_text())
for row in d['sources']:
 now=(R/row['source']).read_text();before=subprocess.check_output(['git','show',d['base']+':'+row['source']],cwd=R).decode()
 assert now==row['current'] and before==row['before']
 assert hashlib.sha256(now.encode()).hexdigest()==row['current_sha256']
 assert hashlib.sha256(before.encode()).hexdigest()==row['before_sha256']
 assert len(now.splitlines()) <= (6018 if row['source']=='src/workspace_graph.rs' else 1500)
rows={x['source']:x for x in d['sources']}
r=rows['src/workspace_graph/function_alias/before_core.rs'];prefix=r['current'].split('// Test-only selection preserves')[0].rstrip()+'\n';assert prefix.replace('pub(in crate::workspace_graph) fn build_resolved_core_current(', 'pub(super) fn build_resolved_core_current(',1)==r['before']
r=rows['src/workspace_graph/function_alias/before_core/tests.rs'];assert r['current'].startswith(r['before']);assert r['current'].count('#[test]')==r['before'].count('#[test]')+1
assert 'actual_borrowed_alias_project_keeps_complete_sources_graph_and_image_of_old_core' in r['current']
for exact in ['assert_eq!(old.semantic_graph(), current.semantic_graph());','assert_eq!(old_image.to_json(), current_image.to_json());','current_borrowed > 0,']:
 assert exact in r['current']
proof={'status':'SOURCE_ONLY_PASS','base':d['base'],'full_inputs':len(d['sources']),'new_owning_tests':1,'old_core_body_preserved':True,'original_test_body_preserved':True,'normalization':None,'runtime_executed':False}
(P/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
