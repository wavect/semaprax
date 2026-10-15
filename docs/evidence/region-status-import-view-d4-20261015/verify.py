"""Source-only full region-entry runtime-domain repair; no Rust execution."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess,copy
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='a0c1213695b475c585e94cb599fedc218aa37ef5';sha=lambda b:hashlib.sha256(b).hexdigest()
def get(rev,p):return subprocess.check_output(['git','show',rev+':'+p])
def apply(row,b):
 assert sha(b)==row['current_sha256']
 for e in row['edits']:
  a=e['new'].encode();assert b.count(a)==1;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==row['original_sha256'];return b
rows=json.loads((D/'current-to-a0c.json').read_text());runtime=json.loads((D/'runtime-views.json').read_text());negative=0
for row in rows+runtime:
 raw=(R/row['source']).read_bytes();old=apply(row,raw);assert old==(R/row['archive']).read_bytes()
 if row in rows:assert old==get(BASE,row['source'])
 if raw!=old:
  for bad in (raw+b' ',old):
   try:apply(row,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('stale/altered accepted')
assert len(rows)==2 and len(runtime)==6
by={r['source']:r for r in runtime};manifest='docs/evidence/region-entry-receiving-816/exact-source-inverses.json';physical=json.loads((R/manifest).read_text());alias=json.loads(by[manifest]['edits'][0]['old']);add='\nmod status_prefix;\npub(super) use status_prefix::Owner as StatusInsertionOwner;\n'
expected=copy.deepcopy(physical);assert expected[-1]['edits'][-1]['after'].endswith(add);expected[-1]['edits'][-1]['after']=expected[-1]['edits'][-1]['after'][:-len(add)];assert alias==expected
# Every prior row/edit/field is retained, with only the separately removed
# registration excluded from its full-after domain. Full physical JSON untouched.
for record in alias:
 p=record['source'];b=apply(by[p],(R/p).read_bytes()).decode()
 if p.endswith('/directory.rs'):assert b.endswith(add) and b.count(add)==1;b=b[:-len(add)]
 for edit in reversed(record['edits']):assert b.count(edit['after'])==1,p;b=b.replace(edit['after'],edit['before'],1)
 original=(R/'docs/evidence/region-entry-receiving-816'/record['archive']).read_bytes();assert b.encode()==original and sha(original)==record['sha256'],p
status='src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs';old=apply(by[status],(R/status).read_bytes());assert old==get('cf7b8ad90',status) and sha(old)=='914e1a78079f871cb37462414b1991cde6722e78b7af39cb7979e11f16f767e9'
# Two complete original variants and unchanged complete region entry receiver.
root=(R/'src/cleanup_plan/replay/expression_facts/receiving/regions_entry.rs').read_text();assert sha(root.encode())=='f27fb4d28d71bb522bafb208a3da4bd8098ec61b5cca904f33d5d9858738b0e4';helper='helpers::validate_place(function, place, storage, leaves, budget)?';assert root.count(helper)==1;oldroot=root.replace(helper,'validate_place(function, place, storage, leaves)?',1);assert sha(oldroot.encode())=='327237ebc01755438f25ff967fc327a3ffb4f9d6be2a2a54bad6a36803b7095b'
for record,name in zip(json.loads((R/'docs/evidence/region-entry-receiving-816/receiver-projections.json').read_text()),['regions.before.txt','entry.before.txt']):
 original=(R/'docs/evidence/region-entry-receiving-816'/name).read_text();assert sha(original.encode())==record['sha256'] and original==record['before'];assert oldroot.count(record['after'])==1;after=record['after'].replace('let under = validate_place(function, place, storage, leaves)?;','let under = helpers::validate_place(function, place, storage, leaves, budget)?;');assert root.count(after)==1;assert (R/'src/cleanup_plan/replay.rs').read_text().count(original)==1
def replay(script,named,scope):
 old=R/script;source=old.read_text();rb0,rt0,wt0,co0=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output;capture=[]
 def rb(p):return named[p.resolve()] if p.resolve() in named else rb0(p)
 def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*a,**kw)
 def wt(p,s,*a,**kw):assert p.resolve()==(old.parent/'source-proof.json').resolve();capture.append(json.loads(s));return len(s)
 def co(args,*a,**kw):
  if isinstance(args,list) and args[:3]==['git','diff','--name-only'] and '--' in args:
   i=args.index('--');revs=[x for x in args[3:i] if not x.startswith('-')]
   if len(revs)==1:args=args[:i]+[scope]+args[i:]
  return co0(args,*a,**kw)
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
  with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(source,str(old),'exec'),{'__file__':str(old),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb0,rt0,wt0,co0
 return capture[-1] if capture else json.loads(out.getvalue())

named={(R/r['source']).resolve():r['edits'][0]['old'].encode() for r in rows}
prior=replay('docs/evidence/setup825-reference-controls-assembly-20261015/verify.py',named,BASE)
assert all(x.startswith('A\t') for x in subprocess.check_output(['git','diff','--name-status',BASE,'--','docs/evidence'],text=True).splitlines())
changed=subprocess.check_output(['git','diff','--name-only',BASE,'--','src','tests','crates'],text=True).splitlines();assert sorted(changed)==sorted(r['source'] for r in rows)
selection=json.loads((D/'pending-selection.json').read_text());oldsel=json.loads((R/'docs/evidence/setup825-relative-starter-assembly-20261015/pending-selection.json').read_text());assert selection['checks'][:152]==oldsel['checks'];assert selection['total']==178;assert len({(c['package'],c['kind'],c['selector']) for c in selection['checks']})==178
proof={'status':'SOURCE_ONLY_NO_RUST_GATES','source_basis':BASE,'full_source_successor_rows':2,'full_runtime_binding_rows':6,'all_four_complete_originals_and_two_variants_restored':True,'original_status_literal_preserved':True,'physical_816_JSON_unchanged':True,'all_prior_evidence_unchanged':True,'prior_corrected825_full_source_proof':prior,'stale_altered_refusals':negative,'future_selection':178,'new_test_bodies':0,'production_changes':0,'runtime_bodies_executed':0}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if k!='prior_corrected825_full_source_proof'}))
