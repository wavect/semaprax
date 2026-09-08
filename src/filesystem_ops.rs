//! Closed capability-authenticated filesystem operation identities and signatures.
use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{OwnershipMode, ResolvedHostCommandOperation as Op, ResolvedType};

pub(crate) const READ_NAME: &str = "file_read";
pub(crate) const WRITE_NEW_NAME: &str = "file_write_new";
pub(crate) const READ_ID: &str = "core.host.file-read";
pub(crate) const WRITE_NEW_ID: &str = "core.host.file-write-new";
pub(crate) const READ_EFFECT: &str = "fs.read";
pub(crate) const WRITE_EFFECT: &str = "fs.write";
pub(crate) const FILESYSTEM_EFFECTS: [&str; 2] = [READ_EFFECT, WRITE_EFFECT];
pub(crate) const STATUS_DOMAIN: &str = "semaprax.filesystem.v1";
pub(crate) const STATUS_CODES: [u32; 7] = [1, 2, 3, 4, 5, 6, 7];
pub(crate) const INVALID_PATH: u32 = 1;
pub(crate) const NOT_FOUND: u32 = 2;
pub(crate) const ALREADY_EXISTS: u32 = 3;
pub(crate) const CAPACITY_EXCEEDED: u32 = 4;
pub(crate) const IO_FAILURE: u32 = 5;
pub(crate) const AUTHORITY_DENIED: u32 = 6;
pub(crate) const INVALID_FILE_TYPE: u32 = 7;
pub(crate) const MAX_PATH_BYTES: u64 = crate::filesystem_provider::MAX_PATH_BYTES as u64;
pub(crate) const MAX_FILE_BYTES: u64 = crate::filesystem_provider::MAX_FILE_BYTES as u64;
pub(crate) const MAX_TOTAL_BYTES: u64 = crate::filesystem_provider::MAX_TOTAL_BYTES as u64;
pub(crate) const MAX_OPERATIONS: u64 = crate::filesystem_provider::MAX_OPERATIONS as u64;
pub(crate) const OPERATIONS: [Op; 7] = [
    Op::FileRead,
    Op::FileWriteNew,
    Op::FileStat,
    Op::FileList,
    Op::FileCreateDir,
    Op::FileRemove,
    Op::FileWriteAtomic,
];
pub(crate) const fn is_v2(op: Op) -> bool {
    matches!(
        op,
        Op::FileStat | Op::FileList | Op::FileCreateDir | Op::FileRemove | Op::FileWriteAtomic
    )
}
pub(crate) const fn permits_root(op: Op) -> bool {
    matches!(op, Op::FileStat | Op::FileList)
}
pub(crate) const fn is_filesystem(op: Op) -> bool {
    matches!(op, Op::FileRead | Op::FileWriteNew) || is_v2(op)
}
pub(crate) fn by_name(value: &str) -> Option<Op> {
    OPERATIONS
        .into_iter()
        .find(|operation| name(*operation) == value)
}
pub(crate) fn by_id(value: &str) -> Option<Op> {
    OPERATIONS
        .into_iter()
        .find(|operation| id(*operation) == value)
}
pub(crate) const fn name(op: Op) -> &'static str {
    match op {
        Op::FileRead => READ_NAME,
        Op::FileStat => "file_stat",
        Op::FileList => "file_list",
        Op::FileCreateDir => "file_create_dir",
        Op::FileRemove => "file_remove",
        Op::FileWriteAtomic => "file_write_atomic",
        Op::FileWriteNew => WRITE_NEW_NAME,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn id(op: Op) -> &'static str {
    match op {
        Op::FileRead => READ_ID,
        Op::FileStat => "core.host.file-stat",
        Op::FileList => "core.host.file-list",
        Op::FileCreateDir => "core.host.file-create-dir",
        Op::FileRemove => "core.host.file-remove",
        Op::FileWriteAtomic => "core.host.file-write-atomic",
        Op::FileWriteNew => WRITE_NEW_ID,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn effect(op: Op) -> &'static str {
    match op {
        Op::FileRead | Op::FileStat | Op::FileList => READ_EFFECT,
        Op::FileWriteNew | Op::FileCreateDir | Op::FileRemove | Op::FileWriteAtomic => WRITE_EFFECT,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn arity(op: Op) -> usize {
    match op {
        Op::FileRead | Op::FileList => 3,
        Op::FileWriteNew | Op::FileWriteAtomic => 4,
        Op::FileStat | Op::FileCreateDir | Op::FileRemove => 2,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn ast_return_type(op: Op) -> Type {
    match op {
        Op::FileRead | Op::FileList => Type::Bytes,
        Op::FileWriteNew
        | Op::FileStat
        | Op::FileCreateDir
        | Op::FileRemove
        | Op::FileWriteAtomic => Type::Usize,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn return_type(op: Op) -> ResolvedType {
    match op {
        Op::FileRead | Op::FileList => ResolvedType::Bytes,
        Op::FileWriteNew
        | Op::FileStat
        | Op::FileCreateDir
        | Op::FileRemove
        | Op::FileWriteAtomic => ResolvedType::Usize,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn result_ownership(op: Op) -> OwnershipMode {
    match op {
        Op::FileRead | Op::FileList => OwnershipMode::Own,
        Op::FileWriteNew
        | Op::FileStat
        | Op::FileCreateDir
        | Op::FileRemove
        | Op::FileWriteAtomic => OwnershipMode::Value,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) fn accepts_ast(op: Op, index: usize, ty: &Type) -> bool {
    index < arity(op)
        && if index == 0 || (matches!(op, Op::FileWriteNew | Op::FileWriteAtomic) && index == 2) {
            *ty == Type::SliceU8
        } else {
            *ty == Type::Usize
        }
}
pub(crate) fn accepts_resolved(op: Op, index: usize, ty: &ResolvedType) -> bool {
    index < arity(op)
        && if index == 0 || (matches!(op, Op::FileWriteNew | Op::FileWriteAtomic) && index == 2) {
            *ty == ResolvedType::SliceU8
        } else {
            *ty == ResolvedType::Usize
        }
}
pub(crate) fn ast_params(op: Op) -> Vec<Param> {
    let names = match arity(op) {
        2 => &["path", "length"][..],
        3 => &["path", "length", "max"][..],
        _ => &["path", "length", "data", "data_length"][..],
    };
    names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let borrowed =
                index == 0 || (matches!(op, Op::FileWriteNew | Op::FileWriteAtomic) && index == 2);
            Param {
                name: (*name).to_owned(),
                mode: if borrowed {
                    ParamMode::Borrow
                } else {
                    ParamMode::Value
                },
                ty: if borrowed { Type::SliceU8 } else { Type::Usize },
                span: Span::default(),
            }
        })
        .collect()
}

pub(crate) fn encode_metadata(
    value: crate::filesystem_provider::FileMetadata,
) -> Result<u64, crate::filesystem_provider::FileFailure> {
    use crate::filesystem_provider::{FileFailure, FileKind};
    match value.kind {
        FileKind::File => value
            .size
            .checked_mul(4)
            .and_then(|size| size.checked_add(1))
            .ok_or(FileFailure::CapacityExceeded),
        FileKind::Directory if value.size == 0 => Ok(2),
        FileKind::Directory => Err(FileFailure::IoFailure),
    }
}

/// Validate a provider's canonical packed directory result before publishing it.
pub(crate) fn validate_listing(
    bytes: &[u8],
) -> Result<(), crate::filesystem_provider::FileFailure> {
    use crate::filesystem_provider::FileFailure::IoFailure;
    if bytes.is_empty() {
        return Ok(());
    }
    if bytes.last() != Some(&0) {
        return Err(IoFailure);
    }
    let mut previous: Option<&[u8]> = None;
    let mut count = 0usize;
    for name in bytes[..bytes.len() - 1].split(|byte| *byte == 0) {
        count += 1;
        if count > 1024
            || name.is_empty()
            || name.len() > MAX_PATH_BYTES as usize
            || name == b"."
            || name == b".."
            || name.contains(&b'/')
            || previous.is_some_and(|old| old >= name)
        {
            return Err(IoFailure);
        }
        previous = Some(name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filesystem_v2_provider_wire_is_canonical() {
        for bytes in [&b""[..], &b"a\0b\0"[..], &b"a:b\0z\\q\0\xff\0"[..]] {
            validate_listing(bytes).unwrap();
        }
        for bytes in [
            &b"\0"[..],
            &b"a"[..],
            &b"a\0a\0"[..],
            &b"b\0a\0"[..],
            &b".\0"[..],
            &b"..\0"[..],
            &b"a/b\0"[..],
        ] {
            assert!(validate_listing(bytes).is_err(), "{bytes:?}");
        }
        use crate::filesystem_provider::{FileKind, FileMetadata};
        assert_eq!(
            encode_metadata(FileMetadata {
                kind: FileKind::File,
                size: 3
            })
            .unwrap(),
            13
        );
        assert_eq!(
            encode_metadata(FileMetadata {
                kind: FileKind::Directory,
                size: 0
            })
            .unwrap(),
            2
        );
        assert!(encode_metadata(FileMetadata {
            kind: FileKind::Directory,
            size: 1
        })
        .is_err());
        assert!(encode_metadata(FileMetadata {
            kind: FileKind::File,
            size: u64::MAX
        })
        .is_err());
    }
}
