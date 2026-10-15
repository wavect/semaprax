//! Internal error custody only; it grants no report allowance or authority.
use super::{active_builder_limit, limit_error};
use crate::diagnostic::Diagnostic;

#[allow(
    clippy::large_enum_variant,
    reason = "the complete single diagnostic moves inline; boxing a refusal would acquire unowned backing"
)]
#[derive(Debug)]
pub(super) enum Failure {
    One(Diagnostic),
    Many(Vec<Diagnostic>),
}
impl Failure {
    pub(super) fn one(diagnostic: Diagnostic) -> Self {
        Self::One(diagnostic)
    }
    pub(super) fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::One(diagnostic) => std::slice::from_ref(diagnostic),
            Self::Many(diagnostics) => diagnostics,
        }
    }
    /// Real compatibility acquisition, deliberately not described as paid.
    /// Existing multi-error backing moves with every diagnostic in order.
    pub(super) fn into_legacy_unfunded(self) -> Vec<Diagnostic> {
        match self {
            Self::One(diagnostic) => vec![diagnostic],
            Self::Many(diagnostics) => diagnostics,
        }
    }
}
impl From<Vec<Diagnostic>> for Failure {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Many(diagnostics)
    }
}
impl From<Diagnostic> for Failure {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::One(diagnostic)
    }
}
pub(super) type Result<T> = std::result::Result<T, Failure>;
pub(super) fn reserve(bytes: usize) -> Result<()> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(Failure::One(limit_error(
            "builder_bytes",
            active_builder_limit(),
        )))
    }
}

/// Real resolved-core constructor chain. Legacy helper failures retain all
/// diagnostics and backing; the default/stub/overlay chain carries One inline.
pub(super) fn construct_transient_core<'a>(
    program: &'a crate::ast::Program,
    authored: &'a std::collections::BTreeMap<&'a str, super::AuthoredDeclaration<'a>>,
    programs: &'a [crate::ast::Program],
    registry: &super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
    core: &mut super::expected_projection::CoreProviderProofs<'a>,
) -> Result<super::expected_projection::borrowed_leaf::Input<'a>> {
    use super::expected_projection::{borrowed_imports, borrowed_leaf};
    use borrowed_leaf::Input;
    // Complete initialized Result layout, without a legacy header discount.
    reserve(std::mem::size_of::<Result<Input<'a>>>())?;
    if borrowed_leaf::eligible(program) {
        borrowed_leaf::construct(program)
            .map(Input::Borrowed)
            .map_err(Failure::from)
    } else if borrowed_imports::eligible(program, authored, programs) {
        borrowed_imports::inline::construct_using(program, authored, programs, registry, Some(core))
            .map(Input::Borrowed)
    } else {
        Input::construct_transient_core(program, authored, programs, registry, core)
            .map_err(Failure::from)
    }
}

pub(super) mod receiving;
