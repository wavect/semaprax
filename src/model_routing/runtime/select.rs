//! Screening and selection at a new invocation boundary. The router names a
//! profile id only; the host maps it back onto its own approved profile.

use std::collections::BTreeSet;

use serde_json::json;

use super::error::RuntimeRoutingError;
use super::features::RuntimeFeatures;
use super::profiles::{ApprovedProfile, ApprovedProfileSet};
use super::record::{RouteRecord, RouteSource};
use crate::model_routing::engine::json;
use crate::model_routing::engine::{
    recommend, screen, ConfiguredProvider, DecisionInvoker, RouteContext, RouteInputs, RouteRequest,
};

/// A selected approved profile and its frozen routing record.
pub struct RoutedProfile<'s> {
    profile: &'s ApprovedProfile,
    record: RouteRecord,
}

impl<'s> RoutedProfile<'s> {
    pub fn profile(&self) -> &'s ApprovedProfile {
        self.profile
    }
    pub fn record(&self) -> &RouteRecord {
        &self.record
    }
    pub fn into_record(self) -> RouteRecord {
        self.record
    }
}

/// Screen the approved catalog and select one profile for a new invocation.
/// With no provider this is the zero-call rules path; with a provider the
/// shared decision core consults it and validates the answer. An operator
/// pin is screened and refuses rather than falling back. No admissible
/// profile fails here, before any generation adapter exists.
pub fn route_new_invocation<'s, I: ?Sized + DecisionInvoker>(
    set: &'s ApprovedProfileSet,
    features: &RuntimeFeatures,
    ctx: &RouteContext,
    provider: Option<&mut ConfiguredProvider<'_, I>>,
) -> Result<RoutedProfile<'s>, RuntimeRoutingError> {
    route_among(set, None, features, ctx, provider)
}

/// [`route_new_invocation`] restricted to an authorized subset of profile ids
/// (MR-10 role/specialist allowlists). A pin outside the subset refuses.
pub(crate) fn route_among<'s, I: ?Sized + DecisionInvoker>(
    set: &'s ApprovedProfileSet,
    allowed: Option<&BTreeSet<String>>,
    features: &RuntimeFeatures,
    ctx: &RouteContext,
    provider: Option<&mut ConfiguredProvider<'_, I>>,
) -> Result<RoutedProfile<'s>, RuntimeRoutingError> {
    features.validate()?;
    let mut excluded: Vec<(String, String)> = Vec::new();
    let mut candidates: Vec<&ApprovedProfile> = Vec::new();
    for profile in set.profiles() {
        if allowed.is_some_and(|a| !a.contains(profile.id())) {
            excluded.push((profile.id().to_owned(), "not authorized here".into()));
        } else if let Some(m) = features
            .required_modalities
            .iter()
            .find(|m| !profile.model().modalities.contains(m))
        {
            excluded.push((
                profile.id().to_owned(),
                format!("missing `{}` modality", m.as_str()),
            ));
        } else {
            candidates.push(profile);
        }
    }
    let base = |profile: &ApprovedProfile| RouteRecord {
        profile_set: set.digest().to_owned(),
        route_policy: set.route_policy().digest(),
        features: features.digest(),
        profile: profile.id().to_owned(),
        deployment: profile.deployment_digest().to_owned(),
        definition: set.definition_digest().to_owned(),
        source: RouteSource::Rules,
        decision_provider: String::new(),
        decision_checkpoint: String::new(),
        decision: String::new(),
        router_calls: 0,
        lineage: ctx.lineage_id.clone(),
        explanation: None,
    };

    if let Some(pin) = &features.operator_pin {
        let profile = set
            .profile(pin)
            .ok_or_else(|| RuntimeRoutingError::UnknownPin(pin.clone()))?;
        if let Some((_, why)) = excluded.iter().find(|(id, _)| id == pin) {
            return Err(RuntimeRoutingError::InadmissiblePin {
                profile: pin.clone(),
                reason: why.clone(),
            });
        }
        let request = RouteRequest::new(
            features.task_features(),
            vec![profile.plan()],
            features.budget(),
        )
        .map_err(|d| RuntimeRoutingError::InvalidConfig(d.message))?;
        let screened = screen(&request, set.route_policy());
        if let Some((_, why)) = screened.excluded.first() {
            return Err(RuntimeRoutingError::InadmissiblePin {
                profile: pin.clone(),
                reason: (*why).to_owned(),
            });
        }
        let mut record = base(profile);
        record.source = RouteSource::Pin;
        record.decision_provider = "operator-pin".into();
        record.decision_checkpoint = "host".into();
        record.decision = json::digest(
            "semaprax.runtime-route-pin.v1",
            &json!({"pin": pin, "profile_set": set.digest(), "features": features.digest()}),
        );
        return Ok(RoutedProfile { profile, record });
    }

    if candidates.is_empty() {
        return Err(RuntimeRoutingError::NoAdmissibleProfile { excluded });
    }
    let request = RouteRequest::new(
        features.task_features(),
        candidates.iter().map(|p| p.plan()).collect(),
        features.budget(),
    )
    .map_err(|d| RuntimeRoutingError::InvalidConfig(d.message))?;
    let inputs = RouteInputs {
        request,
        policy: set.route_policy().clone(),
    };
    let consulted = provider.as_ref().map(|p| p.profile.provider_id.clone());
    let rec = match recommend(&inputs, ctx, provider, None) {
        Ok(rec) => rec,
        Err(d) if d.code == "SPX-HPJ005" => {
            let screened = screen(&inputs.request, &inputs.policy);
            excluded.extend(
                screened
                    .excluded
                    .into_iter()
                    .map(|(id, why)| (id, why.to_owned())),
            );
            excluded.sort();
            return Err(RuntimeRoutingError::NoAdmissibleProfile { excluded });
        }
        Err(d) => {
            return Err(RuntimeRoutingError::Router {
                code: d.code.to_owned(),
                message: d.message,
            })
        }
    };
    let profile = *rec
        .select(&candidates, |p| p.id())
        .ok_or_else(|| RuntimeRoutingError::RecordMismatch("unapproved recommendation".into()))?;
    let decision = rec.decision();
    let (source, reason) = RouteSource::from_decision(decision.source);
    let mut record = base(profile);
    record.source = source;
    record.decision_provider = decision.provider_id.clone();
    record.decision_checkpoint = decision.checkpoint.clone();
    record.decision = decision.plan.decision_digest.clone();
    record.router_calls = decision.router_calls;
    record.explanation = reason.map(|reason| {
        let mut text = format!(
            "decision provider `{}` gave no usable choice ({reason:?}); the permitted rules fallback chose `{}`",
            consulted.as_deref().unwrap_or("none"),
            profile.id()
        );
        if let Some(note) = &decision.wire.note {
            text.push_str("; ");
            text.push_str(note);
        }
        text.chars().take(1024).collect()
    });
    Ok(RoutedProfile { profile, record })
}
