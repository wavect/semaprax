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

def count_step_swap(a: Bool, b: Bool, n: Nat) -> {count_step(a, count_step(b, n)) == count_step(b, count_step(a, n)) : Nat}:
  match a b:
    case False{} False{}: {==}
    case False{} True{}: {==}
    case True{} False{}: {==}
    case True{} True{}: {==}

law count_insert.fin:
  for +probe : U32
  for +x : U32
  for +h : U32
  for +t : List<&2, U32>
  for rec : {count(probe, Sort.insert(x, t)) == count_step(U32.is_eq(probe, x), count(probe, t)) : Nat}
  for c : Bool
  {count(probe, Sort.insert_choice(x, h, t, Sort.insert(x, t), c)) == count_step(U32.is_eq(probe, x), count(probe, h <> t)) : Nat}

def count_insert.fin(probe, x, h, t, rec, c):
  match c:
    case True{}: {==}
    case False{}:
      +first = Equal.cong(Nat, Nat, count_step(U32.is_eq(probe, h)), count(probe, Sort.insert(x, t)), count_step(U32.is_eq(probe, x), count(probe, t)), rec)
      +second = count_step_swap(U32.is_eq(probe, h), U32.is_eq(probe, x), count(probe, t))
      Equal.trans(Nat, count_step(U32.is_eq(probe, h), count(probe, Sort.insert(x, t))), count_step(U32.is_eq(probe, h), count_step(U32.is_eq(probe, x), count(probe, t))), count_step(U32.is_eq(probe, x), count_step(U32.is_eq(probe, h), count(probe, t))), first, second)

law count_insert:
  for +probe : U32
  for +x : U32
  for xs : List<&2, U32>
  {count(probe, Sort.insert(x, xs)) == count_step(U32.is_eq(probe, x), count(probe, xs)) : Nat}

def count_insert(probe, x, xs):
  match xs:
    case Nil{}: {==}
    case +h <> +t:
      count_insert.fin(probe, x, h, t, count_insert(probe, x, t), U32.is_le(x, h))

law count_sort:
  for +probe : U32
  for +xs : List<&2, U32>
  {count(probe, Sort.sort(xs)) == count(probe, xs) : Nat}

def count_sort(probe, xs):
  match xs:
    case Nil{}: {==}
    case h <> t:
      +first = count_insert(probe, h, Sort.sort(t))
      +second = Equal.cong(Nat, Nat, count_step(U32.is_eq(probe, h)), count(probe, Sort.sort(t)), count(probe, t), count_sort(probe, t))
      Equal.trans(Nat, count(probe, Sort.insert(h, Sort.sort(t))), count_step(U32.is_eq(probe, h), count(probe, Sort.sort(t))), count_step(U32.is_eq(probe, h), count(probe, t)), first, second)
