//! Linked owned-data closure registration for the declaration index.
//!
//! The Project owned-data profiles link one exact closure instead of whole
//! modules, so this route rebuilds the index from the supplied declarations
//! and their Phase-A facts alone. It lives beside its owner so the declaration
//! index stays inside its module-size budget.

use super::super::*;
use super::DeclarationIndex;

impl DeclarationIndex {
    /// Extend the canonical compiler prelude with exactly one authenticated
    /// Project-v8 closure. Every retained declaration must have one matching
    /// Phase-A fact and every supplied fact must be consumed; this prevents
    /// both declaration omission and unrelated whole-module leakage.
    pub(in crate::hir) fn extend_linked_owned_data(
        &mut self,
        types: &[ResolvedTypeDeclaration],
        interfaces: &[ResolvedInterface],
        functions: &[ResolvedFunction],
        templates: &[ResolvedFunctionTemplate],
        facts: &BTreeMap<DeclarationId, LinkedDeclarationFact>,
    ) -> Result<(), Diagnostic> {
        fn require_fact<'a>(
            facts: &'a BTreeMap<DeclarationId, LinkedDeclarationFact>,
            used: &mut BTreeSet<DeclarationId>,
            id: &DeclarationId,
            kind: DeclarationKind,
            owner: Option<&DeclarationId>,
        ) -> Result<&'a LinkedDeclarationFact, Diagnostic> {
            let fact = facts.get(id).ok_or_else(|| {
                Diagnostic::io(
                    "SPX-G173",
                    format!("linked declaration `{id}` has no authenticated Phase-A fact"),
                )
            })?;
            if fact.kind != kind || fact.owner.as_ref() != owner {
                return Err(Diagnostic::io(
                    "SPX-G173",
                    format!("linked declaration `{id}` disagrees with its Phase-A fact"),
                ));
            }
            if !used.insert(id.clone()) {
                return Err(Diagnostic::io(
                    "SPX-G173",
                    format!("linked declaration `{id}` is retained more than once"),
                ));
            }
            Ok(fact)
        }

        let mut used = BTreeSet::new();
        for declaration in types {
            let kind = match &declaration.kind {
                ResolvedTypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                ResolvedTypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
                ResolvedTypeDeclarationKind::Class { .. }
                | ResolvedTypeDeclarationKind::Resource { .. } => {
                    return Err(Diagnostic::io(
                        "SPX-G172",
                        format!(
                            "linked owned-data type `{}` is outside the shared record/variant target profile",
                            declaration.id
                        ),
                    ));
                }
            };
            let fact = require_fact(facts, &mut used, &declaration.id, kind, None)?;
            if self.declarations.contains_key(&declaration.id) {
                return Err(Diagnostic::io(
                    "SPX-G173",
                    format!(
                        "linked type `{}` aliases the compiler prelude",
                        declaration.id
                    ),
                ));
            }
            self.insert_top_level(
                declaration.name.clone(),
                declaration.id.clone(),
                kind,
                fact.origin,
            );
            self.type_parameters
                .insert(declaration.id.clone(), declaration.type_parameters.clone());

            match &declaration.kind {
                ResolvedTypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        let fact = require_fact(
                            facts,
                            &mut used,
                            &field.id,
                            DeclarationKind::Field,
                            Some(&declaration.id),
                        )?;
                        self.insert_field(declaration.id.clone(), field.clone(), fact.origin);
                    }
                    self.record_fields
                        .insert(declaration.id.clone(), fields.clone());
                }
                ResolvedTypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        let fact = require_fact(
                            facts,
                            &mut used,
                            &case.id,
                            DeclarationKind::VariantCase,
                            Some(&declaration.id),
                        )?;
                        self.insert_case(
                            declaration.id.clone(),
                            case.name.clone(),
                            case.id.clone(),
                            fact.origin,
                        );
                        for field in &case.fields {
                            let fact = require_fact(
                                facts,
                                &mut used,
                                &field.id,
                                DeclarationKind::CaseField,
                                Some(&case.id),
                            )?;
                            self.insert_case_field(case.id.clone(), field.clone(), fact.origin);
                        }
                        self.case_fields
                            .insert(case.id.clone(), case.fields.clone());
                    }
                    self.variant_cases
                        .insert(declaration.id.clone(), cases.clone());
                }
                ResolvedTypeDeclarationKind::Class { .. }
                | ResolvedTypeDeclarationKind::Resource { .. } => unreachable!("rejected above"),
            }
        }

        for interface in interfaces {
            let fact = require_fact(
                facts,
                &mut used,
                &interface.id,
                DeclarationKind::Interface,
                None,
            )?;
            if self.declarations.contains_key(&interface.id) {
                return Err(Diagnostic::io(
                    "SPX-G173",
                    format!(
                        "linked interface `{}` aliases the compiler prelude",
                        interface.id
                    ),
                ));
            }
            self.insert_top_level(
                interface.name.clone(),
                interface.id.clone(),
                DeclarationKind::Interface,
                fact.origin,
            );
            for import in &interface.imports {
                let fact = require_fact(
                    facts,
                    &mut used,
                    &import.id,
                    DeclarationKind::Import,
                    Some(&interface.id),
                )?;
                if self
                    .imports_by_key
                    .insert(import.import_key.clone(), import.id.clone())
                    .is_some()
                {
                    return Err(Diagnostic::io(
                        "SPX-G173",
                        "linked import keys are not unique",
                    ));
                }
                if import.native_rust
                    && self
                        .native_rust_imports_by_name
                        .insert(import.name.clone(), import.id.clone())
                        .is_some()
                {
                    return Err(Diagnostic::io(
                        "SPX-G173",
                        "linked Native Rust import names are not unique",
                    ));
                }
                self.insert_owned_declaration(
                    interface.id.clone(),
                    import.name.clone(),
                    import.id.clone(),
                    DeclarationKind::Import,
                    fact.origin,
                );
            }
        }

        for function in functions {
            let fact = require_fact(
                facts,
                &mut used,
                &function.id,
                DeclarationKind::Function,
                None,
            )?;
            if self.declarations.contains_key(&function.id) {
                return Err(Diagnostic::io(
                    "SPX-G173",
                    format!(
                        "linked function `{}` aliases the compiler prelude",
                        function.id
                    ),
                ));
            }
            self.insert_top_level(
                function.name.clone(),
                function.id.clone(),
                DeclarationKind::Function,
                fact.origin,
            );
            self.type_parameters.insert(function.id.clone(), Vec::new());
        }
        // A generic template owns an ordinary function identity; only its
        // canonical type-parameter metadata separates it from a monomorphic
        // declaration, and canonical validation reconstructs that metadata
        // from this index rather than from the retained template body.
        for template in templates {
            let fact = require_fact(
                facts,
                &mut used,
                &template.id,
                DeclarationKind::Function,
                None,
            )?;
            if self.declarations.contains_key(&template.id) {
                return Err(Diagnostic::io(
                    "SPX-G173",
                    format!(
                        "linked generic template `{}` aliases the compiler prelude",
                        template.id
                    ),
                ));
            }
            self.insert_top_level(
                template.name.clone(),
                template.id.clone(),
                DeclarationKind::Function,
                fact.origin,
            );
            self.type_parameters
                .insert(template.id.clone(), template.type_parameters.clone());
        }
        if interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .filter(|import| import.native_rust)
            .any(|import| self.functions_by_name.contains_key(&import.name))
        {
            return Err(Diagnostic::io(
                "SPX-B107",
                "Native Rust Interop declaration set is unsupported: symbol collision",
            ));
        }
        if used.len() != facts.len() {
            return Err(Diagnostic::io(
                "SPX-G173",
                "linked declaration facts contain an unrelated declaration",
            ));
        }
        Ok(())
    }
}
