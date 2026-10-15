"""Authoring helper: source bytes only; no Rust/compiler/runtime execution."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='940fcf220e47d1975f757e76f0b3401691db9358'
sha=lambda b:hashlib.sha256(b).hexdigest()
owners=['src/workspace_graph.rs','src/workspace_graph/expected_projection/private_layout.rs','src/workspace_graph/expected_projection/private_layout/names.rs','src/workspace_graph/expected_projection/private_layout/registry.rs','tests/workspace/semantic_graph/source_inventory.rs']
def rows(names):
 result=[]
 for path in names:
  old=subprocess.check_output(['git','show',B+':'+path],cwd=R);new=(R/path).read_bytes();archive=D/'original'/Path(path).name
  archive.write_bytes(old)
  result.append({'source':path,'basis':B,'archive':str(archive.relative_to(R)),'current_sha256':sha(new),'original_sha256':sha(old),'edits':[{'new':new.decode(),'before':old.decode()}]})
 return result
children=['src/workspace_graph/expected_projection/private_layout/before_registry.rs','src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs']
packet={'status':'SOURCE_AUTHORED_UNEXECUTED_NO_RUST_OR_CLAUDE','basis':B,'owners':rows(owners),'children':[{'source':p,'sha256':sha((R/p).read_bytes())}for p in children]}
(D/'source-inverses.json').write_text(json.dumps(packet,indent=2)+'\n')
(D/'documentation-inverses.json').write_text(json.dumps(rows(['docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md','CHANGELOG.md']),indent=2)+'\n')
old_owner='src/workspace_graph/expected_projection/private_layout/owner_index.rs';(D/'original/owner_index.rs').write_bytes(subprocess.check_output(['git','show',B+':'+old_owner],cwd=R))
P=Path('/Volumes/Semaprax/provenance/qualification-registration-module-path-20261014.py');prefix=P.read_text().split("parse(R/'src/lib.rs'")[0];n={};exec(prefix,n);n['R']=R
n['parse'](R/'src/lib.rs',[],[],'lib','semaprax');n['parse'](R/'tests/workspace.rs',[],[],'test:workspace','semaprax')
new=[k for k in n['IDX'] if k[2].startswith('workspace_graph::expected_projection::private_layout::registry::scratch_tests::')]
required=[k for k in n['IDX'] if k[2].startswith('workspace_graph::expected_projection::private_layout::registry::tests::')]
required += [('semaprax','test:workspace','semantic_graph::expected_projection_source_boundary_is_pure_and_keeps_shared_helpers_in_root'),('semaprax','lib','workspace_graph::tests::builder_limits::identity_prebound_production_core_retry_preserves_phase_debit_and_nested_refusal'),('semaprax','lib','workspace_graph::tests::builder_limits::catalog_normalizer_compact_core_fits_production_builder_cap'),('semaprax','lib','project::incremental::cold_source_tests::explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse'),('semaprax','lib','project::incremental::checked_hir::tests::lifecycle::actual_cold_warm_cache_core_and_publication_match_owned_receiver_through_drop')]
assert len(new)==7 and len(required)==11
checks=[]
for role,keys in [('new',new),('required_existing',required)]:
 for key in keys:
  bound=n['IDX'][key];assert not bound['ignored'];p=bound['source_path'];checks.append({'package':key[0],'harness':key[1],'selector':key[2],'source':p,'source_sha256':sha((R/p).read_bytes()),'source_binding':bound,'role':role,'status':'SOURCE_AUTHORED_UNEXECUTED'})
(D/'pending-selectors.json').write_text(json.dumps({'basis':B,'status':'SOURCE_AUTHORED_UNEXECUTED_NO_RUST','parser_sha256':sha(P.read_bytes()),'new':7,'required_existing':11,'checks':checks},indent=2)+'\n')
print('Authored 5 full source inverses,2 docs,2 children; source-only resolved7new+11existing exact unignored bodies')
