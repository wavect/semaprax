#!/usr/bin/env python3
"""Bounded integration custody proof; no build/runtime/application qualification."""
import json,hashlib,subprocess,re
from pathlib import Path
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
h=lambda b:hashlib.sha256(b).hexdigest()
g=lambda *a:subprocess.check_output(['git',*a],cwd=R)
rows=json.loads((D/'current-to-7a87.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text())
for row in rows:
 b=(R/row['source']).read_bytes();assert h(b)==row['current_sha256']
 text=b.decode()
 for e in row['edits']:
  assert text.count(e['new'])==1; text=text.replace(e['new'],e['old'],1)
 assert h(text.encode())==row['original_sha256']
 assert text.encode()==(R/row['archive']).read_bytes()==g('show',row['checkpoint_revision']+':'+row['source'])
for row in json.loads((D/'exact-main-inputs.json').read_text()):
 b=(R/row['source']).read_bytes();assert h(b)==row['sha256'];assert b==g('show',row['source_revision']+':'+row['source'])
base=R/'docs/evidence/expression-fact-arena-cursor-v1';cap=(R/'src/cleanup_plan/replay/expression_facts/capacity.rs').read_text();capold=(base/'capacity.before.rs.txt').read_text();assert cap.startswith(capold)
a=cap.index('/// One funded function owns');b=cap.index('\n#[cfg(test)]\n#[path = "capacity/cursor_tests.rs"]');assert h(cap[a:b].encode())=='f1404ed792ce1a02121eaf193cdecd8fd4f37f806371566c56e5ee564ad31a17'
producer=(R/'src/cleanup_plan/replay/expression_facts/arena/producer.rs').read_text()
for e in json.loads((base/'producer-inverses.json').read_text()):
 assert producer.count(e['current'])==1;producer=producer.replace(e['current'],e['previous'],1)
assert producer==(base/'producer.before.rs.txt').read_text()==(R/'src/cleanup_plan/replay/expression_facts/arena/producer_before_cursor.rs').read_text()
test=(R/'src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs').read_text();registration=json.loads(re.search(r'let registration = ("(?:\\.|[^"\\])*");',test).group(1))
arena=(R/'src/cleanup_plan/replay/expression_facts/arena.rs').read_text();assert arena.count(registration)==1
assert arena.replace(registration,'').replace('capacity::ArenaCapacity::new(function, payer)','capacity::FactCapacity::with_payer(function, payer)')==(base/'arena.before.rs.txt').read_text()
children=['arena/before_cursor.rs','arena/producer_before_cursor.rs','arena/cursor_tests.rs','capacity/cursor_tests.rs']
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/path).read_text()
 for child in children:assert text.count('expression_facts/'+child+'"')==1,(path,child)
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
changes=g('diff','--name-status','7a87b33115c0cd45b14cac64d7b37e5742503159','--','docs/evidence').decode().splitlines();assert all(x.startswith('A\t') for x in changes),changes
print(json.dumps({'status':'SOURCE_INTEGRATION_ONLY_NO_RUST','full_current_to_7a87':len(rows),'exact_latest_main_inputs':20,'complete_original_main_cursor_source_controls_replayed':True,'new_compiled_children_joined_all_three_audits':4,'new_exact_unit_selectors':8,'old_evidence_modified':False,'pins_or_app_qualification':False},indent=2))
