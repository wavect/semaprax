#!/usr/bin/env python3
"""Bounded transport/source proof. No Rust, Cargo or performance claims."""
from pathlib import Path
import argparse,json,hashlib,subprocess,re,ast
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);a=p.parse_args();R=a.repo.resolve();D=Path(__file__).resolve().parent
J=json.loads((D/'source-inverses.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest();old={}
for row in J['rows']:
 before=(D/row['before_archive']).read_bytes();current=(R/row['source']).read_bytes()
 assert sha(before)==row['before_sha256']and sha(current)==row['current_sha256']
 assert before==subprocess.check_output(['git','show',J['base']+':'+row['source']],cwd=R)
 text=current.decode()
 for e in reversed(row['edits']):
  i=e['new_offset'];assert text[i:i+len(e['after'])]==e['after'];text=text[:i]+e['before']+text[i+len(e['after']):]
 assert text.encode()==before;old[row['source']]=before.decode()
# Lexer preserves complete literal bytes, including raw SPX strings; only host
# whitespace/comments may change under rustfmt.
def tokens(s):
 out=[];i=0
 while i<len(s):
  if s[i].isspace():i+=1;continue
  if s.startswith('//',i):
   j=s.find('\n',i);i=len(s)if j<0 else j+1;continue
  if s.startswith('/*',i):
   n=1;i+=2
   while n:
    if s.startswith('/*',i):n+=1;i+=2
    elif s.startswith('*/',i):n-=1;i+=2
    else:i+=1
   continue
  raw=re.match(r'r(#+)?"',s[i:])
  if raw:
   end='"'+(raw[1]or'');j=s.index(end,i+raw.end())+len(end);out.append(s[i:j]);i=j;continue
  if s[i]=='"':
   j=i+1
   while True:
    if s[j]=='\\':j+=2
    elif s[j]=='"':j+=1;break
    else:j+=1
   out.append(s[i:j]);i=j;continue
  m=re.match(r'[A-Za-z_]\w*',s[i:])
  if m:out.append(m[0]);i+=len(m[0])
  else:out.append(s[i]);i+=1
 return out
helper='macro_rules! audit_source_join { ($($piece:expr),* $(,)?) => {{ let pieces: &[&str] = &[$(&$piece),*]; pieces.concat() }}; }'
tiny='    assert_eq!(audit_source_join!("", "α", "\\0", "\\r\\n", "β"), "α\\0\\r\\nβ");\n'
paths=list(old)[:5]
for path in paths:
 text=(R/path).read_text()
 if path in [paths[0],paths[1],paths[3]]:
  assert text.startswith(helper);text=text[len(helper):]
  text=text.replace('audit_source_join!', 'concat!')
 else:assert helper not in text
 # Exact tiny cross-boundary assertion was added only to original owners.
 if path in [paths[0],paths[2],paths[4]]:
  # Alias is already reversed only in the HIR inventory owner.
  assertion=tiny.replace('audit_source_join!', 'concat!')if path==paths[0]else tiny
  assert text.count(assertion)==1;text=text.replace(assertion,'',1)
 text=text.replace('compiled-audit-pieces-798-next-20261016/','afba-presence-successor-20261016/')
 if path==paths[0]:text=text.replace('        &validation_raw,','        validation_raw,',1)
 if path==paths[2]:text=text.replace('historical_projection_root_facade(current_projection_root.as_str())','historical_projection_root_facade(current_projection_root)',1)
 if path==paths[4]:
  text=text.replace('    let hir_validator = hir_validator_source_inventory!();\n    let hir_validator = [hir_validator.as_str(), scope_projection::TEXT].concat();','    let hir_validator = [hir_validator_source_inventory!(), scope_projection::TEXT].concat();',1)
  for name in ['hir_resolver','verifier','lower']:text=text.replace(name+'.as_str()',name)
  # rustfmt adds only these optional two-element-tuple trailing commas when
  # the explicit borrow makes the original layout assertion line longer.
  for pin in ["size_of::<VerifierFrame<'static>>() == 320","size_of::<VariantMatchState<'static>>() == 312"]:
   text=text.replace('"'+pin+'",\n','"'+pin+'"\n',1)
 assert tokens(text)==tokens(old[path]),path
 # Every include occurrence and string separator is retained by the token proof.
 assert len((R/path).read_text().splitlines())<=1500
assert len((R/paths[3]).read_text().splitlines())==1500
# Reuse the unchanged literal Expr language to independently expand all four
# interop roots, including every alias repetition and separator. No old flattened
# payload is created in the Rust binary; this external proof streams current inputs.
parser=R/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py'
ns={};exec(compile(parser.read_text(),str(parser),'exec'),ns);mask=ns['mask']
gen=ast.parse((D/'original-generate-authority.py.txt').read_text())
parts=[n for n in gen.body if isinstance(n,ast.ClassDef)and n.name=='Expr' or isinstance(n,ast.FunctionDef)and n.name=='close']
env={'R':R,'re':re,'json':json};exec(compile(ast.Module(body=parts,type_ignores=[]),'original-literal-grammar','exec'),env)
def macro_arms(text):
 masked=mask(text);out={}
 for m in re.finditer(r'macro_rules!\s+(\w+)\s*\{',masked):
  end=env['close'](masked,m.end()-1,'{','}');body=text[m.end():end-1];bm=mask(body);arms={}
  for v in re.finditer(r'\(([^)]*)\)\s*=>\s*\{',bm):arms[v[1].strip()]=body[v.end():env['close'](bm,v.end()-1,'{','}')-1]
  out[m[1]]=arms
 return out
before_arms=macro_arms(old[paths[3]])
after_arms=macro_arms((R/paths[3]).read_text()[len(helper):].replace('audit_source_join!', 'concat!'))
expansion={};base=(R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs').parent
for name in ['hir_resolver_source_inventory','hir_validator_source_inventory','verifier_source_inventory','lower_source_inventory']:
 env['macros']=before_arms;before=env['Expr'](before_arms[name][''],base).expr()
 env['macros']=after_arms;after=env['Expr'](after_arms[name][''],base).expr();assert before==after,name
 digest=hashlib.sha256();size=0
 for chunk in after:
  b=(R/chunk['path']).read_bytes()if 'path'in chunk else chunk['literal'].encode();digest.update(b);size+=len(b)
 expansion[name]={'identical_ordered_chunks':len(after),'current_input_bytes':size,'current_joined_sha256':digest.hexdigest()}
(D/'expanded-interop-proof.json').write_text(json.dumps(expansion,indent=2)+'\n')

# Existing authority parser/classifier and six-negative algorithm stay exact.
g=ast.parse((D/'generate-authority.py').read_text());og=ast.parse((D/'original-generate-authority.py.txt').read_text())
for node in og.body:
 if isinstance(node,(ast.FunctionDef,ast.ClassDef)):
  current=next(n for n in g.body if type(n)==type(node)and n.name==node.name)
  assert ast.dump(node,include_attributes=False)==ast.dump(current,include_attributes=False),node.name
v=(D/'verify-authority.py').read_text();ov=(D/'original-verify-authority.py.txt').read_text()
assert v[:v.index('\n# Exact chunk order')]==ov[:ov.index('\nimport subprocess\n')]
# No shipping source, Cargo, runtime fixture or original table is changed.
changed=subprocess.check_output(['git','diff','--name-only',J['base']],cwd=R,text=True).splitlines()
assert all(not p.startswith('src/')and not p.endswith('Cargo.toml')and not p.endswith('Cargo.lock')for p in changed)
assert all(not p.startswith('docs/evidence/')or p.startswith(str(D.relative_to(R))+'/')for p in changed)
assert len(json.loads((D/'pending-selectors.json').read_text()))==3
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_before_rows':len(J['rows']),'exact_original_token_streams_after_declared_transport_and_borrow_adapters':5,'new_rust_owners':0,'old_classifier_parser_and_six_negatives_unchanged':True,'rust_executed':False,'memory_gain_claim':False}))
