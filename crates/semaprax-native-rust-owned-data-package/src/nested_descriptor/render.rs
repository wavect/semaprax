use super::{Descriptor, FieldType, API_SCHEMA, PROJECT_SCHEMA};

pub(super) fn canonical(descriptor: &Descriptor) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("{\"schema\":");
    json(&mut out, API_SCHEMA);
    out.push_str(",\"project_schema\":");
    json(&mut out, PROJECT_SCHEMA);
    for (name, value) in [
        ("project_revision", descriptor.project_revision.as_str()),
        ("workspace_revision", descriptor.workspace_revision.as_str()),
        (
            "project_graph_digest",
            descriptor.project_graph_digest.as_str(),
        ),
    ] {
        out.push_str(",\"");
        out.push_str(name);
        out.push_str("\":");
        json(&mut out, value);
    }
    out.push_str(",\"exports\":[");
    for (ei, e) in descriptor.exports.iter().enumerate() {
        if ei > 0 {
            out.push(',')
        }
        out.push_str("{\"stable_id\":");
        json(&mut out, &e.stable_id);
        out.push_str(",\"typescript_name\":");
        json(&mut out, &e.stable_id);
        out.push_str(",\"rust_method_name\":");
        json(&mut out, &e.rust_method_name);
        out.push_str(",\"parameters\":[");
        for (i, p) in e.parameters.iter().enumerate() {
            if i > 0 {
                out.push(',')
            }
            out.push_str("{\"stable_id\":");
            json(&mut out, &p.stable_id);
            out.push_str(",\"source_name\":");
            json(&mut out, &p.source_name);
            out.push_str(",\"ordinal\":");
            out.push_str(&i.to_string());
            out.push_str(",\"type\":");
            json(&mut out, p.kind.wire_name());
            out.push('}')
        }
        out.push_str("],\"result_record_id\":");
        json(&mut out, &e.result_record_id);
        out.push_str(",\"leaves\":[");
        for (i, l) in e.leaves.iter().enumerate() {
            if i > 0 {
                out.push(',')
            }
            out.push_str("{\"path\":[");
            for (pi, p) in l.path.iter().enumerate() {
                if pi > 0 {
                    out.push(',')
                }
                json(&mut out, p)
            }
            out.push_str("],\"ordinal\":");
            out.push_str(&i.to_string());
            out.push_str(",\"type\":");
            json(&mut out, l.ty.wire_name());
            out.push('}')
        }
        out.push_str("]}")
    }
    out.push_str("],\"records\":[");
    for (ri, r) in descriptor.records.iter().enumerate() {
        if ri > 0 {
            out.push(',')
        }
        out.push_str("{\"stable_id\":");
        json(&mut out, &r.stable_id);
        out.push_str(",\"source_name\":");
        json(&mut out, &r.source_name);
        out.push_str(",\"host_name\":");
        json(&mut out, &r.host_name);
        out.push_str(",\"fields\":[");
        for (fi, f) in r.fields.iter().enumerate() {
            if fi > 0 {
                out.push(',')
            }
            out.push_str("{\"stable_id\":");
            json(&mut out, &f.stable_id);
            out.push_str(",\"source_name\":");
            json(&mut out, &f.source_name);
            out.push_str(",\"host_name\":");
            json(&mut out, &f.host_name);
            out.push_str(",\"ordinal\":");
            out.push_str(&fi.to_string());
            out.push_str(",\"type\":");
            json(&mut out, f.ty.wire_name());
            if let FieldType::Record(id) = &f.ty {
                out.push_str(",\"record_id\":");
                json(&mut out, id)
            }
            out.push('}')
        }
        out.push_str("]}")
    }
    out.push_str("],\"limits\":{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_record_depth\":64,\"max_owned_leaves\":256,\"max_examined_fields\":4096,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576},\"settlement\":{\"carrier\":\"opaque-multi-handle-plus-scalars.v1\",\"preflight_all_handles\":true,\"batch_attach\":true,\"copy_all_before_settle\":true,\"publish_after_settle\":true}}\n");
    out.into_bytes()
}

fn json(out: &mut String, value: &str) {
    use std::fmt::Write as _;
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"')
}
