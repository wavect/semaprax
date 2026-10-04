# Mutant control: score True as False while retaining the original law proof.
# Both ordinary Bend checking and --verdict must reject its false equality step.
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

#|SOME PROOFS FAIL
