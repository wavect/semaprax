use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    with_authenticated_project, ProjectNpmBuild, PROJECT_NPM_BUILD_SCHEMA_V10,
};

static SERIAL: AtomicU64 = AtomicU64::new(1);

fn configured_node() -> PathBuf {
    if let Some(path) = std::env::var_os("NODE").map(PathBuf::from) {
        assert!(path.is_absolute() && path.is_file(), "invalid NODE");
        return path;
    }
    [
        "/usr/bin/node",
        "/usr/local/bin/node",
        "/opt/homebrew/bin/node",
        r"C:\Program Files\nodejs\node.exe",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_absolute() && path.is_file())
    .expect("NODE must name an installed absolute Node executable")
}

const MANIFEST: &str = "schema = \"semaprax.project.v11\"\nname = \"nested-npm\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"nested.npm\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"nested.make\"]\ntests = [\"nested.tests\"]\n";

const SOURCE: &str = r#"module nested.npm;
@id("nested.payload") record Payload {
    @id("nested.payload.bytes") bytes: Bytes,
    @id("nested.payload.size") size: usize,
}
@id("nested.envelope") record Envelope {
    @id("nested.envelope.left") left: Payload,
    @id("nested.envelope.enabled") enabled: bool,
    @id("nested.envelope.right") right: Payload,
}
@id("nested.marker") fn marker() -> Bytes { let value = [120u8]; bytes_copy(array_as_slice(value)) }
@id("nested.make") fn make(input: borrow Slice<u8>, enabled: bool) -> Envelope {
    Envelope {
        left: Payload { bytes: bytes_copy(input), size: byte_len(input) },
        enabled: enabled,
        right: Payload { bytes: marker(), size: 1usize },
    }
}
@id("nested.main") fn main() -> i64 { 0 }
"#;

const TEST_SOURCE: &str =
    "module nested.tests;\n@id(\"nested.tests.main\") fn main() -> i64 { 0 }\n";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-nested-npm-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
        fs::write(
            root.join("src/app.spx"),
            semaprax::format::canonical(
                &semaprax::parse(SOURCE, Path::new("src/app.spx")).unwrap(),
            ),
        )
        .unwrap();
        fs::write(
            root.join("src/tests.spx"),
            semaprax::format::canonical(
                &semaprax::parse(TEST_SOURCE, Path::new("src/tests.spx")).unwrap(),
            ),
        )
        .unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn artifacts(build: &ProjectNpmBuild) -> Vec<(String, Vec<u8>)> {
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect()
}

#[test]
fn generated_v11_node_facade_is_atomic_bounded_and_reusable() {
    let fixture = Fixture::new();
    let manifest = fixture.0.join("semaprax.toml").canonicalize().unwrap();
    let build = with_authenticated_project(&manifest, |snapshot| {
        snapshot.build_npm_inline(40 * 1024 * 1024)
    })
    .unwrap();
    build.verify().unwrap();
    let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    assert_eq!(envelope["schema"], PROJECT_NPM_BUILD_SCHEMA_V10);

    let package = fixture.0.join("package");
    fs::create_dir(&package).unwrap();
    for (path, bytes) in artifacts(&build) {
        fs::write(package.join(path), bytes).unwrap();
    }
    fs::write(
        package.join("contract.mjs"),
        r#"import assert from 'node:assert/strict';
import fs from 'node:fs';
import instantiate from './semaprax.bindings.js';
const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url)));
const api=await instantiate(wasm);
const call=(input,enabled)=>api.functions['nested.make'](input,enabled);
function assertResult(result,input,enabled){
  assert.equal(Object.getPrototypeOf(result),null);assert.equal(Object.isFrozen(result),true);
  assert.equal(Object.getPrototypeOf(result.spx_field_id_6e65737465642e656e76656c6f70652e6c656674),null);
  const left=result.spx_field_id_6e65737465642e656e76656c6f70652e6c656674;
  const right=result.spx_field_id_6e65737465642e656e76656c6f70652e7269676874;
  assert.equal(Object.isFrozen(left),true);assert.equal(Object.isFrozen(right),true);
  assert.deepEqual(left.spx_field_id_6e65737465642e7061796c6f61642e6279746573,input);
  assert.deepEqual(right.spx_field_id_6e65737465642e7061796c6f61642e6279746573,new Uint8Array([120]));
  assert.notEqual(left.spx_field_id_6e65737465642e7061796c6f61642e6279746573,input);
  assert.notEqual(left.spx_field_id_6e65737465642e7061796c6f61642e6279746573,right.spx_field_id_6e65737465642e7061796c6f61642e6279746573);
  assert.equal(left.spx_field_id_6e65737465642e7061796c6f61642e73697a65,BigInt(input.length));
  assert.equal(right.spx_field_id_6e65737465642e7061796c6f61642e73697a65,1n);
  assert.equal(result.spx_field_id_6e65737465642e656e76656c6f70652e656e61626c6564,enabled);
}
for(let i=0;i<4;i++){const input=new Uint8Array([i,0,255]);assertResult(call(input,(i&1)===0),input,(i&1)===0)}
const exact=new Uint8Array(65535);exact[0]=7;exact[65534]=9;assertResult(call(exact,true),exact,true);
let overflow;try{call(new Uint8Array(65536),false)}catch(error){overflow=error}
assert.ok(overflow instanceof Error,'cumulative +1 output must reject');
const recovered=new Uint8Array([21,22]);assertResult(call(recovered,false),recovered,false);
console.log('nested-owned-record-npm-ok');
"#,
    )
    .unwrap();
    let output = Command::new(configured_node())
        .arg("contract.mjs")
        .current_dir(&package)
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .output()
        .expect("Node is required by the Project-v11 executable conformance gate");
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(output.stdout, b"nested-owned-record-npm-ok\n");
    assert!(output.stderr.is_empty());
}
