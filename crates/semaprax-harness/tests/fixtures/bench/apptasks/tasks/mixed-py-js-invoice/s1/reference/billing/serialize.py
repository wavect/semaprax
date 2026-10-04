from .models import Invoice


def to_json(inv: Invoice) -> dict:
    out = {"number": inv.number, "totalCents": inv.total_cents}
    if inv.due_date is not None:
        out["dueDate"] = inv.due_date
    return out
