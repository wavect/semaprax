//! Heterogeneous exact-key lookup in the existing ordered cache. Both stored
//! strings and queries use precisely their canonical UTF-8 byte ordering; no
//! directory, whole-map scan, allocation or replacement cached facts are added.
use super::super::{DeclarationId, TypeFacts};
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
mod compound;

trait Key {
    fn bytes(&self) -> Bytes<'_>;
}
// The bounded compound cursor deliberately stays inline: boxing would allocate
// scratch during every ordered-map comparison and invalidate the allocation-free
// type-key receiver. Its fixed storage is covered by the layout/Drop controls;
// compound census overflow retains the complete owned canonical-key fallback.
#[allow(clippy::large_enum_variant)]
enum Bytes<'a> {
    Text(std::iter::Copied<std::slice::Iter<'a, u8>>),
    Nominal(NominalBytes<'a>),
    Compound(compound::Bytes<'a>),
}
impl Iterator for Bytes<'_> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        match self {
            Self::Text(bytes) => bytes.next(),
            Self::Nominal(bytes) => bytes.next(),
            Self::Compound(bytes) => bytes.next(),
        }
    }
}
impl Key for String {
    fn bytes(&self) -> Bytes<'_> {
        Bytes::Text(self.as_bytes().iter().copied())
    }
}
// The local trait-object Borrow target permits heterogeneous BTreeMap queries.
// Its Ord must equal String::Ord for EVERY stored key, including hostile keys.
impl<'a> Borrow<dyn Key + 'a> for String {
    fn borrow(&self) -> &(dyn Key + 'a) {
        self
    }
}
impl PartialEq for dyn Key + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for dyn Key + '_ {}
impl PartialOrd for dyn Key + '_ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for dyn Key + '_ {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bytes().cmp(other.bytes())
    }
}
struct Nominal<'a>(&'a DeclarationId);
impl Key for Nominal<'_> {
    fn bytes(&self) -> Bytes<'_> {
        Bytes::Nominal(NominalBytes::new(self.0.as_str().as_bytes()))
    }
}
struct NominalBytes<'a> {
    identity: &'a [u8],
    // One byte per binary digit is a platform-independent upper bound for the
    // decimal usize spelling. This fixed scalar backing is never heap storage.
    decimal: [u8; usize::BITS as usize],
    start: usize,
    segment: u8,
    offset: usize,
}
impl<'a> NominalBytes<'a> {
    fn new(identity: &'a [u8]) -> Self {
        let mut decimal = [0; usize::BITS as usize];
        let mut start = decimal.len();
        let mut length = identity.len();
        loop {
            start -= 1;
            decimal[start] = b'0' + (length % 10) as u8;
            length /= 10;
            if length == 0 {
                break;
            }
        }
        Self {
            identity,
            decimal,
            start,
            segment: 0,
            offset: 0,
        }
    }
}
impl Iterator for NominalBytes<'_> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        loop {
            let bytes: &[u8] = match self.segment {
                0 => b"nominal:",
                1 => &self.decimal[self.start..],
                2 => b":",
                3 => self.identity,
                4 => b":0:",
                _ => return None,
            };
            if let Some(byte) = bytes.get(self.offset) {
                self.offset += 1;
                return Some(*byte);
            }
            self.segment += 1;
            self.offset = 0;
        }
    }
}
pub(super) fn get<'a>(
    facts: &'a BTreeMap<String, TypeFacts>,
    identity: &DeclarationId,
) -> Option<&'a TypeFacts> {
    let query = Nominal(identity);
    facts.get::<dyn Key + '_>(&query)
}

struct Compound<'a>(compound::Plan<'a>);
impl Key for Compound<'_> {
    fn bytes(&self) -> Bytes<'_> {
        Bytes::Compound(self.0.bytes())
    }
}
pub(super) fn compound_get<'a>(
    facts: &'a BTreeMap<String, TypeFacts>,
    ty: &super::super::ResolvedType,
) -> Option<Option<&'a TypeFacts>> {
    let query = Compound(compound::Plan::new(ty)?);
    Some(facts.get::<dyn Key + '_>(&query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{DeclarationIndex, ResolvedType};
    use crate::test_allocation_observer::measure;

    #[test]
    fn streamed_nominal_keys_preserve_all_bytes_order_and_hostile_cached_facts() {
        let mut index = DeclarationIndex::default();
        let names = ["", "row", "0123456789", "colon:0:row", "nul\0row", "é✓.row"];
        for (ordinal, name) in names.iter().enumerate() {
            let id = DeclarationId::new(*name);
            let ty = ResolvedType::Nominal {
                declaration: id.clone(),
                arguments: Vec::new(),
            };
            let key = ty.identity_key();
            assert_eq!(Nominal(&id).bytes().collect::<Vec<_>>(), key.as_bytes());
            index.type_facts_by_id.insert(
                key,
                TypeFacts {
                    copy: ordinal % 2 == 0,
                    contains_resource: true,
                    sized: false,
                    needs_drop: ordinal % 2 != 0,
                    layout_key: format!("hostile-layout-{ordinal}"),
                },
            );
        }
        // Noncanonical neighboring keys participate in the SAME ordered tree.
        for key in [
            "",
            "nominal:",
            "nominal:01:row:0:",
            "nominal:3:row:1:",
            "zz",
        ] {
            index.type_facts_by_id.insert(
                key.to_owned(),
                TypeFacts {
                    copy: false,
                    contains_resource: false,
                    sized: true,
                    needs_drop: false,
                    layout_key: "neighbor".to_owned(),
                },
            );
        }
        for name in names.iter().chain(["missing"].iter()) {
            let id = DeclarationId::new(*name);
            let ty = ResolvedType::Nominal {
                declaration: id.clone(),
                arguments: Vec::new(),
            };
            let expected = index
                .type_facts_owned_key_oracle(&ty)
                .as_ref()
                .map(super::super::TypeFlags::from);
            assert_eq!(index.type_flags(&ty), expected);
            for stored in index.type_facts_by_id.keys() {
                let query = Nominal(&id);
                let other: &(dyn Key + '_) = stored;
                assert_eq!(
                    (&query as &dyn Key).cmp(other),
                    ty.identity_key().cmp(stored)
                );
            }
        }
        // Generic and missing nominal types retain the untouched owned fallback.
        let generic = ResolvedType::Nominal {
            declaration: DeclarationId::new("row"),
            arguments: vec![ResolvedType::I64],
        };
        assert_eq!(
            index.type_flags(&generic),
            index
                .type_facts_owned_key_oracle(&generic)
                .as_ref()
                .map(super::super::TypeFlags::from)
        );
    }

    #[test]
    fn zero_argument_cached_flags_remove_literal_key_and_layout_requests_through_drop() {
        let mut index = DeclarationIndex::default();
        let ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("current.row"),
            arguments: Vec::new(),
        };
        const KEY: &str = "nominal:11:current.row:0:";
        const LAYOUT: &str = "retained-layout-payload";
        const QUERIES: usize = 32;
        assert_eq!(ty.identity_key(), KEY);
        index.type_facts_by_id.insert(
            KEY.to_owned(),
            TypeFacts {
                copy: true,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: LAYOUT.to_owned(),
            },
        );
        let expected = super::super::TypeFlags {
            copy: true,
            contains_resource: true,
            sized: false,
            needs_drop: true,
        };
        let (_, old) = measure(|| {
            for _ in 0..QUERIES {
                // Saved complete owned-key lookup and layout clone API.
                let facts = index
                    .type_facts_owned_key_oracle(std::hint::black_box(&ty))
                    .unwrap();
                assert_eq!(super::super::TypeFlags::from(&facts), expected);
                drop(facts);
            }
        });
        let (_, new) = measure(|| {
            for _ in 0..QUERIES {
                assert_eq!(index.type_flags(std::hint::black_box(&ty)), Some(expected));
            }
        });
        assert!(!old.invalid && !new.invalid);
        assert_eq!(old.allocated, old.freed);
        assert_eq!(old.live, 0);
        // format!'s String may grow while producing KEY: the independent
        // literal lower bound retains every actual old replacement request.
        assert!(old.allocated >= QUERIES * (KEY.len() + LAYOUT.len()));
        assert!(old.allocations >= 2 * QUERIES);
        assert_eq!(
            (
                new.allocations,
                new.reallocations,
                new.allocated,
                new.freed,
                new.live
            ),
            (0, 0, 0, 0, 0)
        );
    }
}
