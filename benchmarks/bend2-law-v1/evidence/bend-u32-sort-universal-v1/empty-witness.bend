import Base
import ./empty-sort.bend as Sort

# Natural counts cannot wrap at the machine word boundary.
def count_step(hit: Bool, n: Nat) -> Nat:
  match hit:
    case False{}: n
    case True{}: 1n+n

def count(+probe: U32, xs: List<&2, U32>) -> Nat:
  match xs:
    case Nil{}: 0n
    case h <> t: count_step(U32.is_eq(probe, h), count(probe, t))

law empty_sort_loses_one:
  ({count(1, Sort.sort([1])) == 0n : Nat} & {count(1, [1]) == 1n : Nat})

def empty_sort_loses_one():
  ({==}, {==})
