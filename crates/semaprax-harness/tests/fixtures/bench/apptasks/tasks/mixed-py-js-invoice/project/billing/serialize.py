from .models import Invoice


def to_json(inv: Invoice) -> dict:
    return {"number": inv.number, "totalCents": inv.total_cents}
