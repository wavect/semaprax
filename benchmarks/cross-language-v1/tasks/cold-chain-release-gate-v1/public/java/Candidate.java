// Candidate: release a shipment only when both independent sensor readings
// lie within their inclusive accepted bands. Unchanged between the public
// and hidden phases; the hidden overlay replaces only `Main.java`.
public final class Candidate {
    static long releaseAllowed(long coreTemperature, long sealPressure) {
        return (coreTemperature >= 2 && coreTemperature <= 8 && sealPressure >= 95 && sealPressure <= 105) ? 1 : 0;
    }
}
