//! Private OPT847 function presentation for the retained-wrapper producer.
//!
//! A presentation borrows the alias cell in the immutable directory entry. It
//! never borrows a constructor argument, iterator item, or copied FunctionView.
//! The alias controls every name-sensitive consumer; payload() is an explicit
//! boundary for consumers that need original fields. There is no Deref/AsRef.
use super::{FunctionDirectory, FunctionEntry};
use crate::ast::Function;

// Ordinary declaration helpers keep their existing AST input APIs while the
// function-specific receivers migrate. These explicit borrowed methods form
// no presentation, clone no name/body and provide no implicit conversion.
impl Function {
    pub(crate) fn payload(&self) -> &Self {
        self
    }
    #[cfg(test)]
    #[allow(
        dead_code,
        reason = "Retained compatibility entry for independently compiled historical and semantic controls."
    )]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FunctionRef<'a> {
    payload: &'a Function,
    alias_entry: Option<&'a FunctionEntry<'a>>,
}
/// The original capacity owner: a class declaration pointer is sufficient to
/// recover its exact name, leaving the tuple's backing available for the view.
pub(crate) type FunctionCell<'a> = (Option<&'a crate::ast::TypeDeclaration>, FunctionRef<'a>);

impl<'a> FunctionRef<'a> {
    pub(crate) fn ordinary(payload: &'a Function) -> Self {
        Self {
            payload,
            alias_entry: None,
        }
    }
    pub(crate) fn name(&self) -> &'a str {
        match self.alias_entry {
            Some(FunctionEntry::Borrowed(view)) => view._alias,
            Some(FunctionEntry::Owned(_)) => unreachable!("owned entry uses its ordinary payload"),
            None => &self.payload.name,
        }
    }
    /// Original declaration name; local binding lookup continues to use name().
    pub(crate) fn declaration_name(&self) -> &'a str {
        &self.payload.name
    }
    pub(crate) fn payload(&self) -> &'a Function {
        self.payload
    }
    /// Only raw-body classifiers proven false for the finite scalar default.
    /// This does not remove the function from ordinary source/HIR validation.
    pub(crate) fn classifier_payload(&self) -> Option<&'a Function> {
        match self.alias_entry {
            Some(FunctionEntry::Borrowed(super::FunctionView {
                _provider: None, ..
            })) => None,
            _ => Some(self.payload),
        }
    }
    pub(crate) fn is_signature(&self) -> bool {
        matches!(
            self.alias_entry,
            Some(FunctionEntry::Borrowed(super::FunctionView {
                _provider: None,
                ..
            }))
        )
    }
    // Each getter replaces payload() plus the corresponding field loan. Its
    // receiver/return remain two references; no owning AST or extra view forms.
    pub(crate) fn body(&self) -> &'a crate::ast::Expr {
        match self.alias_entry {
            Some(FunctionEntry::Borrowed(super::FunctionView {
                _provider: None, ..
            })) => match self.payload.return_type {
                crate::ast::Type::I64 => &super::scalar_signature::I64,
                crate::ast::Type::I32 => &super::scalar_signature::I32,
                crate::ast::Type::Char => &super::scalar_signature::CHAR,
                crate::ast::Type::U8 => &super::scalar_signature::U8,
                crate::ast::Type::Usize => &super::scalar_signature::USIZE,
                crate::ast::Type::F32 => &super::scalar_signature::F32,
                crate::ast::Type::F64 => &super::scalar_signature::F64,
                crate::ast::Type::Bool => &super::scalar_signature::BOOL,
                crate::ast::Type::String => &super::scalar_signature::STRING,
                _ => unreachable!("signature producer admitted a finite scalar return"),
            },
            _ => &self.payload.body,
        }
    }
    pub(crate) fn requires(&self) -> &'a Vec<crate::ast::Expr> {
        match self.alias_entry {
            Some(FunctionEntry::Borrowed(super::FunctionView {
                _provider: None, ..
            })) => &super::scalar_signature::EMPTY,
            _ => &self.payload.requires,
        }
    }
    pub(crate) fn ensures(&self) -> &'a Vec<crate::ast::Expr> {
        match self.alias_entry {
            Some(FunctionEntry::Borrowed(super::FunctionView {
                _provider: None, ..
            })) => &super::scalar_signature::EMPTY,
            _ => &self.payload.ensures,
        }
    }
    pub(crate) fn yields(&self) -> &'a Option<crate::ast::YieldsClause> {
        match self.alias_entry {
            Some(FunctionEntry::Borrowed(super::FunctionView {
                _provider: None, ..
            })) => &None,
            _ => &self.payload.yields,
        }
    }
    pub(crate) fn follows(&self) -> &'a Option<Box<crate::ast::SessionProtocolFollowsClause>> {
        match self.alias_entry {
            Some(FunctionEntry::Borrowed(super::FunctionView {
                _provider: None, ..
            })) => &None,
            _ => &self.payload.follows,
        }
    }
    pub(crate) fn has_stored_alias(&self) -> bool {
        self.alias_entry.is_some()
    }

    /// A genuine owning transform materializes the effective name once. Raw
    /// payload Clone order is retained and no provider-name String is cloned
    /// only to be discarded. This is never used for lookup or classification.
    pub(crate) fn materialize(&self) -> Function {
        if !self.is_signature() && self.name() == self.payload.name {
            return self.payload.clone();
        }
        let source = self.payload;
        Function {
            stable_id: source.stable_id.clone(),
            explicit_id: source.explicit_id,
            name: self.name().to_owned(),
            name_span: source.name_span,
            type_parameters: source.type_parameters.clone(),
            params: source.params.clone(),
            return_type: source.return_type.clone(),
            effects: source.effects.clone(),
            yields: self.yields().clone(),
            follows: self.follows().clone(),
            requires: self.requires().to_vec(),
            ensures: self.ensures().to_vec(),
            body: self.body().clone(),
            span: source.span,
        }
    }

    /// A genuine validation-only specialization retains this same stored alias
    /// cell while borrowing its owned, substituted signature/body. The new
    /// payload is never a directory member merely because the alias matches.
    pub(crate) fn specialized_payload<'s>(&self, payload: &'s Function) -> FunctionRef<'s>
    where
        'a: 's,
    {
        FunctionRef {
            payload,
            alias_entry: self.alias_entry,
        }
    }

    /// Exact membership of an immutable presentation, including its stored cell.
    /// Declaration equality remains a separate semantic comparison below.
    pub(crate) fn same_presentation(&self, other: &Self) -> bool {
        std::ptr::eq(self.payload, other.payload)
            && match (self.alias_entry, other.alias_entry) {
                (None, None) => true,
                (Some(a), Some(b)) => std::ptr::eq(a, b),
                _ => false,
            }
    }

    /// Stored alias-cell identity, independent of a genuine specialized payload.
    /// This predicate alone grants no membership or producer authority.
    pub(crate) fn same_alias_cell(&self, other: &Self) -> bool {
        match (self.alias_entry, other.alias_entry) {
            (None, None) => true,
            (Some(a), Some(b)) => std::ptr::eq(a, b),
            _ => false,
        }
    }

    /// Historical reuse-environment equality deliberately excludes the body.
    /// Individual lowered-function reuse still compares complete declarations.
    pub(crate) fn same_signature(&self, other: &Self) -> bool {
        self.name() == other.name()
            && self.signature_metadata(other.payload)
            && self.yields() == other.yields()
            && self.follows() == other.follows()
            && self.requires() == other.requires()
            && self.ensures() == other.ensures()
    }
    fn signature_metadata(&self, b: &Function) -> bool {
        let a = self.payload;
        a.stable_id == b.stable_id
            && a.explicit_id == b.explicit_id
            && a.name_span == b.name_span
            && a.type_parameters == b.type_parameters
            && a.params == b.params
            && a.return_type == b.return_type
            && a.effects == b.effects
            && a.span == b.span
    }
    pub(crate) fn same_ordinary_signature(a: &Function, b: &Function) -> bool {
        a.name == b.name && Self::signature_fields(a, b)
    }
    fn signature_fields(a: &Function, b: &Function) -> bool {
        a.stable_id == b.stable_id
            && a.explicit_id == b.explicit_id
            && a.name_span == b.name_span
            && a.type_parameters == b.type_parameters
            && a.params == b.params
            && a.return_type == b.return_type
            && a.effects == b.effects
            && a.yields == b.yields
            && a.follows == b.follows
            && a.requires == b.requires
            && a.ensures == b.ensures
            && a.span == b.span
    }

    /// Equality of the historical renamed Function, without materializing it.
    /// The provider spelling alone is not part of the projected declaration.
    pub(crate) fn same_declaration(&self, other: &Self) -> bool {
        self.same_signature(other) && self.body() == other.body()
    }
    pub(crate) fn same_ordinary_declaration(&self, b: &Function) -> bool {
        self.name() == b.name
            && self.signature_metadata(b)
            && self.yields() == &b.yields
            && self.follows() == &b.follows
            && self.requires() == &b.requires
            && self.ensures() == &b.ensures
            && self.body() == &b.body
    }
}

/// Authored ordinals, then generated directory order, exactly as DeclSlices.
/// This intentionally has no Index implementation: an item is a value whose
/// effective name cannot be obtained by implicit conversion to &Function.
#[derive(Clone, Copy)]
pub(crate) enum FunctionSlices<'a> {
    Ordinary {
        authored: &'a [Function],
        indices: Option<&'a [usize]>,
        generated: &'a [Function],
    },
    Projected {
        authored: &'a [Function],
        directory: &'a FunctionDirectory<'a>,
        start: usize,
    },
}
impl<'a> FunctionSlices<'a> {
    pub(crate) fn one(authored: &'a [Function]) -> Self {
        Self::Ordinary {
            authored,
            indices: None,
            generated: &[],
        }
    }
    pub(crate) fn append(authored: &'a [Function], generated: &'a [Function]) -> Option<Self> {
        authored.len().checked_add(generated.len())?;
        Some(Self::Ordinary {
            authored,
            indices: None,
            generated,
        })
    }
    pub(crate) fn selected(
        authored: &'a [Function],
        indices: &'a [usize],
        generated: &'a [Function],
    ) -> Option<Self> {
        if indices.iter().any(|index| *index >= authored.len())
            || indices.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return None;
        }
        indices.len().checked_add(generated.len())?;
        Some(Self::Ordinary {
            authored,
            indices: Some(indices),
            generated,
        })
    }
    pub(crate) fn projected(
        authored: &'a [Function],
        directory: &'a FunctionDirectory<'a>,
    ) -> Option<Self> {
        authored.len().checked_add(directory.entries.len())?;
        Some(Self::Projected {
            authored,
            directory,
            start: 0,
        })
    }
    fn authored_len(&self) -> usize {
        match self {
            Self::Ordinary {
                authored, indices, ..
            } => indices.map_or(authored.len(), <[usize]>::len),
            Self::Projected { authored, .. } => authored.len(),
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Ordinary { generated, .. } => self.authored_len() + generated.len(),
            Self::Projected {
                directory, start, ..
            } => self.authored_len() + directory.entries.len() - start,
        }
    }
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "ordered facade migration retains first lookup")
    )]
    pub(crate) fn first(&self) -> Option<FunctionRef<'a>> {
        self.get(0)
    }
    pub(crate) fn get(&self, index: usize) -> Option<FunctionRef<'a>> {
        match self {
            Self::Ordinary {
                authored,
                indices,
                generated,
            } => {
                let count = indices.map_or(authored.len(), <[usize]>::len);
                let payload = if index < count {
                    authored.get(indices.map_or(index, |indices| indices[index]))
                } else {
                    generated.get(index.checked_sub(count)?)
                }?;
                Some(FunctionRef::ordinary(payload))
            }
            Self::Projected {
                authored,
                directory,
                start,
            } => {
                if index < authored.len() {
                    return authored.get(index).map(FunctionRef::ordinary);
                }
                // Check before addition, including usize::MAX hostile lookups.
                if index >= self.len() {
                    return None;
                }
                match directory.entries.get(start + index - authored.len())? {
                    FunctionEntry::Owned(index) => directory
                        .roots
                        .functions
                        .get(*index)
                        .map(FunctionRef::ordinary),
                    entry @ FunctionEntry::Borrowed(view) => Some(FunctionRef {
                        payload: view.original,
                        alias_entry: Some(entry),
                    }),
                }
            }
        }
    }
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "selected/segmented receiver migration retains suffix routing"
        )
    )]
    pub(crate) fn suffix(&self, start: usize) -> Option<Self> {
        if start > self.len() {
            return None;
        }
        match self {
            Self::Ordinary {
                authored,
                indices,
                generated,
            } => {
                let count = self.authored_len();
                if start < count {
                    match indices {
                        Some(indices) => Self::selected(authored, &indices[start..], generated),
                        None => Self::append(&authored[start..], generated),
                    }
                } else {
                    Some(Self::one(&generated[start - count..]))
                }
            }
            Self::Projected {
                authored,
                directory,
                start: offset,
            } => {
                if start < authored.len() {
                    Some(Self::Projected {
                        authored: &authored[start..],
                        directory,
                        start: *offset,
                    })
                } else {
                    Some(Self::Projected {
                        authored: &[],
                        directory,
                        start: offset + start - authored.len(),
                    })
                }
            }
        }
    }
    pub(crate) fn iter(&self) -> FunctionIter<'a> {
        FunctionIter {
            view: *self,
            front: 0,
            back: self.len(),
        }
    }
}
pub(crate) struct FunctionIter<'a> {
    view: FunctionSlices<'a>,
    front: usize,
    back: usize,
}
impl<'a> Iterator for FunctionIter<'a> {
    type Item = FunctionRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let next = self.view.get(self.front);
        self.front += 1;
        next
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (remaining, Some(remaining))
    }
}
impl DoubleEndedIterator for FunctionIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        self.view.get(self.back)
    }
}
impl ExactSizeIterator for FunctionIter<'_> {}
impl std::iter::FusedIterator for FunctionIter<'_> {}
impl<'a> IntoIterator for FunctionSlices<'a> {
    type Item = FunctionRef<'a>;
    type IntoIter = FunctionIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[path = "presentation/metadata.rs"]
mod metadata;
