//! Recursive repair response clients; authored, never executed during this change.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const REPORT: &str = "urn:semaprax.project-candidate-repair-catalog.v1";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-recursive-repair-client-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "recursive-repair-client"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "recursive.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["recursive.public"]
tests = ["recursive.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module recursive.core;
@id("recursive.packet") record Packet { @id("recursive.packet.bytes") bytes:Bytes, }
@id("recursive.make") fn make(input:borrow Slice<u8>)->Packet {Packet {bytes:bytes_copy(input)}}
@id("recursive.inspect") fn inspect(packet:own Packet)->usize {byte_len(bytes_as_slice(packet.bytes))}
@id("recursive.identity") fn identity(value:usize)->usize {value}
@id("recursive.public") fn public_value(value:i64)->i64 {value}
@id("recursive.evaluate") fn evaluate()->i64 {let input=[7u8];if inspect(make(array_as_slice(input)))==1usize {42}else{0}}
"#,
            ),
            (
                "src/app.spx",
                r#"module recursive.app;
use function @id("recursive.evaluate") from recursive.core as evaluate;
@id("recursive.main") fn main()->i64 {evaluate()}
"#,
            ),
            (
                "src/tests.spx",
                r#"module recursive.tests;
use function @id("recursive.evaluate") from recursive.core as evaluate;
@id("recursive.test") fn main()->i64 {if evaluate()==42 {0}else{1}}
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn session(&self, diagnostics: bool) -> VNextSession {
        VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: true,
                diagnostics,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|p| std::fs::read(self.0.join(p)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let request =
        json!({"jsonrpc":"2.0","id":"recursive-evidence","method":method,"params":params});
    let response: Value = serde_json::from_slice(
        &session
            .handle_frame(request.to_string().as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert!(response.get("error").is_none(), "{method}: {response}");
    response
}
fn payload(response: &Value) -> &Value {
    &response["result"]["payload"]
}
fn catalog(session: &mut VNextSession, candidate: &str, target: &str, body: Value) -> Value {
    let image = session.image_revision().to_owned();
    let attempt = call(
        session,
        "candidate/attempt",
        json!({"image_revision":image,"candidate_revision":candidate,"intent":{"kind":"replace_function_body","target":target,"body":body}}),
    );
    assert_eq!(payload(&attempt)["status"], "rejected");
    assert!(payload(&attempt)["candidate"].is_null());
    call(
        session,
        "attempt/repair-catalog",
        json!({"image_revision":image,"attempt_revision":payload(&attempt)["attempt"]["attempt_revision"]}),
    )
}
fn responses(session: &mut VNextSession) -> Value {
    let image = session.image_revision().to_owned();
    let candidate = call(session, "candidate/open", json!({"image_revision":image}));
    let candidate = payload(&candidate)["candidate_revision"].as_str().unwrap();
    let literal = catalog(
        session,
        candidate,
        "recursive.public",
        json!({"kind":"i32","value":42}),
    );
    let borrow = catalog(
        session,
        candidate,
        "recursive.inspect",
        json!({"kind":"builtin_call","target":"core.bytes.len","arguments":[{"kind":"builtin_call","target":"core.bytes.as-slice","arguments":[{"kind":"project","target":"recursive.packet.bytes","base":{"kind":"place","name":"packet"}}]}]}),
    );
    let empty = catalog(
        session,
        candidate,
        "recursive.public",
        json!({"kind":"bool","value":true}),
    );
    assert_eq!(
        payload(&literal)["repairs"][0]["class"],
        "retag_integer_literal_to_retained_return_type"
    );
    assert_eq!(
        payload(&borrow)["repairs"][0]["class"],
        "borrow_owned_byte_field_without_staging"
    );
    assert!(payload(&empty)["repairs"].as_array().unwrap().is_empty());
    json!({"literal":literal,"borrow":borrow,"empty":empty})
}

#[test]
fn full_repair_report_is_bundled_only_with_selected_diagnostics_and_all_clients_are_deterministic()
{
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    for selected in [false, true] {
        let mut session = fixture.session(selected);
        let schemas = call(&mut session, "protocol/schemas", json!({}));
        let schemas = payload(&schemas);
        let document = schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|doc| doc["$id"] == REPORT);
        assert_eq!(document.is_some(), selected);
        assert_eq!(
            schemas["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|method| method["method"] == "attempt/repair-catalog"),
            selected
        );
        assert!(!schemas["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!(REPORT)));
        if let Some(document) = document {
            assert_eq!(document["additionalProperties"], false);
            assert_eq!(document["properties"]["source_authority"]["const"], false);
            assert_eq!(document["properties"]["schema"]["const"], &REPORT[4..]);
            assert!(document["required"]
                .as_array()
                .unwrap()
                .contains(&json!("repairs")));
            let alternatives = document["properties"]["repairs"]["items"]["oneOf"]
                .as_array()
                .unwrap();
            assert_eq!(alternatives.len(), 2);
            for alternative in alternatives {
                assert_eq!(alternative["additionalProperties"], false);
                assert_eq!(
                    alternative["properties"]["source_authority"]["const"],
                    false
                );
            }
            assert_eq!(
                alternatives
                    .iter()
                    .map(|schema| schema["properties"]["class"]["const"].as_str().unwrap())
                    .collect::<std::collections::BTreeSet<_>>(),
                std::collections::BTreeSet::from([
                    "retag_integer_literal_to_retained_return_type",
                    "borrow_owned_byte_field_without_staging"
                ])
            );
        }
        for language in ["rust", "typescript", "python"] {
            let generated = call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            );
            assert_eq!(
                generated,
                call(
                    &mut session,
                    "protocol/client",
                    json!({"language":language})
                )
            );
            let source = payload(&generated)["source"].as_str().unwrap();
            assert!(source.len() <= 900 * 1024);
            assert_eq!(payload(&generated)["io"], false);
            for token in [
                "AttemptRepairCatalogPayload",
                "AttemptRepairCatalogResult",
                "decode_request_attempt_repair_catalog_typed",
            ] {
                assert_eq!(source.contains(token), selected, "{language}: {token}");
            }
            if selected {
                assert!(source.contains("borrow_owned_byte_field_without_staging"));
                assert!(source.contains("retag_integer_literal_to_retained_return_type"));
                assert!(!source.contains("AttemptRepairCatalogPayload = Any"));
                assert!(!source.contains("AttemptRepairCatalogPayload = unknown"));
                let mut independent = fixture.session(true);
                assert_eq!(
                    generated,
                    call(
                        &mut independent,
                        "protocol/client",
                        json!({"language":language})
                    )
                );
                independent.finish().unwrap();
            }
        }
        session.finish().unwrap();
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn authored_python_harness_checks_actual_recursive_repair_payloads_and_hostile_nested_values() {
    // This invokes Python only when the test is explicitly run in a later gate.
    // No client process is run by the generator or while authoring this source.
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(true);
    let generated = call(
        &mut session,
        "protocol/client",
        json!({"language":"python"}),
    );
    let responses = responses(&mut session);
    std::fs::write(
        fixture.0.join("generated_client.py"),
        payload(&generated)["source"].as_str().unwrap(),
    )
    .unwrap();
    std::fs::write(fixture.0.join("responses.json"), responses.to_string()).unwrap();
    std::fs::write(fixture.0.join("check_recursive.py"), PYTHON).unwrap();
    let output = Command::new("python3")
        .arg("-I")
        .arg(fixture.0.join("check_recursive.py"))
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"recursive-repair-client-ok\n");
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn authored_rust_harness_converts_recursive_typed_repairs_after_runtime_validation() {
    // Compilation and execution happen only if a later test runner invokes
    // this test. The isolated harness is offline and does not touch sources.
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(true);
    let generated = call(&mut session, "protocol/client", json!({"language":"rust"}));
    let source = payload(&generated)["source"].as_str().unwrap();
    assert!(source.contains("ResponseTypeDecodeGuard"));
    let examples = responses(&mut session);
    let root = fixture.0.join("rust-client");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let locked_version = |name: &str| {
        let selected = format!("name = \"{name}\"");
        include_str!("../Cargo.lock")
            .split("[[package]]")
            .find(|package| package.lines().any(|line| line == selected))
            .unwrap()
            .lines()
            .find_map(|line| {
                line.strip_prefix("version = \"")
                    .and_then(|value| value.strip_suffix('"'))
            })
            .unwrap()
    };
    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "recursive-response-evidence"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
serde = { version = "=@SERDE_VERSION@", features = ["derive"] }
serde_json = "=@SERDE_JSON_VERSION@"
[profile.dev]
debug = 0
incremental = false
"#
        .replace("@SERDE_VERSION@", locked_version("serde"))
        .replace("@SERDE_JSON_VERSION@", locked_version("serde_json")),
    )
    .unwrap();
    std::fs::write(root.join("src/client.rs"), source).unwrap();
    std::fs::write(root.join("src/main.rs"), RUST).unwrap();
    std::fs::write(root.join("responses.json"), examples.to_string()).unwrap();
    let output = Command::new("cargo")
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"recursive-rust-conversion-ok\n");
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

const RUST: &str = r#"
#![allow(dead_code)]
mod client;
use serde_json::{json, Value};

fn accepted(response: &Value) {
    let line = response.to_string();
    let id = client::RpcId::Text("recursive-evidence".into());
    let ordinary = client::decode_request_attempt_repair_catalog(&line, &id).unwrap();
    let typed: client::AttemptRepairCatalogResult =
        client::decode_request_attempt_repair_catalog_typed(&line, &id).unwrap();
    assert_eq!(ordinary.payload, response["result"]["payload"]);
    assert_eq!(serde_json::to_value(&typed).unwrap(), response["result"]);
}

fn main() {
    let examples: Value = serde_json::from_str(include_str!("../responses.json")).unwrap();
    for response in examples.as_object().unwrap().values() {
        accepted(response);
    }
    // Shape evidence only: the decoder does not verify or remint the copied
    // compiler report identity after this synthetic constructor substitution.
    let mut nested = examples["borrow"].clone();
    let mut node = nested["result"]["payload"]["repairs"][0]["change"]["intent"]["body"].clone();
    for _ in 0..10 {
        node = json!({"kind":"call","target":"recursive.identity","arguments":[node]});
    }
    nested["result"]["payload"]["repairs"][0]["change"]["intent"]["body"] = node;
    // This reaches the generated custom recursive-union Deserialize path,
    // beyond the ordinary JSON schema validator checked by the old decoder.
    accepted(&nested);
    let id = client::RpcId::Text("recursive-evidence".into());
    let mut hostile = nested.clone();
    hostile["result"]["payload"]["repairs"][0]["change"]["intent"]["body"]["extra"] = json!(true);
    assert!(client::decode_request_attempt_repair_catalog_typed(&hostile.to_string(), &id).is_err());
    let mut deep = examples["borrow"].clone();
    let mut node = json!({"kind":"usize","value":0});
    for _ in 0..160 {
        node = json!({"kind":"call","target":"recursive.identity","arguments":[node]});
    }
    deep["result"]["payload"]["repairs"][0]["change"]["intent"]["body"] = node;
    assert!(client::decode_request_attempt_repair_catalog_typed(&deep.to_string(), &id).is_err());
    // Bypass only the ordinary decoder in this hostile helper probe so the
    // secondary conversion guard itself must stop recursion. It stays sticky
    // across failed serde union alternatives inside the same payload.
    let envelope: client::ResultEnvelope = serde_json::from_value(deep["result"].clone()).unwrap();
    let error = client::typed_result::<client::AttemptRepairCatalogPayload>(envelope)
        .err().expect("recursive typed conversion must hit its own capacity guard");
    assert_eq!(error, "recursive typed response conversion capacity exceeded");
    // A prior rejected conversion must not exhaust the next independent call.
    let envelope: client::ResultEnvelope = serde_json::from_value(examples["borrow"]["result"].clone()).unwrap();
    let recovered = client::typed_result::<client::AttemptRepairCatalogPayload>(envelope).unwrap();
    assert_eq!(serde_json::to_value(recovered).unwrap(), examples["borrow"]["result"]);
    accepted(&examples["borrow"]);
    println!("recursive-rust-conversion-ok");
}
"#;

const PYTHON: &str = r#"
import copy
import importlib.util
import json
from pathlib import Path
import sys
import typing

root = Path(__file__).parent
spec = importlib.util.spec_from_file_location('generated_client', root / 'generated_client.py')
client = importlib.util.module_from_spec(spec)
sys.modules['generated_client'] = client
spec.loader.exec_module(client)
responses = json.loads((root / 'responses.json').read_text())
decoders = (client.decode_request_attempt_repair_catalog, client.decode_request_attempt_repair_catalog_typed)
for response in responses.values():
    for decoder in decoders:
        assert decoder(json.dumps(response), 'recursive-evidence') == response['result']
assert client.AttemptRepairCatalogPayload is not typing.Any
assert 'repairs' in client.AttemptRepairCatalogPayload.__required_keys__
typing.get_type_hints(client.AttemptRepairCatalogPayload, vars(client), vars(client))
pending = list(client.META['documents'].values())
while pending:
    value = pending.pop()
    if isinstance(value, dict):
        if '$ref' in value:
            reference = value['$ref']
            assert reference.startswith('urn:')
            assert reference in client.META['documents'] or reference in client.META['unbundled']
        pending.extend(value.values())
    elif isinstance(value, list):
        pending.extend(value)

def rejects(response):
    for decoder in decoders:
        try:
            decoder(json.dumps(response), 'recursive-evidence')
        except ValueError:
            pass
        else:
            raise AssertionError('accepted malformed recursive repair response')

for name in ('literal', 'borrow'):
    original = responses[name]
    for key, value in [('source_authority', True), ('schema', 'foreign.report')]:
        bad = copy.deepcopy(original)
        bad['result']['payload'][key] = value
        rejects(bad)
    bad = copy.deepcopy(original)
    bad['result']['payload']['repairs'][0]['invented_authority'] = True
    rejects(bad)
    bad = copy.deepcopy(original)
    bad['result']['payload']['repairs'][0]['change']['schema'] = 'foreign.change'
    rejects(bad)
    bad = copy.deepcopy(original)
    bad['result']['payload']['repairs'][0]['semantic_change_intent']['rejected_intent']['extra'] = 1
    rejects(bad)

bad = copy.deepcopy(responses['borrow'])
node = bad['result']['payload']['repairs'][0]['change']['intent']['body']['arguments'][0]['arguments'][0]
assert node['kind'] == 'field_place'
node['root'] = ['packet']
rejects(bad)
bad = copy.deepcopy(responses['borrow'])
bad['result']['payload']['repairs'][0]['change']['intent']['body']['arguments'][0]['arguments'][0]['extra'] = 1
rejects(bad)
bad = copy.deepcopy(responses['borrow'])
bad['result']['payload']['repairs'][0]['replacements'][0]['root'] = None
rejects(bad)
bad = copy.deepcopy(responses['literal'])
bad['result']['payload']['repairs'][0]['change']['intent']['body']['value'] = True
rejects(bad)
bad = copy.deepcopy(responses['literal'])
bad['result']['payload']['repairs'][0]['change']['intent']['body']['value'] = 2**64
rejects(bad)

# Schema recursion is allowed, while hostile runtime depth is bounded. This
# synthetic response is shape evidence only, never candidate validation.
linear = copy.deepcopy(responses['borrow'])
node = linear['result']['payload']['repairs'][0]['change']['intent']['body']
for _ in range(10):
    node = {'kind':'call', 'target':'recursive.identity', 'arguments':[node]}
linear['result']['payload']['repairs'][0]['change']['intent']['body'] = node
# The ordinary function identity exists in the source fixture, but this changed
# report is only a client-shape probe. Clients do not remint compiler evidence.
# Every rejected builtin-call alternative must check its kind/target before
# recursively walking the same argument tree; otherwise this exceeds the
# moderate shared budget despite remaining below the schema depth bound.
client._check(linear['result']['payload'], client.META['methods']['attempt/repair-catalog']['payload'], budget=[8192])
for decoder in decoders:
    assert decoder(json.dumps(linear), 'recursive-evidence') == linear['result']

bad = copy.deepcopy(responses['borrow'])
node = {'kind': 'usize', 'value': 0}
for index in range(160):
    node = {'kind': 'let', 'name': 'nested_' + str(index), 'value': {'kind':'usize', 'value':0}, 'body': node}
bad['result']['payload']['repairs'][0]['change']['intent']['body'] = node
rejects(bad)

# Exercise audited schema primitives directly without manufacturing a new
# compiler report class. These are the same validators used by both decoders.
def rejects_shape(value, schema):
    try:
        client._check(value, schema)
    except ValueError:
        pass
    else:
        raise AssertionError(('accepted invalid shape', value, schema))

for pattern, valid, invalid in [
    (r'^[A-Za-z_][A-Za-z0-9_]*$', 'packet_2', 'packet.left'),
    (r'^[A-Za-z0-9_.:-]+$', 'scope:field-id.1', 'scope/name'),
    (r'^[a-z0-9._-]+$', 'package.module-1', 'UpperCase'),
]:
    schema = {'type':'string', 'pattern':pattern}
    client._check(valid, schema)
    rejects_shape(invalid, schema)
excluded = {'type':'string', 'not':{'enum':['builtin_call','field_place']}}
client._check('literal', excluded)
rejects_shape('field_place', excluded)
unique = {'type':'array','items':{'type':'string'},'uniqueItems':True}
client._check(['left','right'], unique)
rejects_shape(['left','left'], unique)
rejects_shape(['left',False], unique)

# A successful union branch must not hide a later traversal-budget failure.
for alternative in ('anyOf','oneOf'):
    try:
        client._check('ok', {alternative:[{},{}]}, budget=[2])
    except client._ShapeLimitError:
        pass
    else:
        raise AssertionError('union swallowed a shared work-limit failure')
    try:
        client._check('ok', {alternative:[{}, {alternative:[{}]}]}, depth=127)
    except client._ShapeLimitError:
        pass
    else:
        raise AssertionError('union swallowed a depth-limit failure')
try:
    client._check(['left','right'], unique, budget=[3])
except client._ShapeLimitError:
    pass
else:
    raise AssertionError('uniqueItems did not consume shared work budget')
print('recursive-repair-client-ok')
"#;
