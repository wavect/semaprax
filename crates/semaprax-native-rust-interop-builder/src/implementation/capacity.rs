//! Pure source/HIR census and conservative builder-capacity proofs.
//!
//! This module performs bounded traversal and arithmetic only. It has no
//! filesystem, process, platform, publication, or settlement authority.
//!
//! The proof is staged: `source_budget` admits the source expression budget,
//! `ast_walk` and `ast_census` measure the parsed program, `declaration_dag`,
//! `cleanup_events`, and `cleanup_retained` account for cleanup, and
//! `hir_pre_resolve` and `hir_owned` bound the resolver reservation and the
//! resolved program it produces. The frame-size constants below are shared by
//! every stage.

use super::*;

mod ast_census;
mod ast_walk;
mod cleanup_events;
mod cleanup_retained;
mod declaration_dag;
mod hir_owned;
mod hir_pre_resolve;
mod source_budget;

pub(super) use ast_census::*;
pub(super) use ast_walk::*;
pub(super) use cleanup_events::*;
use cleanup_retained::*;
pub(super) use declaration_dag::*;
pub(super) use hir_owned::*;
pub(super) use hir_pre_resolve::*;
pub(super) use source_budget::*;

const HIR_RESOLVER_FRAME_BYTES: usize = 552;
const HIR_VALIDATOR_FRAME_BYTES: usize = 288;
const SOURCE_VERIFIER_FRAME_BYTES: usize = 320;
const SOURCE_VARIANT_MATCH_STATE_BYTES: usize = 312;
const CLEANUP_INVENTORY_SHAPE_FRAME_BYTES: usize = 40;
const CLEANUP_INVENTORY_EXPR_FRAME_BYTES: usize = 24;
const CLEANUP_LOWER_FRAME_BYTES: usize = 344;
const CLEANUP_EVAL_RESULT_BYTES: usize = 128;
const CALL_INDEX_FRAME_BYTES: usize = 16;
