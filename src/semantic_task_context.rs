//! Semantic Task Context v1 (issue #197): a goal-aware, token-budgeted
//! compilation over the existing bounded Agent Context v2 engine
//! ([`crate::graph::agent_context_v2_json`]).
//!
//! # What already existed before this module
//!
//! `semaprax context` already compiles one deterministic, byte- and
//! node-bounded semantic closure for exactly **one** seed identity, with
//! per-item omission reasons (`depth`, `max_nodes`, `max_bytes`,
//! `unavailable_filters`), a resumable frontier, and mandatory
//! contracts/ownership/effects/types facets. That engine owns every closure
//! rule (callers, callees, types, effects, ownership, contracts) and this
//! module never reimplements, relaxes, or re-derives any of it: every seed
//! is compiled by calling that exact function unchanged.
//!
//! # What this module adds
//!
//! 1. **A goal**: an explicit, ordered set of seeds
//!    ([`CompilationGoal`]/[`CompilationSeed`]), each an explicit stable ID
//!    plus an integer priority and an opaque `reason` string. The `reason`
//!    is untrusted, caller-supplied data -- exactly like natural-language
//!    goal text in issue #197's failure list -- and is carried through only
//!    for human explainability. It never participates in seed resolution,
//!    ranking, or budget selection; see
//!    `tests::seed_reason_text_never_influences_selection_or_budget` for a
//!    hostile-input regression proving an injection-shaped reason string
//!    changes nothing but the echoed text.
//! 2. **A token budget in an explicit, honestly named unit**
//!    ([`CompilationBudget`], [`TokenizerId`]) instead of only a byte
//!    budget. Two units are supported, and both say plainly what they are:
//!    - `byte-v1`: one budget unit per UTF-8 byte of a seed's compiled
//!      context. Exact, but explicitly not a model token.
//!    - `lexical-v1`: [`crate::agent_economics::lexical_tokens`], this
//!      repository's existing non-model lexical counter (already documented
//!      there as "deliberately not a model tokenizer"). Every output marks
//!      this unit `"exactness":"approximate"`; it is never reported or
//!      claimed as an exact model-token count.
//!
//!    Any other tokenizer identity is refused with `SPX-Z803` rather than
//!    silently downgraded to one of the two above -- a caller can never
//!    receive output that silently claims a tokenizer it did not ask for.
//! 3. **Deterministic multi-seed selection under that shared budget.** Each
//!    seed is compiled independently, then seeds are ordered by descending
//!    priority and, to break ties, ascending stable ID -- never by the
//!    order the caller listed them in, so re-listing one goal's seeds in a
//!    different order produces byte-identical output
//!    (`tests::seed_list_order_does_not_affect_output`). Seeds are then
//!    walked in that order and a seed is included exactly when the running
//!    used-token total plus its own token count does not exceed the budget;
//!    a seed that does not fit is omitted **whole** and the walk continues
//!    to the next (lower-priority) seed, so a small low-priority seed can
//!    still fill space a larger higher-priority seed left unusable. A
//!    seed's compiled JSON is never truncated mid-document: it is either
//!    included complete or omitted complete, and every omitted seed is
//!    reported with its exact token cost and the reason
//!    `"omitted_budget_exhausted"`, never silently dropped.
//! 4. **A deterministic digest** ([`compile`]'s `goal_digest` output field)
//!    binding schema, tokenizer, budget, and every seed's exact compiled
//!    content. It changes whenever the source revision, tokenizer, budget,
//!    or goal changes (`tests::digest_changes_with_revision`,
//!    `tests::digest_changes_with_tokenizer`,
//!    `tests::digest_changes_with_budget`), which is exactly the property a
//!    future cache layer would need to reject a stale entry by key
//!    mismatch. This module computes that key; it does not implement a
//!    cache store, invalidation, or persistence -- see "Honesty bar" below.
//!
//! # Deliberately out of scope here
//!
//! Issue #197 asks for a much larger surface: natural-language/lexical seed
//! *suggestion*, requirement/test/diagnostic/candidate-diff integration into
//! the closure itself, an actual persistent cache store with invalidation,
//! and CLI/MCP/SDK exposure. None of that is in this module. This is one
//! narrow, honestly-scoped slice -- the "goal representation" and "token
//! budget, not byte budget" bullets of #197's "In scope" list -- delivered
//! as a Rust-host library capability, matching the precedent
//! `semantic_embedding` set for shipping one narrow slice of a large issue
//! rather than an unverifiable broader claim (see
//! `docs/SEMANTIC-EMBEDDING-V1.md`).
//!
//! # No cross-seed deduplication
//!
//! Two seeds whose closures overlap (for example, two functions that share
//! a common callee) each compile their **own** independent context; a
//! shared declaration's facts appear once per seed that reaches it, and its
//! token cost is charged once per seed. This module does not merge or
//! deduplicate declaration facts across seeds -- doing so would require
//! re-deriving the single-seed engine's own per-declaration facet rules,
//! which this module is designed specifically not to duplicate. A caller
//! that wants only the smallest possible closure over many related seeds
//! should still call the existing multi-seed-unaware engine directly with
//! the union of call sites as its one root, when that shape fits.
//!
//! # No ambient authority
//!
//! [`compile`] takes an already-parsed `&Program` and calls only
//! [`crate::graph::agent_context_v2_json`] and
//! [`crate::agent_economics::lexical_tokens`]. It opens no file, spawns no
//! process, and contacts no network.
//!
//! # Honesty bar
//!
//! This module claims exactly four things: an explicit multi-seed goal
//! representation, an explicit and honestly labeled token-accounting unit,
//! deterministic whole-seed selection under a real budget enforced (not
//! advisory) at an exact boundary, and a cache-key digest sensitive to every
//! field that determines the output. It does not claim natural-language
//! goal understanding, cross-seed semantic deduplication, a working cache,
//! requirement/test-facing integration, or any CLI/MCP/SDK surface.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::agent_economics::lexical_tokens;
use crate::ast::Program;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::digest_hex::LowerHex;
use crate::graph::{self, AgentContextV2Options};

/// Schema identity of the compiled goal-aware bundle this module renders.
pub const SCHEMA: &str = "semaprax.semantic-task-context.v1";
/// Smallest accepted token budget.
pub const MIN_MAX_TOKENS: usize = 1;
/// Largest accepted token budget. Generous on purpose: the unit-dependent
/// ceiling is enforced per seed by the underlying byte-bounded engine, not
/// here.
pub const MAX_MAX_TOKENS: usize = 64 * 1024 * 1024;

const DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-task-context.goal-digest.v1\0";

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z801", message)
}

fn seed_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z802", message)
}

fn tokenizer_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z803", message)
}

fn seed_not_found(id: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z804",
        format!("goal seed `{id}` does not resolve to a context root"),
    )
}

/// One caller-declared contribution to a [`CompilationGoal`].
///
/// `reason` is untrusted, caller-supplied explanatory text (it may hold
/// natural language). It is echoed back in [`compile`]'s output for human
/// readers and never inspected by this module's selection logic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationSeed {
    id: String,
    priority: u32,
    reason: String,
}

impl CompilationSeed {
    #[must_use]
    pub fn new(id: impl Into<String>, priority: u32, reason: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            priority,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// A structured goal: an explicit, deduplicated set of seeds. See the module
/// docs for why no natural-language text drives selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationGoal {
    seeds: Vec<CompilationSeed>,
}

impl CompilationGoal {
    pub fn new(seeds: Vec<CompilationSeed>) -> Result<Self, Diagnostic> {
        if seeds.is_empty() {
            return Err(seed_error(
                "a compilation goal requires at least one seed".to_owned(),
            ));
        }
        let mut ids = BTreeSet::new();
        for seed in &seeds {
            if seed.id.is_empty() {
                return Err(seed_error(
                    "a compilation seed id must be nonempty".to_owned(),
                ));
            }
            if !ids.insert(seed.id.as_str()) {
                return Err(seed_error(format!(
                    "seed `{}` is duplicated in this goal",
                    seed.id
                )));
            }
        }
        Ok(Self { seeds })
    }
}

/// One explicit, honestly labeled token-accounting unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenizerId {
    /// One unit per UTF-8 byte. Exact, but plainly not a model token.
    Byte,
    /// `agent_economics::lexical_tokens`. Always reported `"approximate"`.
    LexicalApprox,
}

impl TokenizerId {
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "byte-v1" => Some(Self::Byte),
            "lexical-v1" => Some(Self::LexicalApprox),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Byte => "byte-v1",
            Self::LexicalApprox => "lexical-v1",
        }
    }

    /// `"exact"` for `byte-v1` (an exact byte count, never claimed to be a
    /// model token), `"approximate"` for `lexical-v1` (a non-model lexical
    /// estimate). Neither claims a real model tokenizer's exact count.
    #[must_use]
    pub const fn exactness(self) -> &'static str {
        match self {
            Self::Byte => "exact",
            Self::LexicalApprox => "approximate",
        }
    }

    fn count(self, text: &str) -> usize {
        match self {
            Self::Byte => text.len(),
            Self::LexicalApprox => lexical_tokens(text),
        }
    }
}

/// Validated token budget for one [`compile`] call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilationBudget {
    max_tokens: usize,
    tokenizer: TokenizerId,
}

impl CompilationBudget {
    /// Refuses an unsupported `tokenizer_name` (`SPX-Z803`) instead of
    /// silently falling back to a different unit, and refuses a `max_tokens`
    /// outside `MIN_MAX_TOKENS..=MAX_MAX_TOKENS` (`SPX-Z801`).
    pub fn new(max_tokens: usize, tokenizer_name: &str) -> Result<Self, Diagnostic> {
        let Some(tokenizer) = TokenizerId::parse(tokenizer_name) else {
            return Err(tokenizer_error(format!(
                "tokenizer `{tokenizer_name}` is unavailable; exact-token mode is refused rather \
                 than approximated. Use `byte-v1` (exact byte accounting, not a model token) or \
                 `lexical-v1` (approximate lexical-unit accounting, never reported as exact)"
            )));
        };
        if !(MIN_MAX_TOKENS..=MAX_MAX_TOKENS).contains(&max_tokens) {
            return Err(option_error(format!(
                "semantic task context max_tokens {max_tokens} is outside \
                 {MIN_MAX_TOKENS}..={MAX_MAX_TOKENS}"
            )));
        }
        Ok(Self {
            max_tokens,
            tokenizer,
        })
    }
}

struct CompiledSeed<'a> {
    seed: &'a CompilationSeed,
    json: String,
    tokens: usize,
}

/// Compile one goal-aware, token-budgeted [`SCHEMA`] bundle.
///
/// Every seed is resolved by calling
/// [`crate::graph::agent_context_v2_json`] unchanged with `per_seed_options`
/// shared across every seed; an unresolved seed id fails the whole call
/// closed (`SPX-Z804`) rather than silently dropping it, because a goal's
/// seeds are explicit stable IDs the caller is expected to have already
/// resolved. See the module docs for the deterministic merge and budget
/// rules.
pub fn compile(
    program: &Program,
    goal: &CompilationGoal,
    per_seed_options: &AgentContextV2Options,
    budget: CompilationBudget,
) -> Result<String, Vec<Diagnostic>> {
    let source_revision = graph::revision(program);

    let mut compiled = Vec::with_capacity(goal.seeds.len());
    for seed in &goal.seeds {
        let json = graph::agent_context_v2_json(program, &seed.id, per_seed_options)?
            .ok_or_else(|| vec![seed_not_found(&seed.id)])?;
        let tokens = budget.tokenizer.count(&json);
        compiled.push(CompiledSeed { seed, json, tokens });
    }

    // Deterministic merge order: descending priority, then ascending stable
    // ID. Never the caller's list order or any hash-map iteration order.
    compiled.sort_by(|a, b| {
        b.seed
            .priority
            .cmp(&a.seed.priority)
            .then_with(|| a.seed.id.cmp(&b.seed.id))
    });

    let mut used_tokens = 0usize;
    let mut entries = Vec::with_capacity(compiled.len());
    for item in &compiled {
        let included = used_tokens.saturating_add(item.tokens) <= budget.max_tokens;
        if included {
            used_tokens += item.tokens;
        }
        entries.push(render_entry(item, included));
    }

    let goal_digest = digest(&source_revision, budget, &compiled);

    Ok(format!(
        "{{\"schema\":{schema},\"source_revision\":{revision},\"goal_digest\":{digest},\
         \"budget\":{{\"tokenizer\":{tokenizer},\"exactness\":{exactness},\"max_tokens\":{max_tokens},\
         \"used_tokens\":{used_tokens}}},\"seeds\":[{entries}]}}",
        schema = quote_json(SCHEMA),
        revision = quote_json(&source_revision),
        digest = quote_json(&goal_digest),
        tokenizer = quote_json(budget.tokenizer.name()),
        exactness = quote_json(budget.tokenizer.exactness()),
        max_tokens = budget.max_tokens,
        used_tokens = used_tokens,
        entries = entries.join(","),
    ))
}

fn render_entry(item: &CompiledSeed<'_>, included: bool) -> String {
    if included {
        format!(
            "{{\"id\":{id},\"priority\":{priority},\"reason\":{reason},\"tokens\":{tokens},\
             \"status\":\"included\",\"context\":{context}}}",
            id = quote_json(&item.seed.id),
            priority = item.seed.priority,
            reason = quote_json(&item.seed.reason),
            tokens = item.tokens,
            context = item.json,
        )
    } else {
        format!(
            "{{\"id\":{id},\"priority\":{priority},\"reason\":{reason},\"tokens\":{tokens},\
             \"status\":\"omitted_budget_exhausted\"}}",
            id = quote_json(&item.seed.id),
            priority = item.seed.priority,
            reason = quote_json(&item.seed.reason),
            tokens = item.tokens,
        )
    }
}

/// A digest sensitive to every field that determines `compile`'s exact
/// output byte-for-byte: the source revision, the tokenizer and budget, and
/// each seed's identity, priority, reason, and exact compiled content -- in
/// the same order `compile` renders them in, so it is insensitive to the
/// order the caller originally listed seeds in, exactly like the rendered
/// output itself.
fn digest(
    source_revision: &str,
    budget: CompilationBudget,
    compiled: &[CompiledSeed<'_>],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    update_field(&mut hasher, SCHEMA.as_bytes());
    update_field(&mut hasher, source_revision.as_bytes());
    update_field(&mut hasher, budget.tokenizer.name().as_bytes());
    update_field(&mut hasher, &budget.max_tokens.to_le_bytes());
    for item in compiled {
        update_field(&mut hasher, item.seed.id.as_bytes());
        update_field(&mut hasher, &item.seed.priority.to_le_bytes());
        update_field(&mut hasher, item.seed.reason.as_bytes());
        update_field(&mut hasher, item.json.as_bytes());
    }
    format!("sha256:{:x}", LowerHex(hasher.finalize()))
}

fn update_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests;
