from dataclasses import dataclass


@dataclass
class Invoice:
    number: str
    total_cents: int
