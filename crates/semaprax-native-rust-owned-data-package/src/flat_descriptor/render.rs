use super::{Descriptor, FieldKind, API_SCHEMA, PROJECT_SCHEMA};

pub(super) fn canonical(descriptor: &Descriptor) -> Vec<u8> {
    let mut output = String::new();
    output.push_str("{\"schema\":");
    super::super::descriptor::json_string(&mut output, API_SCHEMA);
    output.push_str(",\"project_schema\":");
    super::super::descriptor::json_string(&mut output, PROJECT_SCHEMA);
    output.push_str(",\"project_revision\":");
    super::super::descriptor::json_string(&mut output, &descriptor.project_revision);
    output.push_str(",\"workspace_revision\":");
    super::super::descriptor::json_string(&mut output, &descriptor.workspace_revision);
    output.push_str(",\"project_graph_digest\":");
    super::super::descriptor::json_string(&mut output, &descriptor.project_graph_digest);
    output.push_str(",\"exports\":[");
    for (export_index, export) in descriptor.exports.iter().enumerate() {
        if export_index != 0 {
            output.push(',');
        }
        output.push_str("{\"stable_id\":");
        super::super::descriptor::json_string(&mut output, &export.stable_id);
        output.push_str(",\"typescript_name\":");
        super::super::descriptor::json_string(&mut output, &export.stable_id);
        output.push_str(",\"rust_method_name\":");
        super::super::descriptor::json_string(&mut output, &export.rust_method_name);
        output.push_str(",\"parameters\":[");
        for (ordinal, parameter) in export.parameters.iter().enumerate() {
            if ordinal != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            super::super::descriptor::json_string(&mut output, &parameter.stable_id);
            output.push_str(",\"source_name\":");
            super::super::descriptor::json_string(&mut output, &parameter.source_name);
            output.push_str(",\"ordinal\":");
            output.push_str(&ordinal.to_string());
            output.push_str(",\"type\":");
            super::super::descriptor::json_string(&mut output, parameter.kind.wire_name());
            output.push('}');
        }
        output.push_str("],\"result\":{\"type\":\"flat-owned-record\",\"record_id\":");
        super::super::descriptor::json_string(&mut output, &export.record_id);
        output.push_str(",\"record_source_name\":");
        super::super::descriptor::json_string(&mut output, &export.record_source_name);
        output.push_str(",\"record_host_name\":");
        super::super::descriptor::json_string(&mut output, &export.record_host_name);
        output.push_str(",\"fields\":[");
        for (ordinal, field) in export.fields.iter().enumerate() {
            if ordinal != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            super::super::descriptor::json_string(&mut output, &field.stable_id);
            output.push_str(",\"source_name\":");
            super::super::descriptor::json_string(&mut output, &field.source_name);
            output.push_str(",\"host_name\":");
            super::super::descriptor::json_string(&mut output, &field.host_name);
            output.push_str(",\"ordinal\":");
            output.push_str(&ordinal.to_string());
            output.push_str(",\"type\":");
            super::super::descriptor::json_string(
                &mut output,
                match field.kind {
                    FieldKind::I64 => "i64",
                    FieldKind::Bool => "bool",
                    FieldKind::Usize => "usize",
                    FieldKind::OwnedBytes => "owned-bytes",
                },
            );
            output.push('}');
        }
        output.push_str("]}}");
    }
    output.push_str("],\"limits\":{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_record_fields\":64,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576},\"settlement\":{\"carrier\":\"opaque-handle-plus-scalars.v1\",\"copy_before_settle\":true,\"publish_after_settle\":true,\"exactly_one_owned_field\":true}}\n");
    output.into_bytes()
}
