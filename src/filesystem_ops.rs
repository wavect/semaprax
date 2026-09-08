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
pub(crate) const OPERATIONS: [Op; 2] = [Op::FileRead, Op::FileWriteNew];
pub(crate) const fn is_filesystem(op: Op) -> bool {
    matches!(op, Op::FileRead | Op::FileWriteNew)
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
        Op::FileWriteNew => WRITE_NEW_NAME,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn id(op: Op) -> &'static str {
    match op {
        Op::FileRead => READ_ID,
        Op::FileWriteNew => WRITE_NEW_ID,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn effect(op: Op) -> &'static str {
    match op {
        Op::FileRead => READ_EFFECT,
        Op::FileWriteNew => WRITE_EFFECT,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn arity(op: Op) -> usize {
    match op {
        Op::FileRead => 3,
        Op::FileWriteNew => 4,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn ast_return_type(op: Op) -> Type {
    match op {
        Op::FileRead => Type::Bytes,
        Op::FileWriteNew => Type::Usize,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn return_type(op: Op) -> ResolvedType {
    match op {
        Op::FileRead => ResolvedType::Bytes,
        Op::FileWriteNew => ResolvedType::Usize,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) const fn result_ownership(op: Op) -> OwnershipMode {
    match op {
        Op::FileRead => OwnershipMode::Own,
        Op::FileWriteNew => OwnershipMode::Value,
        _ => panic!("not a filesystem operation"),
    }
}
pub(crate) fn accepts_ast(op: Op, index: usize, ty: &Type) -> bool {
    index < arity(op)
        && if index == 0 || (op == Op::FileWriteNew && index == 2) {
            *ty == Type::SliceU8
        } else {
            *ty == Type::Usize
        }
}
pub(crate) fn accepts_resolved(op: Op, index: usize, ty: &ResolvedType) -> bool {
    index < arity(op)
        && if index == 0 || (op == Op::FileWriteNew && index == 2) {
            *ty == ResolvedType::SliceU8
        } else {
            *ty == ResolvedType::Usize
        }
}
pub(crate) fn ast_params(op: Op) -> Vec<Param> {
    let names = if op == Op::FileRead {
        &["path", "length", "max"][..]
    } else {
        &["path", "length", "data", "data_length"][..]
    };
    names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let borrowed = index == 0 || (op == Op::FileWriteNew && index == 2);
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
