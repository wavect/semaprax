//! Owning regressions for the ordinary Core-Wasm shared-owner consumer
//! closure. Node cases skip when Node is absent; they are source-authored.
use super::{
    admit_producer, classify, consumer_closure, Crossing, Disposition, Host, Refusal, DOMAIN,
    HOST_SOURCE, LEAF_FIXTURE_JOINED, LEASE_AWARE, PARTITION_STATUS_CODES, PARTITION_STATUS_DOMAIN,
    PAYLOAD_CONSUMERS, REGISTRY_SOURCE,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const HOSTILE: &str = include_str!("tests/hostile.mjs");
pub(super) const ORDINARY: &str = include_str!("tests/ordinary.mjs");
/// The actual owning host of record/leaf Vec, record iterator and sort
/// payloads; its `shared-owner` mode is the joined consumer.
const LEAF_HOST: &str = include_str!("../../../tests/owned_data/owned_leaf_vec/host.js");
const HOSTS: [Host; 2] = [Host::PrivateOrdinary, Host::OwnedLeafVecFixture];

const VEC_SOURCE: &str = r#"module shared.owner.vec;
@id("app.main") fn main()->i64 {
 let input=[1u8,2u8,3u8];
 let empty=vec_with_capacity<Bytes>(1usize);
 let first=vec_push<Bytes>(empty,bytes_copy(array_as_slice(input)));
 let reserved=vec_reserve_exact<Bytes>(first,1usize);
 let replaced=vec_set<Bytes>(reserved,0usize,bytes_copy(array_as_slice(input)));
 let second=vec_push<Bytes>(replaced,bytes_copy(array_as_slice(input)));
 let observed=vec_len<Bytes>(second)==2usize && vec_capacity<Bytes>(second)==2usize;
 let cleared=vec_clear<Bytes>(second);
 let lexical=vec_push<Bytes>(cleared,bytes_copy(array_as_slice(input)));
 if observed && vec_len<Bytes>(lexical)==1usize && vec_capacity<Bytes>(lexical)==2usize {29}else{0}
}
"#;

const ITER_SOURCE: &str = r#"module shared.owner.iter;
@id("iter.size") fn size(value:own Bytes)->usize {
 let view=bytes_as_slice(value);byte_len(view)
}
@id("app.main") fn main()->i64 {
 let values=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_set(bytes_zeroed(3usize),1usize,7u8));
 let more=vec_push<Bytes>(values,bytes_zeroed(2usize));
 let mut seen=0;
 for own item in vec_into_iter<Bytes>(more) {
  let length=size(item);
  if length==3usize {seen=seen+1;0}else{seen=seen+10;0}
 }
 seen
}
"#;

/// Box<Bytes> v2 consume and lexical recursive drop
/// (`tests/owned_data/owned_box_bytes_runtime.rs`).
const BOX_SOURCE: &str = r#"
module test.owned_box_bytes_runtime;

@id("app.main")
fn main() -> i64 {
    let input = [65u8, 66u8, 67u8];
    let boxed = box_new<Bytes>(bytes_copy(array_as_slice(input)));
    let inner = box_into_inner<Bytes>(boxed);
    let lexical = box_new<Bytes>(bytes_copy(array_as_slice(input)));
    if byte_len(bytes_as_slice(inner)) == 3usize { 29 } else { 1 }
}
"#;

/// `tests/owned_data/owned_leaf_vec.rs` declaration and two of its bodies.
const LEAF_DECLARATION: &str = r#"module app.leaf;
@id("app.leaf.row") record Row {
 @id("app.leaf.row.priority") priority:i64,
 @id("app.leaf.row.title") title:string,
 @id("app.leaf.row.payload") payload:Bytes,
}
"#;

const LEAF_STRING_BODY: &str = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<string>(2usize);
 let one=vec_push<string>(empty,"z");
 let two=vec_push<string>(one,"a");
 let sorted=vec_sort_owned<string>(two);
 let first=vec_clone_at<string>(sorted,0usize);
 let renewed=vec_replace<string>(sorted,1usize,"m");
 let reserved=vec_reserve_owned<string>(renewed,2usize);
 let good=first=="a" && vec_len<string>(reserved)==2usize && vec_capacity<string>(reserved)==4usize;
 let cleared=vec_clear<string>(reserved);
 if good && vec_len<string>(cleared)==0usize && vec_capacity<string>(cleared)==4usize {29}else{0}
}
"#;

const LEAF_ROW_BODY: &str = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(3usize);
 let first=vec_push<Row>(empty,Row{priority:7,title:"é\u{0}",payload:bytes_zeroed(3usize)});
 let second=vec_push<Row>(first,Row{priority:11,title:"x",payload:bytes_zeroed(1usize)});
 let rows=vec_push<Row>(second,Row{priority:13,title:"",payload:bytes_zeroed(0usize)});
 let mut order=0; let mut good=true;
 for borrow row at index in rows {
  let text=row.title; let data=row.payload; let utf8=str_as_bytes(row.title);
  order=order*10+i64_from_usize(index)+1;
  good=good && byte_len(data)==byte_len(utf8) && str_len_bytes(text)==i64_from_usize(byte_len(utf8));
  let scalar=row.priority;
  good=good && scalar>0;
  0
 }
 let renewed=vec_sort_owned<Row>(rows);
 if good && vec_len<Row>(renewed)==3usize && order==123 {29}else{0}
}"#;

/// Every `env` import name the ordinary emitters can request.
const ORDINARY_IMPORTS: &[&str] = &[
    "spx_add",
    "spx_sub",
    "spx_mul",
    "spx_div",
    "spx_rem",
    "spx_neg",
    "spx_contract_fail",
    "spx_box_drop",
    "spx_box_drop_v2",
    "spx_box_get",
    "spx_box_get_v2",
    "spx_box_into_inner",
    "spx_box_into_inner_v2",
    "spx_box_new",
    "spx_box_new_v2",
    "spx_bytes_as_slice",
    "spx_bytes_copy",
    "spx_bytes_drop",
    "spx_bytes_get",
    "spx_bytes_set",
    "spx_bytes_set1_or5",
    "spx_bytes_set1_or6_or48",
    "spx_bytes_set5",
    "spx_bytes_zeroed",
    "spx_collection_checked_v2",
    "spx_collection_drop_v2",
    "spx_command_arg_utf8_v1",
    "spx_command_args_len_v1",
    "spx_command_owned_bytes_validate_v1",
    "spx_command_stdin_read_v1",
    "spx_environment_len_v1",
    "spx_environment_name_utf8_v1",
    "spx_environment_value_utf8_v1",
    "spx_file_read_text_v2",
    "spx_filesystem_create_dir_v2",
    "spx_filesystem_list_v2",
    "spx_filesystem_read_v1",
    "spx_filesystem_remove_v2",
    "spx_filesystem_stat_v2",
    "spx_filesystem_write_atomic_checked_v3",
    "spx_filesystem_write_atomic_v2",
    "spx_filesystem_write_new_v1",
    "spx_format_step_v1",
    "spx_https_get_v1",
    "spx_https_post_v1",
    "spx_iter_bytes_drop_v2",
    "spx_iter_bytes_into_v2",
    "spx_iter_bytes_next_v2",
    "spx_iter_record_drop_v3",
    "spx_iter_record_into_v3",
    "spx_iter_record_next_v3",
    "spx_network_close_v1",
    "spx_network_connect_v1",
    "spx_network_recv_v1",
    "spx_network_send_v1",
    "spx_network_settle_v1",
    "spx_network_stream_stdout_v1",
    "spx_network_wait_v1",
    "spx_owned_abort",
    "spx_owned_begin",
    "spx_owned_cancel_result",
    "spx_owned_commit",
    "spx_owned_drop",
    "spx_owned_publish",
    "spx_owned_reserve_result",
    "spx_owned_stage",
    "spx_owned_success",
    "spx_owned_utf8_validate_v1",
    "spx_pg_component_input_encode_v1",
    "spx_pg_component_result_copy_v1",
    "spx_pg_v1_call",
    "spx_pg_v1_input_prepare",
    "spx_pg_v1_open",
    "spx_pg_v1_provider_close",
    "spx_pg_v1_result_export",
    "spx_pg_v1_result_release",
    "spx_pg_v1_scratch_capacity",
    "spx_pg_v1_scratch_ptr",
    "spx_pg_v1_scratch_reserve",
    "spx_pg_v1_value_release",
    "spx_process_run_v1",
    "spx_process_settle_v1",
    "spx_semantic_event",
    "spx_status_record",
    "spx_string_byte_at_v2",
    "spx_string_clone",
    "spx_string_compare_v2",
    "spx_string_concat",
    "spx_string_concat_v1",
    "spx_string_contains",
    "spx_string_contains_v1",
    "spx_string_eq",
    "spx_string_find_v2",
    "spx_string_from_char",
    "spx_string_from_char_v1",
    "spx_string_from_i64",
    "spx_string_from_i64_v1",
    "spx_string_from_str_v2",
    "spx_string_from_usize",
    "spx_string_from_usize_v1",
    "spx_string_from_utf8_v1",
    "spx_string_len",
    "spx_string_len_chars",
    "spx_string_len_chars_v1",
    "spx_string_new",
    "spx_string_slice_v2",
    "spx_string_starts_with",
    "spx_string_starts_with_v1",
    "spx_string_to_i64_v2",
    "spx_string_trim_v2",
    "spx_vec_capacity",
    "spx_vec_capacity_v2",
    "spx_vec_clear",
    "spx_vec_clear_v2",
    "spx_vec_drop",
    "spx_vec_drop_v2",
    "spx_vec_get",
    "spx_vec_get_v2",
    "spx_vec_leaf_clone_at_v1",
    "spx_vec_leaf_field_read_v1",
    "spx_vec_leaf_into_iter_v1",
    "spx_vec_leaf_iter_drop_v1",
    "spx_vec_leaf_iter_next_v1",
    "spx_vec_leaf_new_v1",
    "spx_vec_leaf_push_v1",
    "spx_vec_leaf_replace_v1",
    "spx_vec_leaf_reserve_v1",
    "spx_vec_leaf_sort_v1",
    "spx_vec_len",
    "spx_vec_len_v2",
    "spx_vec_push",
    "spx_vec_push_v2",
    "spx_vec_record_push_v2",
    "spx_vec_reserve_exact",
    "spx_vec_reserve_exact_v2",
    "spx_vec_set",
    "spx_vec_set_v2",
    "spx_vec_sort_by_v1",
    "spx_vec_sort_v3",
    // Record-Vec field uniqueness (96e6fbfb6) and scalar field update (4249cfc62).
    "spx_vec_unique_by_v1",
    "spx_vec_update_field_v1",
    "spx_vec_with_capacity",
    "spx_vec_with_capacity_v2",
];

/// `spx_` literals in emitter sources that name exports or export prefixes,
/// never imports.
const EMITTER_EXPORT_NAMES: &[&str] = &[
    "spx_call_depth_exceeded",
    "spx_owned_v1_",
    "spx_text_",
    "spx_scalar_",
    "spx_data_",
    "spx_semantic_fuel_used",
    "spx_semantic_fuel_exhausted",
    "spx_semantic_event_count",
    "spx_semantic_event_overflow",
    "spx_semantic_event_",
];

/// Every complete `"spx_…"` string literal in `body`.
fn quoted_names(body: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (at, _) in body.match_indices("\"spx_") {
        let name: String = body[at + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if body[at + 1 + name.len()..].starts_with('"') {
            names.insert(name);
        }
    }
    names
}

fn object_keys(source: &str, open: &str, close: &str) -> BTreeSet<String> {
    let body = source
        .split_once(open)
        .and_then(|(_, rest)| rest.split_once(close))
        .unwrap_or_else(|| panic!("object {open}"))
        .0;
    let mut keys = BTreeSet::new();
    for (at, _) in body.match_indices("spx_") {
        let name: String = body[at..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if body[at + name.len()..].starts_with(':') {
            keys.insert(name);
        }
    }
    keys
}

fn emitter_literals(directory: &Path, found: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.contains("test") || name == "shared_owner" || name == "shared_owner.rs" {
            continue;
        }
        if path.is_dir() {
            emitter_literals(&path, found);
        } else if name.ends_with(".rs") {
            found.extend(quoted_names(&std::fs::read_to_string(&path).unwrap()));
        }
    }
}

fn module(source: &str) -> Vec<u8> {
    let checked = crate::check(source, "shared-owner.spx").unwrap();
    crate::wasm::emit_module(&checked).unwrap()
}

pub(super) fn node_available() -> bool {
    Command::new("node").arg("--version").output().is_ok()
}

pub(super) fn stage(files: &[(&str, &[u8])]) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-wasm-shared-owner-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for (name, contents) in [
        ("registry.mjs", REGISTRY_SOURCE.as_bytes()),
        ("host.mjs", HOST_SOURCE.as_bytes()),
    ]
    .into_iter()
    .chain(files.iter().copied())
    {
        std::fs::write(directory.join(name), contents).unwrap();
    }
    directory
}

pub(super) fn run_node(directory: &Path, script: &str, args: &[&str]) -> String {
    let output = Command::new("node")
        .arg(directory.join(script))
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{script} {args:?} failed\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(directory).unwrap();
    stdout
}

#[test]
fn every_ordinary_import_is_classified_without_an_unknown_default() {
    for host in HOSTS {
        for name in ORDINARY_IMPORTS {
            match classify(host, "env", name) {
                Ok(_) | Err(Refusal::Crossing(_, _)) => {}
                other => panic!("{host:?} {name} is unclassified: {other:?}"),
            }
        }
        assert_eq!(
            classify(host, "env", "spx_bytes_partition_v1"),
            Err(Refusal::Unknown("spx_bytes_partition_v1".to_owned()))
        );
        assert_eq!(
            classify(host, "semaprax.private.selected-fused.v1", "spx_bytes_drop"),
            Err(Refusal::Namespace(
                "semaprax.private.selected-fused.v1.spx_bytes_drop".to_owned()
            ))
        );
        for (name, crossing) in [
            ("spx_owned_drop", Crossing::PublicOwnedEnvelope),
            ("spx_pg_v1_value_release", Crossing::Provider),
            ("spx_filesystem_write_new_v1", Crossing::Provider),
            ("spx_file_read_text_v2", Crossing::Provider),
            ("spx_collection_checked_v2", Crossing::Collection),
            ("spx_format_step_v1", Crossing::Format),
            ("spx_string_slice_v2", Crossing::StringToolkit),
            ("spx_string_concat", Crossing::StringToolkit),
            ("spx_box_new", Crossing::LegacyBox),
            ("spx_vec_push", Crossing::LegacyVec),
        ] {
            assert_eq!(
                classify(host, "env", name),
                Err(Refusal::Crossing(name.to_owned(), crossing)),
                "{host:?}"
            );
        }
        assert_eq!(
            classify(host, "env", "spx_add"),
            Ok(Disposition::CarrierFree)
        );
        assert_eq!(
            classify(host, "env", "spx_box_drop_v2"),
            Ok(Disposition::LeaseAware)
        );
    }
    // Each host refuses the other's Vec-table consumers by name.
    for name in [
        "spx_vec_leaf_replace_v1",
        "spx_iter_record_next_v3",
        "spx_vec_sort_v3",
    ] {
        assert_eq!(
            classify(Host::PrivateOrdinary, "env", name),
            Err(Refusal::Crossing(name.to_owned(), Crossing::OtherHost))
        );
        assert_eq!(
            classify(Host::OwnedLeafVecFixture, "env", name),
            Ok(Disposition::LeaseAware)
        );
    }
    assert_eq!(
        classify(Host::OwnedLeafVecFixture, "env", "spx_iter_bytes_next_v2"),
        Err(Refusal::Crossing(
            "spx_iter_bytes_next_v2".to_owned(),
            Crossing::OtherHost
        ))
    );
    // The String runtime group is served whole by both hosts.
    for host in HOSTS {
        assert_eq!(
            classify(host, "env", "spx_string_concat_v1"),
            Ok(Disposition::LeaseAware)
        );
    }
}

#[test]
fn every_emitter_import_literal_is_in_the_classified_inventory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = quoted_names(&std::fs::read_to_string(root.join("wasm.rs")).unwrap());
    emitter_literals(&root.join("wasm"), &mut found);
    let known: BTreeSet<&str> = ORDINARY_IMPORTS.iter().copied().collect();
    // The private source-checked partition producer (597d6e1a5) is emitted by
    // `aggregate::emit_partition` but deliberately stays outside the ordinary
    // classification: only `PartitionModule` admits it, and only as the host's
    // exact `producer` object. Bind the exception to that host object.
    let producer = object_keys(HOST_SOURCE, "const producer = {", "\n  };");
    assert_eq!(producer, BTreeSet::from(["spx_bytes_partition".to_owned()]));
    for name in &producer {
        assert!(!known.contains(name.as_str()), "{name} classified as ordinary");
    }
    for name in &found {
        let export = EMITTER_EXPORT_NAMES
            .iter()
            .any(|prefix| name == prefix || (prefix.ends_with('_') && name.starts_with(prefix)));
        assert!(
            known.contains(name.as_str()) || export || producer.contains(name),
            "{name} is neither a classified import nor a known export"
        );
    }
}

#[test]
fn private_host_implements_exactly_the_lease_aware_consumers() {
    let owned = |list: &[&str]| -> BTreeSet<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    };
    let expected = owned(LEASE_AWARE);
    assert_eq!(
        object_keys(HOST_SOURCE, "const consumers = {", "\n  };"),
        expected
    );
    assert_eq!(expected.len(), LEASE_AWARE.len(), "duplicate consumer");
    let payload = owned(PAYLOAD_CONSUMERS);
    let exported = HOST_SOURCE
        .split_once("export const PAYLOAD_CONSUMERS = Object.freeze([")
        .and_then(|(_, rest)| rest.split_once("]);"))
        .expect("payload consumer export")
        .0;
    assert_eq!(quoted_names(exported), payload);
    assert!(payload.is_subset(&expected));
    // Producer remains a separate, exact private source-checked import;
    // the raw import-only receipt continues to deny its admission.
    assert_eq!(
        object_keys(HOST_SOURCE, "const producer = {", "\n  };"),
        owned(&["spx_bytes_partition"])
    );
    assert_eq!(
        classify(Host::PrivateOrdinary, "env", "spx_bytes_partition"),
        Err(Refusal::Unknown("spx_bytes_partition".to_owned()))
    );
    assert!(REGISTRY_SOURCE.contains(DOMAIN));
    // The registry's status domain and codes are the owning contract's.
    assert!(REGISTRY_SOURCE.contains(PARTITION_STATUS_DOMAIN));
    for (symbol, code) in PARTITION_STATUS_CODES {
        assert!(
            REGISTRY_SOURCE.contains(&format!("  {symbol}: {code},")),
            "{symbol}"
        );
    }
    // The realm marker contains no mint/lookup authority. It cannot be
    // replaced or adopted by a separately evaluated copy.
    assert!(HOST_SOURCE.contains("Object.prototype.hasOwnProperty.call(globalThis, REALM_KEY)"));
    assert!(HOST_SOURCE.contains(
        "value: Object.freeze({}), writable: false, configurable: false, enumerable: false"
    ));
    assert!(!HOST_SOURCE.contains("value: TOKENS"));
    assert!(!HOST_SOURCE.contains("export const TOKENS"));
    assert!(!HOST_SOURCE.contains("export function nextHandle"));
    for forbidden in ["FinalizationRegistry", "WeakRef"] {
        assert!(!HOST_SOURCE.contains(forbidden), "{forbidden}");
        assert!(!REGISTRY_SOURCE.contains(forbidden), "{forbidden}");
    }
    for forbidden in ["globalThis", "Symbol.for"] {
        assert!(!REGISTRY_SOURCE.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn leaf_host_serves_its_joined_consumers_through_the_shared_hooks() {
    let served = object_keys(LEAF_HOST, "const env = {", "\n};");
    for name in LEAF_FIXTURE_JOINED {
        assert!(
            served.contains(*name),
            "{name} is not served by the leaf host"
        );
    }
    // Shared mode overrides exactly the private payload consumers, and every
    // host-local payload path has a shared branch.
    assert!(
        LEAF_HOST.contains("for(const name of PAYLOAD_CONSUMERS)env[name]=shared.imports[name];")
    );
    for hook in [
        "if (shared) return shared.payload.owned(carrier);",
        "shared.payload.clone(carrier)",
        "shared.payload.own(new Uint8Array(bytes))",
        "if (shared) { shared.payload.drop(carrier); drops++; return; }",
        "if(shared)return shared.payload.owned(word);",
        "shared?shared.payload.owned(word):payloads.get(",
        "shared.facade.endInvocation(receipt)",
    ] {
        assert!(LEAF_HOST.contains(hook), "{hook}");
    }
}

#[test]
fn public_runtime_keeps_its_token_table_and_never_sees_the_domain() {
    let runtime = super::super::browser_runtime();
    assert!(!runtime.contains(DOMAIN));
    assert!(!runtime.contains("shared-byte-owner"));
    assert!(runtime.contains("const entries = new Map();"));
}

#[test]
fn ordinary_modules_close_per_host_but_the_producer_stays_closed() {
    for source in [VEC_SOURCE, ITER_SOURCE, BOX_SOURCE] {
        let bytes = module(source);
        let closure = consumer_closure(&bytes, Host::PrivateOrdinary).unwrap();
        assert!(closure
            .lease_aware
            .iter()
            .any(|name| name == "spx_bytes_drop"
                || name == "spx_vec_drop_v2"
                || name == "spx_iter_bytes_drop_v2"
                || name == "spx_box_drop_v2"));
        assert_eq!(&*closure.module, bytes.as_slice());
        assert_eq!(admit_producer(&closure), Err(Refusal::ProducerClosed));
    }
    assert!(matches!(
        consumer_closure(&module(ITER_SOURCE), Host::OwnedLeafVecFixture),
        Err(Refusal::Crossing(_, Crossing::OtherHost))
    ));
    for body in [LEAF_STRING_BODY, LEAF_ROW_BODY] {
        let bytes = module(&format!("{LEAF_DECLARATION}{body}"));
        let closure = consumer_closure(&bytes, Host::OwnedLeafVecFixture).unwrap();
        assert_eq!(closure.host, Host::OwnedLeafVecFixture);
        assert_eq!(admit_producer(&closure), Err(Refusal::ProducerClosed));
        assert!(matches!(
            consumer_closure(&bytes, Host::PrivateOrdinary),
            Err(Refusal::Crossing(_, Crossing::OtherHost))
        ));
    }
}

#[test]
fn closure_refuses_provider_and_non_function_crossings() {
    use wasm_encoder::{EntityType, ImportSection, MemoryType, Module, TypeSection, ValType};
    let mut types = TypeSection::new();
    types.ty().function([ValType::I64], [] as [ValType; 0]);
    let mut imports = ImportSection::new();
    imports.import("env", "spx_bytes_drop", EntityType::Function(0));
    imports.import("env", "spx_filesystem_read_v1", EntityType::Function(0));
    let mut provider = Module::new();
    provider.section(&types).section(&imports);
    let provider = provider.finish();
    let mut imports = ImportSection::new();
    imports.import(
        "env",
        "memory",
        EntityType::Memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        }),
    );
    let mut memory = Module::new();
    memory.section(&imports);
    let memory = memory.finish();
    for host in HOSTS {
        assert_eq!(
            consumer_closure(&provider, host),
            Err(Refusal::Crossing(
                "spx_filesystem_read_v1".to_owned(),
                Crossing::Provider
            ))
        );
        assert_eq!(
            consumer_closure(&memory, host),
            Err(Refusal::NotFunction("env.memory".to_owned()))
        );
        assert_eq!(
            consumer_closure(b"\0asm\x02", host),
            Err(Refusal::Malformed)
        );
        assert_eq!(
            consumer_closure(b"\0asm\x0d\0\x01\0", host),
            Err(Refusal::Malformed)
        );
        // A valid Core module cannot authenticate a known import by name alone.
        let mut types = TypeSection::new();
        types.ty().function([] as [ValType; 0], [] as [ValType; 0]);
        let mut imports = ImportSection::new();
        imports.import("env", "spx_bytes_drop", EntityType::Function(0));
        let mut wrong = Module::new();
        wrong.section(&types).section(&imports);
        assert_eq!(
            consumer_closure(&wrong.finish(), host),
            Err(Refusal::Signature("spx_bytes_drop".into()))
        );
        let mut imports = ImportSection::new();
        imports.import("env", "spx_bytes_drop", EntityType::Function(9));
        let mut invalid = Module::new();
        invalid.section(&types).section(&imports);
        assert_eq!(
            consumer_closure(&invalid.finish(), host),
            Err(Refusal::Malformed)
        );
    }
}

#[test]
fn private_host_runs_ordinary_vec_iterator_and_box_programs_unchanged() {
    if !node_available() {
        return;
    }
    let ensures = BOX_SOURCE.replace("fn main() -> i64 {", "fn main() -> i64 ensures false {");
    let requires = BOX_SOURCE.replace("fn main() -> i64 {", "fn main() -> i64 requires false {");
    for (source, expected) in [
        (VEC_SOURCE, "29"),
        (ITER_SOURCE, "11"),
        (BOX_SOURCE, "29"),
        (ensures.as_str(), "status:10"),
        (requires.as_str(), "status:9"),
    ] {
        let bytes = module(source);
        consumer_closure(&bytes, Host::PrivateOrdinary).unwrap();
        for mode in ["plain", "lease"] {
            let directory = stage(&[("ordinary.mjs", ORDINARY.as_bytes()), ("app.wasm", &bytes)]);
            let stdout = run_node(&directory, "ordinary.mjs", &["app.wasm", expected, mode]);
            assert!(stdout.contains("shared-owner-ordinary-ok"), "{stdout}");
        }
    }
}

#[test]
fn leaf_host_runs_record_and_string_rows_over_shared_owners_and_leases() {
    if !node_available() {
        return;
    }
    for body in [LEAF_STRING_BODY, LEAF_ROW_BODY] {
        let bytes = module(&format!("{LEAF_DECLARATION}{body}"));
        consumer_closure(&bytes, Host::OwnedLeafVecFixture).unwrap();
        for mode in ["shared-owner", "shared-owner-lease"] {
            let directory = stage(&[("host.js", LEAF_HOST.as_bytes()), ("app.wasm", &bytes)]);
            run_node(&directory, "host.js", &["app.wasm", "0", "29", mode]);
        }
    }
}

#[test]
fn shared_owner_hostile_lifecycle_regressions() {
    if !node_available() {
        return;
    }
    let directory = stage(&[("hostile.mjs", HOSTILE.as_bytes())]);
    let stdout = run_node(&directory, "hostile.mjs", &[]);
    assert!(stdout.contains("shared-owner-ok"), "{stdout}");
}
