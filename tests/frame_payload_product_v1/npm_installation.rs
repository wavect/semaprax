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
    let npm_command = |directory: &Path| {
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
            .arg(root.join("npm-cache"));
        command
    };
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

        let packed = case.join("packed");
        fs::create_dir(&packed).unwrap();
        let report = success(
            npm_command(&package)
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
