//! Tuple-specific identities over the shared exact package observations.
#[path = "../support/owned_bytes_package.rs"]
mod shared;
pub(super) use shared::{read, verify};

pub(super) fn identities(bytes: &[u8], flat: bool) {
    let descriptor: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(
        descriptor["schema"],
        if flat {
            "semaprax.public-flat-owned-record-api.v1"
        } else {
            "semaprax.public-owned-data-api.v1"
        }
    );
    let exports = descriptor["exports"].as_array().unwrap();
    let ids: &[&str] = if flat {
        &["tuple.bytes", "tuple.text"]
    } else {
        &["tuple.bytes", "tuple.maybe", "tuple.result", "tuple.text"]
    };
    assert_eq!(exports.len(), ids.len());
    for (export, id) in exports.iter().zip(ids) {
        assert_eq!(export["stable_id"], *id);
        assert_eq!(
            export["rust_method_name"],
            format!("spx_tuple_dot_{}", id.strip_prefix("tuple.").unwrap())
        );
        let parameters = export["parameters"].as_array().unwrap();
        let variant = matches!(*id, "tuple.maybe" | "tuple.result");
        assert_eq!(parameters.len(), if variant { 4 } else { 3 });
        for (parameter, ty) in
            parameters
                .iter()
                .zip(["borrow-str", "borrow-slice-u8", "borrow-slice-u8", "bool"])
        {
            assert_eq!(parameter["type"], ty);
        }
        if flat {
            assert_eq!(export["result"]["record_id"], "tuple.Record");
            assert_eq!(
                export["result"]["record_host_name"],
                "SpxRecordId7475706c652e5265636f7264"
            );
            let fields = export["result"]["fields"].as_array().unwrap();
            assert_eq!(fields.len(), 4);
            for (ordinal, ((field, id), ty)) in fields
                .iter()
                .zip(["bytes", "text", "left", "right"])
                .zip(["owned-bytes", "usize", "usize", "usize"])
                .enumerate()
            {
                assert_eq!(field["stable_id"], id);
                assert_eq!(field["type"], ty);
                assert_eq!(field["ordinal"], ordinal);
            }
        } else {
            assert_eq!(
                export["result"],
                match *id {
                    "tuple.maybe" => "option-owned-bytes",
                    "tuple.result" => "result-owned-bytes-i64",
                    _ => "owned-bytes",
                }
            );
        }
    }
    assert_eq!(descriptor["limits"]["max_borrowed_input_bytes"], 65_536);
    assert_eq!(descriptor["limits"]["max_owned_output_bytes"], 65_536);
}
