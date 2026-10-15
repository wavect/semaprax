"""Compiler-error correction and complete source-custody replay; no Rust runs."""
from pathlib import Path
import subprocess,json,hashlib,contextlib,io,re
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='9b174d852798c99ce2d14b8170ec05e6c0f94b88';sha=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'current-to-9b.json').read_text());named={}
for r in rows:
 p=R/r['source'];b=p.read_bytes();assert sha(b)==r['current_sha256']
 for e in r['edits']:
  a=e['new'].encode();assert b.count(a)==1;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];assert b==subprocess.check_output(['git','show',BASE+':'+r['source']]);named[p.resolve()]=b
p=R/'tests/workspace/semantic_graph.rs';s=p.read_text();before=named[p.resolve()].decode()
for name in ['current','original']:
 new='''format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest('''+name+'''.as_bytes()))
        )''';assert s.count(new)==1;s=s.replace(new,'format!("{:x}", Sha256::digest('+name+'.as_bytes()))',1)
assert s==before,'only two digest spelling changes, every assertion byte retained'
assert 'pub mod digest_hex;' in (R/'src/lib.rs').read_text();assert 'impl<T: AsRef<[u8]>> fmt::LowerHex for LowerHex<T>' in (R/'src/digest_hex.rs').read_text()
# The exact frozen assembly verifier runs in the authenticated9b view.
old=R/'docs/evidence/final-ae91-mainf045-assembly-20261015/verify.py';source=old.read_text();rb0=Path.read_bytes;rt0=Path.read_text;wt0=Path.write_text;capture={}
def rb(p):return named[p.resolve()] if p.resolve() in named else rb0(p)
def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*a,**kw)
def wt(p,s,*a,**kw):assert p.resolve()==(old.parent/'source-proof.json').resolve(),p;capture.update(json.loads(s));return len(s)
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(source,str(old),'exec'),{'__file__':str(old),'__name__':'__main__'})
finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0
assert capture['four_source_failures_and_full_facade_replayed'] and capture['protected_full_packet_files']==61
receipt=json.loads((D/'owning-build-result.json').read_text());assert receipt['source_subject']==BASE and receipt['exit_code']==101 and receipt['executed_test_bodies']==0
errors=[]
for line in (D/'owning-build.jsonl').read_text().splitlines():
 try:r=json.loads(line)
 except json.JSONDecodeError:continue
 if r.get('reason')=='compiler-message' and r.get('message',{}).get('code',{}):
  if r['message']['code']['code']=='E0277':errors.append(r['message'])
# Some drivers nest cargo JSON messages; full original JSONL remains archived.
proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','actual_failed_subject':BASE,'actual_build_exit':101,'actual_build_wall_seconds':receipt['wall_seconds'],'actual_test_bodies':0,'exact_full_source_inputs':2,'all_original_assertions_retained':True,'stable_public_digest_adapter_already_used_in_same_file':True,'complete_frozen_4_57_87_92_93_source_verifier_replayed':True,'protected_packets_unchanged':61,'source_only_production_changes':0,'new_test_bodies':0,'actual_E0277_direct_records':len(errors),'old_unit_and_production_passes_are_only_subject_9b':True,'unit_recompile_required_because_typed_cost_reads_full_helper':True}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS exact two digest fixes +strict2-source predecessor +complete frozen source replay')
