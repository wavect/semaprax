from pathlib import Path
from collections import Counter
import ast,hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;H='6444d6d950bc01be03720c33ad6cb175e62e8cc5';M='f4a448da6ed71da57719ea7bc8062b134484e8a7';g=lambda *a:subprocess.check_output(['git',*a],cwd=R);h=lambda b:hashlib.sha256(b).hexdigest();counts={}
for name,rev in [('6444',H),('main',M)]:
 rows=json.loads((D/f'current-to-{name}.json').read_text());seen=set()
 for row in rows:
  source=row['source'];assert source not in seen;seen.add(source);current=(R/source).read_bytes();assert h(current)==row['current_sha256']
  for edit in row['edits']:
   new=edit['new'].encode();assert current.count(new)==1;current=current.replace(new,edit['old'].encode(),1)
  assert h(current)==row['original_sha256'] and current==(R/row['archive']).read_bytes()==g('show',rev+':'+source)
 tracked=set(g('ls-tree','-r','--name-only',rev).decode().splitlines())
 for p in g('diff','--name-only',rev).decode().splitlines():
  if p not in tracked:continue
  if name=='main' and p.startswith('docs/evidence/'):continue # immutable physical histories retain their own domains
  if (R/p).read_bytes()!=g('show',rev+':'+p):assert p in seen,p
 counts[name]=len(rows)
assert (D/'source-inverses.json').read_bytes()==(D/'current-to-6444.json').read_bytes()
# All MAIN changed Rust code and bodies are exact; only independent inventories/helper differ.
mainrust=[p for p in g('diff','--name-only','27424cdcd5ad28daf4ce25fc8e633a4586852d9f',M).decode().splitlines() if p.endswith('.rs')]
for p in mainrust:assert (R/p).read_bytes()==g('show',M+':'+p),p
# Every old runtime JSON/script/archive remains physical byte-exact, apart from
# MAIN's explicit structural-cursor documentation update in its own complete row.
for p in g('diff','--name-only',H).decode().splitlines():
 if p.startswith('docs/evidence/') and g('ls-tree',H,'--',p).strip():
  assert p=='docs/evidence/structural-call-cursor-v1/README.md',p
# Both changelog entry sets and exact new outer prefix are retained.
s=(R/'src/source_reflows_merge.rs').read_text();old=g('show',H+':src/source_reflows_merge.rs').decode();prefix='    let before_main = project(\n        include_str!(concat!(\n            env!("CARGO_MANIFEST_DIR"),\n            "/docs/evidence/main-f4a448-integration-20261015/current-to-6444.json"\n        )),\n        source,\n        current,\n    );\n    let current = before_main.as_str();\n';assert s.count(prefix)==1 and s.replace(prefix,'')==old
log=(R/'CHANGELOG.md').read_text();assert '<<<<<<<' not in log
for line in g('diff','--unified=0','27424cdcd5ad28daf4ce25fc8e633a4586852d9f',M,'--','CHANGELOG.md').decode().splitlines():
 if line.startswith('+- '):assert line[1:] in log
assert 'Author the sealed initial builder-report policy' in log
# All three actual inventories retain their complete original body plus two includes.
audits=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
for p in audits:
 current=(R/p).read_text();before=g('show',H+':'+p).decode();marker='src/cleanup_plan/replay/structural_control/call_receiving.rs"),';i=current.index(marker)+len(marker);a=current.rfind('include_str!("',0,i)+len('include_str!("');prefix=current[a:current.index('src/',a)];addition=' '+' '.join('include_str!("'+prefix+'src/cleanup_plan/replay/supplemental/'+n+'"),' for n in ['receiving.rs','receiving/tests.rs']);assert current.count(addition)==1 and current.replace(addition,'')==before
assert len((R/audits[-1]).read_text().splitlines())==1500
# Evaluate the literal old-source substitutions in MAIN's existing source guards.
test=(R/'src/cleanup_plan/replay/supplemental/receiving/tests.rs').read_text();current=(R/'src/cleanup_plan/replay/supplemental/receiving.rs').read_text();restored=current[:current.index('// Capacity never supplies traversal authority.')];block=test.split('let replacements = [',1)[1].split('];',1)[0]
for a,b in re.findall(r'\(("(?:[^"\\]|\\.)*"),\s*("(?:[^"\\]|\\.)*")\)',block):
 a,b=ast.literal_eval(a),ast.literal_eval(b);assert restored.count(a)==1;restored=restored.replace(a,b,1)
assert restored.removesuffix('\n')==(R/'docs/evidence/supplemental-cursor-growth-v1/receiving.before.rs.txt').read_text()
assert (R/'docs/evidence/supplemental-cursor-growth-v1/receiving.before.rs.txt').read_bytes()==g('show',M+':docs/evidence/supplemental-cursor-growth-v1/receiving.before.rs.txt')
assert (R/'docs/evidence/supplemental-cursor-growth-v1/receiving.before.rs.txt').read_bytes()==g('show',H+':src/cleanup_plan/replay/supplemental/receiving.rs')
test=(R/'src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs').read_text();body=test.split('fn optional_source_work_selection_preserves_the_complete_prior_capture_body()',1)[1];vals={}
for name in ['helper','guard','debit','anchor']:
 m=re.search(r'let '+name+r'\s*=\s*("(?:[^"\\]|\\.)*");',body);assert m,name;vals[name]=ast.literal_eval(m[1])
restored=(R/'src/workspace_graph/static_text/scalar_helpers.rs').read_text()
for n in ['helper','guard']:assert restored.count(vals[n])==1;restored=restored.replace(vals[n],'',1)
assert restored.count(vals['anchor'])==1;restored=restored.replace(vals['anchor'],vals['debit']+vals['anchor'],1);assert restored==(R/'docs/evidence/optional-helper-source-work-v1/scalar_helpers.before.rs.txt').read_text()
proof={'status':'SOURCE_ONLY_MAIN_MERGE_NOT_RUNTIME_QUALIFIED','source_parents':[H,M],'complete_named_source_views':counts,'MAIN_changed_Rust_exact':len(mainrust),'old_runtime_JSON_scripts_archives_not_rewritten':True,'all_three_source_audits_preserved_plus_two_owners':True,'source_caps_preserved':True,'MAIN_complete_source_guards_literal_replay':2,'inherited_6444_namespace_full_restoration':True,'prior_6444_proof_reused_only_by_exact_whole_namespace_equivalence':True,'Cargo_Rust_Node_Clippy_producer_execution':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
