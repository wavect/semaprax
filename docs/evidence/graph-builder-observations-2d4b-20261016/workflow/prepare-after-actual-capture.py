"""Strict read-only preparation only AFTER ROOT supplies actual source-bound capture."""
from pathlib import Path
import argparse,hashlib,json,subprocess
HERE=Path(__file__).resolve().parent
RECORD=json.loads((HERE/'skeleton-record.json').read_text())
ROOT=Path(RECORD['repository'])
HEAD=RECORD['frozen_source_head']
p=argparse.ArgumentParser()
p.add_argument('--capture',required=True,type=Path)
p.add_argument('--round',required=True,type=Path)
p.add_argument('--expected-build-receipt-sha256',required=True)
p.add_argument('--expected-producer-receipt-sha256',required=True)
p.add_argument('--out',required=True,type=Path)
a=p.parse_args();capture=a.capture.resolve();round=a.round.resolve();out=a.out.resolve()
assert not out.exists(),'new actual output directory required; preserve earlier receipts'
assert round==Path(RECORD['required_round']).resolve(), 'exact50 round required'
assert hashlib.sha256((round/'expected-counts.json').read_bytes()).hexdigest()==RECORD['required_manifest_sha256'], 'exact50 manifest required'
assert not out.is_relative_to(capture) and not out.is_relative_to(ROOT),'output cannot recurse into capture or ROOT'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==HEAD,'ROOT drift; no historical adaptation authorized by this live checker'
assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=ROOT).strip(),'ROOT including untracked must be clean'
for name,expected in RECORD['old_source_sha256'].items():
 raw=(ROOT/name).read_bytes();assert hashlib.sha256(raw).hexdigest()==expected,'all source bytes must preflight before preparation'
 assert raw==(HERE/'frozen-source-before'/name).read_bytes()
command=['python3',str(HERE/'strict-828f-capture-only.py'),str(capture),'--repo',str(ROOT),'--expected-capture-head',HEAD,'--round',str(round),'--expected-build-receipt-sha256',a.expected_build_receipt_sha256,'--expected-producer-receipt-sha256',a.expected_producer_receipt_sha256]
result=subprocess.run(command,capture_output=True)
out.mkdir()
(out/'readonly-command.json').write_text(json.dumps(command,indent=2)+'\n')
(out/'readonly-plan.json').write_bytes(result.stdout);(out/'readonly.stderr').write_bytes(result.stderr)
assert result.returncode==0,'strict live validator refused; retained actual refusal, no patch generated'
plan=json.loads(result.stdout);assert plan['capture_head']==HEAD and len(plan['edits'])==11
meta={'capture_head':HEAD,'later_root_head_at_preparation':HEAD,'build_receipt_sha256':a.expected_build_receipt_sha256,'producer_receipt_sha256':a.expected_producer_receipt_sha256,'root':str(ROOT),'capture':str(capture),'round':str(round)}
(out/'trusted-inputs.json').write_text(json.dumps(meta,indent=2)+'\n')
for name in RECORD['old_source_sha256']:
 target=out/'source-before'/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((HERE/'frozen-source-before'/name).read_bytes())
subprocess.run(['python3',str(HERE/'archive-actual-and-build-patch.py'),str(out),str(capture),'39130d9237121b61e40b47f95c072c09a456554e'],check=True)
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==HEAD,'ROOT drift after validation; do not label current'
assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=ROOT).strip(),'ROOT drift after validation'
print(str(out/'prepared-patch-record.json'))
