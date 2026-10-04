# Universal all-finite-list source proof for the exact imported U32 sort.
# ULE follows the same Word.cmp ordering as U32.is_le; branch equalities
# connect order evidence to the implementation's Boolean dispatch.
import Base
import ./sort.bend as Sort

def Flip(c: Cmp) -> Cmp:
  match c:
    case LT{}: GT{}
    case EQ{}: EQ{}
    case GT{}: LT{}

def cmp_fin_swap(a: Bool, b: Bool, c: Cmp) -> {Word.cmp.fin(a, b, c) == Flip(Word.cmp.fin(b, a, Flip(c))) : Cmp}:
  match a b c:
    case False{} False{} LT{}: {==}
    case False{} False{} EQ{}: {==}
    case False{} False{} GT{}: {==}
    case False{} True{} LT{}: {==}
    case False{} True{} EQ{}: {==}
    case False{} True{} GT{}: {==}
    case True{} False{} LT{}: {==}
    case True{} False{} EQ{}: {==}
    case True{} False{} GT{}: {==}
    case True{} True{} LT{}: {==}
    case True{} True{} EQ{}: {==}
    case True{} True{} GT{}: {==}

def fin_flip(a: Bool, b: Bool, c: Cmp) -> Cmp:
  Flip(Word.cmp.fin(b, a, c))

def flip_flip(c: Cmp) -> {Flip(Flip(c)) == c : Cmp}:
  match c:
    case LT{}: {==}
    case EQ{}: {==}
    case GT{}: {==}

def word_cmp_swap(+n: Nat, +a: Word(n), +b: Word(n)) -> {Word.cmp(n, a, b) == Flip(Word.cmp(n, b, a)) : Cmp}:
  match n:
    case 0n:
      {==}
    case 1n+ +p:
      match a b:
        case WCon{ah, +at} WCon{bh, +bt}:
          +ih = word_cmp_swap(p, at, bt)
          +fih = Equal.cong(Cmp, Cmp, Flip, Word.cmp(p, at, bt), Flip(Word.cmp(p, bt, at)), ih)
          +inv = flip_flip(Word.cmp(p, bt, at))
          +e1 = cmp_fin_swap(ah, bh, Word.cmp(p, at, bt))
          +e2 = Equal.cong(Cmp, Cmp, fin_flip(ah, bh), Flip(Word.cmp(p, at, bt)), Flip(Flip(Word.cmp(p, bt, at))), fih)
          +e3 = Equal.cong(Cmp, Cmp, fin_flip(ah, bh), Flip(Flip(Word.cmp(p, bt, at))), Word.cmp(p, bt, at), inv)
          +e12 = Equal.trans(Cmp, Word.cmp.fin(ah, bh, Word.cmp(p, at, bt)), fin_flip(ah, bh, Flip(Word.cmp(p, at, bt))), fin_flip(ah, bh, Flip(Flip(Word.cmp(p, bt, at)))), e1, e2)
          Equal.trans(Cmp, Word.cmp.fin(ah, bh, Word.cmp(p, at, bt)), fin_flip(ah, bh, Flip(Flip(Word.cmp(p, bt, at)))), fin_flip(ah, bh, Word.cmp(p, bt, at)), e12, e3)

def ULE.cmp(c: Cmp) -> Data:
  match c:
    case LT{}: Unit
    case EQ{}: Unit
    case GT{}: Empty

def ULE(a: U32, b: U32) -> Data:
  match a b:
    case U32{aw} U32{bw}:
      ULE.cmp(Word.cmp(32n, aw, bw))

type UChoice<-a: U32, -b: U32> is Data:
  Before{e: ULE(a, b), branch: {U32.is_le(a, b) == True{} : Bool}}
  After{e: ULE(b, a), branch: {U32.is_le(a, b) == False{} : Bool}}

def IsGT(c: Cmp) -> Data:
  match c:
    case GT{}: Unit
    case LT{}: Empty
    case EQ{}: Empty

def gt_transport(r: Cmp, e: {GT{} == Flip(r) : Cmp}) -> IsGT(Flip(r)):
  %e : IsGT(_)
  Unit{}

def le_from_lt(c: Cmp, e: {c == LT{} : Cmp}) -> ULE.cmp(c):
  +back = Equal.sym(Cmp, c, LT{}, e)
  %back : ULE.cmp(_)
  Unit{}

def le_false_from_gt(c: Cmp, e: {c == GT{} : Cmp}) -> {Cmp.is_le(c) == False{} : Bool}:
  +back = Equal.sym(Cmp, c, GT{}, e)
  %back : {Cmp.is_le(_) == False{} : Bool}
  {==}

def le_from_eq(c: Cmp, e: {c == EQ{} : Cmp}) -> ULE.cmp(c):
  +back = Equal.sym(Cmp, c, EQ{}, e)
  %back : ULE.cmp(_)
  Unit{}

def le_true_from_lt(c: Cmp, e: {c == LT{} : Cmp}) -> {Cmp.is_le(c) == True{} : Bool}:
  +back = Equal.sym(Cmp, c, LT{}, e)
  %back : {Cmp.is_le(_) == True{} : Bool}
  {==}

def le_true_from_eq(c: Cmp, e: {c == EQ{} : Cmp}) -> {Cmp.is_le(c) == True{} : Bool}:
  +back = Equal.sym(Cmp, c, EQ{}, e)
  %back : {Cmp.is_le(_) == True{} : Bool}
  {==}

def u32_choice.gt(+a: Word(32n), +b: Word(32n), r: Cmp, same: {Word.cmp(32n, a, b) == GT{} : Cmp}, rev: {Word.cmp(32n, b, a) == r : Cmp}, e: {GT{} == Flip(r) : Cmp}) -> UChoice<U32{a}, U32{b}>:
  match r:
    case LT{}:
      After{le_from_lt(Word.cmp(32n, b, a), rev), le_false_from_gt(Word.cmp(32n, a, b), same)}
    case EQ{}:
      +bad = gt_transport(EQ{}, e)
      Empty.absurd(UChoice<U32{a}, U32{b}>, bad)
    case GT{}:
      +bad = gt_transport(GT{}, e)
      Empty.absurd(UChoice<U32{a}, U32{b}>, bad)

def u32_choice.fin(+a: Word(32n), +b: Word(32n), c: Cmp, +same: {Word.cmp(32n, a, b) == c : Cmp}, e: {c == Flip(Word.cmp(32n, b, a)) : Cmp}) -> UChoice<U32{a}, U32{b}>:
  match c:
    case LT{}:
      Before{le_from_lt(Word.cmp(32n, a, b), same), le_true_from_lt(Word.cmp(32n, a, b), same)}
    case EQ{}:
      Before{le_from_eq(Word.cmp(32n, a, b), same), le_true_from_eq(Word.cmp(32n, a, b), same)}
    case GT{}:
      u32_choice.gt(a, b, Word.cmp(32n, b, a), same, {==}, e)


def u32_choice(+a: U32, +b: U32) -> UChoice<a, b>:
  match a b:
    case U32{aw} U32{bw}:
      u32_choice.fin(aw, bw, Word.cmp(32n, aw, bw), {==}, word_cmp_swap(32n, aw, bw))

def transport_le(a: Cmp, b: Cmp, e: {a == b : Cmp}, w: ULE.cmp(a)) -> ULE.cmp(b):
  %e : ULE.cmp(_)
  w

def zero_word(n: Nat) -> Word(n):
  match n:
    case 0n: WNil{}
    case 1n+p: WCon{False{}, zero_word(p)}

def zero_word_le.fin(+p: Nat, b: Bool, t: Word(p), c: Cmp, same: {Word.cmp(p, zero_word(p), t) == c : Cmp}, ih: ULE.cmp(Word.cmp(p, zero_word(p), t))) -> ULE.cmp(Word.cmp.fin(False{}, b, c)):
  match b c:
    case False{} LT{}: Unit{}
    case False{} EQ{}: Unit{}
    case False{} GT{}:
      +bad = transport_le(Word.cmp(p, zero_word(p), t), GT{}, same, ih)
      Empty.absurd(ULE.cmp(Word.cmp.fin(False{}, False{}, GT{})), bad)
    case True{} LT{}: Unit{}
    case True{} EQ{}: Unit{}
    case True{} GT{}:
      +bad = transport_le(Word.cmp(p, zero_word(p), t), GT{}, same, ih)
      Empty.absurd(ULE.cmp(Word.cmp.fin(False{}, True{}, GT{})), bad)

def zero_word_le(+n: Nat, w: Word(n)) -> ULE.cmp(Word.cmp(n, zero_word(n), w)):
  match n:
    case 0n: Unit{}
    case 1n+ +p:
      match w:
        case WCon{b, +t}:
          zero_word_le.fin(p, b, t, Word.cmp(p, zero_word(p), t), {==}, zero_word_le(p, t))

def zero_le(x: U32) -> ULE(0, x):
  match x:
    case U32{w}:
      zero_word_le(32n, w)


def SortedB(xs: List<&2, U32>, lo: U32) -> Type:
  match xs:
    case Nil{}: Unit
    case h <> t: (ULE(lo, h) & SortedB(t, h))

law sorted_ins.fin:
  for -lo : U32
  for -x : U32
  for -h : U32
  for -t : List<&2, U32>
  for sx : SortedB(h <> t, lo)
  for lx : ULE(lo, x)
  for rec : (SortedB(t, h) -> ULE(h, x) -> SortedB(Sort.insert(x, t), h))
  for c : UChoice<x, h>
  SortedB(Sort.insert_choice(x, h, t, Sort.insert(x, t), U32.is_le(x, h)), lo)

def sorted_ins.fin(lo, x, h, t, sx, lx, rec, c):
  (lh, st) = sx
  match c:
    case Before{e, branch}:
      +back = Equal.sym(Bool, U32.is_le(x, h), True{}, branch)
      %back : SortedB(Sort.insert_choice(x, h, t, Sort.insert(x, t), _), lo)
      (lx, e, st)
    case After{e, branch}:
      +back = Equal.sym(Bool, U32.is_le(x, h), False{}, branch)
      %back : SortedB(Sort.insert_choice(x, h, t, Sort.insert(x, t), _), lo)
      (lh, rec(st, e))

law sorted_ins:
  for xs : List<&2, U32>
  for -lo : U32
  for +x : U32
  SortedB(xs, lo) -> ULE(lo, x) -> SortedB(Sort.insert(x, xs), lo)

def sorted_ins(xs, lo, x):
  match xs:
    case Nil{}:
      sx => lx => (lx, Unit{})
    case h <> t:
      sx => lx => sorted_ins.fin(lo, x, h, t, sx, lx, sorted_ins(t, h, x), u32_choice(x, h))

law sorted_sort:
  for +xs : List<&2, U32>
  SortedB(Sort.sort(xs), 0)

def sorted_sort(xs):
  match xs:
    case Nil{}: Unit{}
    case h <> t:
      sorted_ins(Sort.sort(t), 0, h)(sorted_sort(t), zero_le(h))

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
