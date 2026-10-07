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
//!    binding schema, tokenizer (including its [`TokenizerId::algorithm_digest`],
//!    not only its short name), budget, and every seed's exact compiled
//!    content. It changes whenever the source revision, tokenizer, budget,
//!    or goal changes (`tests::digest_changes_with_revision`,
//!    `tests::digest_changes_with_tokenizer`,
//!    `tests::digest_changes_with_budget`).
//! 5. **An explicit tokenizer algorithm identity** ([`TokenizerId::algorithm_digest`]),
//!    separate from and stricter than the short unit name. If a tokenizer's
//!    *counting behavior* ever changes without its short name changing --
//!    "tokenizer version drift changes the budget" in issue #197's failure
//!    list -- this digest is the thing a future change is obligated to bump,
//!    and every [`cache_key`] folds it in, so an unnoticed drift cannot
//!    silently reuse a stale cache entry keyed only by name.
//! 6. **A pre-compile cache key** ([`cache_key`], [`TaskContextCache`],
//!    [`compile_cached`]): an input-only digest over the source revision,
//!    the goal's seeds (order-independent, like [`compile`]'s own merge), the
//!    shared per-seed policy (`AgentContextV2Options`'s `Debug` projection --
//!    exposed accessors don't cover every field, and this module does not
//!    duplicate the underlying engine's private representation to get one),
//!    the tokenizer and budget, and a caller-declared `access_scope` string
//!    naming the caller's own authorization boundary (the security list's
//!    "context caching can leak source across authorization boundaries").
//!    [`TaskContextCache`] is a plain in-memory key-value store: no eviction,
//!    no expiry, no persistence. It stores exactly the bytes `compile` would
//!    have produced for that exact key and never returns a value for a key
//!    it was not explicitly given.
//! 7. **A lexical seed suggestion** ([`suggest_seeds`]), deterministic
//!    word-overlap ranking over each declaration's plain name and leading doc
//!    comment lines (`crate::doc::document`'s `Entry::name`/`description`)
//!    against a caller-supplied, untrusted `query` string. It is a pure,
//!    side-effect-free function returning suggestions for a human or calling
//!    tool to review; nothing in this module calls it, and it never builds,
//!    mutates, or feeds a [`CompilationGoal`] on its own --
//!    `tests::suggest_seeds_never_influences_a_goal_that_does_not_explicitly_include_it`
//!    proves calling it changes nothing about a subsequent unrelated
//!    [`compile`] call.
//! 8. **An explicit inclusion-policy summary** in every [`compile`] bundle's
//!    top-level `policy` field (`depth`, `max_bytes`, `max_nodes`,
//!    `direction`) -- the shared `AgentContextV2Options` this call used,
//!    surfaced once at the bundle level rather than only inside each seed's
//!    own embedded `query` object. `filters` is deliberately absent from this
//!    summary: `AgentContextV2Options` exposes no public accessor for it, and
//!    it remains visible per seed inside that seed's own compiled content.
//! 9. **A second, genuinely derivable seed kind**
//!    ([`seed_id_for_diagnostic`], [`CompilationSeed::from_diagnostic`]):
//!    a seed resolved from a diagnostic's structural `span` -- the byte
//!    range the parser/resolver already computed -- to the smallest
//!    enclosing function, type, or class method's persistent stable id.
//!    `diagnostic.message` (free text, possibly natural language) is never
//!    inspected; only `span` and the declarations' own
//!    `stable_id`/`span` fields decide the result
//!    (`tests::diagnostic_derived_seed_ignores_message_text`). Proven to
//!    resolve to the exact id, and therefore compile to the exact closure,
//!    a hand-written [`CompilationSeed::new`] with that id would
//!    (`tests::diagnostic_derived_seed_selects_the_same_closure_as_the_hand_written_stable_id`).
//! 10. **Cross-seed deduplication** ([`dedup_seed_json`], internal to
//!    [`compile`]). A fact `agent_context_v2_json` renders for one
//!    declaration id is a pure function of `(program, id, filters, schema)`
//!    (see the module-internal "Cross-seed deduplication" section below for
//!    why), so two seeds sharing `per_seed_options` that both reach the same
//!    declaration render byte-identical fact text for it. When a
//!    lower-priority seed's closure reaches a declaration an earlier,
//!    higher-priority *included* seed in the same goal already delivered in
//!    full, this seed's own copy is replaced by a small
//!    `{"id":...,"deduplicated_owner_seed":...}` reference stub instead of
//!    being re-embedded and re-charged against the budget -- real savings,
//!    not merely an accounting fiction: the stub is what is actually
//!    embedded and what `byte-v1`/`lexical-v1` actually count, so the exact
//!    tokenizer's "exact" label still means exactly what it always meant
//!    (the literal size of what this call embeds). A seed's own root
//!    declaration is always kept in full inside its own entry, even when an
//!    earlier seed's closure already carries a copy of the same content, so
//!    reading one seed's entry never requires chasing a reference elsewhere
//!    to see what was actually asked for. Proven
//!    (`tests::cross_seed_dedup_total_is_byte_identical_to_the_manually_unioned_total`)
//!    against an independently, manually unioned reference total -- not
//!    merely smaller than the naive double-counted total, but byte-for-byte
//!    equal to it.
//! 11. **Requirement/test/candidate-diff facets integrated into the closure**
//!    ([`DeclarationFacets`], [`compile_with_declaration_facets`]), attached
//!    per declaration id inside the same bundle a caller already reads,
//!    rather than a separate report the caller must cross-reference by hand.
//!    This module still has no ambient authority to discover requirements,
//!    tests, or a candidate diff itself; a caller who already has this data
//!    (for example from [`crate::requirement_traceability`], a project's own
//!    test index, or a [`crate::patch`]-computed diff) supplies it, keyed by
//!    exact declaration id, never by free text.
//!
//! # Deliberately out of scope here
//!
//! Issue #197 asks for a much larger surface this module still does not
//! cover: real content summarization of a distant or omitted item (this
//! module and the engine it composes only ever include or omit a whole typed
//! unit, or a dedup reference stub, never a compressed substitute for one),
//! automatic discovery of requirement/test/candidate-diff data (residual 3 is
//! met only for caller-supplied data joined by exact id -- this module still
//! opens no file and calls no other subsystem to derive it itself), and
//! CLI/MCP/SDK exposure of the cache, suggestion, diagnostic-derived seed,
//! and declaration-facets additions (the existing `compact task-context` CLI
//! route, documented in `docs/SEMANTIC-TASK-CONTEXT-V1.md`, wires the
//! goal/budget surface only). This is still one honestly-scoped slice,
//! matching the precedent `semantic_embedding` set for shipping one narrow
//! slice of a large issue rather than an unverifiable broader claim (see
//! `docs/SEMANTIC-EMBEDDING-V1.md`).
//!
//! # No ambient authority
//!
//! [`compile`] takes an already-parsed `&Program` and calls only
//! [`crate::graph::agent_context_v2_json`] and
//! [`crate::agent_economics::lexical_tokens`]. It opens no file, spawns no
//! process, and contacts no network. [`suggest_seeds`] additionally calls
//! [`crate::doc::document`] on the same already-parsed `&Program` plus a
//! caller-supplied `&Comments` -- it does not lex or read anything itself.
//! [`seed_id_for_diagnostic`] reads only `program.functions`/`program.types`
//! (and their spans/stable ids) and the caller-supplied `Diagnostic`'s
//! `span`; it never reads `diagnostic.message`. [`TaskContextCache`] holds
//! compiled bytes only in process memory; it opens no file and outlives
//! nothing beyond the caller's own process.
//! [`compile_with_declaration_facets`] takes its facets as an argument the
//! caller already computed; it discovers none of that data itself.
//!
//! # Honesty bar
//!
//! This module claims exactly ten things: an explicit multi-seed goal
//! representation, a second seed kind derived from a diagnostic's structural
//! span rather than its message, an explicit and honestly labeled
//! token-accounting unit carrying its own algorithm-identity digest,
//! deterministic whole-seed selection under a real budget enforced (not
//! advisory) at an exact boundary, real cross-seed deduplication that
//! shrinks what is actually embedded (not only what is accounted), a
//! cache-key digest sensitive to every field that determines the output, a
//! working (if unbounded, unevicting) in-memory cache keyed by that digest
//! and separated by caller-declared access scope, a deterministic lexical
//! seed *suggestion* that never influences selection on its own, a
//! top-level summary of the shared inclusion policy's exposable fields, and
//! caller-supplied requirement/test/candidate-diff facets joined into the
//! closure by exact declaration id. It does not claim natural-language goal
//! *understanding*, automatic requirement/test/candidate-diff discovery,
//! real content summarization of an omitted or distant item, cache eviction
//! or persistence, or any CLI/MCP/SDK surface for the cache, suggestion,
//! diagnostic-derived seed, or declaration-facets additions specifically
//! (the existing `compact task-context` route, described in
//! `docs/SEMANTIC-TASK-CONTEXT-V1.md`, covers goal/budget only).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sha2::{Digest, Sha256};

use crate::agent_economics::lexical_tokens;
use crate::ast::{Program, Span, TypeDeclarationKind};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::digest_hex::LowerHex;
use crate::doc;
use crate::format::comments::Comments;
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
const FACET_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-task-context.goal-digest.facets.v1\0";
const MAX_FACET_IDENTIFIER_BYTES: usize = 4 * 1024;
const MAX_DECLARATION_FACET_BYTES: usize = 1024 * 1024;
const MAX_DECLARATION_FACET_ENTRIES: usize = 65_536;
/// Domain separator for [`cache_key`]'s input-only digest. Distinct from
/// [`DIGEST_DOMAIN`] on purpose: the two digests are never comparable, and a
/// value produced under one must never be mistaken for the other.
const CACHE_KEY_DOMAIN: &[u8] = b"semaprax.semantic-task-context.cache-key.v1\0";
/// Domain separator for each [`TokenizerId`]'s `byte-v1` algorithm identity.
const TOKENIZER_ALGORITHM_DOMAIN_BYTE: &[u8] =
    b"semaprax.semantic-task-context.tokenizer.byte-v1.utf8-byte-count.v1\0";
/// Domain separator for each [`TokenizerId`]'s `lexical-v1` algorithm
/// identity.
const TOKENIZER_ALGORITHM_DOMAIN_LEXICAL: &[u8] =
    b"semaprax.semantic-task-context.tokenizer.lexical-v1.agent-economics-lexical-tokens.v1\0";

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

fn diagnostic_seed_not_found(diagnostic: &Diagnostic) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z805",
        match diagnostic.span {
            Some(span) => format!(
                "diagnostic `{}` at byte offset {} does not resolve to any function or type \
                 declaration in this program",
                diagnostic.code, span.start
            ),
            None => format!(
                "diagnostic `{}` carries no span and cannot be resolved to a declaration",
                diagnostic.code
            ),
        },
    )
}

/// Resolve a diagnostic's structural span to the persistent stable id of the
/// smallest top-level function, type declaration, or class method whose own
/// span encloses it -- using only [`crate::ast::Function::span`] /
/// [`crate::ast::TypeDeclaration::span`] and their `stable_id`, never the
/// diagnostic's `message` text. This is issue #197's "seed detection beyond
/// stable IDs" residual: a second, genuinely derivable seed kind, proven
/// (`tests::diagnostic_derived_seed_selects_the_same_closure_as_the_hand_written_stable_id`)
/// to resolve to the exact same stable id -- and therefore compile to the
/// exact same closure -- a caller who already knew that id would have
/// written by hand. `message` is never inspected
/// (`tests::diagnostic_derived_seed_ignores_message_text`): only `span`, a
/// byte range the parser/resolver already computed, decides the result, so
/// a diagnostic's caller-authored explanatory text can never steer which
/// seed is derived.
///
/// Returns `None` when `diagnostic` carries no span, or its span falls
/// within no declaration in `program` (for example a module-level
/// diagnostic). Only `functions`, `types`, and class `methods` are
/// searched; interfaces, protocols, implementations, and agent
/// declarations are not yet covered by this derivation.
#[must_use]
pub fn seed_id_for_diagnostic(program: &Program, diagnostic: &Diagnostic) -> Option<String> {
    let span = diagnostic.span?;
    let mut best: Option<(usize, String)> = None;
    let mut consider = |candidate_span: Span, id: &str, best: &mut Option<(usize, String)>| {
        if candidate_span.start > span.start || span.start >= candidate_span.end {
            return;
        }
        let width = candidate_span.end - candidate_span.start;
        let better = match best {
            None => true,
            Some((best_width, _)) => width < *best_width,
        };
        if better {
            *best = Some((width, id.to_owned()));
        }
    };
    for function in &program.functions {
        consider(function.span, &function.stable_id, &mut best);
    }
    for declaration in &program.types {
        consider(declaration.span, &declaration.stable_id, &mut best);
        if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
            for method in methods {
                consider(method.span, &method.stable_id, &mut best);
            }
        }
    }
    best.map(|(_, id)| id)
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

    /// Construct a seed from a diagnostic's structural span rather than a
    /// hand-written stable id -- see [`seed_id_for_diagnostic`], issue
    /// #197's "seed detection beyond stable IDs" residual. Fails closed
    /// (`SPX-Z805`) instead of silently falling back to a default seed when
    /// the diagnostic carries no span or its span resolves to no
    /// declaration. `reason` remains caller-declared, untrusted
    /// explanatory text with the same guarantee as [`Self::new`]: the
    /// derivation itself never reads `diagnostic.message`, only its
    /// `span`.
    pub fn from_diagnostic(
        program: &Program,
        diagnostic: &Diagnostic,
        priority: u32,
        reason: impl Into<String>,
    ) -> Result<Self, Diagnostic> {
        let id = seed_id_for_diagnostic(program, diagnostic)
            .ok_or_else(|| diagnostic_seed_not_found(diagnostic))?;
        Ok(Self::new(id, priority, reason))
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

    /// An explicit content digest of this tokenizer's exact counting
    /// algorithm, independent of and stricter than its short [`Self::name`].
    /// Folded into every [`compile`] bundle's `budget.tokenizer_digest` field
    /// and into every [`cache_key`]. Issue #197's failure list names
    /// "tokenizer version drift" -- a counting algorithm's behavior changing
    /// without its short name changing -- as a risk; this digest is the
    /// value a future change to either counting algorithm is obligated to
    /// bump, so a drift that forgets to also change `name()` still changes
    /// this digest and therefore still invalidates a cache keyed on it. This
    /// module cannot detect an undeclared drift by itself -- only a
    /// discipline of bumping the domain string below when behavior changes.
    #[must_use]
    pub fn algorithm_digest(self) -> String {
        let domain: &[u8] = match self {
            Self::Byte => TOKENIZER_ALGORITHM_DOMAIN_BYTE,
            Self::LexicalApprox => TOKENIZER_ALGORITHM_DOMAIN_LEXICAL,
        };
        let mut hasher = Sha256::new();
        hasher.update(domain);
        format!("sha256:{:x}", LowerHex(hasher.finalize()))
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
}

/// The one deterministic seed order this module ever uses: descending
/// priority, then ascending stable ID. Shared by [`compile`]'s merge and
/// [`cache_key`]'s seed hashing so the two agree on what "the same goal"
/// means regardless of the caller's original list order.
fn seed_order_key(seed: &CompilationSeed) -> (std::cmp::Reverse<u32>, &str) {
    (std::cmp::Reverse(seed.priority), seed.id.as_str())
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
    compile_inner(program, goal, per_seed_options, budget, None)
}

/// [`compile`], plus caller-supplied [`DeclarationFacets`] joined into each
/// included seed's own closure by exact declaration id -- issue #197's
/// residual 3 ("requirement, test, and candidate-diff facets integrated
/// into the closure itself, rather than available separately"). A separate
/// function rather than a new parameter on [`compile`]: this module's
/// existing callers (the `compact task-context` CLI route) call `compile`
/// directly and are outside this change's file lease, so `compile`'s
/// signature and byte-for-byte output are left completely unchanged; a
/// caller that has no facet data keeps calling `compile` exactly as before.
///
/// This module still has no ambient authority to discover requirements,
/// tests, or a candidate diff itself -- no file is opened, no other module
/// is called into for this data. A caller who already computed it (for
/// example from [`crate::requirement_traceability`], a project's own test
/// index, or a [`crate::patch`]-computed diff) supplies it as
/// [`DeclarationFacets`], keyed by the exact declaration ids the facets
/// name; nothing here interprets free text.
///
/// The facet-bearing route counts each seed's compact context and selected
/// canonical facet array as one semantic payload, and binds the rendered seed
/// entries to a distinct facet-aware goal-digest domain. Facet inputs are
/// bounded before any seed context is compiled.
pub fn compile_with_declaration_facets(
    program: &Program,
    goal: &CompilationGoal,
    per_seed_options: &AgentContextV2Options,
    budget: CompilationBudget,
    facets: &DeclarationFacets,
) -> Result<String, Vec<Diagnostic>> {
    facets.validate().map_err(|diagnostic| vec![diagnostic])?;
    compile_inner(program, goal, per_seed_options, budget, Some(facets))
}

fn compile_inner(
    program: &Program,
    goal: &CompilationGoal,
    per_seed_options: &AgentContextV2Options,
    budget: CompilationBudget,
    facets: Option<&DeclarationFacets>,
) -> Result<String, Vec<Diagnostic>> {
    let source_revision = graph::revision(program);

    let mut compiled = Vec::with_capacity(goal.seeds.len());
    for seed in &goal.seeds {
        let json = graph::agent_context_v2_json(program, &seed.id, per_seed_options)?
            .ok_or_else(|| vec![seed_not_found(&seed.id)])?;
        compiled.push(CompiledSeed { seed, json });
    }

    // Deterministic merge order: descending priority, then ascending stable
    // ID. Never the caller's list order or any hash-map iteration order.
    compiled.sort_by(|a, b| seed_order_key(a.seed).cmp(&seed_order_key(b.seed)));

    // Cross-seed deduplication (issue #197 residual 2). `seen_fact_ids` and
    // `owner_of_fact` accumulate only across seeds that actually end up
    // `included`, walked in the same deterministic merge order as above, so
    // a fact's dedup "owner" is always the first included, highest-priority
    // seed whose closure reaches it. See `dedup_seed_json`.
    let mut used_tokens = 0usize;
    let mut seen_fact_ids: BTreeSet<String> = BTreeSet::new();
    let mut owner_of_fact: BTreeMap<String, String> = BTreeMap::new();
    let mut entries = Vec::with_capacity(compiled.len());
    for item in &compiled {
        let dedup = dedup_seed_json(&item.json, &item.seed.id, &seen_fact_ids, &owner_of_fact);
        let facet_entries = facets.map(|facets| facets.render_for(&dedup.all_ids));
        let semantic_payload = facet_entries.as_ref().map_or_else(
            || dedup.rewritten_json.clone(),
            |facet_entries| {
                format!(
                    "{{\"context\":{},\"declaration_facets\":{facet_entries}}}",
                    dedup.rewritten_json
                )
            },
        );
        let tokens = budget.tokenizer.count(&semantic_payload);
        let included = used_tokens.saturating_add(tokens) <= budget.max_tokens;
        if included {
            used_tokens += tokens;
            for id in dedup.newly_present_ids {
                seen_fact_ids.insert(id.clone());
                owner_of_fact
                    .entry(id)
                    .or_insert_with(|| item.seed.id.clone());
            }
        }
        entries.push(render_entry(
            item,
            included,
            tokens,
            &dedup.rewritten_json,
            facet_entries.as_deref(),
        ));
    }

    let goal_digest = if facets.is_some() {
        facet_digest(&source_revision, budget, &entries)
    } else {
        digest(&source_revision, budget, &compiled)
    };

    Ok(format!(
        "{{\"schema\":{schema},\"source_revision\":{revision},\"goal_digest\":{digest},\
         \"budget\":{{\"tokenizer\":{tokenizer},\"tokenizer_digest\":{tokenizer_digest},\
         \"exactness\":{exactness},\"max_tokens\":{max_tokens},\
         \"used_tokens\":{used_tokens}}},\"policy\":{{\"depth\":{depth},\"max_bytes\":{max_bytes},\
         \"max_nodes\":{max_nodes},\"direction\":{direction}}},\"seeds\":[{entries}]}}",
        schema = quote_json(SCHEMA),
        revision = quote_json(&source_revision),
        digest = quote_json(&goal_digest),
        tokenizer = quote_json(budget.tokenizer.name()),
        tokenizer_digest = quote_json(&budget.tokenizer.algorithm_digest()),
        exactness = quote_json(budget.tokenizer.exactness()),
        max_tokens = budget.max_tokens,
        used_tokens = used_tokens,
        depth = per_seed_options.depth(),
        max_bytes = per_seed_options.max_bytes(),
        max_nodes = per_seed_options.max_nodes(),
        direction = quote_json(per_seed_options.direction().name()),
        entries = entries.join(","),
    ))
}

fn render_entry(
    item: &CompiledSeed<'_>,
    included: bool,
    tokens: usize,
    rewritten_json: &str,
    facet_entries: Option<&str>,
) -> String {
    if !included {
        return format!(
            "{{\"id\":{id},\"priority\":{priority},\"reason\":{reason},\"tokens\":{tokens},\
             \"status\":\"omitted_budget_exhausted\"}}",
            id = quote_json(&item.seed.id),
            priority = item.seed.priority,
            reason = quote_json(&item.seed.reason),
            tokens = tokens,
        );
    }
    let mut out = format!(
        "{{\"id\":{id},\"priority\":{priority},\"reason\":{reason},\"tokens\":{tokens},\
         \"status\":\"included\",\"context\":{context}",
        id = quote_json(&item.seed.id),
        priority = item.seed.priority,
        reason = quote_json(&item.seed.reason),
        tokens = tokens,
        context = rewritten_json,
    );
    if let Some(facet_entries) = facet_entries {
        out.push_str(",\"declaration_facets\":");
        out.push_str(facet_entries);
    }
    out.push('}');
    out
}

fn facet_digest(source_revision: &str, budget: CompilationBudget, entries: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(FACET_DIGEST_DOMAIN);
    update_field(&mut hasher, SCHEMA.as_bytes());
    update_field(&mut hasher, source_revision.as_bytes());
    update_field(&mut hasher, budget.tokenizer.name().as_bytes());
    update_field(&mut hasher, budget.tokenizer.algorithm_digest().as_bytes());
    update_field(&mut hasher, &budget.max_tokens.to_le_bytes());
    for entry in entries {
        update_field(&mut hasher, entry.as_bytes());
    }
    format!("sha256:{:x}", LowerHex(hasher.finalize()))
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
    update_field(&mut hasher, budget.tokenizer.algorithm_digest().as_bytes());
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

/// An opaque, input-only cache key from [`cache_key`]. Unlike `compile`'s
/// `goal_digest` (which is computed *from* the compiled output and therefore
/// cannot be known before compiling), this key is computed entirely from the
/// declared inputs, so a caller -- or [`compile_cached`] -- can check for a
/// cache hit without paying for a fresh compile first.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CacheKey(String);

/// Compute the pre-compile cache key for one `(source_revision, goal,
/// per_seed_options, budget, access_scope)` input tuple.
///
/// Folds in, in order: the schema, the exact source revision, the tokenizer's
/// name and [`TokenizerId::algorithm_digest`], the budget's `max_tokens`, the
/// caller-declared `access_scope` (the security list's "context caching can
/// leak source across authorization boundaries" -- two calls with the same
/// revision/goal/policy/tokenizer/budget but different `access_scope` never
/// collide), the shared per-seed policy's `Debug` projection (this module has
/// no public accessor for every `AgentContextV2Options` field -- notably
/// `filters` -- so it hashes the same `Debug` text the engine itself would
/// print rather than re-deriving a private representation), and every seed's
/// identity/priority/reason in the same order-independent sequence
/// [`compile`] itself merges by (see [`seed_order_key`]), so listing one
/// goal's seeds in a different order yields the same key.
///
/// `access_scope` is opaque, caller-declared data (for example a workspace
/// or session id already established by the caller's own authorization
/// layer). This function does not itself perform or verify access control;
/// it only ensures two different scopes are never keyed identically.
#[must_use]
pub fn cache_key(
    source_revision: &str,
    goal: &CompilationGoal,
    per_seed_options: &AgentContextV2Options,
    budget: CompilationBudget,
    access_scope: &str,
) -> CacheKey {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_KEY_DOMAIN);
    update_field(&mut hasher, SCHEMA.as_bytes());
    update_field(&mut hasher, source_revision.as_bytes());
    update_field(&mut hasher, budget.tokenizer.name().as_bytes());
    update_field(&mut hasher, budget.tokenizer.algorithm_digest().as_bytes());
    update_field(&mut hasher, &budget.max_tokens.to_le_bytes());
    update_field(&mut hasher, access_scope.as_bytes());
    update_field(&mut hasher, format!("{per_seed_options:?}").as_bytes());
    let mut ordered_seeds: Vec<&CompilationSeed> = goal.seeds.iter().collect();
    ordered_seeds.sort_by(|a, b| seed_order_key(a).cmp(&seed_order_key(b)));
    for seed in ordered_seeds {
        update_field(&mut hasher, seed.id.as_bytes());
        update_field(&mut hasher, &seed.priority.to_le_bytes());
        update_field(&mut hasher, seed.reason.as_bytes());
    }
    CacheKey(format!("sha256:{:x}", LowerHex(hasher.finalize())))
}

/// A plain in-memory replay cache for [`compile`]'s output, keyed by
/// [`CacheKey`].
///
/// This is intentionally minimal: no eviction, no expiry, no size bound, no
/// persistence across process restarts. It stores exactly the bytes
/// [`compile`] produced for a key and returns them only for that exact key --
/// it never repairs, merges, or reinterprets a stored value, and never
/// returns a value for a key it was never given (see `tests::` for the
/// invalidation properties this yields for a changed revision, tokenizer,
/// budget, policy, goal, or access scope).
#[derive(Debug, Default)]
pub struct TaskContextCache {
    entries: HashMap<String, String>,
}

impl TaskContextCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, key: &CacheKey) -> Option<&str> {
        self.entries.get(&key.0).map(String::as_str)
    }

    pub fn insert(&mut self, key: CacheKey, value: String) {
        self.entries.insert(key.0, value);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// [`compile`], but through a [`TaskContextCache`]: on a cache hit, returns
/// the previously stored bytes without recompiling and reports `true`; on a
/// miss, compiles fresh, stores the result under this call's [`cache_key`],
/// and reports `false`. The source revision is read from `program` (via
/// [`crate::graph::revision`]) independently inside both this function and
/// `compile` itself; this function does not trust or accept a caller-supplied
/// revision, so a cache hit can only ever occur for the exact source this
/// call actually rechecked.
pub fn compile_cached(
    cache: &mut TaskContextCache,
    program: &Program,
    goal: &CompilationGoal,
    per_seed_options: &AgentContextV2Options,
    budget: CompilationBudget,
    access_scope: &str,
) -> Result<(String, bool), Vec<Diagnostic>> {
    let source_revision = graph::revision(program);
    let key = cache_key(
        &source_revision,
        goal,
        per_seed_options,
        budget,
        access_scope,
    );
    if let Some(cached) = cache.get(&key) {
        return Ok((cached.to_owned(), true));
    }
    let compiled = compile(program, goal, per_seed_options, budget)?;
    cache.insert(key, compiled.clone());
    Ok((compiled, false))
}

/// One deterministic lexical suggestion from [`suggest_seeds`]: a candidate
/// stable id and plain name, and the word-overlap score that ranked it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeedSuggestion {
    id: String,
    name: String,
    score: u32,
}

impl SeedSuggestion {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn score(&self) -> u32 {
        self.score
    }
}

/// Deterministic lexical seed *suggestion* over a module's declaration names
/// and leading doc comments (issue #197 step 2: "deterministic lexical
/// matching over names/docs" as "a seed suggestion, never a replacement for
/// semantic resolution"). `query` is untrusted, caller-supplied text -- like
/// a goal's natural-language description -- and is only ever split into
/// lowercase ASCII-alphanumeric words for a plain overlap count; it is never
/// parsed as a command, never executed, and never mutates `program` or
/// anything else. This function is pure and side-effect-free: nothing in
/// this module calls it, and it never builds, mutates, or feeds a
/// [`CompilationGoal`] on its own. A caller who wants a suggested id to
/// participate in [`compile`] must explicitly wrap it in
/// [`CompilationSeed::new`] and add it to a goal.
///
/// Ranked by descending overlap score, then ascending stable id to break
/// ties, so the result is deterministic for one `(program, comments, query)`
/// triple. A declaration matching no query word is omitted entirely (never
/// reported with a zero score). An empty or all-punctuation `query` yields
/// an empty result rather than matching everything.
#[must_use]
pub fn suggest_seeds(program: &Program, comments: &Comments, query: &str) -> Vec<SeedSuggestion> {
    let query_words = lexical_words(query);
    if query_words.is_empty() {
        return Vec::new();
    }
    let document = doc::document(program, comments);
    let mut suggestions: Vec<SeedSuggestion> = document
        .entries
        .iter()
        .filter_map(|entry| {
            let mut entry_words = lexical_words(&entry.name);
            for line in &entry.description {
                entry_words.extend(lexical_words(line));
            }
            let score = query_words.intersection(&entry_words).count();
            (score > 0).then(|| SeedSuggestion {
                id: entry.id.clone(),
                name: entry.name.clone(),
                score: score as u32,
            })
        })
        .collect();
    suggestions.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    suggestions
}

/// Split `text` into a deduplicated set of lowercase ASCII-alphanumeric
/// words, on any non-alphanumeric ASCII boundary. Deliberately simple and
/// deterministic: this is a suggestion heuristic, never a claim of natural-
/// language understanding.
fn lexical_words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

// --- Requirement/test/candidate-diff facets (issue #197 residual 3). ---

/// Caller-supplied, structurally-keyed facets joined into a compiled
/// closure by exact declaration id -- never by free text. See
/// [`compile_with_declaration_facets`] for why this module accepts this
/// data from the caller rather than discovering it itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationFacets {
    requirements: BTreeMap<String, BTreeSet<String>>,
    tests: BTreeMap<String, BTreeSet<String>>,
    candidate_diff: BTreeSet<String>,
}

impl DeclarationFacets {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that requirement `requirement_id` names `declaration_id` as
    /// one of its assurance subjects. Both are opaque, caller-declared
    /// identifiers this module never interprets beyond exact matching. Their
    /// byte and inventory bounds are enforced by
    /// [`compile_with_declaration_facets`].
    #[must_use]
    pub fn with_requirement(
        mut self,
        declaration_id: impl Into<String>,
        requirement_id: impl Into<String>,
    ) -> Self {
        self.requirements
            .entry(declaration_id.into())
            .or_default()
            .insert(requirement_id.into());
        self
    }

    /// Record that test `test_id` exercises `declaration_id`.
    #[must_use]
    pub fn with_test(
        mut self,
        declaration_id: impl Into<String>,
        test_id: impl Into<String>,
    ) -> Self {
        self.tests
            .entry(declaration_id.into())
            .or_default()
            .insert(test_id.into());
        self
    }

    /// Record that `declaration_id` was changed by the candidate diff under
    /// review.
    #[must_use]
    pub fn with_candidate_diff_change(mut self, declaration_id: impl Into<String>) -> Self {
        self.candidate_diff.insert(declaration_id.into());
        self
    }

    /// The JSON object fields (without the enclosing braces) to attach for
    /// `declaration_id`, or `None` when it carries no requirement, test, or
    /// candidate-diff facet at all -- so a declaration untouched by any of
    /// this data never gets a bare, all-empty entry.
    fn facet_fields_for(&self, declaration_id: &str) -> Option<String> {
        let requirements = self.requirements.get(declaration_id);
        let tests = self.tests.get(declaration_id);
        let in_diff = self.candidate_diff.contains(declaration_id);
        if requirements.is_none() && tests.is_none() && !in_diff {
            return None;
        }
        let requirements_json = requirements
            .map(|ids| {
                ids.iter()
                    .map(|id| quote_json(id))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        let tests_json = tests
            .map(|ids| {
                ids.iter()
                    .map(|id| quote_json(id))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        Some(format!(
            "\"requirements\":[{requirements_json}],\"tests\":[{tests_json}],\
             \"candidate_diff\":{in_diff}"
        ))
    }

    fn render_for(&self, declaration_ids: &[String]) -> String {
        let entries = declaration_ids
            .iter()
            .filter_map(|id| {
                self.facet_fields_for(id)
                    .map(|fields| format!("{{\"id\":{},{fields}}}", quote_json(id)))
            })
            .collect::<Vec<_>>();
        format!("[{}]", entries.join(","))
    }

    fn validate(&self) -> Result<(), Diagnostic> {
        let mut bytes = 0usize;
        let mut entries = 0usize;
        let mut add = |value: &str| -> Result<(), Diagnostic> {
            if value.len() > MAX_FACET_IDENTIFIER_BYTES {
                return Err(option_error(format!(
                    "declaration facet identifier exceeds {MAX_FACET_IDENTIFIER_BYTES} bytes"
                )));
            }
            bytes = bytes.checked_add(value.len()).ok_or_else(|| {
                option_error("declaration facet byte accounting overflowed".to_string())
            })?;
            entries = entries.checked_add(1).ok_or_else(|| {
                option_error("declaration facet entry accounting overflowed".to_string())
            })?;
            if bytes > MAX_DECLARATION_FACET_BYTES || entries > MAX_DECLARATION_FACET_ENTRIES {
                return Err(option_error(format!(
                    "declaration facets exceed {MAX_DECLARATION_FACET_BYTES} bytes or \
                     {MAX_DECLARATION_FACET_ENTRIES} entries"
                )));
            }
            Ok(())
        };
        for (declaration, requirements) in &self.requirements {
            add(declaration)?;
            for requirement in requirements {
                add(requirement)?;
            }
        }
        for (declaration, tests) in &self.tests {
            add(declaration)?;
            for test in tests {
                add(test)?;
            }
        }
        for declaration in &self.candidate_diff {
            add(declaration)?;
        }
        Ok(())
    }
}

// --- Cross-seed deduplication (issue #197 residual 2). ---
//
// Every fact `agent_context_v2_json` renders for one declaration id is a
// pure function of `(program, id, filters, schema)`: it never embeds the
// requesting root's identity, its depth from that root, or which direction
// reached it (those live only in the surrounding `frontier`/`reached_by`
// bookkeeping, never inside a fact's own JSON -- see
// `graph::agent_function_json_for_schema`). That means two different
// seeds sharing `per_seed_options` that both reach the same declaration
// render byte-identical fact text for it
// (`tests::shared_fact_content_is_byte_identical_regardless_of_which_seed_reaches_it`).
// The functions below exploit that: they locate each seed's already-
// compiled `facts` array by scanning the exact compact JSON
// `agent_context_v2_json` returned (never re-serializing it, so no
// re-ordering or re-escaping can silently change what byte-v1 counts as
// "exact"), and replace a fact already fully delivered by an earlier,
// higher-priority *included* seed with a small reference stub instead of
// re-embedding and re-charging it. A seed's own root fact is always kept in
// full inside its own entry, even if some other seed already carries a copy
// of the same content, so a caller reading one seed's entry always gets
// that seed's own requested declaration without having to chase a
// reference elsewhere.

/// The result of deduplicating one seed's already-compiled context JSON
/// against the facts already introduced by earlier, higher-priority
/// *included* seeds in the same [`compile`] call.
struct DedupResult {
    /// The seed's context JSON with any duplicate fact fully replaced by a
    /// small `{"id":...,"deduplicated_owner_seed":...}` stub. Byte-
    /// identical to the original when nothing was deduplicated.
    rewritten_json: String,
    /// Declaration ids this seed's closure reaches for the first time in
    /// this compile (not already in the caller's `seen` set when this seed
    /// was processed). Folded into the shared `seen`/`owner` state by the
    /// caller only if this seed ends up `included`.
    newly_present_ids: Vec<String>,
    /// Every declaration id this seed's closure reaches, in the engine's
    /// original order, regardless of whether it was kept full or stubbed.
    /// Used to attach [`DeclarationFacets`] to exactly the ids a seed's
    /// closure actually reaches.
    all_ids: Vec<String>,
}

fn dedup_seed_json(
    json: &str,
    own_root_id: &str,
    seen: &BTreeSet<String>,
    owner_of: &BTreeMap<String, String>,
) -> DedupResult {
    let Some((_, start, end)) = top_level_object_fields(json)
        .into_iter()
        .find(|(key, _, _)| *key == "facts")
    else {
        return DedupResult {
            rewritten_json: json.to_owned(),
            newly_present_ids: Vec::new(),
            all_ids: Vec::new(),
        };
    };

    let elements = json_array_elements(&json[start..end]);
    let mut rewritten = Vec::with_capacity(elements.len());
    let mut newly_present_ids = Vec::new();
    let mut all_ids = Vec::with_capacity(elements.len());
    for element in elements {
        let Some(id) = fact_id(element) else {
            // Never seen in practice (every fact this module compiles
            // starts with `"id":`), but fail safe rather than drop content
            // silently if the engine's fact shape ever changes underneath.
            rewritten.push(element.to_owned());
            continue;
        };
        let already_seen = seen.contains(&id);
        if id == own_root_id || !already_seen {
            rewritten.push(element.to_owned());
            if !already_seen {
                newly_present_ids.push(id.clone());
            }
        } else {
            let owner = owner_of.get(&id).map(String::as_str).unwrap_or("");
            rewritten.push(format!(
                "{{\"id\":{},\"deduplicated_owner_seed\":{}}}",
                quote_json(&id),
                quote_json(owner)
            ));
        }
        all_ids.push(id);
    }

    let rewritten_facts = format!("[{}]", rewritten.join(","));
    let rewritten_json = format!("{}{}{}", &json[..start], rewritten_facts, &json[end..]);
    DedupResult {
        rewritten_json,
        newly_present_ids,
        all_ids,
    }
}

/// Decode a `"id"` field's value text from a fact element, using this
/// module's own writer's escaping rules ([`quote_json`]) in reverse.
/// Declaration ids are plain identifier-shaped text that `quote_json`
/// never needs to escape in practice, but this decodes properly rather than
/// assuming that.
fn fact_id(fact_text: &str) -> Option<String> {
    let (_, start, end) = top_level_object_fields(fact_text)
        .into_iter()
        .find(|(key, _, _)| *key == "id")?;
    decode_json_string(&fact_text[start..end])
}

fn decode_json_string(literal: &str) -> Option<String> {
    let inner = literal.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                let code = u32::from_str_radix(&hex, 16).ok()?;
                out.push(char::from_u32(code)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// Parse the *direct* (top-level) fields of a compact JSON object, returning
/// each field's key and the exact byte range of its value within `json`
/// (never a re-serialized copy). Assumes `json` is well-formed, compact
/// (no insignificant whitespace) JSON, which is what every writer this
/// module calls into always emits.
fn top_level_object_fields(json: &str) -> Vec<(&str, usize, usize)> {
    let bytes = json.as_bytes();
    let mut fields = Vec::new();
    if bytes.first() != Some(&b'{') {
        return fields;
    }
    let mut i = 1usize;
    loop {
        if i >= bytes.len() || bytes[i] == b'}' {
            break;
        }
        if bytes[i] != b'"' {
            break;
        }
        let key_start = i;
        let key_end = skip_json_string(bytes, i);
        let key = &json[key_start + 1..key_end - 1];
        i = key_end;
        if bytes.get(i) != Some(&b':') {
            break;
        }
        i += 1;
        let value_start = i;
        let value_end = skip_json_value(bytes, i);
        fields.push((key, value_start, value_end));
        i = value_end;
        if bytes.get(i) == Some(&b',') {
            i += 1;
        }
    }
    fields
}

/// Split the inner text of a compact JSON array (including its enclosing
/// `[`/`]`) into its top-level elements, as exact byte-substrings of
/// `array_text` -- never re-serialized.
fn json_array_elements(array_text: &str) -> Vec<&str> {
    let bytes = array_text.as_bytes();
    let mut elements = Vec::new();
    if bytes.first() != Some(&b'[') {
        return elements;
    }
    let mut i = 1usize;
    loop {
        if i >= bytes.len() || bytes[i] == b']' {
            break;
        }
        let start = i;
        let end = skip_json_value(bytes, i);
        elements.push(&array_text[start..end]);
        i = end;
        if bytes.get(i) == Some(&b',') {
            i += 1;
        }
    }
    elements
}

/// Advance past one JSON value starting at `bytes[i]`, returning the index
/// just past its end. Handles strings, objects, and arrays (recursively,
/// respecting nested strings and escapes) and treats any other value
/// (number, `true`, `false`, `null`) as running until the next unescaped
/// structural delimiter.
fn skip_json_value(bytes: &[u8], i: usize) -> usize {
    match bytes.get(i) {
        Some(b'"') => skip_json_string(bytes, i),
        Some(b'{' | b'[') => {
            let mut depth = 1i32;
            let mut j = i + 1;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'"' => j = skip_json_string(bytes, j),
                    b'{' | b'[' => {
                        depth += 1;
                        j += 1;
                    }
                    b'}' | b']' => {
                        depth -= 1;
                        j += 1;
                    }
                    _ => j += 1,
                }
            }
            j
        }
        _ => {
            let mut j = i;
            while j < bytes.len() && !matches!(bytes[j], b',' | b'}' | b']') {
                j += 1;
            }
            j
        }
    }
}

/// Advance past one JSON string literal starting at `bytes[i] == b'"'`,
/// returning the index just past its closing quote.
fn skip_json_string(bytes: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => j += 1,
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests;
