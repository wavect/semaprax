from pathlib import Path
import json,hashlib,subprocess,re,ast
R=Path('/Volumes/Semaprax/worktrees/main-e177-source-integration-20261017');HEAD='5dff900cd1c3004437e3f706547ef55eda254563';AFA='afa1e4d9da88da6420776ae3184cc9efb552d022';BASE='9ad00e8d04eb64fb6cba987ae5c9eabd496deca0';REL='docs/evidence/main-e177-final-source-binding-20261017';D=R/REL;D.mkdir(parents=True,exist_ok=True);OUT=Path(__file__).parent;MODULE='src/source_reflows_merge/main_e177_final_inputs.rs';API='crate::source_reflows_merge::main_e177_final_inputs';H=lambda b:hashlib.sha256(b).hexdigest();git=lambda *x:subprocess.check_output(['git',*x],cwd=R);G=lambda rev,p:git('show',rev+':'+p)
assert git('rev-parse','HEAD').decode().strip()==HEAD
changes=[]
def edit(path,steps):
 b=G(HEAD,path);s=b.decode();ops=[]
 for a,z,n in steps:assert s.count(a)==n,(path,a,s.count(a));s=s.replace(a,z);ops.append(dict(before=a,after=z,count=n))
 restored=s
 for e in reversed(ops):assert restored.count(e['after'])==e['count'];restored=restored.replace(e['after'],e['before'])
 assert restored.encode()==b
 a=s.encode();assert (R/path).read_bytes()in(b,a),path;before=REL+'/before/'+path+'.txt';p=R/before;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b);(R/path).write_bytes(a);changes.append(dict(source=path,before=dict(length=len(b),sha256=H(b),archive=before),after=dict(length=len(a),sha256=H(a)),edits=ops))
# Necessary test-only inherited scope repair. Original child tests unchanged.
p='src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs';n='use crate::cleanup_plan::replay::replay_error;';edit(p,[(n,n+'\n#[cfg(test)]\nuse super::{ExpressionId, StatusProducer, StatusSource, MAX_REPLAY_WORK_UNITS};',1)])
# Old54 complete helper/body/table preserved; only exact new-stage acquisition before its prior actual check.
p='src/source_reflows_merge/next_big_batch_inputs.rs';edit(p,[('let value=held(&root.join(source),current["length"].as_u64().unwrap()as usize,current["sha256"].as_str().unwrap()).unwrap();','let value='+API+'::file(&root.join(source),current["length"].as_u64().unwrap()as usize,current["sha256"].as_str().unwrap())\n        .unwrap_or_else(|| held(&root.join(source),current["length"].as_u64().unwrap()as usize,current["sha256"].as_str().unwrap()).unwrap());',1),('if row["compiled"].as_bool().unwrap() { assert_eq!(Some(value.as_str()),compiled(source),"complete actual compiled source"); }','if row["compiled"].as_bool().unwrap() {\n        let compiled='+API+'::value(source,current["sha256"].as_str().unwrap(),compiled(source).unwrap());\n        assert_eq!(value,compiled,"complete actual compiled source");\n    }',1)])
p='src/source_reflows_merge/main99f_inputs.rs';edit(p,[('let actual=read(path,row["current_length"].as_u64().unwrap()as usize,row["current_sha256"].as_str().unwrap())?;','let actual=if let Some(prior)='+API+'::file(path,row["current_length"].as_u64().unwrap()as usize,row["current_sha256"].as_str().unwrap()) {prior}else{read(path,row["current_length"].as_u64().unwrap()as usize,row["current_sha256"].as_str().unwrap())?};',1),('if let Some(compiled)=compiled(relative) {assert_eq!(actual,compiled,"whole current compiled MAIN99F input");}','if let Some(compiled)=compiled(relative) {\n        let compiled='+API+'::value(relative,row["current_sha256"].as_str().unwrap(),compiled);\n        assert_eq!(actual,compiled,"whole current compiled MAIN99F input");\n    }',1)])
p='src/source_reflows_merge.rs';b=G(HEAD,p).decode();tail='\n#[path = "source_reflows_merge/main_e177_final_inputs.rs"]\npub(crate) mod main_e177_final_inputs;\n';edit(p,[(b[-120:],b[-120:]+tail,1)])
new=sorted({x for x in git('diff','--name-only','--diff-filter=A',BASE,HEAD,'--','*.rs').decode().splitlines()if x.startswith(('src/','tests/','crates/'))}|{MODULE});assert len(new)==36
# Actual raw3 joined source union, no occurrence removed.
carriers=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
for p in carriers:
 base='../'if p==carriers[0]else('../../'if p==carriers[1]else'../../../../../');s=(R/p).read_text();needle='include_str!("'+base+'src/cleanup_plan/replay/factored/canonical_shared.rs")';missing=[x for x in new if 'include_str!("'+base+x+'")'not in s];addition=''.join(', include_str!("'+base+x+'")'for x in missing);steps=[(needle,needle+addition,1)]
 if p==carriers[2]:
  # Compact only newly-main-added literal arms. Preserve all content and ordering.
  for x in ['src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/reused_cursor.rs','src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/reused_cursor/tests.rs']:
   match=re.search(r'\n([ \t]*)(include_str!\("'+re.escape(base+x)+r'"\),?)',s)
   if match:steps.append((match[0],' '+match[2],1))
 edit(p,steps);after=(R/p).read_text();assert len(after.splitlines())<=1500,(p,len(after.splitlines()))
 for x in new:assert after.count('include_str!("'+base+x+'")')==1,(p,x)
# One original carrier-expression parser; no repo graph rebuild.
parser_path='docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py';parser=G(HEAD,parser_path);assert H(parser)=='87a903ce0f9927d3d90063fdd686b50b5302e2ac10cfea324918f7c030b28599';ns={};exec(compile(parser,parser_path,'exec'),ns)
ep='docs/evidence/a3-e47-final-custody-20261016/generate-authority.py';expr=G(HEAD,ep);assert H(expr)=='0c4b31857cdb8a4f645d61c6f0d2453c33c6adddb8c6f26d9c37b56620841590';defs=[n for n in ast.parse(expr).body if isinstance(n,(ast.FunctionDef,ast.ClassDef))and n.name in ('close','Expr')];scope=dict(R=R,re=re,json=json,mask=ns['mask']);exec(compile(ast.Module(body=defs,type_ignores=[]),ep,'exec'),scope)
JOIN='macro_rules! audit_source_join { ($($piece:expr),* $(,)?) => {{ let pieces: &[&str] = &[$(&$piece),*]; pieces.concat() }}; }'
s=(R/carriers[1]).read_text();assert s.count(JOIN)==1;inv=s.replace(JOIN,'',1).replace('audit_source_join!','concat!');masked=ns['mask'](inv);macros={}
for t in re.finditer(r'macro_rules!\s+(\w+)\s*\{',masked):
 end=scope['close'](masked,t.end()-1,'{','}');body=inv[t.end():end-1];bm=ns['mask'](body);arms={}
 for v in re.finditer(r'\(([^)]*)\)\s*=>\s*\{',bm):arms[body[v.start(1):v.end(1)].strip()]=body[v.end():scope['close'](bm,v.end()-1,'{','}')-1]
 macros[t[1]]=arms
scope['macros']=macros;pieces=scope['Expr'](macros['semantic_graph_private_layout_source_inventory'][''],R/'tests/workspace').expr();assert all(set(x)=={'path'}for x in pieces);order=[x['path']for x in pieces]
prior='docs/evidence/next-big-batch-source-binding-20261017/current-projection-inputs.tsv';old=G(AFA,prior);lines=old.decode().splitlines();added=[p for p in order if p not in lines];assert[p for p in order if p not in added]==lines;assert len(order)==len(set(order))
ins=[dict(at=i,path=p)for i,p in enumerate(order)if p in added];check=order.copy()
for x in reversed(ins):assert check.pop(x['at'])==x['path']
assert ('\n'.join(check)+'\n').encode()==old;table=REL+'/current-projection-inputs.tsv';(R/table).write_text('\n'.join(order)+'\n');(D/'prior426.whole.tsv.txt').write_bytes(old)
p='tests/workspace/semantic_graph.rs';edit(p,[(prior,table,1)])
# Parent/raw/new-module metadata is sealed after the new source helper and E177 tuple reseal.
(D/'hook-and-raw-inverses.json').write_text(json.dumps(dict(status='SOURCE_ONLY_PENDING_FINAL_SEAL',head=HEAD,rows=changes,new_modules=new,raw_carriers=carriers,ordered_table=dict(path=table,rows=len(order),prior=prior,prior_rows=len(lines),insertions=ins,complete_prior_archive=REL+'/prior426.whole.tsv.txt')),indent=2)+'\n')
print(json.dumps({'modules':len(new),'tsv_rows':len(order),'insertions':len(ins),'native_lines':len((R/carriers[2]).read_bytes().splitlines())}))
