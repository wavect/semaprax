//! Copy-record vectors reuse one authenticated scalar-word allocation.
pub(super) fn emit_runtime(output: &mut impl super::super::COutput) {
    output.push_str(RUNTIME);
}
const RUNTIME: &str = r#"
static __attribute__((unused)) struct spx_vec_authority_entry *spx_copy_vec_check(struct spx_context *c, const spx_vec_v1 *s, uint64_t width) {
    struct spx_vec_authority_entry *e = spx_vec_require_valid(c,s,UINT32_C(1));
    if(width==UINT64_C(0)||width>UINT64_C(8)||s->len%width||s->capacity%width)spx_runtime_invariant_failure("invalid Copy record Vec stride");
    return e;
}
static __attribute__((unused)) void spx_copy_vec_commit(struct spx_context *c, spx_vec_v1 *s, struct spx_vec_authority_entry *e, spx_vec_v1 *r) {
    if(r==NULL||r==s)spx_runtime_invariant_failure("invalid Copy record Vec result");
    s->generation=spx_vec_next_generation(c);e->generation=s->generation;e->len=s->len;
    *r=*s;*s=(spx_vec_v1){0};
}
static __attribute__((unused)) spx_status_token spx_copy_vec_new(struct spx_context*c,uint64_t width,uint64_t n,spx_vec_v1*r) {
    if(width==UINT64_C(0)||width>UINT64_C(8))spx_runtime_invariant_failure("invalid Copy record Vec width");
    if(n>UINT64_C(8192)/width)return spx_vec_failure(c,UINT32_C(3));
    return spx_vec_with_capacity(c,UINT32_C(1),n*width,r);
}
static __attribute__((unused)) spx_status_token spx_copy_vec_push(struct spx_context*c,spx_vec_v1*s,uint64_t width,const uint64_t*words,spx_vec_v1*r) {
    struct spx_vec_authority_entry*e=spx_copy_vec_check(c,s,width);
    if(s->len==s->capacity)return spx_vec_failure(c,UINT32_C(1));
    uint64_t*v=(uint64_t*)(void*)s->ptr;
    for(uint64_t k=0;k<width;++k)v[s->len+k]=words[k];
    s->len+=width;spx_copy_vec_commit(c,s,e,r);return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_copy_vec_set(struct spx_context*c,spx_vec_v1*s,uint64_t width,uint64_t i,const uint64_t*words,spx_vec_v1*r) {
    struct spx_vec_authority_entry*e=spx_copy_vec_check(c,s,width);
    if(i>=s->len/width)return spx_vec_failure(c,UINT32_C(2));
    uint64_t*v=(uint64_t*)(void*)s->ptr;
    for(uint64_t k=0;k<width;++k)v[i*width+k]=words[k];
    spx_copy_vec_commit(c,s,e,r);return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) uint64_t spx_copy_vec_key(uint32_t tag,uint64_t b) {
    if(tag==UINT32_C(1))return b^UINT64_C(0x8000000000000000);
    if(tag==UINT32_C(2))return (b&UINT64_C(0xffffffff))^UINT64_C(0x80000000);
    if(tag==UINT32_C(6)){b&=UINT64_C(0xffffffff);return (b&UINT64_C(0x80000000))?(~b&UINT64_C(0xffffffff)):(b^UINT64_C(0x80000000));}
    if(tag==UINT32_C(7))return (b&UINT64_C(0x8000000000000000))?~b:(b^UINT64_C(0x8000000000000000));
    return b;
}
static __attribute__((unused)) spx_status_token spx_copy_vec_sort(struct spx_context*c,spx_vec_v1*s,uint64_t width,const uint32_t*tags,spx_vec_v1*r) {
    struct spx_vec_authority_entry*e=spx_copy_vec_check(c,s,width);
    uint64_t*v=(uint64_t*)(void*)s->ptr;
    for(uint64_t i=1;i<s->len/width;++i){
        uint64_t row[8]={0};for(uint64_t k=0;k<width;++k)row[k]=v[i*width+k];
        uint64_t j=i;
        while(j>0){
            int order=0;
            for(uint64_t k=0;k<width&&!order;++k){uint64_t a=spx_copy_vec_key(tags[k],v[(j-1)*width+k]);uint64_t b=spx_copy_vec_key(tags[k],row[k]);order=(a>b)-(a<b);}
            if(order<=0)break;
            for(uint64_t k=0;k<width;++k)v[j*width+k]=v[(j-1)*width+k];
            --j;
        }
        for(uint64_t k=0;k<width;++k)v[j*width+k]=row[k];
    }
    spx_copy_vec_commit(c,s,e,r);return SPX_STATUS_SUCCESS;
}
"#;
