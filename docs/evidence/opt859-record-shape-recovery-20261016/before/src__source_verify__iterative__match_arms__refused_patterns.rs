//! Declaration-only pattern facts for a scrutinee whose check returned no value.
//! This ordinary diagnostic recovery storage grants no type or ownership fact.
use crate::ast::{
    FieldDeclaration, MatchPattern, RecordMatchFieldPattern, RecordMatchPatternField, Span,
    TypeDeclarationKind,
};
use crate::source_verify::{
    diagnostics::{error, source_identifier},
    IterativeVerifier,
};
use std::collections::HashSet;

enum Task<'p> {
    Pattern(&'p MatchPattern, bool),
    Record(&'p str, &'p [RecordMatchPatternField], Span),
    Fields {
        name: &'p str,
        fields: &'p [RecordMatchPatternField],
        declared: Option<&'p [FieldDeclaration]>,
        index: usize,
        supplied: HashSet<&'p str>,
        span: Span,
    },
}

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    pub(super) fn prepare_refused_match_pattern(
        &mut self,
        pattern: &'p MatchPattern,
        scope: usize,
        origin: usize,
        covered: &mut HashSet<String>,
        wildcard_seen: bool,
        unconditional: bool,
    ) {
        let mut tasks = vec![Task::Pattern(pattern, false)];
        let mut names = HashSet::new();
        while let Some(task) = tasks.pop() {
            match task {
                Task::Pattern(
                    MatchPattern::Variant {
                        type_name,
                        case_name,
                        fields,
                        span,
                        ..
                    },
                    in_or,
                ) => {
                    let cases =
                        self.types.declaration(type_name).and_then(
                            |declaration| match &declaration.kind {
                                TypeDeclarationKind::Variant { cases } => Some(cases.as_slice()),
                                _ => None,
                            },
                        );
                    let case =
                        cases.and_then(|cases| cases.iter().find(|case| case.name == *case_name));
                    if case.is_none() {
                        self.diagnostics.push(error(self.program, "SPX-M103", format!("pattern `{type_name}::{case_name}` is incompatible with the match scrutinee"), *span));
                    } else {
                        // Full authored pattern identity, not an inferred scrutinee type.
                        let key = format!("{type_name}::{case_name}");
                        if wildcard_seen
                            || covered.contains(&key)
                            || (unconditional && !covered.insert(key))
                        {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-M102",
                                format!("unreachable duplicate case `{type_name}::{case_name}`"),
                                *span,
                            ));
                        }
                    }
                    if in_or
                        && case.is_some_and(|case| !case.fields.is_empty() || !fields.is_empty())
                    {
                        self.diagnostics.push(error(self.program, "SPX-M105", format!("or-pattern alternative `{type_name}::{case_name}` must be a payload-free case without bindings"), *span).with_help("match a case that carries a payload in an arm of its own"));
                    }
                    let mut supplied = HashSet::new();
                    for field in fields {
                        let declared = case.and_then(|case| {
                            case.fields
                                .iter()
                                .find(|candidate| candidate.name == field.name)
                        });
                        if !supplied.insert(field.name.as_str())
                            || (case.is_some() && declared.is_none())
                        {
                            self.diagnostics.push(error(self.program, "SPX-M104", format!("unknown or duplicate pattern field `{}` in `{type_name}::{case_name}`", field.name), field.span));
                        }
                        if !source_identifier(&field.binding)
                            || !names.insert(field.binding.as_str())
                            || self.scopes[scope].bindings.contains_key(&field.binding)
                            || self.recovery.reads_refused(scope, &field.binding)
                        {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-M104",
                                format!("invalid or duplicate pattern binding `{}`", field.binding),
                                field.binding_span,
                            ));
                        } else {
                            self.recovery.refused_origin(scope, &field.binding, origin);
                        }
                    }
                    if let Some(case) = case {
                        for field in &case.fields {
                            if !supplied.contains(field.name.as_str()) {
                                self.diagnostics.push(error(self.program, "SPX-M104", format!("pattern `{type_name}::{case_name}` is missing payload field `{}`", field.name), *span));
                            }
                        }
                    }
                }
                Task::Pattern(
                    MatchPattern::Record {
                        type_name,
                        fields,
                        span,
                        ..
                    },
                    _,
                ) => tasks.push(Task::Record(type_name, fields, *span)),
                Task::Pattern(MatchPattern::Or { alternatives, .. }, _) => {
                    // Match checking, including independent alternatives, stays source ordered.
                    for alternative in alternatives.iter().rev() {
                        tasks.push(Task::Pattern(alternative, true));
                    }
                }
                Task::Pattern(MatchPattern::Binding { name, span }, _) => {
                    if !source_identifier(name)
                        || !names.insert(name.as_str())
                        || self.scopes[scope].bindings.contains_key(name)
                        || self.recovery.reads_refused(scope, name)
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-M104",
                            format!("invalid or duplicate pattern binding `{name}`"),
                            *span,
                        ));
                    } else {
                        self.recovery.refused_origin(scope, name, origin);
                    }
                }
                Task::Pattern(MatchPattern::Wildcard { .. }, _) => {}
                Task::Pattern(MatchPattern::Literal { .. }, _) => {}
                Task::Record(name, fields, span) => {
                    let declared = self.types.declaration(name).and_then(|declaration| {
                        match &declaration.kind {
                            TypeDeclarationKind::Record { fields } => Some(fields.as_slice()),
                            TypeDeclarationKind::Class { fields, .. } => Some(
                                self.types
                                    .merged_class_fields
                                    .get(name)
                                    .map_or(fields.as_slice(), Vec::as_slice),
                            ),
                            _ => None,
                        }
                    });
                    if declared.is_none() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-M103",
                            format!("record pattern `{name}` has no declared record"),
                            span,
                        ));
                    }
                    tasks.push(Task::Fields {
                        name,
                        fields,
                        declared,
                        index: 0,
                        supplied: HashSet::new(),
                        span,
                    });
                }
                Task::Fields {
                    name,
                    fields,
                    declared,
                    index,
                    mut supplied,
                    span,
                } => {
                    let Some(field) = fields.get(index) else {
                        if let Some(declared) = declared {
                            for field in declared {
                                if !supplied.contains(field.name.as_str()) {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-M104",
                                        format!(
                                            "record pattern `{name}` is missing field `{}`",
                                            field.name
                                        ),
                                        span,
                                    ));
                                }
                            }
                        }
                        continue;
                    };
                    if !supplied.insert(field.name.as_str())
                        || declared.is_some_and(|declared| {
                            !declared
                                .iter()
                                .any(|candidate| candidate.name == field.name)
                        })
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-M104",
                            format!(
                                "unknown or duplicate record pattern field `{}.{}`",
                                name, field.name
                            ),
                            field.span,
                        ));
                    }
                    tasks.push(Task::Fields {
                        name,
                        fields,
                        declared,
                        index: index + 1,
                        supplied,
                        span,
                    });
                    match &field.pattern {
                        RecordMatchFieldPattern::Binding { name, span } => {
                            if !source_identifier(name)
                                || !names.insert(name.as_str())
                                || self.scopes[scope].bindings.contains_key(name)
                                || self.recovery.reads_refused(scope, name)
                            {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-M104",
                                    format!("invalid or duplicate record pattern binding `{name}`"),
                                    *span,
                                ));
                            } else {
                                self.recovery.refused_origin(scope, name, origin);
                            }
                        }
                        RecordMatchFieldPattern::Record {
                            type_name,
                            fields,
                            span,
                            ..
                        } => tasks.push(Task::Record(type_name, fields, *span)),
                        RecordMatchFieldPattern::Wildcard { .. } => {}
                    }
                }
            }
        }
    }
}
