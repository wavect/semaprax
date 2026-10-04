# Bounded balance-transfer witness: 20 - 7 = 13, 30 + 7 = 37, total = 50.
# All values are below 101, so this fixed witness cannot observe U32 wrapping.
import Base

type Balances is Data:
  Balances{debit: U32, credit: U32}

def transfer() -> Balances:
  Balances{U32.sub(20, 7), U32.add(30, 7)}

def debit_after() -> U32:
  U32.sub(20, 7)

def credit_after() -> U32:
  U32.add(30, 7)

law debit_after_exact:
  {debit_after() == 13 : U32}

def debit_after_exact():
  {==}

law credit_after_exact:
  {credit_after() == 37 : U32}

def credit_after_exact():
  {==}

law total_after_exact:
  {U32.add(debit_after(), credit_after()) == 50 : U32}

def total_after_exact():
  {==}

def main() -> IO(Unit):
  do IO<Unit>:
    IO.print(U32.show(debit_after()))
    IO.print(U32.show(credit_after()))
    IO.print(U32.show(U32.add(debit_after(), credit_after())))

#|13
#|37
#|50
