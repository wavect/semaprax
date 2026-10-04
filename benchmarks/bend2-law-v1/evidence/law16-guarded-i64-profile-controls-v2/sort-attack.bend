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

law case_0:
  {sort([3, 1, 3, 2]) == [1, 2, 3, 3] : List<&2, U32>}

def case_0():
  {==}

law case_1:
  {sort([]) == [] : List<&2, U32>}

def case_1():
  {==}

law case_2:
  {sort([4294967295, 0, 2147483648, 4294967295]) == [0, 2147483648, 4294967295, 4294967295] : List<&2, U32>}

def case_2():
  {==}

law case_3:
  {sort([1, 2]) == [1, 2] : List<&2, U32>}

def case_3():
  {==}

law case_4:
  {sort([4294967295]) == [4294967295] : List<&2, U32>}

def case_4():
  {==}

def main() -> IO(Unit):
  IO.print("full-u32-sort-controls-pass")
