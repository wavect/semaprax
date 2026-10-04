#!/usr/bin/env python3
"""Emit explicit LAW-16 nonresults for cells without matched numeric semantics."""
import json,pathlib
CELLS=(
 'structured-balance-transfer-v1',
 'supported-list-theorem-v1',
 'law-preserving-refactor-v1',
 'law-breaking-agent-edit-v1',
 'project-incremental-edit-v1',
)
def main():
 root=pathlib.Path(__file__).with_name('fixtures'); rows=[]
 for cell in CELLS:
  x=json.loads((root/(cell+'.json')).read_text())
  if x['numeric_domain']!='u32 checked': raise ValueError('unexpected domain')
  rows.append({'id':cell,'status':'unsupported','reason':'SEMAPRAX reviewed scalar profile lacks checked u32; Bend U32 cannot be relabelled as i32/i64','fixture_sha256':__import__('hashlib').sha256((root/(cell+'.json')).read_bytes()).hexdigest()})
 print(json.dumps({'schema':'semaprax.bend2-law-benchmark.law16-unavailable-cells.v1','cells':rows,'nonclaims':['not an executed benchmark result','not a win']},indent=2,sort_keys=True))
if __name__=='__main__': main()
