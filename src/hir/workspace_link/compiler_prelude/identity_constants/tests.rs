//! Narrow identity payload scope; other prelude allocations remain intact.
use super::*;
use crate::test_allocation_observer::measure;
mod old;

#[test]
fn fixed_compiler_prelude_identities_preserve_complete_saved_index_and_drop_payloads() {
    for (include_vec, include_box, texts) in [
        (false, false, &[][..]),
        (true, false, &[crate::prelude::VEC_ID][..]),
        (
            false,
            true,
            &[crate::prelude::VEC_ID, crate::prelude::BOX_ID][..],
        ),
        (
            true,
            true,
            &[crate::prelude::VEC_ID, crate::prelude::BOX_ID][..],
        ),
    ] {
        let current = compiler_prelude_declarations_for(include_vec, include_box).unwrap();
        let saved = old::saved_compiler_prelude_declarations_for(include_vec, include_box).unwrap();
        assert_eq!(current, saved);
        for text in texts {
            let declaration = current.declarations.get(*text).unwrap();
            // A `const &str` has no guaranteed unique address (MSVC links a
            // separate copy per use), so pointer equality with the constant is
            // checked only where the linker merges them. Everywhere the identity
            // must be the compiler-owned static text with no owned backing.
            assert!(declaration.id.is_compiler_static());
            assert_eq!(declaration.id.owned_allocation_bytes(), 0);
            assert_eq!(declaration.id.as_str(), *text);
            #[cfg(not(windows))]
            assert_eq!(declaration.id.as_str().as_ptr(), text.as_ptr());
            assert_eq!(declaration.identity_origin, IdentityOrigin::CompilerOwned);
            assert_eq!(current.type_parameters[&declaration.id].len(), 1);
            assert!(current.record_fields[&declaration.id].is_empty());
            // Equal authored text retains exact lookup meaning, without gaining
            // compiler provenance or changing the closed prelude selection.
            let shared = DeclarationId::new(*text);
            assert_eq!(shared, declaration.id);
            assert_eq!(
                current.record_fields.get(&shared),
                saved.record_fields.get(&shared)
            );
        }
        drop((current, saved));
        let (_, before) = measure(|| {
            drop(old::saved_compiler_prelude_declarations_for(include_vec, include_box).unwrap());
        });
        let (_, after) = measure(|| {
            drop(compiler_prelude_declarations_for(include_vec, include_box).unwrap());
        });
        let word = std::mem::size_of::<usize>();
        // Each saved borrowed new copies one String then one Arc<str> with
        // two counters and word-rounded text. Static identities copy neither.
        let removed = texts
            .iter()
            .map(|text| text.len() + (2 * word + text.len()).div_ceil(word) * word)
            .sum::<usize>();
        assert_eq!(before.allocations, after.allocations + 2 * texts.len());
        assert_eq!(before.reallocations, after.reallocations);
        assert_eq!(before.allocated, after.allocated + removed);
        for observed in [before, after] {
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
