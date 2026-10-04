import Base

def negate(value: Bool) -> Bool:
  Bool.not(value)

law negate_matches_boolean_not:
  ∀ value: Bool
  negate(value) == Bool.not(value)