; Mathematical representation bridge, NOT a proof of either source translator.
; Every a, b, n is an arbitrary 32-bit unsigned value: all 2^96 triples.
; Bend's primitive add/sub are 32-bit wrapping. SEMAPRAX i64 stores zero-extended
; values and uses signed comparisons plus checked arithmetic after the guards.
(set-logic QF_BV)
(declare-const a (_ BitVec 32))
(declare-const b (_ BitVec 32))
(declare-const n (_ BitVec 32))
(define-fun m32 () (_ BitVec 32) #xffffffff)
(define-fun m64 () (_ BitVec 64) #x00000000ffffffff)
(define-fun a64 () (_ BitVec 64) ((_ zero_extend 32) a))
(define-fun b64 () (_ BitVec 64) ((_ zero_extend 32) b))
(define-fun n64 () (_ BitVec 64) ((_ zero_extend 32) n))
(define-fun funds32 () Bool (bvugt n a))
(define-fun funds64 () Bool (bvsgt n64 a64))
(define-fun overflow32 () Bool (bvugt b (bvsub m32 n)))
(define-fun overflow64 () Bool (bvsgt b64 (bvsub m64 n64)))
(define-fun admitted () Bool (and (not funds32) (not overflow32)))
(define-fun debit32 () (_ BitVec 32) (ite admitted (bvsub a n) a))
(define-fun credit32 () (_ BitVec 32) (ite admitted (bvadd b n) b))
(define-fun admitted64 () Bool (and (not funds64) (not overflow64)))
(define-fun debit64 () (_ BitVec 64) (ite admitted64 (bvsub a64 n64) a64))
(define-fun credit64 () (_ BitVec 64) (ite admitted64 (bvadd b64 n64) b64))
(push)
(assert (not (and
  (= funds32 funds64)
  (= overflow32 overflow64)
  (= debit64 ((_ zero_extend 32) debit32))
  (= credit64 ((_ zero_extend 32) credit32))
  (bvule debit64 m64)
  (bvule credit64 m64)
  (= (bvadd debit64 credit64) (bvadd a64 b64))
  ; The comparison-only sort uses these exact same orders/equalities.
  (= (bvule a b) (bvsle a64 b64))
  (= (= a b) (= a64 b64))
  ; Positive successful transfer cannot be a no-op on either account.
  (=> (and admitted (not (= n #x00000000)))
      (and (not (= debit64 a64)) (not (= credit64 b64))))
)))
(check-sat) ; unsat: all above properties hold over the full scalar domain.
(pop)
(push)
; Deliberately omit the credit overflow guard: this false bridge has a model.
(assert (and (not funds32)
  (not (= ((_ zero_extend 32) (bvadd b n)) (bvadd b64 n64)))))
(check-sat) ; sat: wrapping arithmetic is not a valid substitute.
(pop)
(push)
; Deliberately narrow to signed i32 ordering: also a false bridge.
(assert (not (= (bvule a b) (bvsle a b))))
(check-sat) ; sat: the upper half of the u32 range must not be narrowed away.
(pop)
