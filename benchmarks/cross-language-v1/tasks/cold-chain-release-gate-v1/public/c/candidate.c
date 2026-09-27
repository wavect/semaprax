/* Candidate: release a shipment only when both independent sensor readings
 * lie within their inclusive accepted bands. Unchanged between the public
 * and hidden phases; the hidden overlay replaces only `main.c`, which
 * #includes this file.
 */
static long long release_allowed(long long core_temperature, long long seal_pressure) {
    if (core_temperature >= 2 && core_temperature <= 8 && seal_pressure >= 95 && seal_pressure <= 105) {
        return 1;
    }
    return 0;
}
