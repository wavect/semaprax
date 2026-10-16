from pathlib import Path
import hashlib,json,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2];j=json.loads((D/'source-inverses.json').read_text());b=json.loads((D/'binding.json').read_text())
def sha(x):return hashlib.sha256(x).hexdigest()
for r in j['rows']:
 current=(R/r['path']).read_bytes();assert sha(current)==r['current_sha256'];text=current.decode()
 for e in r['inverse_edits']:assert text.count(e['current'])==1;text=text.replace(e['current'],e['before'])
 restored=text.encode();assert sha(restored)==r['before_sha256'];assert restored==(D/r['before_archive']).read_bytes()==subprocess.check_output(['git','show',j['base']+':'+r['path']],cwd=R)
 # Every original assertion and the complete zero-output control survive.
 old=restored.decode();assert [l.strip()for l in old.splitlines()if 'assert' in l]==[l.strip()for l in current.decode().splitlines()if 'assert' in l]
 marker='    // Both routes have genuine zero first-output work, after input work.';assert old[old.index(marker):]==current.decode()[current.decode().index(marker):]
for p,h in b['unchanged_independent_old'].items():assert sha((R/p).read_bytes())==h and (R/p).read_bytes()==subprocess.check_output(['git','show',j['base']+':'+p],cwd=R)
assert sha((D/'actual-index18-failure.log').read_bytes())==b['actual_failure']['sha256']
print('PASS complete_before_inverse old_assertions zero_output full_old_receivers actual_failure_log; SOURCE_ONLY')
