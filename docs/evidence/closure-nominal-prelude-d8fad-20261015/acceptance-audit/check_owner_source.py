from pathlib import Path
import json,subprocess,re,hashlib
R=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang');D=Path(__file__).resolve().parent;o=json.loads((D/'required-owner-map.json').read_text());prefix=Path('/Volumes/Semaprax/provenance/qualification-registration-module-path-20261014.py').read_text().split("parse(R/'src/lib.rs'")[0];n={};exec(prefix,n)
def body(s,name):
 masked=n['mask'](s);matches=list(re.finditer(r'\bfn\s+'+re.escape(name)+r'\s*\(',masked));assert len(matches)==1,(name,len(matches));start=matches[0].start();a=masked.index('{',matches[0].end());depth=1;b=a+1
 while depth:
  if masked[b]=='{':depth+=1
  if masked[b]=='}':depth-=1
  b+=1
 return s[start:b]
rows=[]
for row in o['issues']['827']:
 s=row['selector'];r=row['latest_actual'];P=R/r['receipt'];M=P.parent/'expected-counts.json'
 if not M.exists():M=P.parent/'compilation-manifest.json'
 try:m=json.loads(M.read_text());seed=next(x for x in m['checks'] if x['selector']==s);path=seed['source_binding']['source_path']
 except Exception:
  name=s.rsplit('::',1)[-1];result=subprocess.check_output(['rg','-l',r'\bfn\s+'+re.escape(name)+r'\s*\(','src','tests'],cwd=R,text=True).splitlines();result=[p for p in result if re.search(r'\bfn\s+'+re.escape(name)+r'\s*\(',n['mask']((R/p).read_text()))];assert len(result)==1,(s,result);path=result[0]
 old=subprocess.check_output(['git','show',r['head']+':'+path],cwd=R,text=True);new=(R/path).read_text();oldbody=body(old,s.rsplit('::',1)[-1]);newbody=body(new,s.rsplit('::',1)[-1]);rows.append(dict(selector=s,source_path=path,actual_head=r['head'],whole_source_exact=old==new,whole_test_body_exact=oldbody==newbody,old_body_sha256=hashlib.sha256(oldbody.encode()).hexdigest(),current_body_sha256=hashlib.sha256(newbody.encode()).hexdigest()))
(D/'owner-source-comparison.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps({'rows':len(rows),'whole_file_exact':sum(x['whole_source_exact'] for x in rows),'body_exact':sum(x['whole_test_body_exact'] for x in rows),'changed_bodies':[(x['selector'],x['actual_head'][:8]) for x in rows if not x['whole_test_body_exact']]},indent=2))
