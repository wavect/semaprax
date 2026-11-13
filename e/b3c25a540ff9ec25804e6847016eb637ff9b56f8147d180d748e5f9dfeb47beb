//! Fixed compiler subject messages plus a fully funded unknown-label fallback.
use super::*;
const REFUSED: &str = "identity audit exceeds the active validation budget";
const SUFFIX: &str = " identity contains NUL";
pub(super) fn refused() -> Diagnostic {
    hir_error_static(REFUSED)
}
pub(super) fn reserve(bytes: usize) -> Result<(), Diagnostic> {
    if crate::bounded_output::active_remaining().is_none() {
        return Err(hir_error_static(
            "paid identity audit requires an active validation owner",
        ));
    }
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(refused())
    }
}
pub(super) fn physical() -> Diagnostic {
    crate::bounded_output::refuse_active_required(usize::MAX);
    refused()
}
pub(super) fn message(subject: &str) -> Option<&'static str> {
    Some(match subject {
        "cleanup field" => "cleanup field identity contains NUL",
        "cleanup inventory expression" => "cleanup inventory expression identity contains NUL",
        "cleanup inventory lifecycle" => "cleanup inventory lifecycle identity contains NUL",
        "cleanup inventory projection" => "cleanup inventory projection identity contains NUL",
        "cleanup inventory value" => "cleanup inventory value identity contains NUL",
        "cleanup lifecycle" => "cleanup lifecycle identity contains NUL",
        "cleanup record" => "cleanup record identity contains NUL",
        "cleanup variant" => "cleanup variant identity contains NUL",
        "cleanup variant case" => "cleanup variant case identity contains NUL",
        "cleanup variant case inventory has a non-canonical index" => {
            "cleanup variant case inventory has a non-canonical index identity contains NUL"
        }
        "cleanup variant field" => "cleanup variant field identity contains NUL",
        "cleanup-plan authenticated case" => {
            "cleanup-plan authenticated case identity contains NUL"
        }
        "cleanup-plan authenticated variant" => {
            "cleanup-plan authenticated variant identity contains NUL"
        }
        "cleanup-plan boolean expression" => {
            "cleanup-plan boolean expression identity contains NUL"
        }
        "cleanup-plan call-argument call" => {
            "cleanup-plan call-argument call identity contains NUL"
        }
        "cleanup-plan call-argument value" => {
            "cleanup-plan call-argument value identity contains NUL"
        }
        "cleanup-plan committed call" => "cleanup-plan committed call identity contains NUL",
        "cleanup-plan finalizer lifecycle" => {
            "cleanup-plan finalizer lifecycle identity contains NUL"
        }
        "cleanup-plan initialize expression" => {
            "cleanup-plan initialize expression identity contains NUL"
        }
        "cleanup-plan initialized variant" => {
            "cleanup-plan initialized variant identity contains NUL"
        }
        "cleanup-plan match scrutinee" => "cleanup-plan match scrutinee identity contains NUL",
        "cleanup-plan projection" => "cleanup-plan projection identity contains NUL",
        "cleanup-plan propagated callee" => "cleanup-plan propagated callee identity contains NUL",
        "cleanup-plan scalar result" => "cleanup-plan scalar result identity contains NUL",
        "cleanup-plan scalar-match scrutinee" => {
            "cleanup-plan scalar-match scrutinee identity contains NUL"
        }
        "cleanup-plan staged Option `?` None case" => {
            "cleanup-plan staged Option `?` None case identity contains NUL"
        }
        "cleanup-plan staged Option `?` Option" => {
            "cleanup-plan staged Option `?` Option identity contains NUL"
        }
        "cleanup-plan staged Option `?` Some case" => {
            "cleanup-plan staged Option `?` Some case identity contains NUL"
        }
        "cleanup-plan staged Option `?` Some field" => {
            "cleanup-plan staged Option `?` Some field identity contains NUL"
        }
        "cleanup-plan staged Option `?` expression" => {
            "cleanup-plan staged Option `?` expression identity contains NUL"
        }
        "cleanup-plan staged Option `?` operand" => {
            "cleanup-plan staged Option `?` operand identity contains NUL"
        }
        "cleanup-plan staged `?` Err case" => {
            "cleanup-plan staged `?` Err case identity contains NUL"
        }
        "cleanup-plan staged `?` Err field" => {
            "cleanup-plan staged `?` Err field identity contains NUL"
        }
        "cleanup-plan staged `?` Ok case" => {
            "cleanup-plan staged `?` Ok case identity contains NUL"
        }
        "cleanup-plan staged `?` Ok field" => {
            "cleanup-plan staged `?` Ok field identity contains NUL"
        }
        "cleanup-plan staged `?` Result" => "cleanup-plan staged `?` Result identity contains NUL",
        "cleanup-plan staged `?` expression" => {
            "cleanup-plan staged `?` expression identity contains NUL"
        }
        "cleanup-plan staged `?` operand" => {
            "cleanup-plan staged `?` operand identity contains NUL"
        }
        "cleanup-plan staged body expression" => {
            "cleanup-plan staged body expression identity contains NUL"
        }
        "cleanup-plan status expression" => "cleanup-plan status expression identity contains NUL",
        "cleanup-plan temporary storage" => "cleanup-plan temporary storage identity contains NUL",
        "cleanup-plan transfer expression" => {
            "cleanup-plan transfer expression identity contains NUL"
        }
        "cleanup-plan transferred variant" => {
            "cleanup-plan transferred variant identity contains NUL"
        }
        "cleanup-plan value storage" => "cleanup-plan value storage identity contains NUL",
        "cleanup-plan variant authentication expression" => {
            "cleanup-plan variant authentication expression identity contains NUL"
        }
        "cleanup-plan variant case" => "cleanup-plan variant case identity contains NUL",
        "cleanup-plan variant initialize expression" => {
            "cleanup-plan variant initialize expression identity contains NUL"
        }
        "cleanup-plan variant transfer expression" => {
            "cleanup-plan variant transfer expression identity contains NUL"
        }
        "closure capture" => "closure capture identity contains NUL",
        "closure parameter" => "closure parameter identity contains NUL",
        "declaration index key" => "declaration index key identity contains NUL",
        "function reference" => "function reference identity contains NUL",
        "lexical target execution" => "lexical target execution identity contains NUL",
        "lexical target loop" => "lexical target loop identity contains NUL",
        "resolved Option `?` None case" => "resolved Option `?` None case identity contains NUL",
        "resolved Option `?` Option" => "resolved Option `?` Option identity contains NUL",
        "resolved Option `?` Some case" => "resolved Option `?` Some case identity contains NUL",
        "resolved Option `?` Some field" => "resolved Option `?` Some field identity contains NUL",
        "resolved `?` Err case" => "resolved `?` Err case identity contains NUL",
        "resolved `?` Err field" => "resolved `?` Err field identity contains NUL",
        "resolved `?` Ok case" => "resolved `?` Ok case identity contains NUL",
        "resolved `?` Ok field" => "resolved `?` Ok field identity contains NUL",
        "resolved `?` Result" => "resolved `?` Result identity contains NUL",
        "resolved byte-range operation" => "resolved byte-range operation identity contains NUL",
        "resolved byte-view operation" => "resolved byte-view operation identity contains NUL",
        "resolved call target" => "resolved call target identity contains NUL",
        "resolved case field" => "resolved case field identity contains NUL",
        "resolved case field declaration" => {
            "resolved case field declaration identity contains NUL"
        }
        "resolved case initializer field" => {
            "resolved case initializer field identity contains NUL"
        }
        "resolved case-field owner" => "resolved case-field owner identity contains NUL",
        "resolved class declaration" => "resolved class declaration identity contains NUL",
        "resolved declaration owner" => "resolved declaration owner identity contains NUL",
        "resolved entry point" => "resolved entry point identity contains NUL",
        "resolved expression" => "resolved expression identity contains NUL",
        "resolved field" => "resolved field identity contains NUL",
        "resolved field declaration" => "resolved field declaration identity contains NUL",
        "resolved field lookup" => "resolved field lookup identity contains NUL",
        "resolved field owner lookup" => "resolved field owner lookup identity contains NUL",
        "resolved function" => "resolved function identity contains NUL",
        "resolved function declaration" => "resolved function declaration identity contains NUL",
        "resolved function lookup" => "resolved function lookup identity contains NUL",
        "resolved host-command call identity is inconsistent" => {
            "resolved host-command call identity is inconsistent identity contains NUL"
        }
        "resolved import" => "resolved import identity contains NUL",
        "resolved import declaration" => "resolved import declaration identity contains NUL",
        "resolved import lookup" => "resolved import lookup identity contains NUL",
        "resolved import owner" => "resolved import owner identity contains NUL",
        "resolved interface" => "resolved interface identity contains NUL",
        "resolved interface declaration" => "resolved interface declaration identity contains NUL",
        "resolved lifecycle import" => "resolved lifecycle import identity contains NUL",
        "resolved lifecycle logical import key" => {
            "resolved lifecycle logical import key identity contains NUL"
        }
        "resolved logical import key" => "resolved logical import key identity contains NUL",
        "resolved match binding" => "resolved match binding identity contains NUL",
        "resolved match case" => "resolved match case identity contains NUL",
        "resolved match field" => "resolved match field identity contains NUL",
        "resolved match variant" => "resolved match variant identity contains NUL",
        "resolved native Rust import call identity is inconsistent" => {
            "resolved native Rust import call identity is inconsistent identity contains NUL"
        }
        "resolved native Rust import target" => {
            "resolved native Rust import target identity contains NUL"
        }
        "resolved nominal type" => "resolved nominal type identity contains NUL",
        "resolved place field" => "resolved place field identity contains NUL",
        "resolved place root" => "resolved place root identity contains NUL",
        "resolved place variant case" => "resolved place variant case identity contains NUL",
        "resolved place variant field" => "resolved place variant field identity contains NUL",
        "resolved projected field" => "resolved projected field identity contains NUL",
        "resolved record" => "resolved record identity contains NUL",
        "resolved record constructor" => "resolved record constructor identity contains NUL",
        "resolved record declaration" => "resolved record declaration identity contains NUL",
        "resolved record initializer field" => {
            "resolved record initializer field identity contains NUL"
        }
        "resolved record match" => "resolved record match identity contains NUL",
        "resolved record match binding" => "resolved record match binding identity contains NUL",
        "resolved record match field" => "resolved record match field identity contains NUL",
        "resolved record replacement field" => {
            "resolved record replacement field identity contains NUL"
        }
        "resolved record update" => "resolved record update identity contains NUL",
        "resolved record-field owner" => "resolved record-field owner identity contains NUL",
        "resolved resource" => "resolved resource identity contains NUL",
        "resolved resource declaration" => "resolved resource declaration identity contains NUL",
        "resolved resource lifecycle" => "resolved resource lifecycle identity contains NUL",
        "resolved resource lifecycle declaration" => {
            "resolved resource lifecycle declaration identity contains NUL"
        }
        "resolved type lookup" => "resolved type lookup identity contains NUL",
        "resolved type-parameter owner" => "resolved type-parameter owner identity contains NUL",
        "resolved value" => "resolved value identity contains NUL",
        "resolved variant" => "resolved variant identity contains NUL",
        "resolved variant case" => "resolved variant case identity contains NUL",
        "resolved variant case declaration" => {
            "resolved variant case declaration identity contains NUL"
        }
        "resolved variant case lookup" => "resolved variant case lookup identity contains NUL",
        "resolved variant constructor" => "resolved variant constructor identity contains NUL",
        "resolved variant declaration" => "resolved variant declaration identity contains NUL",
        "resolved variant owner lookup" => "resolved variant owner lookup identity contains NUL",
        "resolved variant-case owner" => "resolved variant-case owner identity contains NUL",
        "row loan bound" => "row loan bound identity contains NUL",
        "row loan callee" => "row loan callee identity contains NUL",
        "row loan parameter" => "row loan parameter identity contains NUL",
        "row loan traversal" => "row loan traversal identity contains NUL",
        "scoped vector field" => "scoped vector field identity contains NUL",
        _ => return None,
    })
}
pub(super) fn reject_nul_identity(subject: &str, value: &str) -> Result<(), Diagnostic> {
    reserve(0)?;
    if !value.contains('\0') {
        return Ok(());
    }
    if let Some(message) = message(subject) {
        return Err(hir_error_static(message));
    }
    let count = subject
        .len()
        .checked_add(SUFFIX.len())
        .ok_or_else(physical)?;
    let layout = std::alloc::Layout::array::<u8>(count).map_err(|_| physical())?;
    // Inline owned String carrier and requested Global payload are distinct.
    reserve(std::mem::size_of::<String>())?;
    reserve(layout.size())?;
    let mut message = String::new();
    message.try_reserve_exact(count).map_err(|_| physical())?;
    if message.capacity() != count {
        return Err(physical());
    }
    message.push_str(subject);
    message.push_str(SUFFIX);
    Err(hir_error(message))
}
