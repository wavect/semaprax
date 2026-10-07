//! Additive pairwise constraints and explicit field migration projections.
use super::*;

pub(super) fn project<'a>(
    function: &'a Function,
    index: usize,
    suffix: &str,
    kind: &Kind,
    entities: &[Entity],
    enums: &BTreeMap<String, Vec<String>>,
    translator: &mut Translator<'a>,
) -> Result<(bool, String), Vec<Diagnostic>> {
    let entity = &entities[index];
    let mut bound = Vec::new();
    let mut target = None;
    let mut inputs = Vec::new();
    let migration = matches!(kind, Kind::Migration(_));
    let help = "`constraint[_name]` returns bool and takes row fields, `self_id: i64`, and `other_<entity>_<field>` / `other_<entity>_id` from exactly one entity; `migrate_<field>` returns that field's type and takes scalar `old_<field>` parameters (zero parameters supplies a new field's default)";
    let error = |message: String| vec![shape_error(message, function.name_span, help)];
    for param in &function.params {
        let Some(ty) = scalar(&param.ty, enums).filter(|_| param.mode == ParamMode::Value) else {
            return Err(error(format!("v3 parameter `{}` must be a value scalar", param.name)));
        };
        let js = if migration {
            let Some(field) = param.name.strip_prefix("old_").filter(|s| !s.is_empty() && *s != "password") else {
                return Err(error(format!("migration parameter `{}` must name an old field", param.name)));
            };
            let descriptor = match &ty {
                Ty::Enum(name) => format!("type: \"enum\", enum: {}", translate::js_string(name)),
                _ => format!("type: {}", translate::js_string(ty.js_name())),
            };
            inputs.push(format!("{{ name: {}, {descriptor} }}", translate::js_string(field)));
            format!("o.{field}")
        } else if param.name == "self_id" && ty == Ty::Int {
            "r.id".to_owned()
        } else if entity.fields.iter().any(|f| f.name == param.name && f.ty == ty) {
            format!("r.{}", param.name)
        } else if let Some(other) = param.name.strip_prefix("other_") {
            let Some((other_index, length)) = owning_entity(entities, other) else {
                return Err(error(format!("constraint parameter `{}` names no entity", param.name)));
            };
            if target.is_some_and(|t| t != other_index) {
                return Err(error("a pairwise constraint must name exactly one other entity".to_owned()));
            }
            target = Some(other_index);
            let field = &other[length + 1..];
            if !(field == "id" && ty == Ty::Int || entities[other_index].fields.iter().any(|f| f.name == field && f.ty == ty)) {
                return Err(error(format!("constraint parameter `{}` has no matching field/type", param.name)));
            }
            format!("o.{field}")
        } else {
            return Err(error(format!("v3 parameter `{}` does not bind", param.name)));
        };
        bound.push(Bound { name: param.name.clone(), js, ty, field: !migration && !param.name.starts_with("other_") });
    }
    if let Kind::Migration(field) = kind {
        let Some(destination) = entity.fields.iter().find(|f| f.name == *field) else {
            return Err(error(format!("migration destination `{field}` is absent")));
        };
        if scalar(&function.return_type, enums).as_ref() != Some(&destination.ty) {
            return Err(error(format!("migration `{}` returns the wrong field type", function.name)));
        }
        let (body, _) = translator.body(function, &bound)?;
        Ok((false, format!("{{ field: {}, inputs: [{}], value: (o) => {body} }}", translate::js_string(field), inputs.join(", "))))
    } else {
        let Some(target) = target else { return Err(error("a constraint requires other-row parameters".to_owned())); };
        if function.return_type != Type::Bool { return Err(error("a constraint must return bool".to_owned())); }
        let (body, fields) = translator.body(function, &bound)?;
        let fields = fields.iter().map(|f| translate::js_string(f)).collect::<Vec<_>>().join(", ");
        Ok((true, format!("{{ name: {}, other: {}, fields: [{fields}], test: (r, o) => {body} }}", translate::js_string(suffix), translate::js_string(&entities[target].path))))
    }
}
