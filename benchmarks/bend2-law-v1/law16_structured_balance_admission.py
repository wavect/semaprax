#!/usr/bin/env python3
"""Authenticate why the required structured balance cell cannot enter a matched route."""
import argparse,hashlib,json,pathlib
ROOT=pathlib.Path(__file__).parent
FIXTURE=ROOT/'fixtures/structured-balance-transfer-v1.json'
NONADMISSION=ROOT/'evidence/law16-checked-u32-nonadmission-v1/review.json'
SCHEMA='semaprax.bend2-law-benchmark.structured-balance-admission.v1'
def digest(p):return 'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
def review():
 fixture=json.loads(FIXTURE.read_text()); non=json.loads(NONADMISSION.read_text())
 if fixture.get('schema')!='semaprax.bend2-law-benchmark.fixture.v1' or fixture.get('id')!='structured-balance-transfer-v1' or fixture.get('numeric_domain')!='u32 checked':raise ValueError('structured balance fixture identity drifted')
 success=fixture.get('success');attack=fixture.get('attacks',{}).get('no-op-transfer')
 if not isinstance(success,list) or len(success)!=1 or not isinstance(attack,list) or len(attack)!=1:raise ValueError('structured balance controls drifted')
 s,a=success[0],attack[0]
 if s!={'before':[9,4],'amount':3,'after':[6,7]} or a!={'before':[9,4],'amount':3,'after':[9,4]}:raise ValueError('structured balance witness no longer binds conservation, state change, and no-op attack')
 if non.get('status')!='authenticated_unsupported_by_pinned_parser' or non.get('numeric_domain')!='u32 checked':raise ValueError('pinned u32 non-admission receipt drifted')
 return {'schema':SCHEMA,'status':'unsupported','cell':'structured-balance-transfer-v1','fixture':{'path':'fixtures/structured-balance-transfer-v1.json','sha256':digest(FIXTURE)},'laws':['conservation','intended-state-change','nonnegative-balances'],'attack':'no-op-transfer','semantic_witness':s,'attack_witness':a,'admission_blocker':{'receipt':'evidence/law16-checked-u32-nonadmission-v1/review.json','sha256':digest(NONADMISSION),'reason':'the pinned SEMAPRAX parser rejects u32 sources before a matched success-plus-overflow or Z3 route can run'},'nonclaims':['the 0..100 bounded-balance witness is not this checked-u32 cell','i32, i64, u8, and usize are not substitutes','no Bend/SEMAPRAX matched execution, proof, timing, or winner result']}
def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',required=True,type=pathlib.Path);a=p.parse_args(argv)
 if a.output.exists() or not a.output.parent.is_dir():p.error('output must be new')
 try:v=review()
 except (OSError,ValueError,json.JSONDecodeError) as e:p.error(str(e))
 a.output.write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
