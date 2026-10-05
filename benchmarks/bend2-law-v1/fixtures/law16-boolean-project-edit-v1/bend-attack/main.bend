import Base
import ./core.bend as Core
import ./tests.bend as Tests

def main() -> Bool:
  Bool.and(Core.negate(False{}), Tests.probe())
