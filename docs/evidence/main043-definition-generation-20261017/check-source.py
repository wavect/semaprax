"""Pure fixed definition, original loop and finite source controls; no refresh."""
from pathlib import Path
import ast,copy,hashlib,importlib.util,json,os,runpy,shutil,tempfile
D=Path(__file__).resolve().parent;R=D.parents[2];G=R/'docs/evidence/main043-independent-custody-20261016';sha=lambda raw:hashlib.sha256(raw).hexdigest()
original=(D/'verify-or-refresh.before.py').read_bytes();public=(G/'verify-or-refresh.py').read_text();before=json.loads((D/'definition-bindings.before.json').read_bytes());actual=(G/'definition-bindings.json').read_bytes();assert actual==(D/'definition-bindings.before.json').read_bytes()
seal=json.loads((D/'definition-seal.json').read_bytes());row=next(x for x in before['definitions'] if x['path']==seal['path']);assert row==seal['old_row']
oldrecipe=json.loads((D/'guard-extension-recipe-edits.before.json').read_bytes());recipe=json.loads((G/'guard-extension-recipe-edits.json').read_bytes());assert recipe[:-1]==oldrecipe and recipe[-1]=={'before':original.decode(),'after':public}
expected=runpy.run_path(str(G/'guard-extensions.py'));expected['verify_recipe']()
# Run only the bootstrap. Its final complete verifier exec is NOT executed.
last="exec(compile(current,str(P),'exec'),{'__name__':'__main__','__file__':str(P),'_definition_payload':m.definition_payload})";assert public.count(last)==1
bootstrap=public.replace(last,'',1);namespace={'__file__':str(G/'verify-or-refresh.py'),'__name__':'source_checks_only'};exec(compile(bootstrap,'<fixed-bootstrap-only>','exec'),namespace)
helper=namespace['m'];rendered=namespace['current'];assert rendered.replace(" payload=_definition_payload(R,row,read)"," payload=read(row['path'])",1).encode()==original
old=(D/'semantic_receivers.c874.rs.txt').read_text();assert helper.definition_payload(R,row,lambda _: (_ for _ in ()).throw(AssertionError('unexpected oldread')))==old
# Actual original definition-loop statements remain unchanged after payload acquisition.
node=next(x for x in ast.parse(rendered).body if isinstance(x,ast.For) and isinstance(x.target,ast.Name) and x.target.id=='row' and ast.unparse(x.iter)=="bindings['definitions']")
ns={'bindings':{'definitions':[copy.deepcopy(row)]},'read':lambda _: (_ for _ in ()).throw(AssertionError('unexpected read')),'R':R,'sha':lambda s:sha(s.encode()),'a':type('Args',(),{'write_endpoints':True})(),'_definition_payload':helper.definition_payload}
exec(compile(ast.Module(body=[node],type_ignores=[]),'<unchanged-definition-loop>','exec'),ns)
negative=0
for mutation in [dict(row,bytes=row['bytes']+1),dict(row,sha256='0'*64),dict(row,extra=True)]:
 try:helper.definition_payload(R,mutation,lambda _: 'notcalled')
 except AssertionError:negative+=1
 else:raise AssertionError('oldrow retarget admitted')
calls=[];unknown={'path':'unchanged-other-definition'};assert helper.definition_payload(R,unknown,lambda path:calls.append(path) or 'wholeoldread')=='wholeoldread';assert calls==[unknown['path']]
with tempfile.TemporaryDirectory(prefix='main043-definition-controls-',dir=os.environ['TMPDIR']) as temporary:
 root=Path(temporary);paths=[seal['path'],seal['current_archive'],seal['old_archive']]+[x['path'] for x in seal['raw_inventories']]+[str((D/'definition-seal.json').relative_to(R))]
 for path in paths:
  target=root/path;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/path,target)
 for path in paths:
  target=root/path;keep=target.read_bytes()
  for bad in [keep+b' ',keep[:-1],b'foreign']:
   target.write_bytes(bad)
   try:helper.definition_payload(root,row,lambda _: 'notcalled')
   except (AssertionError,ValueError,UnicodeError):negative+=1
   else:raise AssertionError(('foreign source admitted',path))
  target.write_bytes(keep)
 # Nofollow at the actual definition acquisition, not a precheck-only symlink test.
 target=root/seal['path'];keep=target.read_bytes();target.unlink();target.symlink_to(root/seal['current_archive'])
 try:helper.definition_payload(root,row,lambda _: 'notcalled')
 except OSError:negative+=1
 else:raise AssertionError('definition symlink admitted')
 target.unlink();target.write_bytes(keep)
 # Every new trusted helper is authenticated before import; modified code must
 # fail as an assertion, never execute its sentinel RuntimeError.
 fake=root/'docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py';fake.parent.mkdir(parents=True,exist_ok=True)
 for path in ['verify-or-refresh.before.py','definition_payload.py']:
  dest=root/D.relative_to(R)/path;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(D/path,dest)
  keep=dest.read_bytes();dest.write_bytes(keep+b'\nraise RuntimeError("MUST_NOT_EXECUTE")\n')
  try:exec(compile(bootstrap,'<mutated-bootstrap>','exec'),{'__file__':str(fake),'__name__':'source_checks_only'})
  except AssertionError:negative+=1
  else:raise AssertionError('mutated trusted source imported')
  dest.write_bytes(keep)
# Closed newmain binder branch preserves all original non-wrapper statements.
binder=R/'docs/evidence/main043-newmain-acquisition-20261017/bind-current.py';ast.parse(binder.read_bytes());ast.parse((D/'bind-current.before.py').read_bytes())
assert sha((G/'verify-or-refresh.py').read_bytes()) in binder.read_text() and sha((G/'guard-extension-recipe-edits.json').read_bytes()) in binder.read_text()
print(json.dumps({'status':'PURE_SOURCE_PASS_NO_REFRESH_OR_RUST','whole_old_verifier_retained':True,'old_definition_row_and_table_unchanged':True,'original_definition_loop_passes_on_restored_full6160bytes':True,'actual_current7516_separately_authenticated':True,'three_actual_raw_scopes_authenticated':True,'guard_recipe_full_reconstruction':True,'foreign_controls':negative,'common_refresh_executed':False}))
