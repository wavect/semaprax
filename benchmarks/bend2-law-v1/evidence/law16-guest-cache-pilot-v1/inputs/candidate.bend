import Base

def negate(value: Bool) -> Bool:
  Bool.not(value)

law negate_matches_boolean_not:
  for value: Bool
  {negate(value) == Bool.not(value) : Bool}

def negate_matches_boolean_not(value):
  {==}

def main() -> IO(Unit):
  do IO<Unit>:
    IO.print(Bool.show(negate(False{})))
    IO.print(Bool.show(negate(True{})))
