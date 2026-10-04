import Base

type Balances is Data:
  Balances{debit: U32, credit: U32}

type Decision is Data:
  Decision{debit: U32, credit: U32, code: U32}

def transfer_success(debit: U32, credit: U32, +amount: U32) -> Decision:
  Decision{U32.sub(debit, amount), U32.add(credit, amount), 0}

def transfer_credit(debit: U32, credit: U32, amount: U32, overflow: Bool) -> Decision:
  match overflow:
    case True{}:
      Decision{debit, credit, 2}
    case False{}:
      transfer_success(debit, credit, amount)

def transfer_funds(debit: U32, +credit: U32, +amount: U32, insufficient: Bool) -> Decision:
  match insufficient:
    case True{}:
      Decision{debit, credit, 1}
    case False{}:
      transfer_credit(debit, credit, amount, U32.is_gt(credit, U32.sub(4294967295, amount)))

def transfer(before: Balances, +amount: U32) -> Decision:
  match before:
    case Balances{+debit, credit}:
      transfer_funds(debit, credit, amount, U32.is_gt(amount, debit))

law case_0:
  {transfer(Balances{9, 4}, 3) == Decision{6, 7, 0} : Decision}

def case_0():
  {==}

law case_1:
  {transfer(Balances{4294967295, 0}, 4294967295) == Decision{0, 4294967295, 0} : Decision}

def case_1():
  {==}

law case_2:
  {transfer(Balances{4294967295, 4294967295}, 0) == Decision{4294967295, 4294967295, 0} : Decision}

def case_2():
  {==}

law case_3:
  {transfer(Balances{0, 0}, 1) == Decision{0, 0, 1} : Decision}

def case_3():
  {==}

law case_4:
  {transfer(Balances{1, 4294967295}, 1) == Decision{1, 4294967295, 2} : Decision}

def case_4():
  {==}

law case_5:
  {transfer(Balances{0, 4294967295}, 1) == Decision{0, 4294967295, 1} : Decision}

def case_5():
  {==}

law case_6:
  {transfer(Balances{2147483648, 2147483647}, 1) == Decision{2147483647, 2147483648, 0} : Decision}

def case_6():
  {==}

def main() -> IO(Unit):
  IO.print("full-u32-balance-controls-pass")
