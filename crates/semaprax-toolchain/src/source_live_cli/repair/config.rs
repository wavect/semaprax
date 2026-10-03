use super::*;

impl RepairConfig {
    pub(super) fn load(path: &Path) -> Result<Self, CliError> {
        if !is_absolute_like(path) {
            return Err(CliError::usage("configuration path must be absolute"));
        }
        let bytes = bounded_read(path, MAX_CONFIG_BYTES)?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| CliError::refused("configuration JSON is malformed"))?;
        let canonical = serde_json::to_vec(&value)
            .map_err(|_| CliError::refused("configuration cannot be encoded"))?;
        if bytes.as_slice() != canonical.as_slice()
            && bytes.strip_suffix(b"\n") != Some(canonical.as_slice())
        {
            return Err(CliError::refused("configuration must be canonical JSON"));
        }
        let map = value
            .as_object()
            .ok_or(CliError::refused("configuration must be an object"))?;
        const V1_KEYS: [&str; 24] = [
            "schema",
            "manifest",
            "source_path",
            "agent_id",
            "step_id",
            "selector_field_id",
            "deployment_migration_id",
            "target",
            "malformed_operation_id",
            "corrected_operation_id",
            "effect_id",
            "argument_id",
            "proposal_field_id",
            "result_id",
            "task_path",
            "task_budget",
            "deadline_millis",
            "ceiling",
            "reservation_units",
            "max_total_steps",
            "effect_budget",
            "malformed_replacement",
            "malformed_bool_literal",
            "turns",
        ];
        const V2_KEYS: [&str; 23] = [
            "schema",
            "manifest",
            "source_path",
            "agent_id",
            "step_id",
            "selector_field_id",
            "deployment_migration_id",
            "target",
            "malformed_operation_id",
            "corrected_operation_id",
            "effect_id",
            "argument_id",
            "proposal_field_id",
            "result_id",
            "task_path",
            "task_budget",
            "deadline_millis",
            "ceiling",
            "reservation_units",
            "max_total_steps",
            "effect_budget",
            "malformed_replacement",
            "malformed_bool_literal",
        ];
        let schema = text(map, "schema")?;
        let v1 = schema == CONFIG_SCHEMA_V1
            && map.len() == V1_KEYS.len()
            && V1_KEYS.iter().all(|key| map.contains_key(*key));
        let v2 = schema == CONFIG_SCHEMA_V2
            && map.len() == V2_KEYS.len()
            && V2_KEYS.iter().all(|key| map.contains_key(*key));
        let v3 = schema == CONFIG_SCHEMA_V3
            && map.len() == V2_KEYS.len()
            && V2_KEYS.iter().all(|key| map.contains_key(*key));
        if !v1 && !v2 && !v3 {
            return Err(CliError::refused(
                "repair configuration has missing or unknown keys",
            ));
        }
        let manifest = absolute(map, "manifest")?;
        let source_path = token(map, "source_path")?;
        if is_absolute_like(Path::new(&source_path))
            || Path::new(&source_path)
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || !source_path.ends_with(".spx")
        {
            return Err(CliError::refused(
                "source_path must be a relative Project .spx path",
            ));
        }
        let target = token(map, "target")?;
        if target.len() > 256 {
            return Err(CliError::refused("target exceeds bounds"));
        }
        let effect_budget = map
            .get("effect_budget")
            .and_then(Value::as_object)
            .ok_or(CliError::refused("effect_budget must be an object"))?;
        const EFFECT_BUDGET_KEYS: [&str; 4] = [
            "max_calls",
            "max_argument_bytes",
            "max_result_bytes",
            "max_total_bytes",
        ];
        if effect_budget.len() != EFFECT_BUDGET_KEYS.len()
            || !EFFECT_BUDGET_KEYS
                .iter()
                .all(|key| effect_budget.contains_key(*key))
        {
            return Err(CliError::refused(
                "effect_budget has missing or unknown keys",
            ));
        }
        // These V1 fixture-only fields remain syntactically validated to keep
        // its frozen configuration key set, but candidate feedback now comes
        // from the live handler's actual preceding effect result.
        let _ = signed_i64(map, "malformed_replacement")?;
        let _ = map
            .get("malformed_bool_literal")
            .and_then(Value::as_bool)
            .ok_or(CliError::refused("malformed_bool_literal must be boolean"))?;
        let provider = if v1 {
            let turns = map
                .get("turns")
                .and_then(Value::as_array)
                .ok_or(CliError::refused("turns must be an array"))?;
            let [first, second] = turns.as_slice() else {
                return Err(CliError::refused("turns must have exactly two entries"));
            };
            let turn = |value: &Value| -> Result<RepairTurn, CliError> {
                let object = value
                    .as_object()
                    .ok_or(CliError::refused("turn must be an object"))?;
                const TURN_KEYS: [&str; 2] = ["document", "requires_prior_feedback"];
                if object.len() != TURN_KEYS.len()
                    || !TURN_KEYS.iter().all(|key| object.contains_key(*key))
                {
                    return Err(CliError::refused("turn has missing or unknown keys"));
                }
                let document = text(object, "document")?.to_owned();
                if document.is_empty() || document.len() > MAX_PROPOSAL_BYTES {
                    return Err(CliError::refused("turn document exceeds bounds"));
                }
                let requires_prior_feedback = object
                    .get("requires_prior_feedback")
                    .and_then(Value::as_bool)
                    .ok_or(CliError::refused("requires_prior_feedback must be boolean"))?;
                Ok(RepairTurn {
                    document,
                    requires_prior_feedback,
                })
            };
            RepairProvider::Scripted([turn(first)?, turn(second)?])
        } else if v3 {
            RepairProvider::Claude
        } else {
            RepairProvider::OpenCode
        };
        Ok(Self {
            manifest,
            source_path,
            agent_id: token(map, "agent_id")?,
            step_id: token(map, "step_id")?,
            selector_field_id: token(map, "selector_field_id")?,
            deployment_migration_id: token(map, "deployment_migration_id")?,
            target,
            malformed_operation_id: token(map, "malformed_operation_id")?,
            corrected_operation_id: token(map, "corrected_operation_id")?,
            effect_id: token(map, "effect_id")?,
            argument_id: token(map, "argument_id")?,
            proposal_field_id: token(map, "proposal_field_id")?,
            result_id: token(map, "result_id")?,
            task_path: absolute(map, "task_path")?,
            task_budget: nonnegative_i64(map, "task_budget")?,
            deadline_millis: positive_i64(map, "deadline_millis")?,
            ceiling: nonnegative_i64(map, "ceiling")?,
            reservation_units: positive_i64(map, "reservation_units")?,
            max_total_steps: positive_usize(map, "max_total_steps")?,
            max_calls: positive_usize(effect_budget, "max_calls")?,
            max_argument_bytes: positive_usize(effect_budget, "max_argument_bytes")?,
            max_result_bytes: positive_usize(effect_budget, "max_result_bytes")?,
            max_total_bytes: positive_usize(effect_budget, "max_total_bytes")?,
            provider,
        })
    }
}
