#!/opt/homebrew/bin/python3
"""Parser-only controls; authority files are never rewritten in the checkout."""
import argparse, collections, contextlib, hashlib, importlib.util, io, json, re, subprocess, tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3]
D=R/"docs/evidence/main043-independent-custody-20261016"
p=argparse.ArgumentParser();p.add_argument("--output",required=True);p.add_argument("--allowed-ref");args=p.parse_args()
out=Path(args.output).resolve();assert out.is_relative_to("/Volumes/Semaprax");out.mkdir(parents=True,exist_ok=False)
script=D/"guard-extensions.py";spec=importlib.util.spec_from_file_location("guard847",script);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
old_script=(Path(__file__).parent/"before/guard-extensions.py").read_bytes()
assert script.read_bytes().split(b"def verify_guard(",1)[1]==old_script.split(b"def verify_guard(",1)[1]
old_scope={"__file__":str(script),"__name__":"old_guard"};exec(compile(old_script,str(script),"exec"),old_scope)
def retained(original,current):
    edits=m.fragments("fixture",current,original);restored=current
    for edit in edits:assert restored.count(edit)==1;restored=restored.replace(edit,"",1)
    assert restored==original
    return edits
original='const S = concat!(include_str!("old/a.rs"), include_str!("old/b.rs"),include_str!("old/c.rs"),);\n'
current=original.replace('concat!(', 'concat!(include_str!("new/x.rs"),').replace('include_str!("old/b.rs"),','include_str!("new/y.rs"),include_str!("old/b.rs"),').replace('include_str!("old/c.rs"),','include_str!("old/c.rs"),include_str!("new/z.rs"),')
assert len(retained(original,current))==3
positives=1
for ending in ["\n","\r\n"]:
    before='// α exact whitespace'+ending+'const S=concat!('+ending+'  include_str!("old/a.rs"),'+ending+');'+ending
    for gap in ["", " ", "\t", ending+"    "]:
        after=before.replace('include_str!("old/a.rs"),',gap+'include_str!("new/unique.rs"),'+gap+'include_str!("old/a.rs"),')
        retained(before,after);positives+=1
negative_cases={
 "replacement":original.replace("old/a.rs","changed/a.rs"),
 "deletion":original.replace('include_str!("old/b.rs"),',""),
 "reorder":original.replace('include_str!("old/a.rs"),','TMP').replace('include_str!("old/b.rs"),','include_str!("old/a.rs"),').replace('TMP','include_str!("old/b.rs"),'),
 "plain-whitespace-insert":original.replace("const S", "const  S"),
 "plain-whitespace-delete":original.replace("const S", "constS"),
 "plain-whitespace-replace":original.replace("const S", "const\tS"),
 "crlf-rewrite":original.replace("\n","\r\n"),
 "old-macro-spacing":original.replace('include_str!("old/a.rs")','include_str! ("old/a.rs")'),
 "non-macro-addition":current+"foreign();",
 "partial-macro":original.replace('concat!(', 'concat!(include_str!("new/partial.rs")'),
 "wrong-separator":original.replace('concat!(', 'concat!(include_str!("new/partial.rs");'),
 "duplicate-fragment":original.replace('concat!(', 'concat!(include_str!("new/duplicate.rs"),').replace('include_str!("old/c.rs"),','include_str!("old/c.rs"),include_str!("new/duplicate.rs"),'),
 "unicode-change":("// α\n"+current).replace("α","β"),
}
for name,wrong in negative_cases.items():
    expected="// α\n"+original if name=="unicode-change" else original
    try:m.fragments(name,wrong,expected)
    except AssertionError:pass
    else:raise AssertionError("hostile parser edit accepted: "+name)
# Keep the original complete controls unmodified. Only their generated binding
# outputs are directed into an isolated test fixture, never common authority.
with tempfile.TemporaryDirectory(dir=out,prefix="isolated-binding-fixture-") as temp:
    fixture=Path(temp)
    for name in ["guard-extension-allowed.json","guard-extension-original/verify-or-refresh.py","guard-extension-recipe-edits.json","verify-or-refresh.py"]:
        data=subprocess.check_output(["git","show",args.allowed_ref+":docs/evidence/main043-independent-custody-20261016/"+name],cwd=R) if args.allowed_ref else (D/name).read_bytes()
        target=fixture/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
    m.D=fixture;allowed=json.loads((fixture/"guard-extension-allowed.json").read_bytes());raw_proofs=[];old_refusals=0;actual_hostile=0;maximum_refusals=0
    for path in allowed:
        raw=(R/path).read_bytes().decode();old=subprocess.check_output(["git","show","a82e00cc8:"+path],cwd=R).decode()
        try:old_scope["fragments"](path,raw,old)
        except AssertionError:old_refusals+=1
        else:raise AssertionError("expected original line-hunk parser failure absent")
        record=m.verify_guard(path,raw,old,refresh=True)
        original_macros=re.findall(r'include_str!\("[^"\n]+"\),',old)
        first,second=original_macros[:2]
        for wrong in ["!"+raw[1:],raw.replace(" ","\t",1),raw.replace("\n","\r\n",1),raw.replace(first,"",1),raw.replace(first,first.replace("include_str!", "include_str !"),1),raw.replace(first,"SWAP-ORIGINAL",1).replace(second,first,1).replace("SWAP-ORIGINAL",second,1)]:
            try:m.fragments(path,wrong,old)
            except AssertionError:actual_hostile+=1
            else:raise AssertionError("actual original guard replacement/deletion/reorder/whitespace accepted")
        tokens=[token for e in record["added_fragments"] for token in re.findall(r'include_str!\("([^"\n]+)"\)',e)]
        for token in tokens:
            base=(R/path).parent if path=="tests/hir_module_boundaries.rs" else (R/path).parent.parent
            key=str((base/token).resolve().relative_to(R))
            if allowed[path]["maximum"][key]!=1:continue
            wrong=raw+'\n\t\tinclude_str!("'+token+'"),\n'
            try:m.verify_guard(path,wrong,old)
            except AssertionError as error:
                assert error.args[0][0]=="missing or duplicate added include",error
                maximum_refusals+=1;break
            else:raise AssertionError("maximum count violation accepted")
        else:raise AssertionError("actual max1 include control absent")
        raw_proofs.append(dict(path=path,current_sha256=record["current_sha256"],original_sha256=record["original_sha256"],runs=len(record["added_fragments"]),inserted_includes=len(tokens),whole_restoration=True))
    captured=io.StringIO()
    with contextlib.redirect_stdout(captured):m.controls()
    original_controls=json.loads(captured.getvalue())
receipt=dict(status="PASS_SOURCE_ONLY_INCLUDE_INSERTION_CONTROLS",source_head=subprocess.check_output(["git","rev-parse","HEAD"],cwd=R,text=True).strip(),allowed_ref=args.allowed_ref,positive_cases=positives,hostile_refusals=list(negative_cases),actual_raw_hostile_refusals=actual_hostile,actual_maximum_refusals=maximum_refusals,old_parser_actual_failures=old_refusals,raw_proofs=raw_proofs,unchanged_original_controls=original_controls,authority_writes="isolated temporary unit fixture only; no checkout/common refresh",rust_executed=False)
(out/"receipt.json").write_text(json.dumps(receipt,indent=2)+"\n");print(json.dumps(receipt))
