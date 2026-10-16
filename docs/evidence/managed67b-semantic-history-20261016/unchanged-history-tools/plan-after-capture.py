#!/usr/bin/env python3
"""Require actual producer3 readiness, then run the unchanged strict107 plan pipeline."""
from pathlib import Path
import argparse,json,hashlib,subprocess
P=argparse.ArgumentParser();P.add_argument('--expected-head',required=True);P.add_argument('--round',required=True,type=Path);P.add_argument('--capture',required=True,type=Path);P.add_argument('--readiness',required=True,type=Path);P.add_argument('--expected-contract-sha256',required=True);P.add_argument('--expected-manifest-sha256',required=True);P.add_argument('--expected-build-receipt-sha256',required=True);P.add_argument('--expected-producer-receipt-sha256',required=True);P.add_argument('--expected-readiness-sha256',required=True);P.add_argument('--out',required=True,type=Path);a=P.parse_args();D=Path(__file__).resolve().parent
TOOLS=Path('/Volumes/Semaprax/provenance/capture83ca-main65-derivative-20261016')
def sha(p):
 with Path(p).open('rb')as f:return hashlib.file_digest(f,'sha256').hexdigest()
record=json.loads((D/'skeleton-record.json').read_text());derivation=json.loads((D/'tool-derivation.json').read_text());assert a.expected_head==record['frozen_source_head']and a.round.resolve()==Path(record['required_round']).resolve()
assert a.expected_contract_sha256==record['producer_build_contract_sha256']and a.expected_manifest_sha256==record['required_manifest_sha256']
assert sha(TOOLS/'check-ready.py')==derivation['checker_sha256']and sha(a.readiness)==a.expected_readiness_sha256
proof=json.loads(a.readiness.read_text());assert proof['status']=='ACTUAL_PRODUCER_ONLY_THREE_BUILD_ARTIFACTS_VALIDATED_NO_CAPTURE'and proof['source_subject']==a.expected_head
assert not a.out.exists();preflight=a.out.with_name(a.out.name+'.actual-three-readiness-preflight.json');assert not preflight.exists()
subprocess.run(['python3',str(TOOLS/'check-ready.py'),'--expected-head',a.expected_head,'--round',str(a.round),'--expected-contract-sha256',a.expected_contract_sha256,'--expected-manifest-sha256',a.expected_manifest_sha256,'--expected-build-receipt-sha256',a.expected_build_receipt_sha256,'--out',str(preflight)],check=True)
assert preflight.read_bytes()==a.readiness.read_bytes()
for n,h in record['trusted_tool_sha256'].items():assert sha(D/n)==h
# Both strict core and fullsource107/archive stage are original algorithms.
subprocess.run(['python3',str(D/'prepare-after-actual-capture.py'),'--capture',str(a.capture),'--round',str(a.round),'--expected-build-receipt-sha256',a.expected_build_receipt_sha256,'--expected-producer-receipt-sha256',a.expected_producer_receipt_sha256,'--out',str(a.out)],check=True)
