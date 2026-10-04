; Universal, bounded-list equal-spec control for the full U32 value domain.
; a,b,c,d and query are arbitrary U32 values.  The first check therefore
; ranges over every input list of length four and every multiplicity query.
; This is a model-level proof of insertion-sort semantics, not a certificate
; that either language source was translated into this SMT term.
(set-logic QF_BV)
(declare-const a (_ BitVec 32))
(declare-const b (_ BitVec 32))
(declare-const c (_ BitVec 32))
(declare-const d (_ BitVec 32))
(declare-const query (_ BitVec 32))

(define-fun lo ((left (_ BitVec 32)) (right (_ BitVec 32))) (_ BitVec 32)
  (ite (bvule left right) left right))
(define-fun hi ((left (_ BitVec 32)) (right (_ BitVec 32))) (_ BitVec 32)
  (ite (bvule left right) right left))
; Sort b,c, then insert a, then insert d.  This is insertion sort unrolled
; over an arbitrary four-element U32 list.
(define-fun bc0 () (_ BitVec 32) (lo b c))
(define-fun bc1 () (_ BitVec 32) (hi b c))
(define-fun a_bc1_lo () (_ BitVec 32) (lo a bc1))
(define-fun abc0 () (_ BitVec 32) (lo bc0 a_bc1_lo))
(define-fun abc1 () (_ BitVec 32) (hi bc0 a_bc1_lo))
(define-fun abc2 () (_ BitVec 32) (hi a bc1))
(define-fun abcd2 () (_ BitVec 32) (lo abc2 d))
(define-fun abcd3 () (_ BitVec 32) (hi abc2 d))
(define-fun abcd1 () (_ BitVec 32) (lo abc1 abcd2))
(define-fun abcd2p () (_ BitVec 32) (hi abc1 abcd2))
(define-fun abcd0 () (_ BitVec 32) (lo abc0 abcd1))
(define-fun abcd1p () (_ BitVec 32) (hi abc0 abcd1))
(define-fun count4 ((w (_ BitVec 32)) (x (_ BitVec 32)) (y (_ BitVec 32)) (z (_ BitVec 32)) (needle (_ BitVec 32))) (_ BitVec 3)
  (bvadd (ite (= w needle) #b001 #b000)
         (bvadd (ite (= x needle) #b001 #b000)
                (bvadd (ite (= y needle) #b001 #b000) (ite (= z needle) #b001 #b000)))))
(push)
; `unsat`: all four output positions are ordered and every value's exact
; multiplicity is preserved.  `query` is arbitrary, so this is stronger than
; checking the witness values or list length alone.
(assert (or (not (bvule abcd0 abcd1p))
            (not (bvule abcd1p abcd2p))
            (not (bvule abcd2p abcd3))
            (not (= (count4 a b c d query)
                    (count4 abcd0 abcd1p abcd2p abcd3 query)))))
(check-sat)
(pop)
(push)
; An empty output cannot preserve the multiplicity of a nonempty input.
(assert (= a query))
(assert (not (= (count4 a b c d query) #b000)))
(check-sat)
(pop)
