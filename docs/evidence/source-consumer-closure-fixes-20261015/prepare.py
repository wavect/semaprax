"""Finite actual consumer-source input joins, not runtime qualification."""
from pathlib import Path
import json,hashlib,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='3d4b996c9a8cd2b100a3c594e741314c108b5591';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def put(n,x):(D/n).write_text(json.dumps(x,indent=2)+'\n')
def row(p,current,target):return {'source':p,'current_sha256':sha(current),'original_sha256':sha(target),'edits':[{'new':current.decode(),'old':target.decode()}]}
contexts={'decl825':'docs/evidence/constructor-declaration-rewrite-825/source-inverses.json','cost825':'docs/evidence/constructor-cost-propagation-825/source-inverses.json','signature825':'docs/evidence/constructor-signature-rewrite-825/source-inverses.json','capacity834':'docs/evidence/private-layout-entry-capacity-834/exact-source-inverses.json','registry833':'docs/evidence/registry-census-scratch-833/source-inverses.json','lexical832':'docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json','cold835':'docs/evidence/canonical-comment-comparison-835/source-inverses.json'}
counts={}
for domain,packet in contexts.items():
 data=json.loads((R/packet).read_text());old=data['owners'] if isinstance(data,dict) else data;rows=[]
 for r in old:
  p=r['source'];cur=(R/p).read_bytes()
  if sha(cur)==r['current_sha256']:continue
  assert len(r['edits'])>=1 and 'new' in r['edits'][0],(domain,p,'complete input required');target=r['edits'][0]['new'].encode();assert sha(target)==r['current_sha256'],(domain,p)
  new=row(p,cur,target);new['target_manifest']=packet;new['target_complete_input_field']='edits[0].new';rows.append(new)
 put(domain+'.json',rows);counts[domain]=len(rows)
prior=json.loads((R/'docs/evidence/inline-weighted-successor-842/source-inverses.json').read_text());receiving=[]
for p in ['src/cleanup_plan/replay/factored/canonical_shared/singleton_target_tests.rs','src/cleanup_plan/replay/factored/canonical_shared/successor_targets.rs']:
 r=next(r for r in prior if r['source']==p);target=r['edits'][0]['old'].encode();assert sha(target)==r['original_sha256'];new=row(p,(R/p).read_bytes(),target);new['target_manifest']='docs/evidence/inline-weighted-successor-842/source-inverses.json';new['target_complete_input_field']='edits[0].old';receiving.append(new)
put('receiving831.json',receiving);counts['receiving831']=len(receiving)
paths=git('diff','--name-only',BASE,'--','src','tests','crates').decode().splitlines();rows=[]
for p in paths:
 raw=(R/p).read_bytes();before=git('show',BASE+':'+p);r=row(p,raw,before);r['checkpoint_revision']=BASE;rows.append(r)
put('current-to-3d4b.json',rows);put('contexts.json',{'basis':BASE,'consumer_packets':contexts,'input_counts':counts,'source_owners':paths,'new_rust_selectors':0,'source_only':True})
parser=Path('/Volumes/Semaprax/provenance/status-memo-traversal-final-registration-v2-20261015/registration-prefix-original.py').read_bytes();(D/'module-parser-original.py.txt').write_bytes(parser)
print('prepared',len(rows),'whole source owners;',counts)
