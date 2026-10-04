import Base

def negate(value: Bool) -> Bool:
  match value:
    case False{}:
      True{}
    case True{}:
      False{}

law negate_matches_boolean_not:
  for value: Bool
  {negate(value) == Bool.not(value) : Bool}

def negate_matches_boolean_not(value):
  match value:
    case False{}:
      {==}
    case True{}:
      {==}

def main() -> IO(Unit):
  do IO<Unit>:
    IO.print(Bool.show(negate(False{})))
    IO.print(Bool.show(negate(True{})))
