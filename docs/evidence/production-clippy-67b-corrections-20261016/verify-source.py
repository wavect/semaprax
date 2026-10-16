"""Finite full-source inversion only; no compiler or Rust test execution."""
from pathlib import Path
import argparse,contextlib,hashlib,io,json,runpy,subprocess,sys,tempfile,re
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);R=p.parse_args().repo.resolve();D=R/'docs/evidence/production-clippy-67b-corrections-20261016';data=json.loads((D/'source-inverses.json').read_text());h=lambda b:hashlib.sha256(b).hexdigest();view={}
assert len(data['rows'])==8
for row in data['rows']:
 current=(R/row['source']).read_bytes();assert h(current)==row['current_sha256'];assert current.decode()==row['new'];assert row['new'] != row['old']
 old=current.decode().replace(row['new'],row['old'],1).encode();assert h(old)==row['before_sha256'];assert old==(R/row['archive']).read_bytes()==subprocess.check_output(['git','show',data['base']+':'+row['source']],cwd=R);view[str((R/row['source']).resolve())]=old
# Complete old receiving predicates/assertions run on exact original inputs.
rb,rt,argv=Path.read_bytes,Path.read_text,sys.argv[:]
try:
 def readb(p):return view[str(p.resolve())]if str(p.resolve())in view else rb(p)
 def readt(p,*a,**kw):return view[str(p.resolve())].decode(kw.get('encoding')or'utf-8')if str(p.resolve())in view else rt(p,*a,**kw)
 Path.read_bytes=readb;Path.read_text=readt;sys.argv=['verify-source.py','--repo',str(R)];out=io.StringIO()
 with contextlib.redirect_stdout(out):runpy.run_path(str(R/'docs/evidence/a3-e47-final-custody-20261016/verify-source.py'),run_name='__main__')
 inherited=json.loads(out.getvalue());assert inherited['status']=='PASS_SOURCE_ONLY'
finally:Path.read_bytes,Path.read_text,sys.argv=rb,rt,argv
# Exact array layout expression retains eight usize elements on every Rust target.
old=view[str((R/'src/cleanup_plan/replay/final_output.rs').resolve())].decode();new=(R/'src/cleanup_plan/replay/final_output.rs').read_text();assert new.replace('size_of::<[usize; 8]>()','8 * size_of::<usize>()')==old
paid=(R/'src/cleanup_plan/replay/inventory_coverage/paid.rs').read_text();assert '#[cfg(test)]\npub(super) use admit_initial;'in paid
for path in ['src/cleanup/inventory_builder/typed.rs','src/cleanup/inventory_builder/typed/selected.rs','src/cleanup/inventory_builder/typed/traversal.rs']:
 source=(R/path).read_text();assert '.ok_or(InventoryFailure'not in source,'must not eagerly form terminal payloads'
# Every original scoped diagnostic has an explicit source correction below.
findings=json.loads((D/'findings.json').read_text())['findings'];assert len(findings)==19
log=(D/'actual-production-clippy.log').read_text()
assert len(re.findall(r'\nerror: (?!could not compile)',log))==19
read=lambda path:(R/path).read_text()
typed=read('src/cleanup/inventory_builder/typed.rs')
selected=read('src/cleanup/inventory_builder/typed/selected.rs')
traversal=read('src/cleanup/inventory_builder/typed/traversal.rs')
replay=read('src/cleanup_plan/replay.rs')
vector=read('src/cleanup_plan/replay/sequence_vector.rs')
assert '\nuse key_bytes;' not in typed
assert 'ResolvedMatchPattern' not in traversal.split('impl InventoryBuilder',1)[0]
assert 'ResolvedRecordMatchFieldPattern' not in traversal.split('impl InventoryBuilder',1)[0]
assert re.search(r'use super::\{[^}]*\bDiagnostic\b',paid) is None
# Seven terminal failures remain constructed only in the failed Option arm.
for source,reason in [(typed,'ConditionalEntryReferencesUnknownStorage'),
 (selected,'SelectedInventoryCloneBackingOverflow'),
 (selected,'SelectedInventoryHasNoPrimitiveLifecycle'),
 (traversal,'RecordPatternHasNoFieldInventory'),
 (traversal,'RecordPatternFieldInventoryIsIncomplete'),
 (traversal,'NestedRecordPatternHasNoFieldInventory'),
 (traversal,'NestedRecordPatternFieldInventoryIsIncomplete')]:
 assert re.search(r'let Some\([^;]+?else\s*\{\s*return Err\(InventoryFailure\(Kind::Finite\(\s*Reason::'+reason+r'\s*,?\s*\)\)\);\s*\};',source,re.S),reason
assert 'while let Some(path) = paths.next() {' not in replay
assert 'for path in paths {' in replay
for guard in ['if single && first {','if !precharged && share_prefix {',
 'if work.budget.selected() || len < final_output::FINAL_MINIMUM_ITEMS {',
 '} else if last.observations.prefix.is_some() || capacity == len {']:
 assert guard in replay,guard
assert 'Some(expected) if expected == capacity => {' in vector
assert 'Some(min_saved) if bytes >= min_saved => {' in vector
# Independently apply the exact committed inverse/forward patch to copies only.
# tempfile obeys the caller's SSD TMPDIR; no checkout source is modified.
with tempfile.TemporaryDirectory(prefix='clippy67b-source-',dir='/Volumes/Semaprax/runtime-tmp') as temp:
 root=Path(temp)
 for row in data['rows']:
  target=root/row['source'];target.parent.mkdir(parents=True,exist_ok=True)
  target.write_bytes((R/row['source']).read_bytes())
 subprocess.run(['git','apply','--no-index',str(D/'changes.inverse.patch')],cwd=root,check=True)
 for row in data['rows']:assert (root/row['source']).read_bytes()==(R/row['archive']).read_bytes()
 subprocess.run(['git','apply','--no-index',str(D/'changes.patch')],cwd=root,check=True)
 for row in data['rows']:assert (root/row['source']).read_bytes()==(R/row['source']).read_bytes()
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole67b_inputs':8,'shipping_files':7,'all19_actual_findings_covered':True,'forward_inverse_patch_roundtrip':True,'eight_usize_array_layout_equivalent':True,'cfgtest_macro_reexport_preserved':True,'lazy_terminal_error_formation_preserved':True,'inherited67b_algorithm':inherited['status'],'rust_executed':False},sort_keys=True))
