from pathlib import Path
import hashlib,json,runpy,subprocess,tempfile,shutil
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest()
m=runpy.run_path(str(D/'acquire.py'));rows=json.loads((D/'sources.json').read_bytes());original_packet=R/'docs/evidence/selected-successor-generation-20261017/full-source-inverses.json';assert sha(original_packet.read_bytes())=='947c41698347ca27788aae5985e63f19c85d3f7eb3f25fee4374dac9d0d743d1'
oldrows=json.loads(original_packet.read_bytes());refusals=0
def refuse(fn):
 global refusals
 try:fn()
 except (AssertionError,OSError,ValueError,UnicodeError):refusals+=1;return
 raise AssertionError('foreign source admitted')
for row,old in zip(rows,oldrows):
 raw=(R/row['source']).read_bytes();stage=(R/row['stage_archive']).read_bytes();assert len(stage)==old['current_length'] and sha(stage)==old['current_sha256'];assert m['acquire'](R,row['source'],raw)==stage;assert m['acquire'](R,row['source'],stage)==stage
 for bad in [b'',b'foreign',raw[:-1],raw+b' ',stage+stage]:refuse(lambda bad=bad:m['acquire'](R,row['source'],bad))
 assert stage==subprocess.check_output(['git','show','7827d3687da21e1093d40f8c8e3f7116edba1993:'+row['source']],cwd=R)
assert m['acquire'](R,'foreign',b'unchanged')==b'unchanged'
with tempfile.TemporaryDirectory(prefix='canonical-import-',dir='/Volumes/Semaprax/runtime-tmp')as tmp:
 root=Path(tmp);paths=[str((D/'sources.json').relative_to(R))]+[v for row in rows for v in [row['source'],row['current_archive'],row['stage_archive']]]
 for p in paths:(root/p).parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/p,root/p)
 for p in paths:
  f=root/p;raw=f.read_bytes()
  for bad in [raw[:-1],raw+b' ',b'foreign']:
   f.write_bytes(bad);refuse(lambda:m['acquire'](root,rows[0]['source'],(R/rows[0]['source']).read_bytes()) if p in [rows[0]['source'],rows[0]['current_archive'],rows[0]['stage_archive'],paths[0]]else m['acquire'](root,rows[1]['source'],(R/rows[1]['source']).read_bytes()))
  f.write_bytes(raw);f.unlink();f.symlink_to(R/p);refuse(lambda:m['acquire'](root,rows[0]['source'],(R/rows[0]['source']).read_bytes())if p in [rows[0]['source'],rows[0]['current_archive'],rows[0]['stage_archive'],paths[0]]else m['acquire'](root,rows[1]['source'],(R/rows[1]['source']).read_bytes()));f.unlink();f.write_bytes(raw)
 for row in rows:
  p=root/row['source'];raw=p.read_bytes();p.write_bytes((R/row['stage_archive']).read_bytes());refuse(lambda:m['acquire'](root,row['source'],p.read_bytes()));p.write_bytes(raw)
# Real importable public binder + whole previous render and complete root inverse.
public=R/'docs/evidence/selected-successor-generation-20261017/bind-selected-endpoints.py';bind=runpy.run_path(str(public),run_name='source_controls_no_main');assert bind['selected_previous'](R,(R/rows[0]['source']).read_bytes())==(R/oldrows[0]['archive']).read_bytes();bind['render']()
# Full original Selected source proof with only its two physical input sites staged.
# All assertions remain byte exact; old helper/API main guards stay inactive.
oldcheck=(R/'docs/evidence/selected-successor-generation-20261017/check-source.py').read_text();edits=[('old=(R/row[\'archive\']).read_bytes();raw=(R/row[\'path\']).read_bytes()',"old=(R/row['archive']).read_bytes();raw=_acquire(R,row['path'],(R/row['path']).read_bytes())"),("root=(R/rows[0]['path']).read_text();prior=", "root=_acquire(R,rows[0]['path'],(R/rows[0]['path']).read_bytes()).decode();prior="),("new=(R/rows[1]['path']).read_text();old=", "new=_acquire(R,rows[1]['path'],(R/rows[1]['path']).read_bytes()).decode();old=")];value=oldcheck
for old,new in edits:assert value.count(old)==1;value=value.replace(old,new,1)
restored=value
for old,new in reversed(edits):assert restored.count(new)==1;restored=restored.replace(new,old,1)
assert restored==oldcheck
exec(compile(value,'fixed_selected_source_controls','exec'),{'__file__':str(R/'docs/evidence/selected-successor-generation-20261017/check-source.py'),'_acquire':m['acquire']})
# Every prior main67107 helper byte/assertion is recoverable.
p=R/'src/source_reflows_merge/main67107_inputs.rs';old=(D/'before/src/source_reflows_merge/main67107_inputs.rs').read_text();new=p.read_text();appended='\n\n// Import-only compile generation'+new.split('\n\n// Import-only compile generation',1)[1];new=new.removesuffix(appended)
a='''        let raw_compiled = compiled;
        let compiled_stage = canonical_import_input(path, compiled);
        let compiled = compiled_stage.as_str();
        let physical = compiled_stage.clone();
        assert_eq!(physical.len(), length);
        assert_eq!(digest(&physical), row["current_sha256"]);''';b='        let physical = factored_core_file(&root.join(path), length, row["current_sha256"].as_str().unwrap()).unwrap();';assert new.count(a)==1;new=new.replace(a,b,1);a='            let current = if current == raw_compiled { physical.as_str() } else { current };\n';assert new.count(a)==1;new=new.replace(a,'',1);assert new==old
# Mutated helper bytes must refuse before importing any attacker definition.
with tempfile.TemporaryDirectory(prefix='canonical-bootstrap-',dir='/Volumes/Semaprax/runtime-tmp') as tmp:
    root=Path(tmp);packet=root/D.relative_to(R);packet.mkdir(parents=True)
    (packet/'acquire.py').write_bytes((D/'acquire.py').read_bytes()+b'\nraise RuntimeError("foreign helper executed")\n')
    public_path=root/public.relative_to(R);public_path.parent.mkdir(parents=True)
    prefix=public.read_text().split("exec(compile(_text,str(P),'exec'),globals())",1)[0]
    refuse(lambda:exec(compile(prefix,str(public_path),'exec'),{'__file__':str(public_path)}))
result={'status':'SOURCE_ONLY_NOT_RUST_OR_BINDER_MAIN_QUALIFIED','whole_old_selected_packet_retained':True,'whole_original_selected_source_assertions_executed':True,'two_subject_stage_channels':4,'refusals':refusals,'main67107_full_previous_recovered':True,'main67107_lines':len(p.read_text().splitlines())};(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
