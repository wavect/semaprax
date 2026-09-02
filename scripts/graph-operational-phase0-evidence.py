#!/usr/bin/env python3
"""Execute the selected graph-operational Phase 0 gates at one exact local HEAD."""
import argparse, hashlib, json, os, platform, re, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
SCHEMA="semaprax.graph-operational-phase0-execution-evidence.v1"
DOMAIN=b"semaprax.graph-operational-phase0-execution-evidence.bundle.v1\0"
HEX=re.compile(r"[0-9a-f]{40}|[0-9a-f]{64}")
MAX_LOG=16*1024*1024; MAX_ARTIFACT=512*1024*1024; TIMEOUT=30*60
POLICY={"schema":"semaprax.workspace-host-policy.v7","candidate_prepare":True,"diagnostics":False,"build_enabled":False,"test_policy":None,"git_commit":None,"frontend_cache":False,"candidate_archives":[],"semantic_cache":False,"semantic_cache_entry":None,"draft_archives":[],"read_batch_workers":None}
COMPONENTS=(
 ("canonical-git","scripts/graph-operational-evidence.py","semaprax.graph-operational-execution-evidence.v1","semaprax.graph-operational-execution-evidence.bundle.v1"),
 ("client-mcp","scripts/graph-operational-client-mcp-evidence.py","semaprax.graph-operational-client-mcp-execution-evidence.v2","semaprax.graph-operational-client-mcp-execution-evidence.bundle.v2"),
 ("vscode-host","scripts/graph-operational-vscode-host-evidence.py","semaprax.graph-operational-vscode-host-execution-evidence.v1","semaprax.graph-operational-vscode-host-execution-evidence.bundle.v1"),)
class Failure(Exception): pass
def canonical(v): return json.dumps(v,sort_keys=True,separators=(",",":"),allow_nan=False).encode()
def sha(b): return "sha256:"+hashlib.sha256(b).hexdigest()
def run(args,timeout=TIMEOUT,**kw):
 try: return subprocess.run(args,cwd=ROOT,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,check=False,timeout=timeout,**kw)
 except subprocess.TimeoutExpired as e: raise Failure(f"command timeout: {args[0]}") from e
def command(args,label,**kw):
 r=run(args,**kw)
 if len(r.stdout)>MAX_LOG: raise Failure(f"{label} output exceeds bound")
 if r.returncode: raise Failure(f"{label} failed ({r.returncode}):\n"+r.stdout[-8192:].decode("utf-8","replace"))
 return r.stdout
def text(args,label): return command(args,label).decode("utf-8","strict").strip()
def git(*args): return text([shutil.which("git"),*args],"Git")
def clean():
 if git("status","--porcelain=v1","--untracked-files=all"): raise Failure("worktree is not clean")
def file_row(path,relative=None):
 p=Path(path).resolve(strict=True)
 if not p.is_file() or p.is_symlink(): raise Failure(f"artifact is not a regular file: {p}")
 body=p.read_bytes()
 if len(body)>MAX_ARTIFACT: raise Failure(f"artifact exceeds bound: {p}")
 return {"path":relative or str(p),"bytes":len(body),"sha256":sha(body)}
def executable(value,label):
 p=Path(value).expanduser()
 if not p.is_absolute() or not os.access(p,os.X_OK): raise Failure(f"{label} must be an absolute executable")
 return str(p.resolve(strict=True))
def repository_inputs(): return [file_row(ROOT/name,name) for name in ("Cargo.toml","Cargo.lock")]
def verify_repository(commit,tree,inputs):
 clean()
 if git("rev-parse","HEAD^{commit}")!=commit or git("rev-parse","HEAD^{tree}")!=tree: raise Failure("repository subject drift")
 for expected in inputs:
  if file_row(ROOT/expected["path"],expected["path"])!=expected: raise Failure(f"repository input drift: {expected['path']}")
def child_bundle(domain,artifacts,raw):
 seed=domain.encode()+b"\0"
 seed+=b"".join(bytes.fromhex(x["sha256"][7:]) for x in artifacts) if raw else b"\0".join(x["sha256"].encode() for x in artifacts)
 return hashlib.sha256(seed).hexdigest()
def validate_outcomes(name,value):
 repo=value.get("repository",{})
 if name=="canonical-git":
  if (repo.get("clean_before"),repo.get("clean_after"),repo.get("head_unchanged"))!=(True,True,True): raise Failure("canonical Git repository state mismatch")
  gates=value.get("gates")
  counts={"selected":3,"passed":3,"failed":0,"ignored":0,"measured":0,"filtered_out":0}
  if not isinstance(gates,list) or len(gates)!=2 or gates[0].get("id")!="graph_operational_git_workflow_v1" or gates[0].get("outcome")!="passed" or gates[0].get("exit_code")!=0 or gates[0].get("counts")!=counts or gates[1].get("id")!="graph_operational_managed_workflow_v1" or gates[1].get("outcome")!="not_selected": raise Failure("canonical Git gate outcomes mismatch")
 elif name=="client-mcp":
  if (repo.get("clean_before"),repo.get("clean_after"),repo.get("head_unchanged"))!=(True,True,True): raise Failure("client/MCP repository state mismatch")
  expected={"generated_clients_ordinary_v1":{"selected":12,"passed":10,"failed":0,"ignored":2,"measured":0,"filtered_out":0},"generated_client_typescript_request_provisioned_v1":{"selected":1,"passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":4},"generated_client_typescript_provisioned_v1":{"selected":1,"passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":3},"workspace_mcp_adapter_v1":{"selected":8,"passed":8,"failed":0,"ignored":0,"measured":0,"filtered_out":0},"workspace_mcp_cli_stdio_v1":{"selected":5,"passed":5,"failed":0,"ignored":0,"measured":0,"filtered_out":0}}
  rows=value.get("executions")
  if not isinstance(rows,list) or {x.get("id") for x in rows}!=set(expected): raise Failure("client/MCP gate inventory mismatch")
  for gate in rows:
   if gate.get("outcome")!="passed" or gate.get("exit_code")!=0 or gate.get("counts")!=expected[gate["id"]]: raise Failure(f"client/MCP gate outcome mismatch: {gate.get('id')}")
  required={"generated_client_python_request_admission":"passed","generated_client_rust_request_admission":"passed","generated_client_typescript_request_admission":"passed_provisioned_local","mcp_adapter_in_process":"passed","mcp_cli_stdio_local_subprocess":"passed"}
  if any(value.get("observations",{}).get(k)!=v for k,v in required.items()): raise Failure("client/MCP observations mismatch")
 else:
  if repo.get("clean_before_and_after") is not True or repo.get("current_head") is not True: raise Failure("VS Code repository state mismatch")
  expected=[{"id":"vscode_node_mock_controllers_v1","passed":50,"failed":0,"ignored":0},{"id":"vscode_extension_host_real_compiler_v1","passed":1,"failed":0,"ignored":0}]
  if value.get("executions")!=expected: raise Failure("VS Code execution outcomes mismatch")
  observation=value.get("observation",{}); required={"schema":"semaprax.vscode-extension-host-result.v1","app_name":"Visual Studio Code","typed_intent":"rename_declaration","verified_virtual_diff":True,"dirty_buffer_invalidated":True,"source_bytes_unchanged":True}
  if any(observation.get(k)!=v for k,v in required.items()): raise Failure("VS Code host observation mismatch")
def validate_component(path,name,schema,domain,commit,tree):
 body=(path/"evidence.json").read_bytes(); value=json.loads(body)
 if canonical(value)!=body or value.get("schema")!=schema: raise Failure(f"invalid {name} envelope")
 repo=value.get("repository",{})
 if repo.get("commit")!=commit or repo.get("tree")!=tree: raise Failure(f"{name} subject mismatch")
 artifacts=value.get("artifacts")
 if not isinstance(artifacts,list) or not artifacts: raise Failure(f"{name} artifact inventory missing")
 paths=[x.get("path") for x in artifacts]
 if any(not isinstance(x,str) or Path(x).name!=x for x in paths) or len(paths)!=len(set(paths)): raise Failure(f"{name} artifact paths mismatch")
 if {x.name for x in path.iterdir()}!={"evidence.json",*paths}: raise Failure(f"{name} bundle inventory mismatch")
 for item in artifacts:
  if file_row(path/item["path"],item["path"])!={k:item[k] for k in ("path","bytes","sha256")}: raise Failure(f"{name} artifact mismatch: {item['path']}")
 if value.get("bundle_id")!=child_bundle(domain,artifacts,name=="vscode-host"): raise Failure(f"{name} bundle ID mismatch")
 validate_outcomes(name,value); return value
def distribution_binding(python):
 script='''import importlib.metadata as m,json\nnames=("mcp","anyio","pydantic","pydantic_core")\nprint(json.dumps({n:m.version(n) for n in names},sort_keys=True,separators=(",",":")))\nd=m.distribution("mcp")\nfor x in d.files or (): print(d.locate_file(x))'''
 lines=text([python,"-I","-c",script],"MCP SDK distribution identity").splitlines(); versions=json.loads(lines[0])
 if versions.get("mcp")!="1.27.0": raise Failure("requires provisioned mcp==1.27.0")
 rows=[]
 for name in lines[1:]:
  p=Path(name)
  if p.is_file() and "__pycache__" not in p.parts and p.suffix!=".pyc":
   item=file_row(p); item["path"]=name; rows.append(item)
 rows.sort(key=lambda x:x["path"])
 if not rows: raise Failure("MCP SDK distribution inventory is empty")
 return {"distribution":"mcp","version":"1.27.0","runtime_distributions":versions,"distribution_files":len(rows),"distribution_payload_sha256":sha(canonical(rows))}
def tool_binding(path,args,label):
 row=file_row(path); row["version"]=text([path,*args],f"{label} version"); return row
def validate_sdk(value):
 if set(value)!={"schema","sdk","protocol","catalogue","workspace","candidate","notification_probe","ordinary_discard","source"}: raise Failure("MCP SDK observation fields mismatch")
 if value["schema"]!="semaprax.python-mcp-sdk-interoperability-observation.v1" or value["sdk"]!={"distribution":"mcp","version":"1.27.0"}: raise Failure("MCP SDK identity mismatch")
 if value["protocol"]!={"requested":"2025-11-25","negotiated":"2025-11-25","server_name":"semaprax","server_version":"0.2.0","tools_capability":True}: raise Failure("MCP SDK negotiation mismatch")
 cat=value["catalogue"]
 if cat.get("required_present")!=["workspace__open","candidate__open","candidate__query","candidate__discard"] or cat.get("forbidden_absent")!=["candidate__build","candidate__test","candidate__commit","candidate__commit-report"] or cat.get("terminal_cursor") is not True or not 1<=cat.get("pages",0)<=64 or not 4<=cat.get("tools",0)<=512: raise Failure("MCP SDK catalogue mismatch")
 if value["workspace"].get("state")!="open" or value["candidate"].get("source_authority") is not False or value["candidate"].get("tests")!="not_run": raise Failure("MCP SDK workspace/candidate mismatch")
 if value["notification_probe"]!={"method":"tools/call","tool":"candidate__discard","subsequent_query":"passed_same_candidate"} or value["ordinary_discard"]!={"discarded":True,"post_discard_query_error_code":-32000}: raise Failure("MCP SDK notification/discard mismatch")
 if value["source"].get("unchanged") is not True or value["source"].get("before_sha256")!=value["source"].get("after_sha256"): raise Failure("MCP SDK source preservation mismatch")
def execute_sdk(stage,python,cargo,rustc,commit,tree,inputs):
 pre={"python":tool_binding(python,("--version",),"Python"),"cargo":tool_binding(cargo,("--version","--verbose"),"Cargo"),"rustc":tool_binding(rustc,("--version","--verbose"),"rustc"),"mcp_sdk":distribution_binding(python)}
 with tempfile.TemporaryDirectory(prefix="semaprax-phase0-sdk-build-",dir="/private/tmp") as build,tempfile.TemporaryDirectory(prefix="semaprax-phase0-sdk-host-",dir="/private/tmp") as host:
  target=Path(build)/"target"; env=os.environ.copy(); env.update({"CARGO_NET_OFFLINE":"true","CARGO_INCREMENTAL":"0","CARGO_TERM_COLOR":"never","RUSTC":rustc,"CARGO_TARGET_DIR":str(target)})
  build_log=command([cargo,"build","--locked","--offline","-p","semaprax","--bin","semaprax"],"SDK compiler build",env=env)
  compiler=(target/"debug/semaprax").resolve(strict=True); compiler_row=file_row(compiler)
  area=Path(host); workspace=area/"workspace"; shutil.copytree(ROOT/"examples/calculator-project",workspace); policy=area/"policy.json"; policy.write_bytes(canonical(POLICY)); stderr=area/"server-stderr.log"
  harness=ROOT/"tools/mcp-sdk-conformance/harness.py"; sdk_command=[python,"-I",str(harness),"--compiler",str(compiler),"--manifest",str(workspace/"semaprax.toml"),"--policy",str(policy),"--stderr",str(stderr)]
  harness_env=os.environ.copy(); harness_env["PYTHONDONTWRITEBYTECODE"]="1"; observation_log=command(sdk_command,"independent MCP SDK",env=harness_env)
  if observation_log.count(b"\n")!=1 or not observation_log.endswith(b"\n"): raise Failure("MCP SDK observation framing mismatch")
  observation_body=observation_log[:-1]; observation=json.loads(observation_body)
  if canonical(observation)!=observation_body: raise Failure("MCP SDK observation is not canonical")
  validate_sdk(observation)
  if file_row(compiler)!=compiler_row: raise Failure("SDK compiler drift")
  stderr_body=stderr.read_bytes()
  if len(stderr_body)>MAX_LOG: raise Failure("MCP SDK stderr exceeds bound")
  sdk_dir=stage/"independent-mcp-sdk"; sdk_dir.mkdir(); bodies={"compiler-build-cargo.log":build_log,"server-stderr.log":stderr_body,"observation.json":observation_body}; artifacts=[]
  for name,body in bodies.items(): (sdk_dir/name).write_bytes(body); artifacts.append({"path":f"independent-mcp-sdk/{name}","bytes":len(body),"sha256":sha(body)})
 post={"python":tool_binding(python,("--version",),"Python"),"cargo":tool_binding(cargo,("--version","--verbose"),"Cargo"),"rustc":tool_binding(rustc,("--version","--verbose"),"rustc"),"mcp_sdk":distribution_binding(python)}
 if post!=pre: raise Failure("MCP SDK tool or distribution drift")
 verify_repository(commit,tree,inputs); return pre,compiler_row,observation,artifacts,sdk_command
def cross_tools(values):
 client=values["client-mcp"]["runner"]["tools"]; vscode=values["vscode-host"]["runner"]["tools"]; canonical_tools=values["canonical-git"]["runner"]["tools"]
 for name in ("cargo","rustc"):
  if client[name]["resolved_path"]!=vscode[name]["path"] or client[name]["resolved_path"]!=canonical_tools[name]["executable"] or client[name]["sha256"]!=vscode[name]["sha256"]: raise Failure(f"cross-component {name} identity mismatch")
 if client["node"]["resolved_path"]!=vscode["node"]["path"] or client["node"]["sha256"]!=vscode["node"]["sha256"]: raise Failure("cross-component Node identity mismatch")
 if client["git"]["resolved_path"]!=canonical_tools["git"]["executable"]: raise Failure("cross-component Git identity mismatch")
def verify_final(destination,evidence):
 if {x.name for x in destination.iterdir()}!={"evidence.json","canonical-git","client-mcp","vscode-host","independent-mcp-sdk"}: raise Failure("aggregate top-level inventory mismatch")
 body=(destination/"evidence.json").read_bytes()
 if canonical(evidence)!=body or json.loads(body)!=evidence: raise Failure("aggregate evidence replay mismatch")
 expected={x["path"] for x in evidence["artifacts"]}; actual={str(x.relative_to(destination)) for x in destination.rglob("*") if x.is_file() and x.relative_to(destination)!=Path("evidence.json")}
 if actual!=expected: raise Failure("aggregate nested inventory mismatch")
 for row in evidence["artifacts"]:
  if file_row(destination/row["path"],row["path"])!=row: raise Failure(f"aggregate artifact mismatch: {row['path']}")
 bid=hashlib.sha256(DOMAIN+b"".join(canonical(x) for x in evidence["artifacts"])).hexdigest()
 if evidence["bundle_id"]!=bid: raise Failure("aggregate bundle ID mismatch")
def main():
 ap=argparse.ArgumentParser(); ap.add_argument("--node",required=True); ap.add_argument("--tsc",required=True); ap.add_argument("--vscode-app",required=True); ap.add_argument("--mcp-python",required=True); ap.add_argument("--output"); ns=ap.parse_args()
 clean(); commit=git("rev-parse","HEAD^{commit}"); tree=git("rev-parse","HEAD^{tree}")
 if not HEX.fullmatch(commit) or not HEX.fullmatch(tree): raise Failure("invalid exact subject")
 symbolic=run([shutil.which("git"),"symbolic-ref","--quiet","--short","HEAD"]); branch=symbolic.stdout.decode("utf-8","strict").strip() if symbolic.returncode==0 else None; tags=sorted(filter(None,git("tag","--points-at",commit).splitlines())); inputs=repository_inputs()
 node=executable(ns.node,"Node"); tsc=executable(ns.tsc,"TypeScript"); mcp_python=executable(ns.mcp_python,"MCP Python"); cargo=executable(os.path.abspath(shutil.which("cargo")),"Cargo"); rustc=executable(os.path.abspath(shutil.which("rustc")),"rustc")
 with tempfile.TemporaryDirectory(prefix="semaprax-phase0-components-",dir="/private/tmp") as temporary:
  stage=Path(temporary); commands={"canonical-git":[sys.executable,str(ROOT/"scripts/graph-operational-evidence.py")],"client-mcp":[sys.executable,str(ROOT/"scripts/graph-operational-client-mcp-evidence.py"),"--node",node,"--tsc",tsc],"vscode-host":[sys.executable,str(ROOT/"scripts/graph-operational-vscode-host-evidence.py"),"--node",node,"--vscode-app",str(Path(ns.vscode_app).resolve(strict=True))]}; components=[]; values={}; artifacts=[]
  for name,script,schema,domain in COMPONENTS:
   incoming=stage/name/"incoming"; cmd=[*commands[name],"--output",str(incoming)]; command(cmd,f"{name} component"); value=validate_component(incoming,name,schema,domain,commit,tree); verify_repository(commit,tree,inputs); bundle=value["bundle_id"]; target=incoming.parent/bundle; incoming.rename(target); values[name]=value
   components.append({"id":name,"schema":schema,"bundle_id":bundle,"path":f"{name}/{bundle}","outcome":"passed","command":cmd,"provisioning":"explicit_local_node_tsc" if name=="client-mcp" else ("explicit_local_visual_studio_code_product" if name=="vscode-host" else "local_unix_git")})
   for path in sorted(target.iterdir()): artifacts.append(file_row(path,f"{name}/{bundle}/{path.name}"))
  cross_tools(values); sdk_tools,compiler_row,observation,sdk_artifacts,sdk_command=execute_sdk(stage,mcp_python,cargo,rustc,commit,tree,inputs)
  for name in ("cargo","rustc"):
   child=values["client-mcp"]["runner"]["tools"][name]
   if sdk_tools[name]["path"]!=child["resolved_path"] or sdk_tools[name]["sha256"]!=child["sha256"]: raise Failure(f"SDK/component {name} identity mismatch")
  artifacts.extend(sdk_artifacts); components.append({"id":"independent-mcp-sdk","schema":observation["schema"],"bundle_id":sha(canonical(observation))[7:],"path":"independent-mcp-sdk","outcome":"passed","command":sdk_command,"provisioning":"explicit_local_python_mcp_1.27.0"}); artifacts.sort(key=lambda x:x["path"]); bid=hashlib.sha256(DOMAIN+b"".join(canonical(x) for x in artifacts)).hexdigest()
  evidence={"schema":SCHEMA,"bundle_id":bid,"repository":{"commit":commit,"tree":tree,"subject_kind":"exact_local_commit","head_relation_at_capture":"HEAD","current_head_at_capture":True,"branch":branch,"clean_before_and_after":True,"head_unchanged":True,"inputs":inputs},"exact_tag":{"selection":"not_required","observed":tags},"runner":{"host":{"system":platform.system(),"release":platform.release(),"machine":platform.machine()},"sdk_tools":sdk_tools,"sdk_compiler":compiler_row},"components":components,"ignored_tests":[{"test":"signature_evolution_merge_reports_tests_and_separate_managed_publication","default":"not_selected","reason":"SPX-G150 wrong ACTIVE schema, needs workspace init fix"},{"test":"provisioned_typescript_submits_exact_typed_request_for_compiler_admission","default":"ignored","separate":"passed_provisioned_local"},{"test":"provisioned_typescript_harness_checks_actual_recursive_repair_payloads_and_hostile_nested_values","default":"ignored","separate":"passed_provisioned_local"}],"dimensions":{"canonical_git":{"passed":3,"failed":0},"generated_client_and_authored_mcp":{"passed":25,"failed":0,"default_ignored":2},"vscode":{"standalone_controllers_passed":50,"actual_host_passed":1},"independent_mcp_sdk":{"passed":1,"failed":0}},"artifacts":artifacts,"claims":{"same_exact_subject_selected_components":"executed","phase0_selected_local_evidence_set":"passed","typescript_python_rust_request_admission":"passed_selected_flow","independent_python_mcp_sdk_interoperability":"passed","full_mcp_conformance_certification":"not_claimed","managed_active":"not_executed","exact_release_tag":"not_claimed","remote_main_or_later_head":"not_claimed","hosted_cross_platform":"not_observed","network_isolation":"not_claimed","full_quality":"not_selected","programme_completion":"not_claimed"}}
  destination=Path(ns.output).resolve() if ns.output else ROOT/".semaprax/evidence/graph-operational-phase0"/commit/bid; destination.parent.mkdir(parents=True,exist_ok=True)
  if destination.exists() or destination.is_symlink(): raise Failure(f"destination exists: {destination}")
  publish=Path(tempfile.mkdtemp(prefix=".graph-operational-phase0-",dir=destination.parent))
  try:
   for name in ("canonical-git","client-mcp","vscode-host","independent-mcp-sdk"): shutil.copytree(stage/name,publish/name)
   (publish/"evidence.json").write_bytes(canonical(evidence)); publish.rename(destination)
  except BaseException: shutil.rmtree(publish,ignore_errors=True); raise
 try: verify_repository(commit,tree,inputs); verify_final(destination,evidence)
 except BaseException: shutil.rmtree(destination,ignore_errors=True); raise
 print(destination); print(bid)
if __name__=="__main__":
 try: main()
 except (Failure,OSError,ValueError,KeyError,TypeError) as e: print(f"phase0 evidence failed: {e}",file=sys.stderr); raise SystemExit(1)
