//! The registry changes authority kind at Vec -> Iter; Vec APIs cannot accept it.
pub(super) const RUNTIME: &str = r#"
typedef struct {
    uint32_t spx_tag; uint32_t reserved;
    union { struct { spx_bytes_v1 ITER_ITEM; spx_iter_v1 ITER_REST; } ITER_CASE; } spx_payload;
} spx_iter_bytes_step_v2;
static __attribute__((unused)) struct spx_vec_authority_entry *spx_iter_bytes_valid(struct spx_context *c, const spx_iter_v1 *s) {
    if(c==NULL||c->state!=SPX_CONTEXT_INITIALIZED||s==NULL||s->vec.type_tag!=UINT32_C(10)||s->vec.authority==0||s->vec.authority>SPX_VEC_AUTHORITY_CAPACITY||s->cursor>s->vec.len||s->vec.len>s->vec.capacity||s->vec.capacity>SPX_VEC_MAX_CAPACITY||((s->vec.capacity==0)!=(s->vec.ptr==NULL))) spx_runtime_invariant_failure("invalid owning iterator carrier");
    struct spx_vec_authority_entry *e=&c->vec_authority[s->vec.authority-1];
    if(!e->live||e->type_tag!=UINT32_C(10)||e->ptr!=(void*)s->vec.ptr||e->capacity!=s->vec.capacity||e->generation!=s->vec.generation||e->len!=s->cursor||e->iterator_end!=s->vec.len) spx_runtime_invariant_failure("stale owning iterator window");
    return e;
}
static __attribute__((unused)) spx_iter_v1 spx_iter_bytes_from_vec(struct spx_context *c, spx_vec_v1 *s) {
    struct spx_vec_authority_entry *e=spx_vec_require_valid(c,s,UINT32_C(9));
    uint64_t generation=spx_vec_next_generation(c);
    spx_iter_v1 r={.vec=*s,.cursor=0};
    r.vec.type_tag=UINT32_C(10); r.vec.generation=generation;
    e->type_tag=UINT32_C(10); e->iterator_end=s->len; e->len=0; e->generation=generation;
    *s=(spx_vec_v1){0}; return r;
}
static __attribute__((unused)) spx_iter_v1 spx_iter_move(struct spx_context *c, spx_iter_v1 *s) {
    if(s!=NULL&&s->vec.type_tag==UINT32_C(10)) {
        (void)spx_iter_bytes_valid(c,s); spx_iter_v1 r=*s; *s=(spx_iter_v1){0}; return r;
    }
    return spx_iter_scalar_move(c,s);
}
static __attribute__((unused)) void spx_iter_drop(struct spx_context *c, spx_iter_v1 *s) {
    if(s!=NULL&&s->vec.type_tag==UINT32_C(10)) {
        struct spx_vec_authority_entry *e=spx_iter_bytes_valid(c,s);
        for(uint64_t i=s->cursor;i<s->vec.len;++i) spx_bytes_drop(&s->vec.ptr[i]);
        free(s->vec.ptr); *e=(struct spx_vec_authority_entry){0}; *s=(spx_iter_v1){0}; return;
    }
    spx_iter_scalar_drop(c,s);
}
static __attribute__((unused)) uint32_t spx_iter_bytes_next(struct spx_context *c, spx_iter_v1 *s, spx_iter_bytes_step_v2 *out) {
    struct spx_vec_authority_entry *e=spx_iter_bytes_valid(c,s);
    if(out==NULL) spx_runtime_invariant_failure("invalid owning iterator output");
    spx_iter_bytes_step_v2 r={0};
    if(s->cursor==s->vec.len) {spx_iter_drop(c,s);*out=r;return SPX_STATUS_SUCCESS;}
    uint64_t generation=spx_vec_next_generation(c);
    r.spx_payload.ITER_CASE.ITER_ITEM=spx_bytes_move(&s->vec.ptr[s->cursor]);
    r.spx_payload.ITER_CASE.ITER_REST=*s;
    r.spx_payload.ITER_CASE.ITER_REST.cursor++;
    r.spx_payload.ITER_CASE.ITER_REST.vec.generation=generation;
    e->len++; e->generation=generation;
    r.spx_tag=UINT32_C(1); *s=(spx_iter_v1){0}; *out=r;return SPX_STATUS_SUCCESS;
}
"#;
