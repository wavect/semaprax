//! Incremental grammar validation continues after semantic-buffer refusal.

use crate::ast::TypeDeclaration;

pub(super) fn source(root: &TypeDeclaration) -> String {
    include_str!("stream.spx")
        .replace("__R__", &root.name)
        .replace("__ID__", &root.stable_id)
        .replace("__FINISH_VALUE__", "if phase==0 {phase=9;true}else{if phase==3 {phase=4;true}else{if phase==6 || phase==8 {phase=7;true}else{grammar=1;grammar_at=raw;false}}}")
}
