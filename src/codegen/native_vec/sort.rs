//! In-place heapsort over authenticated Copy-scalar vector storage.
pub(super) fn emit_runtime(output: &mut impl super::super::COutput) {
    output.push_str(RUNTIME);
}
const RUNTIME: &str = r#"
static __attribute__((unused)) uint64_t spx_vec_sort_key(uint32_t tag, uint64_t b) {
    if (tag == UINT32_C(1)) return b ^ UINT64_C(0x8000000000000000);
    if (tag == UINT32_C(2)) return (b & UINT64_C(0xffffffff)) ^ UINT64_C(0x80000000);
    if (tag == UINT32_C(6)) { b &= UINT64_C(0xffffffff); return (b & UINT64_C(0x80000000)) ? (~b & UINT64_C(0xffffffff)) : (b ^ UINT64_C(0x80000000)); }
    if (tag == UINT32_C(7)) return (b & UINT64_C(0x8000000000000000)) ? ~b : (b ^ UINT64_C(0x8000000000000000));
    return b;
}
static __attribute__((unused)) void spx_vec_sort_sift(uint64_t *v, uint64_t root, uint64_t n, uint32_t tag) {
    while (root < n / UINT64_C(2)) {
        uint64_t child = root * UINT64_C(2) + UINT64_C(1);
        if (child + UINT64_C(1) < n && spx_vec_sort_key(tag,v[child]) < spx_vec_sort_key(tag,v[child+UINT64_C(1)])) ++child;
        if (spx_vec_sort_key(tag,v[root]) >= spx_vec_sort_key(tag,v[child])) return;
        uint64_t swap=v[root];v[root]=v[child];v[child]=swap;root=child;
    }
}
static __attribute__((unused)) spx_status_token spx_vec_sort(struct spx_context *c,uint32_t tag,spx_vec_v1 *s,spx_vec_v1 *r) {
    if (tag < UINT32_C(1) || tag > UINT32_C(8)) spx_runtime_invariant_failure("Vec sort requires Copy scalars");
    struct spx_vec_authority_entry *e=spx_vec_require_valid(c,s,tag);
    if(r==NULL||r==s)spx_runtime_invariant_failure("invalid Vec sort result");
    uint64_t *v=(uint64_t *)(void *)s->ptr;
    for(uint64_t i=s->len/UINT64_C(2);i>UINT64_C(0);--i)spx_vec_sort_sift(v,i-UINT64_C(1),s->len,tag);
    for(uint64_t n=s->len;n>UINT64_C(1);--n){uint64_t swap=v[0];v[0]=v[n-UINT64_C(1)];v[n-UINT64_C(1)]=swap;spx_vec_sort_sift(v,UINT64_C(0),n-UINT64_C(1),tag);}
    s->generation=spx_vec_next_generation(c);e->generation=s->generation;
    *r=*s;*s=(spx_vec_v1){0};return SPX_STATUS_SUCCESS;
}
"#;
