#!/usr/bin/env python3
"""Validate the bounded checked-u32 non-admission capsule without a compiler."""
import argparse
import hashlib
import importlib.util
import json
import pathlib

ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location("law16_checked_u32_nonadmission",ROOT/"law16_checked_u32_nonadmission.py")
NONADMISSION=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(NONADMISSION)
SCHEMA=NONADMISSION.SCHEMA
RESULT_SCHEMA="semaprax.bend2-law-benchmark.checked-u32-nonadmission-review.v1"

def digest(data): return "sha256:"+hashlib.sha256(data).hexdigest()
def exact(root, ref):
    if not isinstance(ref,dict) or set(ref)!={"path","bytes","sha256"}: raise ValueError("malformed raw reference")
    path=root/ref["path"]
    if not path.is_file() or path.stat().st_size!=ref["bytes"] or digest(path.read_bytes())!=ref["sha256"]: raise ValueError("raw receipt drifted")
    return path
def review(root):
    result=json.loads((root/"result.json").read_text())
    if result.get("schema")!=SCHEMA or result.get("status")!="unsupported_by_pinned_parser" or result.get("numeric_domain")!="u32 checked": raise ValueError("unexpected non-admission result")
    executable=result.get("semaprax",{}).get("executable",{})
    if result.get("semaprax",{}).get("commit")!=NONADMISSION.PINNED_SEMAPRAX_COMMIT or executable.get("sha256")!=NONADMISSION.PINNED_SEMAPRAX_SHA256: raise ValueError("pinned executable identity drifted")
    expected={"success_source":NONADMISSION.SUCCESS,"overflow_source":NONADMISSION.OVERFLOW}
    for label,fixture in expected.items():
        source=result["sources"].get("success" if label=="success_source" else "overflow")
        if source.get("bytes")!=fixture.stat().st_size or source.get("sha256")!=digest(fixture.read_bytes()): raise ValueError("fixed probe source drifted")
        row=result["checks"].get(label)
        if not isinstance(row,dict) or not NONADMISSION.expected_parser_nonadmission(row,root/"raw"): raise ValueError("parser did not retain exact non-admission diagnostic")
        exact(root/"raw",row["stdout"]); exact(root/"raw",row["stderr"])
    return {"schema":RESULT_SCHEMA,"status":"authenticated_unsupported_by_pinned_parser","numeric_domain":"u32 checked","checked_sources":2,"required_diagnostic":result["required_diagnostic"],"consequence":result["consequence"],"nonclaims":result["nonclaims"]}
def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument("--capsule",required=True,type=pathlib.Path);parser.add_argument("--output",required=True,type=pathlib.Path);args=parser.parse_args(argv)
    if args.output.exists() or not args.output.parent.is_dir(): parser.error("output must be new")
    try: value=review(args.capsule)
    except (OSError,ValueError,json.JSONDecodeError) as error: parser.error(str(error))
    args.output.write_text(json.dumps(value,indent=2,sort_keys=True)+"\n")
if __name__=="__main__":main()
