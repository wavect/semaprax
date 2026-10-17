from pathlib import Path
import json,hashlib,subprocess,ast
R=Path('/Volumes/Semaprax/worktrees/wasm-custody-compile-repair-20261017');BASE='0f9fb3766f0413694e4a12447538f7c7bb99627f';REL='docs/evidence/wasm-workspace-custody-compile-repair-20261017';D=R/REL;ST='docs/evidence/main-e177-final-source-binding-20261017';H=lambda b:hashlib.sha256(b).hexdigest();G=lambda p:subprocess.check_output(['git','show',BASE+':'+p],cwd=R);records=[]
def edit(p,new,ops):
 b=G(p);assert (R/p).read_bytes()==b
 a=REL+'/before/'+p.replace('/','__')+'.txt';(R/a).parent.mkdir(parents=True,exist_ok=True);(R/a).write_bytes(b);(R/p).write_bytes(new)
 records.append(dict(source=p,before=dict(length=len(b),sha256=H(b),archive=a),after=dict(length=len(new),sha256=H(new)),edits=ops,restore_complete_before=a))
p='docs/evidence/main-e177-full-controller-acquisition-20261017/inputs.json';old=G(p);packet=json.loads(old);assert len(packet['rows'])==471
row=next(x for x in packet['rows']if x['path']=='tests/workspace.rs');b=G(row['path']);a=(R/row['path']).read_bytes();assert (row['current_length'],row['current_sha256'])==(len(b),H(b));op=dict(source=row['path'],before_current_length=len(b),before_current_sha256=H(b),after_current_length=len(a),after_current_sha256=H(a));row['current_length']=len(a);row['current_sha256']=H(a)
new=(json.dumps(packet,indent=2)+'\n').encode();assert len(new)<=262144;undo=json.loads(new);u=next(x for x in undo['rows']if x['path']=='tests/workspace.rs');u['current_length']=len(b);u['current_sha256']=H(b);assert undo==json.loads(old)
# Full incoming archives, names, fields and order are unchanged; exactly one current tuple changes.
edit(p,new,[op]);e177old=old;e177new=new
helper='src/source_reflows_merge/main_e177_inputs.rs';raw=G(helper);s=raw.decode();ops=[]
for before,after in [(f'assert_eq!(data.len(), {len(old)}); // MAIN_E177_PACKET_LENGTH',f'assert_eq!(data.len(), {len(new)}); // MAIN_E177_PACKET_LENGTH'),(H(old),H(new))]:
 if before==after:continue
 assert s.count(before)==1;s=s.replace(before,after);ops.append(dict(before=before,after=after,count=1))
edit(helper,s.encode(),ops);e177body=s.encode()
p=ST+'/current-inputs.json';old=G(p);packet=json.loads(old);assert len(packet['rows'])==49;row=next(x for x in packet['rows']if x['source']==helper);assert (row['current']['length'],row['current']['sha256'])==(len(raw),H(raw));archive=ST+'/current/'+H(e177body)+'.txt';(R/archive).write_bytes(e177body);row['current']=dict(length=len(e177body),sha256=H(e177body),archive=archive);new=(json.dumps(packet,indent=2)+'\n').encode();assert len(new)<=262144;edit(p,new,[dict(operation='replace_only_complete_current_descriptor',source=helper,before_current=json.loads(old)['rows'][[x['source']for x in json.loads(old)['rows']].index(helper)]['current'],after_current=row['current'])]);old49=old;new49=new
helper2='src/source_reflows_merge/main_e177_final_inputs.rs';raw2=G(helper2);s=raw2.decode();ops=[]
for before,after in [(f'assert_eq!(data.len(), {len(old49)});',f'assert_eq!(data.len(), {len(new49)});'),(H(old49),H(new49))]:
 if before==after:continue
 assert s.count(before)==1;s=s.replace(before,after);ops.append(dict(before=before,after=after,count=1))
edit(helper2,s.encode(),ops);finalbody=s.encode()
gp=ST+'/guard_payload.py';raw=G(gp);text=raw.decode();node=next(n.value for n in ast.parse(text).body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id=='CONFIG');cfg=ast.literal_eval(node);cfg['packet'].update(length=len(new49),sha256=H(new49));assert cfg['rows']==49
for h,body in [(helper,e177body),(helper2,finalbody)]:cfg['modules'][h].update(length=len(body),sha256=H(body))
lines=text.splitlines(True);a=sum(map(len,lines[:node.lineno-1]))+node.col_offset;z=sum(map(len,lines[:node.end_lineno-1]))+node.end_col_offset;actual=text[:a]+repr(cfg)+text[z:];ast.parse(actual);edit(gp,actual.encode(),[dict(before=text[a:z],after=repr(cfg),count=1)]);guardold=raw;guardnew=actual.encode()
for path in ['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']:
 raw=G(path);t=raw.decode();assert t.count(H(guardold))==1;t=t.replace(H(guardold),H(guardnew));ops=[dict(before=H(guardold),after=H(guardnew),count=1)]
 if len(guardold)!=len(guardnew):
  prefix='guard_raw=held(guard_helper,'if 'verify-or-refresh' in path else 'helper_raw=bounded(helper,'
  before=prefix+str(len(guardold));after=prefix+str(len(guardnew));assert t.count(before)==1;t=t.replace(before,after);ops.append(dict(before=before,after=after,count=1))
 ast.parse(t);edit(path,t.encode(),ops)
(D/'binding-inverses.json').write_text(json.dumps(dict(base=BASE,status='SOURCE_ONLY',rows=records,incoming471_row_order_and_all_incoming_fields_unchanged=True,current49_row_order_all_views_and_all_other_rows_unchanged=True,all_old_packets_retained_whole=True,guard_algorithm_body_unchanged=True),indent=2)+'\n')
print(json.dumps(dict(guard_length=len(guardnew),guard_sha256=H(guardnew),packet49_length=len(new49),packet49_sha256=H(new49),packet471_length=len(e177new),packet471_sha256=H(e177new),changed_bindings=len(records))))
