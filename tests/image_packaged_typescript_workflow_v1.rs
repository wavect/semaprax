//! Provisioned installed-package evidence for the bounded TypeScript workflow SDK.
#![cfg(unix)]

use semaprax::image_transport::{GitCommitHost, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, CandidateGitAuthority, CandidateGitCommitMetadata,
    CandidateGitObject, CandidateGitObjectKind, CandidateGitRefUpdate, CandidateGitRepository,
    CandidateGitTarget, CandidateTestPolicy, ProjectRevision,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const PACKAGE: &str = "@semaprax/agent-workflow";
const VERSION: &str = "0.1.0";
const TARBALL: &str = "semaprax-agent-workflow-0.1.0.tgz";
const DEPENDENCY: &str = "file:../packed/semaprax-agent-workflow-0.1.0.tgz";
const BRANCH: &str = "refs/heads/review";
const PROJECT_FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
const PACKAGE_SOURCE_FILES: [&str; 6] = [
    "README.md",
    "dist/index.d.ts",
    "dist/index.js",
    "package.json",
    "src/index.ts",
    "tsconfig.json",
];
const PACKAGE_FILES: [&str; 4] = [
    "README.md",
    "dist/index.d.ts",
    "dist/index.js",
    "package.json",
];
static SERIAL: AtomicU64 = AtomicU64::new(0);

fn provisioned(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("provision {name}")));
    assert!(
        path.is_absolute() && path.is_file(),
        "{name} must be an absolute file"
    );
    path.canonicalize().unwrap()
}

fn node(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .current_dir(directory)
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH");
    command
}

fn success(command: &mut Command) -> Output {
    let output = command.output().expect("cannot invoke provisioned tool");
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn recursive_inventory(root: &Path) -> Vec<String> {
    fn walk(root: &Path, current: &Path, rows: &mut Vec<String>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let file_type = entry.file_type().unwrap();
            assert!(
                !file_type.is_symlink(),
                "package inventory contains a symlink"
            );
            if file_type.is_dir() {
                walk(root, &entry.path(), rows);
            } else {
                assert!(
                    file_type.is_file(),
                    "package inventory contains a special file"
                );
                rows.push(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut rows = Vec::new();
    walk(root, root, &mut rows);
    rows
}

fn copy_regular_tree(source: &Path, destination: &Path) {
    for relative in recursive_inventory(source) {
        let output = destination.join(&relative);
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::copy(source.join(relative), output).unwrap();
    }
}

fn npm_command(node_path: &Path, npm: &Path, root: &Path, directory: &Path) -> Command {
    let mut command = node(node_path, directory);
    for (key, _) in std::env::vars_os() {
        if key
            .as_encoded_bytes()
            .get(..11)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"npm_config_"))
        {
            command.env_remove(key);
        }
    }
    command
        .arg(npm)
        .args([
            "--offline",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "--workspaces=false",
        ])
        .arg("--userconfig")
        .arg(root.join("empty.npmrc"))
        .arg("--globalconfig")
        .arg(root.join("global.npmrc"))
        .arg("--cache")
        .arg(root.join("npm-cache"));
    command
}

fn sha256(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn assert_file_bytes(left: &Path, right: &Path) {
    let left_bytes = fs::read(left).unwrap();
    let right_bytes = fs::read(right).unwrap();
    assert!(
        left_bytes == right_bytes,
        "file bytes differ: {} {} != {} {}",
        left.display(),
        sha256(&left_bytes),
        right.display(),
        sha256(&right_bytes)
    );
}

struct Fixture {
    root: PathBuf,
    git: PathBuf,
    repository: PathBuf,
    base: String,
    revision: Arc<ProjectRevision>,
    original: BTreeMap<String, Vec<u8>>,
}

impl Fixture {
    fn new(git: PathBuf) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-packaged-workflow-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in PROJECT_FILES {
            fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let admitted = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        fs::write(
            root.join("semaprax.toml"),
            admitted.manifest().to_canonical_toml(),
        )
        .unwrap();
        for source in admitted.sources() {
            fs::write(root.join(source.path()), source.source()).unwrap();
        }
        let revision = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        let original = PROJECT_FILES
            .into_iter()
            .map(|path| (path.to_owned(), fs::read(root.join(path)).unwrap()))
            .collect();
        let repository = root.join("published.git");
        let output = Command::new(&git)
            .env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "-c",
                "init.templateDir=",
                "init",
                "--bare",
                "--object-format=sha256",
            ])
            .arg(&repository)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        fs::write(repository.join("config"), "[core]\nrepositoryformatversion = 1\nbare = true\n[extensions]\nobjectformat = sha256\n").unwrap();
        let mut fixture = Self {
            root,
            git,
            repository,
            base: String::new(),
            revision,
            original,
        };
        let sources = fixture.tree(
            fixture
                .revision
                .sources()
                .iter()
                .map(|source| {
                    (
                        "100644",
                        source.path().strip_prefix("src/").unwrap().to_owned(),
                        fixture.object("blob", source.source().as_bytes()),
                    )
                })
                .collect(),
        );
        let manifest = fixture.object(
            "blob",
            fixture.revision.manifest().to_canonical_toml().as_bytes(),
        );
        let keep = fixture.object("blob", b"unrelated executable entry\n");
        let tree = fixture.tree(vec![
            ("40000", "src".into(), sources),
            ("100644", "semaprax.toml".into(), manifest),
            ("100755", "keep.sh".into(), keep),
        ]);
        fixture.base = fixture.object(
            "commit",
            format!("tree {tree}\nauthor Host <host@example.invalid> 1 +0000\ncommitter Host <host@example.invalid> 1 +0000\n\nOriginal\n").as_bytes(),
        );
        fixture.run_git(&["update-ref", BRANCH, &fixture.base], &[]);
        fixture
    }

    fn manifest(&self) -> PathBuf {
        self.root.join("semaprax.toml")
    }

    fn run_git(&self, args: &[&str], input: &[u8]) -> Vec<u8> {
        let mut child = Command::new(&self.git)
            .env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.logAllRefUpdates=false",
            ])
            .arg(format!("--git-dir={}", self.repository.display()))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    fn object(&self, kind: &str, bytes: &[u8]) -> String {
        String::from_utf8(self.run_git(&["hash-object", "-w", "--stdin", "-t", kind], bytes))
            .unwrap()
            .trim_end()
            .to_owned()
    }

    fn tree(&self, mut entries: Vec<(&str, String, String)>) -> String {
        entries.sort_by_key(|(mode, name, _)| {
            format!("{name}{}", if *mode == "40000" { "/" } else { "\0" })
        });
        let mut bytes = Vec::new();
        for (mode, name, oid) in entries {
            bytes.extend_from_slice(format!("{mode} {name}\0").as_bytes());
            for index in (0..oid.len()).step_by(2) {
                bytes.push(u8::from_str_radix(&oid[index..index + 2], 16).unwrap());
            }
        }
        self.object("tree", &bytes)
    }

    fn head(&self) -> String {
        String::from_utf8(self.run_git(&["rev-parse", BRANCH], &[]))
            .unwrap()
            .trim_end()
            .to_owned()
    }

    fn unchanged(&self) {
        for (path, bytes) in &self.original {
            assert_eq!(fs::read(self.root.join(path)).unwrap(), *bytes, "{path}");
        }
        assert!(!self.root.join(".semaprax-workspace").exists());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct ProfileAuthority;

impl CandidateGitAuthority for ProfileAuthority {
    fn repository(&self) -> io::Result<CandidateGitRepository> {
        Ok(CandidateGitRepository {
            identity: "packaged-workflow-profile".into(),
            bare: true,
            sha256: true,
        })
    }
    fn read_ref(&mut self, _: &str) -> io::Result<Option<String>> {
        Err(io::Error::other("profile authority cannot read"))
    }
    fn read_object(&mut self, _: &str, _: usize) -> io::Result<CandidateGitObject> {
        Err(io::Error::other("profile authority cannot read"))
    }
    fn write_object(&mut self, _: CandidateGitObjectKind, _: &[u8], _: &str) -> io::Result<()> {
        Err(io::Error::other("profile authority cannot write"))
    }
    fn compare_and_swap_ref(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
    ) -> io::Result<CandidateGitRefUpdate> {
        Err(io::Error::other("profile authority cannot publish"))
    }
}

fn policy() -> VNextPolicy {
    VNextPolicy {
        candidate_prepare: true,
        diagnostics: true,
        test_policy: Some(CandidateTestPolicy::new(100_000, 65_536, 262_144).unwrap()),
        ..Default::default()
    }
}

struct GeneratedClient {
    source: String,
    contract_revision: String,
    profile_revision: String,
}

fn source_constant(source: &str, declaration: &str) -> String {
    let tail = source
        .split_once(declaration)
        .unwrap_or_else(|| panic!("generated client lacks {declaration}"))
        .1;
    let quoted = tail
        .split_once('"')
        .unwrap_or_else(|| panic!("generated client {declaration} is not a string"))
        .1;
    quoted.split_once('"').unwrap().0.to_owned()
}

fn generated_client(session: &mut VNextSession, publish: bool) -> GeneratedClient {
    let capabilities_request = json!({
        "jsonrpc":"2.0","id":"packaged-capabilities","method":"protocol/capabilities","params":{}
    })
    .to_string();
    let capabilities: Value = serde_json::from_slice(
        &session
            .handle_frame(capabilities_request.as_bytes())
            .unwrap(),
    )
    .unwrap();
    let capabilities = &capabilities["result"]["payload"];
    assert!(capabilities["capabilities"]
        .as_array()
        .unwrap()
        .contains(&json!("candidate_diagnostics")));
    assert_eq!(capabilities["test_policy"]["max_steps"], 100_000);
    assert_eq!(capabilities["source_authority"], publish);
    let request = json!({
        "jsonrpc":"2.0",
        "id":"packaged-workflow",
        "method":"protocol/client",
        "params":{"language":"typescript"}
    })
    .to_string();
    let response: Value =
        serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap();
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["payload"]["io"], false);
    let source = response["result"]["payload"]["source"]
        .as_str()
        .unwrap()
        .to_owned();
    GeneratedClient {
        contract_revision: source_constant(&source, "export const CLIENT_CONTRACT_REVISION = "),
        profile_revision: source_constant(
            &source,
            "export const EXPECTED_WORKFLOW_PROFILE_REVISION: string | null = ",
        ),
        source,
    }
}

fn generated_clients(fixture: &Fixture) -> (GeneratedClient, GeneratedClient) {
    let mut review = VNextSession::open(&fixture.manifest(), policy()).unwrap();
    let repository = ProfileAuthority.repository().unwrap();
    let target =
        CandidateGitTarget::new(&repository.identity, BRANCH, &"0".repeat(64), "").unwrap();
    let metadata = CandidateGitCommitMetadata::new(
        "Host",
        "host@example.invalid",
        2,
        "Reviewed signature evolution\n",
    )
    .unwrap();
    let mut host = GitCommitHost::new(
        &fixture.manifest(),
        target,
        metadata,
        Box::new(ProfileAuthority),
    )
    .unwrap();
    host.approve(&format!("sha256:{}", "0".repeat(64))).unwrap();
    let mut publish = VNextSession::open(&fixture.manifest(), policy())
        .unwrap()
        .with_git_commit_host(host)
        .unwrap();
    (
        generated_client(&mut review, false),
        generated_client(&mut publish, true),
    )
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn review_policy() -> Value {
    json!({
        "schema":"semaprax.workspace-host-policy.v1",
        "candidate_prepare":true,
        "diagnostics":true,
        "build_enabled":false,
        "test_policy":{
            "max_steps":100_000,
            "max_execution_bytes":65_536,
            "max_report_bytes":262_144
        },
        "git_commit":null
    })
}

fn publish_policy(fixture: &Fixture, candidate: &str) -> Value {
    json!({
        "schema":"semaprax.workspace-host-policy.v1",
        "candidate_prepare":true,
        "diagnostics":true,
        "build_enabled":false,
        "test_policy":{
            "max_steps":100_000,
            "max_execution_bytes":65_536,
            "max_report_bytes":262_144
        },
        "git_commit":{
            "git_executable":fixture.git,
            "repository":fixture.repository,
            "reference":BRANCH,
            "base_commit":fixture.base,
            "project_prefix":"",
            "author_name":"Host",
            "author_email":"host@example.invalid",
            "unix_seconds":2,
            "message":"Reviewed signature evolution\n",
            "max_commands":128,
            "timeout_ms":10_000,
            "approved_candidate_digest":candidate
        }
    })
}

const STRICT_CONSUMER: &str = r#"
import {runReview,runPublish,type FailureClassifier,type InspectPublication,type WorkflowHandoff,type WorkflowTransport} from '@semaprax/agent-workflow';
import * as review from './review-client.js';
import * as publish from './publish-client.js';
const classify:FailureClassifier=()=> 'semantic_review_rejection';
const transport:WorkflowTransport={sessionId:'typed-consumer',exchange:async(_frame:string)=>'{"jsonrpc":"2.0","id":"x","error":{"code":-32603,"message":"not executed"}}'};
const input={target:'calculator.add',parameters:[{from:'right',name:'rhs'},{from:'left',name:'lhs'},{name:'offset',type:'i64' as const,argument:{kind:'i64' as const,value:0}}],classifyFailure:classify};
void runReview(review,transport,input);
const inspect=Object.assign(async()=>true,{classifyFailure:classify}) as InspectPublication;
void runPublish(publish,{...transport,sessionId:'typed-publish'},{} as WorkflowHandoff,inspect);
"#;

const INSTALLED_RUNNER: &str = r#"
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {spawn,spawnSync} from 'node:child_process';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';
import {runReview,runPublish} from '@semaprax/agent-workflow';
import * as reviewCodec from './out/review-client.js';
import * as publishCodec from './out/publish-client.js';

class Session {
  constructor(id,compiler,manifest,policy) {
    this.sessionId=id; this.pending=[]; this.lines=[]; this.stderr='';
    this.child=spawn(compiler,['serve-workspace',manifest,policy],{stdio:['pipe','pipe','pipe']});
    this.child.stderr.setEncoding('utf8'); this.child.stderr.on('data',chunk=>this.stderr+=chunk);
    this.reader=createInterface({input:this.child.stdout,crlfDelay:Infinity});
    this.reader.on('line',line=>{const waiter=this.pending.shift();if(waiter)waiter.resolve(line);else this.lines.push(line);});
    this.child.on('error',error=>{for(const waiter of this.pending.splice(0))waiter.reject(error);});
    this.child.on('exit',code=>{if(code!==null&&code!==0)for(const waiter of this.pending.splice(0))waiter.reject(new Error(`workspace server ${code}: ${this.stderr}`));});
  }
  exchange(frame) {
    assert.ok(frame.endsWith('\n')); assert.equal(frame.slice(0,-1).includes('\n'),false);
    const ready=this.lines.shift(); if(ready!==undefined){this.child.stdin.write(frame);return Promise.resolve(ready);}
    const answer=new Promise((resolve,reject)=>this.pending.push({resolve,reject}));
    this.child.stdin.write(frame); return answer;
  }
  async close(){this.child.stdin.end();const code=await new Promise(resolve=>this.child.once('exit',resolve));assert.equal(code,0,this.stderr);}
}

async function raw(session,id,method,params={}) {
  const response=JSON.parse(await session.exchange(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n'));
  assert.equal(response.id,id); assert.equal(response.error,undefined,JSON.stringify(response)); return response.result.payload;
}
function generatedConstant(source,declaration) {
  const tail=source.split(declaration); assert.equal(tail.length,2,`missing ${declaration}`);
  const match=tail[1].match(/^"([^"]+)"/); assert.ok(match); return match[1];
}
async function bindLiveProfile(session,codec,publish) {
  const capabilities=await raw(session,'package-capabilities','protocol/capabilities');
  assert.ok(capabilities.capabilities.includes('candidate_diagnostics'));
  assert.equal(capabilities.test_policy.max_steps,100000); assert.equal(capabilities.source_authority,publish);
  const client=await raw(session,'package-client','protocol/client',{language:'typescript'});
  assert.equal(client.schema,'semaprax.image-agent-client.v5'); assert.equal(client.io,false);
  assert.equal(generatedConstant(client.source,'export const CLIENT_CONTRACT_REVISION = '),codec.CLIENT_CONTRACT_REVISION);
  assert.equal(generatedConstant(client.source,'export const EXPECTED_WORKFLOW_PROFILE_REVISION: string | null = '),codec.EXPECTED_WORKFLOW_PROFILE_REVISION);
}

const classify=({phase})=>phase==='review'?'semantic_review_rejection':'publish_precondition_rejection';
const parameters=[{from:'right',name:'rhs'},{from:'left',name:'lhs'},{name:'offset',type:'i64',argument:{kind:'i64',value:0}}];
const [action,compiler,manifest,policy,handoffPath,git,repository,reference,inspectionPath]=process.argv.slice(2);
if(action==='review') {
  const session=new Session('installed-review',compiler,manifest,policy);
  await bindLiveProfile(session,reviewCodec,false);
  const result=await runReview(reviewCodec,session,{target:'calculator.add',parameters,classifyFailure:classify});
  await session.close(); assert.equal(result.status,'ready'); assert.deepEqual(result.compilerRepairOptions,[]); assert.equal(result.blindRetry,false);
  const malformed=await runReview(reviewCodec,{sessionId:'malformed',exchange:()=>'{not-json'},{target:'calculator.add',parameters,classifyFailure:classify});
  assert.equal(malformed.status,'failure'); assert.equal(malformed.outcome,'transport_uncertain_no_publish_claim'); assert.deepEqual(malformed.compilerRepairOptions,[]); assert.deepEqual(malformed.transitionRepairOptions,[]); assert.equal(malformed.blindRetry,false);
  const rejectedSession=new Session('installed-structured-failure',compiler,manifest,policy);
  await bindLiveProfile(rejectedSession,reviewCodec,false);
  const rejected=await runReview(reviewCodec,rejectedSession,{target:'missing.function',parameters,classifyFailure:classify});
  await rejectedSession.close(); assert.equal(rejected.status,'failure'); assert.equal(rejected.outcome,'review_rejected'); assert.deepEqual(rejected.compilerRepairOptions,[]); assert.deepEqual(rejected.transitionRepairOptions,['start_new_review_with_different_intention']);
  process.stdout.write(JSON.stringify(result));
} else if(action==='publish'||action==='publish-loss') {
  const saved=JSON.parse(readFileSync(handoffPath,'utf8')); const handoff=saved.handoff??saved;
  const expected=JSON.parse(readFileSync(inspectionPath,'utf8'));
  const session=new Session('installed-publish',compiler,manifest,policy);
  await bindLiveProfile(session,publishCodec,true);
  const gitRun=(args,encoding)=>{
    const output=spawnSync(git,['-c','core.hooksPath=/dev/null','-c','core.logAllRefUpdates=false',`--git-dir=${repository}`,...args],{
      encoding,env:{GIT_CONFIG_NOSYSTEM:'1',GIT_CONFIG_GLOBAL:'/dev/null'}
    });
    assert.equal(output.status,0,output.stderr?.toString()); return output.stdout;
  };
  const inspect=Object.assign(async observation=>{
    const receipt=observation.receipt;
    const head=gitRun(['rev-parse',reference],'utf8').trim();
    assert.equal(receipt.reference,reference); assert.equal(receipt.previous_commit,expected.baseCommit);
    assert.equal(receipt.published_commit,head); assert.equal(receipt.git_object_format,'sha256');
    assert.equal(gitRun(['rev-parse',`${reference}^`],'utf8').trim(),expected.baseCommit);
    assert.equal(gitRun(['rev-parse',`${reference}^{tree}`],'utf8').trim(),receipt.tree);
    assert.deepEqual(gitRun(['ls-tree','-r','--name-only',reference],'utf8').trimEnd().split('\n'),expected.treePaths);
    assert.ok(gitRun(['show',`${reference}:semaprax.toml`]).equals(Buffer.from(expected.manifest,'utf8')));
    assert.ok(gitRun(['show',`${reference}:${expected.unrelated.path}`]).equals(Buffer.from(expected.unrelated.bytes,'utf8')));
    assert.equal(gitRun(['ls-tree',reference,expected.unrelated.path],'utf8'),`${expected.unrelated.mode} blob ${gitRun(['rev-parse',`${reference}:${expected.unrelated.path}`],'utf8').trim()}\t${expected.unrelated.path}\n`);
    const sourceReview=JSON.parse(handoff.sourceReview); const paths=[];
    for(const file of sourceReview.files){paths.push(file.path);assert.ok(gitRun(['show',`${reference}:${file.path}`]).equals(Buffer.from(file.candidate_source,'utf8')));}
    assert.deepEqual(receipt.updated_source_paths,paths);
    assert.equal(observation.candidateRevision,handoff.candidateRevision); return true;
  },{classifyFailure:classify});
  const transport=action==='publish-loss'?{sessionId:'installed-publish-loss',exchange:async frame=>{
    const response=await session.exchange(frame); if(JSON.parse(frame).method==='candidate/commit')throw new Error('injected result loss after real commit'); return response;
  }}:session;
  const result=await runPublish(publishCodec,transport,handoff,inspect);
  if(action==='publish-loss') {
    assert.equal(result.status,'failure'); assert.equal(result.outcome,'publication_uncertain'); assert.deepEqual(result.compilerRepairOptions,[]); assert.deepEqual(result.transitionRepairOptions,[]); assert.equal(result.commitInvoked,true); assert.equal(result.blindRetry,false);
    await session.close(); process.stdout.write(JSON.stringify(result)); process.exit(0);
  }
  assert.equal(result.status,'published',JSON.stringify(result)); assert.equal(result.commitCalls,1); assert.equal(result.blindRetry,false); assert.equal(result.inspected,true);
  const duplicate=publishCodec.request('duplicate','candidate/commit',{image_revision:handoff.imageRevision,candidate_revision:handoff.candidateRevision,approval_revision:'sha256:'+'0'.repeat(64)});
  const duplicateResponse=JSON.parse(await session.exchange(duplicate));
  assert.match(duplicateResponse.error.message,/SPX-G287/);
  await session.close(); process.stdout.write(JSON.stringify(result));
} else { throw new Error('unknown action'); }
"#;

fn run_installed(
    node_path: &Path,
    consumer: &Path,
    action: &str,
    fixture: &Fixture,
    policy: &Path,
    handoff: Option<&Path>,
) -> Value {
    let inspection = fixture
        .root
        .join(format!("package-inspection-{action}.json"));
    write_json(
        &inspection,
        &json!({
            "baseCommit": fixture.base,
            "manifest": String::from_utf8(fixture.original["semaprax.toml"].clone()).unwrap(),
            "treePaths": ["keep.sh", "semaprax.toml", "src/app.spx", "src/core.spx", "src/tests.spx"],
            "unrelated": {
                "path": "keep.sh",
                "mode": "100755",
                "bytes": "unrelated executable entry\n"
            }
        }),
    );
    let output = success(node(node_path, consumer).arg("installed-runner.mjs").args([
        action,
        env!("CARGO_BIN_EXE_semaprax"),
        fixture.manifest().to_str().unwrap(),
        policy.to_str().unwrap(),
        handoff.and_then(Path::to_str).unwrap_or("-"),
        fixture.git.to_str().unwrap(),
        fixture.repository.to_str().unwrap(),
        BRANCH,
        inspection.to_str().unwrap(),
    ]));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
#[ignore = "requires absolute NODE, NPM_CLI, TypeScript 5.8.3 TSC_CLI and SEMAPRAX_TEST_GIT; offline package installation only"]
fn installed_typescript_sdk_drives_review_and_separately_approved_publish() {
    let node_path = provisioned("NODE");
    let npm = provisioned("NPM_CLI");
    let tsc = provisioned("TSC_CLI");
    let git = provisioned("SEMAPRAX_TEST_GIT");
    let fixture = Fixture::new(git);
    let root = fixture.root.join("package-gate");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("empty.npmrc"), b"").unwrap();
    fs::write(root.join("global.npmrc"), b"").unwrap();

    let node_version = success(node(&node_path, &root).arg("--version"));
    let major: u64 = String::from_utf8(node_version.stdout)
        .unwrap()
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(major >= 22);
    let tsc_version = success(node(&node_path, &root).arg(&tsc).arg("--version"));
    assert_eq!(
        String::from_utf8(tsc_version.stdout).unwrap().trim(),
        "Version 5.8.3"
    );

    let production_source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("packages/semaprax-agent-workflow");
    assert_eq!(
        recursive_inventory(&production_source),
        PACKAGE_SOURCE_FILES
    );
    let production = root.join("package");
    copy_regular_tree(&production_source, &production);
    let manifest: Value =
        serde_json::from_slice(&fs::read(production.join("package.json")).unwrap()).unwrap();
    assert_eq!(manifest["name"], PACKAGE);
    assert_eq!(manifest["version"], VERSION);
    assert_eq!(manifest["type"], "module");
    assert_eq!(manifest["engines"]["node"], ">=22");
    assert!(manifest.get("scripts").is_none());
    assert!(manifest.get("dependencies").is_none());
    assert_eq!(manifest["exports"]["."]["import"], "./dist/index.js");
    assert_eq!(manifest["exports"]["."]["types"], "./dist/index.d.ts");
    let compiled = root.join("compiled-package");
    success(
        node(&node_path, &production)
            .arg(&tsc)
            .args(["-p", "tsconfig.json", "--outDir"])
            .arg(&compiled),
    );
    assert_eq!(recursive_inventory(&production), PACKAGE_SOURCE_FILES);
    assert_eq!(recursive_inventory(&compiled), ["index.d.ts", "index.js"]);
    assert_file_bytes(
        &compiled.join("index.js"),
        &production.join("dist/index.js"),
    );
    assert_file_bytes(
        &compiled.join("index.d.ts"),
        &production.join("dist/index.d.ts"),
    );

    let packed = root.join("packed");
    fs::create_dir(&packed).unwrap();
    let report = success(
        npm_command(&node_path, &npm, &root, &production)
            .args(["pack", "--json", "--pack-destination"])
            .arg(&packed),
    );
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    let rows = report.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], PACKAGE);
    assert_eq!(rows[0]["version"], VERSION);
    assert_eq!(rows[0]["filename"], TARBALL);
    let mut packed_files = rows[0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect::<Vec<_>>();
    packed_files.sort();
    assert_eq!(packed_files, PACKAGE_FILES);
    assert_eq!(recursive_inventory(&packed), [TARBALL]);
    let tarball = fs::read(packed.join(TARBALL)).unwrap();
    let hashes = success(
        node(&node_path, &root)
            .args(["--input-type=module", "--eval", "import{readFileSync}from'node:fs';import{createHash}from'node:crypto';const b=readFileSync(process.argv[1]);console.log(JSON.stringify({sha256:'sha256:'+createHash('sha256').update(b).digest('hex'),integrity:'sha512-'+createHash('sha512').update(b).digest('base64')}));" ])
            .arg(packed.join(TARBALL)),
    );
    let hashes: Value = serde_json::from_slice(&hashes.stdout).unwrap();
    assert_eq!(hashes["sha256"], sha256(&tarball));
    let integrity = hashes["integrity"].as_str().unwrap().to_owned();
    assert_eq!(rows[0]["integrity"], integrity);

    let consumer = root.join("consumer");
    fs::create_dir(&consumer).unwrap();
    let consumer_manifest = json!({
        "name":"semaprax-workflow-consumer",
        "version":"1.0.0",
        "private":true,
        "type":"module",
        "dependencies":{PACKAGE:DEPENDENCY}
    });
    fs::write(
        consumer.join("package.json"),
        serde_json::to_vec(&consumer_manifest).unwrap(),
    )
    .unwrap();
    success(npm_command(&node_path, &npm, &root, &consumer).args([
        "install",
        "--package-lock-only",
        "--lockfile-version=3",
    ]));
    assert!(!consumer.join("node_modules").exists());
    let lock = fs::read(consumer.join("package-lock.json")).unwrap();
    let lock_value: Value = serde_json::from_slice(&lock).unwrap();
    assert_eq!(lock_value["lockfileVersion"], 3);
    let packages = lock_value["packages"].as_object().unwrap();
    assert_eq!(packages.len(), 2);
    assert_eq!(
        packages[""]["dependencies"],
        consumer_manifest["dependencies"]
    );
    let installed_lock = &packages["node_modules/@semaprax/agent-workflow"];
    assert_eq!(installed_lock["version"], VERSION);
    assert_eq!(installed_lock["resolved"], DEPENDENCY);
    assert_eq!(installed_lock["integrity"], integrity);
    for row in packages.values() {
        for key in [
            "link",
            "optionalDependencies",
            "peerDependencies",
            "devDependencies",
        ] {
            assert!(row.get(key).is_none(), "unexpected lockfile key {key}");
        }
    }
    assert!(installed_lock.get("dependencies").is_none());
    success(npm_command(&node_path, &npm, &root, &consumer).arg("ci"));
    assert_eq!(fs::read(consumer.join("package-lock.json")).unwrap(), lock);
    let installed = consumer.join("node_modules/@semaprax/agent-workflow");
    assert!(fs::symlink_metadata(&installed)
        .unwrap()
        .file_type()
        .is_dir());
    assert_eq!(recursive_inventory(&installed), PACKAGE_FILES);
    for path in PACKAGE_FILES {
        assert_file_bytes(&installed.join(path), &production.join(path));
    }

    let (review, publish) = generated_clients(&fixture);
    fs::write(consumer.join("review-client.ts"), &review.source).unwrap();
    fs::write(consumer.join("publish-client.ts"), &publish.source).unwrap();
    fs::write(consumer.join("consumer.ts"), STRICT_CONSUMER).unwrap();
    fs::write(consumer.join("installed-runner.mjs"), INSTALLED_RUNNER).unwrap();
    let compiled_clients = consumer.join("out");
    let types = success(
        node(&node_path, &consumer)
            .arg(&tsc)
            .args([
                "--strict",
                "--noEmitOnError",
                "--target",
                "ES2023",
                "--module",
                "NodeNext",
                "--moduleResolution",
                "NodeNext",
                "--outDir",
            ])
            .arg(&compiled_clients)
            .args(["review-client.ts", "publish-client.ts", "consumer.ts"]),
    );
    assert!(types.stdout.is_empty());
    assert!(types.stderr.is_empty());
    let resolved = success(node(&node_path, &consumer).args([
        "--input-type=module",
        "--eval",
        "import{realpathSync}from'node:fs';import{fileURLToPath}from'node:url';process.stdout.write(fileURLToPath(import.meta.resolve('@semaprax/agent-workflow')));",
    ]));
    assert_eq!(
        PathBuf::from(String::from_utf8(resolved.stdout).unwrap()),
        installed.join("dist/index.js").canonicalize().unwrap()
    );

    let review_policy_path = root.join("review-policy.json");
    write_json(&review_policy_path, &review_policy());
    let reviewed = run_installed(
        &node_path,
        &consumer,
        "review",
        &fixture,
        &review_policy_path,
        None,
    );
    assert_eq!(reviewed["status"], "ready");
    assert_eq!(
        reviewed["handoff"]["reviewClientContractRevision"],
        review.contract_revision
    );
    assert_eq!(
        reviewed["handoff"]["reviewProfileRevision"],
        review.profile_revision
    );
    assert_eq!(reviewed["handoff"]["compilerRepairOptions"], json!([]));
    for field in [
        "reviewClientContractRevision",
        "reviewProfileRevision",
        "handoffSha256",
        "candidateRevision",
    ] {
        assert!(reviewed["handoff"][field]
            .as_str()
            .is_some_and(|value| value.starts_with("sha256:") && value.len() == 71));
    }
    fixture.unchanged();
    assert_eq!(fixture.head(), fixture.base);
    let handoff_path = root.join("handoff.json");
    // Cross a non-JavaScript serialization boundary before publication. The
    // handoff digest is over sorted canonical JSON, not object insertion order.
    write_json(&handoff_path, &reviewed["handoff"]);
    let publish_policy_path = root.join("publish-policy.json");
    write_json(
        &publish_policy_path,
        &publish_policy(
            &fixture,
            reviewed["handoff"]["candidateRevision"].as_str().unwrap(),
        ),
    );
    let published = run_installed(
        &node_path,
        &consumer,
        "publish",
        &fixture,
        &publish_policy_path,
        Some(&handoff_path),
    );
    assert_eq!(published["status"], "published");
    assert_eq!(
        published["publishClientContractRevision"],
        publish.contract_revision
    );
    assert_eq!(
        published["publishProfileRevision"],
        publish.profile_revision
    );
    assert_eq!(published["commitCalls"], 1);
    assert_eq!(
        published["candidateRevision"],
        reviewed["handoff"]["candidateRevision"]
    );
    assert_eq!(
        published["receipt"]["approved_candidate_digest"],
        reviewed["handoff"]["candidateRevision"]
    );
    let head = fixture.head();
    let receipt = &published["receipt"];
    assert_eq!(receipt["previous_commit"], fixture.base);
    assert_eq!(receipt["published_commit"], head);
    assert_eq!(receipt["reference"], BRANCH);
    assert_eq!(receipt["git_object_format"], "sha256");
    assert_eq!(
        String::from_utf8(fixture.run_git(&["rev-parse", &format!("{BRANCH}^")], &[]))
            .unwrap()
            .trim_end(),
        fixture.base
    );
    let tree_spec = format!("{BRANCH}^{{tree}}");
    assert_eq!(
        String::from_utf8(fixture.run_git(&["rev-parse", &tree_spec], &[]))
            .unwrap()
            .trim_end(),
        receipt["tree"].as_str().unwrap()
    );
    assert_eq!(
        fixture.run_git(&["show", &format!("{BRANCH}:semaprax.toml")], &[]),
        fixture.original["semaprax.toml"]
    );
    assert_eq!(
        fixture.run_git(&["show", &format!("{BRANCH}:keep.sh")], &[]),
        b"unrelated executable entry\n"
    );
    let keep = String::from_utf8(fixture.run_git(&["ls-tree", BRANCH, "keep.sh"], &[])).unwrap();
    assert!(keep.starts_with("100755 blob ") && keep.ends_with("\tkeep.sh\n"));
    let source_review: Value =
        serde_json::from_str(reviewed["handoff"]["sourceReview"].as_str().unwrap()).unwrap();
    let mut reviewed_paths = Vec::new();
    for file in source_review["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        reviewed_paths.push(json!(path));
        assert_eq!(
            fixture.run_git(&["show", &format!("{BRANCH}:{path}")], &[]),
            file["candidate_source"].as_str().unwrap().as_bytes()
        );
    }
    assert_eq!(
        receipt["updated_source_paths"],
        Value::Array(reviewed_paths)
    );
    fixture.unchanged();

    // Drop the real commit response after the fixed ref has moved. The
    // installed driver must report terminal uncertainty and forbid blind retry.
    let loss_fixture = Fixture::new(fixture.git.clone());
    let loss_reviewed = run_installed(
        &node_path,
        &consumer,
        "review",
        &loss_fixture,
        &review_policy_path,
        None,
    );
    let loss_handoff_path = root.join("loss-handoff.json");
    write_json(&loss_handoff_path, &loss_reviewed["handoff"]);
    let loss_policy_path = root.join("loss-publish-policy.json");
    write_json(
        &loss_policy_path,
        &publish_policy(
            &loss_fixture,
            loss_reviewed["handoff"]["candidateRevision"]
                .as_str()
                .unwrap(),
        ),
    );
    let lost = run_installed(
        &node_path,
        &consumer,
        "publish-loss",
        &loss_fixture,
        &loss_policy_path,
        Some(&loss_handoff_path),
    );
    assert_eq!(lost["status"], "failure");
    assert_eq!(lost["outcome"], "publication_uncertain");
    assert_eq!(lost["commitInvoked"], true);
    assert_eq!(lost["blindRetry"], false);
    assert_ne!(loss_fixture.head(), loss_fixture.base);
    assert_eq!(
        String::from_utf8(loss_fixture.run_git(&["rev-parse", &format!("{BRANCH}^")], &[]))
            .unwrap()
            .trim_end(),
        loss_fixture.base
    );
    loss_fixture.unchanged();
}
