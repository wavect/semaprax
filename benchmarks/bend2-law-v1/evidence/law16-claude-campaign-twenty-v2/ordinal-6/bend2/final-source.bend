import Base

def negate(value: Bool) -> Bool:
  Bool.not(value)

law negate_matches_boolean_not:
  for value: Bool
  {negate(value) == Bool.not(value) : Bool}

def negate_matches_boolean_not(value):
  {}
