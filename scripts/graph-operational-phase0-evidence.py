#!/usr/bin/env python3
"""Execute the selected graph-operational Phase 0 gates at one exact local HEAD."""
import argparse, hashlib, json, os, platform, re, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
SCHEMA="semaprax.graph-operational-phase0-execution-evidence.v3"
DOMAIN=b"semaprax.graph-operational-phase0-execution-evidence.bundle.v3\0"
HEX=re.compile(r"[0-9a-f]{40}|[0-9a-f]{64}")
MAX_LOG=16*1024*1024; MAX_ARTIFACT=512*1024*1024; TIMEOUT=30*60
PACKAGED_TEST="installed_typescript_sdk_drives_review_and_separately_approved_publish"
PACKAGED_SCHEMA="semaprax.packaged-typescript-workflow-mcp-observation.v1"
POLICY={"schema":"semaprax.workspace-host-policy.v7","candidate_prepare":True,"diagnostics":False,"build_enabled":False,"test_policy":None,"git_commit":None,"frontend_cache":False,"candidate_archives":[],"semantic_cache":False,"semantic_cache_entry":None,"draft_archives":[],"read_batch_workers":None}
COMPONENTS=(
 ("canonical-git","scripts/graph-operational-evidence.py","semaprax.graph-operational-execution-evidence.v2","semaprax.graph-operational-execution-evidence.bundle.v2","digest_text"),
 ("client-mcp","scripts/graph-operational-client-mcp-evidence.py","semaprax.graph-operational-client-mcp-execution-evidence.v2","semaprax.graph-operational-client-mcp-execution-evidence.bundle.v2","digest_text"),
 ("product-workflow","scripts/graph-operational-phase1-product-workflow-evidence.py","semaprax.graph-operational-phase1-product-workflow-execution-evidence.v1","semaprax.graph-operational-phase1-product-workflow-execution-evidence.bundle.v1","canonical_rows"),
 ("vscode-host","scripts/graph-operational-vscode-host-evidence.py","semaprax.graph-operational-vscode-host-execution-evidence.v2","semaprax.graph-operational-vscode-host-execution-evidence.bundle.v2","raw_digests"),)
CANONICAL_GATES=(
 ("graph_operational_git_workflow_v1",["cargo","test","--locked","--offline","-p","semaprax","--test","project_graph_operational_git_workflow_v1","--","--test-threads=1","--nocapture"],"local_unix_git","cargo.log",("competing_real_git_ref_consumes_approval_without_overwriting_the_other_commit","real_git_ref_update_with_lost_response_is_terminal_and_requires_inspection","twelve_step_v5_review_to_real_sha1_git_commit","twelve_step_v5_review_to_real_sha256_git_commit")),
 ("candidate_managed_publication_v1",["cargo","test","--locked","--offline","-p","semaprax","--test","project_candidate_publication_v1","--","--test-threads=1","--nocapture"],"local_managed_workspace","candidate-publication-cargo.log",("existing_exclusive_lock_is_required_before_replay_or_candidate_approval_checks","prepare_is_read_only_and_apply_changes_only_the_managed_active_generation","proof_tamper_approval_and_host_substitution_reject_before_any_generation_write","raw_source_drift_and_single_changed_file_never_pad_or_publish")),
 ("graph_operational_managed_workflow_v1",["cargo","test","--locked","--offline","-p","semaprax","--test","project_graph_operational_workflow_v1","--","--test-threads=1","--nocapture"],"local_managed_workspace","managed-workflow-cargo.log",("signature_evolution_merge_reports_tests_and_separate_managed_publication",)),)
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
def regular_input(value,label):
 p=Path(value).expanduser()
 if not p.is_absolute(): raise Failure(f"{label} must be an absolute file")
 if p.is_symlink(): raise Failure(f"{label} must not be a symlink")
 p=p.resolve(strict=True)
 if not p.is_file(): raise Failure(f"{label} must be a regular file")
 return str(p)
def repository_inputs(): return [file_row(ROOT/name,name) for name in ("Cargo.toml","Cargo.lock")]
def verify_repository(commit,tree,inputs):
 clean()
 if git("rev-parse","HEAD^{commit}")!=commit or git("rev-parse","HEAD^{tree}")!=tree: raise Failure("repository subject drift")
 for expected in inputs:
  if file_row(ROOT/expected["path"],expected["path"])!=expected: raise Failure(f"repository input drift: {expected['path']}")
def child_bundle(domain,artifacts,mode):
 seed=domain.encode()+b"\0"
 if mode=="raw_digests": seed+=b"".join(bytes.fromhex(x["sha256"][7:]) for x in artifacts)
 elif mode=="canonical_rows": seed+=b"".join(canonical(x) for x in artifacts)
 elif mode=="digest_text": seed+=b"\0".join(x["sha256"].encode() for x in artifacts)
 else: raise Failure(f"unknown child bundle mode: {mode}")
 return hashlib.sha256(seed).hexdigest()
def validate_outcomes(name,value):
 repo=value.get("repository",{})
 if name=="canonical-git":
  if (repo.get("clean_before"),repo.get("clean_after"),repo.get("head_unchanged"))!=(True,True,True): raise Failure("canonical Git repository state mismatch")
  gates=value.get("gates")
  if not isinstance(gates,list) or len(gates)!=len(CANONICAL_GATES): raise Failure("canonical publication gate inventory mismatch")
  for gate,(gate_id,gate_command,prerequisite,log,tests) in zip(gates,CANONICAL_GATES):
   counts={"selected":len(tests),"passed":len(tests),"failed":0,"ignored":0,"measured":0,"filtered_out":0}
   expected={"id":gate_id,"selection":"default","prerequisite":prerequisite,"provisioning":"not_required","command":gate_command,"outcome":"passed","exit_code":0,"counts":counts,"tests":[{"name":test,"outcome":"passed"} for test in tests],"log":log}
   if gate!=expected: raise Failure(f"canonical publication gate mismatch: {gate_id}")
  claims=value.get("claims",{})
  required={"real_git_post_cas_uncertainty":"executed_injected_result_loss_after_real_ref_update","managed_publication_boundary_controls":"executed_local","bounded_twelve_step_managed_workflow":"executed_local","managed_active_workflow":"executed_local_managed_generation"}
  if any(claims.get(key)!=expected for key,expected in required.items()): raise Failure("canonical publication claims mismatch")
 elif name=="client-mcp":
  if (repo.get("clean_before"),repo.get("clean_after"),repo.get("head_unchanged"))!=(True,True,True): raise Failure("client/MCP repository state mismatch")
  expected={"generated_clients_ordinary_v1":{"selected":12,"passed":10,"failed":0,"ignored":2,"measured":0,"filtered_out":0},"generated_client_typescript_request_provisioned_v1":{"selected":1,"passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":4},"generated_client_typescript_provisioned_v1":{"selected":1,"passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":3},"workspace_mcp_adapter_v1":{"selected":8,"passed":8,"failed":0,"ignored":0,"measured":0,"filtered_out":0},"workspace_mcp_cli_stdio_v1":{"selected":5,"passed":5,"failed":0,"ignored":0,"measured":0,"filtered_out":0}}
  rows=value.get("executions")
  if not isinstance(rows,list) or {x.get("id") for x in rows}!=set(expected): raise Failure("client/MCP gate inventory mismatch")
  for gate in rows:
   if gate.get("outcome")!="passed" or gate.get("exit_code")!=0 or gate.get("counts")!=expected[gate["id"]]: raise Failure(f"client/MCP gate outcome mismatch: {gate.get('id')}")
  required={"generated_client_python_request_admission":"passed","generated_client_rust_request_admission":"passed","generated_client_typescript_request_admission":"passed_provisioned_local","mcp_adapter_in_process":"passed","mcp_cli_stdio_local_subprocess":"passed"}
  if any(value.get("observations",{}).get(k)!=v for k,v in required.items()): raise Failure("client/MCP observations mismatch")
 elif name=="product-workflow":
  if (repo.get("clean_before"),repo.get("clean_after"),repo.get("head_unchanged"))!=(True,True,True): raise Failure("product workflow repository state mismatch")
  expected={"generated_product_workflow_python_rust_v1":{"selected":3,"passed":2,"failed":0,"ignored":1,"measured":0,"filtered_out":0},"generated_product_workflow_hostile_v1":{"selected":1,"passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0},"generated_product_workflow_typescript_v1":{"selected":1,"passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":2}}
  rows=value.get("executions")
  if not isinstance(rows,list) or {x.get("id") for x in rows}!=set(expected): raise Failure("product workflow gate inventory mismatch")
  for gate in rows:
   if gate.get("outcome")!="passed" or gate.get("exit_code")!=0 or gate.get("counts")!=expected[gate["id"]]: raise Failure(f"product workflow gate outcome mismatch: {gate.get('id')}")
  observations=value.get("observations",{})
  required={"workflow":"function_signature_review_publish_v1","generated_python":"passed_local_exact_subject","generated_rust":"passed_local_exact_subject","generated_typescript":"passed_explicitly_provisioned_local_exact_subject","closed_success_transcripts":3,"closed_handoff_artifacts":3,"closed_generated_client_artifacts":3,"hostile_transition_cases":10,"publication_fixture":"isolated_local_unix_bare_sha256_git","raw_source_preservation":"passed_per_language"}
  if any(observations.get(key)!=expected for key,expected in required.items()): raise Failure("product workflow observations mismatch")
  if "packaged_sdk_editor_ui_or_mcp_certification" not in value.get("nonclaims",[]): raise Failure("product workflow MCP/package nonclaim is absent")
 else:
  if repo.get("clean_before_and_after") is not True or repo.get("current_head") is not True: raise Failure("VS Code repository state mismatch")
  expected=[{"id":"vscode_node_mock_controllers_v2","passed":57,"failed":0,"ignored":0},{"id":"vscode_extension_host_real_compiler_task_control_v2","passed":1,"failed":0,"ignored":0}]
  if value.get("executions")!=expected: raise Failure("VS Code execution outcomes mismatch")
  observation=value.get("observation",{}); required={"schema":"semaprax.vscode-extension-host-result.v2","app_name":"Visual Studio Code","typed_intent":"rename_declaration","verified_virtual_diff":True,"explicit_cooperative_cancellation":True,"pending_task_dirty_buffer_invalidated":True,"dirty_buffer_invalidated":True,"source_bytes_unchanged":True}
  if any(observation.get(k)!=v for k,v in required.items()): raise Failure("VS Code host observation mismatch")
  if observation.get("discovered_task_tools")!=["candidate/test-task-start","candidate/test-task-status","candidate/test-task-cancel","candidate/test-task-result"]: raise Failure("VS Code task catalogue mismatch")
  if observation.get("cancellation")!={"state":"cancelled","before_step":1,"steps_used":0,"report_released":False,"source_authority":False}: raise Failure("VS Code cancellation observation mismatch")
  if observation.get("authority")!={"source_write":False,"build":False,"commit":False,"publication":False}: raise Failure("VS Code authority observation mismatch")
def validate_component(path,name,schema,domain,bundle_mode,commit,tree):
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
 if value.get("bundle_id")!=child_bundle(domain,artifacts,bundle_mode): raise Failure(f"{name} bundle ID mismatch")
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
def execute_packaged_sdk(stage,node,npm_cli,tsc,git_tool,cargo,rustc,commit,tree,inputs):
 def bindings():
  rows={name:tool_binding(path,args,name) for name,path,args in (("node",node,("--version",)),("cargo",cargo,("--version","--verbose")),("rustc",rustc,("--version","--verbose")),("git",git_tool,("--version",)))}
  rows["npm_cli"]=file_row(npm_cli); rows["npm_cli"]["version"]=text([node,npm_cli,"--version"],"npm version")
  rows["tsc"]=file_row(tsc); rows["tsc"]["version"]=text([node,tsc,"--version"],"TypeScript version")
  if rows["tsc"]["version"]!="Version 5.8.3": raise Failure("packaged SDK requires TypeScript 5.8.3")
  return rows
 pre=bindings()
 command_line=[cargo,"test","--locked","--offline","-p","semaprax","--test","image_packaged_typescript_workflow_v1",PACKAGED_TEST,"--","--exact","--ignored","--nocapture","--test-threads=1"]
 with tempfile.TemporaryDirectory(prefix="semaprax-phase0-packaged-sdk-",dir="/private/tmp") as build:
  env=os.environ.copy()
  for key in ("CARGO_BUILD_RUSTFLAGS","CARGO_BUILD_TARGET","CARGO_ENCODED_RUSTFLAGS","RUSTC_WRAPPER","RUSTC_WORKSPACE_WRAPPER","RUSTFLAGS","RUSTDOCFLAGS","NODE_OPTIONS","NODE_PATH"): env.pop(key,None)
  env.update({"CARGO_NET_OFFLINE":"true","CARGO_INCREMENTAL":"0","CARGO_TERM_COLOR":"never","CARGO_TARGET_DIR":str(Path(build)/"target"),"RUSTC":rustc,"NODE":node,"NPM_CLI":npm_cli,"TSC_CLI":tsc,"SEMAPRAX_TEST_GIT":git_tool})
  log=command(command_line,"packaged TypeScript SDK over MCP",timeout=65*60,env=env)
 body=log.decode("utf-8","strict")
 if not re.search(rf"^test {re.escape(PACKAGED_TEST)} \.\.\. ok(?:, [^\r\n]+)?$",body,re.MULTILINE): raise Failure("packaged SDK selected test did not pass exactly")
 if not re.search(r"^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [^\r\n]+$",body,re.MULTILINE): raise Failure("packaged SDK libtest summary mismatch")
 post=bindings()
 if post!=pre: raise Failure("packaged SDK tool binding drift")
 verify_repository(commit,tree,inputs)
 observation={"schema":PACKAGED_SCHEMA,"protocol":{"mcp":"2025-11-25","inner":"semaprax.image-agent-protocol.v5"},"package":{"name":"@semaprax/agent-workflow","version":"0.1.0","installation":"fresh_offline_tarball"},"execution":{"test":PACKAGED_TEST,"outcome":"passed","selection":"explicit_ignored","counts":{"selected":1,"passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0}},"flows":{"review":"passed","structured_failure":"passed","approved_publication":"passed","duplicate_publication":"rejected","post_cas_result_loss":"terminal_publication_uncertain"},"authority":{"package_or_mcp_grants_authority":False,"publication_authority":"startup_selected_test_host","raw_project_source_unchanged":True},"nonclaims":["general_sdk_support","full_mcp_conformance","network_isolation","hosted_or_cross_platform","external_consumer_compatibility","programme_completion"]}
 body=canonical(observation); area=stage/"packaged-sdk-mcp"; area.mkdir(); (area/"cargo.log").write_bytes(log); (area/"observation.json").write_bytes(body)
 artifacts=[]
 for name,payload in (("cargo.log",log),("observation.json",body)): artifacts.append({"path":f"packaged-sdk-mcp/{name}","bytes":len(payload),"sha256":sha(payload)})
 return pre,observation,artifacts,command_line
def cross_tools(values):
 client=values["client-mcp"]["runner"]["tools"]; vscode=values["vscode-host"]["runner"]["tools"]; canonical_tools=values["canonical-git"]["runner"]["tools"]
 product=values["product-workflow"]["runner"]["tools"]
 for name in ("cargo","rustc"):
  if client[name]["resolved_path"]!=vscode[name]["path"] or client[name]["resolved_path"]!=canonical_tools[name]["executable"] or client[name]["sha256"]!=vscode[name]["sha256"]: raise Failure(f"cross-component {name} identity mismatch")
  if product[name]["resolved_path"]!=client[name]["resolved_path"] or product[name]["sha256"]!=client[name]["sha256"]: raise Failure(f"product workflow {name} identity mismatch")
 if client["node"]["resolved_path"]!=vscode["node"]["path"] or client["node"]["sha256"]!=vscode["node"]["sha256"]: raise Failure("cross-component Node identity mismatch")
 if product["node"]["resolved_path"]!=client["node"]["resolved_path"] or product["node"]["sha256"]!=client["node"]["sha256"]: raise Failure("product workflow Node identity mismatch")
 if client["git"]["resolved_path"]!=canonical_tools["git"]["executable"]: raise Failure("cross-component Git identity mismatch")
 if product["git"]["resolved_path"]!=client["git"]["resolved_path"] or product["git"]["sha256"]!=client["git"]["sha256"]: raise Failure("product workflow Git identity mismatch")
def verify_final(destination,evidence):
 if {x.name for x in destination.iterdir()}!={"evidence.json","canonical-git","client-mcp","product-workflow","vscode-host","independent-mcp-sdk","packaged-sdk-mcp"}: raise Failure("aggregate top-level inventory mismatch")
 body=(destination/"evidence.json").read_bytes()
 if canonical(evidence)!=body or json.loads(body)!=evidence: raise Failure("aggregate evidence replay mismatch")
 expected={x["path"] for x in evidence["artifacts"]}; actual={str(x.relative_to(destination)) for x in destination.rglob("*") if x.is_file() and x.relative_to(destination)!=Path("evidence.json")}
 if actual!=expected: raise Failure("aggregate nested inventory mismatch")
 for row in evidence["artifacts"]:
  if file_row(destination/row["path"],row["path"])!=row: raise Failure(f"aggregate artifact mismatch: {row['path']}")
 bid=hashlib.sha256(DOMAIN+b"".join(canonical(x) for x in evidence["artifacts"])).hexdigest()
 if evidence["bundle_id"]!=bid: raise Failure("aggregate bundle ID mismatch")
def main():
 ap=argparse.ArgumentParser(); ap.add_argument("--node",required=True); ap.add_argument("--npm-cli",required=True); ap.add_argument("--tsc",required=True); ap.add_argument("--typescript-package-root",required=True); ap.add_argument("--vscode-app",required=True); ap.add_argument("--mcp-python",required=True); ap.add_argument("--output"); ns=ap.parse_args()
 clean(); commit=git("rev-parse","HEAD^{commit}"); tree=git("rev-parse","HEAD^{tree}")
 if not HEX.fullmatch(commit) or not HEX.fullmatch(tree): raise Failure("invalid exact subject")
 symbolic=run([shutil.which("git"),"symbolic-ref","--quiet","--short","HEAD"]); branch=symbolic.stdout.decode("utf-8","strict").strip() if symbolic.returncode==0 else None; tags=sorted(filter(None,git("tag","--points-at",commit).splitlines())); inputs=repository_inputs()
 node=executable(ns.node,"Node"); npm_cli=regular_input(ns.npm_cli,"npm CLI"); tsc=executable(ns.tsc,"TypeScript"); mcp_python=executable(ns.mcp_python,"MCP Python"); cargo=executable(os.path.abspath(shutil.which("cargo")),"Cargo"); rustc=executable(os.path.abspath(shutil.which("rustc")),"rustc"); git_tool=executable(os.path.abspath(shutil.which("git")),"Git")
 with tempfile.TemporaryDirectory(prefix="semaprax-phase0-components-",dir="/private/tmp") as temporary:
  typescript_root=str(Path(ns.typescript_package_root).resolve(strict=True)); stage=Path(temporary); commands={"canonical-git":[sys.executable,str(ROOT/"scripts/graph-operational-evidence.py")],"client-mcp":[sys.executable,str(ROOT/"scripts/graph-operational-client-mcp-evidence.py"),"--node",node,"--tsc",tsc],"product-workflow":[sys.executable,str(ROOT/"scripts/graph-operational-phase1-product-workflow-evidence.py"),"--python",mcp_python,"--node",node,"--tsc",tsc,"--typescript-package-root",typescript_root],"vscode-host":[sys.executable,str(ROOT/"scripts/graph-operational-vscode-host-evidence.py"),"--node",node,"--vscode-app",str(Path(ns.vscode_app).resolve(strict=True))]}; components=[]; values={}; artifacts=[]
  for name,script,schema,domain,bundle_mode in COMPONENTS:
   incoming=stage/name/"incoming"; cmd=[*commands[name],"--output",str(incoming)]; command(cmd,f"{name} component",timeout=65*60 if name in ("canonical-git","product-workflow") else TIMEOUT); value=validate_component(incoming,name,schema,domain,bundle_mode,commit,tree); verify_repository(commit,tree,inputs); bundle=value["bundle_id"]; target=incoming.parent/bundle; incoming.rename(target); values[name]=value
   provisioning={"client-mcp":"explicit_local_node_tsc","product-workflow":"explicit_local_python_node_typescript","vscode-host":"explicit_local_visual_studio_code_product"}.get(name,"local_unix_git")
   components.append({"id":name,"schema":schema,"bundle_id":bundle,"path":f"{name}/{bundle}","outcome":"passed","command":cmd,"provisioning":provisioning})
   for path in sorted(target.iterdir()): artifacts.append(file_row(path,f"{name}/{bundle}/{path.name}"))
  cross_tools(values); sdk_tools,compiler_row,observation,sdk_artifacts,sdk_command=execute_sdk(stage,mcp_python,cargo,rustc,commit,tree,inputs)
  for name in ("cargo","rustc"):
   child=values["client-mcp"]["runner"]["tools"][name]
   if sdk_tools[name]["path"]!=child["resolved_path"] or sdk_tools[name]["sha256"]!=child["sha256"]: raise Failure(f"SDK/component {name} identity mismatch")
  product_python=values["product-workflow"]["runner"]["tools"]["python"]
  if product_python["resolved_path"]!=sdk_tools["python"]["path"] or product_python["sha256"]!=sdk_tools["python"]["sha256"]: raise Failure("SDK/product workflow Python identity mismatch")
  artifacts.extend(sdk_artifacts); components.append({"id":"independent-mcp-sdk","schema":observation["schema"],"bundle_id":sha(canonical(observation))[7:],"path":"independent-mcp-sdk","outcome":"passed","command":sdk_command,"provisioning":"explicit_local_python_mcp_1.27.0"}); artifacts.sort(key=lambda x:x["path"]); bid=hashlib.sha256(DOMAIN+b"".join(canonical(x) for x in artifacts)).hexdigest()
  packaged_tools,packaged_observation,packaged_artifacts,packaged_command=execute_packaged_sdk(stage,node,npm_cli,tsc,git_tool,cargo,rustc,commit,tree,inputs)
  for name in ("cargo","rustc","node","git"):
   child=values["product-workflow"]["runner"]["tools"][name]
   if packaged_tools[name]["path"]!=child["resolved_path"] or packaged_tools[name]["sha256"]!=child["sha256"]: raise Failure(f"packaged SDK/product workflow {name} identity mismatch")
  artifacts.extend(packaged_artifacts); components.append({"id":"packaged-sdk-mcp","schema":PACKAGED_SCHEMA,"bundle_id":sha(canonical(packaged_observation))[7:],"path":"packaged-sdk-mcp","outcome":"passed","command":packaged_command,"provisioning":"explicit_local_node_npm_typescript_git"}); artifacts.sort(key=lambda x:x["path"]); bid=hashlib.sha256(DOMAIN+b"".join(canonical(x) for x in artifacts)).hexdigest()
  evidence={"schema":SCHEMA,"bundle_id":bid,"repository":{"commit":commit,"tree":tree,"subject_kind":"exact_local_commit","head_relation_at_capture":"HEAD","current_head_at_capture":True,"branch":branch,"clean_before_and_after":True,"head_unchanged":True,"inputs":inputs},"exact_tag":{"selection":"not_required","observed":tags,"claim":"not_claimed"},"runner":{"host":{"system":platform.system(),"release":platform.release(),"machine":platform.machine()},"sdk_tools":sdk_tools,"sdk_compiler":compiler_row,"packaged_sdk_tools":packaged_tools},"components":components,"evidence_classes":{"current_head":{"status":"executed_exact_local_subject","components":["canonical-git","client-mcp","product-workflow","vscode-host","independent-mcp-sdk","packaged-sdk-mcp"]},"exact_tag":{"status":"not_selected","observed":tags},"provisioned":{"status":"executed_selected_local_tools","components":["client-mcp","product-workflow","vscode-host","independent-mcp-sdk","packaged-sdk-mcp"]},"default_ignored":{"status":"not_counted_as_default_execution","separately_selected":["provisioned_typescript_submits_exact_typed_request_for_compiler_admission","provisioned_typescript_harness_checks_actual_recursive_repair_payloads_and_hostile_nested_values","provisioned_typescript_reference_review_export_and_real_git_commit","installed_typescript_sdk_drives_review_and_separately_approved_publish"]},"authored_unrun":{"status":"not_executed_by_this_aggregate","slices":["vscode_marketplace_or_vsix","full_mcp_conformance"]}},"ignored_tests":[{"test":"provisioned_typescript_submits_exact_typed_request_for_compiler_admission","default":"ignored","separate":"passed_provisioned_local"},{"test":"provisioned_typescript_harness_checks_actual_recursive_repair_payloads_and_hostile_nested_values","default":"ignored","separate":"passed_provisioned_local"},{"test":"provisioned_typescript_reference_review_export_and_real_git_commit","default":"ignored","separate":"passed_provisioned_local"},{"test":"installed_typescript_sdk_drives_review_and_separately_approved_publish","default":"ignored","separate":"passed_provisioned_local"}],"dimensions":{"canonical_git":{"passed":4,"failed":0},"candidate_managed_publication":{"passed":4,"failed":0},"integrated_managed_workflow":{"passed":1,"failed":0},"generated_client_and_authored_mcp":{"passed":25,"failed":0,"default_ignored":2},"generated_product_workflow":{"passed":4,"failed":0,"default_ignored":1,"hostile_cases":10,"successful_language_workflows":3},"packaged_typescript_workflow_over_mcp":{"passed":1,"failed":0,"default_ignored":1},"vscode":{"standalone_controllers_passed":57,"actual_host_passed":1,"task_control":True},"independent_mcp_sdk":{"passed":1,"failed":0}},"artifacts":artifacts,"claims":{"same_exact_subject_selected_components":"executed","phase0_selected_local_evidence_set":"passed","typescript_python_rust_request_admission":"passed_selected_flow","generated_python_rust_typescript_product_workflow":"passed_selected_flow","independent_python_mcp_sdk_interoperability":"passed","vscode_task_cancellation_and_session_invalidation":"passed_selected_flow","packaged_typescript_workflow_over_mcp":"passed_selected_flow","full_mcp_conformance_certification":"not_claimed","managed_active":"executed_local_managed_generation","real_git_post_cas_uncertainty":"executed_injected_result_loss_after_real_ref_update","exact_release_tag":"not_claimed","remote_main_or_later_head":"not_claimed","hosted_cross_platform":"not_observed","network_isolation":"not_claimed","full_quality":"not_selected","programme_completion":"not_claimed"}}
  destination=Path(ns.output).resolve() if ns.output else ROOT/".semaprax/evidence/graph-operational-phase0"/commit/bid; destination.parent.mkdir(parents=True,exist_ok=True)
  if destination.exists() or destination.is_symlink(): raise Failure(f"destination exists: {destination}")
  publish=Path(tempfile.mkdtemp(prefix=".graph-operational-phase0-",dir=destination.parent))
  try:
   for name in ("canonical-git","client-mcp","product-workflow","vscode-host","independent-mcp-sdk","packaged-sdk-mcp"): shutil.copytree(stage/name,publish/name)
   (publish/"evidence.json").write_bytes(canonical(evidence)); publish.rename(destination)
  except BaseException: shutil.rmtree(publish,ignore_errors=True); raise
 try: verify_repository(commit,tree,inputs); verify_final(destination,evidence)
 except BaseException: shutil.rmtree(destination,ignore_errors=True); raise
 print(destination); print(bid)
if __name__=="__main__":
 try: main()
 except (Failure,OSError,ValueError,KeyError,TypeError) as e: print(f"phase0 evidence failed: {e}",file=sys.stderr); raise SystemExit(1)
