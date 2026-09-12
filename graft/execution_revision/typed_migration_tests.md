# execution_revision/typed_migration_tests.rs

- SOURCE · constant · L4-L17 — const SOURCE: &str = r#"module migration.copy;
- state · function · L18-L26 — fn state(value: RetainedValue) -> RetainedValue
- migration_replays_exact_old_and_new_nominal_state_without_effects · function · L28-L102 — fn migration_replays_exact_old_and_new_nominal_state_without_effects()
- migration_rejects_borrowed_owned_old_state_parameter · function · L105-L135 — fn migration_rejects_borrowed_owned_old_state_parameter()
