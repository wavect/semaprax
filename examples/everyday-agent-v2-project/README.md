# Everyday Agent V2 Project readiness

This sibling preserves `examples/everyday-agent-project` as the Lifecycle V1
durable-checkpoint product. It supplies an independently authenticated,
multi-module Project whose source-declared Agent has the checked V2 `Step`
shape and uses the exact OpenCode provider/model identity admitted by the
private source-live repair route:

```
opencode/muse-spark-1.3-contributor-free
```

The Project includes the Agent source, a selected target declaration, and a
golden test module. `everyday.v2.repair.target` deliberately returns `0`; the
test independently requires `42`. The repair envelope fixes the candidate
write scope to the target function, so it cannot edit the golden assertion.
Its integration regression authenticates the Project, compiles the linked
Agent lifecycle, checks the v3 Project-associated lifecycle document and the
compiler-derived proposal schema, proves the committed test fails, then proves
the bounded scalar candidate `42` passes the exact manifest test closure.

This is a genuine checked repair subject, not real-provider evidence. #323
still requires an operator-authorized OpenCode run, an interrupt after a
retained settlement, a resumed run with zero duplicate dispatch for that
settled attempt, and the repair-tested host's independently observed passing
candidate check. The route produces an ephemeral candidate only: it does not
modify source or publish a change.

When an authorized operator selects a real failing Project, they must use the
private `source-live repair-tested` route and its absolute config, checkpoint,
OpenCode executable, and empty scratch operands as specified in
[`SOURCE-LIVE-CLI-V1`](../../docs/SOURCE-LIVE-CLI-V1.md). Start from a clean
checkout, then create a canonical V2 config with the selected absolute paths:

```sh
project_root="$(pwd)/examples/everyday-agent-v2-project"
run_root="$(mktemp -d)"
checkpoint="$run_root/checkpoint"
scratch="$run_root/scratch"
mkdir -m 700 "$scratch"
deadline_millis="$(( $(date +%s) * 1000 + 600000 ))"
jq -cnS \
  --arg manifest "$project_root/semaprax.toml" \
  --arg task "$project_root/fixtures/repair-task.txt" \
  --argjson deadline "$deadline_millis" \
  '{agent_id:"everyday.v2.agent",argument_id:"replacement",ceiling:2,
    corrected_operation_id:"everyday.v2.repair.corrected",deadline_millis:$deadline,
    deployment_migration_id:"everyday.v2.repair.opencode.v1",effect_budget:{max_argument_bytes:4096,max_calls:2,max_result_bytes:4096,max_total_bytes:8192},effect_id:"repair",
    malformed_bool_literal:true,malformed_operation_id:"everyday.v2.repair.malformed",malformed_replacement:0,
    manifest:$manifest,max_total_steps:2000000,proposal_field_id:"everyday.v2.agent.type.proposal.budget",
    reservation_units:1,result_id:"feedback",schema:"semaprax.source-live-cli.repair-config.v2",
    selector_field_id:"everyday.v2.agent.type.proposal.sequence",source_path:"src/agent.spx",
    step_id:"everyday.v2.agent.type.step",target:"everyday.v2.repair.target",task_budget:12,task_path:$task}' \
  >"$run_root/repair.json"
semaprax-full source-live repair-tested run "$run_root/repair.json" "$checkpoint" \
  --opencode "$(command -v opencode)" --scratch "$scratch"

# After intentionally interrupting a run whose checkpoint records a settlement:
semaprax-full source-live repair-tested resume "$run_root/repair.json" "$checkpoint" \
  --opencode "$(command -v opencode)" --scratch "$scratch"
```

The later resume uses the same config and checkpoint with `resume` in place of
`run`; it must occur after an intentional interruption that leaves a settled
attempt. The command needs `semaprax-full` on `PATH`, `jq`, and an
absolute OpenCode executable already authenticated and able to use the fixed
free `opencode/muse-spark-1.3-contributor-free` profile. No credential is put
in the config or repository; OpenCode supplies its own configured credential.
