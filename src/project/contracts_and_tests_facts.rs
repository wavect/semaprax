//! Canonical contract and declared-test facts derived from admitted Project HIR.
//!
//! This standalone semantic object is descriptive only. It is not a ProgramRoot
//! segment, does not imply coverage or proof, and grants no source or execution
//! authority.

use std::sync::Arc;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedFunction, ResolvedFunctionTemplate};

use super::ProjectRevision;

pub const CONTRACTS_AND_TESTS_FACTS_SCHEMA: &str = "semaprax.contracts-and-tests-facts.v1";
pub const MAX_DECLARED_CONTRACT_FUNCTIONS: usize = 4096;
pub const MAX_DECLARED_CONTRACT_CLAUSES: usize = 16_384;
pub const MAX_DECLARED_TESTS: usize = 4096;
pub const MAX_CONTRACT_SOURCE_FACT_BYTES: usize = 256 * 1024;
pub const MAX_CONTRACTS_AND_TESTS_FACTS_BYTES: usize = 8 * 1024 * 1024;

const DIGEST_DOMAIN: &[u8] = b"semaprax.contracts-and-tests-facts.digest.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractSourceFact {
    phase: &'static str,
    index: usize,
    expression_id: String,
    type_id: String,
    source_fact: String,
}

impl ContractSourceFact {
    pub fn phase(&self) -> &str {
        self.phase
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn expression_id(&self) -> &str {
        &self.expression_id
    }
    pub fn type_id(&self) -> &str {
        &self.type_id
    }
    pub fn source_fact(&self) -> &str {
        &self.source_fact
    }

    fn value(&self) -> Result<Value, Vec<Diagnostic>> {
        let source_fact: Value = serde_json::from_str(&self.source_fact)
            .map_err(|_| invalid("compiler-derived contract source fact is not JSON"))?;
        Ok(json!({
            "expression_id": self.expression_id,
            "index": self.index,
            "phase": self.phase,
            "source_fact": source_fact,
            "type_id": self.type_id,
        }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredFunctionContractFacts {
    stable_id: String,
    module: String,
    declaration_kind: &'static str,
    requires: Vec<ContractSourceFact>,
    ensures: Vec<ContractSourceFact>,
}

impl DeclaredFunctionContractFacts {
    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }
    pub fn module(&self) -> &str {
        &self.module
    }
    pub fn declaration_kind(&self) -> &str {
        self.declaration_kind
    }
    pub fn requires(&self) -> &[ContractSourceFact] {
        &self.requires
    }
    pub fn ensures(&self) -> &[ContractSourceFact] {
        &self.ensures
    }

    fn value(&self) -> Result<Value, Vec<Diagnostic>> {
        Ok(json!({
            "declaration_kind": self.declaration_kind,
            "ensures": self.ensures.iter().map(ContractSourceFact::value).collect::<Result<Vec<_>, _>>()?,
            "module": self.module,
            "requires": self.requires.iter().map(ContractSourceFact::value).collect::<Result<Vec<_>, _>>()?,
            "stable_id": self.stable_id,
        }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredTestFact {
    stable_id: String,
    name: String,
    kind: &'static str,
}

impl DeclaredTestFact {
    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> &str {
        self.kind
    }

    fn value(&self) -> Value {
        json!({"kind": self.kind, "name": self.name, "stable_id": self.stable_id})
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractsAndTestsFacts {
    project_revision: String,
    functions: Vec<DeclaredFunctionContractFacts>,
    tests: Vec<DeclaredTestFact>,
    json: String,
    facts_digest: String,
}

impl ContractsAndTestsFacts {
    pub fn derive(
        revision: Arc<ProjectRevision>,
        expected_project_revision: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_digest(expected_project_revision)?;
        if expected_project_revision != revision.project_revision() {
            return Err(stale(
                "contracts/tests facts expected Project revision is stale",
            ));
        }

        let mut functions = Vec::new();
        let mut tests = Vec::new();
        let mut clause_count = 0usize;
        for module in revision.semantic.image_modules() {
            for function in module.functions() {
                clause_count = clause_count
                    .checked_add(function.requires.len() + function.ensures.len())
                    .ok_or_else(|| invalid("contract clause count overflowed"))?;
                functions.push(function_facts(module.module(), function)?);
                if module.module() == revision.manifest().test_module() {
                    let executable = revision
                        .test_program()
                        .functions
                        .iter()
                        .find(|candidate| candidate.id == function.id)
                        .is_some_and(|candidate| {
                            candidate.params.is_empty()
                                && candidate.return_type == crate::hir::ResolvedType::I64
                                && revision
                                    .test_program()
                                    .declarations
                                    .declaration(&candidate.id)
                                    .is_some_and(|declaration| {
                                        declaration.identity_origin
                                            == crate::hir::IdentityOrigin::Explicit
                                    })
                        });
                    let kind = if function.name == "main" && executable {
                        Some("test_main")
                    } else if function.name.starts_with("test_") && executable {
                        Some("named_test")
                    } else {
                        None
                    };
                    if let Some(kind) = kind {
                        tests.push(DeclaredTestFact {
                            stable_id: function.id.as_str().to_owned(),
                            name: function.name.clone(),
                            kind,
                        });
                    }
                }
            }
            for template in module.function_templates() {
                clause_count = clause_count
                    .checked_add(template.requires.len() + template.ensures.len())
                    .ok_or_else(|| invalid("contract clause count overflowed"))?;
                functions.push(template_facts(module.module(), template)?);
            }
        }
        if functions.len() > MAX_DECLARED_CONTRACT_FUNCTIONS
            || clause_count > MAX_DECLARED_CONTRACT_CLAUSES
        {
            return Err(invalid("declared contract inventory exceeds its bound"));
        }
        if tests.len() > MAX_DECLARED_TESTS {
            return Err(invalid("declared test inventory exceeds its bound"));
        }
        functions.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
        tests.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
        if functions
            .windows(2)
            .any(|pair| pair[0].stable_id == pair[1].stable_id)
            || tests
                .windows(2)
                .any(|pair| pair[0].stable_id == pair[1].stable_id)
        {
            return Err(invalid(
                "contract or test inventory contains a duplicate stable identity",
            ));
        }

        let value = json!({
            "coverage_claimed": false,
            "execution_claimed": false,
            "functions": functions.iter().map(DeclaredFunctionContractFacts::value).collect::<Result<Vec<_>, _>>()?,
            "limits": {
                "max_bundle_bytes": MAX_CONTRACTS_AND_TESTS_FACTS_BYTES,
                "max_clause_source_fact_bytes": MAX_CONTRACT_SOURCE_FACT_BYTES,
                "max_contract_clauses": MAX_DECLARED_CONTRACT_CLAUSES,
                "max_contract_functions": MAX_DECLARED_CONTRACT_FUNCTIONS,
                "max_declared_tests": MAX_DECLARED_TESTS,
            },
            "nonclaims": [
                "not_a_program_root_segment",
                "no_contract_proof_or_coverage_claim",
                "no_test_execution_or_result_claim",
                "no_source_execution_or_publication_authority",
            ],
            "project_graph_digest": revision.semantic_graph_digest(),
            "project_revision": revision.project_revision(),
            "schema": CONTRACTS_AND_TESTS_FACTS_SCHEMA,
            "source_authority": false,
            "tests": tests.iter().map(DeclaredTestFact::value).collect::<Vec<_>>(),
            "workspace_revision": revision.workspace_revision(),
        });
        let json = canonical_json(value)?;
        if json.len() > MAX_CONTRACTS_AND_TESTS_FACTS_BYTES {
            return Err(invalid(
                "canonical contracts/tests facts exceed their byte limit",
            ));
        }
        let facts_digest = framed_digest(DIGEST_DOMAIN, json.as_bytes());
        Ok(Self {
            project_revision: revision.project_revision().to_owned(),
            functions,
            tests,
            json,
            facts_digest,
        })
    }

    pub fn replay(
        revision: Arc<ProjectRevision>,
        expected_project_revision: &str,
        expected_facts_digest: &str,
        bytes: &[u8],
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_digest(expected_facts_digest)?;
        if bytes.len() > MAX_CONTRACTS_AND_TESTS_FACTS_BYTES {
            return Err(invalid(
                "submitted contracts/tests facts exceed their byte limit",
            ));
        }
        let source = std::str::from_utf8(bytes)
            .map_err(|_| invalid("submitted contracts/tests facts are not UTF-8"))?;
        let value: Value = serde_json::from_str(source)
            .map_err(|_| invalid("submitted contracts/tests facts are not JSON"))?;
        if canonical_json(value.clone())?.as_bytes() != bytes {
            return Err(invalid(
                "submitted contracts/tests facts are not canonical JSON",
            ));
        }
        validate_wire_shape(&value)?;
        let derived = Self::derive(revision, expected_project_revision)?;
        if expected_facts_digest != derived.facts_digest || bytes != derived.json.as_bytes() {
            return Err(stale(
                "contracts/tests facts differ from exact Project replay",
            ));
        }
        Ok(derived)
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }
    pub fn functions(&self) -> &[DeclaredFunctionContractFacts] {
        &self.functions
    }
    pub fn tests(&self) -> &[DeclaredTestFact] {
        &self.tests
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn facts_digest(&self) -> &str {
        &self.facts_digest
    }
}

fn function_facts(
    module: &str,
    function: &ResolvedFunction,
) -> Result<DeclaredFunctionContractFacts, Vec<Diagnostic>> {
    clauses(
        module,
        function.id.as_str(),
        "function",
        &function.requires,
        &function.ensures,
    )
}

fn template_facts(
    module: &str,
    function: &ResolvedFunctionTemplate,
) -> Result<DeclaredFunctionContractFacts, Vec<Diagnostic>> {
    clauses(
        module,
        function.id.as_str(),
        "function_template",
        &function.requires,
        &function.ensures,
    )
}

fn clauses(
    module: &str,
    stable_id: &str,
    declaration_kind: &'static str,
    requires: &[ResolvedExpr],
    ensures: &[ResolvedExpr],
) -> Result<DeclaredFunctionContractFacts, Vec<Diagnostic>> {
    Ok(DeclaredFunctionContractFacts {
        stable_id: stable_id.to_owned(),
        module: module.to_owned(),
        declaration_kind,
        requires: render_clauses("requires", requires)?,
        ensures: render_clauses("ensures", ensures)?,
    })
}

fn render_clauses(
    phase: &'static str,
    expressions: &[ResolvedExpr],
) -> Result<Vec<ContractSourceFact>, Vec<Diagnostic>> {
    expressions
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            let source_fact =
                crate::graph::agent_contract_expr_json(expression).map_err(|error| vec![error])?;
            if source_fact.len() > MAX_CONTRACT_SOURCE_FACT_BYTES {
                return Err(invalid(
                    "compiler-derived contract source fact exceeds its byte bound",
                ));
            }
            Ok(ContractSourceFact {
                phase,
                index,
                expression_id: expression.id.as_str().to_owned(),
                type_id: expression.ty.identity_key(),
                source_fact,
            })
        })
        .collect()
}

fn validate_wire_shape(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("contracts/tests facts must be an object"))?;
    require_keys(
        object,
        &[
            "coverage_claimed",
            "execution_claimed",
            "functions",
            "limits",
            "nonclaims",
            "project_graph_digest",
            "project_revision",
            "schema",
            "source_authority",
            "tests",
            "workspace_revision",
        ],
    )?;
    let limits = value["limits"]
        .as_object()
        .ok_or_else(|| invalid("contracts/tests facts limits must be an object"))?;
    require_keys(
        limits,
        &[
            "max_bundle_bytes",
            "max_clause_source_fact_bytes",
            "max_contract_clauses",
            "max_contract_functions",
            "max_declared_tests",
        ],
    )?;
    let exact_limits = json!({
        "max_bundle_bytes": MAX_CONTRACTS_AND_TESTS_FACTS_BYTES,
        "max_clause_source_fact_bytes": MAX_CONTRACT_SOURCE_FACT_BYTES,
        "max_contract_clauses": MAX_DECLARED_CONTRACT_CLAUSES,
        "max_contract_functions": MAX_DECLARED_CONTRACT_FUNCTIONS,
        "max_declared_tests": MAX_DECLARED_TESTS,
    });
    let exact_nonclaims = json!([
        "not_a_program_root_segment",
        "no_contract_proof_or_coverage_claim",
        "no_test_execution_or_result_claim",
        "no_source_execution_or_publication_authority",
    ]);
    if value["schema"] != CONTRACTS_AND_TESTS_FACTS_SCHEMA
        || value["coverage_claimed"] != false
        || value["execution_claimed"] != false
        || value["source_authority"] != false
        || value["limits"] != exact_limits
        || value["nonclaims"] != exact_nonclaims
        || value["functions"]
            .as_array()
            .is_none_or(|items| items.len() > MAX_DECLARED_CONTRACT_FUNCTIONS)
        || value["tests"]
            .as_array()
            .is_none_or(|items| items.len() > MAX_DECLARED_TESTS)
    {
        return Err(invalid(
            "contracts/tests facts have an invalid schema, claim, or inventory",
        ));
    }
    for key in [
        "project_graph_digest",
        "project_revision",
        "workspace_revision",
    ] {
        validate_digest(
            value[key]
                .as_str()
                .ok_or_else(|| invalid("contracts/tests facts digest field must be text"))?,
        )?;
    }
    let functions = value["functions"].as_array().expect("checked above");
    let mut previous = None;
    let mut clauses = 0usize;
    for function in functions {
        let function = function
            .as_object()
            .ok_or_else(|| invalid("contract function fact must be an object"))?;
        require_keys(
            function,
            &[
                "declaration_kind",
                "ensures",
                "module",
                "requires",
                "stable_id",
            ],
        )?;
        let stable_id = bounded_text(function.get("stable_id"), "function stable identity")?;
        bounded_text(function.get("module"), "function module")?;
        if !matches!(
            function["declaration_kind"].as_str(),
            Some("function" | "function_template")
        ) || previous.is_some_and(|prior: &str| prior.as_bytes() >= stable_id.as_bytes())
        {
            return Err(invalid(
                "contract functions must have exact kinds and unique stable-ID order",
            ));
        }
        previous = Some(stable_id);
        for (phase, facts) in [
            ("requires", &function["requires"]),
            ("ensures", &function["ensures"]),
        ] {
            let facts = facts
                .as_array()
                .ok_or_else(|| invalid("contract clause inventory must be an array"))?;
            clauses = clauses
                .checked_add(facts.len())
                .ok_or_else(|| invalid("contract clause count overflowed"))?;
            if clauses > MAX_DECLARED_CONTRACT_CLAUSES {
                return Err(invalid("contract clause inventory exceeds its bound"));
            }
            for (index, fact) in facts.iter().enumerate() {
                validate_clause_fact(fact, phase, index)?;
            }
        }
    }
    let tests = value["tests"].as_array().expect("checked above");
    let mut previous = None;
    for test in tests {
        let test = test
            .as_object()
            .ok_or_else(|| invalid("declared test fact must be an object"))?;
        require_keys(test, &["kind", "name", "stable_id"])?;
        let stable_id = bounded_text(test.get("stable_id"), "test stable identity")?;
        bounded_text(test.get("name"), "test name")?;
        if !matches!(test["kind"].as_str(), Some("named_test" | "test_main"))
            || previous.is_some_and(|prior: &str| prior.as_bytes() >= stable_id.as_bytes())
        {
            return Err(invalid(
                "declared tests must have exact kinds and unique stable-ID order",
            ));
        }
        previous = Some(stable_id);
    }
    Ok(())
}

fn validate_clause_fact(value: &Value, phase: &str, index: usize) -> Result<(), Vec<Diagnostic>> {
    let fact = value
        .as_object()
        .ok_or_else(|| invalid("contract clause fact must be an object"))?;
    require_keys(
        fact,
        &["expression_id", "index", "phase", "source_fact", "type_id"],
    )?;
    bounded_text(fact.get("expression_id"), "contract expression identity")?;
    bounded_text(fact.get("type_id"), "contract expression type identity")?;
    let source_fact = fact
        .get("source_fact")
        .ok_or_else(|| invalid("contract clause source fact is missing"))?;
    let source_bytes = serde_json::to_vec(source_fact)
        .map_err(|_| invalid("contract clause source fact cannot be serialized"))?;
    if source_bytes.len() > MAX_CONTRACT_SOURCE_FACT_BYTES
        || !source_fact.is_object()
        || fact["phase"] != phase
        || fact["index"].as_u64() != Some(index as u64)
    {
        return Err(invalid(
            "contract clause fact has invalid phase, index, source, or bound",
        ));
    }
    Ok(())
}

fn bounded_text<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, Vec<Diagnostic>> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty() && text.len() <= crate::project::MAX_STABLE_ID_BYTES)
        .ok_or_else(|| {
            invalid(format!(
                "contracts/tests facts {field} is invalid or over-bound"
            ))
        })
}

fn require_keys(object: &Map<String, Value>, expected: &[&str]) -> Result<(), Vec<Diagnostic>> {
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(invalid(
            "contracts/tests facts contain missing or unknown fields",
        ));
    }
    Ok(())
}

fn canonical_json(mut value: Value) -> Result<String, Vec<Diagnostic>> {
    sort_json(&mut value);
    let mut rendered = serde_json::to_string(&value)
        .map_err(|_| invalid("contracts/tests facts could not be serialized"))?;
    rendered.push('\n');
    Ok(rendered)
}

fn sort_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let old = std::mem::take(object);
            let mut entries = old.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
            for (key, mut child) in entries {
                sort_json(&mut child);
                object.insert(key, child);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json),
        _ => {}
    }
}

fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "contracts/tests facts require a lowercase sha256 digest",
        ));
    }
    Ok(())
}

fn framed_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G574", message)]
}
fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G575", message)]
}
