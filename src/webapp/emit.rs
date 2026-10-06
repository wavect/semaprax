//! `schema.js` emission for a built webapp model.

use super::model::Model;
use super::translate::js_string;
use super::{Counts, Ty};

fn list(out: &mut String, name: &str, items: &[String]) {
    out.push_str(",\n    ");
    out.push_str(name);
    out.push_str(": [");
    for item in items {
        out.push_str("\n      ");
        out.push_str(item);
        out.push(',');
    }
    if !items.is_empty() {
        out.push_str("\n    ");
    }
    out.push(']');
}

fn optional(out: &mut String, name: &str, value: &Option<String>) {
    out.push_str(",\n    ");
    out.push_str(name);
    out.push_str(": ");
    out.push_str(value.as_deref().unwrap_or("null"));
}

/// The `schema.js` body after its header, and the counts it contains.
pub(super) fn schema(model: &Model, module: &str, title: &str) -> (String, Counts) {
    let mut out = String::from("import * as rt from \"./runtime.js\";\n");
    out.push_str(&model.helpers);
    out.push_str("export const app = { module: ");
    out.push_str(&js_string(module));
    out.push_str(", title: ");
    out.push_str(&js_string(title));
    out.push_str(" };\nexport const enums = {");
    for (index, (name, cases)) in model.enums.iter().enumerate() {
        out.push_str(if index == 0 { "\n  " } else { ",\n  " });
        out.push_str(name);
        out.push_str(": [");
        let quoted: Vec<String> = cases.iter().map(|case| js_string(case)).collect();
        out.push_str(&quoted.join(", "));
        out.push(']');
    }
    out.push_str("\n};\nexport const account = ");
    out.push_str(model.account.as_deref().unwrap_or("null"));
    out.push_str(";\nexport const entities = [");
    let mut counts = Counts {
        entities: model.entities.len(),
        enums: model.enums.len(),
        accounts: model.account.is_some(),
        ..Counts::default()
    };
    for entity in &model.entities {
        counts.rules += entity.rules.len();
        counts.computed += entity.computed.len();
        counts.keys += entity.keys.len();
        counts.workflows += entity.steps.len();
        counts.rollups += entity.rollups.len();
        counts.permissions +=
            usize::from(entity.can_read.is_some()) + usize::from(entity.can_write.is_some());
        let label = entity
            .fields
            .iter()
            .find(|field| field.ty == Ty::Str)
            .map_or("id", |field| field.name.as_str());
        out.push_str("\n  {\n    name: ");
        out.push_str(&js_string(&entity.name));
        out.push_str(", path: ");
        out.push_str(&js_string(&entity.path));
        out.push_str(", label: ");
        out.push_str(&js_string(label));
        let fields: Vec<String> = entity
            .fields
            .iter()
            .map(|field| {
                let kind = match (&field.reference, &field.ty) {
                    (Some(target), _) => format!("type: \"ref\", ref: {}", js_string(target)),
                    (None, Ty::Enum(name)) => format!("type: \"enum\", enum: {}", js_string(name)),
                    (None, ty) => format!("type: \"{}\"", ty.js_name()),
                };
                format!("{{ name: {}, {kind} }}", js_string(&field.name))
            })
            .collect();
        list(&mut out, "fields", &fields);
        list(&mut out, "rules", &entity.rules);
        list(&mut out, "computed", &entity.computed);
        list(&mut out, "keys", &entity.keys);
        list(&mut out, "steps", &entity.steps);
        let rollups: Vec<String> = entity
            .rollups
            .iter()
            .map(|rollup| {
                format!(
                    "{{ name: {}, kind: \"{}\", child: {}, via: {}, field: {}, type: \"{}\" }}",
                    js_string(&rollup.name),
                    rollup.kind,
                    js_string(&rollup.child),
                    js_string(&rollup.via),
                    rollup.field.as_deref().map_or("null".to_owned(), js_string),
                    rollup.ty.js_name()
                )
            })
            .collect();
        list(&mut out, "rollups", &rollups);
        optional(&mut out, "canRead", &entity.can_read);
        optional(&mut out, "canWrite", &entity.can_write);
        out.push_str(",\n  },");
    }
    out.push_str("\n];\n");
    (out, counts)
}

/// A compact plain-text listing of the generated API: one line per entity
/// with its route, fields, computed fields, keys, workflows, and policies.
pub(super) fn api(model: &Model) -> String {
    let mut out = String::new();
    if let Some((entity, login)) = &model.login {
        out.push_str(&format!(
            "auth: POST /api/session {{\"login\": <{entity}.{login}>, \"password\"}} sets the session cookie; GET /api/session; DELETE /api/session; first run: --setup\n"
        ));
    }
    out.push_str("routes: GET|POST /api/<entity>[?q=&<enum field>=&format=csv]; GET|PUT|DELETE /api/<entity>/<id>; GET /api/<entity>/<id>/history; GET /api/audit\n");
    for entity in &model.entities {
        let fields: Vec<String> = entity
            .fields
            .iter()
            .map(|field| match (&field.reference, &field.ty) {
                (Some(target), _) => format!("{}->{target}", field.name),
                (None, Ty::Enum(name)) => format!("{}:{name}", field.name),
                (None, ty) => format!("{}:{}", field.name, ty.js_name()),
            })
            .collect();
        let policy = |rule: &Option<String>| match rule {
            None => "any",
            Some(text) if text.starts_with("{ row: true") => "row rule",
            Some(_) => "role rule",
        };
        out.push_str(&format!("{} {}", entity.path, fields.join(" ")));
        if !entity.summary.is_empty() {
            out.push_str(&format!(" | {}", entity.summary.join(" ")));
        }
        if !entity.rules.is_empty() {
            out.push_str(&format!(" | {} rules", entity.rules.len()));
        }
        if model.login.is_some() {
            out.push_str(&format!(
                " | read {}, write {}",
                policy(&entity.can_read),
                policy(&entity.can_write)
            ));
        }
        out.push('\n');
    }
    out
}
