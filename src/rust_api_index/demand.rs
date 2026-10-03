//! Bounded concrete generic demand resolution and a local Serde projection.
//!
//! The resolver only establishes an exact, deduplicated request identity. Rust
//! itself remains responsible for trait solving when a generated wrapper is
//! compiled. The projection is intentionally nominal and field-by-field: it
//! does not claim a layout relationship with SEMAPRAX storage.

use super::{
    GenericParameterKind, IndexError, ItemKind, RejectionReason, RustApiIndex, Support, Visibility,
    MAX_TYPE_REFERENCES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const MAX_DEMANDED_INSTANTIATIONS: usize = 64;
pub const MAX_CONCRETE_TYPE_BYTES: usize = 256;
pub const MAX_PROJECTED_RECORDS: usize = 128;
pub const MAX_PROJECTED_LABEL_BYTES: usize = 256;

const INSTANTIATION_DOMAIN: &[u8] = b"semaprax.rust-demanded-instantiation.v1\0";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConcreteType(String);

impl ConcreteType {
    pub fn parse(value: &str) -> Result<Self, DemandError> {
        if value.is_empty()
            || value.len() > MAX_CONCRETE_TYPE_BYTES
            || value.split("::").any(|segment| {
                segment.is_empty()
                    || !segment.bytes().enumerate().all(|(index, byte)| {
                        byte.is_ascii_alphanumeric() || byte == b'_' && (index > 0 || byte == b'_')
                    })
                    || !segment.as_bytes()[0].is_ascii_alphabetic() && segment.as_bytes()[0] != b'_'
            })
        {
            return Err(DemandError::MalformedType);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConstArgument {
    pub ty: String,
    pub value: String,
}

impl ConstArgument {
    pub fn decimal(ty: &str, value: &str) -> Result<Self, DemandError> {
        if ty != "usize"
            || value.is_empty()
            || value.len() > 20
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(DemandError::MalformedConst);
        }
        value
            .parse::<u64>()
            .map_err(|_| DemandError::MalformedConst)?;
        Ok(Self {
            ty: ty.to_owned(),
            value: value.to_owned(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstantiationRequest {
    pub item_path: String,
    pub type_arguments: Vec<ConcreteType>,
    pub const_arguments: Vec<ConstArgument>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemandedInstantiation {
    pub item_path: String,
    pub type_arguments: Vec<ConcreteType>,
    pub const_arguments: Vec<ConstArgument>,
    pub identity: String,
}

/// A public associated type projected through one explicit concrete Rust type.
/// The generated wrapper still asks rustc to prove the trait implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeRequest {
    pub associated_type_path: String,
    pub implementor: ConcreteType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemandedAssociatedType {
    pub associated_type_path: String,
    pub implementor: ConcreteType,
    pub projection: String,
    pub identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DemandError {
    Index(IndexError),
    ItemUnavailable,
    UnsupportedItem,
    UnsupportedGeneric,
    AmbiguousArguments,
    MalformedType,
    MalformedConst,
    ExpansionLimit,
}

/// Resolves a finite set of concrete, receiver-free function requests. Equal
/// requests share one identity, while distinct substitutions bind different
/// identities. This does not infer missing arguments or solve trait bounds.
pub fn resolve_demanded_instantiations(
    index: &RustApiIndex,
    requests: &[InstantiationRequest],
) -> Result<Vec<DemandedInstantiation>, DemandError> {
    if requests.is_empty() {
        return Err(DemandError::ExpansionLimit);
    }
    let mut demanded = BTreeMap::new();
    for request in requests {
        let item = index
            .items()
            .iter()
            .find(|item| item.path == request.item_path)
            .ok_or(DemandError::ItemUnavailable)?;
        if item.visibility != Visibility::Public || !item.closure_complete {
            return Err(DemandError::ItemUnavailable);
        }
        if item.kind != ItemKind::Function || !matches!(item.receiver, super::Receiver::None) {
            return Err(DemandError::UnsupportedItem);
        }
        if !matches!(
            item.support,
            Support::Supported
                | Support::Rejected {
                    reason: RejectionReason::UnsupportedGeneric
                }
        ) {
            return Err(DemandError::UnsupportedGeneric);
        }
        if item.reachable_types.len() > MAX_TYPE_REFERENCES {
            return Err(DemandError::ExpansionLimit);
        }
        let type_count = item
            .generics
            .parameters
            .iter()
            .filter(|parameter| parameter.kind == GenericParameterKind::Type)
            .count();
        let const_parameters = item
            .generics
            .parameters
            .iter()
            .filter(|parameter| parameter.kind == GenericParameterKind::Const)
            .collect::<Vec<_>>();
        if item
            .generics
            .parameters
            .iter()
            .any(|parameter| parameter.kind == GenericParameterKind::Lifetime)
            || request.type_arguments.len() != type_count
            || request.const_arguments.len() != const_parameters.len()
        {
            return Err(DemandError::AmbiguousArguments);
        }
        if request
            .const_arguments
            .iter()
            .zip(const_parameters)
            .any(|(argument, parameter)| {
                parameter.const_type.as_deref() != Some(argument.ty.as_str())
            })
        {
            return Err(DemandError::MalformedConst);
        }
        let key = canonical_request(request);
        let identity = instantiation_identity(index.digest(), &key);
        demanded.entry(key).or_insert(DemandedInstantiation {
            item_path: request.item_path.clone(),
            type_arguments: request.type_arguments.clone(),
            const_arguments: request.const_arguments.clone(),
            identity,
        });
        if demanded.len() > MAX_DEMANDED_INSTANTIATIONS {
            return Err(DemandError::ExpansionLimit);
        }
    }
    Ok(demanded.into_values().collect())
}

/// Resolves public, non-generic associated-type projections for explicit
/// implementors. Rust trait solving remains a generated-wrapper obligation.
pub fn resolve_demanded_associated_types(
    index: &RustApiIndex,
    requests: &[AssociatedTypeRequest],
) -> Result<Vec<DemandedAssociatedType>, DemandError> {
    if requests.is_empty() {
        return Err(DemandError::ExpansionLimit);
    }
    let mut demanded = BTreeMap::new();
    for request in requests {
        let item = index
            .items()
            .iter()
            .find(|item| item.path == request.associated_type_path)
            .ok_or(DemandError::ItemUnavailable)?;
        if item.visibility != Visibility::Public || !item.closure_complete {
            return Err(DemandError::ItemUnavailable);
        }
        if item.kind != ItemKind::AssociatedType || item.associated_type.is_none() {
            return Err(DemandError::UnsupportedItem);
        }
        if !item.generics.parameters.is_empty()
            || !item.generics.where_predicates.is_empty()
            || !matches!(
                item.support,
                Support::Supported
                    | Support::Rejected {
                        reason: RejectionReason::UnsupportedSignature
                    }
            )
        {
            return Err(DemandError::UnsupportedGeneric);
        }
        let (trait_path, name) = request
            .associated_type_path
            .rsplit_once("::")
            .ok_or(DemandError::UnsupportedItem)?;
        let projection = format!("<{} as {trait_path}>::{name}", request.implementor.as_str());
        let key = format!(
            "assoc:{}<{}>",
            request.associated_type_path,
            request.implementor.as_str()
        );
        let identity = instantiation_identity(index.digest(), &key);
        demanded.entry(key).or_insert(DemandedAssociatedType {
            associated_type_path: request.associated_type_path.clone(),
            implementor: request.implementor.clone(),
            projection,
            identity,
        });
        if demanded.len() > MAX_DEMANDED_INSTANTIATIONS {
            return Err(DemandError::ExpansionLimit);
        }
    }
    Ok(demanded.into_values().collect())
}

fn canonical_request(request: &InstantiationRequest) -> String {
    let types = request
        .type_arguments
        .iter()
        .map(ConcreteType::as_str)
        .collect::<Vec<_>>()
        .join(",");
    let constants = request
        .const_arguments
        .iter()
        .map(|argument| format!("{}={}", argument.ty, argument.value))
        .collect::<Vec<_>>()
        .join(",");
    format!("{}<{types}>[{constants}]", request.item_path)
}

fn instantiation_identity(index_digest: &str, request: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(INSTANTIATION_DOMAIN);
    for value in [index_digest.as_bytes(), request.as_bytes()] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value);
    }
    format!(
        "sha256:{}",
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedRecord {
    pub id: i64,
    pub label: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ProjectedRecordMirror {
    id: i64,
    label: String,
    enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    Json,
    Domain(&'static str),
    Limit,
}

/// A nominal local mirror used by generated Rust adapters. Values move through
/// this conversion one field at a time; no SEMAPRAX layout is exposed to Serde.
pub fn projected_records_from_json(input: &str) -> Result<Vec<ProjectedRecord>, ProjectionError> {
    let mirrors = serde_json::from_str::<Vec<ProjectedRecordMirror>>(input)
        .map_err(|_| ProjectionError::Json)?;
    if mirrors.len() > MAX_PROJECTED_RECORDS {
        return Err(ProjectionError::Limit);
    }
    mirrors.into_iter().map(project_record).collect()
}

pub fn projected_records_to_json(records: &[ProjectedRecord]) -> Result<String, ProjectionError> {
    if records.len() > MAX_PROJECTED_RECORDS {
        return Err(ProjectionError::Limit);
    }
    let mirrors = records
        .iter()
        .cloned()
        .map(|record| {
            validate_record(&record)?;
            Ok(ProjectedRecordMirror {
                id: record.id,
                label: record.label,
                enabled: record.enabled,
            })
        })
        .collect::<Result<Vec<_>, ProjectionError>>()?;
    serde_json::to_string(&mirrors).map_err(|_| ProjectionError::Json)
}

/// First admitted collection pipeline: retain enabled records, preserving their
/// source order and ownership through the local projection.
pub fn enabled_projected_records(records: Vec<ProjectedRecord>) -> Vec<ProjectedRecord> {
    records
        .into_iter()
        .filter(|record| record.enabled)
        .collect()
}

fn project_record(mirror: ProjectedRecordMirror) -> Result<ProjectedRecord, ProjectionError> {
    let record = ProjectedRecord {
        id: mirror.id,
        label: mirror.label,
        enabled: mirror.enabled,
    };
    validate_record(&record)?;
    Ok(record)
}

fn validate_record(record: &ProjectedRecord) -> Result<(), ProjectionError> {
    if record.id < 0 {
        return Err(ProjectionError::Domain("id"));
    }
    if record.label.is_empty() || record.label.len() > MAX_PROJECTED_LABEL_BYTES {
        return Err(ProjectionError::Domain("label"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::process::Command;

    const INDEX: &[u8] = include_bytes!(
        "../../crates/semaprax-rust-api-index/fixtures/protocol-envelope-example.json"
    );

    fn generic_index() -> RustApiIndex {
        let mut value: Value = serde_json::from_slice(INDEX).unwrap();
        let item = value["items"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|item| item["path"] == "local_api_fixture::generic_output")
            .unwrap();
        item["closure_complete"] = Value::Bool(true);
        item["support"] = Value::String("rejected".to_owned());
        item["reason"] = Value::String("unsupported_generic".to_owned());
        value["types"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|record| record["path"] == "core::clone::Clone")
            .unwrap()["kind"] = Value::String("struct".to_owned());
        value["types"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|record| record["path"] == "core::clone::Clone")
            .unwrap()["visibility"] = Value::String("public".to_owned());
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        RustApiIndex::replay(&bytes).unwrap()
    }

    fn const_and_associated_index() -> RustApiIndex {
        let mut value: Value = serde_json::from_slice(INDEX).unwrap();
        let generic = value["items"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|item| item["path"] == "local_api_fixture::generic_output")
            .unwrap();
        generic["path"] = Value::String("local_api_fixture::const_repeat".into());
        generic["signature"] =
            Value::String("fn const_repeat<const N: usize>(value: u8) -> [u8; N]".into());
        generic["closure_complete"] = Value::Bool(true);
        generic["reachable_types"] = Value::Array(Vec::new());
        generic["type_roots"] = Value::Array(Vec::new());
        generic["type_closure_depth"] = Value::from(0);
        generic["generics"] = serde_json::json!({"parameters":[{"bounds":[],"const_type":"usize","default":null,"kind":"const","name":"N"}],"where_predicates":[]});
        value["items"]
            .as_array_mut()
            .unwrap()
            .sort_by(|left, right| {
                left["path"]
                    .as_str()
                    .unwrap()
                    .cmp(right["path"].as_str().unwrap())
            });
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        RustApiIndex::replay(&bytes).unwrap()
    }

    fn compile_and_run(source: &str, label: &str) -> std::process::Output {
        let root = std::env::temp_dir().join(format!(
            "semaprax-ri07-demand-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("main.rs");
        let executable = root.join("fixture");
        std::fs::write(&path, source).unwrap();
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(rustc)
            .args(["--edition=2021"])
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        if output.status.success() {
            let run = Command::new(&executable).output().unwrap();
            assert!(run.status.success(), "physical generated fixture failed");
        }
        std::fs::remove_dir_all(root).unwrap();
        output
    }

    #[test]
    fn demanded_concrete_generics_reuse_equal_requests_and_separate_types() {
        let request = |ty| InstantiationRequest {
            item_path: "local_api_fixture::generic_output".to_owned(),
            type_arguments: vec![ConcreteType::parse(ty).unwrap()],
            const_arguments: Vec::new(),
        };
        let demanded = resolve_demanded_instantiations(
            &generic_index(),
            &[
                request("semaprax::ProjectedRecord"),
                request("i64"),
                request("semaprax::ProjectedRecord"),
            ],
        )
        .unwrap();
        assert_eq!(demanded.len(), 2);
        assert_ne!(demanded[0].identity, demanded[1].identity);
        assert!(demanded
            .iter()
            .any(|entry| entry.type_arguments[0].as_str() == "i64"));
    }

    #[test]
    fn missing_or_malformed_concrete_arguments_fail_closed() {
        let index = generic_index();
        assert_eq!(
            ConcreteType::parse("Vec<i64>"),
            Err(DemandError::MalformedType)
        );
        assert_eq!(
            ConstArgument::decimal("usize", "01"),
            Err(DemandError::MalformedConst)
        );
        assert_eq!(
            resolve_demanded_instantiations(
                &index,
                &[InstantiationRequest {
                    item_path: "local_api_fixture::generic_output".to_owned(),
                    type_arguments: Vec::new(),
                    const_arguments: Vec::new(),
                }],
            ),
            Err(DemandError::AmbiguousArguments)
        );
    }

    #[test]
    fn real_serde_json_mirror_round_trip_and_collection_pipeline() {
        let records = projected_records_from_json(
            r#"[{"id":7,"label":"first","enabled":true},{"id":8,"label":"second","enabled":false}]"#,
        )
        .unwrap();
        let enabled = enabled_projected_records(records);
        assert_eq!(enabled.len(), 1);
        assert_eq!(
            projected_records_to_json(&enabled).unwrap(),
            r#"[{"id":7,"label":"first","enabled":true}]"#
        );
        assert_eq!(
            projected_records_from_json(r#"[{"id":-1,"label":"no","enabled":true}]"#),
            Err(ProjectionError::Domain("id"))
        );
        assert_eq!(
            projected_records_from_json("not-json"),
            Err(ProjectionError::Json)
        );
    }
    #[test]
    fn const_and_associated_demands_compile_and_execute_with_trait_bound_refusal() {
        let index = const_and_associated_index();
        let const_request = InstantiationRequest {
            item_path: "local_api_fixture::const_repeat".into(),
            type_arguments: Vec::new(),
            const_arguments: vec![ConstArgument::decimal("usize", "3").unwrap()],
        };
        let repeated =
            resolve_demanded_instantiations(&index, &[const_request.clone(), const_request])
                .unwrap();
        assert_eq!(repeated.len(), 1);
        let associated = resolve_demanded_associated_types(
            &index,
            &[AssociatedTypeRequest {
                associated_type_path: "local_api_fixture::Measures::Output".into(),
                implementor: ConcreteType::parse("local_api_fixture::ReExported").unwrap(),
            }],
        )
        .unwrap();
        assert_eq!(
            associated[0].projection,
            "<local_api_fixture::ReExported as local_api_fixture::Measures>::Output"
        );

        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            if env!("CARGO_PKG_NAME") == "semaprax" {
                "crates/semaprax-rust-api-index/fixtures/local_api_fixture.rs"
            } else {
                "fixtures/local_api_fixture.rs"
            },
        );
        let fixture = fixture.display().to_string().replace('\\', "\\\\");
        let projection = &associated[0].projection;
        let positive = format!(
            "#[path=\"{fixture}\"] mod local_api_fixture;\nfn demand_const<const N:usize>(value:u8)->[u8;N]{{local_api_fixture::const_repeat::<N>(value)}}\nfn demand_associated(value:&local_api_fixture::ReExported)->{projection}{{local_api_fixture::Measures::measure(value)}}\nfn main(){{assert_eq!(demand_const::<3>(7),[7,7,7]);assert_eq!(demand_associated(&local_api_fixture::ReExported(9)),9);}}\n"
        );
        assert!(compile_and_run(&positive, "positive").status.success());

        let negative = format!(
            "#[path=\"{fixture}\"] mod local_api_fixture;\nstruct NotClone;\nfn main(){{let _=local_api_fixture::generic_output(NotClone);}}\n"
        );
        let failure = compile_and_run(&negative, "trait-bound");
        assert!(!failure.status.success());
        let stderr = String::from_utf8_lossy(&failure.stderr);
        assert!(
            stderr.contains("Clone") && stderr.contains("generic_output"),
            "{stderr}"
        );

        let sealed = resolve_demanded_associated_types(
            &index,
            &[AssociatedTypeRequest {
                associated_type_path: "local_api_fixture::SealedApi::HiddenOutput".into(),
                implementor: ConcreteType::parse("local_api_fixture::SealedType").unwrap(),
            }],
        );
        assert_eq!(sealed, Err(DemandError::ItemUnavailable));
    }

    #[test]
    fn distinct_demand_expansion_limit_is_deterministic() {
        let index = generic_index();
        let requests = (0..=MAX_DEMANDED_INSTANTIATIONS)
            .map(|value| InstantiationRequest {
                item_path: "local_api_fixture::generic_output".into(),
                type_arguments: vec![ConcreteType::parse(&format!("T{value}")).unwrap()],
                const_arguments: Vec::new(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            resolve_demanded_instantiations(&index, &requests),
            Err(DemandError::ExpansionLimit)
        );
    }
}
