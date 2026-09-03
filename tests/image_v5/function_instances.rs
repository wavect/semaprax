//! Authored, unrun transport evidence for retained generic-instance navigation.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ImageFacet, ImageFacetOptions, ProjectSemanticImage,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const LIST: &str = "image/function-instances";
const FACET: &str = "image/function-instance-facet";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-instance-v5-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "instance-transport"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "instances.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["instances.public"]
tests = ["instances.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/app.spx",
                "module instances.app;\n@id(\"instances.main\") fn main()->i64 {0}\n",
            ),
            (
                "src/core.spx",
                r#"module instances.core;
@id("instances.keep") fn keep<T>(value:T)->T {value}
@id("instances.unused") fn unused<T>(value:T)->T {value}
@id("instances.use-i64") fn use_i64(value:i64)->i64 {keep<i64>(value)}
@id("instances.use-bool") fn use_bool(value:bool)->bool {keep<bool>(value)}
@id("instances.public") fn public_value(value:i64)->i64 {value}
"#,
            ),
            (
                "src/tests.spx",
                "module instances.tests;\n@id(\"instances.test\") fn main()->i64 {0}\n",
            ),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(&self.0.join("semaprax.toml"), VNextPolicy::default()).unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn frame(id: usize, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&frame(1, method, params)).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn error(response: Value, code: &str) {
    assert!(response.get("result").is_none(), "{response}");
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains(code),
        "{response}"
    );
}
fn listing(image: &ProjectSemanticImage) -> Value {
    serde_json::from_str(
        &image
            .function_instances(
                image.image_digest(),
                "instances.keep",
                None,
                ImageFacetOptions::default(),
            )
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn readonly_listing_and_all_instance_facets_match_library_and_parallel_frames() {
    let fixture = Fixture::new();
    let bytes = fixture.bytes();
    let image = fixture.image();
    let original_image = image.to_json().to_owned();
    let list = listing(&image);
    assert_eq!(list["total_instances"], 2);
    let mut session = fixture.session();
    let list_params = json!({"image_revision":image.image_digest(),"target":"instances.keep"});
    assert_eq!(payload(call(&mut session, LIST, list_params.clone())), list);
    let empty = payload(call(
        &mut session,
        LIST,
        json!({"image_revision":image.image_digest(),"target":"instances.unused"}),
    ));
    assert_eq!(empty["instances"], json!([]));
    assert_eq!(empty["next_cursor"], Value::Null);
    let row = &list["instances"][0];
    let mut frames = vec![frame(1, LIST, list_params)];
    for (index, facet) in ImageFacet::ALL.into_iter().enumerate() {
        let handle = row["facets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["facet"] == facet.name())
            .unwrap()["handle"]
            .as_str()
            .unwrap();
        let params = json!({"image_revision":image.image_digest(),"target":"instances.keep","instance_id":row["instance_id"],"facet":facet.name(),"handle":handle});
        let expected: Value = serde_json::from_str(
            &image
                .expand_instance_facet(
                    image.image_digest(),
                    "instances.keep",
                    row["instance_id"].as_str().unwrap(),
                    facet,
                    handle,
                    None,
                    ImageFacetOptions::default(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(payload(call(&mut session, FACET, params.clone())), expected);
        frames.push(frame(index + 2, FACET, params));
    }
    let mut sequential = fixture.session();
    let expected = frames
        .iter()
        .map(|frame| sequential.handle_frame(frame))
        .collect::<Vec<_>>();
    let borrowed = frames.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            fixture
                .session()
                .handle_read_batch(&borrowed, workers)
                .unwrap(),
            expected
        );
    }
    let mut rpc = fixture.session().with_read_batch_workers(2).unwrap();
    let batch = payload(call(
        &mut rpc,
        "workspace/read-batch",
        json!({"image_revision":image.image_digest(),"batch":{"frames":frames.iter().map(|frame|std::str::from_utf8(frame).unwrap()).collect::<Vec<_>>()}}),
    ));
    let texts = expected
        .iter()
        .map(|response| {
            response
                .as_ref()
                .map(|bytes| std::str::from_utf8(bytes).unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(batch["responses"], json!(texts));
    assert_eq!(image.to_json(), original_image);
    assert_eq!(fixture.bytes(), bytes);
}

#[test]
fn discovery_closes_new_envelopes_but_explicitly_leaves_facet_items_opaque() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let bundle = payload(call(&mut session, "protocol/schemas", json!({})));
    for (method, params) in [(LIST, 5), (FACET, 8)] {
        let descriptor = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == method)
            .unwrap();
        assert_eq!(descriptor["capability"], "semantic_read");
        assert_eq!(descriptor["query"], true);
        let shape = &descriptor["request_schema"]["properties"]["params"];
        assert_eq!(shape["additionalProperties"], false);
        assert_eq!(shape["properties"].as_object().unwrap().len(), params);
        assert_eq!(shape["properties"]["page_size"]["maximum"], 128);
    }
    for (schema, count) in [
        ("urn:semaprax.image-function-instances.v1", 20),
        ("urn:semaprax.image-instance-facet.v1", 21),
    ] {
        let shape = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["$id"] == schema)
            .unwrap();
        assert_eq!(shape["additionalProperties"], false);
        assert_eq!(shape["required"].as_array().unwrap().len(), count);
        assert!(!bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!(schema)));
        if schema.contains("function-instances") {
            assert_eq!(
                shape["properties"]["instances"]["items"]["required"]
                    .as_array()
                    .unwrap()
                    .len(),
                8
            );
        } else {
            assert_eq!(
                shape["properties"]["items"]["items"]["$ref"],
                "urn:semaprax.image-instance-facet-item.v1"
            );
        }
    }
    assert!(bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!("urn:semaprax.image-instance-facet-item.v1")));
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        for name in [
            "ImageFunctionInstancesPayload",
            "ImageFunctionInstanceFacetPayload",
            "request_image_function_instances_typed",
            "decode_request_image_function_instance_facet_typed",
        ] {
            assert!(source.contains(name), "{language}: {name}");
        }
    }
    let mut old = ImageSession::open(
        &fixture.0.join("semaprax.toml"),
        ImageHostCapability::ReadOnly,
    )
    .unwrap();
    for method in [LIST, FACET] {
        let response: Value =
            serde_json::from_slice(&old.handle_frame(&frame(1, method, json!({}))).unwrap())
                .unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
}

#[test]
fn stale_selectors_malformed_requests_and_live_source_drift_fail_closed() {
    let fixture = Fixture::new();
    let image = fixture.image();
    let list = listing(&image);
    let row = &list["instances"][0];
    let handle = &row["facets"][0]["handle"];
    let mut session = fixture.session();
    let mut params = json!({"image_revision":image.image_digest(),"target":"instances.keep","instance_id":row["instance_id"],"facet":"signature","handle":handle});
    params["handle"] = json!(format!("sha256:{}", "0".repeat(64)));
    error(call(&mut session, FACET, params.clone()), "SPX-G229");
    params["handle"] = handle.clone();
    params["target"] = json!("instances.unused");
    error(call(&mut session, FACET, params), "SPX-G227");
    error(
        call(
            &mut session,
            LIST,
            json!({"image_revision":format!("sha256:{}", "0".repeat(64)),"target":"instances.keep"}),
        ),
        "SPX-G282",
    );
    let malformed = call(
        &mut session,
        LIST,
        json!({"image_revision":image.image_digest(),"target":"instances.keep","page_size":0}),
    );
    assert_eq!(malformed["error"]["code"], -32602);
    let source = fixture.0.join("src/core.spx");
    let original = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, original + "\n").unwrap();
    let drifted = call(
        &mut session,
        LIST,
        json!({"image_revision":image.image_digest(),"target":"instances.keep"}),
    );
    assert!(drifted.get("error").is_some(), "{drifted}");
}
