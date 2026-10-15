#!/usr/bin/env python3
"""Static visitor correction/full-source successor proof. No compiler execution."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;h=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-677.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text());aliases={}
for row in rows:
 p=R/row['source'];b=p.read_bytes();assert h(b)==row['current_sha256'];text=b.decode()
 for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 assert h(text.encode())==row['original_sha256'];assert text.encode()==(R/row['archive']).read_bytes()==subprocess.check_output(['git','show',row['checkpoint_revision']+':'+row['source']],cwd=R);aliases[p.resolve()]=text.encode()
p=R/'src/vec_ops/field_update/tests.rs';old=aliases[p.resolve()].decode();before='''        for index in 0..expression.child_count() {
            pending.push(expression.child(index).unwrap());
        }''';after='''        pending.extend(crate::hir::lexical_exit::ChildCursor::new(expression));''';assert old.count(before)==1;assert p.read_text()==old.replace(before,after,1)
walk=(R/'src/hir/lexical_exit/walk.rs').read_text();assert "impl<'a> Iterator for ChildCursor<'a>" in walk and "type Item = &'a ResolvedExpr;" in walk
# Replay the complete unchanged677 joined proof in exactly its two restored inputs.
prior=R/'docs/evidence/ready-main-844-846-836-assembly-20261015/verify.py';code=prior.read_text();rb,rt=Path.read_bytes,Path.read_text
try:
 Path.read_bytes=lambda p,*a,**k:aliases[p.resolve()] if p.resolve() in aliases else rb(p,*a,**k)
 Path.read_text=lambda p,*a,**k:aliases[p.resolve()].decode() if p.resolve() in aliases else rt(p,*a,**k)
 with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(code,str(prior),'exec'),{'__file__':str(prior),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text=rb,rt
joined=json.loads(out.getvalue());assert joined['old_836_full_input_checks']==25 and joined['old_packet_files_byte_exact']==222
print(json.dumps({'status':'SOURCE_ONLY_NO_RUST','full_source_inverses':2,'test_body_change':'only nonexistent Expr child API replaced by owning ChildCursor Iterator','all_positive_negative_and_exact_runtime_child_assertions_unchanged':True,'new_selectors':0,'production_changed':False,'whole_previous_joined_source_proof':joined},indent=2))
