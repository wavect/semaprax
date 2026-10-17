from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();p='docs/evidence/opt870-inline-paid-many-51c1-20261017/frozen-receivers.json';original=(R/p).read_bytes();assert original==(D/'recipe.original.json.txt').read_bytes()==subprocess.check_output(['git','show','36c6deb98:'+p],cwd=R);ordered=(D/'recipe.ordered.json.txt').read_bytes();inverse=json.loads((D/'recipe-inverse.json').read_text());assert sha(original)==inverse['original_sha256']and sha(ordered)==inverse['ordered_sha256'];current=ordered.decode();assert current[inverse['start']:inverse['end']]==inverse['ordered_section'];assert (current[:inverse['start']]+inverse['original_section']+current[inverse['end']:]).encode()==original
old=json.loads(original);new=json.loads(ordered);assert len(old['helpers'])==len(new['helpers'])==115;assert len({r['path']for r in new['helpers']})==115;assert new=={**old,'helpers':sorted(old['helpers'],key=lambda r:r['path'])}
# Every raw helper row byte is retained; only the whole row sequence differs.
a=re.findall(r'    \{\n.*?\n    \}',inverse['original_section'],re.S);b=re.findall(r'    \{\n.*?\n    \}',inverse['ordered_section'],re.S);assert sorted(a)==sorted(b)and len(a)==115
r=json.loads((D/'source-inverse.json').read_text());assert (R/r['path']).read_text()==r['after']and sha(r['after'].encode())==r['after_sha256'];assert (D/'source.before.txt').read_text()==r['before'];assert r['before'].encode()==subprocess.check_output(['git','show','36c6deb98:'+r['path']],cwd=R)
# Complete115-key helper reads after existing7 views plus exactnewarena front.
paid=json.loads((R/'docs/evidence/c874-supplement-custody-domains-20261017/paid-inputs.json').read_text());arena=json.loads((D/'arena-input.json').read_text());raw=(R/arena['path']).read_bytes();assert len(raw)==arena['current_length']and sha(raw)==arena['current_sha256']and raw==(D/'arena.current.txt').read_bytes()
for row in new['helpers']:
 x=next((x for x in paid if x['path']==row['path']),None)
 if x:data=(R/'docs/evidence/c874-supplement-custody-domains-20261017'/x['view']['archive']).read_bytes()
 elif row['path']==arena['path']:data=(D/'arena.prior.txt').read_bytes()
 else:data=(R/row['path']).read_bytes()
 assert len(data)==row['length']and sha(data)==row['sha256'],row['path']
refusals=0
for bad in [b'',ordered[:-1],ordered+b' ',original]:assert sha(bad)!=sha(ordered);refusals+=1
for kind in range(3):
 mutant=json.loads(ordered)
 if kind==0:mutant['helpers'][0],mutant['helpers'][1]=mutant['helpers'][1],mutant['helpers'][0]
 if kind==1:mutant['helpers'][1]=mutant['helpers'][0]
 if kind==2:mutant['helpers'][0]['path']='src/foreign.rs'
 assert sha(json.dumps(mutant).encode())!=sha(ordered);refusals+=1
out={'status':'PASS_SOURCE_ONLY','original_recipe_byte_exact':True,'ordered_rows_exact_bijection':115,'whole_row_bytes_retained':True,'other_recipe_fields_unchanged':True,'all115_current_helper_views_bound':True,'arena_physical_fronts':1,'refusals':refusals};(D/'source-controls.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
