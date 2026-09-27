//! Provisioned installed-package evidence, not a registry publication or download.
use super::*;
use serde_json::{json, Value};
use std::process::Output;

const TARBALL: &str = "frame-payload-0.1.0.tgz";
const DEPENDENCY: &str = "file:../packed/frame-payload-0.1.0.tgz";
const FILES: [&str; 6] = [
    "app.wasm",
    "package.json",
    "semaprax.api.json",
    "semaprax.bindings.d.ts",
    "semaprax.bindings.js",
    "semaprax.js",
];

fn tool(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("provision {name}")));
    assert!(
        path.is_absolute() && path.is_file(),
        "{name} must be an absolute file"
    );
    path
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
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn release_preview_command() -> Command {
    let mut command = Command::new("python3");
    command.arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/generated-package-release.py"));
    command
}

fn preview_tamper_is_refused(prepared: &Path, payload: &str) {
    let path = prepared.join("payload").join(payload);
    let original = fs::read(&path).unwrap();
    let mut altered = original.clone();
    altered[0] ^= 1;
    fs::write(&path, altered).unwrap();
    let output = release_preview_command()
        .args(["check", "--kind", "npm", "--prepared-dir"])
        .arg(prepared)
        .output()
        .unwrap();
    fs::write(&path, original).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("on-disk file digests disagree"),
        "tampered preview must be refused before package use: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// Build a hostile copy of a genuine packed npm tarball by rewriting exactly
// one payload member (`app.wasm`) of the real, compiler-produced archive:
// either its path (path traversal) or its content (a swapped/substituted
// Wasm binary). Every other member is carried over byte-for-byte from the
// real archive, so these mutations exercise the guards against the genuine
// artifact, not a synthetic fixture.
const NPM_TARBALL_MUTATION: &str = r#"
import gzip, io, sys, tarfile
from pathlib import Path

MODE, TARBALL, OUTPUT = sys.argv[1:4]

with tarfile.open(TARBALL, mode="r:gz") as source:
    members = source.getmembers()
    contents = {
        member.name: source.extractfile(member).read() if member.isfile() else None
        for member in members
    }

buffer = io.BytesIO()
with tarfile.open(fileobj=buffer, mode="w", format=tarfile.USTAR_FORMAT) as archive:
    for member in members:
        data = contents[member.name]
        if member.isfile() and member.name == "package/app.wasm":
            if MODE == "traversal":
                member = tarfile.TarInfo("package/../app.wasm")
                member.size = len(data)
            elif MODE == "substitute":
                data = bytes([data[0] ^ 0xFF]) + data[1:]
            else:
                raise ValueError("unknown MODE: " + MODE)
        archive.addfile(member, io.BytesIO(data) if data is not None else None)
Path(OUTPUT).write_bytes(gzip.compress(buffer.getvalue(), mtime=0))
"#;

fn hostile_npm_tarball(mode: &str, tarball: &Path, output: &Path) {
    let result = Command::new("python3")
        .args(["-c", NPM_TARBALL_MUTATION, mode])
        .arg(tarball)
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "cannot build hostile {mode} tarball: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

// Reuse the release-preparation script's own archive byte-binding guard
// (`_verify_npm_tarball_payload`) directly, rather than reimplementing
// archive-member admission, against a hostile tarball whose `app.wasm`
// member path escapes the package root.
const NPM_TARBALL_PAYLOAD_GUARD: &str = r#"
import importlib.util, sys
from pathlib import Path

TARBALL, PACKAGE_DIR, RELEASE = sys.argv[1:]
spec = importlib.util.spec_from_file_location("release", RELEASE)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)

expected = {path.name: path.read_bytes() for path in Path(PACKAGE_DIR).iterdir()}

try:
    release._verify_npm_tarball_payload(Path(TARBALL), expected)
except release.Rejected as error:
    print(f"refused[traversal]: {error}")
else:
    print("ERROR: accepted a path-traversing archive member", file=sys.stderr)
    sys.exit(1)
"#;

fn npm_tarball_payload_guard(tarball: &Path, package_dir: &Path) -> Output {
    Command::new("python3")
        .args(["-c", NPM_TARBALL_PAYLOAD_GUARD])
        .arg(tarball)
        .arg(package_dir)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/generated-package-release.py"))
        .output()
        .unwrap()
}

fn inventory(path: &Path) -> Vec<String> {
    let mut files = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            entry.file_name().into_string().unwrap()
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[test]
#[ignore = "requires provisioned NODE, NPM_CLI and TypeScript 5.8.3 TSC_CLI; offline only"]
fn installed_owned_npm_package_resolves_and_runs_without_compiler() {
    let executable = tool("NODE");
    let npm = tool("NPM_CLI");
    let tsc = tool("TSC_CLI");
    let root = temporary("installed-owned-npm");
    fs::create_dir(&root).unwrap();
    let version = success(node(&executable, &root).arg(&tsc).arg("--version"));
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        "Version 5.8.3"
    );
    // Remove environment configuration as well as replacing user/global files;
    // these changes apply only to each child, never the test process.
    let config = root.join("empty.npmrc");
    fs::write(&config, b"").unwrap();
    let global_config = root.join("global.npmrc");
    fs::write(&global_config, b"").unwrap();
    let npm_command_with_cache = |directory: &Path, cache: &Path| {
        let mut command = node(&executable, directory);
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
            .arg(&npm)
            .args([
                "--offline",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                "--workspaces=false",
            ])
            .arg("--userconfig")
            .arg(&config)
            .arg("--globalconfig")
            .arg(&global_config)
            .arg("--cache")
            .arg(cache);
        command
    };
    let npm_command = |directory: &Path| npm_command_with_cache(directory, &root.join("npm-cache"));
    for renamed in [false, true] {
        let case = root.join(if renamed { "renamed" } else { "baseline" });
        fs::create_dir(&case).unwrap();
        let project = case.join("project");
        copy_project(&project, renamed);
        let manifest = project.join("semaprax.toml");
        let inline = semaprax::project::with_authenticated_project(&manifest, |snapshot| {
            snapshot.build_npm_inline(semaprax::project::MAX_PROJECT_NPM_BUILD_BYTES)
        })
        .unwrap();
        inline.verify().unwrap();
        let expected = artifacts(&inline);
        let package = case.join("package");
        build(
            Path::new(full_toolchain::binary()),
            &manifest,
            "npm",
            &package,
        );
        assert_eq!(inventory(&package), FILES);
        for (name, bytes) in &expected {
            assert_eq!(fs::read(package.join(name)).unwrap(), *bytes);
        }

        // Reuse the hardened preview boundary with the actual compiler
        // output. Both the script's packed-tarball consumer and the richer
        // corpus/type consumer below use only its exact byte snapshot.
        let preview = case.join("preview");
        success(
            release_preview_command()
                .args(["prepare", "--kind", "npm", "--package-dir"])
                .arg(&package)
                .args([
                    "--project-name",
                    "frame-payload",
                    "--project-version",
                    "0.1.0",
                    "--output",
                ])
                .arg(&preview),
        );
        preview_tamper_is_refused(&preview, "semaprax.bindings.js");
        // A swapped Wasm binary is a byte tamper like any other payload
        // member; the same digest-bound preview guard must catch it before
        // the compiler-produced `app.wasm` is ever packed or shipped.
        preview_tamper_is_refused(&preview, "app.wasm");
        success(
            release_preview_command()
                .args(["check", "--kind", "npm", "--prepared-dir"])
                .arg(&preview)
                .args(["--npm-bin"])
                .arg(&npm)
                .args(["--npm-tarball-consumer", "--node-bin"])
                .arg(&executable),
        );
        let preview_payload = preview.join("payload");

        let packed = case.join("packed");
        fs::create_dir(&packed).unwrap();
        let report = success(
            npm_command(&preview_payload)
                .args(["pack", "--json", "--pack-destination"])
                .arg(&packed),
        );
        let report: Value = serde_json::from_slice(&report.stdout).unwrap();
        assert_eq!(report.as_array().unwrap().len(), 1);
        let report = &report[0];
        assert_eq!(report["name"], "frame-payload");
        assert_eq!(report["version"], "0.1.0");
        assert_eq!(report["filename"], TARBALL);
        let mut names = report["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["path"].as_str().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, FILES);
        assert_eq!(inventory(&packed), [TARBALL]);
        let integrity = success(node(&executable, &case).args(["--input-type=module", "--eval",
            "import{readFileSync}from'node:fs';import{createHash}from'node:crypto';console.log('sha512-'+createHash('sha512').update(readFileSync(process.argv[1])).digest('base64'));"
        ]).arg(packed.join(TARBALL)));
        let integrity = String::from_utf8(integrity.stdout)
            .unwrap()
            .trim()
            .to_owned();
        assert_eq!(report["integrity"], integrity);

        let consumer = case.join("consumer");
        fs::create_dir(&consumer).unwrap();
        let package_json = json!({"name":"owned-install-consumer","version":"1.0.0","private":true,"type":"module","dependencies":{"frame-payload":DEPENDENCY}});
        fs::write(
            consumer.join("package.json"),
            serde_json::to_vec(&package_json).unwrap(),
        )
        .unwrap();
        success(npm_command(&consumer).args([
            "install",
            "--package-lock-only",
            "--lockfile-version=3",
        ]));
        assert!(!consumer.join("node_modules").exists());
        let lock = fs::read(consumer.join("package-lock.json")).unwrap();
        let parsed: Value = serde_json::from_slice(&lock).unwrap();
        assert_eq!(parsed["lockfileVersion"], 3);
        let packages = parsed["packages"].as_object().unwrap();
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[""]["dependencies"], package_json["dependencies"]);
        let installed = &packages["node_modules/frame-payload"];
        assert_eq!(installed["version"], "0.1.0");
        assert_eq!(installed["resolved"], DEPENDENCY);
        assert_eq!(installed["integrity"], integrity);
        for row in packages.values() {
            assert!(row.get("link").is_none());
            for key in [
                "optionalDependencies",
                "peerDependencies",
                "devDependencies",
            ] {
                assert!(row.get(key).is_none());
            }
        }
        assert!(installed.get("dependencies").is_none());

        let tarball_path = packed.join(TARBALL);

        // Hostile guard: a packed archive whose member path escapes the
        // package root (path traversal) must be refused by the same
        // byte-binding verification the release-preparation script already
        // applies to every packed npm tarball, exercised here against a
        // hostile derivative of the genuine compiler-produced archive
        // rather than a synthetic fixture.
        let hostile_traversal = case.join("hostile-traversal.tgz");
        hostile_npm_tarball("traversal", &tarball_path, &hostile_traversal);
        let traversal = npm_tarball_payload_guard(&hostile_traversal, &package);
        assert!(
            traversal.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&traversal.stdout),
            String::from_utf8_lossy(&traversal.stderr)
        );
        assert!(
            String::from_utf8_lossy(&traversal.stdout).contains("refused[traversal]"),
            "a path-traversing archive member must be refused: {}",
            String::from_utf8_lossy(&traversal.stdout)
        );

        // Hostile guard: a tarball substituted after the lockfile recorded
        // its integrity (a stale lock pointed at swapped/substituted bytes
        // -- here, a hostile derivative of the genuine tarball with its
        // `app.wasm` member content changed) must be refused by npm's own
        // subresource-integrity check before any installed byte is trusted.
        // A cache already populated from the earlier lock-only install of
        // the genuine tarball can otherwise mask a substituted `file:`
        // dependency (reproduced separately against this exact npm), so
        // this check uses a cache the genuine artifact has never touched.
        let hostile_substitute = case.join("hostile-substitute.tgz");
        hostile_npm_tarball("substitute", &tarball_path, &hostile_substitute);
        let original_tarball = fs::read(&tarball_path).unwrap();
        fs::copy(&hostile_substitute, &tarball_path).unwrap();
        let hostile_cache = case.join("npm-cache-hostile");
        fs::create_dir(&hostile_cache).unwrap();
        let stale = npm_command_with_cache(&consumer, &hostile_cache)
            .arg("ci")
            .output()
            .unwrap();
        fs::write(&tarball_path, &original_tarball).unwrap();
        assert!(
            !stale.status.success(),
            "a tarball substituted after the lockfile recorded its integrity must be refused"
        );
        assert!(
            String::from_utf8_lossy(&stale.stderr).contains("EINTEGRITY"),
            "stale-lock refusal must name the integrity mismatch: {}",
            String::from_utf8_lossy(&stale.stderr)
        );
        assert!(!consumer.join("node_modules").exists());

        success(npm_command(&consumer).arg("ci"));
        assert_eq!(fs::read(consumer.join("package-lock.json")).unwrap(), lock);
        let installed = consumer.join("node_modules/frame-payload");
        assert!(fs::symlink_metadata(&installed)
            .unwrap()
            .file_type()
            .is_dir());
        assert_eq!(inventory(&installed), FILES);
        for (name, bytes) in &expected {
            assert_eq!(fs::read(installed.join(name)).unwrap(), *bytes);
        }

        fs::write(consumer.join("corpus.json"), CORPUS).unwrap();
        fs::write(consumer.join("adversarial.json"), adversarial::CORPUS).unwrap();
        fs::write(
            consumer.join("corpus-runner.mjs"),
            include_bytes!("../../examples/frame-payload-web/corpus-runner.mjs"),
        )
        .unwrap();
        fs::write(consumer.join("consumer.mjs"), r#"
import assert from 'node:assert/strict';
import {readFileSync,realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import instantiate from 'frame-payload';
import {runCorpus} from './corpus-runner.mjs';
for(const [specifier,file] of [['frame-payload','semaprax.bindings.js'],['frame-payload/app.wasm','app.wasm'],['frame-payload/manifest','semaprax.api.json']]) {
  assert.equal(fileURLToPath(import.meta.resolve(specifier)),realpathSync(`./node_modules/frame-payload/${file}`));
}
const wasm=new Uint8Array(readFileSync(new URL(import.meta.resolve('frame-payload/app.wasm'))));
const api=await instantiate(wasm);
for(const [file,count] of [['corpus.json',9],['adversarial.json',72]]) {
  const result=runCorpus(api,JSON.parse(readFileSync(new URL(file,import.meta.url),'utf8')));
  assert.equal(result.cases,count);
}
console.log('installed-owned-npm-ok');
"#).unwrap();
        let run = success(node(&executable, &consumer).arg("consumer.mjs"));
        assert_eq!(run.stdout, b"installed-owned-npm-ok\n");
        assert!(run.stderr.is_empty());
        let types = include_str!("../../examples/frame-payload-web/consumer.ts");
        assert_eq!(types.matches("./generated/semaprax.bindings.js").count(), 1);
        fs::write(
            consumer.join("consumer.ts"),
            types.replace("./generated/semaprax.bindings.js", "frame-payload"),
        )
        .unwrap();
        let compile = |file: &str| {
            node(&executable, &consumer)
                .arg(&tsc)
                .args([
                    "--strict",
                    "--noEmit",
                    "--pretty",
                    "false",
                    "--target",
                    "ES2022",
                    "--module",
                    "NodeNext",
                    "--moduleResolution",
                    "NodeNext",
                    file,
                ])
                .output()
                .unwrap()
        };
        let positive = compile("consumer.ts");
        assert!(
            positive.status.success(),
            "{}{}",
            String::from_utf8_lossy(&positive.stdout),
            String::from_utf8_lossy(&positive.stderr)
        );
        for (name, statement, code) in [
            ("wrong-argument.ts", "api.functions['frame.payload'](1);", "TS2345"),
            ("unguarded-result.ts", "const result=api.functions['frame.payload-result'](new Uint8Array());result.value;", "TS2339"),
        ] {
            fs::write(consumer.join(name), format!("import {{instantiate}} from 'frame-payload';const api=await instantiate(new Uint8Array());{statement}\n")).unwrap();
            let output = compile(name);
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stdout).contains(code));
        }
        assert_eq!(fs::read(consumer.join("package-lock.json")).unwrap(), lock);
        for (name, bytes) in &expected {
            assert_eq!(fs::read(installed.join(name)).unwrap(), *bytes);
        }
    }
    // Retain the exclusively created packages, npm cache and lockfiles, including
    // failed fixtures; no recursive deletion of package-manager output.
    eprintln!(
        "retained offline npm installation evidence: {}",
        root.display()
    );
}
