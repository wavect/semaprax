#!/usr/bin/env python3
"""Authenticate the committed bounded-balance local raw capsule without tools."""
import argparse,hashlib,json,pathlib
SCHEMA='semaprax.bend2-law-benchmark.bounded-balance-raw-capsule.v1'; MAX=32*1024*1024

def review(root):
 m=json.loads((root/'manifest.json').read_text())
 if m.get('schema')!=SCHEMA or m.get('status')!='local_unhosted_raw_evidence': raise ValueError('unsupported capsule')
 seen=set()
 for x in m.get('files',[]):
  p=x.get('path'); q=root/pathlib.PurePosixPath(p or '')
  if not isinstance(p,str) or p.startswith('/') or '..' in pathlib.PurePosixPath(p).parts or p in seen: raise ValueError('unsafe inventory')
  seen.add(p); b=q.read_bytes()
  if len(b)!=x.get('bytes') or len(b)>MAX or 'sha256:'+hashlib.sha256(b).hexdigest()!=x.get('sha256'): raise ValueError('inventory mismatch')
 markers=(b'api_key',b'authorization',b'bearer',b'password',b'secret')
 hits=[]
 for p in root.rglob('events.jsonl'):
  b=p.read_bytes().lower()
  if any(x in b for x in markers): hits.append(p.relative_to(root).as_posix())
 if hits: raise ValueError('credential marker in raw events')
 a=m.get('aggregate',{})
 expected={'v2_matched_pairs':10,'bend_candidate_passes':9,'bend_candidate_failures':1,'semaprax_z3_candidate_discharges':10,'attack_debit_rejections':10,'attack_credit_discharges':10,'attack_total_rejections':10,'v1_ineligible_pilot':1}
 if a!=expected: raise ValueError('aggregate drift')
 return {'schema':SCHEMA+'.review.v1','status':'authenticated_local_observation','aggregate':a,'cost_usage':{'status':'unavailable'},'nonclaims':['not checked-u32','not a general theorem or incremental cell','not complete LAW-16']}
def main():
 p=argparse.ArgumentParser();p.add_argument('--capsule',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args();a.output.write_text(json.dumps(review(a.capsule),indent=2,sort_keys=True)+'\n')
if __name__=='__main__': main()
