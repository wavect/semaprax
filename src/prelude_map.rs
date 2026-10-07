//! Prelude v13 composes frozen v1-v12 meanings with typed Map/Set ownership.
use crate::ast::{Span, TypeDeclaration, TypeDeclarationKind, TypeParameterDeclaration};
use sha2::{Digest, Sha256};
pub(crate) fn declaration(set: bool) -> TypeDeclaration {
    TypeDeclaration {
        stable_id: if set {
            crate::map_ops::SET_ID
        } else {
            crate::map_ops::MAP_ID
        }
        .into(),
        explicit_id: true,
        name: if set { "Set" } else { "Map" }.into(),
        name_span: Span::default(),
        type_parameters: if set { vec!["K"] } else { vec!["K", "V"] }
            .into_iter()
            .map(|name| TypeParameterDeclaration {
                name: name.into(),
                span: Span::default(),
            })
            .collect(),
        kind: TypeDeclarationKind::Record { fields: Vec::new() },
        extends: None,
        invariants: None,
        span: Span::default(),
    }
}
pub(crate) fn contract_bytes_v13() -> Vec<u8> {
    let old = crate::prelude::contract_bytes_v12();
    let mut bytes = std::str::from_utf8(&old)
        .expect("prelude is UTF-8")
        .replacen(crate::prelude::SCHEMA_V12, crate::prelude::SCHEMA_V13, 1)
        .into_bytes();
    bytes.extend_from_slice(b"record core.collection.map.v2 Map<K,V>\nrecord core.collection.set.v2 Set<K>\nrepresentation collections opaque_owned_sorted_entries\nkeys string,i64,bool order unsigned_UTF8,signed_numeric,false_before_true\nvalues string,i64,i32,u8,usize,char,f32,f64,bool\nrule collection scalar_or_String_values copied_on_insert_and_read no_owner_aliasing\nrule collection capacity_max:65536 remove_missing_noop same_owner_reopen atomically_committed\ncleanup_leaf core.collection.drop.v2\nstatus semaprax.map.v2 full:1,index:2,capacity:3,overflow:4\n");
    for op in crate::map_ops::MapOp::ALL {
        bytes.extend_from_slice(
            format!(
                "operation {} {} generic_parameters:{} arguments:{}\n",
                op.id(),
                op.name(),
                op.type_arity(),
                op.arity()
            )
            .as_bytes(),
        );
    }
    bytes
}
pub(crate) fn digest_text_v13() -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes_v13()))
    )
}
