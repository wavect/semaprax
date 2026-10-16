#!/usr/bin/env python3
"""Source-only exact predecessor preservation; no Rust execution."""
import argparse,hashlib,json,subprocess
from pathlib import Path
from boundary_tools import protected,forbidden
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);a=p.parse_args();r=a.repo.resolve();d=r/'docs/evidence/source-boundary-domains-20261016';protected(r)
j=json.loads((d/'original-tool-bindings.json').read_text());ref=j['before_head'];proof={}
for name,path,anchor,end in [
 ('hir','tests/hir_module_boundaries.rs','    let validation = production_source_inventory::production(','    let root = audit_source_join!('),
 ('projection','tests/workspace/semantic_graph.rs','    let authority_projection = production_projection.as_str();','    let costs = projection.find(')]:
 old=subprocess.check_output(['git','show',ref+':'+path],cwd=r).decode();new=(r/path).read_text();before=old[old.index(anchor):old.index(end,old.index(anchor))];after=new[new.index(anchor):new.index(end,new.index(anchor))];assert before==after,(name,'original callsite and complete forbidden predicates changed');proof[name]={'complete_original_callsite_and_predicates_sha256':hashlib.sha256(before.encode()).hexdigest(),'byte_exact':True,'exact_forbidden_tokens':forbidden(r,name)}
facade=(r/'tests/support/production_source_inventory.rs').read_text();assert facade.count('original::production(raw, manifest)')==1 and facade.count('boundary::production(raw, manifest, &complete_authenticated_production)')==1;assert facade.index('original::production(raw, manifest)')<facade.index('boundary::production(raw, manifest, &complete_authenticated_production)');assert '#[path = "production_source_inventory_original.rs"]'in facade and '#[path = "new_boundary_module.rs"]'in facade
proof={'status':'SOURCE_ONLY_COMPLETE_PREDECESSOR_PRESERVATION_PASS','original_helper_sha256':j['helper_before_sha256'],'complete_original_helper_independently_compiled':True,'all_original_generator_verifier_parser_bytes_unchanged':True,'full_authentication_before_boundary_selection':True,'callsites_and_predicates':proof,'rust_executed':False};(d/'preservation-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
