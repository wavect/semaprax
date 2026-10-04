#!/usr/bin/env python3
"""Record a pinned parser's exact checked-u32 non-admission without relabelling it.

The two fixed sources are intentionally not run as a task result.  A matched
LAW-16 checked-u32 cell needs one language to accept its success program and
reject its overflow program under the same domain; this records why the
reviewed SEMAPRAX parser cannot enter that gate at all.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import time

ROOT = pathlib.Path(__file__).parent
SUCCESS = ROOT / "fixtures/semaprax-checked-u32-success-v1.spx"
OVERFLOW = ROOT / "fixtures/semaprax-checked-u32-overflow-v1.spx"
SCHEMA = "semaprax.bend2-law-benchmark.checked-u32-nonadmission.v1"
PINNED_SEMAPRAX_COMMIT = "9a9db7a8117ac8d292b24ffd5671ec3333272290"
PINNED_SEMAPRAX_SHA256 = "sha256:cc9dd3ca99a74dd973cbfb904621e27b801d8ddea6065873d24c14de9dee1d89"


def digest(data): return "sha256:" + hashlib.sha256(data).hexdigest()
def reference(path, root=None):
    data = path.read_bytes()
    return {"path": str(path.relative_to(root)) if root else str(path.resolve()), "bytes": len(data), "sha256": digest(data)}

def invoke(executable, source, raw_root, label, timeout):
    argv=[str(executable),"check",str(source.resolve()),"--json"]
    started=time.monotonic_ns()
    try:
        completed=subprocess.run(argv,capture_output=True,timeout=timeout)
        code,timed_out,stdout,stderr=completed.returncode,False,completed.stdout,completed.stderr
    except subprocess.TimeoutExpired as error:
        code,timed_out,stdout,stderr=None,True,error.stdout or b"",error.stderr or b""
    stdout_path,stderr_path=raw_root/f"{label}.stdout",raw_root/f"{label}.stderr"
    stdout_path.write_bytes(stdout); stderr_path.write_bytes(stderr)
    return {"argv":argv,"command_sha256":digest(json.dumps(argv,separators=(",",":")).encode()),"elapsed_ns":time.monotonic_ns()-started,"exit_code":code,"timed_out":timed_out,"stdout":reference(stdout_path,raw_root),"stderr":reference(stderr_path,raw_root)}

def expected_parser_nonadmission(row, raw_root):
    if row["exit_code"] in (0,None) or row["timed_out"]: return False
    diagnostics=[]
    for stream in ("stdout", "stderr"):
        try: diagnostics.append(json.loads((raw_root/row[stream]["path"]).read_text()))
        except (OSError, json.JSONDecodeError): pass
    return any(diagnostic.get("code")=="SPX-P003" and diagnostic.get("message")=="integer literals accept only an `i32`, `u8`, or `usize` suffix" for diagnostic in diagnostics if isinstance(diagnostic, dict))

def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semaprax",required=True,type=pathlib.Path)
    parser.add_argument("--raw-artifact-dir",required=True,type=pathlib.Path)
    parser.add_argument("--output",required=True,type=pathlib.Path)
    parser.add_argument("--timeout-seconds",type=float,default=15)
    args=parser.parse_args(argv)
    if args.output.exists() or args.raw_artifact_dir.exists(): parser.error("--output and --raw-artifact-dir must be new")
    if not args.semaprax.is_file(): parser.error("--semaprax must be a regular file")
    executable=reference(args.semaprax)
    if executable["sha256"] != PINNED_SEMAPRAX_SHA256: parser.error("--semaprax digest does not match the reviewed pinned executable")
    args.raw_artifact_dir.mkdir(parents=True)
    rows={"success_source":invoke(args.semaprax,SUCCESS,args.raw_artifact_dir,"success",args.timeout_seconds),"overflow_source":invoke(args.semaprax,OVERFLOW,args.raw_artifact_dir,"overflow",args.timeout_seconds)}
    observed=all(expected_parser_nonadmission(row,args.raw_artifact_dir) for row in rows.values())
    result={"schema":SCHEMA,"status":"unsupported_by_pinned_parser" if observed else "unexpected_observation","numeric_domain":"u32 checked","semaprax":{"commit":PINNED_SEMAPRAX_COMMIT,"executable":executable},"sources":{"success":reference(SUCCESS),"overflow":reference(OVERFLOW)},"checks":rows,"required_diagnostic":{"code":"SPX-P003","message":"integer literals accept only an `i32`, `u8`, or `usize` suffix"},"raw_artifact_dir":str(args.raw_artifact_dir.resolve()),"consequence":"the reviewed SEMAPRAX route cannot enter a matched checked-u32 success-plus-overflow gate; do not substitute i32, i64, u8, or usize","nonclaims":["not a matched Bend/SEMAPRAX result","not an overflow runtime observation","not an SMT or Lean checked-u32 proof result","not a task acceptance or timing result"]}
    args.output.write_text(json.dumps(result,indent=2,sort_keys=True)+"\n")
    return 0 if observed else 1
if __name__=="__main__": raise SystemExit(main())
