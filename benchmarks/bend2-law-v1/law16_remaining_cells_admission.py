#!/usr/bin/env python3
"""Bind the remaining LAW-16 u32 fixture controls to the pinned non-admission gate."""
import argparse,hashlib,json,pathlib
ROOT=pathlib.Path(__file__).parent
NON=ROOT/'evidence/law16-checked-u32-nonadmission-v1/review.json'
CELLS={
 'supported-list-theorem-v1':(['permutation-and-multiplicity','sortedness'],'empty-sort','list theorem needs list/source-proof route; none is retained'),
 'law-preserving-refactor-v1':(['before-after-observational-equivalence'],'law-dropped-during-refactor','refactor equivalence route is not retained'),
 'law-breaking-agent-edit-v1':(['declared-law-inventory'],'agent-law-gaming','law-inventory preservation route is not retained'),
}
SCHEMA='semaprax.bend2-law-benchmark.remaining-u32-cells-admission.v1'
def digest(p):return 'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
def review():
 non=json.loads(NON.read_text())
 if non.get('status')!='authenticated_unsupported_by_pinned_parser':raise ValueError('pinned non-admission receipt drifted')
 rows=[]
 for cell,(laws,attack,extra) in CELLS.items():
  p=ROOT/'fixtures'/f'{cell}.json';v=json.loads(p.read_text())
  if v.get('schema')!='semaprax.bend2-law-benchmark.fixture.v1' or v.get('id')!=cell or v.get('numeric_domain')!='u32 checked' or not isinstance(v.get('success'),list) or len(v['success'])!=1 or set(v.get('attacks',{}))!={attack}:raise ValueError(f'{cell} fixture/attack identity drifted')
  rows.append({'id':cell,'status':'unsupported','fixture':{'path':f'fixtures/{cell}.json','sha256':digest(p)},'laws':laws,'attack':attack,'success_witness':v['success'][0],'attack_witness':v['attacks'][attack][0],'reason':'pinned SEMAPRAX parser rejects checked u32 before a matched route can start','additional_unobserved_route':extra})
 return {'schema':SCHEMA,'status':'unsupported','admission_blocker':{'receipt':'evidence/law16-checked-u32-nonadmission-v1/review.json','sha256':digest(NON)},'cells':rows,'nonclaims':['not executable matched evidence','no i32, i64, u8, or usize substitute','no list theorem, refactor, or agent-edit acceptance result']}
def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',required=True,type=pathlib.Path);a=p.parse_args(argv)
 if a.output.exists() or not a.output.parent.is_dir():p.error('output must be new')
 try:v=review()
 except (OSError,ValueError,json.JSONDecodeError) as e:p.error(str(e))
 a.output.write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
