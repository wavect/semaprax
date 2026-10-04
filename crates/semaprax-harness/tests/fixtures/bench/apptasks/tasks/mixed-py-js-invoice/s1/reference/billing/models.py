from dataclasses import dataclass
from typing import Optional


@dataclass
class Invoice:
    number: str
    total_cents: int
    due_date: Optional[str] = None
