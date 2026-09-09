//! Immutable, caller-injected environment input for hosted commands.
//!
//! This type never consults the process environment. Construction owns and
//! canonicalizes supplied UTF-8 entries once; clones and reads share the
//! immutable allocation.

use std::fmt;
use std::sync::Arc;

pub const MAX_ENVIRONMENT_ENTRIES: usize = 256;
pub const MAX_ENVIRONMENT_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentSnapshotError {
    InvalidName,
    InvalidValue,
    InvalidUtf8,
    DuplicateName,
    CapacityExceeded,
}

#[derive(Clone, Eq, PartialEq)]
pub struct EnvironmentSnapshot {
    entries: Arc<[(Arc<[u8]>, Arc<[u8]>)]>,
    byte_len: usize,
}

impl fmt::Debug for EnvironmentSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EnvironmentSnapshot")
            .field("entries", &self.entries.len())
            .field("byte_len", &self.byte_len)
            .finish()
    }
}

impl Default for EnvironmentSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

impl EnvironmentSnapshot {
    pub fn empty() -> Self {
        Self {
            entries: Arc::from([]),
            byte_len: 0,
        }
    }

    pub fn from_entries(entries: Vec<(String, String)>) -> Result<Self, EnvironmentSnapshotError> {
        Self::canonicalize(entries)
    }

    pub fn from_raw_entries(
        entries: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<Self, EnvironmentSnapshotError> {
        if entries.len() > MAX_ENVIRONMENT_ENTRIES {
            return Err(EnvironmentSnapshotError::CapacityExceeded);
        }
        let mut raw_bytes = 0usize;
        for (name, value) in &entries {
            raw_bytes = raw_bytes
                .checked_add(name.len())
                .and_then(|total| total.checked_add(value.len()))
                .ok_or(EnvironmentSnapshotError::CapacityExceeded)?;
            if raw_bytes > MAX_ENVIRONMENT_BYTES {
                return Err(EnvironmentSnapshotError::CapacityExceeded);
            }
        }
        let mut decoded = Vec::with_capacity(entries.len());
        for (name, value) in entries {
            let name =
                String::from_utf8(name).map_err(|_| EnvironmentSnapshotError::InvalidUtf8)?;
            let value =
                String::from_utf8(value).map_err(|_| EnvironmentSnapshotError::InvalidUtf8)?;
            decoded.push((name, value));
        }
        Self::canonicalize(decoded)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub fn get(&self, index: usize) -> Option<(&str, &str)> {
        self.entries.get(index).map(|(name, value)| {
            (
                std::str::from_utf8(name).expect("snapshot names are validated UTF-8"),
                std::str::from_utf8(value).expect("snapshot values are validated UTF-8"),
            )
        })
    }

    pub fn entries(&self) -> impl ExactSizeIterator<Item = (&str, &str)> + '_ {
        self.entries.iter().map(|(name, value)| {
            (
                std::str::from_utf8(name).expect("snapshot names are validated UTF-8"),
                std::str::from_utf8(value).expect("snapshot values are validated UTF-8"),
            )
        })
    }

    pub(crate) fn raw_entry(&self, index: usize) -> Option<(Arc<[u8]>, Arc<[u8]>)> {
        self.entries
            .get(index)
            .map(|(name, value)| (Arc::clone(name), Arc::clone(value)))
    }

    fn canonicalize(mut entries: Vec<(String, String)>) -> Result<Self, EnvironmentSnapshotError> {
        if entries.len() > MAX_ENVIRONMENT_ENTRIES {
            return Err(EnvironmentSnapshotError::CapacityExceeded);
        }
        for (name, value) in &entries {
            if name.is_empty() || name.as_bytes().contains(&b'=') || name.as_bytes().contains(&0) {
                return Err(EnvironmentSnapshotError::InvalidName);
            }
            if value.as_bytes().contains(&0) {
                return Err(EnvironmentSnapshotError::InvalidValue);
            }
            let entry_bytes = name
                .len()
                .checked_add(value.len())
                .ok_or(EnvironmentSnapshotError::CapacityExceeded)?;
            if entry_bytes > MAX_ENVIRONMENT_BYTES {
                return Err(EnvironmentSnapshotError::CapacityExceeded);
            }
        }
        entries.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
        if entries
            .windows(2)
            .any(|pair| pair[0].0.as_bytes() == pair[1].0.as_bytes())
        {
            return Err(EnvironmentSnapshotError::DuplicateName);
        }
        let mut byte_len = 0usize;
        for (name, value) in &entries {
            byte_len = byte_len
                .checked_add(name.len())
                .and_then(|total| total.checked_add(value.len()))
                .ok_or(EnvironmentSnapshotError::CapacityExceeded)?;
            if byte_len > MAX_ENVIRONMENT_BYTES {
                return Err(EnvironmentSnapshotError::CapacityExceeded);
            }
        }
        Ok(Self {
            entries: Arc::from(
                entries
                    .into_iter()
                    .map(|(name, value)| {
                        (
                            Arc::<[u8]>::from(name.into_bytes()),
                            Arc::<[u8]>::from(value.into_bytes()),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            byte_len,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_byte_order_and_keeps_empty_values() {
        let snapshot = EnvironmentSnapshot::from_entries(vec![
            ("z".to_owned(), "".to_owned()),
            ("é".to_owned(), "two".to_owned()),
            ("A".to_owned(), "one".to_owned()),
        ])
        .unwrap();
        assert_eq!(
            snapshot.entries().collect::<Vec<_>>(),
            vec![("A", "one"), ("z", ""), ("é", "two")]
        );
        assert_eq!(snapshot.byte_len(), 1 + 3 + 1 + 0 + 2 + 3);
        assert_eq!(snapshot.get(3), None);
    }

    #[test]
    fn exact_bounds_admit_and_first_excess_rejects() {
        let exact = EnvironmentSnapshot::from_entries(vec![(
            "a".to_owned(),
            "x".repeat(MAX_ENVIRONMENT_BYTES - 1),
        )])
        .unwrap();
        assert_eq!(exact.byte_len(), MAX_ENVIRONMENT_BYTES);
        assert_eq!(
            EnvironmentSnapshot::from_entries(vec![(
                "a".to_owned(),
                "x".repeat(MAX_ENVIRONMENT_BYTES),
            )]),
            Err(EnvironmentSnapshotError::CapacityExceeded)
        );
        let combined = EnvironmentSnapshot::from_entries(vec![
            ("a".to_owned(), "x".repeat(MAX_ENVIRONMENT_BYTES - 2)),
            ("b".to_owned(), String::new()),
        ])
        .unwrap();
        assert_eq!(combined.byte_len(), MAX_ENVIRONMENT_BYTES);
        assert_eq!(
            EnvironmentSnapshot::from_entries(vec![
                ("a".to_owned(), "x".repeat(MAX_ENVIRONMENT_BYTES - 2)),
                ("b".to_owned(), "x".to_owned()),
            ]),
            Err(EnvironmentSnapshotError::CapacityExceeded)
        );
        let entries = (0..MAX_ENVIRONMENT_ENTRIES)
            .map(|index| (format!("k{index}"), String::new()))
            .collect::<Vec<_>>();
        assert_eq!(
            EnvironmentSnapshot::from_entries(entries).unwrap().len(),
            MAX_ENVIRONMENT_ENTRIES
        );
        let too_many = (0..=MAX_ENVIRONMENT_ENTRIES)
            .map(|index| (format!("k{index}"), String::new()))
            .collect::<Vec<_>>();
        assert_eq!(
            EnvironmentSnapshot::from_entries(too_many),
            Err(EnvironmentSnapshotError::CapacityExceeded)
        );
    }

    #[test]
    fn rejects_raw_utf8_names_values_and_invalid_entries() {
        let too_many_raw = (0..=MAX_ENVIRONMENT_ENTRIES)
            .map(|_| (Vec::new(), Vec::new()))
            .collect::<Vec<_>>();
        assert_eq!(
            EnvironmentSnapshot::from_raw_entries(too_many_raw),
            Err(EnvironmentSnapshotError::CapacityExceeded)
        );
        assert_eq!(
            EnvironmentSnapshot::from_raw_entries(vec![(
                vec![0xff; MAX_ENVIRONMENT_BYTES + 1],
                Vec::new(),
            )]),
            Err(EnvironmentSnapshotError::CapacityExceeded)
        );
        assert_eq!(
            EnvironmentSnapshot::from_raw_entries(vec![(vec![0xff], Vec::new())]),
            Err(EnvironmentSnapshotError::InvalidUtf8)
        );
        assert_eq!(
            EnvironmentSnapshot::from_raw_entries(vec![(b"x".to_vec(), vec![0xff])]),
            Err(EnvironmentSnapshotError::InvalidUtf8)
        );
        for name in ["", "A=B", "A\0B"] {
            assert_eq!(
                EnvironmentSnapshot::from_entries(vec![(name.to_owned(), String::new())]),
                Err(EnvironmentSnapshotError::InvalidName)
            );
        }
        assert_eq!(
            EnvironmentSnapshot::from_entries(vec![("A".to_owned(), "x\0y".to_owned())]),
            Err(EnvironmentSnapshotError::InvalidValue)
        );
        assert_eq!(
            EnvironmentSnapshot::from_entries(vec![
                ("A".to_owned(), "one".to_owned()),
                ("A".to_owned(), "two".to_owned()),
            ]),
            Err(EnvironmentSnapshotError::DuplicateName)
        );
    }

    #[test]
    fn empty_and_clones_are_cheap_immutable_views() {
        let empty = EnvironmentSnapshot::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.byte_len(), 0);
        let snapshot =
            EnvironmentSnapshot::from_entries(vec![("A".to_owned(), "one".to_owned())]).unwrap();
        let cloned = snapshot.clone();
        assert_eq!(snapshot, cloned);
        assert!(Arc::ptr_eq(&snapshot.entries, &cloned.entries));
        assert_eq!(cloned.get(0), Some(("A", "one")));
        assert!(!format!("{cloned:?}").contains("one"));
    }
}
