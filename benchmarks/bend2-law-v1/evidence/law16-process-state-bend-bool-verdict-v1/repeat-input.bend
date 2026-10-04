# This is an upstream Bend 2 smoke fixture, not a checked-u32 comparison cell.
# It exercises both Bool constructors and proves the matched result agrees with
# Bend's own Bool.to_u32 definition before printing the two observable values.
import Base

def boolean_score(value: Bool) -> U32:
  match value:
    case False{}:
      0
    case True{}:
      1

law boolean_score_matches_base:
  for value: Bool
  {boolean_score(value) == Bool.to_u32(value) : U32}

def boolean_score_matches_base(value):
  match value:
    case False{}:
      {==}
    case True{}:
      {==}

def main() -> IO(Unit):
  do IO<Unit>:
    IO.print(U32.show(boolean_score(False{})))
    IO.print(U32.show(boolean_score(True{})))

#|0
#|1
