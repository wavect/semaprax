"""Explicit actual-build/readiness gate around the unchanged role-fixed producer."""
from pathlib import Path
import argparse,hashlib,json,subprocess
HERE=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);p.add_argument('--expected-build-receipt-sha256',required=True);p.add_argument('--expected-readiness-sha256',required=True);a=p.parse_args()
r=json.loads((HERE/'skeleton-record.json').read_text());round=Path(r['required_round']);root=Path(r['repository']);ready=HERE/'actual-completed-build-artifact-readiness.json'
def sha(p):
 with Path(p).open('rb')as f:return hashlib.file_digest(f,'sha256').hexdigest()
assert len(a.expected_build_receipt_sha256)==len(a.expected_readiness_sha256)==64
assert sha(round/'owning-build-result.json')==a.expected_build_receipt_sha256 and sha(ready)==a.expected_readiness_sha256
proof=json.loads(ready.read_text());assert proof['source_subject']==r['frozen_source_head'] and proof['actual_build_receipt_sha256']==a.expected_build_receipt_sha256 and proof['manifest_sha256']==r['required_manifest_sha256']
output=a.out.resolve();assert not output.exists() and not output.is_relative_to(root)
preflight=output.with_name(output.name+'.readiness-preflight.json');assert not preflight.exists()
# Reuse the actual readonly checker, including four executed/settled commands and
# lib/workspace + both production CLI artifact roles. No stale executable fallback.
subprocess.run(['python3',str(HERE/'check-ready.py'),'--round',str(round),'--expected-build-receipt-sha256',a.expected_build_receipt_sha256,'--out',str(preflight)],check=True)
assert preflight.read_bytes()==ready.read_bytes()
producer=Path(r['role_fixed_producer']['path']);assert sha(producer)==r['role_fixed_producer']['sha256']
command=['python3',str(producer),'--round',str(round),'--out',str(output)]
raise SystemExit(subprocess.run(command).returncode)
