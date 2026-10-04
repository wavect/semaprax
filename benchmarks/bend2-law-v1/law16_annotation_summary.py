import pathlib,json,re,hashlib
r=pathlib.Path('benchmarks/bend2-law-v1/evidence/law16-bounded-balance-v2/v2-turns-and-replay'); f=pathlib.Path('benchmarks/bend2-law-v1/fixtures'); rows=[]
for n in range(1,11):
 for lane,ext,seed in [('bend2','bend','bend-bounded-balance-transfer-law-gaming-v1.bend'),('semaprax-scalar-v1','spx','semaprax-bounded-balance-transfer-law-gaming-v1.spx')]:
  d=r/(lane if n==1 else f'ordinal-{n}/{lane}'); b=(d/f'final-source.{ext}').read_bytes();s=b.decode(); before=(f/seed).read_bytes();
  counts={k:len(re.findall(p,s,re.M)) for k,p in ({'law':r'^law ','proof_bodies':r'^def .*\(.*\):$'} if lane=='bend2' else {'requires':r'^\s+requires ','ensures':r'^\s+ensures ','stable_ids':r'^@id\('}).items()}
  rows.append({'ordinal':n,'language':lane,'annotation_counts':counts,'final_bytes':len(b),'changed_bytes_vs_seed':sum(x!=y for x,y in zip(b,before))+abs(len(b)-len(before)),'final_sha256':'sha256:'+hashlib.sha256(b).hexdigest()})
print(json.dumps({'schema':'semaprax.bend2-law-benchmark.annotation-summary.v1','rows':rows,'nonclaims':['byte difference is not semantic proof effort']},indent=2,sort_keys=True))
