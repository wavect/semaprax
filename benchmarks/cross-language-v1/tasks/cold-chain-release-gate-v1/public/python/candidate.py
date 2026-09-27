"""Candidate: release a shipment only when both independent sensor readings
lie within their inclusive accepted bands. Unchanged between the public and
hidden phases.
"""


def release_allowed(core_temperature: int, seal_pressure: int) -> int:
    if core_temperature >= 2 and core_temperature <= 8 and seal_pressure >= 95 and seal_pressure <= 105:
        return 1
    return 0
