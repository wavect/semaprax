//! Bounded concrete generic demand resolution and a local Serde projection.
//!
//! The resolver only establishes an exact, deduplicated request identity. Rust
//! itself remains responsible for trait solving when a generated wrapper is
//! compiled. The projection is intentionally nominal and field-by-field: it
//! does not claim a layout relationship with SEMAPRAX storage.

use crate::{
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
    if requests.is_empty() || requests.len() > MAX_DEMANDED_INSTANTIATIONS {
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
        if item.kind != ItemKind::Function || !matches!(item.receiver, crate::Receiver::None) {
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

    const INDEX: &[u8] = include_bytes!("../fixtures/protocol-envelope-example.json");

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
}
