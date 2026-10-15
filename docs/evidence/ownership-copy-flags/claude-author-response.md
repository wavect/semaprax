# Diagnosis and draft: `expected_ownership` unpaid fallback

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

This is source-only. Nothing was compiled or run, and I haven't seen the other `Payer` impls or `nominal_facts.get`, so the signature guesses below are marked.

## Diagnosis (suspect, not proven)

The only sink in the shown path that can allocate unpaid memory is the `else` arm of `borrowed_type_facts`:

```
declarations.type_facts(ty).map(Cow::Owned)
```

`type_facts` has two unpaid allocation sources:
- It calls `ty.identity_key()`, which allocates a `String`, on every non-`cached_facts` hit.
- It calls `.cloned()` on a cached `TypeFacts`, or `recompute_type_facts`, so it deep-copies or builds the owning layout. `expected_ownership` reads only `.copy`.

None of that is charged to the active budget, and `charge::clone_type` is not applied here. That fits "allocated > paid, live 0, freed = allocated": the memory is transient and unpaid. It does not prove that this site accounts for the 18 bytes. I'm not attributing them from sizes. Confirming that needs your measurement after the change (see Verification).

The change avoids the allocations rather than paying for them. `type_flags_paid` takes `self.type_flags(ty)` for leaves and borrows the cached facts for cache hits, with no clone and no key `String`. Only a real miss reaches `Storage<P>`, and that path is charged.

## Draft

New file `src/hir/validation/ownership_flags.rs`, with `mod ownership_flags;` added in `validation.rs`:

```rust
//! Same-active-budget payer for the flags-only ownership query.
use super::hir_error_static; // guess: adjust to wherever validation.rs imports it from
use super::scope_checkpoint::charge;
use crate::bounded_output::reserve_active_required;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedType;
use std::mem::size_of;
// Needs a path visible from validation; see risk 1.
use crate::hir::declaration_index::TypeFactsPayer;

pub(super) struct TypeFlagsPayer;

fn pay(bytes: usize) -> Result<(), Diagnostic> {
    if reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(hir_error_static(
            "ownership type flags exceed the active validation budget",
        ))
    }
}

fn pay_checked(bytes: Option<usize>) -> Result<(), Diagnostic> {
    match bytes {
        Some(bytes) => pay(bytes),
        None => Err(charge::failure(
            usize::MAX,
            "ownership type flags layout overflowed",
        )),
    }
}

impl TypeFactsPayer for TypeFlagsPayer {
    // Zero is not short-circuited: type_flags_paid relies on bytes(0) for
    // sticky first-refusal, and reserve_active_required(0) reports it.
    fn bytes(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        pay(bytes)
    }
    fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
        if values.capacity() - values.len() >= additional {
            return Ok(());
        }
        let total = values.len().checked_add(additional);
        pay_checked(total.and_then(|n| n.checked_mul(size_of::<T>())))?;
        values.try_reserve_exact(additional).map_err(|_| {
            charge::failure(usize::MAX, "ownership type flags growth failed")
        })
    }
    fn node<K, V>(&mut self) -> Result<(), Diagnostic> {
        pay_checked(crate::cleanup_plan::replay_btree_node_upper::<K, V>())
    }
    fn clone_type(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
        pay_checked(ty.clone_payload_bytes())?;
        Ok(ty.clone())
    }
    fn boxed_type(&mut self, ty: ResolvedType) -> Result<Box<ResolvedType>, Diagnostic> {
        pay(size_of::<ResolvedType>())?;
        Ok(Box::new(ty))
    }
    fn construction_refusal(&mut self, message: &'static str) -> Diagnostic {
        hir_error_static(message)
    }
}
```

I did not use `charge::reserve` or `charge::clone_type`, because their `cfg(test)` `operation_guard(ScopeCheckpoint)` would misattribute stage debits.

In `validation.rs`, the hook keeps the `nominal_facts.get(..)?` call first, so its precedence and errors are unchanged. Only the `declarations.type_facts` fallback is replaced:

```rust
    fn expected_ownership(
        &self,
        ty: &ResolvedType,
        non_copy: OwnershipMode,
    ) -> Result<OwnershipMode, Diagnostic> {
        if *ty == ResolvedType::Never {
            return Ok(OwnershipMode::Value);
        }
        let copy = match self.nominal_facts.get(self.program, ty)? {
            Some(facts) => Some(facts.copy),
            None => self
                .program
                .declarations
                .type_flags_paid(ty, ownership_flags::TypeFlagsPayer)?
                .map(|flags| flags.copy),
        };
        copy.map(|copy| if copy { OwnershipMode::Value } else { non_copy })
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }

    /// Verbatim original, kept as the independent oracle.
    #[cfg(test)]
    fn expected_ownership_owned_reference(
        &self,
        ty: &ResolvedType,
        non_copy: OwnershipMode,
    ) -> Result<OwnershipMode, Diagnostic> {
        /* original body, unchanged */
    }
```

I used `#[cfg(test)]` for the oracle. If "compiled" means it must exist in production builds, drop the cfg and add `#[allow(dead_code, reason = "independent oracle")]`.

## Tests to add (existing assertions untouched)

- **Parity:** for each `ResolvedProgram` in the existing sources (`CONTRACT_SOURCE`, `iterative_validator_tests::SOURCE`, `PAYLOAD_SOURCE`), collect every param, return and body type. Add the fixed corners: `Never`, all leaves, `ArrayU8`, a valid and an invalid `Function`, a zero-arg nominal, a generic compound, and an unresolved type parameter. For both `non_copy` modes, assert that `expected_ownership` and `expected_ownership_owned_reference` give equal `Ok` values or equal `Err` messages.
- **Hostile cache:** reuse the hostile-cache fixtures from `flags/tests`, with a poisoned `type_facts_by_id` entry, and assert the same parity.
- **Output parity:** run `validate_function` over those programs and compare diagnostics with budget unlimited.
- **Existing controls:** `production_chain_observes_actual_backing_debits_...` stays as is. It should now satisfy `allocated <= paid` for the failing function.

## Risks

1. **Visibility.** `flags` is likely private to `declaration_index`, and `TypeFactsPayer` is re-exported only inside it. If validation can't name it, you need a one-line `pub(crate) use` in `declaration_index.rs`. That is outside the flags module but is still an edit. I avoided naming `TypeFlags` for the same reason.
2. **Hostile and cached facts.** Both paths check the cache first. They differ in that `type_facts` clones the cached entry, while the paid path returns flags. `Some(None)` and miss both go to recompute, but `ArrayU8` and `Function` get a hardcoded `copy: true` shortcut in `recompute_flags`. That must equal `recompute_type_facts`, which only the parity test can show. Also, `type_facts` has a `cfg(test)` `old_key_enabled()` oracle bypass that the paid path doesn't, so parity tests must run with it off.
3. **Generic types.** A compound type containing an unresolved parameter should return `None`, and the "no semantic facts" message should be preserved. Under a nearly exhausted budget, a payment failure can now pre-empt that message through `Storage.first`.
4. **Imported types.** `nominal_facts` stays first, so any imported-declaration handling there is untouched. The fallback only sees `self.program.declarations`, as before. If `nominal_facts.get` itself allocates unpaid, that is outside this change and could still account for part of the gap.
5. **Budget behavior.** `bytes(0)` now fails if the active budget is already sticky-refused, which wasn't true before. This is harmless only if refusal always propagates as an error. Real misses now add charges, so `paid` rises, and any hardcoded `LIMIT` or short-limit constants may shift.
6. **Under-charge.** My `grow` charges `(len + additional) * size_of::<T>()`, and `construction_refusal` doesn't mark the refusal sticky. Both are guesses at what the existing payers do, so check them against those impls. If real allocation exceeds the charge on the miss path, the gap would persist there.

## Verification (not run)

Run the `function_scope` lifetime test and the new parity tests. If `allocated` still exceeds `paid` for some function, the 18 bytes come from somewhere other than this fallback. In that case, instrument by phase instead of guessing from sizes.
