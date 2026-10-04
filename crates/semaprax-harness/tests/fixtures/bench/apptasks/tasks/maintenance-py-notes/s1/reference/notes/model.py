from dataclasses import dataclass, field


@dataclass
class Note:
    id: int
    title: str
    body: str = ""
    tags: list = field(default_factory=list)
