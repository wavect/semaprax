import Base

def insert_choice(value: U32, head: U32, tail: List<&2, U32>, rest: List<&2, U32>, before: Bool) -> List<&2, U32>:
  match before:
    case True{}:
      value <> head <> tail
    case False{}:
      head <> rest

def insert(+value: U32, +input: List<&2, U32>) -> List<&2, U32>:
  match input:
    case Nil{}:
      [value]
    case +head <> tail:
      insert_choice(value, head, tail, insert(value, tail), U32.is_le(value, head))

def sort(input: List<&2, U32>) -> List<&2, U32>:
  match input:
    case Nil{}:
      Nil{}
    case head <> tail:
      Nil{}

