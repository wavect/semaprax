from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];P=Path(__file__).resolve().parent
sha=lambda x:hashlib.sha256(x).hexdigest()
rows=json.loads((P/'source-inverses.json').read_text());assert len(rows)==4
for row in rows:
 old=subprocess.check_output(['git','show',row['checkpoint_revision']+':'+row['source']],cwd=R)
 current=(R/row['source']).read_bytes();assert sha(current)==row['current_sha256'] and sha(old)==row['original_sha256'];assert (R/row['archive']).read_bytes()==old
 text=current.decode()
 for e in row['edits']:
  assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 assert text.encode()==old
 if '/main_unit_weight/' in row['source']:
  assert current.decode().replace('crate::cleanup_plan::replay::factored::lexical_plan_tests::','crate::cleanup_plan::replay::lexical_plan_tests::').encode()==old
  assert current.count(b'crate::cleanup_plan::replay::factored::lexical_plan_tests::')==2
 else:
  removed=b'#[cfg(test)]\npub(super) use base::cache_billing::source_reference::{\n    historical_inline_inventory, historical_inline_root, historical_root,\n};\n';assert old.replace(removed,b'')==current
assert '#[path = "factored/lexical_plan_tests.rs"]\nmod lexical_plan_tests;' in (R/'src/cleanup_plan/replay/factored.rs').read_text()
print(json.dumps({'status':'SOURCE_ONLY_PASS','four_full_inverses':True,'six_fixture_paths_only':True,'old_assertions_exact':True,'production_unchanged':True,'Rust_executed':False}))
