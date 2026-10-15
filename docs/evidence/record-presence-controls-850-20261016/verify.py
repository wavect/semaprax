"""Finite source-only actual-layout correction custody; no Rust execution."""
from pathlib import Path
import json,subprocess,hashlib
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'source-inverses.json').read_text());runtime=json.loads((D/'runtime-inputs.json').read_text())
for p,row in rows.items():
 assert (R/p).read_text()==row['current']
 assert sha(row['current'].encode())==row['current_sha256']
 assert subprocess.check_output(['git','show','2a9059ed8:'+p],cwd=R).decode()==row['before']
 assert sha(row['before'].encode())==row['before_sha256']
 assert (D/'before'/p).read_text()==row['before']
for p,row in runtime.items():assert row==rows[p]
base=R/'docs/evidence/record-presence-phase-v1';inv=json.loads((base/'inverse.json').read_text())
for p,edits in inv.items():
 current=runtime[p]['before'];assert current==subprocess.check_output(['git','show','2691824df:'+p],cwd=R).decode()
 for start,new,old in reversed(edits):
  assert current[start:start+len(new)]==new;current=current[:start]+old+current[start+len(new):]
 before={'consuming.rs':'consuming.before.rs.txt','mod.rs':'ordinary.before.rs.txt','ordinary.rs':'module_index.before.rs.txt'}[Path(p).name]
 assert current==(base/before).read_text()
original=(base/'consuming.before.rs.txt').read_text();current=(R/'src/cleanup_plan/renewal_profile/ordinary/consuming.rs').read_text();marker="pub(super) fn collect<'a>(";start=current.index(marker);end=current.index('pub(super) fn collect_with_presence');assert current[start:end].rstrip()==original[original.index(marker):].rstrip()
main=subprocess.check_output(['git','show','2691824df:src/cleanup_plan/renewal_profile/ordinary/consuming.rs'],cwd=R).decode();assert main[main.index('    visit(',main.index("fn collect_body<'a>(")):].rstrip()==current[current.index('    visit(',end):].rstrip()
presence=(R/'src/cleanup_plan/renewal_profile/ordinary/record_presence.rs').read_text();mainpresence=subprocess.check_output(['git','show','2691824df:src/cleanup_plan/renewal_profile/ordinary/record_presence.rs'],cwd=R).decode()
assert presence[presence.index('    pub(crate) fn get'):].replace('''        // The Directory caller prepays one read bank before Some/closure/get
        // evaluation. This exact original gateway keeps sticky/work/Subject
        // ordering; it creates no new allowance and retains no donor payer.
''','')==mainpresence[mainpresence.index('    pub(crate) fn get'):]
proof={'mode':'SOURCE_ONLY_NO_RUST_EXECUTION','whole_immediate_inverses':len(rows),'whole_main269_runtime_inputs':len(runtime),'original_collect_exact':True,'direct_visit_body_exact_main':True,'original_get_auth_work_subject_order_exact':True,'all_original_main_inverses_replayed':True,'no_new_selector':True}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
