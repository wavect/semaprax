#!/usr/bin/env python3
"""Execute exact local VS Code Extension Host evidence for SEMAPRAX."""
import argparse, hashlib, json, os, platform, re, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = "semaprax.graph-operational-vscode-host-execution-evidence.v1"
MAX_LOG = 16 * 1024 * 1024
TIMEOUT = 180
FILES = [
    "Cargo.toml", "Cargo.lock", "editors/vscode/package.json",
    "editors/vscode/extension.js", "editors/vscode/protocol.js", "editors/vscode/review.js",
    "editors/vscode/holes.js", "editors/vscode/repairs.js",
    "editors/vscode/test/extension-host/index.js",
    "examples/calculator-project/semaprax.toml", "examples/calculator-project/src/app.spx",
    "examples/calculator-project/src/core.spx", "examples/calculator-project/src/tests.spx",
]
NODE_TESTS = [
    "editors/vscode/test/protocol.test.js", "editors/vscode/test/review.test.js",
    "editors/vscode/test/holes.test.js", "editors/vscode/test/holes-suggestions.test.js",
    "editors/vscode/test/repairs.test.js",
]
POLICY = {"schema":"semaprax.workspace-host-policy.v7","candidate_prepare":True,
 "diagnostics":False,"build_enabled":False,"test_policy":None,"git_commit":None,
 "frontend_cache":False,"candidate_archives":[],"semantic_cache":False,
 "semantic_cache_entry":None,"draft_archives":[],"read_batch_workers":None}
MARKER = re.compile(rb"^SEMAPRAX_VSCODE_HOST_RESULT=(\{[^\r\n]+\})$", re.MULTILINE)

class Failure(Exception): pass

def run(args, **kw):
    try:
        return subprocess.run(args, cwd=ROOT, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, timeout=kw.pop("timeout", TIMEOUT), check=False, **kw)
    except subprocess.TimeoutExpired as e: raise Failure(f"timeout: {args[0]}") from e

def command(args, label, **kw):
    r=run(args, **kw)
    if len(r.stdout)>MAX_LOG: raise Failure(f"{label} log exceeds bound")
    if r.returncode:
        tail=r.stdout[-8192:].decode("utf-8","replace")
        raise Failure(f"{label} failed ({r.returncode}):\n{tail}")
    return r.stdout

def text(args, label):
    return command(args,label).decode("utf-8","strict").strip()

def sha(body): return "sha256:"+hashlib.sha256(body).hexdigest()
def canonical(value): return json.dumps(value,sort_keys=True,separators=(",",":"),allow_nan=False).encode()
def file_row(path):
    p=Path(path).resolve(strict=True); body=p.read_bytes()
    return {"path":str(p),"bytes":len(body),"sha256":sha(body)}
def repo_row(name):
    p=ROOT/name; body=p.read_bytes()
    return {"path":name,"bytes":len(body),"sha256":sha(body)}
def git(*args): return text([shutil.which("git"),*args],"git")
def clean():
    if git("status","--porcelain=v1","--untracked-files=all"): raise Failure("worktree is not clean")
def tool(name):
    p=shutil.which(name)
    if not p: raise Failure(f"missing tool: {name}")
    return str(Path(p).resolve(strict=True))
def artifact(name,body): return {"path":name,"bytes":len(body),"sha256":sha(body)}

def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--vscode-app",required=True); ap.add_argument("--node"); ap.add_argument("--output")
    ns=ap.parse_args(); clean()
    commit=git("rev-parse","HEAD^{commit}"); tree=git("rev-parse","HEAD^{tree}"); tags=git("tag","--points-at",commit).splitlines()
    inputs=[repo_row(x) for x in FILES]
    app=Path(ns.vscode_app).resolve(strict=True)
    choices=[app/"Contents/MacOS/Code",app/"Contents/MacOS/Electron"]
    code=next((candidate for candidate in choices if candidate.exists()),choices[0]); cli=app/"Contents/Resources/app/bin/code"
    product=app/"Contents/Resources/app/product.json"; product_package=app/"Contents/Resources/app/package.json"
    for p in (code,cli,product,product_package):
        if not p.exists(): raise Failure(f"incomplete VS Code product: {p}")
    product_value=json.loads(product.read_text()); package_value=json.loads(product_package.read_text())
    if product_value.get("nameLong") != "Visual Studio Code": raise Failure("selected product is not Visual Studio Code")
    cli_version=text([str(cli),"--version"],"VS Code identity").splitlines()
    if len(cli_version)!=3 or cli_version[0] != package_value.get("version"): raise Failure("VS Code version mismatch")
    node=str(Path(ns.node or tool("node")).resolve(strict=True)); cargo=tool("cargo"); rustc=tool("rustc")
    bound_paths={"node":node,"cargo":cargo,"rustc":rustc,"code":str(code),"cli":str(cli),"product":str(product),"package":str(product_package)}
    bound_rows={name:file_row(path) for name,path in bound_paths.items()}
    versions={"node":text([node,"--version"],"node version"),"cargo":text([cargo,"--version"],"cargo version"),"rustc":text([rustc,"--version"],"rustc version"),"vscode":cli_version}
    node_log=command([node,"--test","--test-concurrency=1","--test-reporter=tap",*NODE_TESTS],"Node controllers")
    for name,expected in ((b"tests",50),(b"pass",50),(b"fail",0),(b"skipped",0)):
        rows=re.findall(rb"^# "+name+rb" ([0-9]+)$",node_log,re.MULTILINE)
        if rows != [str(expected).encode()]: raise Failure(f"unexpected Node controller {name.decode()} inventory: {rows!r}")
    build_temp=tempfile.TemporaryDirectory(prefix="semaprax-vscode-build-",dir="/private/tmp")
    build_target=Path(build_temp.name)/"target"
    build_env=os.environ.copy(); build_env.update({"CARGO_NET_OFFLINE":"true","CARGO_INCREMENTAL":"0","CARGO_TERM_COLOR":"never","RUSTC":rustc,"CARGO_TARGET_DIR":str(build_target)})
    build_log=command([cargo,"build","--locked","--offline","-p","semaprax","--bin","semaprax"],"compiler build",env=build_env)
    compiler=(build_target/"debug/semaprax").resolve(strict=True); compiler_before=file_row(compiler)
    with tempfile.TemporaryDirectory(prefix="semaprax-vscode-host-",dir="/private/tmp") as td:
        area=Path(td); workspace=area/"workspace"; shutil.copytree(ROOT/"examples/calculator-project",workspace)
        policy=area/"policy.json"; policy.write_bytes(canonical(POLICY))
        user=area/"user"; extensions=area/"extensions"; (user/"User").mkdir(parents=True); extensions.mkdir()
        settings={"semaprax.compilerPath":str(compiler),"semaprax.manifestPath":str(workspace/"semaprax.toml"),"semaprax.hostPolicyPath":str(policy)}
        (user/"User/settings.json").write_bytes(canonical(settings))
        source=workspace/"src/core.spx"; fixture_before={str(p.relative_to(workspace)):sha(p.read_bytes()) for p in sorted(workspace.rglob("*")) if p.is_file()}
        env=os.environ.copy(); env.update({
          "SEMAPRAX_VSCODE_COMPILER":str(compiler),"SEMAPRAX_VSCODE_MANIFEST":str(workspace/"semaprax.toml"),
          "SEMAPRAX_VSCODE_POLICY":str(policy),"SEMAPRAX_VSCODE_SOURCE":str(source)})
        args=[str(code),f"--user-data-dir={user}",f"--extensions-dir={extensions}","--disable-extensions",
          "--disable-workspace-trust","--disable-gpu","--disable-updates","--skip-welcome","--skip-release-notes",
          f"--extensionDevelopmentPath={ROOT/'editors/vscode'}",
          f"--extensionTestsPath={ROOT/'editors/vscode/test/extension-host/index.js'}",str(workspace)]
        host_log=command(args,"VS Code Extension Host",env=env)
        matches=MARKER.findall(host_log)
        if len(matches)!=1: raise Failure("expected one Extension Host result")
        observation=json.loads(matches[0])
        expected_keys={"schema","vscode_version","app_name","extension_host_exec_path","extension_version","registered_commands","image_revision","candidate_revision","source_sha256","typed_intent","target","verified_virtual_diff","dirty_buffer_invalidated","source_bytes_unchanged"}
        if set(observation)!=expected_keys: raise Failure("unexpected Extension Host observation schema")
        if observation["schema"]!="semaprax.vscode-extension-host-result.v1" or observation["vscode_version"]!=cli_version[0] or observation["app_name"]!="Visual Studio Code" or observation["registered_commands"]!=26: raise Failure("Extension Host identity mismatch")
        for key in ("image_revision","candidate_revision"):
            if not re.fullmatch(r"sha256:[0-9a-f]{64}",observation[key]): raise Failure(f"invalid {key}")
        if not all(observation[k] is True for k in ("verified_virtual_diff","dirty_buffer_invalidated","source_bytes_unchanged")): raise Failure("host scenario incomplete")
        host_exec=Path(observation["extension_host_exec_path"]).resolve(strict=True)
        if app not in host_exec.parents: raise Failure("Extension Host executable is outside selected product")
        fixture_after={str(p.relative_to(workspace)):sha(p.read_bytes()) for p in sorted(workspace.rglob("*")) if p.is_file()}
        if fixture_after!=fixture_before: raise Failure("fixture bytes changed")
    clean()
    if git("rev-parse","HEAD^{commit}")!=commit or git("rev-parse","HEAD^{tree}")!=tree: raise Failure("repository subject drift")
    compiler_after=file_row(compiler)
    if compiler_after!=compiler_before: raise Failure("compiler binary drift")
    for recorded in inputs:
        if repo_row(recorded["path"]) != recorded: raise Failure(f"repository input drift: {recorded['path']}")
    for name,path in bound_paths.items():
        if file_row(path) != bound_rows[name]: raise Failure(f"tool or product drift: {path}")
    host_exec_row=file_row(host_exec)
    if file_row(host_exec) != host_exec_row: raise Failure(f"Extension Host executable drift: {host_exec}")
    logs={"controller-node.tap":node_log,"compiler-build-cargo.log":build_log,"vscode-extension-host.log":host_log,"vscode-host-observation.json":canonical(observation)}
    rows=[artifact(name,body) for name,body in logs.items()]
    domain=b"semaprax.graph-operational-vscode-host-execution-evidence.bundle.v1\0"
    bundle=hashlib.sha256(domain+b"".join(bytes.fromhex(row["sha256"][7:]) for row in rows)).hexdigest()
    default=ROOT/".semaprax/evidence/graph-operational-vscode-host"/commit/bundle
    destination=Path(ns.output).resolve() if ns.output else default
    ignored=run([shutil.which("git"),"check-ignore","-q",str(default)]).returncode==0
    evidence={"schema":SCHEMA,"bundle_id":bundle,
      "repository":{"commit":commit,"tree":tree,"current_head":True,"exact_tags":tags,"clean_before_and_after":True,"default_output_git_ignored":ignored,"inputs":inputs},
      "runner":{"path":"scripts/graph-operational-vscode-host-evidence.py","host":{"system":platform.system(),"machine":platform.machine()},"versions":versions,
        "tools":{"node":bound_rows["node"],"cargo":bound_rows["cargo"],"rustc":bound_rows["rustc"],"compiler":compiler_before},
        "vscode":{"app":str(app),"code":bound_rows["code"],"cli":bound_rows["cli"],"product":bound_rows["product"],"package":bound_rows["package"],"extension_host":host_exec_row}},
      "executions":[{"id":"vscode_node_mock_controllers_v1","passed":50,"failed":0,"ignored":0},{"id":"vscode_extension_host_real_compiler_v1","passed":1,"failed":0,"ignored":0}],
      "observation":observation,"artifacts":rows,
      "claims":{"selected_visual_studio_code_product_extension_host":"passed","actual_compiler_mcp_typed_intent_review_invalidation":"passed","source_bytes_unchanged":"passed","node_controllers_are_extension_host":"not_claimed","marketplace_or_vsix":"not_selected","hosted_or_cross_platform":"not_observed","full_quality_or_programme_completion":"not_selected","os_network_isolation":"not_claimed"}}
    if destination.exists(): raise Failure(f"destination exists: {destination}")
    stage=destination.parent/("."+destination.name+".tmp")
    if stage.exists(): shutil.rmtree(stage)
    stage.mkdir(parents=True)
    for name,body in logs.items(): (stage/name).write_bytes(body)
    (stage/"evidence.json").write_bytes(canonical(evidence))
    destination.parent.mkdir(parents=True,exist_ok=True); stage.rename(destination)
    clean(); print(destination); print(bundle)

if __name__=="__main__":
    try: main()
    except Failure as e: print(f"evidence failed: {e}",file=sys.stderr); raise SystemExit(1)
