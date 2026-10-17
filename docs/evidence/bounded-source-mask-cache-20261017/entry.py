#!/opt/homebrew/bin/python3
"""Optional NEXT source-tool entry. Original script bytes/reads/predicates unchanged."""
from pathlib import Path
import argparse,hashlib,importlib.util,json,runpy,subprocess,sys,types,marshal
from memoizer import MaskMemoizer

HERE=Path(__file__).resolve().parent
BIND=json.loads((HERE/"bindings.json").read_text())
PARSER="docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py"
GENERATOR="docs/evidence/a3-e47-final-custody-20261016/generate-authority.py"
BOUNDARY="docs/evidence/source-boundary-domains-20261016/boundary_tools.py"
ENTRIES={"authority-generate":GENERATOR,"authority-verify":"docs/evidence/a3-e47-final-custody-20261016/verify-authority.py","boundary-generate":"docs/evidence/source-boundary-domains-20261016/generate-boundary.py","boundary-verify":"docs/evidence/source-boundary-domains-20261016/verify-boundary.py","preservation-verify":"docs/evidence/source-boundary-domains-20261016/verify-preservation.py"}
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--repo",type=Path,required=True);p.add_argument("--head",required=True)
    p.add_argument("--entry",choices=ENTRIES,required=True);p.add_argument("--stats",type=Path,required=True)
    p.add_argument("--cache-bytes",type=int,default=4*1024*1024);p.add_argument("script_args",nargs=argparse.REMAINDER)
    a=p.parse_args();repo=a.repo.resolve();stats=a.stats.resolve()
    assert repo.is_relative_to("/Volumes/Semaprax") and stats.is_relative_to("/Volumes/Semaprax") and not stats.is_relative_to(repo) and not stats.exists()
    assert subprocess.check_output(["git","rev-parse","HEAD"],cwd=repo,text=True).strip()==a.head
    assert not subprocess.check_output(["git","status","--porcelain=v1","--untracked-files=all"],cwd=repo,text=True).strip()
    def check_scripts():
        for row in BIND["scripts"]:assert hashlib.sha256((repo/row["path"]).read_bytes()).hexdigest()==row["sha256"],row["path"]
    check_scripts();sys.dont_write_bytecode=True
    parser_text=(repo/PARSER).read_text();parser_code=compile(parser_text,str(repo/PARSER),"exec")
    generator_text=(repo/GENERATOR).read_text();anchor="for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]:"
    generator_prefix=generator_text.split(anchor,1)[0];assert len(generator_prefix)<len(generator_text)
    generator_code=compile(generator_prefix,str(repo/GENERATOR),"exec")
    parser_identity=marshal.dumps(parser_code);generator_identity=marshal.dumps(generator_code)
    cache=None;parser_hits=0;prefix_hits=0
    real_exec=exec
    def scoped_exec(code,globals=None,locals=None,*,closure=None):
        nonlocal cache,parser_hits,prefix_hits
        # Every original targeted call supplies an explicit namespace. Generic
        # omitted-namespace exec is deliberately outside this optional adapter.
        if globals is None:raise ValueError("unreviewed implicit exec context")
        exact_parser=isinstance(code,types.CodeType) and marshal.dumps(code)==parser_identity
        exact_generator=isinstance(code,types.CodeType) and marshal.dumps(code)==generator_identity
        if exact_generator:globals["exec"]=scoped_exec;prefix_hits+=1
        result=real_exec(code,globals,locals,closure=closure)
        if exact_parser:
            parser_hits+=1
            if cache is None:cache=MaskMemoizer(globals["mask"],a.cache_bytes)
            globals["mask"]=cache
        return result
    args=a.script_args[1:] if a.script_args[:1]==["--"] else a.script_args
    assert "--repo" in args and Path(args[args.index("--repo")+1]).resolve()==repo
    for flag in ["--out","--full-dir","--scope-dir"]:
        if flag in args:assert Path(args[args.index(flag)+1]).resolve().is_relative_to("/Volumes/Semaprax")
    saved_argv=sys.argv[:];saved_path=sys.path[:];saved_module=sys.modules.get("boundary_tools")
    module=None;previous_exec=None;had_exec=False;completed=False
    try:
        sys.path.insert(0,str((repo/BOUNDARY).parent))
        if a.entry.startswith("boundary-") or a.entry=="preservation-verify":
            spec=importlib.util.spec_from_file_location("boundary_tools",repo/BOUNDARY)
            module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
            had_exec="exec" in module.__dict__;previous_exec=module.__dict__.get("exec")
            module.__dict__["exec"]=scoped_exec;sys.modules["boundary_tools"]=module
        sys.argv=[str(repo/ENTRIES[a.entry]),*args]
        runpy.run_path(str(repo/ENTRIES[a.entry]),run_name="__main__",init_globals={"exec":scoped_exec})
        completed=True
        if a.entry in {"authority-generate","boundary-generate","boundary-verify"}:
            assert cache is not None and parser_hits>0,"reviewed parser hook not reached"
    finally:
        sys.argv=saved_argv;sys.path[:]=saved_path
        if module is not None:
            if had_exec:module.__dict__["exec"]=previous_exec
            else:module.__dict__.pop("exec",None)
        if saved_module is None:sys.modules.pop("boundary_tools",None)
        else:sys.modules["boundary_tools"]=saved_module
        check_scripts()
        report=dict(head=a.head,entry=a.entry,original_scripts_unchanged=True,parser_exec_hooks=parser_hits,
          generator_prefix_hooks=prefix_hits,mask_cache=cache.statistics() if cache else None,
          full_manifest_parity_qualified=False,workingtree_read_caching=False,original_entry_completed=completed)
        with stats.open("x") as stream:json.dump(report,stream,indent=2);stream.write("\n")
if __name__=="__main__":main()
