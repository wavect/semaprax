/* Shared explicit A=alpha, Z=é snapshot; no ambient process reads. */
static void fixture_environment(struct spx_environment_snapshot_v1 *environment) {
    memset(environment, 0, sizeof(*environment));
    environment->count = 2;
    environment->entries[0].name = (spx_str_v1){ .data = (const uint8_t *)"A", .len = 1 };
    environment->entries[0].value = (spx_str_v1){ .data = (const uint8_t *)"alpha", .len = 5 };
    environment->entries[1].name = (spx_str_v1){ .data = (const uint8_t *)"Z", .len = 1 };
    environment->entries[1].value = (spx_str_v1){ .data = (const uint8_t *)"\xc3\xa9", .len = 2 };
}
