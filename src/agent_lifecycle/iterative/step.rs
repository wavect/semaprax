//! Closed, declaration-derived flat Step carrier. No caller supplies a transition.
use super::*;
use crate::hir::{DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};

pub(super) struct StepShape {
    pub id: DeclarationId,
    cases: Vec<(
        DeclarationId,
        &'static str,
        Vec<(DeclarationId, DeclarationId)>,
    )>,
    state: DeclarationId,
    result: DeclarationId,
}

impl StepShape {
    pub(super) fn bind(
        program: &hir::ResolvedProgram,
        id: &str,
        state: &str,
        result: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        let persistent = |id: &DeclarationId| {
            program
                .declarations
                .declaration(id)
                .is_some_and(|d| d.identity_origin.is_persistent())
        };
        let declared = program
            .types
            .iter()
            .find(|d| d.id.as_str() == id)
            .ok_or_else(|| vec![bad("step.identity")])?;
        if !declared.type_parameters.is_empty()
            || !persistent(&declared.id)
            || id == state
            || id == result
        {
            return Err(vec![bad("step.identity")]);
        }
        let ResolvedTypeDeclarationKind::Variant { cases } = &declared.kind else {
            return Err(vec![bad("step.variant")]);
        };
        if cases.len() != 4 {
            return Err(vec![bad("step.cases")]);
        }
        let mut mapped = Vec::new();
        for name in ["Continue", "Complete", "Suspend", "Fail"] {
            let case = cases
                .iter()
                .find(|c| c.name == name)
                .ok_or_else(|| vec![bad("step.case")])?;
            if !persistent(&case.id) || case.fields.iter().any(|f| !persistent(&f.id)) {
                return Err(vec![bad("step.field.identity")]);
            }
            let mut fields = Vec::new();
            if name == "Fail" {
                if case.fields.len() != 1 || case.fields[0].ty != ResolvedType::I64 {
                    return Err(vec![bad("step.fail.code")]);
                }
                fields.push((case.fields[0].id.clone(), case.fields[0].id.clone()));
            } else {
                let target = if name == "Complete" { result } else { state };
                let record = program
                    .types
                    .iter()
                    .find(|d| d.id.as_str() == target)
                    .ok_or_else(|| vec![bad("step.record")])?;
                let ResolvedTypeDeclarationKind::Record {
                    fields: target_fields,
                } = &record.kind
                else {
                    return Err(vec![bad("step.record")]);
                };
                if target_fields.len() != case.fields.len() || target_fields.is_empty() {
                    return Err(vec![bad("step.field.count")]);
                }
                for (source, target) in case.fields.iter().zip(target_fields) {
                    if source.ty != target.ty
                        || !matches!(
                            source.ty,
                            ResolvedType::Bytes
                                | ResolvedType::Bool
                                | ResolvedType::I32
                                | ResolvedType::I64
                                | ResolvedType::U8
                                | ResolvedType::Usize
                        )
                    {
                        return Err(vec![bad("step.field.type")]);
                    }
                    fields.push((source.id.clone(), target.id.clone()));
                }
            }
            mapped.push((case.id.clone(), name, fields));
        }
        Ok(Self {
            id: declared.id.clone(),
            cases: mapped,
            state: DeclarationId::new(state),
            result: DeclarationId::new(result),
        })
    }

    pub(super) fn canonical_json(&self) -> String {
        let cases: Vec<_> = self
            .cases
            .iter()
            .map(|(id, role, mapping)| {
                let fields: Vec<_> = mapping
                    .iter()
                    .map(|(source, target)| {
                        format!(
                            "{{\"source\":{},\"target\":{}}}",
                            quote_json(source.as_str()),
                            quote_json(target.as_str())
                        )
                    })
                    .collect();
                format!(
                    "{{\"role\":{},\"case\":{},\"fields\":[{}]}}",
                    quote_json(role),
                    quote_json(id.as_str()),
                    fields.join(",")
                )
            })
            .collect();
        format!(
            "{{\"type\":{},\"state\":{},\"result\":{},\"cases\":[{}]}}",
            quote_json(self.id.as_str()),
            quote_json(self.state.as_str()),
            quote_json(self.result.as_str()),
            cases.join(",")
        )
    }

    pub(super) fn decode(
        &self,
        value: RetainedValue,
    ) -> Result<(&'static str, RetainedValue), Vec<Diagnostic>> {
        let RetainedValue::Variant(value) = value else {
            return Err(vec![bad("step.value")]);
        };
        if value.variant != self.id {
            return Err(vec![bad("step.value.identity")]);
        }
        let (_, name, mapping) = self
            .cases
            .iter()
            .find(|(id, _, _)| *id == value.case)
            .ok_or_else(|| vec![bad("step.value.case")])?;
        if value.fields.len() != mapping.len() {
            return Err(vec![bad("step.value.fields")]);
        }
        let mut fields = Vec::new();
        for (source, target) in mapping {
            let matches: Vec<_> = value.fields.iter().filter(|f| f.field == *source).collect();
            if matches.len() != 1 {
                return Err(vec![bad("step.value.field")]);
            }
            fields.push(RetainedField {
                field: target.clone(),
                value: matches[0].value.clone(),
            });
        }
        if *name == "Fail" {
            return Ok((name, fields.remove(0).value));
        }
        Ok((
            name,
            RetainedValue::Record(RetainedRecord {
                record: if *name == "Complete" {
                    self.result.clone()
                } else {
                    self.state.clone()
                },
                fields,
            }),
        ))
    }
}
