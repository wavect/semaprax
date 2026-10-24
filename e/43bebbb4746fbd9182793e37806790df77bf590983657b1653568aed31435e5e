//! One lexical invocation's fully drained type-audit scratch, never a verdict cache.
use super::*;
use std::mem::size_of;

type Work<'a> = Pending<'a, ResolvedType>;
type Owner<'a> = Box<Work<'a>>;

// The initial bank owns the first prepayment peek. Each reached composite
// payment renews it before any later macro forms its context loans. Primitive
// leaves execute the unchanged match without borrowing this context at all.
const PEEK: usize = 2 * size_of::<&mut Storage<'static>>()
    + 2 * size_of::<&Storage<'static>>()
    + 2 * size_of::<&Option<Owner<'static>>>()
    + 4 * size_of::<bool>();
pub(super) const HEADER: usize = PEEK
    + 5 * size_of::<Storage<'static>>() // aggregate/return/receive/local/final Drop
    + 2 * size_of::<Option<Owner<'static>>>(); // neutral field transport
                                               // All caller/helper/Option::take/match/Box::as_mut/publication/Drop pointer
                                               // roles; the original Result-returning type call remains one call, not two.
pub(super) const CALL: usize = PEEK
    + 2 * size_of::<&mut Storage<'static>>()
    + 2 * size_of::<&ResolvedType>()
    + 4 * size_of::<Owner<'static>>() // pattern/local and publication OR error Drop
    + 4 * size_of::<Option<Owner<'static>>>() // take and publication OR error Drop
    + 2 * size_of::<&mut Option<Owner<'static>>>()
    + 2 * size_of::<&mut Owner<'static>>()
    + 2 * size_of::<&mut Work<'static>>()
    + 8 * size_of::<bool>();
// The first actual Box is new heap backing. It is paid independently of every
// original inline frame carrier; neither is a physical credit for the other.
// Five moved/constructed Work carriers plus its allocated payload, all pointer
// transfers, allocation/deallocation layouts, sizes, and raw pointer results.
pub(super) const COLD: usize = 6 * size_of::<Work<'static>>()
    + 10 * size_of::<Owner<'static>>() // includes one carried final owner Drop
    + 6 * size_of::<Option<Owner<'static>>>()
    + 4 * size_of::<std::alloc::Layout>()
    + size_of::<[usize; 8]>()
    + 8 * size_of::<std::ptr::NonNull<Work<'static>>>()
    + 4 * size_of::<&mut Work<'static>>();
// New context argument to the expression/record/inventory/plan helpers.
pub(super) const LOAN: usize = 2 * size_of::<&mut Storage<'static>>();
// Record recursion's new context/record arguments consume the previous thin
// peek bank (at most four references); its original fixed-subject reserve(0)
// renews that bank. The exact closed diagnostic is independently compared to
// the unchanged text table; unknown-subject fallback remains in text.rs.
pub(super) fn record_identity(record: &DeclarationId) -> Result<(), Diagnostic> {
    reserve(PEEK)?;
    if record.as_str().contains('\0') {
        Err(hir_error_static(
            "resolved record match identity contains NUL",
        ))
    } else {
        Ok(())
    }
}

pub(super) struct Storage<'hir> {
    pending: Option<Owner<'hir>>,
}
// Only this module can create the cold-formation token, after the composite
// macro's complete payment; the backing constructor cannot be called naked.
pub(super) struct Formation(());
impl<'hir> Storage<'hir> {
    pub(super) fn new() -> Self {
        Self { pending: None }
    }
    pub(super) fn cold(&self) -> bool {
        self.pending.is_none()
    }
    pub(super) fn visit(&mut self, root: &'hir ResolvedType, cold: bool) -> Result<(), Diagnostic> {
        let mut owner = match self.pending.take() {
            Some(owner) if !cold => owner,
            None if cold => Box::new(Work::type_empty(Formation(()))),
            // A false warm declaration never forms unpriced backing. Ordinary
            // lexical callers cannot alter this owner between peek and visit.
            _ => return Err(text::refused()),
        };
        let pending = owner.as_mut();
        pending.push(root)?;
        while let Some(ty) = pending.pop()? {
            match ty {
                ResolvedType::Function { parameters, result } => {
                    pending.extend(parameters)?;
                    pending.push(result)?;
                }
                ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
                | ResolvedType::Never
                | ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::ArrayU8(_)
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::StringMap => {}
                ResolvedType::TypeParameter { owner, .. } => {
                    reject_nul_identity("resolved type-parameter owner", owner.as_str())?;
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    reject_nul_identity("resolved nominal type", declaration.as_str())?;
                    pending.extend(arguments)?;
                }
            }
        }
        // Only the successful exhausted walk publishes reusable scratch. On
        // every ? error above the taken owner drops and the slot stays None.
        self.pending = Some(owner);
        Ok(())
    }
}

macro_rules! audit {
    (@body $root:expr, $loan:expr) => {{
        let root: &ResolvedType = $root;
        match root {
            ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64
            | ResolvedType::Never
            | ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::StringMap => Ok(()),
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if arguments.is_empty() => $crate::hir::inspection::paid_nul::reject_nul_identity(
                "resolved nominal type",
                declaration.as_str(),
            ),
            ResolvedType::TypeParameter { owner, .. } => {
                $crate::hir::inspection::paid_nul::reject_nul_identity(
                    "resolved type-parameter owner",
                    owner.as_str(),
                )
            }
            ResolvedType::Function { .. } | ResolvedType::Nominal { .. } => {
                // The constructor/previous call paid this thin exclusive peek.
                // No caller may mutate the lexical context between peek/payment.
                let types = $loan;
                let cold = types.cold();
                $crate::hir::inspection::paid_nul::reserve(if cold {
                    $crate::hir::inspection::paid_nul::type_scratch::CALL
                        + $crate::hir::inspection::paid_nul::type_scratch::COLD
                } else {
                    $crate::hir::inspection::paid_nul::type_scratch::CALL
                })?;
                types.visit(root, cold)
            }
        }
    }};
    // Forward syntax only: no runtime argument/loan exists until the reached
    // composite branch. The borrowed arm explicitly reborrows, never moves a
    // caller's mutable context reference out of its remaining audit loop.
    ($root:expr, &mut $types:ident) => {
        $crate::hir::inspection::paid_nul::type_scratch::audit!(@body $root, &mut $types)
    };
    ($root:expr, $types:ident) => {
        $crate::hir::inspection::paid_nul::type_scratch::audit!(@body $root, &mut *$types)
    };
}
pub(super) use audit;
