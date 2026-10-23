from pathlib import Path
import re,json,hashlib
r=Path.cwd();o=Path('/Volumes/Semaprax/tmp/ci-38383508340/native-provider-receipt-295');m=r/'target/ci-private/inline-target-focused-root';old=json.loads((o/'focused-root-scope.json').read_text());selected={(x['source'],x['function'])for x in old['exports']};selected.update(("src/hir/validation/byte_traversal/nested/tests.rs",n) for n in re.findall(r'#\[test\]\s*fn (\w+)', (r/'src/hir/validation/byte_traversal/nested/tests.rs').read_text()));selected.update(("src/source_reflows/nested_scope.rs",n) for n in re.findall(r'#\[test\]\s*(?:pub\(crate\) )?fn (\w+)',(r/'src/source_reflows/nested_scope.rs').read_text()));selected.update(("src/hir/validation/byte_traversal/nested/tests/source_composition.rs",n) for n in re.findall(r'#\[test\]\s*fn (\w+)',(r/'src/hir/validation/byte_traversal/nested/tests/source_composition.rs').read_text()));selected.update((rel,n) for rel in ["src/codegen/native_emit/validation_reuse_tests.rs","src/codegen/native_owned_data_provider/validation_reuse_tests.rs","src/codegen/native_emit/owned_strings/tests.rs","src/codegen/native_resource/tests.rs","src/call_index/validation_receipt_tests.rs"] for n in re.findall(r'#\[test\]\s*fn (\w+)', (r/rel).read_text()));retained={(x['source'],name)for x in old['retained_registration_dependencies']for name in ['joined_nested_source_keeps_full_live_union_and_both_complete_parent_custodies','joined_nested_source_refuses_foreign_truncated_drifted_predicates_and_partial_recipes','retained_cache_receiving_fee_preserves_floor_and_sticky_refusal_without_allocation']};ns={};exec(compile((r/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py').read_text(),'unchanged-parser','exec'),ns);receipts=[];kept=[];omitted=0
for p in (r/'src').rglob('*.rs'):
 s=p.read_text();q=m/p.relative_to(r)
 if '#[test]'not in s and not re.search(r'#\[\w+::test',s):
  if not q.exists():
   q.parent.mkdir(parents=True,exist_ok=True);q.symlink_to(p)
  continue
 rel=p.relative_to(r).as_posix();mask=ns['mask'](s);edits=[]
 for v in re.finditer(r'#\[(?:(?:\w+)::)?test(?:\([^\]]*\))?\]',mask):
  fn=re.search(r'\bfn\s+(\w+)\s*\(',mask[v.end():]);name=fn[1]if fn else ''
  if (rel,name)in selected:kept.append((rel,name));continue
  replacement='#[allow(dead_code)]'if (rel,name)in retained else'#[cfg(any())]';omitted+=replacement=='#[cfg(any())]';edits.append((v.start(),v.end(),replacement))
 if not edits and not any(x[0]==rel for x in selected):continue
 for a,b,value in reversed(edits):s=s[:a]+value+s[b:]
 if q.is_symlink():q.unlink()
 q.parent.mkdir(parents=True,exist_ok=True);q.write_text(s);receipts.append(dict(source=rel,original_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),overlay_sha256=hashlib.sha256(s.encode()).hexdigest(),changes='test registration attributes only; no production, IR, layout, fixture or test function bodies changed'))
assert set(kept)==selected and len(kept)==len(selected)
(o/'native-focused-scope.json').write_text(json.dumps(dict(scope='Actual complete crate compiled as native Rust test binary; selected original and added test bodies registered',selected=sorted(selected),other_test_attributes_cfg_false=omitted,required_source_test_dependency_bodies_retained=True,no_ir_or_layout_standins=True,no_production_code_removed_or_modified=True,all_source_overlays=receipts,no_full_cargo_or_full_binary_TLS_qualification=True),indent=2)+'\n');print('Native test binary:',len(kept),'selected; other test registrations disabled:',omitted,'; all real production code unchanged.')
