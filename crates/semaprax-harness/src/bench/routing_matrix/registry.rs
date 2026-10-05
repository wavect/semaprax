//! The approved adapter/profile registry and arm discovery (MR-13).
//!
//! `semaprax.harness-routing-registry.v1` lists approved generation profiles
//! (logical catalog models) and approved decision profiles (an adopted
//! descriptor plus an MR-15 model profile and instance). Arms are derived from
//! these entries and their declared capabilities: nothing here names a vendor
//! or model, so a future provider is a registry entry, not a code change.
//! Availability is decided from declared requirements (environment variable
//! names, hardware tags, a bound live adapter session); an unmet requirement
//! makes the arm `unavailable` with its reason, never a zero-cost success.

use super::{closed, domains, q, text, TaskSet};
use crate::cli::Environment;
use crate::decision::model_profile::{AdapterIdentity, InstanceConfig, ModelProfile};
use crate::decision::route_v2::{ExecutionDomain, Modality};
use crate::diag::HarnessDiagnostic;
use crate::json;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const REGISTRY_SCHEMA: &str = "semaprax.harness-routing-registry.v1";
/// Comma-separated hardware tags the operator declares for this host.
pub const HARDWARE_VAR: &str = "SEMAPRAX_MATRIX_HARDWARE";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Requirements {
    /// Variable names (never values) that must be present.
    pub env: Vec<String>,
    pub hardware: Vec<String>,
}

impl Requirements {
    fn from_json(v: Option<&Value>, what: &str) -> Result<Self, HarnessDiagnostic> {
        let Some(v) = v else {
            return Ok(Self::default());
        };
        let m = closed(v, what, &[], &["env", "hardware"])?;
        let list = |k: &str| -> Vec<String> {
            m.get(k)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        };
        Ok(Self {
            env: list("env"),
            hardware: list("hardware"),
        })
    }

    /// `Some(reason)` naming every unmet requirement on this host.
    pub fn unmet(&self, env: &Environment) -> Option<String> {
        let mut why = Vec::new();
        let missing: Vec<&str> = self
            .env
            .iter()
            .filter(|k| env.vars.get(*k).is_none_or(|v| v.is_empty()))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            why.push(format!(
                "missing credential/endpoint variable(s): {}",
                missing.join(", ")
            ));
        }
        let have: BTreeSet<&str> = env
            .vars
            .get(HARDWARE_VAR)
            .map(|h| h.split(',').map(str::trim).collect())
            .unwrap_or_default();
        let hw: Vec<&str> = self
            .hardware
            .iter()
            .filter(|h| !have.contains(h.as_str()))
            .map(String::as_str)
            .collect();
        if !hw.is_empty() {
            why.push(format!(
                "required hardware not declared on this host: {}",
                hw.join(", ")
            ));
        }
        (!why.is_empty()).then(|| why.join("; "))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationProfile {
    /// The logical catalog model id this profile backs.
    pub model: String,
    pub domains: BTreeSet<ExecutionDomain>,
    pub requires: Requirements,
}

/// Router price as host configuration (an adapter's own `billing` claim never
/// zeroes a priced router).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouterPrice {
    NonBilled,
    Priced {
        /// Micro-units per million tokens.
        input: u64,
        output: u64,
        /// Reservation charged per call whose usage is not provider-reported.
        max_call_micros: u64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct LearnedProfile {
    pub arm_id: String,
    pub descriptor: PathBuf,
    pub descriptor_digest: String,
    pub adapter: AdapterIdentity,
    /// `decision.evaluate` versions the descriptor declares.
    pub versions: Vec<u32>,
    pub model_profile: ModelProfile,
    pub instance: InstanceConfig,
    pub domains: BTreeSet<ExecutionDomain>,
    pub requires: Requirements,
    pub router_price: RouterPrice,
    /// Item ids (or content digests) the profile was trained or calibrated on.
    pub trained_on: BTreeSet<String>,
}

/// A real-run executor declared by the operator: an argv speaking
/// `semaprax.harness-routing-cell.v1` on stdin/stdout.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutorDecl {
    pub argv: Vec<String>,
    pub identity: String,
    pub requires: Requirements,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Registry {
    pub digest: String,
    pub generation: Vec<GenerationProfile>,
    pub learned: Vec<LearnedProfile>,
    pub executor: Option<ExecutorDecl>,
    /// Decision entries that are not arms, with why (bad capability set).
    pub excluded: Vec<Value>,
}

fn read_json(p: &Path) -> Result<Value, HarnessDiagnostic> {
    let b = std::fs::read(p).map_err(|e| q("SPX-HPQ001", format!("{}: {e}", p.display())))?;
    serde_json::from_slice(&b).map_err(|e| q("SPX-HPQ001", format!("{}: {e}", p.display())))
}

impl Registry {
    /// Parse a registry; descriptor paths resolve against `base`.
    pub fn from_json(v: &Value, base: &Path) -> Result<Self, HarnessDiagnostic> {
        let m = closed(
            v,
            "routing registry",
            &["schema", "generation_profiles", "decision_profiles"],
            &["executor", "description"],
        )?;
        if m["schema"] != REGISTRY_SCHEMA {
            return Err(q(
                "SPX-HPQ002",
                format!("registry schema must be `{REGISTRY_SCHEMA}`"),
            ));
        }
        let mut generation = Vec::new();
        for g in m["generation_profiles"].as_array().into_iter().flatten() {
            let gm = closed(
                g,
                "generation profile",
                &["model", "domains"],
                &["requires"],
            )?;
            generation.push(GenerationProfile {
                model: text(gm, "model", "generation profile")?,
                domains: domains(gm.get("domains"), "generation profile")?,
                requires: Requirements::from_json(gm.get("requires"), "requires")?,
            });
        }
        let (mut learned, mut excluded) = (Vec::new(), Vec::new());
        let mut seen = BTreeSet::new();
        for d in m["decision_profiles"].as_array().into_iter().flatten() {
            let dm = closed(
                d,
                "decision profile",
                &[
                    "descriptor",
                    "model_profile",
                    "instance",
                    "domains",
                    "router_price",
                ],
                &["requires", "trained_on"],
            )?;
            let rel = text(dm, "descriptor", "decision profile")?;
            let path = base.join(&rel);
            let desc = read_json(&path)?;
            let provider_id = desc["provider"]["id"].as_str().unwrap_or("").to_string();
            let adapter_version = desc["adapter"]["version"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let versions: Vec<u32> = desc["capabilities"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["kind"] == "decision.evaluate")
                .filter_map(|c| c["version"].as_u64().map(|x| x as u32))
                .collect();
            let model_profile = ModelProfile::from_json(&dm["model_profile"])
                .map_err(|e| q("SPX-HPQ002", format!("{rel}: {}", e.message)))?;
            let reject =
                |why: &str| json!({"descriptor": rel, "provider": provider_id, "reason": why});
            if provider_id.is_empty() || adapter_version.is_empty() {
                excluded.push(reject("descriptor lacks provider.id or adapter.version"));
                continue;
            }
            if versions.is_empty() {
                excluded.push(reject(
                    "descriptor declares no decision.evaluate capability",
                ));
                continue;
            }
            if !model_profile.modalities.contains(&Modality::Text) {
                excluded.push(reject("model profile does not accept text routing state"));
                continue;
            }
            let im = closed(&dm["instance"], "instance", &["instance_id"], &["endpoint"])?;
            let instance = InstanceConfig {
                instance_id: text(im, "instance_id", "instance")?,
                endpoint: im
                    .get("endpoint")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                secret_refs: desc["permissions"]["secrets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect(),
            };
            let mut requires = Requirements::from_json(dm.get("requires"), "requires")?;
            for s in &instance.secret_refs {
                if !requires.env.contains(s) {
                    requires.env.push(s.clone());
                }
            }
            let router_price = match &dm["router_price"] {
                Value::String(s) if s == "non_billed" => RouterPrice::NonBilled,
                p => {
                    let pm = closed(
                        p,
                        "router_price",
                        &["input", "output", "max_call_micros"],
                        &[],
                    )?;
                    let n = |k: &str| {
                        pm[k].as_u64().ok_or_else(|| {
                            q(
                                "SPX-HPQ002",
                                format!("router_price `{k}` must be an integer"),
                            )
                        })
                    };
                    RouterPrice::Priced {
                        input: n("input")?,
                        output: n("output")?,
                        max_call_micros: n("max_call_micros")?.max(1),
                    }
                }
            };
            let arm_id = format!("learned:{provider_id}/{}", model_profile.profile_id);
            if !seen.insert(arm_id.clone()) {
                return Err(q(
                    "SPX-HPQ005",
                    format!("duplicate decision profile `{arm_id}`"),
                ));
            }
            learned.push(LearnedProfile {
                arm_id,
                descriptor: path,
                descriptor_digest: json::digest("semaprax.harness-provider-descriptor.v1", &desc),
                adapter: AdapterIdentity {
                    provider_id,
                    adapter_version,
                },
                versions,
                model_profile,
                instance,
                domains: domains(dm.get("domains"), "decision profile")?,
                requires,
                router_price,
                trained_on: dm
                    .get("trained_on")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect(),
            });
        }
        let executor = match m.get("executor") {
            None | Some(Value::Null) => None,
            Some(e) => {
                let em = closed(e, "executor", &["argv", "identity"], &["requires"])?;
                let argv: Vec<String> = em["argv"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect();
                if argv.is_empty() {
                    return Err(q("SPX-HPQ002", "executor argv must be non-empty"));
                }
                Some(ExecutorDecl {
                    argv,
                    identity: text(em, "identity", "executor")?,
                    requires: Requirements::from_json(em.get("requires"), "requires")?,
                })
            }
        };
        Ok(Self {
            digest: json::digest(REGISTRY_SCHEMA, v),
            generation,
            learned,
            executor,
            excluded,
        })
    }

    pub fn load(path: &Path, base: &Path) -> Result<Self, HarnessDiagnostic> {
        Self::from_json(&read_json(path)?, base)
    }

    pub fn generation_for(&self, model: &str) -> Option<&GenerationProfile> {
        self.generation.iter().find(|g| g.model == model)
    }

    /// Every variable name a requirement mentions (for deliberate forwarding).
    pub fn required_vars(&self) -> BTreeSet<String> {
        let mut s: BTreeSet<String> = self
            .generation
            .iter()
            .flat_map(|g| g.requires.env.clone())
            .chain(self.learned.iter().flat_map(|l| l.requires.env.clone()))
            .chain(self.executor.iter().flat_map(|e| e.requires.env.clone()))
            .collect();
        s.insert(HARDWARE_VAR.into());
        s
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArmKind {
    /// The builtin policy rules.
    Rules,
    /// One generation profile for every item.
    Fixed(String),
    /// TC-10 total-cost routing over calibration evidence.
    CostAware,
    /// Index into `Registry::learned`.
    Learned(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arm {
    pub id: String,
    pub kind: ArmKind,
    pub domains: BTreeSet<ExecutionDomain>,
    /// `Some(reason)`: every cell of this arm is `unavailable`.
    pub unavailable: Option<String>,
}

impl Arm {
    pub fn to_json(&self) -> Value {
        let kind = match &self.kind {
            ArmKind::Rules => "rules",
            ArmKind::Fixed(_) => "fixed",
            ArmKind::CostAware => "cost_aware",
            ArmKind::Learned(_) => "learned",
        };
        json!({"id": self.id, "kind": kind,
               "domains": self.domains.iter().map(|d| d.as_str()).collect::<Vec<_>>(),
               "status": if self.unavailable.is_some() { "unavailable" } else { "available" },
               "reason": self.unavailable})
    }
}

/// Discover the arms: rules, one fixed arm per approved generation profile in
/// the task catalog, the cost-aware arm, and one arm per approved decision
/// profile. `bound` names learned arms with a live adapter session. With a
/// fixture executor (`fixture_lane`) the fixture table stands in for model
/// access, so generation requirements are not consulted; every such cell is
/// fixture-origin and can never qualify.
pub fn discover(
    reg: &Registry,
    tasks: &TaskSet,
    env: &Environment,
    bound: &BTreeSet<String>,
    fixture_lane: bool,
) -> Vec<Arm> {
    let all: BTreeSet<ExecutionDomain> = tasks.items.iter().map(|i| i.domain).collect();
    let mut arms = vec![
        Arm {
            id: "rules".into(),
            kind: ArmKind::Rules,
            domains: all.clone(),
            unavailable: None,
        },
        Arm {
            id: "cost-aware".into(),
            kind: ArmKind::CostAware,
            domains: all.clone(),
            unavailable: None,
        },
    ];
    let catalog: BTreeSet<&str> = tasks
        .catalog
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["id"].as_str())
        .collect();
    let mut fixed: BTreeMap<String, Arm> = BTreeMap::new();
    for model in &catalog {
        let (domains, unavailable) = match reg.generation_for(model) {
            None => (
                all.clone(),
                Some("no approved generation profile backs this catalog model".to_string()),
            ),
            Some(g) => (
                g.domains.intersection(&all).copied().collect(),
                if fixture_lane {
                    None
                } else {
                    g.requires.unmet(env)
                },
            ),
        };
        fixed.insert(
            model.to_string(),
            Arm {
                id: format!("fixed:{model}"),
                kind: ArmKind::Fixed(model.to_string()),
                domains,
                unavailable,
            },
        );
    }
    arms.extend(fixed.into_values());
    for (i, l) in reg.learned.iter().enumerate() {
        let unavailable = l.requires.unmet(env).or_else(|| {
            (!bound.contains(&l.arm_id))
                .then(|| "no live adapter session is bound for this run".to_string())
        });
        arms.push(Arm {
            id: l.arm_id.clone(),
            kind: ArmKind::Learned(i),
            domains: l.domains.intersection(&all).copied().collect(),
            unavailable,
        });
    }
    arms
}
